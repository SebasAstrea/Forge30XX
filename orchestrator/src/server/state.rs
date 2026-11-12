//! Estado compartido del servidor de la UI: cliente de inferencia, parámetros
//! de muestreo y registro de aprobaciones de tool-calls pendientes (RF-20).

use std::collections::HashMap;
use std::sync::Mutex;

use serde::{Deserialize, Serialize};
use tokio::sync::oneshot;

use crate::config::Config;
use crate::inference::{InferenceClient, SamplingParams};

/// Decisión del usuario sobre un tool-call destructivo (RF-20, ADR-0015).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ToolDecision {
    /// Aprobar la ejecución.
    Approve,
    /// Rechazar la ejecución.
    Reject,
}

/// Estado inyectado en los handlers de Axum.
pub struct AppState {
    /// Cliente hacia el llama-server.
    pub client: InferenceClient,
    /// Parámetros de muestreo derivados de `[inference]`.
    pub params: SamplingParams,
    /// Aprobaciones pendientes: `id` del tool_request → canal de respuesta.
    pending: Mutex<HashMap<String, oneshot::Sender<ToolDecision>>>,
}

impl AppState {
    /// Construye el estado a partir de la configuración.
    pub fn new(client: InferenceClient, cfg: &Config) -> Self {
        let params = SamplingParams {
            model: "local".to_string(),
            temperature: cfg.inference.temperature,
            top_p: cfg.inference.top_p,
            max_tokens: cfg.inference.max_tokens,
            grammar: None,
        };
        Self {
            client,
            params,
            pending: Mutex::new(HashMap::new()),
        }
    }

    /// Registra un tool_request pendiente y devuelve el receptor de la decisión.
    ///
    /// El handler del stream espera en el receptor hasta que
    /// `/api/tools/{id}/decision` resuelva el canal (RF-20: sin aprobación no
    /// se ejecuta).
    pub fn register_pending(&self, id: String) -> oneshot::Receiver<ToolDecision> {
        let (tx, rx) = oneshot::channel();
        // El lock solo protege el HashMap; nunca se mantiene a través de un await.
        // Si otro hilo entró en pánico con el lock tomado, recuperamos el guard
        // (el estado del HashMap sigue siendo coherente).
        self.pending
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .insert(id, tx);
        rx
    }

    /// Resuelve un tool_request pendiente. `true` si existía y se notificó.
    pub fn resolve_pending(&self, id: &str, decision: ToolDecision) -> bool {
        let sender = self
            .pending
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .remove(id);
        match sender {
            Some(tx) => tx.send(decision).is_ok(),
            None => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    fn state() -> AppState {
        let client =
            InferenceClient::new("http://127.0.0.1:8080", Duration::from_secs(1)).expect("client");
        AppState::new(client, &crate::config::test_config())
    }

    #[tokio::test]
    async fn resolve_delivers_decision_to_waiter() {
        let st = state();
        let rx = st.register_pending("t1".into());
        assert!(st.resolve_pending("t1", ToolDecision::Approve));
        assert_eq!(rx.await.expect("recv"), ToolDecision::Approve);
    }

    #[test]
    fn resolve_unknown_id_returns_false() {
        let st = state();
        assert!(!st.resolve_pending("missing", ToolDecision::Reject));
    }

    #[test]
    fn params_follow_config() {
        let st = state();
        assert_eq!(st.params.temperature, 0.2);
        assert_eq!(st.params.max_tokens, 4096);
        assert!(st.params.grammar.is_none());
    }
}
