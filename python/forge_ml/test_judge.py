"""Tests del LLM-juez de calidad KV (AGENTS.md 11.5, ADR-0009).

El llama-server y el CLI del juez se sustituyen por dobles locales: aquí se
prueba el cableado, no la calidad del modelo.
"""

from __future__ import annotations

import json
import sys
import threading
from collections.abc import Iterator
from http.server import BaseHTTPRequestHandler, HTTPServer
from pathlib import Path

import pytest

from forge_ml.judge import (
    QUESTIONS,
    Answer,
    JudgeError,
    build_tasks,
    generate,
    load_answers,
    main,
    parse_score,
    score,
)


class _Chat(BaseHTTPRequestHandler):
    def log_message(self, *_: object) -> None:
        pass

    def do_POST(self) -> None:  # noqa: N802 (nombre impuesto por la stdlib)
        body = json.loads(self.rfile.read(int(self.headers["Content-Length"])))
        question = body["messages"][-1]["content"].splitlines()[-1]
        reply = {"choices": [{"message": {"content": f"resp: {question}"}}]}
        data = json.dumps(reply).encode()
        self.send_response(200)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(data)))
        self.end_headers()
        self.wfile.write(data)


@pytest.fixture()
def llama_url() -> Iterator[str]:
    server = HTTPServer(("127.0.0.1", 0), _Chat)
    threading.Thread(target=server.serve_forever, daemon=True).start()
    yield f"http://127.0.0.1:{server.server_address[1]}"
    server.shutdown()


def _fake_judge(tmp_path: Path, text: str) -> list[str]:
    """CLI falso que imita `opencode run --format json` con el texto dado."""
    event = {"type": "text", "part": {"type": "text", "text": text}}
    script = tmp_path / "fake_judge.py"
    script.write_text(f"print({json.dumps(json.dumps(event))})\n", encoding="utf-8")
    return [sys.executable, str(script)]


def _context(tmp_path: Path, n: int) -> list[Path]:
    files = []
    for i in range(n):
        f = tmp_path / f"f{i}.rs"
        f.write_text(f"fn f{i}() {{}}\n" * 50, encoding="utf-8")
        files.append(f)
    return files


def test_build_tasks_crosses_files_and_questions(tmp_path: Path) -> None:
    tasks = build_tasks(_context(tmp_path, 2), max_chars=100)
    assert len(tasks) == 2 * len(QUESTIONS)
    assert all(len(t.context) <= 100 for t in tasks)
    assert len({t.task_id for t in tasks}) == len(tasks)


def test_build_tasks_requires_files() -> None:
    with pytest.raises(JudgeError):
        build_tasks([], max_chars=100)


def test_generate_writes_one_answer_per_task(tmp_path: Path, llama_url: str) -> None:
    tasks = build_tasks(_context(tmp_path, 1), max_chars=200)
    out = tmp_path / "base.jsonl"
    generate(tasks, llama_url, out, max_tokens=32)
    answers = load_answers(out)
    assert list(answers) == [t.task_id for t in tasks]
    assert answers[tasks[0].task_id].text == f"resp: {tasks[0].question}"


def test_generate_reports_unreachable_server(tmp_path: Path) -> None:
    tasks = build_tasks(_context(tmp_path, 1), max_chars=200)
    with pytest.raises(JudgeError):
        generate(tasks, "http://127.0.0.1:9", tmp_path / "x.jsonl", max_tokens=8)


@pytest.mark.parametrize(
    ("text", "expected"),
    [
        ('{"score": 4, "reason": "ok"}', 4),
        ('Veredicto:\n```json\n{"reason": "bien", "score": 5}\n```', 5),
    ],
)
def test_parse_score_extracts_json(text: str, expected: int) -> None:
    assert parse_score(text) == expected


@pytest.mark.parametrize("text", ["sin json", '{"score": 7}', '{"score": "x"}'])
def test_parse_score_rejects_invalid(text: str) -> None:
    with pytest.raises(JudgeError):
        parse_score(text)


def _answers(n: int) -> dict[str, Answer]:
    return {f"t{i}": Answer(f"t{i}", f"pregunta {i}", f"respuesta {i}") for i in range(n)}


def test_score_averages_judge_verdicts(tmp_path: Path) -> None:
    cmd = _fake_judge(tmp_path, '{"score": 4, "reason": "equivalente"}')
    report = score(_answers(3), _answers(3), cmd, model="m", threshold=4.0)
    assert report.mean == 4.0
    assert report.passed
    assert [v.score for v in report.verdicts] == [4, 4, 4]


def test_score_fails_below_threshold(tmp_path: Path) -> None:
    cmd = _fake_judge(tmp_path, '{"score": 3, "reason": "pierde detalle"}')
    report = score(_answers(2), _answers(2), cmd, model="m", threshold=4.0)
    assert not report.passed


def test_score_requires_matching_tasks(tmp_path: Path) -> None:
    cmd = _fake_judge(tmp_path, '{"score": 5}')
    with pytest.raises(JudgeError):
        score(_answers(2), _answers(1), cmd, model="m", threshold=4.0)


def _failing_judge(tmp_path: Path) -> list[str]:
    script = tmp_path / "failing_judge.py"
    script.write_text(
        'import sys\nprint(\'{"type":"error","error":"rate limit"}\')\nsys.exit(1)\n',
        encoding="utf-8",
    )
    return [sys.executable, str(script)]


def test_score_resumes_from_cache_without_calling_judge(tmp_path: Path) -> None:
    cache = tmp_path / "verdicts.jsonl"
    ok = _fake_judge(tmp_path, '{"score": 5, "reason": "igual"}')
    score(_answers(2), _answers(2), ok, model="m", threshold=4.0, cache=cache)
    # Con todo en caché, un juez que siempre falla no llega a invocarse.
    report = score(
        _answers(2), _answers(2), _failing_judge(tmp_path), model="m", threshold=4.0, cache=cache
    )
    assert report.mean == 5.0


def test_score_retries_then_reports_judge_output(tmp_path: Path) -> None:
    with pytest.raises(JudgeError, match="rate limit"):
        score(
            _answers(1),
            _answers(1),
            _failing_judge(tmp_path),
            model="m",
            threshold=4.0,
            retry_delays=(0.0, 0.0),
        )


def test_cli_score_writes_report_and_exit_code(tmp_path: Path) -> None:
    ref, cand = tmp_path / "ref.jsonl", tmp_path / "cand.jsonl"
    for path in (ref, cand):
        lines = [json.dumps(a.__dict__) for a in _answers(2).values()]
        path.write_text("\n".join(lines) + "\n", encoding="utf-8")
    out = tmp_path / "report.json"
    cmd = _fake_judge(tmp_path, '{"score": 5, "reason": "igual"}')
    args = ["score", "--reference", str(ref), "--candidate", str(cand), "--out", str(out)]
    assert main([*args, "--judge-cmd", *cmd]) == 0
    assert json.loads(out.read_text(encoding="utf-8"))["mean"] == 5.0
    cmd_bad = _fake_judge(tmp_path, '{"score": 2, "reason": "peor"}')
    assert main([*args, "--judge-cmd", *cmd_bad]) == 1


def test_cli_generate(tmp_path: Path, llama_url: str) -> None:
    ctx = _context(tmp_path, 1)
    out = tmp_path / "gen.jsonl"
    args = ["generate", "--url", llama_url, "--out", str(out), "--context", str(ctx[0])]
    assert main(args) == 0
    assert len(load_answers(out)) == len(QUESTIONS)
