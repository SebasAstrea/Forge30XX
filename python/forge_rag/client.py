"""Cliente HTTP del RAG portable (port-rag-potty) servido como contenedor externo.

Ver docs/adr/0001-integracion-rag-portable.md (Opción C). El motor del RAG no
vive en este repo; aquí solo se habla con él por HTTP. Contratos tomados del
upstream `rag/server.py` (endpoints /health, /methodology, /query, /route).
"""

from __future__ import annotations

import json
import logging
import urllib.error
import urllib.request
from dataclasses import dataclass

logger = logging.getLogger("forge_rag.client")

_JSON = "application/json"


def _as_list(value: object) -> list[object]:
    return value if isinstance(value, list) else []


def _as_float(value: object, default: float) -> float:
    return float(value) if isinstance(value, int | float | str) else default


def _as_int(value: object, default: int) -> int:
    return int(value) if isinstance(value, int | float | str) else default


class RagError(Exception):
    """Fallo al comunicarse con el servicio RAG."""


@dataclass(frozen=True)
class RouteDecision:
    """Decisión del router: RAG, lectura directa o híbrido."""

    action: str  # "rag" | "direct" | "hybrid"
    conf: float
    save: int
    read: list[str]


@dataclass(frozen=True)
class QueryHit:
    """Un chunk recuperado por búsqueda semántica."""

    path: str
    score: float
    text: str


class RagClient:
    """Cliente mínimo y síncrono del servicio RAG.

    Pensado para `tokio::task::spawn_blocking` desde Rust vía PyO3 o para uso
    directo en scripts Python. Sin estado más allá de la URL base.
    """

    def __init__(self, base_url: str, timeout_s: float = 5.0) -> None:
        self._base = base_url.rstrip("/")
        self._timeout = timeout_s

    def health(self) -> dict[str, object]:
        """Devuelve el estado del servicio (Ollama + colección + grafo)."""
        return self._get("/health")

    def methodology(self) -> list[dict[str, object]]:
        """Devuelve los chunks 'methodology' (el alma del proyecto)."""
        payload = self._get("/methodology")
        chunks = payload.get("chunks", [])
        return chunks if isinstance(chunks, list) else []

    def route(
        self,
        query: str,
        *,
        k: int = 5,
        candidate_files: list[str] | None = None,
        methodology_loaded: bool = False,
        loaded_files: list[str] | None = None,
    ) -> RouteDecision:
        """Pregunta al router si conviene RAG, lectura directa o híbrido."""
        body = {
            "q": query,
            "k": k,
            "candidate_files": candidate_files or [],
            "methodology_loaded": methodology_loaded,
            "loaded_files": loaded_files or [],
            "compact": True,
        }
        data = self._post("/route", body)
        reads = _as_list(data.get("read"))
        paths = [str(r["path"]) for r in reads if isinstance(r, dict) and "path" in r]
        return RouteDecision(
            action=str(data.get("action", "direct")),
            conf=_as_float(data.get("conf"), 0.0),
            save=_as_int(data.get("save"), 0),
            read=paths,
        )

    def query(
        self,
        query: str,
        *,
        k: int = 5,
        category: str | None = None,
        file_glob: str | None = None,
        include_methodology: bool = True,
    ) -> list[QueryHit]:
        """Búsqueda semántica. Devuelve los chunks rankeados por score."""
        body: dict[str, object] = {
            "q": query,
            "k": k,
            "include_methodology": include_methodology,
        }
        if category is not None:
            body["category"] = category
        if file_glob is not None:
            body["file_glob"] = file_glob
        data = self._post("/query", body)
        return [
            QueryHit(
                path=str(r.get("path", "")),
                score=_as_float(r.get("score"), 0.0),
                text=str(r.get("text", "")),
            )
            for r in _as_list(data.get("results"))
            if isinstance(r, dict)
        ]

    def _get(self, path: str) -> dict[str, object]:
        return self._request("GET", path, None)

    def _post(self, path: str, body: dict[str, object]) -> dict[str, object]:
        return self._request("POST", path, body)

    def _request(self, method: str, path: str, body: dict[str, object] | None) -> dict[str, object]:
        url = f"{self._base}{path}"
        data = json.dumps(body).encode("utf-8") if body is not None else None
        req = urllib.request.Request(url, data=data, method=method)
        req.add_header("Accept", _JSON)
        if data is not None:
            req.add_header("Content-Type", _JSON)
        try:
            with urllib.request.urlopen(req, timeout=self._timeout) as resp:
                raw = resp.read().decode("utf-8")
        except (urllib.error.URLError, TimeoutError) as exc:
            logger.error("RAG %s %s falló: %s", method, path, exc)
            raise RagError(f"{method} {path}: {exc}") from exc
        parsed = json.loads(raw)
        if not isinstance(parsed, dict):
            raise RagError(f"{path}: respuesta no es objeto JSON")
        return parsed
