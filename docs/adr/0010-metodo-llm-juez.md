# ADR-0010: Método del LLM-juez — puntuación absoluta y 25 tareas por perfil

- **Estado**: Aceptada
- **Fecha**: 2026-11-09
- **Decisor (PO)**: Sebastia M.
- **Issue relacionada**: AGENTS.md 11.5, ADR-0006, ADR-0009
- **Modifica**: ADR-0009 (método), AGENTS.md 11.5 (50 → 25 prompts)

## Contexto

La primera ejecución del juez (ADR-0009) comparaba cada respuesta candidata con
la de referencia (f16/f16). Dio media 3.32 en el perfil A, pero midió parecido
y no calidad: con temperatura 0 ninguna respuesta coincidió con la referencia,
y las notas bajas revisadas eran respuestas válidas pero distintas. Además, la
cuota gratuita de opencode se agotó tras unas 45 llamadas, y 50 tareas por
perfil no caben en un día.

## Decisión

1. **Puntuación absoluta.** El juez ve el código, la pregunta y la respuesta
   candidata, y la puntúa de 1 a 5 por sí sola. Ya no hay respuesta de
   referencia. `generate` guarda el código con cada respuesta y
   `score --answers` puntúa un único archivo.
2. **25 tareas por perfil** (5 archivos x 5 preguntas). El umbral sigue siendo
   una media ≥4. Con menos de 25 respuestas el informe no aprueba
   (`MIN_TASKS = 25` en `forge_ml.judge`).

## Alternativas consideradas

- Método: **A (elegida)** absoluto con código; B comparación ciega por
  pares; C mantener la comparación con la referencia y bajar el umbral.
- Cuota: A repartir en varios días; B suscripción OpenCode Go; **C (elegida)**
  reducir a 25 tareas.

## Consecuencias

- Cada llamada envía más tokens (el código), pero hay la mitad de llamadas.
- La nota ya no aísla el efecto de la caché KV: también refleja la capacidad
  del modelo base. Una nota baja obliga a puntuar f16/f16 con las mismas
  tareas para separar las dos causas.
- AGENTS.md 11.5 pasa a "LLM-juez ≥4/5 en 25 prompts".
