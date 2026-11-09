# ADR-0012: Dataset de QLoRA para tool-calling — Glaive v2 + sintético

- **Estado**: Aceptada
- **Fecha**: 2026-11-09
- **Decisor (PO)**: Sebastia M.
- **Issue relacionada**: RF-03, RF-03b, HU-01, HU-02 (Sprints S4–S5), RNF-09
- **Modifica**: SDD §8 (ML)

## Contexto

RF-03 pide un pipeline QLoRA reproducible para tool-calling, pero el SDD no
fija los datos. Nuestro formato de tool-call es propio: un único objeto
`{"name": "<tool>", "arguments": {...}}`, con `name` en `[a-z][a-z0-9_]{0,63}`
(`grammars/tool_calls.gbnf`).

Candidatos públicos revisados en Hugging Face (2026-11-09):

| Dataset | Licencia | Acceso |
|---|---|---|
| `glaiveai/glaive-function-calling-v2` | Apache-2.0 | libre |
| `Salesforce/xlam-function-calling-60k` | CC-BY-4.0 | requiere aceptar condiciones |
| `NousResearch/hermes-function-calling-v1` | Apache-2.0 | libre |

## Decisión

Mezcla de dos fuentes, generada por `python -m forge_ml.dataset`:

1. **Pública:** Glaive function-calling v2 (Apache-2.0). Se toma el primer
   turno de usuario y la primera llamada a función de cada conversación, y se
   convierte a nuestro formato (los `arguments` de Glaive son una cadena JSON
   y pasan a ser un objeto). Se descartan nombres que la gramática no admite.
2. **Sintética:** prompts en español e inglés para las tools del proyecto
   (`read_file`, `edit_file`, `create_file`, `delete_file`, `search_web`,
   `run_shell`), generados por plantillas con semilla fija.

Las dos fuentes usan el mismo formato de system prompt con la lista de tools.
Los 30 prompts del test RF-08 quedan fuera del entrenamiento (un test lo
comprueba). Cada ejecución escribe un `manifest.json` con la semilla, el
SHA-256 de las entradas y de las salidas, y los recuentos por fuente (RNF-09).

## Alternativas consideradas

- **A — Solo público.** Más variedad, pero no enseña nuestras tools.
- **B — Solo sintético.** Enseña nuestro formato, pero con poca variedad.
- **C — Mezcla (elegida).**
- xLAM-60k se descarta por ahora: exige aceptar condiciones de acceso y la
  atribución de CC-BY-4.0. Puede añadirse con otra ADR.

## Consecuencias

- El JSON de Glaive (~271 MB) se descarga fuera del repo; no es una
  dependencia de código.
- Atribución: el README cita Glaive v2 y su licencia Apache-2.0.
- La proporción por defecto (ver `forge_ml.dataset`) puede ajustarse sin ADR;
  cambiar de fuente pública requiere una ADR nueva.
