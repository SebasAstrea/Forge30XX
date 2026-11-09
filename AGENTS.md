## AGENTS.md

Reglas vinculantes para agentes codificantes (Claude, Cursor, Aider, etc.) y humanos.

### 11.-1 Lectura obligatoria antes de tocar código

Todo agente (y humano) debe leer, en este orden, antes de empezar cualquier tarea:

1. **`docs/SDD.md`** — Spec-Driven Development. Define propósito, KPIs, perfiles de hardware (A/B), requisitos funcionales (RF) y no funcionales (RNF), historias de usuario (HU), backlog por sprint, arquitectura, ADRs, stack, metodología y umbrales de testing. **Autoridad de decisión en su sección 0.**
2. **`AGENTS.md`** (este archivo) — Reglas vinculantes de ingeniería: principios, flujo de trabajo, reglas por capa, seguridad, rendimiento, commits/PRs y anti-patrones.
3. **`docs/adr/`** — Architecture Decision Records. Consultar las ADRs vigentes antes de proponer o modificar arquitectura o stack.
4. **`README.md`** — Visión general, instalación y uso. Contexto de alto nivel para onboarding.

Documentación de referencia adicional (consultar según la tarea):

- `docs/openapi.yaml` — contrato de la API (cuando exista).
- `CHANGELOG.md` — historial de cambios visibles (Keep a Changelog).

Regla: si el SDD y una instrucción del PO (Sebastia M.) entran en conflicto, prevalece el PO, pero debe quedar registrado como ADR en `docs/adr/`. Si el SDD y este AGENTS.md entran en conflicto en una regla de ingeniería, bloquear la tarea y consultar al PO (ver 11.0).

### 11.0 Autoridad y decisiones críticas

Product Owner: **Sebastia M.** Ante cualquier decisión de arquitectura, cambio de stack, modificación de funciones, requisitos, HUs, alcance del MVP, metodología o umbrales, el agente debe:

1. Detener la tarea afectada.
2. Formular pregunta estructurada al PO: contexto, opciones (mínimo 2), trade-offs (rendimiento, VRAM, complejidad, tiempo, riesgo), recomendación con justificación, impacto en RF/RNF/HUs/sprints.
3. Registrar issue con label `decision:po` y enlazar a la tarea bloqueada.
4. Esperar respuesta. Avanzar solo en tareas no afectadas.
5. Documentar respuesta como ADR y reflejar cambios en SDD y backlog.

Prohibido: improvisar decisiones de arquitectura, asumir stack, añadir/quitar RF, repriorizar HUs, cambiar umbrales, o alterar el alcance del MVP sin aprobación del PO.

### 11.1 Principios

- Si una regla no se verifica con test o linter, no es vinculante. Toda regla debe tener verificación.
- SOLID obligatorio: Single Responsibility, Open/Closed, Liskov, Interface Segregation, Dependency Inversion.
- DRY, KISS, YAGNI. Sin abstracciones especulativas.
- Funciones ≤40 líneas. Archivos ≤400 líneas. Complejidad ciclomática ≤10 por función.
- Sin código muerto, sin TODOs sin issue asociado, sin comentarios obvios. Comentarios solo para "por qué", no "qué".
- Nombres en inglés en código; docs y commits pueden ser español o inglés, consistente por módulo.
- Sin `unwrap()`/`expect()` en Rust fuera de tests; usar `Result` y propagar con `?`.
- Sin `print()` en Python; usar `logging` con logger nombrado.
- Sin `any` implícito en TS; `strict: true`.
- Sin dependencias nuevas sin ADR aprobado por el PO.

### 11.2 Flujo de trabajo del agente

1. Leer issue asignado + RF asociado + criterios de aceptación.
2. Si la tarea implica decisión crítica (sección 11.0), detener y consultar al PO.
3. Escribir test que falla (TDD). Ejecutar y confirmar fallo.
4. Implementar mínimo código para pasar test. Ejecutar.
5. Refactorizar manteniendo verde.
6. Ejecutar `make check` (fmt + lint + test + coverage). Si falla, corregir antes de commit.
7. Ejecutar `make bench` si toca módulos de rendimiento (inferencia, router, guardarraíles). Comparar contra baseline; regresión >5% bloquea.
8. Commit Conventional Commits con scope del RF (`feat(RF-15): add context router thresholds per profile`).
9. Abrir PR con plantilla: RF cubierto, perfiles A/B verificados, benchmarks, riesgos, decisión PO si aplica.

### 11.3 Reglas por capa

#### Rust (orquestador)
- `#![deny(unsafe_code)]` excepto en módulo FFI aislado (`ffi/`), con `// SAFETY:` documentado.
- Errores con `thiserror` en librería, `anyhow` en binarios.
- Async: no bloquear runtime; CPU-bound va a `tokio::task::spawn_blocking`.
- Concurrencia: preferir canales (`tokio::sync::mpsc`) sobre `Mutex` compartido.
- Configuración con `figment` o `config` crate; sin hardcode de rutas o puertos.
- Métricas en cada endpoint y cada decisión del router/degradation manager.

#### Python (ML)
- Type hints obligatorios; `mypy --strict` en módulos core.
- `ruff` + `black` (o `ruff format`).
- Sin `import *`. Sin globals mutables. Dataclasses o pydantic para estructuras.
- Reproducibilidad: `torch.manual_seed`, `numpy.random.seed`, `random.seed` al inicio de cada script de entrenamiento; registrar en manifest.
- Recursos: usar `with` para archivos, context managers para modelos; liberar VRAM con `torch.cuda.empty_cache()` tras inferencia de guard.
- Unsloth obligatorio para QLoRA en Perfil A; `device_map="auto"` en Perfil B.
- Sin `pickle` para artefactos; usar `safetensors` o GGUF.

#### C++/CUDA (inferencia)
- Cambios en kernels CUDA requieren benchmark antes/después en CI hardware (3060 6 GB y 2x3060).
- Sin `malloc`/`new` en hot path; usar memoria gestionada por llama.cpp.
- Errores CUDA chequeados con macro `CUDA_CHECK`.
- No modificar kernels de TurboQuant sin test de regresión numérica (comparar salida vs versión anterior en 100 prompts).

#### TypeScript (frontend)
- `strict: true`, sin `any`, sin `as` innecesario.
- Componentes Svelte ≤150 líneas; extraer lógica a stores o utils.
- SSE con reconexión exponencial; mostrar estado de conexión al usuario.
- Sin `console.log` en producción; usar logger con nivel.
- Accesibilidad: `aria-*`, navegación teclado, contraste WCAG AA.

### 11.4 Reglas de seguridad

- Nunca ejecutar comandos shell arbitrarios. Tool executor valida contra lista blanca y timeout.
- Nunca exponer rutas fuera del workspace declarado. Canonicalizar y verificar prefijo.
- Nunca loggear secretos, tokens o PII. Redactar antes de escribir en audit log.
- Guardarraíl de entrada obligatorio antes de cada llamada al modelo. Si falla, responder con error estructurado y registrar.
- Guardarraíl de salida obligatorio antes de cada ejecución de tool-call.
- Audit log append-only; nunca modificar ni borrar entradas.

### 11.5 Reglas de rendimiento

- Todo cambio en rutas calientes (router, inferencia, KV management) requiere benchmark en A y B.
- Regresión >5% en tok/s o >10% en latencia p95 bloquea merge.
- Cambios en cuantización KV requieren test de calidad (LLM-juez ≥4/5 en 50 prompts) además de benchmark.

### 11.6 Reglas de compatibilidad

- Todo PR debe pasar CI matrix: 3060 6 GB Laptop, 3060 12 GB, 2x3060 12 GB.
- Si un cambio solo funciona en B, debe incluir fallback funcional en A.
- Nuevas features deben declarar perfil mínimo soportado en la issue.

### 11.7 Reglas de documentación

- Cada RF debe tener entrada en docs con ejemplo ejecutable.
- Cada ADR nueva en `docs/adr/NNNN-titulo.md`, aprobada por el PO.
- Cada endpoint API con OpenAPI spec en `docs/openapi.yaml`.
- CHANGELOG.md actualizado por release (Keep a Changelog).

### 11.8 Reglas de commits y PRs

- Conventional Commits: `type(scope): mensaje`. Tipos: feat, fix, perf, refactor, test, docs, build, ci, chore.
- Scope = ID de RF o módulo (`RF-15`, `inference`, `ui`).
- PR ≤400 líneas de diff (excluyendo tests). PRs grandes se parten.
- PR debe enlazar issue y RF. Sin issue, no se mergea.
- Un revisor humano mínimo + agente revisor automático.
- PR que toque decisión crítica debe enlazar la respuesta del PO y la ADR resultante.

### 11.9 Checklist del agente antes de commit

- [ ] Test escrito y verde.
- [ ] `make check` pasa (fmt, lint, test, coverage).
- [ ] `make bench` sin regresión >5% si toca hot path.
- [ ] CI matrix A y B verde.
- [ ] Docs actualizadas si cambia API o comportamiento.
- [ ] CHANGELOG actualizado si es feature o fix visible.
- [ ] Sin TODOs sin issue.
- [ ] Sin secretos ni PII en código o logs.
- [ ] Principios SOLID respetados (revisión manual).
- [ ] Complejidad y tamaño dentro de límites.
- [ ] Si hubo decisión crítica, respuesta del PO documentada como ADR y SDD actualizado.

### 11.10 Comandos make obligatorios

```
make check          # fmt + lint + test + coverage
make bench          # benchmarks rendimiento
make test-a         # tests perfil A (3060 6 GB)
make test-b         # tests perfil B (2x3060 12 GB)
make train-lora PROFILE=A|B
make doctor         # detección hardware + recomendación
make docs           # rustdoc + Sphinx + mdBook
make release        # build Docker A y B + tags
```

### 11.11 Anti-patrones prohibidos

- Modificar pesos manualmente sin LoRA (salvo hotfix de seguridad con ADR aprobada por PO).
- Usar `unsafe` fuera de `ffi/`.
- Hardcodear rutas, puertos, nombres de modelo.
- Ignorar warnings con `#[allow(...)]` sin justificación en comentario.
- Añadir dependencia sin ADR aprobada por PO.
- Mergear sin CI matrix A y B verde.
- Cambiar umbrales de KPI sin actualizar este SDD y aprobación del PO.
- Escribir tests que dependan de orden de ejecución o estado global.
- Mockear la GPU en tests de integración reales (usar modelos pequeños reales en CI hardware).
- Tomar decisiones de arquitectura, stack, funciones, HUs o alcance sin consultar al PO Sebastia M.

### 11.12 Escalado de dudas

Si el agente no puede decidir:
1. Consultar este SDD y AGENTS.md.
2. Consultar ADRs existentes.
3. Si la duda es crítica (sección 11.0), formular pregunta al PO Sebastia M. vía issue `decision:po`.
4. Si no es crítica, abrir issue con label `question` y bloquear la tarea.
5. No inventar requisitos ni umbrales.