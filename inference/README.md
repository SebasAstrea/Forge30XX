# inference/ — C++/CUDA

Capa de inferencia de Forge30XX. Integra el fork
[`atomicmilkshake/llama-cpp-turboquant`](https://github.com/atomicmilkshake/llama-cpp-turboquant)
con los kernels TurboQuant (cuantización KV) y KVFlash (ventana de atención).

Estado: esqueleto de bootstrap. La integración (fork como submódulo, build con
`-DGGML_CUDA=ON`, servidor gRPC) se implementa en los Sprints S1-S3.

Reglas de capa: ver `AGENTS.md` 11.3 (C++/CUDA) — sin `malloc`/`new` en hot path,
`CUDA_CHECK` en errores, test de regresión numérica antes de tocar kernels.

Build (cuando el fork esté integrado):

```bash
cmake -S . -B build -DGGML_CUDA=ON
cmake --build build --config Release
```
