"""Entrypoint de entrenamiento QLoRA. Invocado por `make train-lora PROFILE=A|B`.

Lee `[training]` y `[training.<perfil>]` de `forge.config.toml`, entrena con
`forge_ml.qlora` (Unsloth; requiere el extra `[ml]` y GPU) y escribe en
`artifacts/lora/<perfil>-<fecha>/` el adaptador en safetensors y un
`manifest.json` con semilla, dataset, modelo base y métricas (RNF-09).
"""

from __future__ import annotations

import argparse
import json
import logging
import os
import tomllib
from dataclasses import asdict, dataclass
from datetime import UTC, datetime
from pathlib import Path

logger = logging.getLogger("forge_ml.train")

_REPO_ROOT = Path(__file__).resolve().parents[2]


@dataclass(frozen=True)
class TrainConfig:
    """Hiperparámetros de un perfil (sección `[training.<perfil>]`)."""

    profile: str
    base_model: str
    revision: str
    dataset: Path
    seed: int
    max_seq_len: int
    batch_size: int
    grad_accum: int
    lora_r: int
    lora_alpha: int
    learning_rate: float
    epochs: int


def load_config(path: Path, profile: str, root: Path) -> TrainConfig:
    """Carga la configuración del perfil; `dataset` se resuelve contra `root`."""
    with path.open("rb") as fh:
        training = tomllib.load(fh).get("training", {})
    section = training.get(profile)
    if not isinstance(section, dict):
        raise ValueError(f"falta la sección [training.{profile}] en {path}")
    return TrainConfig(
        profile=profile,
        base_model=str(section["base_model"]),
        revision=str(section["revision"]),
        dataset=root / str(training["dataset"]),
        seed=int(training["seed"]),
        max_seq_len=int(section["max_seq_len"]),
        batch_size=int(section["batch_size"]),
        grad_accum=int(section["grad_accum"]),
        lora_r=int(section["lora_r"]),
        lora_alpha=int(section["lora_alpha"]),
        learning_rate=float(section["learning_rate"]),
        epochs=int(section["epochs"]),
    )


def write_manifest(
    run_dir: Path, cfg: TrainConfig, metrics: dict[str, float], max_steps: int | None = None
) -> None:
    """Manifest del entrenamiento: semilla, dataset, modelo base y métricas."""
    dataset_manifest = json.loads((cfg.dataset / "manifest.json").read_text(encoding="utf-8"))
    config = {k: str(v) if isinstance(v, Path) else v for k, v in asdict(cfg).items()}
    manifest = {
        "created": datetime.now(UTC).isoformat(timespec="seconds"),
        "profile": cfg.profile,
        "seed": cfg.seed,
        "base_model": {"repo": cfg.base_model, "revision": cfg.revision},
        "dataset": {"path": str(cfg.dataset), "manifest": dataset_manifest},
        "config": config,
        "max_steps": max_steps,
        "metrics": metrics,
    }
    (run_dir / "manifest.json").write_text(json.dumps(manifest, indent=2) + "\n", "utf-8")


def _run_training(cfg: TrainConfig, run_dir: Path, max_steps: int | None) -> dict[str, float]:
    # Sin telemetría de Hugging Face/TRL: solo se descarga el modelo base (RNF-06).
    os.environ.setdefault("HF_HUB_DISABLE_TELEMETRY", "1")
    # Import diferido: torch/unsloth solo existen en el entorno con el extra [ml].
    from forge_ml import qlora

    return qlora.run(cfg, run_dir, max_steps)


def main(argv: list[str] | None = None) -> int:
    """Parsea argumentos y entrena el perfil indicado."""
    parser = argparse.ArgumentParser(prog="forge_ml.train")
    parser.add_argument("--profile", choices=["A", "B"], default="A")
    parser.add_argument("--root", type=Path, default=_REPO_ROOT)
    parser.add_argument("--config", type=Path, default=None)
    parser.add_argument("--out", type=Path, default=None)
    parser.add_argument("--max-steps", type=int, default=None, help="limita los pasos (prueba)")
    parser.add_argument("--dry-run", action="store_true", help="valida la config y sale")
    args = parser.parse_args(argv)
    logging.basicConfig(level=logging.INFO)

    cfg = load_config(args.config or args.root / "forge.config.toml", args.profile, args.root)
    logger.info("train-lora perfil %s: %s@%s", cfg.profile, cfg.base_model, cfg.revision[:8])
    if args.dry_run:
        return 0
    stamp = datetime.now(UTC).strftime("%Y%m%dT%H%M%SZ")
    run_dir = (args.out or args.root / "artifacts" / "lora") / f"{cfg.profile}-{stamp}"
    run_dir.mkdir(parents=True)
    metrics = _run_training(cfg, run_dir, args.max_steps)
    write_manifest(run_dir, cfg, metrics, args.max_steps)
    logger.info("adaptador y manifest en %s: %s", run_dir, metrics)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
