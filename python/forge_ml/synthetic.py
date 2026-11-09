"""Ejemplos sintéticos para las tools del proyecto (ADR-0012).

Plantillas en español e inglés con valores aleatorios de pools fijos. Con la
misma semilla se obtiene exactamente el mismo conjunto. Los comandos de shell
respetan la lista blanca de `forge.config.toml` (RF-13).
"""

from __future__ import annotations

import random
from collections.abc import Callable

from forge_ml.toolcall_format import Example, make_example

_STR = {"type": "string"}

PROJECT_TOOLS: tuple[dict[str, object], ...] = (
    {
        "name": "read_file",
        "description": "Lee un archivo del workspace",
        "parameters": {"type": "object", "properties": {"path": _STR}, "required": ["path"]},
    },
    {
        "name": "edit_file",
        "description": "Sustituye el texto `old` por `new` en un archivo",
        "parameters": {
            "type": "object",
            "properties": {"path": _STR, "old": _STR, "new": _STR},
            "required": ["path", "old", "new"],
        },
    },
    {
        "name": "create_file",
        "description": "Crea un archivo con el contenido indicado",
        "parameters": {
            "type": "object",
            "properties": {"path": _STR, "content": _STR},
            "required": ["path", "content"],
        },
    },
    {
        "name": "delete_file",
        "description": "Borra un archivo (requiere aprobación del usuario)",
        "parameters": {"type": "object", "properties": {"path": _STR}, "required": ["path"]},
    },
    {
        "name": "search_web",
        "description": "Busca documentación en dominios permitidos",
        "parameters": {"type": "object", "properties": {"query": _STR}, "required": ["query"]},
    },
    {
        "name": "run_shell",
        "description": "Ejecuta un comando de la lista blanca con timeout",
        "parameters": {"type": "object", "properties": {"command": _STR}, "required": ["command"]},
    },
)

_DIRS = ("src", "lib", "app", "tests", "scripts", "docs", "pkg/core", "crates/net", "web/src")
_STEMS = ("parser", "server", "utils", "config", "client", "models", "router", "cache", "auth")
_EXTS = (".rs", ".py", ".ts", ".toml", ".md", ".json", ".go", ".svelte")
_IDENTS = ("load", "parse_args", "connect", "retry", "handler", "Config", "MAX_SIZE", "timeout")
_TOPICS = (
    "serde rename_all",
    "python dataclass frozen",
    "tokio mpsc channel",
    "typescript satisfies operator",
    "rust lifetimes in structs",
    "pytest fixtures scope",
    "svelte 5 runes",
    "git rebase interactive",
)
_SHELL = (
    "ls -la {dir}",
    "cat {path}",
    "grep -rn {ident} {dir}",
    "find {dir} -name '*{ext}'",
    "git status",
    "git diff {path}",
    "git log --oneline -5",
    "cargo test {ident}",
    "cargo clippy",
    "npm run lint",
    "npm test",
    "python -m pytest {path} -q",
)

_Gen = Callable[[random.Random], tuple[str, str, dict[str, object]]]


def _path(rng: random.Random) -> str:
    return f"{rng.choice(_DIRS)}/{rng.choice(_STEMS)}{rng.choice(_EXTS)}"


def _read(rng: random.Random) -> tuple[str, str, dict[str, object]]:
    p = _path(rng)
    t = rng.choice(("Necesito ver {p}", "Enséñame qué contiene {p}", "Show me {p}", "Open {p}"))
    return t.format(p=p), "read_file", {"path": p}


def _edit(rng: random.Random) -> tuple[str, str, dict[str, object]]:
    p, old, new = _path(rng), rng.choice(_IDENTS), rng.choice(_IDENTS) + "_v2"
    t = rng.choice(
        (
            "En {p} reemplaza {old} por {new}",
            "Replace {old} with {new} in {p}",
            "{p}: {old} -> {new}",
        )
    )
    return t.format(p=p, old=old, new=new), "edit_file", {"path": p, "old": old, "new": new}


def _create(rng: random.Random) -> tuple[str, str, dict[str, object]]:
    p, ident = _path(rng), rng.choice(_IDENTS)
    content = f"// {ident}\n" if p.endswith((".rs", ".ts", ".go")) else f"# {ident}\n"
    t = rng.choice(("Genera {p} con un comentario {i}", "Create {p} containing a {i} header"))
    return t.format(p=p, i=ident), "create_file", {"path": p, "content": content}


def _delete(rng: random.Random) -> tuple[str, str, dict[str, object]]:
    p = _path(rng)
    t = rng.choice(("Quita el archivo {p}", "Ya no necesito {p}, bórralo", "Delete {p}"))
    return t.format(p=p), "delete_file", {"path": p}


def _search(rng: random.Random) -> tuple[str, str, dict[str, object]]:
    q = rng.choice(_TOPICS)
    t = rng.choice(("Busca documentación sobre {q}", "Look up docs for {q}", "¿Cómo funciona {q}?"))
    return t.format(q=q), "search_web", {"query": q}


def _shell(rng: random.Random) -> tuple[str, str, dict[str, object]]:
    cmd = rng.choice(_SHELL).format(
        dir=rng.choice(_DIRS), path=_path(rng), ident=rng.choice(_IDENTS), ext=rng.choice(_EXTS)
    )
    t = rng.choice(("Lanza `{c}`", "Run `{c}` in the terminal", "Corre en la shell: {c}"))
    return t.format(c=cmd), "run_shell", {"command": cmd}


_GENERATORS: tuple[_Gen, ...] = (_read, _edit, _create, _delete, _search, _shell)


def generate(n: int, rng: random.Random, max_attempts_factor: int = 50) -> list[Example]:
    """`n` ejemplos únicos (por prompt de usuario), repartidos entre las tools."""
    seen: set[str] = set()
    examples: list[Example] = []
    for i in range(n * max_attempts_factor):
        if len(examples) == n:
            break
        user, name, args = _GENERATORS[i % len(_GENERATORS)](rng)
        if user not in seen:
            seen.add(user)
            examples.append(make_example(PROJECT_TOOLS, user, name, args, "synthetic"))
    if len(examples) < n:
        raise ValueError(f"solo {len(examples)} prompts sintéticos únicos de {n} pedidos")
    return examples
