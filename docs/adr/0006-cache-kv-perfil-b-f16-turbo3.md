# ADR-0006: Caché KV del perfil B — K f16, V turbo3

- **Estado**: Aceptada
- **Fecha**: 2026-11-09
- **Decisor (PO)**: Sebastia M.
- **Issue relacionada**: RF-05, RNF-01, HU-00, HU-03b (Sprint S3)
- **Modifica**: SDD §2 (perfil B), §5 (HU-00)

## Contexto

El SDD fija turbo3 para el perfil B (7B Q4_K_M, 64K, 2x RTX 3060 12 GB). Al
pedir `-ctk turbo3 -ctv turbo3`, el fork sube K a `q8_0` por el GQA 7:1 del
modelo (auto-asimetría). Medido a 64K (`docs/inference.md`):

| K / V | KV | tok/s @64K | KLD vs f16/f16 | Mismo top-1 |
|---|---:|---:|---:|---:|
| f16 / f16 | 3584 MiB | 39.2 | — | — |
| **f16 / turbo3** | ≈2142 MiB | **23.8** | **0.0040** | **98.25%** |
| q8_0 / turbo3 | 1302 MiB | 17.7 | 0.0056 | 98.10% |
| turbo3 / turbo3 | 700 MiB | 16.9 | — | — |

KPI de B: KV <3 GB y ≥22 tok/s con contexto lleno.

## Decisión

El perfil B arranca el llama-server con `-ctk f16 -ctv turbo3`. `forge doctor`
lo recomienda así. El resto de perfiles no cambia (A: turbo2/turbo2).

## Alternativas consideradas

- **A — f16/turbo3 (elegida).** Única combinación medida que cumple los dos
  KPI; además tiene la menor divergencia frente a f16 de las combinaciones
  turbo.
- **B — turbo3 tal cual (q8_0/turbo3).** Menos VRAM, pero 17.7 tok/s.
- **C — f16/f16.** La más rápida, pero 3.6 GB de KV.

## Consecuencias

- Margen de velocidad pequeño (23.8 frente a 22 tok/s), medido en GPUs de
  escritorio con reparto por capas por defecto. RF-06b (reparto medido por
  PCIe) puede moverlo en cualquier sentido; hay que volver a medir en S3.
- Calidad: KL-divergence medida sobre 10 trozos de 8192 tokens de código
  (C++, Rust, Python) y el SDD: KLD media 0.0040, PPL +0.16%. El LLM-juez de
  AGENTS.md 11.5 (≥4/5 en 50 prompts) se hace con opencode (ADR-0009) y está
  pendiente de completarse.
- Si la VRAM aprieta, RF-08b puede bajar K a `q8_0` antes de recortar contexto.
