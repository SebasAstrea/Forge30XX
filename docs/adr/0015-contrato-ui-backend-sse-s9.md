# ADR-0015: Contrato UI↔backend del S9 (servidor HTTP, SSE, aprobación de tool-calls)

- **Estado**: Aceptada
- **Fecha**: 2026-11-12
- **Decisor (PO)**: Sebastia M.
- **Issue relacionada**: RF-18, RF-20 (Sprint S9)

## Contexto

Hasta S9 el orquestador es una CLI pura: `axum` figura en `Cargo.toml` pero no
hay servidor HTTP, ni endpoints, ni re-emisión de SSE hacia un navegador, ni
ejecutor de tool-calls, ni noción de tool-call "destructiva". El subcomando
`run --ui` era un stub. ADR-0003 ya fijó que la UI web de RF-18 reutilizaría el
cliente de inferencia con streaming.

S9 (RF-18 chat web con streaming + visor + panel de tool-calls; RF-20
aprobar/rechazar tool-calls destructivas, "sin aprobación no ejecuta") exige
definir la **topología de comunicación UI↔backend**, lo que por AGENTS.md 11.0 y
SDD §0 es decisión del PO.

Restricciones: RNF carga de UI <2 s; AGENTS.md 11.3 (SSE con reconexión
exponencial, estado de conexión visible); 11.7 (OpenAPI cuando exista API);
verificable sin GPU en el equipo Windows (Perfil A), con el llama-server
mockeado.

## Decisión

1. **Servidor HTTP en el orquestador (Axum)** levantado por `forge run --ui` en
   `cfg.ui.port` (7860). Sirve los estáticos de SvelteKit y expone una API local
   bajo `/api`.
2. **Transporte del streaming: SSE** (`text/event-stream`). Server→cliente para
   tokens; las acciones cliente→server son POST discretos. El navegador usa
   `EventSource` (reconexión nativa; la UI además muestra el estado de conexión).
3. **Endpoints**:
   - `POST /api/chat` → `text/event-stream`. Body `{ "messages": [{role, content}] }`.
   - `POST /api/tools/{id}/decision` → body `{ "decision": "approve" | "reject" }` (RF-20).
   - `GET /api/health` → estado del llama-server (indicador de conexión).
   - `GET /` y estáticos → UI SvelteKit (`tower-http` `ServeDir`, adapter-static
     con `fallback: index.html`).
4. **Esquema de eventos SSE** (`event:` + `data:` JSON), superset del contrato
   OpenAI del llama-server:
   - `token`        → `{ "text": "..." }`
   - `tool_request` → `{ "id", "name", "arguments", "destructive" }`
   - `tool_result`  → `{ "id", "status": "approved"|"rejected"|"executed" }`
   - `error`        → `{ "message": "..." }`
   - `done`         → `{}`
5. **Clasificación de "destructiva": lista blanca fail-safe.** Todo tool-call
   requiere aprobación **salvo** un conjunto explícito de solo-lectura. Una tool
   no listada es destructiva por defecto. La clasificación vive en el
   orquestador Rust (`tool_call`), no en el frontend, para que no se pueda
   eludir desde el cliente.
6. **Dependencia nueva aprobada**: `tower-http` (feature `fs` para `ServeDir`),
   de la familia Axum/Tower.
7. **Alcance**: en S9 el flujo de aprobación existe end-to-end, pero la
   ejecución efectiva de las tools (RF-09/10/11) queda como stub; la ejecución
   real es S6.

## Alternativas consideradas

- **Transporte** — SSE (elegida) vs WebSocket. SSE reutiliza el protocolo ya
  consumido del llama-server, trae reconexión nativa en el navegador y es
  trivial de mockear; el flujo es server→cliente para tokens. WebSocket sería
  bidireccional pero añade upgrade, ping/pong y reconexión manual sin
  justificación para token-streaming.
- **Clasificación destructiva** — Lista blanca fail-safe (elegida) vs lista
  negra. La lista negra es fail-open: una tool nueva no listada se ejecutaría
  sin aprobación, en contra de RF-11/RF-20 y de AGENTS.md 11.4.
- **Estáticos** — `tower-http` `ServeDir` (elegida) vs servirlos a mano desde
  Axum (más código, menos robusto).

## Consecuencias

- Cubre RF-18 y RF-20. No altera umbrales ni otros RF.
- Nuevo componente (servidor HTTP Axum) y topología HTTP local. Se añade
  `tower-http` como dependencia (AGENTS.md 11.1/11.11, aprobada aquí). Como
  utilidades de implementación del transporte SSE aprobado se añaden además
  `tokio-stream` (adaptador `Stream` para los canales de tokio) y, solo en
  `[dev-dependencies]`, `tower` + `http-body-util` (conducir el router en tests
  sin abrir sockets). En el frontend se declara `svelte-eslint-parser` (ya
  transitivo de `eslint-plugin-svelte`) para que ESLint analice `<script
  lang="ts">`.
- Se crea `docs/openapi.yaml` (AGENTS.md 11.7).
- RF-19 (VRAM en vivo) y RF-21 (CLI) quedan fuera: son S10.
- La ejecución real de tool-calls queda en S6; S9 deja stub tras la aprobación.
- Verificable en el equipo Windows (Perfil A) con el llama-server mockeado; sin
  intervención de GPU ni entrenamiento.
