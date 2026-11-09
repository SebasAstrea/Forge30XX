"""Construcción del dataset de QLoRA para tool-calling (RF-03, ADR-0012, RNF-09).

Mezcla ejemplos de Glaive v2 y sintéticos, los baraja con semilla fija, separa
un 5% para evaluación y escribe `train.jsonl`, `eval.jsonl` y `manifest.json`.

Uso: ``python -m forge_ml.dataset --glaive <glaive-function-calling-v2.json> --out <dir>``
"""

from __future__ import annotations

import argparse
import hashlib
import json
import logging
import random
from collections import Counter
from collections.abc import Sequence
from dataclasses import asdict
from datetime import UTC, datetime
from pathlib import Path

from forge_ml.glaive import convert
from forge_ml.synthetic import generate
from forge_ml.toolcall_format import Example

logger = logging.getLogger("forge_ml.dataset")

DEFAULT_PUBLIC = 6_000
DEFAULT_SYNTHETIC = 3_000
EVAL_FRACTION = 0.05


def _sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as fh:
        for block in iter(lambda: fh.read(1 << 20), b""):
            digest.update(block)
    return digest.hexdigest()


def _public(glaive: Path, n: int, rng: random.Random) -> list[Example]:
    with glaive.open(encoding="utf-8") as fh:
        records = json.load(fh)
    examples = list(convert(records))
    if len(examples) < n:
        raise ValueError(f"glaive solo tiene {len(examples)} ejemplos válidos; se pidieron {n}")
    return rng.sample(examples, n)


def _write(path: Path, examples: Sequence[Example]) -> dict[str, object]:
    with path.open("w", encoding="utf-8") as fh:
        for e in examples:
            fh.write(json.dumps(asdict(e), ensure_ascii=False, sort_keys=True) + "\n")
    return {"rows": len(examples), "sha256": _sha256(path)}


def build(glaive: Path, n_public: int, n_synthetic: int, seed: int, out: Path) -> dict[str, object]:
    """Genera el dataset en `out` y devuelve el manifest (también en disco)."""
    rng = random.Random(seed)
    examples = _public(glaive, n_public, rng) + generate(n_synthetic, rng)
    rng.shuffle(examples)
    n_eval = max(1, round(len(examples) * EVAL_FRACTION))
    out.mkdir(parents=True, exist_ok=True)
    manifest: dict[str, object] = {
        "created": datetime.now(UTC).isoformat(timespec="seconds"),
        "seed": seed,
        "inputs": {"glaive": {"path": glaive.name, "sha256": _sha256(glaive)}},
        "counts": dict(sorted(Counter(e.source for e in examples).items())),
        "outputs": {
            "train.jsonl": _write(out / "train.jsonl", examples[n_eval:]),
            "eval.jsonl": _write(out / "eval.jsonl", examples[:n_eval]),
        },
    }
    (out / "manifest.json").write_text(json.dumps(manifest, indent=2) + "\n", encoding="utf-8")
    logger.info("dataset en %s: %s", out, manifest["counts"])
    return manifest


def main(argv: list[str] | None = None) -> int:
    """CLI de construcción del dataset."""
    parser = argparse.ArgumentParser(prog="forge_ml.dataset", description=__doc__)
    parser.add_argument("--glaive", type=Path, required=True)
    parser.add_argument("--out", type=Path, required=True)
    parser.add_argument("--n-public", type=int, default=DEFAULT_PUBLIC)
    parser.add_argument("--n-synthetic", type=int, default=DEFAULT_SYNTHETIC)
    parser.add_argument("--seed", type=int, default=42)
    args = parser.parse_args(argv)
    logging.basicConfig(level=logging.INFO)
    build(args.glaive, args.n_public, args.n_synthetic, args.seed, args.out)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
