# ADR-0013: Modelos base — Qwen3.5-4B (perfil A) y Qwen3.6-35B-A3B MoE (perfil B)

- **Estado**: Aceptada
- **Fecha**: 2026-11-09
- **Decisor (PO)**: Sebastia M.
- **Issue relacionada**: RF-01, RF-08, HU-00, HU-03, HU-03b
- **Modifica**: SDD §1 (propósito y KPI de modelo), §2 (estrategias y matriz),
  §3.2 (RF-01), §5 (HU-00); ADR-0006 (el modelo del perfil B)

## Contexto

- Qwen2.5-Coder-3B-Instruct, el modelo del perfil A, usa la licencia
  `qwen-research`, no Apache-2.0.
- El PO quiere un modelo más pesado y MoE. Qwen3.6 solo publica dos modelos
  abiertos, ambos Apache-2.0: `Qwen3.6-35B-A3B` (MoE, 35B en total y 3B activos
  por token) y `Qwen3.6-27B` (denso).
- El Q4_K_M del 35B-A3B ocupa 21.1 GiB: no cabe en 6 GB, así que va al perfil
  B (2x12 GB). Qwen3.6 no tiene modelos pequeños.

## Decisión

| Perfil | Modelo | Licencia | GGUF |
|---|---|---|---|
| A (6 GB) | `Qwen/Qwen3.5-4B` | Apache-2.0 | Q4_K_M (~2.6 GiB) |
| B (2x12 GB) | `Qwen/Qwen3.6-35B-A3B` | Apache-2.0 | `unsloth/Qwen3.6-35B-A3B-GGUF`, UD-Q4_K_M (21.1 GiB) |

Flags del perfil B (los imprime `forge doctor`):
`-c 65536 -ngl 99 -fa on -np 1 -ctk f16 -ctv turbo3 --n-cpu-moe 2 --reasoning off`.

- `--n-cpu-moe 2`: con todo en GPU faltan ~1.2 GB; dejar los expertos de 2
  capas en RAM lo resuelve.
- `--reasoning off`: con el modo thinking activo, la gramática de tool-calls
  da 0% (el modelo empieza pensando y la gramática lo impide).

## Mediciones (2026-11-09, `warmachine`, 2x RTX 3060 12 GB)

Perfil B, Qwen3.6-35B-A3B UD-Q4_K_M, K f16 / V turbo3:

| Prueba | Resultado | Umbral |
|---|---|---|
| HU-03b, contexto lleno (64 769 tokens) | **34.8 tok/s**, 11 078 / 10 722 MiB | ≥22 tok/s, ≤11 673 MiB/GPU ✅ |
| RF-08 sin gramática / con gramática | **100% / 100%** | ≥95% ✅ |
| `llama-bench` tg64, `--n-cpu-moe` 2 / 4 / 8 | @4K: 67.1 / 65.2 / 41.7; @64K: 35.1 / 34.0 / 31.8 | — |

Frente al 7B de ADR-0006 (23.7 tok/s a 64K y 97% en RF-08 con LoRA), Qwen3.6
es un 47% más rápido con el contexto lleno y no necesita LoRA para RF-08.

El perfil A (Qwen3.5-4B) está pendiente de medir: hace falta el GGUF.

## Alternativas consideradas

- **Qwen3.6-35B-A3B también en A**, con los expertos en RAM y una cuantización
  de ~2 bits (UD-IQ2_M, 10.7 GiB). Choca con RNF-03b (RAM ≤12 GB en A) y la
  calidad a 2 bits no está medida.
- **Qwen3.6-27B denso en B.** No es MoE: ~16 GiB en Q4 y menos tok/s.

## Consecuencias

- Los niveles A+ y Premium de la matriz siguen con Qwen2.5-Coder 7B; no entran
  en esta decisión.
- El adaptador QLoRA del 3B (S4) deja de aplicar al perfil A. Qué hacer con
  RF-03 en cada perfil queda pendiente del PO: entrenar Qwen3.5-4B y decidir
  si el 35B-A3B necesita LoRA.
- RNF-01 y RNF-03 del perfil B se cumplen con margen.
