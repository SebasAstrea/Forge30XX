# Changelog

Todas las modificaciones notables de este proyecto se documentan aquí.

El formato sigue [Keep a Changelog](https://keepachangelog.com/es-ES/1.1.0/)
y el proyecto se adhiere a [Versionado Semántico](https://semver.org/lang/es/).

## [Unreleased]

### Changed (decisiones del PO 2026-11-09)

- Modelos base (ADR-0013): perfil A Qwen3.5-4B y perfil B Qwen3.6-35B-A3B
  (MoE, Apache-2.0). `forge doctor` recomienda para B `--n-cpu-moe 2
  --reasoning off`; medido 34.8 tok/s a 64K y RF-08 al 100% sin LoRA.
- Con PCIe x4 o menor se mantiene el reparto por capas y `forge doctor` avisa;
  la KV ya no se saca a RAM (ADR-0011).
- RF-06 redefinido (ADR-0005): contexto completo en VRAM con TurboQuant, sin
  ventana KVFlash. `forge doctor` y `forge.config.toml` dejan de mostrar la
  ventana y pasan a indicar los tipos K y V por separado.
- Perfil B recomienda `-ctk f16 -ctv turbo3` (ADR-0006): 23.8 tok/s a 64K con
  ~2.1 GB de KV; KL-divergence 0.0040 frente a f16/f16.
- `@sveltejs/vite-plugin-svelte` 3 → 4 (ADR-0007): soporte oficial de Svelte 5.
- LLM-juez: Qwen2.5-72B sustituido por un modelo gratuito de opencode
  (ADR-0009). Puntuación absoluta con el código y 25 tareas por perfil
  (ADR-0010); AGENTS.md 11.5 pasa de 50 a 25 prompts.

### Added

- QLoRA del perfil B (HU-02): 7B en dos GPUs en 2 h 20 min; RF-08 sin gramática
  pasa del 20% al 97% con el adaptador. RF-04: la fusión en f16 coincide con el
  adaptador separado, y el export a Q4_K_M requiere los pesos de 16 bits
  (`docs/training.md`).
- QLoRA de tool-calling (RF-03, RF-03b, ADR-0012): `make dataset` (Glaive v2 +
  sintético, con manifest) y `make train-lora PROFILE=A|B` con Unsloth. Perfil
  A: época en 33.6 min con un pico de 2.5 GB; RF-08 sin gramática pasa del 30%
  al 100% con el adaptador. Guía en `docs/training.md`.
- `forge doctor` (RF-06b, HU-00b): lee el enlace PCIe de cada GPU, avisa si es
  x4 o menor con su ancho de banda estimado e imprime los flags del
  llama-server para el perfil. Medición del reparto entre dos GPUs en
  `docs/inference.md`.
- `python -m forge_ml.judge` (`generate` + `score`, reanudable con `--cache`):
  LLM-juez de AGENTS.md 11.5 sobre el CLI de opencode, sin dependencias nuevas.
- Test de aceptación de HU-03/HU-03b con el contexto lleno
  (`orchestrator/tests/hu03_full_context.rs`, ignorado sin llama-server).
  Perfil A 50.6 tok/s y 2538 MiB a 32K; perfil B 23.7 tok/s a 64K.

### Removed

- `tonic` del orquestador (ADR-0008): sin uso desde ADR-0004.

### Fixed

- CI: `ci.yml` no era YAML válido (`: ` sin comillas en el job de bootstrap) y
  GitHub lo rechazaba sin ejecutar ningún job desde el primer commit.
- `forge chat` sin llama-server escuchando terminaba con error; ahora informa
  y sale con código 0, como documenta `docs/inference.md`.
- `make check` ignoraba el fallo de cobertura Rust (`|| echo`); ahora falla si
  falta `cargo-llvm-cov` o si la cobertura baja del 80%.

### Changed

- Reglas de AGENTS.md 11.1 verificadas por linter: clippy `too_many_lines`
  (≤40), `unwrap_used`/`expect_used` fuera de tests; ruff `C90` (≤10) y `T20`;
  ESLint `complexity`, `max-lines-per-function`, `max-lines`; `make check-sizes`
  (archivos ≤400, Svelte ≤150) también en CI.
- `hardware::recommend` dividida en `select_tier` + `gpu_warnings` (73 → 21
  líneas), mismo comportamiento.
- Cobertura Rust de 69% a 96%: tests de `RagClient` e `InferenceClient` contra
  un servidor HTTP falso compartido (`test_support`) y tests de la CLI
  (`tests/cli.rs`).
- `@eslint/js` declarado en `devDependencies` (ya estaba instalado como
  dependencia transitiva de `eslint`, misma versión 9.39.5).
- `inference/README.md`, `inference/CMakeLists.txt`, README y diagrama del SDD
  alineados con ADR-0004 (fork TheTom, HTTP en lugar de gRPC).

### Added

- Streaming de tokens (RF-18 adelantado a S1, ADR-0003): `InferenceClient::chat_stream`
  con decodificador SSE propio (sin dependencias nuevas); `forge chat` muestra
  los tokens según llegan.
- Gramática GBNF para tool-calls (RF-08): `grammars/tool_calls.gbnf`,
  `SamplingParams::grammar` y módulo `tool_call` (parser estricto). Test
  `rf08_tool_calls` contra modelo real: 100% parseables con gramática (Q8_0, 3B).
- ADR-0003 (streaming en S1) y ADR-0004 (fork TheTom/llama-cpp-turboquant
  servido por HTTP/OpenAI, sin gRPC).
- Medición de RF-05 y repetición de RF-08 con Q4_K_M (3B) y 7B en el fork
  TurboQuant compilado en Linux (2x RTX 3060 12 GB): tablas en
  `docs/inference.md`. KV de A y B dentro del KPI; en B a 64K solo f16/turbo3
  llega a 22 tok/s; RF-08 97% (3B) y 100% (7B) con gramática.

### Changed

- Timeout de inferencia: total en `chat`, entre trozos en `chat_stream`.

- Inferencia mínima (RF-01, RF-07): módulo `inference` con `InferenceClient`
  (cliente del llama-server vía API OpenAI, reqwest) y `forge chat` como REPL
  real con historial. Sección `[inference]` en la config. Guía `docs/inference.md`.
  Verificado end-to-end contra llama-server (Qwen2.5-Coder-3B) en RTX 3050 6GB.
- Transporte HTTP del RAG en Rust (ADR-0002 aprobada): `rag::RagClient`
  (`health`, `route`, `query`) con `reqwest` sin TLS. Añadida dependencia
  `reqwest` al orquestador.

- Detección de hardware y `forge doctor` (RF-00, RF-21b, HU-00): módulo
  `hardware` con `recommend` (lógica pura de selección de perfil según la
  matriz SDD §2), `parse_nvidia_smi` y `detect`; el comando imprime perfil,
  modelo, contexto, cuantización KV, ventana KVFlash y avisos (TGP<100W,
  GPU no-30xx). Guía `docs/hardware-detection.md`. 10 tests unitarios.
  Verificado en hardware real (RTX 3050 6GB → perfil A, aviso TGP 65W, 64 ms)
  y con `cargo fmt/clippy -D warnings/test` en verde (14 tests).

### Fixed

- `figment` habilitado con feature `test` en dev-dependencies (necesaria para
  `figment::Jail` en los tests de configuración).

- Estructura de directorios del repositorio (Sprint S0 — Bootstrap).
- `docs/SDD.md`: spec movido a `docs/` (antes `SDD_DOC.md`).
- Sección "Lectura obligatoria" en `AGENTS.md` (11.-1).
- `Makefile` con los targets obligatorios (`check`, `bench`, `test-a`, `test-b`,
  `train-lora`, `doctor`, `docs`, `release`).
- Pipeline de CI (GitHub Actions): lint + test por capa y matriz de hardware A/B.
- `forge.config.toml` de ejemplo y `.env.example`.
- Plantillas de issues (`decision:po`, `question`) y de pull request.
- Esqueleto del orquestador Rust (`orchestrator/`): crate `forge` (lib + bin),
  CLI con `clap` (doctor/run/chat/profile/version), carga de config con `figment`.
- Esqueleto Python (`python/`): paquetes `forge_ml` y `forge_rag`, entrypoint
  `forge_ml.train` y `pyproject.toml` (extras `ml` y `dev`).
- Esqueleto frontend SvelteKit + TS (`frontend/`): config strict, ESLint, Prettier.
- Esqueleto de la capa de inferencia (`inference/`): `CMakeLists.txt` con `GGML_CUDA`.
- `docker-compose.yml` (perfiles `a`/`b`) y `docker/Dockerfile.{a,b}` multi-stage.
- Integración del RAG portable (port-rag-potty) como servicio externo vía HTTP
  (ADR-0001, Opción C): servicios `forge-rag` + `ollama` en Compose, cliente
  Python `forge_rag.RagClient`, módulo Rust `rag::RouterPolicy` con umbrales
  por perfil (RF-15), secciones `[context_router]` y `[rag]` en la config, y
  guía `docs/rag-integration.md`.
- `docs/adr/0001-integracion-rag-portable.md`.
- `docs/adr/0002-cliente-http-rust-rag.md` (Propuesta, pendiente de aprobación
  del PO): propone `reqwest` sin TLS para el transporte HTTP del RAG en Rust (S7).
