"""Tests del entrypoint de entrenamiento QLoRA (RF-03, RF-03b, RNF-09).

La parte de GPU (`forge_ml.qlora`) no se prueba aquí: se ejercita con
`make train-lora` en hardware real (AGENTS.md 11.11). Aquí se sustituye por un
doble que devuelve métricas fijas.
"""

from __future__ import annotations

import json
from pathlib import Path

import pytest

from forge_ml import train
from forge_ml.train import TrainConfig, load_config, main, write_manifest

_REPO_CONFIG = Path(__file__).resolve().parents[2] / "forge.config.toml"

_TOML = """
[training]
dataset = "ds"
seed = 7

[training.A]
base_model = "org/model-a"
revision = "abc123"
max_seq_len = 512
batch_size = 2
grad_accum = 4
lora_r = 8
lora_alpha = 16
learning_rate = 0.0002
epochs = 1
"""


def _config_file(tmp_path: Path) -> Path:
    path = tmp_path / "forge.config.toml"
    path.write_text(_TOML, encoding="utf-8")
    return path


def _dataset(tmp_path: Path) -> Path:
    ds = tmp_path / "ds"
    ds.mkdir()
    (ds / "manifest.json").write_text(json.dumps({"seed": 42, "counts": {"x": 1}}), "utf-8")
    return ds


def test_repo_config_profile_a_meets_rf03b_effective_batch() -> None:
    cfg = load_config(_REPO_CONFIG, "A", root=_REPO_CONFIG.parent)
    assert cfg.batch_size * cfg.grad_accum == 8
    assert cfg.base_model.endswith("bnb-4bit")
    assert len(cfg.revision) == 40


def test_repo_config_has_profile_b() -> None:
    cfg = load_config(_REPO_CONFIG, "B", root=_REPO_CONFIG.parent)
    assert "7B" in cfg.base_model


def test_load_config_resolves_dataset_against_root(tmp_path: Path) -> None:
    cfg = load_config(_config_file(tmp_path), "A", root=tmp_path)
    assert cfg == TrainConfig(
        profile="A",
        base_model="org/model-a",
        revision="abc123",
        dataset=tmp_path / "ds",
        seed=7,
        max_seq_len=512,
        batch_size=2,
        grad_accum=4,
        lora_r=8,
        lora_alpha=16,
        learning_rate=0.0002,
        epochs=1,
    )


def test_load_config_rejects_missing_profile(tmp_path: Path) -> None:
    with pytest.raises(ValueError, match="training.B"):
        load_config(_config_file(tmp_path), "B", root=tmp_path)


def test_write_manifest_records_seed_dataset_and_base_hash(tmp_path: Path) -> None:
    cfg = load_config(_config_file(tmp_path), "A", root=tmp_path)
    _dataset(tmp_path)
    run = tmp_path / "run"
    run.mkdir()
    write_manifest(run, cfg, {"train_loss": 0.5})
    manifest = json.loads((run / "manifest.json").read_text(encoding="utf-8"))
    assert manifest["seed"] == 7
    assert manifest["base_model"] == {"repo": "org/model-a", "revision": "abc123"}
    assert manifest["dataset"]["manifest"]["counts"] == {"x": 1}
    assert manifest["metrics"] == {"train_loss": 0.5}
    assert manifest["config"]["lora_r"] == 8


def test_main_dry_run_does_not_train(tmp_path: Path, monkeypatch: pytest.MonkeyPatch) -> None:
    def boom(*_: object) -> dict[str, float]:
        raise AssertionError("no debe entrenar en dry-run")

    monkeypatch.setattr(train, "_run_training", boom)
    args = ["--profile", "A", "--config", str(_config_file(tmp_path)), "--dry-run"]
    assert main([*args, "--root", str(tmp_path)]) == 0


def test_main_trains_and_writes_manifest(tmp_path: Path, monkeypatch: pytest.MonkeyPatch) -> None:
    _dataset(tmp_path)
    seen: list[tuple[TrainConfig, Path, int | None]] = []

    def fake(cfg: TrainConfig, run_dir: Path, max_steps: int | None) -> dict[str, float]:
        seen.append((cfg, run_dir, max_steps))
        return {"train_loss": 0.25}

    monkeypatch.setattr(train, "_run_training", fake)
    out = tmp_path / "runs"
    args = ["--profile", "A", "--config", str(_config_file(tmp_path)), "--root", str(tmp_path)]
    assert main([*args, "--out", str(out), "--max-steps", "5"]) == 0
    [(cfg, run_dir, steps)] = seen
    assert steps == 5 and cfg.profile == "A"
    assert run_dir.parent == out and run_dir.name.startswith("A-")
    manifest = json.loads((run_dir / "manifest.json").read_text(encoding="utf-8"))
    assert manifest["metrics"] == {"train_loss": 0.25}
    assert manifest["max_steps"] == 5
