<!-- Plantilla de PR — AGENTS.md 11.8. PR sin issue no se mergea. PR <=400 líneas de diff (sin tests). -->

## Resumen

<!-- Qué cambia y por qué, en 1-3 frases. -->

## RF / HU cubierto

- Issue: #
- RF / HU:

## Perfiles verificados

- [ ] Perfil A (RTX 3060 6 GB)
- [ ] Perfil B (2x RTX 3060 12 GB)

## Benchmarks (si toca hot path: router / inferencia / KV)

<!-- Baseline vs actual. Regresión >5% tok/s o >10% p95 bloquea. -->

## Decisión del PO (si aplica)

<!-- Enlace a la respuesta del PO y a la ADR resultante (docs/adr/NNNN-*.md). -->

## Checklist (AGENTS.md 11.9)

- [ ] Test escrito y verde (TDD)
- [ ] `make check` pasa (fmt, lint, test, coverage)
- [ ] `make bench` sin regresión >5% si toca hot path
- [ ] CI matrix A y B verde
- [ ] Docs actualizadas si cambia API o comportamiento
- [ ] CHANGELOG actualizado si es feature o fix visible
- [ ] Sin TODOs sin issue, sin secretos ni PII
- [ ] SOLID respetado; funciones <=40 líneas, archivos <=400 líneas, complejidad <=10
- [ ] Si hubo decisión crítica: respuesta del PO como ADR y SDD actualizado
