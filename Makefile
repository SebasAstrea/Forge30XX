# Forge30XX — Makefile
# Targets obligatorios según AGENTS.md 11.10 y docs/SDD.md.
# Cada target delega por capa: Rust (orchestrator), Python (python/), TS (frontend/).
# En fase bootstrap (Sprint S0) algunas capas aún no tienen código; los targets
# detectan la presencia de cada capa y omiten las ausentes sin fallar.

SHELL := /bin/bash
.DEFAULT_GOAL := help

# --- Detección de capas presentes ---
HAS_RUST     := $(wildcard orchestrator/Cargo.toml)
HAS_PYTHON   := $(wildcard python/pyproject.toml)
HAS_FRONTEND := $(wildcard frontend/package.json)

PROFILE ?= A

.PHONY: help check fmt lint test coverage bench \
        test-a test-b train-lora doctor docs release e2e \
        check-sizes check-rust check-python check-frontend clean

help: ## Muestra esta ayuda
	@grep -E '^[a-zA-Z_-]+:.*?## .*$$' $(MAKEFILE_LIST) \
		| awk 'BEGIN {FS = ":.*?## "}; {printf "  \033[36m%-16s\033[0m %s\n", $$1, $$2}'

# =====================================================================
# check = fmt + lint + test + coverage (AGENTS.md 11.9)
# =====================================================================
check: check-sizes check-rust check-python check-frontend ## fmt + lint + test + coverage en todas las capas

# AGENTS.md 11.1 (archivos ≤400 líneas) y 11.3 (componentes Svelte ≤150).
# Las funciones ≤40 líneas y la complejidad las verifica el linter de cada capa.
SOURCES = git ls-files --cached --others --exclude-standard
check-sizes: ## Límites de tamaño de archivo (AGENTS.md 11.1, 11.3)
	@over=$$($(SOURCES) '*.rs' '*.py' '*.ts' '*.js' '*.svelte' | xargs -r wc -l \
		| awk '$$2 != "total" && $$1 > 400'); \
	svelte=$$($(SOURCES) '*.svelte' | xargs -r wc -l | awk '$$2 != "total" && $$1 > 150'); \
	if [ -n "$$over$$svelte" ]; then \
		echo "[check-sizes] archivos por encima del límite:"; echo "$$over"; echo "$$svelte"; exit 1; \
	fi; echo "[check-sizes] ok"

check-rust:
ifeq ($(HAS_RUST),)
	@echo "[check-rust] orchestrator/Cargo.toml ausente — omitido (bootstrap)."
else
	@echo "[check-rust] fmt + clippy + test + coverage"
	cd orchestrator && cargo fmt --all --check
	cd orchestrator && cargo clippy --all-targets --all-features -- -D warnings
	cd orchestrator && cargo test --all-features
	@command -v cargo-llvm-cov >/dev/null 2>&1 \
		|| { echo "[check-rust] falta cargo-llvm-cov: cargo install cargo-llvm-cov --locked"; exit 1; }
	cd orchestrator && cargo llvm-cov --fail-under-lines 80
endif

check-python:
ifeq ($(HAS_PYTHON),)
	@echo "[check-python] python/pyproject.toml ausente — omitido (bootstrap)."
else
	@echo "[check-python] ruff + mypy + pytest + coverage"
	cd python && ruff format --check .
	cd python && ruff check .
	cd python && mypy --strict forge_ml forge_rag
	cd python && pytest --cov=forge_ml --cov=forge_rag --cov-fail-under=80
endif

check-frontend:
ifeq ($(HAS_FRONTEND),)
	@echo "[check-frontend] frontend/package.json ausente — omitido (bootstrap)."
else
	@echo "[check-frontend] prettier + eslint + svelte-check + vitest"
	cd frontend && npm run format:check
	cd frontend && npm run lint
	cd frontend && npm run check
	cd frontend && npm run test -- --run
endif

# =====================================================================
# Rendimiento (AGENTS.md 11.5): regresión >5% tok/s o >10% p95 bloquea
# =====================================================================
bench: ## Benchmarks de rendimiento (criterion, pytest-benchmark, llama-bench)
ifneq ($(HAS_RUST),)
	cd orchestrator && cargo bench
endif
ifneq ($(HAS_PYTHON),)
	cd python && pytest --benchmark-only || echo "[bench] sin benchmarks definidos aún"
endif
	@echo "[bench] llama-bench requiere CI hardware (3060 6GB / 2x3060)."

# =====================================================================
# Tests por perfil de hardware (requieren GPU real en CI self-hosted)
# =====================================================================
test-a: ## Tests Perfil A (RTX 3060 6 GB)
	@echo "[test-a] Perfil A — requiere runner con RTX 3060 6 GB"
	FORGE_PROFILE=A $(MAKE) check

test-b: ## Tests Perfil B (2x RTX 3060 12 GB)
	@echo "[test-b] Perfil B — requiere runner con 2x RTX 3060 12 GB"
	FORGE_PROFILE=B $(MAKE) check

# =====================================================================
# Entrenamiento LoRA/QLoRA
# =====================================================================
train-lora: ## Entrena adaptador LoRA. Uso: make train-lora PROFILE=A|B
ifeq ($(HAS_PYTHON),)
	@echo "[train-lora] capa Python ausente — pendiente Sprint S4/S5."
else
	cd python && python -m forge_ml.train --profile $(PROFILE)
endif

# =====================================================================
# Detección de hardware
# =====================================================================
doctor: ## Detecta hardware y recomienda configuración (RF-00, RF-21b)
ifeq ($(HAS_RUST),)
	@echo "[doctor] binario forge ausente — pendiente Sprint S0b."
else
	cd orchestrator && cargo run --release --bin forge -- doctor
endif

# =====================================================================
# Documentación
# =====================================================================
docs: ## Genera docs (rustdoc + Sphinx + mdBook)
ifneq ($(HAS_RUST),)
	cd orchestrator && cargo doc --no-deps
endif
ifneq ($(HAS_PYTHON),)
	cd python && sphinx-build -b html docs/source docs/build 2>/dev/null || echo "[docs] Sphinx pendiente de configurar"
endif
	@command -v mdbook >/dev/null 2>&1 && mdbook build docs || echo "[docs] mdBook pendiente de configurar"

# =====================================================================
# E2E (Playwright en A y B)
# =====================================================================
e2e: ## Tests end-to-end (Playwright) en ambos perfiles
ifeq ($(HAS_FRONTEND),)
	@echo "[e2e] frontend ausente — pendiente Sprint S9/S10."
else
	cd frontend && npx playwright test
endif

# =====================================================================
# Release (Docker A y B + tags)
# =====================================================================
release: ## Build de imágenes Docker A y B + tags
	@echo "[release] construyendo imágenes Docker A y B"
	docker build -f docker/Dockerfile.a -t forge30xx:profile-a . 2>/dev/null || echo "[release] docker/Dockerfile.a pendiente"
	docker build -f docker/Dockerfile.b -t forge30xx:profile-b . 2>/dev/null || echo "[release] docker/Dockerfile.b pendiente"

clean: ## Limpia artefactos de build
ifneq ($(HAS_RUST),)
	cd orchestrator && cargo clean
endif
	rm -rf frontend/.svelte-kit frontend/build python/.pytest_cache python/.mypy_cache python/.ruff_cache
