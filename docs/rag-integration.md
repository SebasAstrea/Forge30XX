# Integración del RAG portable (RF-14, RF-15)

Forge30XX usa [`port-rag-potty`](https://github.com/SebasAstrea/port-rag-potty)
como motor de RAG. Según **ADR-0001** (Opción C) el motor corre como **servicio
externo** y Forge solo habla con él por HTTP. Su código no vive en este repo.

## Arquitectura

```
orchestrator (Rust)  --HTTP-->  forge-rag (:8765)  --HTTP-->  ollama (:11434)
  src/rag.rs: RouterPolicy                port-rag-potty         bge-m3
python/forge_rag/client.py: RagClient     FastAPI + ChromaDB
```

- **Umbrales RF-15** (router RAG vs lectura directa), por perfil de hardware:
  - Perfil A: `rag_threshold_tokens = 1500`
  - Perfil B: `rag_threshold_tokens = 3000`
- Por debajo del umbral el router elige lectura directa; por encima, RAG o
  híbrido (ver `RouterPolicy` en `orchestrator/src/rag.rs`).

## Puesta en marcha (Docker)

El servicio `forge-rag` del `docker-compose.yml` construye su imagen desde el
repo clonado del RAG. Clónalo junto a Forge y apunta `RAG_REPO_PATH`:

```bash
# 1) Clona el RAG junto al repo de Forge (una sola vez)
git clone https://github.com/SebasAstrea/port-rag-potty.git ../port-rag-potty

# 2) Levanta Forge + RAG + Ollama (perfil A o B)
RAG_REPO_PATH=../port-rag-potty docker compose --profile a up -d

# 3) Indexa el workspace e verifica que no hay drift
curl -X POST http://localhost:8765/reindex
curl -s   http://localhost:8765/stats     # "drift": 0
```

El servicio monta el workspace de Forge en `/app` y persiste ChromaDB en el
volumen `rag-data`. Ollama descarga `bge-m3` en el volumen `ollama-data`.

## Uso desde Python

```python
from forge_rag import RagClient

rag = RagClient("http://localhost:8765")

# ¿Conviene RAG o leer el archivo directamente?
decision = rag.route("¿cómo funciona el router de contexto?", k=5)
if decision.action == "direct":
    # leer decision.read[...] directamente
    ...
else:
    hits = rag.query("¿cómo funciona el router de contexto?", k=5)
    for hit in hits:
        print(hit.path, hit.score)  # noqa: solo ejemplo
```

## Endpoints consumidos

| Método | Endpoint        | Uso en Forge                              |
| ------ | --------------- | ----------------------------------------- |
| GET    | `/health`       | Readiness del servicio (healthcheck)      |
| GET    | `/methodology`  | Inyectar el "alma" del proyecto al modelo |
| POST   | `/route`        | Decidir RAG vs lectura directa (RF-15)    |
| POST   | `/query`        | Búsqueda semántica (RF-14)                |
| POST   | `/reindex`      | Reindexar tras cambios en el workspace    |
| GET    | `/stats`        | Verificar `drift == 0`                    |

## Notas

- El RAG ahorra contexto solo cuando el destino es grande (cruce ~1,7-2k tok);
  por eso el router decide antes de leer. Detalle en el README del upstream.
- El transporte HTTP del lado Rust (más allá de `RouterPolicy`) se implementa en
  Sprint S7 con el cliente HTTP que apruebe el PO (no se añade dependencia aún).
