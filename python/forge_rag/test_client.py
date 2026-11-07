"""Tests del cliente RAG contra un servidor HTTP falso en localhost (sin red)."""

from __future__ import annotations

import json
import threading
from collections.abc import Iterator
from http.server import BaseHTTPRequestHandler, HTTPServer

import pytest

from forge_rag import RagClient, RagError

_ROUTE_RESPONSE = {
    "action": "direct",
    "conf": 0.86,
    "save": 1180,
    "read": [{"path": "src/x.py", "tok": 420}],
}
_QUERY_RESPONSE = {
    "results": [{"path": "src/x.py", "score": 0.91, "text": "fn x() {}"}],
    "methodology_chunks": [],
}


class _Handler(BaseHTTPRequestHandler):
    def log_message(self, *_: object) -> None:  # silencia el log del server
        pass

    def _send(self, payload: dict[str, object]) -> None:
        body = json.dumps(payload).encode("utf-8")
        self.send_response(200)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)

    def do_GET(self) -> None:  # noqa: N802 (nombre impuesto por la stdlib)
        if self.path == "/health":
            self._send({"ok": True})
        else:
            self.send_response(404)
            self.end_headers()

    def do_POST(self) -> None:  # noqa: N802
        length = int(self.headers.get("Content-Length", "0"))
        self.rfile.read(length)
        if self.path == "/route":
            self._send(_ROUTE_RESPONSE)
        elif self.path == "/query":
            self._send(_QUERY_RESPONSE)
        else:
            self.send_response(404)
            self.end_headers()


@pytest.fixture()
def rag_url() -> Iterator[str]:
    server = HTTPServer(("127.0.0.1", 0), _Handler)
    thread = threading.Thread(target=server.serve_forever, daemon=True)
    thread.start()
    host, port = server.server_address
    try:
        yield f"http://{host}:{port}"
    finally:
        server.shutdown()
        server.server_close()


def test_health_ok(rag_url: str) -> None:
    client = RagClient(rag_url)
    assert client.health()["ok"] is True


def test_route_parses_decision(rag_url: str) -> None:
    client = RagClient(rag_url)
    decision = client.route("¿cómo funciona X?", k=5)
    assert decision.action == "direct"
    assert decision.conf == pytest.approx(0.86)
    assert decision.save == 1180
    assert decision.read == ["src/x.py"]


def test_query_parses_hits(rag_url: str) -> None:
    client = RagClient(rag_url)
    hits = client.query("x", k=3)
    assert len(hits) == 1
    assert hits[0].path == "src/x.py"
    assert hits[0].score == pytest.approx(0.91)


def test_connection_error_raises_rag_error() -> None:
    client = RagClient("http://127.0.0.1:1", timeout_s=0.2)
    with pytest.raises(RagError):
        client.health()
