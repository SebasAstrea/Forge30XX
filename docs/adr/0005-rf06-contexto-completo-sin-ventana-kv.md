# ADR-0005: RF-06 sin ventana KV — contexto completo en VRAM con TurboQuant

- **Estado**: Aceptada
- **Fecha**: 2026-11-09
- **Decisor (PO)**: Sebastia M.
- **Issue relacionada**: RF-06, RF-08b, HU-00, HU-03 (Sprints S2–S3)
- **Modifica**: SDD §1, §2, §3.3 (RF-06, RF-08b), §5 (HU-00), §6 (S2), §7

## Contexto

RF-06 pedía una ventana "KVFlash" configurable (4K en A, 8K en B) con
velocidad estable (<20% de variación entre 4K y el contexto máximo). El fork
fijado en ADR-0004 no contiene nada llamado KVFlash. Lo más cercano es *block
KV cache streaming* (`--kv-stream-arena-mib`), que deja en GPU una parte de las
páginas KV y el resto en RAM. Con Qwen2.5-Coder no aporta: solo acelera
modelos con head dim 256 (Qwen2.5 usa 128), exige `-np 1`, desactiva el
context shift y obliga a compilar con `GGML_CUDA_FA_ALL_QUANTS=ON`.

Las mediciones de `docs/inference.md` (2026-11-09) muestran que la caché KV
completa ya cabe en VRAM con TurboQuant: 153–408 MiB a 32K (3B) y
0.7–2.1 GB a 64K (7B). No hace falta sacar KV a RAM.

## Decisión

RF-06 pasa a ser: contexto máximo (32K en A, 64K en B) con toda la caché KV en
VRAM, mediante TurboQuant y flash attention, sin ventana. Criterio de
aceptación: KV a contexto máximo dentro del KPI (A <1.5 GB, B <3 GB). La
velocidad con contexto lleno la cubre RNF-01. El concepto "ventana KVFlash"
desaparece del SDD, de `forge doctor` y de `forge.config.toml`.

## Alternativas consideradas

- **A — Adoptar kv-stream como KVFlash.** Usa código del fork, pero con
  Qwen2.5 no mejora la velocidad y añade restricciones de servidor. No
  cumpliría el criterio original.
- **B — Redefinir RF-06 (elegida).** Sin código nuevo; el criterio de VRAM ya
  se cumple en A y B.
- **C — Ventana KV propia en C++/CUDA.** Varios sprints de kernels, riesgo
  alto, fuera del MVP.

## Consecuencias

- La velocidad cae entre un 26% y un 69% de 4K al contexto máximo (medido). Se
  acepta mientras RNF-01 se cumpla (≥12 tok/s A, ≥22 tok/s B).
- RF-08b pierde la opción "reducir ventana KVFlash"; quedan turbo3→turbo2 y
  reducir contexto.
- `hardware::Recommendation` deja de exponer `kvflash_window` y expone los
  tipos K y V por separado (ver ADR-0006).
- Si en el futuro se usa un modelo con head dim 256, kv-stream puede
  reevaluarse con una ADR nueva.
