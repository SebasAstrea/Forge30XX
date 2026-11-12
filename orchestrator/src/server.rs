//! Servidor HTTP local de la UI (S9, RF-18/RF-20, ADR-0015).
//!
//! `forge run --ui` levanta este servidor en `[ui].port`. Sirve los estáticos
//! de SvelteKit y expone la API bajo `/api`: chat con streaming SSE, decisión
//! sobre tool-calls destructivas (RF-20) y salud del llama-server. El contrato
//! está en `docs/openapi.yaml`.

use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;

use axum::routing::{get, post};
use axum::Router;
use tower_http::services::ServeDir;

use crate::config::Config;
use crate::inference::InferenceClient;

mod chat;
mod state;

pub use state::{AppState, ToolDecision};

/// Construye el router de la API y los estáticos.
///
/// `static_dir` es el directorio con el build de SvelteKit; se sirve en `/` con
/// `index.html` como fallback (SPA).
pub fn router(state: Arc<AppState>, static_dir: PathBuf) -> Router {
    let api = Router::new()
        .route("/health", get(chat::health))
        .route("/chat", post(chat::chat))
        .route("/tools/:id/decision", post(chat::tool_decision))
        .with_state(state);
    let assets = ServeDir::new(&static_dir).append_index_html_on_directories(true);
    Router::new().nest("/api", api).fallback_service(assets)
}

/// Levanta el servidor y bloquea hasta que termine (Ctrl-C lo detiene).
///
/// `static_dir` por defecto es `frontend/build` relativo al directorio actual.
pub async fn serve(cfg: &Config, static_dir: PathBuf) -> crate::Result<()> {
    let client = InferenceClient::new(
        cfg.inference.base_url.clone(),
        std::time::Duration::from_millis(cfg.inference.timeout_ms),
    )
    .map_err(|e| crate::Error::Config(e.to_string()))?;
    let state = Arc::new(AppState::new(client, cfg));
    let app = router(state, static_dir);

    let addr = SocketAddr::from(([127, 0, 0, 1], cfg.ui.port));
    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .map_err(|e| crate::Error::Config(format!("no se pudo abrir {addr}: {e}")))?;
    tracing::info!(%addr, "UI disponible");
    axum::serve(listener, app)
        .await
        .map_err(|e| crate::Error::Config(e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn router_builds_with_temp_static_dir() {
        let client =
            InferenceClient::new("http://127.0.0.1:8080", std::time::Duration::from_secs(1))
                .expect("client");
        let cfg = crate::config::test_config();
        let state = Arc::new(AppState::new(client, &cfg));
        // No debe entrar en pánico al construir el router.
        let _ = router(state, PathBuf::from("."));
    }
}
