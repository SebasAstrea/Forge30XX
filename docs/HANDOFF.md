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

### Pendiente del PO: método del LLM-juez (ADR-0009)

La primera ejecución (perfil A, turbo2 frente a f16/f16) quedó a 44 de 50
veredictos, con media 3.32:
- **Cuota:** la gratuita de opencode se agota en unas 45 llamadas al día y es
  compartida entre todos los modelos gratuitos.
- **Método:** comparar con la respuesta f16 mide parecido, no calidad. Con
  temperatura 0 ninguna respuesta coincidió con la de referencia, y los "2/5"
  revisados eran respuestas válidas pero distintas.

Respuestas ya generadas (reutilizables) en
`~/Descargas/kl-work/judge/{a,b}-{ref,cand}.jsonl` de `warmachine`.
Veredictos de A en `a-verdicts.jsonl`, que `score --cache` reanuda.

## Backlog inmediato (orden del SDD §6)

- [ ] **S2 — KV Perfil A**: test de aceptación `tests/hu03_full_context.rs`
      en verde en una GPU de escritorio (50.6 tok/s, 2538 MiB a 32K). Falta:
      ejecutarlo en una 3060 Laptop 6 GB y cerrar el LLM-juez (ADR-0009).
      RF-05 y RF-06 cumplen el criterio de VRAM.
- [ ] **S3 — KV Perfil B + dual GPU**: RF-06b (reparto KV midiendo PCIe),
      HU-00b, HU-03b. Volver a medir f16/turbo3 tras el reparto (margen
      23.8 vs 22 tok/s).
- [ ] **S4/S5 — QLoRA A/B**: RF-03, RF-03b, RF-04. Stub en
      `python/forge_ml/train.py`.
- [ ] **S6 — tool-calling**: RF-09..RF-11, RF-23. RF-10 necesita tree-sitter
      (dependencia nueva → ADR + PO).
- [ ] **S7 — RAG en orquestador**: cablear `rag::RagClient`/`RouterPolicy` a
      `forge chat`.
- [ ] **S8..S13**: según SDD §6.

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
