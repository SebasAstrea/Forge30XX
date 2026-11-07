"""Forge30XX RAG: cliente del servicio port-rag-potty (externo, vía HTTP).

El motor del RAG corre como contenedor independiente (ADR-0001). Este paquete
expone únicamente el cliente HTTP y sus modelos de datos.
"""

from forge_rag.client import QueryHit, RagClient, RagError, RouteDecision

__all__ = [
    "QueryHit",
    "RagClient",
    "RagError",
    "RouteDecision",
    "__version__",
]

__version__ = "0.1.0"
