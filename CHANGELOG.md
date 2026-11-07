# Changelog

Todas las modificaciones notables de este proyecto se documentan aquí.

El formato sigue [Keep a Changelog](https://keepachangelog.com/es-ES/1.1.0/)
y el proyecto se adhiere a [Versionado Semántico](https://semver.org/lang/es/).

## [Unreleased]

### Added

- Detección de hardware y `forge doctor` (RF-00, RF-21b, HU-00): módulo
  `hardware` con `recommend` (lógica pura de selección de perfil según la
  matriz SDD §2), `parse_nvidia_smi` y `detect`; el comando imprime perfil,
  modelo, contexto, cuantización KV, ventana KVFlash y avisos (TGP<100W,
  GPU no-30xx). Guía `docs/hardware-detection.md`. 10 tests unitarios.

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
