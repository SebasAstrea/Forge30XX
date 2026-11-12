//! Tests de integración del servidor de la UI (S9, RF-18/RF-20, ADR-0015).
//!
//! Mockean el llama-server con el servidor HTTP falso compartido; nunca la GPU
//! ni el modelo (AGENTS.md 11.11). Verifican el contrato de `docs/openapi.yaml`:
//! salud, streaming SSE de tokens y decisión sobre tool-calls.

// Crate de test: todo su código es test, donde AGENTS.md 11.1 permite expect().
#![allow(clippy::expect_used)]

use std::sync::Arc;
use std::time::Duration;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use forge::config::Config;
use forge::inference::InferenceClient;
use forge::server::{router, AppState, ToolDecision};
use http_body_util::BodyExt;
use tower::ServiceExt; // oneshot

#[path = "../src/test_support.rs"]
mod test_support;
use test_support::{serve, Reply};

fn state_for(base_url: &str) -> Arc<AppState> {
    let client = InferenceClient::new(base_url, Duration::from_secs(2)).expect("client");
    // Los tests corren con cwd = orchestrator/; el TOML está un nivel arriba.
    let cfg = Config::load("../forge.config.toml").expect("config del repo");
    Arc::new(AppState::new(client, &cfg))
}

async fn body_string(resp: axum::response::Response) -> String {
    let bytes = resp.into_body().collect().await.expect("body").to_bytes();
    String::from_utf8(bytes.to_vec()).expect("utf8")
}

#[tokio::test]
async fn health_reflects_llama_server() {
    let url = serve(vec![("GET /v1/models", Reply::json(r#"{"data":[]}"#))]).await;
    let app = router(state_for(&url), std::path::PathBuf::from("."));
    let resp = app
        .oneshot(
            Request::builder()
                .uri("/api/health")
                .body(Body::empty())
                .expect("req"),
        )
        .await
        .expect("resp");
    assert_eq!(resp.status(), StatusCode::OK);
    assert!(body_string(resp).await.contains("\"llama_server\":true"));
}

#[tokio::test]
async fn chat_streams_tokens_and_done() {
    // El llama-server falso emite dos deltas y [DONE] en trozos pequeños.
    let sse = "data: {\"choices\":[{\"delta\":{\"content\":\"fn \"}}]}\n\n\
               data: {\"choices\":[{\"delta\":{\"content\":\"a\"}}]}\n\n\
               data: [DONE]\n\n";
    let url = serve(vec![("POST /v1/chat/completions", Reply::sse(sse, 16))]).await;
    let app = router(state_for(&url), std::path::PathBuf::from("."));
    let resp = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/chat")
                .header("content-type", "application/json")
                .body(Body::from(
                    r#"{"messages":[{"role":"user","content":"hola"}]}"#,
                ))
                .expect("req"),
        )
        .await
        .expect("resp");
    assert_eq!(resp.status(), StatusCode::OK);
    let out = body_string(resp).await;
    // Eventos SSE reemitidos hacia la UI (ADR-0015).
    assert!(out.contains("event: token"), "faltan tokens: {out}");
    assert!(out.contains("\"text\":\"fn \""), "token 1: {out}");
    assert!(out.contains("\"text\":\"a\""), "token 2: {out}");
    assert!(out.contains("event: done"), "falta done: {out}");
}

#[tokio::test]
async fn tool_decision_resolves_pending() {
    let url = serve(vec![("GET /v1/models", Reply::json("{}"))]).await;
    let state = state_for(&url);
    // Simula un tool_request pendiente y su aprobación vía el endpoint.
    let rx = state.register_pending("t1".into());
    let app = router(state, std::path::PathBuf::from("."));
    let resp = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/tools/t1/decision")
                .header("content-type", "application/json")
                .body(Body::from(r#"{"decision":"approve"}"#))
                .expect("req"),
        )
        .await
        .expect("resp");
    assert_eq!(resp.status(), StatusCode::OK);
    assert!(body_string(resp).await.contains("\"status\":\"approved\""));
    assert_eq!(rx.await.expect("decision"), ToolDecision::Approve);
}

#[tokio::test]
async fn tool_decision_unknown_id_is_404() {
    let url = serve(vec![("GET /v1/models", Reply::json("{}"))]).await;
    let app = router(state_for(&url), std::path::PathBuf::from("."));
    let resp = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/tools/missing/decision")
                .header("content-type", "application/json")
                .body(Body::from(r#"{"decision":"reject"}"#))
                .expect("req"),
        )
        .await
        .expect("resp");
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}
