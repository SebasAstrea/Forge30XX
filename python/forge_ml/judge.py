"""LLM-juez de calidad para cambios de caché KV (AGENTS.md 11.5, ADR-0009).

Flujo en dos pasos:

1. ``generate``: el llama-server responde 50 tareas (archivos de código x
   preguntas) con una configuración KV. Se ejecuta una vez con la referencia
   (f16/f16) y otra con la candidata (p. ej. f16/turbo3).
2. ``score``: un modelo externo, vía el CLI de opencode, puntúa de 1 a 5 cada
   respuesta candidata frente a la de referencia. Umbral: media >= 4.

El juez solo recibe la pregunta y las dos respuestas, no el código fuente.
"""

from __future__ import annotations

import argparse
import json
import logging
import subprocess
import tempfile
import time
import urllib.error
import urllib.request
from collections.abc import Sequence
from dataclasses import asdict, dataclass
from pathlib import Path

logger = logging.getLogger("forge_ml.judge")

QUESTIONS: tuple[str, ...] = (
    "Explica en 3-5 frases qué hace este código.",
    "Señala el posible bug o caso límite más importante y cómo corregirlo.",
    "Lista las funciones o tipos públicos y su propósito en una línea cada uno.",
    "Escribe un test unitario para la función más importante.",
    "Propón una mejora concreta de legibilidad con el fragmento reescrito.",
)

DEFAULT_JUDGE_CMD: tuple[str, ...] = ("opencode", "run", "--agent", "plan", "--format", "json")
DEFAULT_MODEL = "opencode/deepseek-v4.1-flash-free"
_SEED = 42
# Esperas entre reintentos del juez (los modelos gratuitos limitan la tasa).
_RETRY_DELAYS_S: tuple[float, ...] = (30.0, 120.0)
_JUDGE_TIMEOUT_S = 300

_JUDGE_PROMPT = """Eres un evaluador de asistentes de código. Compara la RESPUESTA
CANDIDATA con la RESPUESTA DE REFERENCIA a la misma pregunta sobre un archivo de
código. Puntúa la candidata de 1 a 5: 5 = igual de correcta y completa que la
referencia (o mejor); 4 = diferencias menores sin errores; 3 = omite partes
importantes; 2 = errores relevantes; 1 = incorrecta o incoherente.
No uses herramientas. Responde SOLO con JSON: {{"score": <1-5>, "reason": "<breve>"}}

PREGUNTA: {question}

RESPUESTA DE REFERENCIA:
{reference}

RESPUESTA CANDIDATA:
{candidate}
"""


class JudgeError(Exception):
    """Fallo al generar respuestas o al obtener un veredicto válido."""


@dataclass(frozen=True)
class Task:
    """Una pregunta sobre un fragmento de código."""

    task_id: str
    question: str
    context: str


@dataclass(frozen=True)
class Answer:
    """Respuesta del modelo local a una tarea."""

    task_id: str
    question: str
    text: str


@dataclass(frozen=True)
class Verdict:
    """Puntuación del juez para una tarea."""

    task_id: str
    score: int
    reason: str


@dataclass(frozen=True)
class Report:
    """Resultado agregado de la evaluación."""

    model: str
    mean: float
    threshold: float
    passed: bool
    verdicts: tuple[Verdict, ...]


def build_tasks(files: Sequence[Path], max_chars: int) -> list[Task]:
    """Cruza cada archivo con cada pregunta; recorta el contexto a `max_chars`."""
    if not files:
        raise JudgeError("se necesita al menos un archivo de contexto")
    tasks = []
    for f in files:
        context = f.read_text(encoding="utf-8", errors="replace")[:max_chars]
        for i, question in enumerate(QUESTIONS):
            tasks.append(Task(f"{f.name}#{i}", question, context))
    return tasks


def _chat(url: str, task: Task, max_tokens: int) -> str:
    body = {
        "messages": [
            {"role": "system", "content": "Eres un asistente de código conciso y correcto."},
            {"role": "user", "content": f"```\n{task.context}\n```\n\n{task.question}"},
        ],
        "temperature": 0,
        "seed": _SEED,
        "max_tokens": max_tokens,
    }
    req = urllib.request.Request(
        f"{url.rstrip('/')}/v1/chat/completions",
        data=json.dumps(body).encode("utf-8"),
        headers={"Content-Type": "application/json"},
    )
    try:
        with urllib.request.urlopen(req, timeout=600) as resp:
            payload = json.loads(resp.read())
    except (urllib.error.URLError, OSError) as e:
        raise JudgeError(f"llama-server en {url}: {e}") from e
    return str(payload["choices"][0]["message"]["content"])


def generate(tasks: Sequence[Task], url: str, out: Path, max_tokens: int) -> None:
    """Responde cada tarea con el llama-server y guarda las respuestas en JSONL."""
    with out.open("w", encoding="utf-8") as fh:
        for n, task in enumerate(tasks, 1):
            answer = Answer(task.task_id, task.question, _chat(url, task, max_tokens))
            fh.write(json.dumps(asdict(answer), ensure_ascii=False) + "\n")
            logger.info("generada %d/%d: %s", n, len(tasks), task.task_id)


def load_answers(path: Path) -> dict[str, Answer]:
    """Lee un JSONL de `generate`, indexado por `task_id`."""
    answers: dict[str, Answer] = {}
    with path.open(encoding="utf-8") as fh:
        for line in fh:
            if line.strip():
                a = Answer(**json.loads(line))
                answers[a.task_id] = a
    return answers


def parse_score(text: str) -> int:
    """Extrae `score` (1-5) del último objeto JSON del texto del juez."""
    start, end = text.rfind("{"), text.rfind("}")
    if start < 0 or end < start:
        raise JudgeError(f"el juez no devolvió JSON: {text[:200]!r}")
    try:
        value = json.loads(text[start : end + 1])["score"]
    except (json.JSONDecodeError, KeyError) as e:
        raise JudgeError(f"JSON del juez inválido: {text[:200]!r}") from e
    if not isinstance(value, int) or not 1 <= value <= 5:
        raise JudgeError(f"score fuera de rango: {value!r}")
    return value


def _event_text(stdout: str) -> str:
    """Concatena los trozos `text` del stream JSON de `opencode run --format json`."""
    parts = []
    for line in stdout.splitlines():
        try:
            event = json.loads(line)
        except json.JSONDecodeError:
            continue
        part = event.get("part", {}) if isinstance(event, dict) else {}
        if event.get("type") == "text" and isinstance(part.get("text"), str):
            parts.append(part["text"])
    return "".join(parts)


def _ask_judge(judge_cmd: Sequence[str], model: str, prompt: str) -> str:
    # Directorio vacío como cwd: el agente del juez no ve ni toca el repo.
    with tempfile.TemporaryDirectory(prefix="forge-judge-") as cwd:
        proc = subprocess.run(
            [*judge_cmd, "-m", model, prompt],
            cwd=cwd,
            capture_output=True,
            text=True,
            timeout=_JUDGE_TIMEOUT_S,
            check=False,
        )
    if proc.returncode != 0:
        detail = (proc.stderr or proc.stdout)[-300:]
        raise JudgeError(f"juez terminó con código {proc.returncode}: {detail}")
    return _event_text(proc.stdout)


def _verdict(
    judge_cmd: Sequence[str],
    model: str,
    pair: tuple[Answer, Answer],
    retry_delays: Sequence[float],
) -> Verdict:
    ref, cand = pair
    prompt = _JUDGE_PROMPT.format(question=ref.question, reference=ref.text, candidate=cand.text)
    for attempt, delay in enumerate([*retry_delays, None], 1):
        try:
            text = _ask_judge(judge_cmd, model, prompt)
            return Verdict(ref.task_id, parse_score(text), _reason(text))
        except (JudgeError, subprocess.TimeoutExpired) as e:
            logger.warning("juez %s intento %d: %s", ref.task_id, attempt, e)
            if delay is None:
                raise JudgeError(f"{ref.task_id}: {e}") from e
            time.sleep(delay)
    raise JudgeError("inalcanzable")


def _reason(text: str) -> str:
    try:
        return str(json.loads(text[text.rfind("{") : text.rfind("}") + 1]).get("reason", ""))
    except json.JSONDecodeError:
        return ""


def _load_cache(cache: Path | None) -> dict[str, Verdict]:
    if cache is None or not cache.exists():
        return {}
    with cache.open(encoding="utf-8") as fh:
        verdicts = (Verdict(**json.loads(line)) for line in fh if line.strip())
        return {v.task_id: v for v in verdicts}


def score(
    reference: dict[str, Answer],
    candidate: dict[str, Answer],
    judge_cmd: Sequence[str],
    model: str,
    threshold: float,
    cache: Path | None = None,
    retry_delays: Sequence[float] = _RETRY_DELAYS_S,
) -> Report:
    """Puntúa cada respuesta candidata frente a la de referencia.

    Con `cache`, cada veredicto se añade al JSONL al obtenerse y los ya
    presentes no se vuelven a pedir: una ejecución interrumpida se reanuda.
    """
    if set(reference) != set(candidate):
        raise JudgeError("referencia y candidata no cubren las mismas tareas")
    done = _load_cache(cache)
    verdicts = []
    for n, task_id in enumerate(sorted(reference), 1):
        v = done.get(task_id)
        if v is None:
            pair = (reference[task_id], candidate[task_id])
            v = _verdict(judge_cmd, model, pair, retry_delays)
            if cache is not None:
                with cache.open("a", encoding="utf-8") as fh:
                    fh.write(json.dumps(asdict(v), ensure_ascii=False) + "\n")
        logger.info("juzgada %d/%d: %s -> %d", n, len(reference), task_id, v.score)
        verdicts.append(v)
    mean = sum(v.score for v in verdicts) / len(verdicts)
    return Report(model, mean, threshold, mean >= threshold, tuple(verdicts))


def _parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(prog="forge_ml.judge", description=__doc__)
    sub = parser.add_subparsers(dest="command", required=True)
    gen = sub.add_parser("generate", help="responder las tareas con un llama-server")
    gen.add_argument("--url", default="http://127.0.0.1:8080")
    gen.add_argument("--out", type=Path, required=True)
    gen.add_argument("--context", type=Path, nargs="+", required=True)
    gen.add_argument("--max-chars", type=int, default=48_000)
    gen.add_argument("--max-tokens", type=int, default=384)
    sc = sub.add_parser("score", help="puntuar candidata frente a referencia")
    sc.add_argument("--reference", type=Path, required=True)
    sc.add_argument("--candidate", type=Path, required=True)
    sc.add_argument("--out", type=Path, required=True)
    sc.add_argument("--model", default=DEFAULT_MODEL)
    sc.add_argument("--threshold", type=float, default=4.0)
    sc.add_argument("--cache", type=Path, help="JSONL de veredictos para reanudar")
    sc.add_argument("--judge-cmd", nargs="+", default=list(DEFAULT_JUDGE_CMD))
    return parser


def main(argv: list[str] | None = None) -> int:
    """CLI: `generate` o `score`. `score` devuelve 1 si no se alcanza el umbral."""
    args = _parser().parse_args(argv)
    logging.basicConfig(level=logging.INFO)
    if args.command == "generate":
        tasks = build_tasks(args.context, args.max_chars)
        generate(tasks, args.url, args.out, args.max_tokens)
        return 0
    ref, cand = load_answers(args.reference), load_answers(args.candidate)
    report = score(ref, cand, args.judge_cmd, args.model, args.threshold, args.cache)
    args.out.write_text(json.dumps(asdict(report), ensure_ascii=False, indent=2), encoding="utf-8")
    logger.info("media %.2f (umbral %.1f): %s", report.mean, report.threshold, report.passed)
    return 0 if report.passed else 1


if __name__ == "__main__":
    raise SystemExit(main())
