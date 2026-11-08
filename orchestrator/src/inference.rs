//! Cliente de inferencia contra `llama-server` vía la API OpenAI (RF-07).
//!
//! El modelo (3B en A, 7B en B) lo sirve `llama-server` como proceso externo
//! que expone `/v1/chat/completions` y `/v1/models`. El orquestador habla con
//! él por HTTP (reqwest, ADR-0002). La gramática GBNF (RF-08) se añade cuando
//! se integre el tool-calling.

use std::time::Duration;

use serde::{Deserialize, Serialize};

/// Error de inferencia.
#[derive(Debug, thiserror::Error)]
pub enum InferenceError {
    /// Fallo de transporte HTTP con el llama-server.
    #[error("transporte inferencia: {0}")]
    Transport(String),
    /// La respuesta no contenía ninguna opción (`choices` vacío).
    #[error("respuesta sin choices")]
    EmptyResponse,
}

/// Rol de un mensaje en el chat.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    /// Mensaje de sistema.
    System,
    /// Mensaje del usuario.
    User,
    /// Mensaje del asistente.
    Assistant,
}

/// Un mensaje del chat.
#[derive(Debug, Clone, Serialize)]
pub struct Message {
    /// Rol del emisor.
    pub role: Role,
    /// Contenido textual.
    pub content: String,
}

impl Message {
    /// Crea un mensaje de usuario.
    pub fn user(content: impl Into<String>) -> Self {
        Self {
            role: Role::User,
            content: content.into(),
        }
    }

    /// Crea un mensaje de sistema.
    pub fn system(content: impl Into<String>) -> Self {
        Self {
            role: Role::System,
            content: content.into(),
        }
    }
}

/// Parámetros de muestreo (mapean la sección `[inference]` de la config).
#[derive(Debug, Clone)]
pub struct SamplingParams {
    /// Nombre del modelo a solicitar.
    pub model: String,
    /// Temperatura de muestreo.
    pub temperature: f32,
    /// Núcleo de probabilidad (top-p).
    pub top_p: f32,
    /// Máximo de tokens a generar.
    pub max_tokens: u32,
}

impl Default for SamplingParams {
    fn default() -> Self {
        Self {
            model: "local".to_string(),
            temperature: 0.2,
            top_p: 0.95,
            max_tokens: 4096,
        }
    }
}

#[derive(Serialize)]
struct ChatRequest<'a> {
    model: &'a str,
    messages: &'a [Message],
    temperature: f32,
    top_p: f32,
    max_tokens: u32,
    stream: bool,
}

#[derive(Deserialize)]
struct ChatResponse {
    choices: Vec<Choice>,
}

#[derive(Deserialize)]
struct Choice {
    message: ResponseMessage,
}

#[derive(Deserialize)]
struct ResponseMessage {
    content: String,
}

/// Cliente del `llama-server` (API OpenAI). Asíncrono, basado en reqwest.
#[derive(Debug, Clone)]
pub struct InferenceClient {
    base_url: String,
    http: reqwest::Client,
}

impl InferenceClient {
    /// Crea un cliente contra `base_url` (p. ej. `http://127.0.0.1:8080`).
    pub fn new(base_url: impl Into<String>, timeout: Duration) -> Result<Self, InferenceError> {
        let http = reqwest::Client::builder()
            .timeout(timeout)
            .build()
            .map_err(|e| InferenceError::Transport(e.to_string()))?;
        Ok(Self {
            base_url: base_url.into().trim_end_matches('/').to_string(),
            http,
        })
    }

    /// `true` si el servidor responde en `/v1/models`.
    pub async fn health(&self) -> Result<bool, InferenceError> {
        let url = format!("{}/v1/models", self.base_url);
        let resp = self
            .http
            .get(url)
            .send()
            .await
            .map_err(|e| InferenceError::Transport(e.to_string()))?;
        Ok(resp.status().is_success())
    }

    /// Completa un chat y devuelve el texto de la primera opción (sin streaming).
    pub async fn chat(
        &self,
        messages: &[Message],
        params: &SamplingParams,
    ) -> Result<String, InferenceError> {
        let body = ChatRequest {
            model: &params.model,
            messages,
            temperature: params.temperature,
            top_p: params.top_p,
            max_tokens: params.max_tokens,
            stream: false,
        };
        let url = format!("{}/v1/chat/completions", self.base_url);
        let resp = self
            .http
            .post(url)
            .json(&body)
            .send()
            .await
            .map_err(|e| InferenceError::Transport(e.to_string()))?;
        if !resp.status().is_success() {
            return Err(InferenceError::Transport(format!("HTTP {}", resp.status())));
        }
        let parsed: ChatResponse = resp
            .json()
            .await
            .map_err(|e| InferenceError::Transport(e.to_string()))?;
        parsed
            .choices
            .into_iter()
            .next()
            .map(|c| c.message.content)
            .ok_or(InferenceError::EmptyResponse)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serializes_chat_request_roles_lowercase() {
        let msgs = vec![Message::system("eres util"), Message::user("hola")];
        let json = serde_json::to_string(&msgs).expect("serialize");
        assert!(json.contains("\"role\":\"system\""));
        assert!(json.contains("\"role\":\"user\""));
    }

    #[test]
    fn parses_openai_chat_response() {
        let json = r#"{"choices":[{"message":{"role":"assistant","content":"fn a(){}"}}]}"#;
        let r: ChatResponse = serde_json::from_str(json).expect("parse");
        assert_eq!(r.choices.len(), 1);
        assert_eq!(r.choices[0].message.content, "fn a(){}");
    }

    #[test]
    fn client_trims_trailing_slash() {
        let c =
            InferenceClient::new("http://127.0.0.1:8080/", Duration::from_secs(1)).expect("build");
        assert_eq!(c.base_url, "http://127.0.0.1:8080");
    }

    #[test]
    fn default_params_match_config_defaults() {
        let p = SamplingParams::default();
        assert_eq!(p.temperature, 0.2);
        assert_eq!(p.top_p, 0.95);
        assert_eq!(p.max_tokens, 4096);
    }
}
