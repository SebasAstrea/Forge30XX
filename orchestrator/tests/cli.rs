//! Tests de extremo a extremo del binario `forge` (RF-21).
//!
//! El llama-server se sustituye por un servidor HTTP falso: estos tests
//! verifican el cableado de la CLI, no el modelo (ver `rf08_tool_calls.rs`
//! para la medición contra un modelo real).

// Crate de test: todo su código es test, donde AGENTS.md 11.1 permite expect().
#![allow(clippy::expect_used)]

#[path = "../src/test_support.rs"]
mod test_support;

use std::process::{Output, Stdio};

use test_support::{closed_port_url, serve, Reply};
use tokio::io::AsyncWriteExt;
use tokio::process::Command;

const CONFIG: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../forge.config.toml");

async fn forge(args: &[&str], base_url: &str, stdin: &str) -> Output {
    let mut child = Command::new(env!("CARGO_BIN_EXE_forge"))
        .arg("--config")
        .arg(CONFIG)
        .args(args)
        .env("FORGE_INFERENCE__BASE_URL", base_url)
        .env("FORGE_INFERENCE__TIMEOUT_MS", "5000")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn forge");
    let mut pipe = child.stdin.take().expect("stdin");
    pipe.write_all(stdin.as_bytes()).await.expect("write stdin");
    drop(pipe);
    child.wait_with_output().await.expect("wait forge")
}

fn stdout(out: &Output) -> String {
    String::from_utf8_lossy(&out.stdout).to_string()
}

#[tokio::test]
async fn version_prints_crate_version() {
    let out = forge(&["version"], "http://127.0.0.1:1", "").await;
    assert!(out.status.success());
    assert!(stdout(&out).contains(env!("CARGO_PKG_VERSION")));
}

#[tokio::test]
async fn profile_echoes_requested_profile() {
    let out = forge(&["profile", "B"], "http://127.0.0.1:1", "").await;
    assert!(out.status.success());
    assert!(stdout(&out).contains("perfil solicitado: B"));
}

#[tokio::test]
async fn doctor_finishes_with_or_without_gpu() {
    let out = forge(&["doctor"], "http://127.0.0.1:1", "").await;
    assert!(out.status.success());
    assert!(stdout(&out).contains("Diagnóstico completado"));
}

#[tokio::test]
async fn run_is_declared_and_exits_cleanly() {
    let out = forge(&["run"], "http://127.0.0.1:1", "").await;
    assert!(out.status.success());
}

#[tokio::test]
async fn chat_reports_unreachable_server_without_failing() {
    let url = closed_port_url().await;
    let out = forge(&["chat"], &url, "").await;
    assert!(out.status.success());
    assert!(stdout(&out).contains("No se pudo contactar"));
}

#[tokio::test]
async fn chat_reports_unhealthy_server_without_failing() {
    let url = serve(vec![("GET /v1/models", Reply::status(503))]).await;
    let out = forge(&["chat"], &url, "").await;
    assert!(out.status.success());
    assert!(stdout(&out).contains("No se pudo contactar"));
}

#[tokio::test]
async fn chat_streams_reply_and_survives_failed_turn() {
    let sse = concat!(
        "data: {\"choices\":[{\"delta\":{\"content\":\"hola \"}}]}\n\n",
        "data: {\"choices\":[{\"delta\":{\"content\":\"mundo\"}}]}\n\n",
        "data: [DONE]\n\n",
    );
    let url = serve(vec![
        ("GET /v1/models", Reply::json("{\"data\":[]}")),
        ("POST /v1/chat/completions", Reply::sse(sse, 9)),
    ])
    .await;
    let out = forge(&["chat"], &url, "\n  \nhola\nquit\n").await;
    assert!(out.status.success());
    assert!(stdout(&out).contains("hola mundo"));

    let broken = serve(vec![("GET /v1/models", Reply::json("{}"))]).await;
    let out = forge(&["chat"], &broken, "hola\n").await;
    assert!(out.status.success());
    assert!(stdout(&out).contains("[error]"));
}
