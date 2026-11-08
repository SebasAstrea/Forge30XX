# ADR-0003: Adelantar el streaming de tokens (RF-18) a S1

- **Estado**: Aceptada
- **Fecha**: 2026-11-08
- **Decisor (PO)**: Sebastia M.
- **Issue relacionada**: RF-07, RF-18 (Sprint S1)

## Contexto

El backlog del SDD (§6) sitúa el streaming de tokens dentro de RF-18 (UI web,
Sprint S9). En S1, `InferenceClient::chat` usa `stream: false`: `forge chat`
no muestra nada hasta que el modelo termina. En el perfil A (≥12 tok/s), una
respuesta de 1000 tokens supone más de 80 s sin feedback.

## Decisión

El streaming de tokens desde el llama-server (SSE de la API OpenAI) se
implementa en S1, en el cliente de inferencia del orquestador y en `forge chat`.
La UI web de RF-18 sigue en S9 y reutilizará este cliente.

No se añaden dependencias: se lee el cuerpo con `reqwest::Response::chunk()`
(ya disponible, ADR-0002) y el parser SSE es propio y mínimo.

## Alternativas consideradas

- **A — Adelantar a S1 (elegida).** Feedback inmediato en CLI; S9 parte de un
  cliente probado. Coste: algo más de trabajo en S1.
- **B — Mantener en S9.** S1 más corto, pero `forge chat` sigue sin feedback y
  el streaming se probaría por primera vez junto con la UI.

## Consecuencias

- S1 pasa a cubrir RF-01, RF-07, RF-08 y la parte de streaming de RF-18.
- El timeout de inferencia deja de ser total en streaming: se aplica entre
  trozos recibidos (`read_timeout`), para no cortar generaciones largas.
- Sin impacto en umbrales ni en otros RF.
