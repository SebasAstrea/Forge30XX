# Handoff — estado del proyecto para otros agentes/sesiones

Última actualización: 2026-11-09. Ver `git log` para el commit cabeza.

> Lectura obligatoria antes de tocar código: `docs/SDD.md`, `AGENTS.md`,
> `docs/adr/`, luego este archivo. PO: **Sebastia M.** (decisiones críticas,
> SDD §0 / AGENTS.md 11.0).

## Retomar en otro equipo

- `python/.venv`, `frontend/node_modules`, `orchestrator/target` y `models/*`
  no están en git. Recrear: `python3 -m venv python/.venv` +
  `python/.venv/bin/pip install -e "python[dev]"`; `cd frontend; npm ci`;
  `cd orchestrator; cargo build`.
- `make check` necesita `rustup component add rustfmt clippy llvm-tools-preview`
  y `cargo install cargo-llvm-cov --locked` (ya no se omite la cobertura).
- Fork TurboQuant: fuera del repo. Compilación en Linux en
  `docs/inference.md#compilar-el-fork-en-linux`.
- Modelos usados en las mediciones (Hugging Face, `Qwen/*-GGUF`):
  `models/qwen2.5-coder-3b-instruct-q4_k_m.gguf` y
  `models/qwen2.5-coder-7b-instruct-q4_k_m.gguf`.

## Equipos de desarrollo

| Equipo | SO | GPU | Perfil | Notas |
|---|---|---|---|---|
| Original | Windows, Rust GNU + MinGW | RTX 3050 6 GB Laptop | A | Sin MSVC: el fork no compila con CUDA ahí |
| `warmachine` | Fedora 44 | 2x RTX 3060 12 GB | B (A+ con 1 GPU) | CUDA 13.3, GCC 16; fork en `~/Descargas/llama-cpp-turboquant` |

## Hecho (verificado)

- [x] **S0 / S0b / S1** completos: bootstrap, `forge doctor` (RF-00/RF-21b),
      inferencia por HTTP/OpenAI con streaming (RF-01/07, ADR-0003/0004),
      gramática GBNF (RF-08).
- [x] **Deuda técnica (2026-11-09)**:
  - `ci.yml` no era YAML válido: GitHub no había ejecutado ningún job desde el
    primer commit. Corregido y validado con `actionlint`.
  - Cobertura Rust 69% → 96% (`make check` la enmascaraba con `|| echo`).
    Servidor HTTP falso compartido en `orchestrator/src/test_support.rs`;
    tests de CLI en `orchestrator/tests/cli.rs`.
  - Bug: `forge chat` sin llama-server terminaba con error en vez de informar.
  - Reglas de AGENTS.md 11.1 verificadas por linter (clippy `too_many_lines`,
    `unwrap_used`/`expect_used`; ruff `C90`/`T20`; ESLint `complexity`,
    `max-lines-per-function`, `max-lines`; `make check-sizes`). La complejidad
    ciclomática en Rust **no** se verifica: el lint de clippy cuenta macros.
  - `@eslint/js` declarado; docs de `inference/`, README y SDD alineados con
    ADR-0004.
- [x] **RF-05 medido** (S2/S3) con el fork en Linux: KV de A (32K) y B (64K)
      dentro del KPI con turbo2/turbo3. Tablas en `docs/inference.md`.
- [x] **RF-08 repetido** con Q4_K_M: 3B 97% (A, umbral 90%), 7B en 2 GPUs
      100% (B, umbral 95%).

## Decisiones del PO (2026-11-09)

| # | Decisión | ADR | Aplicado |
|---|---|---|---|
| 1 | RF-06 sin ventana KV: contexto completo en VRAM con TurboQuant | 0005 | SDD, doctor, config, docs |
| 2 | Perfil B con `-ctk f16 -ctv turbo3` | 0006 | `forge doctor`, docs. KLD 0.0040 vs f16 |
| 3 | `@sveltejs/vite-plugin-svelte` 3 → 4 | 0007 | `frontend/package.json` |
| 4 | Retirar `tonic` | 0008 | `Cargo.toml`, SDD §8 |
| 5 | LLM-juez con opencode y modelo gratuito | 0009 | `python -m forge_ml.judge`, SDD §10 |
| 6 | Juez: puntuación absoluta, 25 tareas por perfil | 0010 | `forge_ml.judge`, AGENTS.md 11.5 |
| 7 | PCIe ≤x4: reparto por capas, sin KV en RAM | 0011 | SDD, `forge doctor` |
| 8 | Dataset QLoRA: Glaive v2 + sintético | 0012 | `forge_ml.dataset` |
| 9 | Modelos: Qwen3.5-4B (A), Qwen3.6-35B-A3B (B) | 0013 | SDD, `forge doctor` |
| 10 | QLoRA solo en el perfil A | 0014 | SDD (RF-03, RF-04, HU-02, S5) |

### LLM-juez (ADR-0009, ADR-0010)

Método aprobado: puntuación absoluta con el código, ≥4/5 en 25 tareas por
perfil. En `warmachine`, `~/Descargas/kl-work/judge25.sh` genera las
respuestas (candidatas y f16/f16 de control) y reintenta la puntuación cada
30 min hasta que se renueve la cuota gratuita de opencode (~45 llamadas al
día). Resultados: `~/Descargas/kl-work/judge25/{a,b}-report.json`. Hay que
copiar las notas a `docs/inference.md` y ADR-0006 cuando estén.

## Backlog inmediato (orden del SDD §6)

- [ ] **S2 — KV Perfil A**: test de aceptación `tests/hu03_full_context.rs`
      en verde en una GPU de escritorio (50.6 tok/s, 2538 MiB a 32K). Falta:
      ejecutarlo en una 3060 Laptop 6 GB y las notas del LLM-juez (ADR-0010).
      RF-05 y RF-06 cumplen el criterio de VRAM.
- [x] **S3 — KV Perfil B + dual GPU**:
  - [x] RF-06b medido: reparto por capas entre 70/30 y 50/50 da 23.4–23.6
        tok/s a 64K (`docs/inference.md#reparto-entre-dos-gpus-rf-06b`).
  - [x] `forge doctor` lee el enlace PCIe, avisa si es ≤x4 e imprime los
        flags del llama-server para el perfil. Con PCIe ≤x4 se mantiene el
        reparto por capas (ADR-0011).
  - [x] HU-03b: `tests/hu03_full_context.rs` en verde (23.7 tok/s a 64K).
- [ ] **S4 — QLoRA A** (`docs/training.md`):
  - [x] Dataset Glaive v2 + sintético con manifest (ADR-0012):
        `make dataset GLAIVE=...` → `artifacts/dataset-v1`.
  - [x] `make train-lora PROFILE=A`: época completa en 33.6 min, pico de
        2490 MiB (RTX 3060 12 GB de escritorio).
  - [x] HU-01: RF-08 con el adaptador 100% sin gramática (30% sin adaptador).
  - [ ] Repetir en la 3060 Laptop 6 GB (tiempo <4 h y sin OOM).
- [ ] **S5 — QLoRA B + merge** (`docs/training.md#perfil-b-hu-02`):
  - [x] HU-02: 7B en dos GPUs, época en 2 h 20 min, pico de 4746 MiB por
        GPU. RF-08 con el adaptador: 97% sin gramática (20% sin adaptador).
  - [x] RF-04, fusión: `llama-export-lora` en f16 coincide con el adaptador
        (PPL −0.1%).
  - [ ] RF-04, export Q4_K_M: recuantizar el Q4_K_M fusionado pierde un 3.2%
        (>2%). Hay que fusionar sobre los pesos de 16 bits de
        `Qwen/Qwen2.5-Coder-7B-Instruct` (~15 GB, la descarga la hace el PO).
- [x] **Cambio de modelos (ADR-0013)**: perfil B = Qwen3.6-35B-A3B UD-Q4_K_M
      (34.8 tok/s a 64K, RF-08 100% sin LoRA); perfil A = Qwen3.5-4B
      (Apache-2.0). Resuelve la licencia `qwen-research` del 3B.
  - [ ] **Siguiente paso (punto 1)**: descargar el GGUF de Qwen3.5-4B y
        medir el perfil A. Archivo sugerido: `unsloth/Qwen3.5-4B-GGUF`,
        `Qwen3.5-4B-Q4_K_M.gguf` (2.55 GiB) en `models/` (ruta ya puesta en
        `forge.config.toml`). Medir con `CUDA_VISIBLE_DEVICES=0`:
        - KV y VRAM a 32K con turbo2 (`llama-server -lv 4`, línea
          `llama_kv_cache: size`).
        - `tests/hu03_full_context.rs` (≥12 tok/s, ≤5837 MiB).
        - `tests/rf08_tool_calls.rs` (≥90%). Si es un modelo con thinking,
          probar `--reasoning off` como en el 35B-A3B.
        Conexión del PO muy lenta el 2026-11-09: la descarga la hace él.
  - [x] **RF-03 tras el cambio (ADR-0014)**: QLoRA solo en A con Qwen3.5-4B;
        B usa Qwen3.6-35B-A3B sin LoRA. HU-02 retirada.
  - [ ] Reentrenar A con Qwen3.5-4B: poner en `[training.A]` su versión
        bnb-4bit con revisión fijada, `make train-lora PROFILE=A`, RF-08 y
        RF-04 sobre pesos de 16 bits.

## Otras notas

- `ci-hardware.yml` (nightly) se queda en cola 24 h y se cancela: no hay
  runners self-hosted. `warmachine` podría registrarse como runner
  `[self-hosted, gpu, rtx3060-dual]`.
- `frontend/` sigue siendo un esqueleto (S9).
- CI de GitHub en verde desde el commit `fix(build): restaurar yaml…`
  (2026-11-09). Generar `package-lock.json` con `npx npm@10` (la versión del
  CI): npm 11 elimina entradas que npm 10 exige.

## Cómo probar lo hecho

```bash
make check                                   # fmt + lint + test + cobertura, 3 capas
cd orchestrator && cargo run --release --bin forge -- doctor
# Chat contra el fork (ver docs/inference.md para los flags por perfil)
llama-server -m models/qwen2.5-coder-3b-instruct-q4_k_m.gguf -c 32768 -ngl 99 \
  -fa on -np 1 -ctk turbo2 -ctv turbo2 --port 8080
cd orchestrator && cargo run --release --bin forge -- --config ../forge.config.toml chat
```
