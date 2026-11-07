# ADR-0002: Cliente HTTP en Rust para consumir el servicio RAG

- **Estado**: Propuesta (requiere aprobación del PO — AGENTS.md 11.1, 11.11)
- **Fecha**: 2026-11-07
- **Decisor (PO)**: Sebastia M.
- **Issue relacionada**: RF-14, RF-15 (Sprint S7). Depende de ADR-0001.

## Contexto

ADR-0001 fijó que el RAG (`port-rag-potty`) corre como servicio externo y que
Forge lo consume por HTTP. El lado Python ya tiene su cliente
(`forge_rag.RagClient`). En el orquestador Rust solo existe la lógica pura
`rag::RouterPolicy` (umbrales RF-15); **falta el transporte HTTP** que llame a
`/route`, `/query`, `/methodology`, `/health` y `/reindex` del servicio.

Implementar ese transporte exige un cliente HTTP. AGENTS.md prohíbe añadir
dependencias nuevas sin una ADR aprobada por el PO (11.1 y anti-patrón 11.11),
de ahí esta ADR. El stack ya aprobado (SDD §8) incluye Axum 0.7, Tokio 1.40 y
tonic 0.12, por lo que `hyper`/`tower`/`http` ya están en el árbol transitivo.

Requisitos del cliente:

- Async sobre el runtime Tokio ya existente (no bloquear; AGENTS.md 11.3).
- POST/GET con cuerpos JSON (serde_json, ya presente) y timeouts configurables
  (la config `[rag] timeout_ms` ya existe).
- Solo HTTP a `localhost`/red interna de Compose; **sin TLS** necesario (RNF-06:
  cero salidas de red salvo RF-12). Evitar arrastrar OpenSSL/rustls si se puede.
- Superficie mínima y mantenible.

## Decisión (propuesta)

Añadir **`reqwest`** como dependencia del orquestador, configurada de forma
mínima para no arrastrar TLS ni dependencias de sistema:

```toml
reqwest = { version = "0.12", default-features = false, features = ["json"] }
```

`default-features = false` desactiva el TLS por defecto (no hacemos HTTPS hacia
el RAG, que es un servicio local); `features = ["json"]` integra serde_json que
ya usamos. `reqwest` se apoya en `hyper` + `tokio`, ya presentes, así que el
coste incremental de compilación y binario es bajo.

## Alternativas consideradas

- **A — `reqwest` (0.12), sin TLS, feature `json`.** API de alto nivel, estable,
  estándar de facto, integra con Tokio y serde. Contras: capa extra sobre hyper;
  si en el futuro se necesita TLS habrá que añadir feature `rustls-tls`.
- **B — `hyper` 1.x + `hyper-util` directamente.** Cero dependencias nuevas de
  "alto nivel" (hyper ya está). Contras: API de bajo nivel verbosa (construir
  `Request`, gestionar el `Connector`, el pool y la (de)serialización a mano),
  más código propio que mantener y testear → mayor superficie de bug para una
  tarea trivial. Choca con KISS.
- **C — `ureq` (cliente síncrono, sin Tokio).** Muy ligero y simple. Contras:
  es bloqueante; habría que envolver cada llamada en `tokio::task::spawn_blocking`
  (AGENTS.md 11.3), añadiendo complejidad en el lado async; es un ecosistema
  distinto al de hyper/tower ya presente, menos coherente con ADR-01.

## Consecuencias

Positivas: desbloquea S7 (transporte HTTP del RAG en Rust) con código mínimo y
coherente con el runtime Tokio; sin TLS → sin dependencias de sistema (OpenSSL),
favorable a RNF-05 (imagen Docker pequeña) y al build en CI. Reutiliza `hyper`
ya presente.

Negativas: una dependencia directa más que auditar y versionar. Si más adelante
se requiere HTTPS (no previsto para el RAG local), habrá que habilitar
`rustls-tls` y actualizar esta ADR.

Impacto: no altera RF/RNF ni umbrales. Afecta solo a la implementación de
RF-14/RF-15 en el orquestador (Sprint S7). Si el PO rechaza `reqwest`, la
alternativa de menor fricción es B (hyper directo) a costa de más código propio.

## Pendiente de aprobación

Bloquea la tarea de implementar el cliente HTTP del RAG en Rust hasta respuesta
del PO. Al aprobarse: cambiar estado a "Aceptada", añadir la dependencia a
`orchestrator/Cargo.toml` y actualizar el CHANGELOG.
