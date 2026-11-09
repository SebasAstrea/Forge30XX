"""Formato común de los ejemplos de entrenamiento de tool-calling (ADR-0012).

La respuesta del asistente es exactamente lo que admite
`grammars/tool_calls.gbnf` y `orchestrator/src/tool_call.rs`: un objeto
`{"name": "<tool>", "arguments": {...}}`.
"""

from __future__ import annotations

import json
import math
import re
from collections.abc import Mapping, Sequence
from dataclasses import dataclass

_NAME = re.compile(r"[a-z][a-z0-9_]{0,63}")
# La gramática admite enteros de hasta 16 dígitos y rechaza el carácter DEL.
_MAX_ABS_NUMBER = 10**16

_SYSTEM_HEADER = (
    "Eres un agente de codificación. Responde SOLO con un tool-call JSON "
    '{"name": <tool>, "arguments": {...}} usando una de estas tools:\n'
)


@dataclass(frozen=True)
class Example:
    """Conversación de entrenamiento: system, user y assistant (el tool-call)."""

    messages: tuple[dict[str, str], ...]
    source: str


def render_system(tools: Sequence[Mapping[str, object]]) -> str:
    """System prompt con la lista de tools en JSON, una por línea."""
    lines = [json.dumps(dict(t), ensure_ascii=False, sort_keys=True) for t in tools]
    return _SYSTEM_HEADER + "\n".join(lines)


def render_call(name: str, arguments: Mapping[str, object]) -> str:
    """Tool-call serializado tal como debe emitirlo el modelo."""
    return json.dumps({"name": name, "arguments": dict(arguments)}, ensure_ascii=False)


def make_example(
    tools: Sequence[Mapping[str, object]],
    user: str,
    name: str,
    arguments: Mapping[str, object],
    source: str,
) -> Example:
    """Construye un ejemplo de tres mensajes."""
    return Example(
        messages=(
            {"role": "system", "content": render_system(tools)},
            {"role": "user", "content": user},
            {"role": "assistant", "content": render_call(name, arguments)},
        ),
        source=source,
    )


def _value_safe(value: object) -> bool:
    if isinstance(value, bool) or value is None:
        return True
    if isinstance(value, int):
        return abs(value) < _MAX_ABS_NUMBER
    if isinstance(value, float):
        return math.isfinite(value) and abs(value) < _MAX_ABS_NUMBER
    if isinstance(value, str):
        return "\x7f" not in value
    if isinstance(value, list):
        return all(_value_safe(v) for v in value)
    if isinstance(value, dict):
        return all(
            isinstance(k, str) and _value_safe(k) and _value_safe(v) for k, v in value.items()
        )
    return False


def is_grammar_safe(name: str, arguments: Mapping[str, object]) -> bool:
    """`True` si la gramática GBNF puede producir este tool-call."""
    return _NAME.fullmatch(name) is not None and _value_safe(dict(arguments))
