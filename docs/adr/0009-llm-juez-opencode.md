# ADR-0009: LLM-juez externo vía el CLI de opencode

- **Estado**: Aceptada
- **Fecha**: 2026-11-09
- **Decisor (PO)**: Sebastia M.
- **Issue relacionada**: AGENTS.md 11.5 (calidad de cuantización KV), RF-16,
  SDD §10 (regresión de calidad por release)
- **Modifica**: SDD §10 (juez Qwen2.5-72B)

## Contexto

AGENTS.md 11.5 exige un LLM-juez (≥4/5 en 50 prompts) para cambios de
cuantización KV, y el SDD §10 fijaba Qwen2.5-72B como juez. Ese modelo no cabe
en 2x RTX 3060 12 GB, así que la condición de calidad de ADR-0006 no podía
comprobarse.

## Decisión

El juez es un modelo gratuito de opencode, invocado con su CLI
(`opencode run --agent plan --format json -m <modelo>`). El modelo por defecto
es `opencode/deepseek-v4.1-flash-free`. La herramienta es
`python -m forge_ml.judge` (`generate` + `score`), solo con la biblioteca
estándar de Python: opencode es un binario externo, como `nvidia-smi`, no una
dependencia del proyecto.

- El juez corre en un directorio temporal vacío y con el agente `plan`, para
  que no pueda leer ni modificar el repo.
- `score --cache` guarda cada veredicto al obtenerlo; una ejecución cortada se
  reanuda sin repetir llamadas.
- Es una herramienta de desarrollo y CI: no forma parte del producto, así que
  RNF-06 (cero salidas de red) no le aplica. Solo envía preguntas y respuestas
  generadas sobre código público.

## Alternativas consideradas

- **A — opencode con modelo gratuito (elegida).** Sin coste ni GPU extra.
- **B — Juez local más pequeño (p. ej. 14B Q4).** Cabe en 2x12 GB, pero un
  juez de tamaño parecido al evaluado es poco fiable.
- **C — API de pago con Qwen2.5-72B.** Fiel al SDD, con coste por uso.

## Consecuencias

Primera ejecución (2026-11-09), perfil A, 3B, turbo2 frente a f16/f16:

- **Cuota.** La cuota gratuita se agotó tras unas 45 llamadas (HTTP 429
  `FreeUsageLimitError`, reintento indicado en ~9 h). La cuota es compartida
  entre todos los modelos gratuitos. Una evaluación completa (50 tareas x 2
  perfiles) no cabe en un día.
- **Calibración.** El modo actual compara la respuesta candidata con la de
  referencia. Con 44 de 50 veredictos la media fue 3.32. Al revisar los
  "2/5", la mayoría son respuestas igual de válidas pero distintas (p. ej. un
  bug distinto pero plausible en la pregunta "señala el bug más importante").
  Con temperatura 0 ninguna de las 50 respuestas fue idéntica a la de
  referencia, así que el modo mide parecido, no calidad.
- Estas dos cuestiones (método de puntuación y cuota) quedan para decisión del
  PO antes de dar por cumplido AGENTS.md 11.5 para ADR-0006 y para el perfil A.
