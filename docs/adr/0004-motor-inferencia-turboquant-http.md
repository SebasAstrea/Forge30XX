# ADR-0004: Motor de inferencia — fork TurboQuant servido por HTTP/OpenAI

- **Estado**: Aceptada
- **Fecha**: 2026-11-08
- **Decisor (PO)**: Sebastia M.
- **Issue relacionada**: RF-05, RF-06, RF-07 (Sprints S1–S3)
- **Modifica**: ADR-03 y ADR-05 del SDD (§7)

## Contexto

El SDD fija el fork `atomicmilkshake/llama-cpp-turboquant` (ADR-03) y gRPC para
inferencia (ADR-05). S1 se implementó contra un llama-server estándar vía la
API OpenAI por HTTP. RF-05 (turbo2/turbo3) y RF-06 (KVFlash) requieren un fork
con esos tipos de caché KV.

El PO dispone de una copia local del fork
[`TheTom/llama-cpp-turboquant`](https://github.com/TheTom/llama-cpp-turboquant)
(rama `feature/turboquant-kv-cache`, commit `643956891`), que expone los tipos
`GGML_TYPE_TURBO2_0` … `GGML_TYPE_TURBO6_0`.

llama-server no ofrece gRPC; usarlo exigiría un puente propio en C++.

## Decisión

1. Se usa el fork **TheTom/llama-cpp-turboquant** como motor de inferencia, en
   lugar de `atomicmilkshake/llama-cpp-turboquant`.
2. El orquestador habla con su `llama-server` por **HTTP, API OpenAI**
   (`/v1/chat/completions` con SSE). No se usa gRPC para inferencia.
3. Si el fork no sirve (no compila para Ampere o no cumple RF-05), se mantiene
   la misma topología HTTP con otro fork (opción A del PO).

## Alternativas consideradas

- **A — Fork + HTTP/OpenAI (elegida).** Cero código nuevo en el orquestador;
  cumple RF-07 de forma directa; sin `tonic` en el camino de inferencia.
  Contra: SSE es unidireccional (suficiente para streaming de tokens).
- **B — Fork + gRPC (ADR-05 original).** Streaming bidireccional, pero requiere
  escribir y mantener un servidor gRPC en C++ sobre llama.cpp y su `.proto`.

## Consecuencias

- `tonic` queda sin uso en inferencia. Retirarlo del `Cargo.toml` es una
  decisión aparte.
- Punto abierto: el fork no contiene ninguna implementación llamada KVFlash
  (RF-06). Antes de S2 hay que identificar qué mecanismo del fork cubre la
  ventana KV en GPU o tratarlo como trabajo propio.
- El SDD §7 (ADR-03, ADR-05) y §8 (Inferencia) se actualizan para reflejarlo.
