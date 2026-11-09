"""Conversión de Glaive function-calling v2 (Apache-2.0) a nuestro formato (ADR-0012).

Cada registro tiene `system` (texto con las funciones como objetos JSON
concatenados) y `chat` (turnos `USER:` / `ASSISTANT:` / `FUNCTION RESPONSE:`).
Se toma el primer turno de usuario y la primera llamada `<functioncall>`.
"""

from __future__ import annotations

import json
import logging
import re
from collections.abc import Iterable, Iterator, Mapping

from forge_ml.toolcall_format import Example, is_grammar_safe, make_example

logger = logging.getLogger("forge_ml.glaive")

_FIRST_USER = re.compile(r"USER:\s*(.*?)\s*(?:\n\s*\n\s*\n|ASSISTANT:)", re.S)
_FIRST_CALL = re.compile(r"ASSISTANT:\s*<functioncall>\s*(\{.*?\})\s*(?:<\|endoftext\|>|$)", re.S)
_CALL_PARTS = re.compile(r'^\{"name":\s*"([^"]+)",\s*"arguments":\s*\'(.*)\'\}$', re.S)


def parse_functions(system: str) -> list[dict[str, object]]:
    """Objetos JSON con clave `name` concatenados en el texto `system`."""
    decoder = json.JSONDecoder()
    funcs: list[dict[str, object]] = []
    i = system.find("{")
    while i >= 0:
        try:
            obj, end = decoder.raw_decode(system, i)
        except json.JSONDecodeError:
            i = system.find("{", i + 1)
            continue
        if isinstance(obj, dict) and "name" in obj:
            funcs.append(obj)
        i = system.find("{", end)
    return funcs


def _call(chat: str) -> tuple[str, dict[str, object]] | None:
    m = _FIRST_CALL.search(chat)
    parts = _CALL_PARTS.match(m.group(1).strip()) if m else None
    if parts is None:
        return None
    try:
        arguments = json.loads(parts.group(2))
    except json.JSONDecodeError:
        return None
    return (parts.group(1), arguments) if isinstance(arguments, dict) else None


def _example(record: Mapping[str, str]) -> Example | None:
    chat = record.get("chat", "")
    user = _FIRST_USER.search(chat)
    call = _call(chat)
    if user is None or call is None or not user.group(1):
        return None
    name, arguments = call
    tools = parse_functions(record.get("system", ""))
    if not tools or not is_grammar_safe(name, arguments):
        return None
    return make_example(tools, user.group(1), name, arguments, "glaive")


def convert(records: Iterable[Mapping[str, str]]) -> Iterator[Example]:
    """Ejemplos válidos de los registros de Glaive; descarta el resto."""
    kept = skipped = 0
    for record in records:
        example = _example(record)
        if example is None:
            skipped += 1
            continue
        kept += 1
        yield example
    logger.info("glaive: %d convertidos, %d descartados", kept, skipped)
