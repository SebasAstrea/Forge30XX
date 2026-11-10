# ADR-0014: QLoRA solo en el perfil A

- **Estado**: Aceptada
- **Fecha**: 2026-11-09
- **Decisor (PO)**: Sebastia M.
- **Issue relacionada**: RF-03, RF-03b, RF-04, HU-01, HU-02 (Sprints S4–S5)
- **Modifica**: SDD §3.2 (RF-03, RF-03b, RF-04), §5 (HU-01, HU-02), §6 (S4, S5)

## Contexto

Con ADR-0013, el perfil B usa Qwen3.6-35B-A3B, que da 100% en RF-08 sin
adaptador (umbral de B: 95%). Entrenar con QLoRA un MoE de 35B exige ~18 GB
solo para los pesos en 4 bits, muy justo en 2x12 GB. El perfil A pasa a
Qwen3.5-4B, para el que no hay adaptador todavía.

## Decisión

- RF-03, RF-03b y RF-04 aplican solo al perfil A, con Qwen3.5-4B.
- El perfil B usa Qwen3.6-35B-A3B sin LoRA.
- HU-02 (entrenar el 7B en dos GPUs) se retira; S5 queda en el merge y el
  export GGUF del adaptador de A.
- `[training.B]` de `forge.config.toml` y los adaptadores 3B/7B entrenados el
  2026-11-09 quedan como histórico; no se usan en ningún perfil.

## Alternativas consideradas

- **A — QLoRA solo en A (elegida).**
- **B — QLoRA también del 35B-A3B.** Sin necesidad medida (RF-08 ya al 100%)
  y con riesgo de OOM en 2x12 GB.

## Consecuencias

- Falta reentrenar con Qwen3.5-4B: cambiar `[training.A]` a su versión
  bnb-4bit con revisión fijada, medir RF-08 y repetir RF-04 sobre pesos de
  16 bits (cuantizar una sola vez; ver `docs/training.md`).
- El pipeline (`forge_ml.dataset`, `forge_ml.train`) no cambia.
