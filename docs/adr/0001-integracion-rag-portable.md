# ADR-0001: Integración del RAG portable (port-rag-potty) como servicio externo

- **Estado**: Aceptada
- **Fecha**: 2026-11-07
- **Decisor (PO)**: Sebastia M.
- **Issue relacionada**: RF-14, RF-15 (SDD §3.4, §3.5)

## Contexto

Forge30XX necesita RAG para reducir el contexto efectivo por consulta (RF-14) y
un router que decida RAG vs lectura directa con umbrales por perfil (RF-15). El
SDD (§7, §8) ya aprueba usar `port-rag-potty` sobre ChromaDB + Ollama con
embeddings bge-m3, y fija comunicación **HTTP** para el RAG (ADR-05).

`port-rag-potty` (https://github.com/SebasAstrea/port-rag-potty, commit
`803579d`) es un servicio FastAPI autónomo: `rag/server.py`, `router.py`,
`indexer.py`, `chunker.py`, `graph.py`, `embeddings.py`, `config.py`, con su
propio `Dockerfile` y `requirements.txt`. Expone `/methodology`, `/query`,
`/route`, `/reindex`, `/stats`, `/health`. Sirve en `:8765`. Es "licencia
privada de uso". Dos de sus módulos (`router.py` 54 KB, `chunker.py` 29 KB)
exceden el límite de 400 líneas de AGENTS.md.

## Decisión

Integrar el RAG como **servicio externo** consumido por HTTP (Opción C):

1. El motor del RAG NO se copia al repo Forge. Corre como contenedor propio
   (su `Dockerfile`) añadido a `docker-compose.yml` como servicio `forge-rag`.
2. En `python/forge_rag/` vive solo un **cliente HTTP ligero** (`client.py`)
   100 % conforme a AGENTS.md (type hints, logging nombrado, ≤400 líneas).
3. El orquestador Rust consume el RAG vía un módulo `rag` (cliente HTTP) que
   implementa la lógica de router por perfil (umbrales RF-15: A 1500, B 3000).
4. La puesta en marcha e indexación inicial usa los `setup.sh/ps1` del upstream,
   documentada en `docs/rag-integration.md`.

## Alternativas consideradas

- **A — Submódulo git.** Trazabilidad exacta al upstream y `git pull` de
  mejoras, pero el layout `rag/` no encaja con el paquete `forge_rag`, añade
  fricción de `--recursive` en clone/CI y requiere adaptador igualmente.
- **B — Vendorizar `rag/` dentro de `forge_rag` ("adaptado").** Encaja con la
  letra del SDD y da control total, pero crea un fork con drift respecto al
  upstream y mete al repo archivos que violan el límite de 400 líneas sin una
  ADR de excepción. Mayor esfuerzo y mantenimiento.

## Consecuencias

Positivas: separación limpia (coherente con ADR-05 HTTP); cero drift de código;
respeta la licencia privada sin copiar fuentes; el cliente es pequeño y
verificable; se actualiza el RAG cambiando el tag de imagen.

Negativas: depende de que la imagen del RAG esté disponible (build local desde
el repo clonado del RAG o un registry); el setup inicial (indexado hasta
`drift = 0`) se ejecuta con los scripts del upstream, fuera del binario `forge`.

Impacto: refuerza ADR-05 (HTTP para RAG). No altera RF/RNF ni umbrales: los
umbrales del router por perfil siguen los de RF-15. Afecta a Sprint S7 (RAG).
