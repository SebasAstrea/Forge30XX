# SDD — Forge30XX (Rev. 4 - DeepSeek & PO: SMorales)

> Spec-Driven Development. Las reglas vinculantes de ingeniería para agentes y humanos están en `../AGENTS.md` (raíz del repo). Las ADRs viven en `adr/`.

Proyecto: Forge30XX. Agente de codificación local para GPUs Nvidia serie 30xx. Hardware objetivo prioritario: (A) RTX 3060 6 GB Laptop, (B) 2x RTX 3060 12 GB en una máquina. RTX 3090 opcional, no objetivo de diseño. Lema: máximo provecho con lo poco disponible.

---

## 0. Autoridad de Decisión

Product Owner: **Sebastia M.** Sobre él recae la decisión final sobre cualquier cambio que altere el rumbo del MVP.

Requieren consulta obligatoria al PO (bloquean la tarea hasta respuesta):

- Cambios de arquitectura: nuevos componentes, modificación de ADRs existentes, cambio de topología de comunicación (gRPC/HTTP/FFI), cambio en el modelo de despliegue (Docker/nativo), cambio en la estrategia de reparto de KV o de offload entre GPUs.
- Cambios de stack tecnológico: sustitución o adición de lenguajes, frameworks, librerías core (Rust/Axum/Tokio, PyTorch/Unsloth, SvelteKit, llama.cpp fork), motores de inferencia, bases de datos, sistemas de embeddings.
- Cambios de funciones: añadir, eliminar o modificar requisitos funcionales (RF) o no funcionales (RNF); alterar umbrales de KPI; cambiar el alcance de un perfil de hardware; introducir nuevas tool-calls o capacidades del agente.
- Cambios de historias de usuario: crear, eliminar, repriorizar o reestimar HUs; cambiar criterios de aceptación; mover HUs entre sprints o cambiar el alcance de un sprint en curso.
- Cambios de alcance del MVP: incluir/excluir perfiles de hardware, funcionalidades, no-objetivos; decidir qué entra en v1.0.
- Cambios de metodología o de DoD: alterar el proceso Scrum, la definición de terminado, las reglas de merge o los umbrales de testing.
- Cualquier decisión irreversible o de alto costo: migraciones de datos, cambios de licencia, dependencias con coste, compromisos de compatibilidad a largo plazo.

Procedimiento del agente ante una decisión crítica:

1. Detener la implementación de la tarea afectada. No improvisar ni asumir.
2. Redactar una pregunta estructurada al PO (Sebastia M.) con: contexto, opciones viables (mínimo 2), trade-offs (rendimiento, VRAM, complejidad, tiempo, riesgo), recomendación del agente con justificación, e impacto en RF/RNF/HUs/sprints.
3. Registrar la pregunta como issue con label `decision:po` y enlazarla a la tarea bloqueada.
4. Esperar respuesta del PO antes de continuar. Mientras tanto, el agente puede avanzar en tareas no afectadas.
5. Una vez respondida, documentar la decisión como ADR nueva (o actualizar la existente) y reflejar los cambios en este SDD y en el backlog.

Regla: ante ambigüedad entre este documento y una instrucción del PO, prevalece la del PO, pero debe quedar registrada como ADR para trazabilidad.

---

## 1. Propósito

Convertir un modelo abierto (Qwen2.5-Coder-3B en A, 7B en B) en agente de codificación autónomo: leer, editar, crear, borrar archivos; buscar docs web; navegar repos. Todo local, sin nube.

Problema: agentes en nube son caros y filtran código. Intentos locales fallan por (a) modelos 7B+ no caben en 6 GB, (b) contexto largo agota VRAM, (c) mala gestión multi-turno. Solución: QLoRA para especialización, TurboQuant (caché KV cuantizada) para contexto, port-rag-potty para reducir contexto efectivo, degradación con gracia bajo OOM.

### KPIs

| KPI | Perfil A (6 GB) | Perfil B (2x12 GB) |
|---|---|---|
| Modelo | 3B Q4_K_M | 7B Q4_K_M split |
| Contexto efectivo | 32K | 64K |
| Velocidad | ≥12 tok/s | ≥22 tok/s |
| KV VRAM | <1.5 GB | <3 GB total |
| Tool-call JSON válido | ≥90% | ≥95% |
| Ahorro RAG | ≥30% | ≥40% |
| Latencia router p95 | <100 ms | <100 ms |
| Arranque frío | <150 s | <180 s |
| VRAM usada | ≤5.7 GB | ≤11.4 GB/GPU |

### No-objetivos v1.0

AMD/Intel/40xx/50xx. Entrenamiento desde cero. Multimodalidad. Nube/multi-tenant. NVLink/P2P (3060 no soporta; usar PCIe + host). Tensor parallelism puro (usar layer/pipeline split u offload asimétrico).

---

## 2. Perfiles de Hardware

`forge doctor` detecta GPU (modelo, VRAM, TGP, PCIe) y selecciona perfil.

### Perfil A: RTX 3060 6 GB Laptop
- 6 GB GDDR6, TGP 85-115 W, ~192 GB/s, RAM sistema ≥16 GB.
- Estrategia: 3B Q4 (~1.9 GB) + KV turbo2 (0.15–0.41 GB medidos a 32K, `docs/inference.md`) + overhead (~0.8 GB). Contexto completo en GPU (ADR-0005). Offload capas a RAM vía mmap.

### Perfil B: 2x RTX 3060 12 GB
- 24 GB total, sin NVLink, PCIe gen3/4, comunicación vía host.
- Estrategia: 7B Q4 (~4.4 GB) en GPU0. KV con K f16 + V turbo3 (ADR-0006) repartida GPU0/GPU1 midiendo ancho PCIe real. Contexto completo en GPU (ADR-0005).
- Enlace PCIe x4 o menor en alguna GPU: se mantiene el reparto por capas y `forge doctor` avisa (ADR-0011). Sacar la KV a RAM se descarta: 0.6 tok/s medidos a 64K.

### Matriz compatibilidad

| GPU | VRAM | Perfil | Modelo | Contexto |
|---|---|---|---|---|
| 3060 Laptop | 6 | A | 3B Q4 | 32K |
| 3060 12 GB single | 12 | A+ | 7B Q4 | 48K |
| 2x 3060 12 GB | 24 | B | 7B Q4 | 64K |
| 3070/3070 Ti | 8 | A+ | 3B Q5 / 7B Q3 | 40K |
| 3080 10 GB | 10 | A+ | 7B Q4 | 48K |
| 3080 Ti/3090 | 12-24 | Premium | 7B Q5 / 14B Q4 | 100K+ |

Regla: nunca fallar por OOM. Degradar (menos contexto, menos capas GPU, modelo menor) y avisar con recomendación concreta.

---

## 3. Requisitos Funcionales

Formato: ID | Requisito | Prioridad | Perfiles | Criterio aceptación.

### 3.1 Hardware
- RF-00 | Detectar GPUs (modelo, VRAM, TGP, PCIe), seleccionar perfil A/B | Must | * | `forge doctor` <3 s; coincide matriz 2 en >95% casos CI.
- RF-00b | Degradar sin OOM: reducir contexto, mover capas CPU, o modelo menor | Must | * | Prompt que excede VRAM responde con menor calidad; registra degradación.

### 3.2 Modelo y especialización
- RF-01 | Cargar 3B (A) o 7B (B) GGUF Q4_K_M/Q5_K_M | Must | * | A: <30 s; B: <60 s.
- RF-02 | Aplicar LoRA sin recargar base | Must | * | Hot-swap <3 s; 10 prompts verificables.
- RF-03 | Pipeline QLoRA reproducible para tool-calling | Must | * | `make train-lora PROFILE=A` <4 h en 6 GB; B <6 h en 2x3060.
- RF-03b | Unsloth + QLoRA 4-bit + gradient_checkpointing + paged_adamw_8bit | Must | A | 3B batch efectivo 8 sin OOM en 6 GB.
- RF-04 | Merge LoRA + export GGUF | Should | * | ±2% equivalente a adaptador separado.

### 3.3 Inferencia y contexto
- RF-05 | Integrar llama-cpp-turboquant; turbo2 (A) / turbo3 (B) | Must | * | A: 32K → <1.5 GB KV. B: 64K → <3 GB KV.
- RF-06 | Contexto máximo (32K A, 64K B) con toda la caché KV en VRAM, vía TurboQuant + flash attention, sin ventana (ADR-0005) | Must | * | KV a contexto máximo dentro del KPI (A <1.5 GB, B <3 GB). La velocidad con contexto lleno la cubre RNF-01.
- RF-06b | Repartir KV entre 2 GPUs midiendo PCIe real | Must | B | Proporción maximiza tok/s; si PCIe ≤x4, mantiene el reparto por capas y avisa (ADR-0011).
- RF-07 | API OpenAI (`/v1/chat/completions`, `/v1/models`) sobre llama-server | Must | * | Cliente `openai` Python sin cambios.
- RF-08 | Gramática GBNF para tool-calls JSON/XML | Must | * | ≥90% (A) / ≥95% (B) parseables.
- RF-08b | Bajo VRAM ≥90%: turbo3→turbo2 o contexto 24K/48K | Must | * | Sigue respondiendo; registra degradación.

### 3.4 Tool-calling
- RF-09 | Leer archivos rutas relativas, límite configurable | Must | * | 10 MB por streaming sin bloquear.
- RF-10 | Editar con diffs validados tree-sitter | Must | * | `.py`/`.rs` válido no rompe parseo.
- RF-11 | Crear/borrar con confirmación obligatoria en borrado | Must | * | Borrado sin confirmación rechazado.
- RF-12 | Búsqueda web con extracción contenido + lista blanca dominios | Must | * | 5 resultados <5 s.
- RF-13 | Shell sandbox lista blanca + timeout | Should | * | No permitidos rechazados; timeout 30 s mata proceso.
- RF-14 | Consultar port-rag-potty | Must | * | Decide RAG vs lectura directa por tamaño.

### 3.5 Contexto y memoria
- RF-15 | Router contexto RAG/lectura/resumen; umbrales por perfil (A 1.5K, B 3K tokens) | Must | * | Precisión >85% en 100 consultas etiquetadas por perfil.
- RF-16 | Resumir historial al 70% ventana (A 22K, B 45K) | Should | * | Coherente tras 3 resúmenes (LLM-juez ≥4/5).
- RF-17 | Persistir sesión en `.forge/session.json` | Should | * | Recuperar <2 s.

### 3.6 UI
- RF-18 | UI web local chat + visor + panel tool-calls | Must | * | Carga <2 s; streaming tokens.
- RF-19 | VRAM por GPU y contexto en vivo | Should | * | Gráfico 1 s; error <5%; B muestra ambas GPUs.
- RF-20 | Aprobar/rechazar tool-calls destructivas | Must | * | Sin aprobación no ejecuta.
- RF-21 | CLI `forge` con `doctor`, `run`, `train`, `chat`, `rag`, `profile` | Should | * | Todos flujos UI reproducibles CLI.
- RF-21b | `forge doctor` recomienda config óptima | Must | * | Coincide matriz 2; advierte si TGP <100 W.

### 3.7 Seguridad
- RF-22 | Guardarraíl entrada (injection, jailbreak, PII) con modelo <150M | Must | * | >90% (A) / >95% (B) detección set adversarial; <8% (A) / <5% (B) falsos positivos.
- RF-22b | Guardarraíl corre en CPU si VRAM ≥90% | Must | A | Funciona con latencia <200 ms bajo presión.
- RF-23 | Guardarraíl salida valida tool-calls vs declaradas y tipos | Must | * | 100% inválidas rechazadas como error estructurado.
- RF-24 | Audit log append-only con timestamp, params, resultado | Must | * | Consultable, no modificable desde UI.

---

## 4. Requisitos No Funcionales

| ID | Categoría | Requisito | Perfil | Criterio |
|---|---|---|---|---|
| RNF-01 | Rendimiento | ≥12 (A) / ≥22 (B) tok/s con contexto lleno | A/B | llama-bench en CI hardware |
| RNF-02 | Rendimiento | Arranque <150 s (A) / <180 s (B) | A/B | docker compose up a UI lista |
| RNF-03 | Memoria | ≤95% VRAM por GPU (5.7 GB A; 11.4 GB B) | A/B | nvidia-smi 1 h uso |
| RNF-03b | Memoria | RAM sistema ≤12 GB (A) / ≤20 GB (B) | A/B | htop durante benchmark |
| RNF-04 | Compatibilidad | Solo 30xx Ampere; drivers ≥535; CUDA ≥12.1 | * | CI matrix 3060 6 GB Laptop, 3060 12 GB, 2x3060 12 GB |
| RNF-04b | Compatibilidad | Windows 11 WSL2 + Linux nativo Ubuntu 22.04+ | * | E2E en ambos |
| RNF-05 | Portabilidad | Docker <6 GB (A) / <8 GB (B) | A/B | docker images |
| RNF-06 | Seguridad | Cero salidas de red salvo RF-12 | * | tcpdump: cero conexiones no declaradas |
| RNF-07 | Mantenibilidad | Cobertura ≥80% core, ≥60% integración | * | cargo llvm-cov + pytest-cov |
| RNF-08 | Calidad | Cero warnings clippy -D warnings, ruff check | * | CI bloquea merge |
| RNF-09 | Reproducibilidad | Semilla, datasets, hash base en manifest | * | manifest.json por train-lora |
| RNF-10 | Observabilidad | /metrics Prometheus + logs JSON | * | Dashboard Grafana |
| RNF-11 | Latencia | Router p95 <100 ms | * | criterion + pytest-benchmark |
| RNF-12 | Documentación | rustdoc/Sphinx + ejemplo ejecutable | * | make docs sin warnings |
| RNF-13 | Accesibilidad | Usable en 6 GB: tool-calling funcional, ≥32K, ≥12 tok/s | A | Demo e2e edición real <5 min en 3060 6 GB Laptop |
| RNF-14 | Escalabilidad | Añadir 2da GPU duplica contexto (32K→64K) solo config | A→B | Mismo binario, distinto forge.config.toml |
| RNF-15 | Tolerancia fallos | Si GPU falla en B, sigue en single-GPU sin reiniciar | B | Test hot-unplug simulado |

---

## 5. Historias de Usuario

- HU-00 | 3060 Laptop 6 GB: `forge doctor` recomienda config sin investigar | RF-00, RF-21b | A: perfil, 3B Q4, 32K, KV turbo2, aviso TGP<100W. B: 7B Q4, 64K, KV f16/turbo3, reparto medido | 5 pts
- HU-00b | 2x3060: reparto automático entre GPUs | 7B y su KV repartidos por capas entre GPUs; mide PCIe; si ≤x4, avisa (ADR-0011) | 8 pts
- HU-01 | 3060 6 GB: entrenar LoRA 3B sin OOM | `make train-lora PROFILE=A` sin OOM batch 8; manifest completo; ≥90% tool-calls válidas | 13 pts
- HU-02 | 2x3060: entrenar 7B usando ambas | device_map=auto <6 h; ≥95% válidas | 8 pts
- HU-03 | 3060 6 GB: 32K contexto sin agotar VRAM | ≤5.7 GB; ≥12 tok/s lleno | 8 pts
- HU-03b | 2x3060: 64K usando ambas | ≤11.4 GB/GPU; ≥22 tok/s | 8 pts
- HU-03c | Reducción automática de contexto antes de fallar | Al 90% VRAM reduce ventana/turbo; sigue respondiendo; registra en UI | 5 pts
- HU-05 | Leer/editar/crear/borrar archivos con seguridad | Leer <500 ms <1 MB; editar valida tree-sitter; borrar requiere aprobación | 13 pts
- HU-06 | Búsqueda web actual | 5 resultados <5 s; solo lista blanca | 8 pts
- HU-07 | Shell sandbox lista blanca + timeout | No permitidos rechazados; timeout 30 s | 5 pts
- HU-08 | UI web local chat+visor+panel | Streaming, resaltado, aprobación | 13 pts
- HU-09 | 2x3060: VRAM ambas GPUs en vivo | Dos gráficos 1 s; error <5% | 5 pts
- HU-10 | 3060 6 GB: guardarraíl sin robar VRAM al modelo | CPU si VRAM ≥90%; <200 ms; ≥90% detección | 8 pts
- HU-11 | Audit log inmutable | Append-only, consultable CLI | 3 pts

---

## 6. Backlog por Sprint (2 semanas, 1 dev + agentes IA)

| Sprint | Objetivo | RF/HU | Demo |
|---|---|---|---|
| S0 (1 sem) | Bootstrap | repo, CI, Docker, esqueleto Rust+Python, AGENTS.md, licencia | docker compose up hello world Rust+Python |
| S0b | Detección hardware | RF-00, RF-00b, RF-21b, HU-00 | doctor imprime A y B correctos en CI |
| S1 | Inferencia mínima | RF-01, RF-07, RF-08, streaming de tokens de RF-18 (ADR-0003) | Chat 3B (A)/7B (B) vía API OpenAI con streaming |
| S2 | KV Perfil A | RF-05 turbo2, RF-06 (ADR-0005), HU-03 | 32K en <1.5 GB KV en 3060 6 GB |
| S3 | KV Perfil B + dual | RF-05 turbo3, RF-06, RF-06b, HU-00b, HU-03b | 64K repartido en 2x3060 |
| S4 | QLoRA A | RF-03, RF-03b, HU-01 | LoRA 3B <4 h en 6 GB |
| S5 | QLoRA B + merge | RF-03 B, RF-04, HU-02 | LoRA 7B en 2x3060 + merge GGUF |
| S6 | Tool-calling base | RF-09, RF-10, RF-11, RF-23 | Edición real con diff validado |
| S7 | RAG | RF-14, RF-15 | Decide RAG vs lectura (umbral por perfil) |
| S8 | Web + shell | RF-12, RF-13 | Búsqueda docs + shell sandbox |
| S9 | UI parte 1 | RF-18, RF-20 | Chat web streaming + aprobación |
| S10 | UI parte 2 + CLI | RF-19, RF-21, HU-09 | UI completa (VRAM x GPU) + CLI |
| S11 | Seguridad | RF-22, RF-22b, RF-24, HU-10, HU-11 | Guard en CPU bajo presión; 90% detección |
| S12 | Memoria + degradación | RF-08b, RF-16, RF-17, HU-03c | Sesión recuperable; degradación con gracia |
| S13 | Release | RNF-05, RNF-07, RNF-12, RNF-15 | v1.0 Docker <6 GB (A) / <8 GB (B) |

DoD: código + tests + docs + demo en A y B + revisión agente revisor + aprobación PO si hubo cambio de alcance.

---

## 7. Arquitectura

```
FRONTEND SvelteKit+TS (chat, visor, panel tool-calls, VRAM x GPU)
        | HTTP/SSE localhost
ORCHESTRATOR Rust Axum+Tokio
  HardwareProfiler | RouterContexto | ToolExecutor+sandbox
  DegradationManager | SessionManager | Guardarrailes
  AuditLog | Metrics
        | HTTP (API OpenAI) | HTTP | FFI PyO3
INFERENCE C++/CUDA          RAG Python            ML Python
llama.cpp fork TurboQuant   port-rag-potty        QLoRA (Unsloth)
                            ChromaDB + Ollama     bge-m3 embeddings
                            bge-m3                guard model <150M
                                                  router classifier
        | CUDA
RTX 30xx (A: 1 GPU 6 GB; B: 2 GPU 12 GB c/u)
```

### ADRs

- ADR-01: Orquestador Rust (Axum+Tokio) por I/O y CPU-bound, latencia predecible, seguridad memoria.
- ADR-02: ML en Python con PyO3 (sin overhead subprocesos).
- ADR-03: Inferencia C++/CUDA vía llama.cpp fork (control kernels TurboQuant). Fork concreto: `TheTom/llama-cpp-turboquant` (ver `adr/0004`).
- ADR-04: Frontend SvelteKit+TS (bundle pequeño, streaming SSE).
- ADR-05: HTTP para RAG. Inferencia también por HTTP (API OpenAI + SSE), no gRPC (modificada por `adr/0004`).
- ADR-06: Docker Compose + nvidia-container-toolkit.
- ADR-07: SQLite audit log + JSON sesión (cero deps externas, mono-usuario).
- ADR-08: Guardarraíles 2 capas: Rust determinista + Python semántico.
- ADR-09: Perfil B usa layer/pipeline split u offload asimétrico, no TP puro (PCIe sin NVLink).
- ADR-10: DegradationManager centraliza decisiones de recorte (contexto, quant KV, offload).
- Decisiones posteriores en `adr/`: 0005 (RF-06 sin ventana KV), 0006 (KV del perfil B), 0007 (vite-plugin-svelte 4), 0008 (retirar tonic), 0009 (LLM-juez vía opencode), 0010 (método del juez: absoluto, 25 tareas), 0011 (PCIe ≤x4 sin KV en RAM), 0012 (dataset QLoRA: Glaive v2 + sintético).

Cualquier ADR nueva o modificación de las existentes requiere aprobación del PO (ver sección 0).

### Flujo consulta

1. Frontend → SSE.
2. Orquestador: guardarraíl entrada (regex + clasificador Python).
3. Router contexto: RAG/lectura/resumen.
4. Prompt con contexto + herramientas.
5. llama-server HTTP/SSE (API OpenAI) + GBNF.
6. Streaming tokens → frontend.
7. Tool-call → guardarraíl salida → aprobación UI si destructiva → ToolExecutor.
8. Resultado vuelve al modelo (loop hasta respuesta final).
9. Audit log + session.json.

---

## 8. Stack

### Orquestador (I/O y CPU-bound)
Rust 1.80+, Axum 0.7+, Tokio 1.40+, PyO3 0.22+, serde/serde_json, rusqlite 0.31+, clap 4.5+, metrics+metrics-exporter-prometheus, tracing+tracing-subscriber.

### Inferencia (C++/CUDA)
llama.cpp fork `TheTom/llama-cpp-turboquant` (ADR-0004), CUDA 12.1+, CMake 3.28+ con -DGGML_CUDA=ON, llama-server vía HTTP/API OpenAI.

### ML (Python)
Python 3.11+, datos de tool-calling Glaive v2 (Apache-2.0) + sintéticos (ADR-0012), PyTorch 2.4+, Unsloth 2024.10+, PEFT 0.13+, bitsandbytes 0.44+, FlagEmbedding (bge-m3) vía Ollama, ModernBERT fine-tuned o Qwen3Guard-Gen-0.6B para guard, ChromaDB, pytest+pytest-cov+hypothesis, ruff+mypy.

### Frontend
SvelteKit 2.0+ con @sveltejs/vite-plugin-svelte 4+ (ADR-0007), TypeScript 5.5+, TailwindCSS 3.4+, EventSource (SSE), CodeMirror 6, uPlot 1.6+, Vitest+Playwright.

### Infra
Docker+docker compose+nvidia-container-toolkit, GitHub Actions (matrix 3060 6 GB Laptop, 3060 12 GB, 2x3060 12 GB en runners self-hosted), pre-commit (rustfmt, clippy, ruff, prettier), mdBook+Sphinx, SemVer+Conventional Commits.

Cualquier adición, sustitución o eliminación en el stack requiere aprobación del PO (sección 0).

---

## 9. Metodología

Scrum adaptado + Kanban soporte.

- Sprints 2 semanas. Planning lunes, daily 15 min async (#forge-daily), Review y Retro viernes semana 2.
- Kanban: Backlog → Ready → In Progress → Review → Done.
- Roles: PO = Sebastia M. (decisiones críticas, sección 0); SM rotativo; Dev = humano + agentes IA (Claude/Cursor/Aider) como pair.
- Artefactos: Product Backlog (sección 6), Sprint Backlog (issues GitHub labels `sprint:N`, `RF-XX`, `priority:P0/P1/P2`), Incremento = tag `v0.N.0-alpha` con demo en docker compose.
- Prácticas obligatorias: TDD (test antes de código), trunk-based development con feature flags, revisión por agente revisor + humano, Conventional Commits, cada PR debe pasar CI matrix A y B.
- Cambios de alcance de sprint en curso requieren aprobación del PO.

---

## 10. Testing y Umbrales

### Estrategia
- Unitarios: Rust (cargo test), Python (pytest), TS (vitest).
- Integración: docker compose con modelos pequeños de prueba (Qwen2.5-Coder-0.5B) en CI sin GPU + nightly en GPU real.
- E2E: Playwright UI + CLI end-to-end en ambos perfiles.
- Property-based: hypothesis (Python), proptest (Rust) para router, tool executor, guardarraíles.
- Rendimiento: criterion (Rust), pytest-benchmark (Python), llama-bench (inferencia).
- Adversarial: set público 500 prompts (injection, jailbreak) + 200 benignos para falsos positivos.
- Regresión de calidad de modelo: LLM-juez externo vía CLI de opencode con un modelo gratuito (por defecto `opencode/deepseek-v4.1-flash-free`, ADR-0009) evalúa 100 respuestas por release. Herramienta: `python -m forge_ml.judge`.

### Umbrales por RF (bloquean merge)
- RF-00/00b: 95% acierto perfil en CI hardware.
- RF-01: tiempos de carga dentro de umbral.
- RF-02: hot-swap <3 s.
- RF-03/03b: entrenamiento sin OOM; manifest completo.
- RF-05/06: VRAM y velocidad dentro de KPI.
- RF-07: cliente openai sin cambios.
- RF-08/08b: ≥90/95% tool-calls válidas; degradación registrada.
- RF-09..14: latencias y comportamientos de seguridad.
- RF-15: >85% precisión router.
- RF-16: LLM-juez ≥4/5 tras 3 resúmenes.
- RF-18..21: E2E Playwright verde en A y B.
- RF-22/22b: ≥90/95% detección, <8/5% falsos positivos, latencia <200 ms CPU.
- RF-23/24: 100% inválidas rechazadas; log no modificable.
- RNF-07: cobertura ≥80/60%.
- RNF-08: cero warnings.
- RNF-13: demo e2e en 3060 6 GB <5 min.

Cambiar cualquier umbral requiere aprobación del PO (sección 0).