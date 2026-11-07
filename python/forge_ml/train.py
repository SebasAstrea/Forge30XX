"""Entrypoint de entrenamiento QLoRA. Invocado por `make train-lora PROFILE=A|B`.

La lógica real de entrenamiento (Unsloth en Perfil A, device_map="auto" en B) se
implementa en Sprint S4/S5 (RF-03, RF-03b). Aquí solo se define la CLI y el
esqueleto que parsea el perfil, sin dependencias pesadas de ML.
"""

from __future__ import annotations

import argparse
import logging

logger = logging.getLogger("forge_ml.train")


def main(argv: list[str] | None = None) -> int:
    """Parsea argumentos y despacha el entrenamiento según el perfil."""
    parser = argparse.ArgumentParser(prog="forge_ml.train")
    parser.add_argument("--profile", choices=["A", "B"], default="A")
    args = parser.parse_args(argv)

    logging.basicConfig(level=logging.INFO)
    logger.info("train-lora solicitado para perfil %s", args.profile)
    logger.warning("entrenamiento real pendiente: RF-03/RF-03b (Sprint S4/S5)")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
