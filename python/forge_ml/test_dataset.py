"""Tests del dataset de QLoRA para tool-calling (RF-03, ADR-0012, RNF-09)."""

from __future__ import annotations

import json
import random
import re
from pathlib import Path

import pytest

from forge_ml.dataset import build, main
from forge_ml.glaive import convert, parse_functions
from forge_ml.synthetic import PROJECT_TOOLS, generate
from forge_ml.toolcall_format import Example, is_grammar_safe, render_call, render_system

_RF08 = Path(__file__).resolve().parents[2] / "orchestrator" / "tests" / "rf08_tool_calls.rs"
_NAME = re.compile(r"[a-z][a-z0-9_]{0,63}")

_GLAIVE_SYSTEM = (
    "SYSTEM: You are a helpful assistant with access to the following functions. "
    'Use them if required -\n{\n "name": "get_weather",\n "description": "Weather",\n'
    ' "parameters": {"type": "object", "properties": {"city": {"type": "string"}}}\n}'
)
_GLAIVE_CHAT = (
    "USER: What's the weather in Lima?\n\n\nASSISTANT: <functioncall> "
    '{"name": "get_weather", "arguments": \'{"city": "Lima"}\'} <|endoftext|>\n\n\n'
    'FUNCTION RESPONSE: {"temp": 20}\n\n\nASSISTANT: It is 20C. <|endoftext|>'
)


def _rf08_prompts() -> list[str]:
    block = _RF08.read_text(encoding="utf-8").split("const PROMPTS")[1].split("];")[0]
    return [json.loads(f'"{m}"') for m in re.findall(r'^\s*"((?:[^"\\]|\\.)*)",', block, re.M)]


def _assistant(example: Example) -> dict[str, object]:
    call: dict[str, object] = json.loads(example.messages[-1]["content"])
    return call


# --- formato --------------------------------------------------------------


def test_render_call_matches_tool_call_shape() -> None:
    raw = render_call("read_file", {"path": "a.rs"})
    assert json.loads(raw) == {"name": "read_file", "arguments": {"path": "a.rs"}}


@pytest.mark.parametrize(
    ("arguments", "safe"),
    [
        ({"n": 3, "x": 0.5, "s": "año\nnueva"}, True),
        ({"n": 10**16}, False),
        ({"x": float("nan")}, False),
        ({"s": "del\x7f"}, False),
        ({"nested": {"list": [1, "a", None, True]}}, True),
    ],
)
def test_is_grammar_safe(arguments: dict[str, object], safe: bool) -> None:
    assert is_grammar_safe("read_file", arguments) is safe
    assert not is_grammar_safe("Read-File", {"x": 1})


def test_render_system_lists_every_tool() -> None:
    system = render_system(PROJECT_TOOLS)
    for tool in PROJECT_TOOLS:
        assert f'"{tool["name"]}"' in system


# --- sintético ------------------------------------------------------------


def test_synthetic_is_deterministic_and_unique() -> None:
    a = generate(300, random.Random(7))
    b = generate(300, random.Random(7))
    assert a == b
    assert len({e.messages[1]["content"] for e in a}) == 300


def test_synthetic_covers_all_project_tools_with_valid_calls() -> None:
    examples = generate(300, random.Random(1))
    names = {_assistant(e)["name"] for e in examples}
    assert names == {t["name"] for t in PROJECT_TOOLS}
    for e in examples:
        call = _assistant(e)
        assert isinstance(call["arguments"], dict)
        assert is_grammar_safe(str(call["name"]), call["arguments"])
        assert e.source == "synthetic"


def test_synthetic_never_contains_rf08_eval_prompts() -> None:
    prompts = _rf08_prompts()
    assert len(prompts) == 30
    generated = {e.messages[1]["content"] for e in generate(2000, random.Random(3))}
    assert generated.isdisjoint(prompts)


def test_synthetic_shell_commands_stay_in_allowlist() -> None:
    allow = {"ls", "cat", "grep", "find", "git", "cargo", "npm", "python"}
    for e in generate(500, random.Random(5)):
        call = _assistant(e)
        if call["name"] == "run_shell":
            args = call["arguments"]
            assert isinstance(args, dict)
            assert str(args["command"]).split()[0] in allow


# --- Glaive ---------------------------------------------------------------


def test_parse_functions_reads_concatenated_json_objects() -> None:
    funcs = parse_functions(_GLAIVE_SYSTEM + "\n{\n" + '"name": "b", "parameters": {}}')
    assert [f["name"] for f in funcs] == ["get_weather", "b"]


def test_convert_takes_first_user_turn_and_first_call() -> None:
    [ex] = list(convert([{"system": _GLAIVE_SYSTEM, "chat": _GLAIVE_CHAT}]))
    assert ex.source == "glaive"
    assert ex.messages[1] == {"role": "user", "content": "What's the weather in Lima?"}
    assert _assistant(ex) == {"name": "get_weather", "arguments": {"city": "Lima"}}
    assert '"get_weather"' in ex.messages[0]["content"]


@pytest.mark.parametrize(
    "chat",
    [
        "USER: hola\n\n\nASSISTANT: hola <|endoftext|>",
        'USER: x\n\n\nASSISTANT: <functioncall> {"name": "Bad-Name", "arguments": \'{}\'}',
        'USER: x\n\n\nASSISTANT: <functioncall> {"name": "f", "arguments": \'no json\'}',
    ],
)
def test_convert_skips_unusable_records(chat: str) -> None:
    assert list(convert([{"system": _GLAIVE_SYSTEM, "chat": chat}])) == []


# --- construcción y manifest ---------------------------------------------


def _glaive_file(tmp_path: Path, n: int) -> Path:
    path = tmp_path / "glaive.json"
    records = [
        {"system": _GLAIVE_SYSTEM, "chat": _GLAIVE_CHAT.replace("Lima", f"City{i}")}
        for i in range(n)
    ]
    path.write_text(json.dumps(records), encoding="utf-8")
    return path


def test_build_writes_split_and_manifest(tmp_path: Path) -> None:
    out = tmp_path / "ds"
    manifest = build(_glaive_file(tmp_path, 40), n_public=30, n_synthetic=30, seed=11, out=out)
    train = (out / "train.jsonl").read_text(encoding="utf-8").splitlines()
    evals = (out / "eval.jsonl").read_text(encoding="utf-8").splitlines()
    assert len(train) + len(evals) == 60
    assert manifest["counts"] == {"glaive": 30, "synthetic": 30}
    assert manifest["seed"] == 11
    on_disk = json.loads((out / "manifest.json").read_text(encoding="utf-8"))
    assert on_disk == json.loads(json.dumps(manifest))
    assert len(on_disk["outputs"]["train.jsonl"]["sha256"]) == 64
    assert len(str(on_disk["inputs"]["glaive"]["sha256"])) == 64


def test_build_is_reproducible(tmp_path: Path) -> None:
    glaive = _glaive_file(tmp_path, 40)
    m1 = build(glaive, n_public=20, n_synthetic=20, seed=5, out=tmp_path / "a")
    m2 = build(glaive, n_public=20, n_synthetic=20, seed=5, out=tmp_path / "b")
    assert m1["outputs"] == m2["outputs"]


def test_build_fails_when_public_source_is_short(tmp_path: Path) -> None:
    with pytest.raises(ValueError, match="glaive"):
        build(_glaive_file(tmp_path, 5), n_public=10, n_synthetic=1, seed=1, out=tmp_path / "x")


def test_cli_build(tmp_path: Path) -> None:
    out = tmp_path / "ds"
    glaive = _glaive_file(tmp_path, 10)
    args = ["--glaive", str(glaive), "--out", str(out), "--n-public", "8", "--n-synthetic", "8"]
    assert main(args) == 0
    lines = (out / "train.jsonl").read_text(encoding="utf-8").splitlines()
    row = json.loads(lines[0])
    assert [m["role"] for m in row["messages"]] == ["system", "user", "assistant"]
    assert _NAME.fullmatch(json.loads(row["messages"][2]["content"])["name"])
