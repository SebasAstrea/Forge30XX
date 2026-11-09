//! Servidor HTTP/1.1 falso para tests de clientes (RAG, inferencia, CLI).
//!
//! Solo sustituye servicios HTTP externos; nunca la GPU ni el modelo
//! (AGENTS.md 11.11). Se comparte con los tests de integración mediante
//! `#[path = "../src/test_support.rs"]`, de ahí el `dead_code` permitido:
//! cada crate de test usa solo parte de la API.
#![allow(dead_code)]

use std::time::Duration;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

/// Respuesta enlatada para una ruta.
#[derive(Debug, Clone, Copy)]
pub struct Reply {
    /// Código de estado HTTP.
    pub status: u16,
    /// Cabecera `Content-Type`.
    pub content_type: &'static str,
    /// Cuerpo completo.
    pub body: &'static str,
    /// Tamaño de los trozos en que se envía el cuerpo (simula streaming).
    pub chunk: usize,
}

impl Reply {
    /// Respuesta JSON 200 enviada de una vez.
    pub fn json(body: &'static str) -> Self {
        Self {
            status: 200,
            content_type: "application/json",
            body,
            chunk: usize::MAX,
        }
    }

    /// Respuesta SSE 200 enviada en trozos de `chunk` bytes.
    pub fn sse(body: &'static str, chunk: usize) -> Self {
        Self {
            status: 200,
            content_type: "text/event-stream",
            body,
            chunk,
        }
    }

    /// Respuesta vacía con el estado indicado.
    pub fn status(status: u16) -> Self {
        Self {
            status,
            content_type: "text/plain",
            body: "",
            chunk: usize::MAX,
        }
    }
}

/// Lanza un servidor en `127.0.0.1:<libre>` y devuelve su URL base.
///
/// Cada ruta es `"MÉTODO /ruta"` (p. ej. `"GET /health"`); las peticiones sin
/// ruta coincidente reciben 404. Atiende conexiones hasta que acaba el test.
pub async fn serve(routes: Vec<(&'static str, Reply)>) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
    let addr = listener.local_addr().expect("addr");
    tokio::spawn(async move {
        while let Ok((sock, _)) = listener.accept().await {
            tokio::spawn(handle(sock, routes.clone()));
        }
    });
    format!("http://{addr}")
}

/// URL de un puerto local sin servidor escuchando (conexión rechazada).
pub async fn closed_port_url() -> String {
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
    let addr = listener.local_addr().expect("addr");
    drop(listener);
    format!("http://{addr}")
}

async fn handle(mut sock: TcpStream, routes: Vec<(&'static str, Reply)>) {
    let request_line = read_request(&mut sock).await;
    let reply = routes
        .iter()
        .find(|(route, _)| request_line.starts_with(&format!("{route} ")))
        .map_or(Reply::status(404), |(_, r)| *r);
    let head = format!(
        "HTTP/1.1 {} X\r\nContent-Type: {}\r\nConnection: close\r\n\r\n",
        reply.status, reply.content_type
    );
    sock.write_all(head.as_bytes()).await.expect("head");
    for part in reply.body.as_bytes().chunks(reply.chunk.max(1)) {
        sock.write_all(part).await.expect("part");
        sock.flush().await.expect("flush");
        tokio::time::sleep(Duration::from_millis(2)).await;
    }
}

/// Lee cabeceras y cuerpo completos (por `Content-Length`) y devuelve la
/// línea de petición. Consumir el cuerpo evita un RST al cerrar el socket.
async fn read_request(sock: &mut TcpStream) -> String {
    let mut buf = Vec::new();
    let mut tmp = [0u8; 4096];
    let header_end = loop {
        let n = sock.read(&mut tmp).await.expect("read");
        if n == 0 {
            return String::new();
        }
        buf.extend_from_slice(&tmp[..n]);
        if let Some(pos) = find(&buf, b"\r\n\r\n") {
            break pos + 4;
        }
    };
    let head = String::from_utf8_lossy(&buf[..header_end]).to_string();
    let body_len = content_length(&head);
    while buf.len() < header_end + body_len {
        let n = sock.read(&mut tmp).await.expect("read body");
        if n == 0 {
            break;
        }
        buf.extend_from_slice(&tmp[..n]);
    }
    head.lines().next().unwrap_or_default().to_string()
}

fn find(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack.windows(needle.len()).position(|w| w == needle)
}

fn content_length(head: &str) -> usize {
    head.lines()
        .filter_map(|l| l.split_once(':'))
        .find(|(k, _)| k.eq_ignore_ascii_case("content-length"))
        .and_then(|(_, v)| v.trim().parse().ok())
        .unwrap_or(0)
}
