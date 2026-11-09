# inference/ — C++/CUDA

Capa de inferencia de Forge30XX. El motor es el fork
[`TheTom/llama-cpp-turboquant`](https://github.com/TheTom/llama-cpp-turboquant)
(rama `feature/turboquant-kv-cache`, commit `643956891`), con los tipos de
caché KV `turbo2`…`turbo6` (RF-05). Su `llama-server` se usa como proceso
externo y el orquestador le habla por HTTP con la API OpenAI (ADR-0004); no hay
gRPC ni FFI en el camino de inferencia.

Estado: el `CMakeLists.txt` de este directorio es un esqueleto. El fork se
compila fuera del repo (ver `docs/inference.md`). Qué mecanismo cubre KVFlash
(RF-06) está pendiente de decisión del PO.

Reglas de capa: ver `AGENTS.md` 11.3 (C++/CUDA) — sin `malloc`/`new` en hot path,
`CUDA_CHECK` en errores, test de regresión numérica antes de tocar kernels.
