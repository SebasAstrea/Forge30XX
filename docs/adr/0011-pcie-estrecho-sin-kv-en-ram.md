# ADR-0011: Enlace PCIe ≤x4 — mantener el reparto por capas, sin KV en RAM

- **Estado**: Aceptada
- **Fecha**: 2026-11-09
- **Decisor (PO)**: Sebastia M.
- **Issue relacionada**: RF-06b, HU-00b (Sprint S3)
- **Modifica**: SDD §2 (perfil B, fallback), §3.3 (RF-06b), §5 (HU-00b)

## Contexto

El SDD proponía que, si una GPU del perfil B está en un enlace PCIe x4, la
caché KV se mueva a la RAM del sistema y esa GPU solo reciba capas (contexto
48K, ≥18 tok/s). Medido en `warmachine` (7B Q4_K_M, f16/turbo3, 64K;
`docs/inference.md`):

- Reparto por capas con GPU0 en **Gen3 x1** (~1 GB/s): 23.5 tok/s.
- KV en RAM (`-nkvo`): **0.6 tok/s**.

Con el reparto por capas, entre GPUs solo viajan las activaciones de cada
token (unos KB). Por eso un enlace estrecho apenas afecta a la generación; sí
alarga la carga del modelo (7.4 s frente a 1.8 s en x16).

## Decisión

Con un enlace de x4 o menos se mantiene el reparto por capas (y su KV) que
hace llama.cpp por defecto. `forge doctor` avisa del enlace estrecho y de su
ancho de banda estimado, y recomienda una ranura x8/x16. La KV nunca se saca a
la RAM por motivo del PCIe.

## Alternativas consideradas

- **A — Reparto por capas y aviso (elegida).** 23.5 tok/s medidos con x1.
- **B — KV a RAM (SDD original).** 0.6 tok/s: incumple RNF-01 por 40 veces.

## Consecuencias

- RF-06b y HU-00b cambian su criterio: "si PCIe ≤x4, mantiene el reparto por
  capas y avisa".
- Desaparece del SDD el fallback "contexto 48K, ≥18 tok/s" del perfil B.
- Ya implementado: `hardware::PcieLink` y el aviso de `forge doctor`.
