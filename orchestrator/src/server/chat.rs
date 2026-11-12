//! Handlers de la API de la UI (ADR-0015): salud, chat con streaming SSE y
//! decisión sobre tool-calls destructivas (RF-20).

use std::convert::Infallible;
use std::sync::Arc;

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::sse::{Event, Sse};
use axum::response::IntoResponse;
use axum::Json;
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;

use super::state::{AppState, ToolDecision};
use crate::inference::Message;

/// Cuerpo de `POST /api/chat`.
#[derive(Debug, Deserialize)]
pub struct ChatRequest {
    /// Historial de mensajes del turno.
    pub messages: Vec<Message>,
}

/// Cuerpo de `POST /api/tools/{id}/decision`.
#[derive(Debug, Deserialize)]
pub struct DecisionRequest {
    /// Decisión del usuario.
    pub decision: ToolDecision,
}

/// Respuesta de salud (`GET /api/health`).
#[derive(Debug, Serialize)]
pub struct Health {
    /// `true` si el llama-server responde en `/v1/models`.
    pub llama_server: bool,
}

/// Resultado de una decisión (`POST /api/tools/{id}/decision`).
#[derive(Debug, Serialize)]
pub struct ToolResult {
    /// Id del tool_request resuelto.
    pub id: String,
    /// Estado resultante.
    pub status: &'static str,
}

/// `GET /api/health`: estado del backend de inferencia.
pub async fn health(State(state): State<Arc<AppState>>) -> Json<Health> {
    let ok = state.client.health().await.unwrap_or(false);
    Json(Health { llama_server: ok })
}

/// `POST /api/tools/{id}/decision`: resuelve un tool_request pendiente (RF-20).
pub async fn tool_decision(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(body): Json<DecisionRequest>,
) -> impl IntoResponse {
    if state.resolve_pending(&id, body.decision) {
        let status = match body.decision {
            ToolDecision::Approve => "approved",
            ToolDecision::Reject => "rejected",
        };
        (StatusCode::OK, Json(ToolResult { id, status })).into_response()
    } else {
        StatusCode::NOT_FOUND.into_response()
    }
}

/// `POST /api/chat`: turno de chat con streaming SSE (RF-18).
pub async fn chat(
    State(state): State<Arc<AppState>>,
    Json(body): Json<ChatRequest>,
) -> Sse<ReceiverStream<Result<Event, Infallible>>> {
    let (tx, rx) = mpsc::channel::<Result<Event, Infallible>>(32);
    tokio::spawn(run_turn(state, body.messages, tx));
    Sse::new(ReceiverStream::new(rx))
}

/// Ejecuta el turno: pide el stream al llama-server y reemite eventos SSE.
async fn run_turn(
    state: Arc<AppState>,
    messages: Vec<Message>,
    tx: mpsc::Sender<Result<Event, Infallible>>,
) {
    // Canal síncrono→async: el callback de `chat_stream` empuja tokens aquí.
    let (token_tx, mut token_rx) = mpsc::unbounded_channel::<String>();
    let client = &state.client;
    let params = &state.params;
    let forward = async {
        while let Some(text) = token_rx.recv().await {
            let _ = tx.send(Ok(sse_json("token", &Token { text }))).await;
        }
    };
    // El closure `move` es el único dueño de `token_tx`: al terminar la
    // generación se destruye y `forward` ve el cierre del canal (fin del bucle).
    let generate = async {
        let out = client
            .chat_stream(&messages, params, move |t| {
                let _ = token_tx.send(t.to_string());
            })
            .await;
        out.map(|_| ()).map_err(|e| e.to_string())
    };
    let (result, ()) = tokio::join!(generate, forward);
    match result {
        Ok(()) => {
            let _ = tx.send(Ok(sse_json("done", &Empty {}))).await;
        }
        Err(msg) => {
            let _ = tx
                .send(Ok(sse_json("error", &ErrorMsg { message: msg })))
                .await;
        }
    }
}

fn sse_json<T: Serialize>(event: &str, data: &T) -> Event {
    // serde_json::to_string de estructuras simples no falla; ante un improbable
    // error se emite un objeto vacío para no cortar el stream.
    let json = serde_json::to_string(data).unwrap_or_else(|_| "{}".to_string());
    Event::default().event(event).data(json)
}

#[derive(Serialize)]
struct Token {
    text: String,
}

#[derive(Serialize)]
struct ErrorMsg {
    message: String,
}

#[derive(Serialize)]
struct Empty {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sse_json_sets_event_name() {
        let ev = sse_json("token", &Token { text: "hi".into() });
        let rendered = format!("{ev:?}");
        assert!(rendered.contains("token"));
    }

    #[test]
    fn decision_request_deserializes_lowercase() {
        let d: DecisionRequest = serde_json::from_str(r#"{"decision":"approve"}"#).expect("parse");
        assert_eq!(d.decision, ToolDecision::Approve);
    }

    #[test]
    fn chat_request_deserializes_messages() {
        let r: ChatRequest =
            serde_json::from_str(r#"{"messages":[{"role":"user","content":"hola"}]}"#)
                .expect("parse");
        assert_eq!(r.messages.len(), 1);
    }
}
