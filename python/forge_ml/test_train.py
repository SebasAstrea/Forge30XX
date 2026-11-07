"""Tests del entrypoint de entrenamiento."""

from __future__ import annotations

from forge_ml.train import main


def test_main_accepts_profile_a() -> None:
    assert main(["--profile", "A"]) == 0


def test_main_accepts_profile_b() -> None:
    assert main(["--profile", "B"]) == 0
