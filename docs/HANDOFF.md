# Handoff — estado del proyecto para otros agentes/sesiones

Última actualización: 2026-11-08. Commit cabeza: `0cdf78d` (en `origin/main`).

> Lectura obligatoria antes de tocar código: `docs/SDD.md`, `AGENTS.md`,
> `docs/adr/`, luego este archivo. PO: **Sebastia M.** (decisiones críticas,
> SDD §0 / AGENTS.md 11.0).

## Commits de referencia

```
0cdf78d feat(RF-07): inferencia minima via llama-server y transporte HTTP del RAG
2e68563 test(RF-00): verificacion en verde y fix de dev-dependency figment
a29218c feat(RF-00): deteccion de hardware y forge doctor (S0b)
39b6d7e docs(RF-14): add ADR-0002 proposal for Rust HTTP client (reqwest)
d3732ce feat(bootstrap): scaffold repo, Makefile/CI, esqueletos y RAG externo
```

## Hecho (verificado)

- [x] **Bootstrap (S0)**: estructura de repo, `.gitignore`/`.dockerignore`,
      `Makefile` (targets obligatorios), CI (`ci.yml` por capa + `ci-hardware.yml`
      matriz A/B), pre-commit, plantillas de issue/PR, `CHANGELOG.md`.
- [x] **Docs/gobernanza**: spec en `docs/SDD.md`; `AGENTS.md` con sección 11.-1
      (lectura obligatoria); `docs/adr/` con plantilla + ADR-0001 y ADR-0002.
- [x] **Esqueletos por capa**: `orchestrator/` (Rust), `python/` (forge_ml,
      forge_rag), `frontend/` (SvelteKit+TS), `inference/` (CMake), Docker.
- [x] **RAG externo (ADR-0001, Opción C)**: servicios `forge-rag` + `ollama` en
      Compose; cliente Python `forge_rag.RagClient`; `docs/rag-integration.md`.
- [x] **RF-00 / RF-21b / HU-00 — detección de hardware (S0b)**: módulo
      `hardware` (`recommend`, `parse_nvidia_smi`, `detect`); `forge doctor`.
      Verificado en RTX 3050 6GB → perfil A, aviso TGP 65W, 64 ms.
- [x] **ADR-0002 aprobada**: `reqwest` (sin TLS) añadido; `rag::RagClient` (Rust)
      async con `health`/`route`/`query`.
- [x] **RF-01 / RF-07 — inferencia mínima (S1)**: `inference::InferenceClient`
      (API OpenAI del llama-server); `forge chat` REPL con historial.
      Verificado end-to-end contra llama-server con Qwen2.5-Coder-3B.
- [x] **Calidad**: `cargo fmt --check`, `clippy -D warnings`, `cargo test`
      (21 tests) en verde con toolchain GNU en Windows.

## Pendiente (siguientes sesiones)

### Cierre de S1
- [ ] **RF-08 — gramática GBNF** para tool-calls JSON/XML (≥90% A / ≥95% B
      parseables). Anotado en `docs/inference.md`; encaja junto a S6.
- [ ] **Streaming SSE** en `forge chat`/API: hoy `InferenceClient::chat` es
      no-streaming (`stream: false`). El SDD pide streaming de tokens (RF-18).

### Backlog inmediato (orden del SDD §6)
- [ ] **S2 — KV Perfil A**: RF-05 (turbo2), RF-06 (KVFlash 4K), HU-03.
      32K en <1.5 GB KV. Requiere el fork llama-cpp-turboquant (hoy se usa un
      llama-server estándar; ver nota de desviación abajo).
- [ ] **S3 — KV Perfil B + dual GPU**: RF-05 turbo3, RF-06b, HU-00b, HU-03b.
- [ ] **S4/S5 — QLoRA A/B**: RF-03, RF-03b, RF-04. Entrypoint stub en
      `python/forge_ml/train.py` (sin lógica real todavía).
- [ ] **S6 — tool-calling**: RF-09..RF-11, RF-23 (leer/editar/crear/borrar con
      validación tree-sitter y aprobación de borrado).
- [ ] **S7 — RAG en orquestador**: cablear `rag::RagClient`/`RouterPolicy` al
      flujo de chat (hoy existen pero no se invocan desde `forge chat`).
- [ ] **S8 — web + shell sandbox**: RF-12, RF-13.
- [ ] **S9/S10 — UI + CLI**: RF-18..RF-21. `frontend/` es solo esqueleto.
- [ ] **S11 — seguridad**: RF-22/22b (guardarraíles), RF-24 (audit log).
- [ ] **S12 — memoria + degradación**: RF-08b, RF-16, RF-17.
- [ ] **S13 — release**: RNF-05/07/12/15.

### Deuda técnica / notas para el PO
- [ ] **Decisión PO pendiente**: el SDD (ADR-03/ADR-05) fija el fork
      `atomicmilkshake/llama-cpp-turboquant` y gRPC para inferencia. S1 se
      implementó contra un **llama-server estándar vía HTTP/OpenAI** (lo
      disponible y lo que exige RF-07). Antes de S2 hay que decidir: integrar el
      fork TurboQuant (necesario para RF-05/RF-06) y si inferencia migra a gRPC
      o se queda en HTTP. Abrir issue `decision:po`.
- [x] `python/`: `pip install -e .[dev]` verificado en venv local (Python 3.13).
      `ruff format`, `ruff check`, `mypy --strict` y `pytest` (6 tests, 93%
      cobertura) en verde. Se corrigieron 7 errores de `mypy --strict` en
      `forge_rag`. Falta confirmarlo en GitHub Actions (Python 3.11).
- [x] `frontend/`: `package-lock.json` generado; `npm ci`, prettier, eslint,
      svelte-check, vitest (`--passWithNoTests`, aún sin tests) y `vite build`
      en verde. Añadido `.prettierignore` (`.svelte-kit/` generado rompía
      `format:check`).
- [ ] `frontend/`: `@sveltejs/vite-plugin-svelte@3` avisa de que el soporte de
      Svelte 5 está en la v4+. Subir de versión es cambio de stack → PO.
- [ ] `frontend/eslint.config.js` importa `@eslint/js`, que no está declarado en
      `package.json` (funciona por ser dependencia transitiva de `eslint`).
- [ ] `inference/CMakeLists.txt` es un stub; no compila kernels todavía.
- [ ] CI de hardware (`ci-hardware.yml`) requiere runners self-hosted con GPU
      que aún no existen; no se ha ejecutado.

## Entorno de desarrollo verificado

- SO: Windows. Toolchain Rust **GNU** (`stable-x86_64-pc-windows-gnu`) +
  MinGW-w64 (WinLibs) en PATH → `cargo build/test/clippy/fmt` funcionan.
- GPU de desarrollo: RTX 3050 6GB Laptop (no es perfil objetivo del SDD, pero
  sirve para perfil A). Objetivo real del proyecto: Linux/WSL2 (RNF-04b).
- Modelos GGUF + `llama-server.exe` (build CUDA) en
  `C:\Users\Sebastian\Downloads\llama-cpp` (fuera del repo).

## Cómo probar lo hecho

```powershell
# Calidad de la capa Rust
cd orchestrator
cargo fmt --all --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test

# Detección de hardware
cargo run --release --bin forge -- doctor

# Chat (requiere llama-server corriendo)
cd C:\Users\Sebastian\Downloads\llama-cpp
.\llama-server.exe -m qwen2.5-coder-3b-instruct-q8_0.gguf --host 127.0.0.1 --port 8080 -c 4096 -ngl 20 --no-webui
# en otra terminal, desde el repo:
cd orchestrator
cargo run --release --bin forge -- --config ..\forge.config.toml chat
```
