//! Cliente de inferencia contra `llama-server` vía la API OpenAI (RF-07).
//!
//! El modelo (3B en A, 7B en B) lo sirve `llama-server` como proceso externo
//! que expone `/v1/chat/completions` y `/v1/models`. El orquestador habla con
//! él por HTTP (reqwest, ADR-0002). La gramática GBNF (RF-08) se añade cuando
//! se integre el tool-calling.

use std::time::Duration;

use serde::{Deserialize, Serialize};

mod sse;

/// Error de inferencia.
#[derive(Debug, thiserror::Error)]
pub enum InferenceError {
    /// Fallo de transporte HTTP con el llama-server.
    #[error("transporte inferencia: {0}")]
    Transport(String),
    /// La respuesta no contenía ninguna opción (`choices` vacío).
    #[error("respuesta sin choices")]
    EmptyResponse,
    /// El stream contenía un evento inválido o un error del servidor.
    #[error("protocolo inferencia: {0}")]
    Protocol(String),
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
    /// Gramática GBNF que restringe la salida (RF-08). `None` = texto libre.
    pub grammar: Option<String>,
}

impl Default for SamplingParams {
    fn default() -> Self {
        Self {
            model: "local".to_string(),
            temperature: 0.2,
            top_p: 0.95,
            max_tokens: 4096,
            grammar: None,
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
    #[serde(skip_serializing_if = "Option::is_none")]
    grammar: Option<&'a str>,
}

impl<'a> ChatRequest<'a> {
    fn new(messages: &'a [Message], params: &'a SamplingParams, stream: bool) -> Self {
        Self {
            model: &params.model,
            messages,
            temperature: params.temperature,
            top_p: params.top_p,
            max_tokens: params.max_tokens,
            stream,
            grammar: params.grammar.as_deref(),
        }
    }
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
    timeout: Duration,
}

fn transport(e: impl std::fmt::Display) -> InferenceError {
    InferenceError::Transport(e.to_string())
}

impl InferenceClient {
    /// Crea un cliente contra `base_url` (p. ej. `http://127.0.0.1:8080`).
    ///
    /// `timeout` es total en `chat` y entre trozos recibidos en `chat_stream`,
    /// para no cortar generaciones largas (ADR-0003).
    pub fn new(base_url: impl Into<String>, timeout: Duration) -> Result<Self, InferenceError> {
        let http = reqwest::Client::builder()
            .connect_timeout(timeout)
            .read_timeout(timeout)
            .build()
            .map_err(transport)?;
        Ok(Self {
            base_url: base_url.into().trim_end_matches('/').to_string(),
            http,
            timeout,
        })
    }

    /// `true` si el servidor responde en `/v1/models`.
    pub async fn health(&self) -> Result<bool, InferenceError> {
        let url = format!("{}/v1/models", self.base_url);
        let resp = self
            .http
            .get(url)
            .timeout(self.timeout)
            .send()
            .await
            .map_err(transport)?;
        Ok(resp.status().is_success())
    }

    /// Completa un chat y devuelve el texto de la primera opción (sin streaming).
    pub async fn chat(
        &self,
        messages: &[Message],
        params: &SamplingParams,
    ) -> Result<String, InferenceError> {
        let req = self
            .completion(messages, params, false)
            .timeout(self.timeout);
        let parsed: ChatResponse = send_ok(req).await?.json().await.map_err(transport)?;
        parsed
            .choices
            .into_iter()
            .next()
            .map(|c| c.message.content)
            .ok_or(InferenceError::EmptyResponse)
    }

    /// Completa un chat en streaming (SSE): llama a `on_token` con cada
    /// fragmento en orden y devuelve el texto completo al terminar.
    pub async fn chat_stream<F: FnMut(&str)>(
        &self,
        messages: &[Message],
        params: &SamplingParams,
        mut on_token: F,
    ) -> Result<String, InferenceError> {
        let mut resp = send_ok(self.completion(messages, params, true)).await?;
        let mut decoder = sse::SseDecoder::default();
        let mut full = String::new();
        while let Some(chunk) = resp.chunk().await.map_err(transport)? {
            for event in decoder.push(&chunk)? {
                match event {
                    sse::SseEvent::Delta(text) => {
                        on_token(&text);
                        full.push_str(&text);
                    }
                    sse::SseEvent::Done => return Ok(full),
                }
            }
        }
        Ok(full)
    }

    fn completion(
        &self,
        messages: &[Message],
        params: &SamplingParams,
        stream: bool,
    ) -> reqwest::RequestBuilder {
        let body = ChatRequest::new(messages, params, stream);
        let url = format!("{}/v1/chat/completions", self.base_url);
        self.http.post(url).json(&body)
    }
}

async fn send_ok(req: reqwest::RequestBuilder) -> Result<reqwest::Response, InferenceError> {
    let resp = req.send().await.map_err(transport)?;
    if !resp.status().is_success() {
        return Err(InferenceError::Transport(format!("HTTP {}", resp.status())));
    }
    Ok(resp)
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
    fn grammar_is_sent_only_when_set() {
        let msgs = [Message::user("x")];
        let mut params = SamplingParams::default();
        let without = serde_json::to_value(ChatRequest::new(&msgs, &params, false)).expect("json");
        assert!(without.get("grammar").is_none());
        params.grammar = Some("root ::= \"a\"".into());
        let with = serde_json::to_value(ChatRequest::new(&msgs, &params, true)).expect("json");
        assert_eq!(with["grammar"], "root ::= \"a\"");
        assert_eq!(with["stream"], true);
    }

    #[test]
    fn default_params_match_config_defaults() {
        let p = SamplingParams::default();
        assert_eq!(p.temperature, 0.2);
        assert_eq!(p.top_p, 0.95);
        assert_eq!(p.max_tokens, 4096);
    }

    /// Servidor HTTP falso que envía `body` en trozos de `chunk` bytes y cierra.
    async fn fake_sse_server(body: &'static str, chunk: usize) -> String {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind");
        let addr = listener.local_addr().expect("addr");
        tokio::spawn(async move {
            let (mut sock, _) = listener.accept().await.expect("accept");
            let mut req = [0u8; 4096];
            let _ = sock.read(&mut req).await;
            let head =
                "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nConnection: close\r\n\r\n";
            sock.write_all(head.as_bytes()).await.expect("head");
            for part in body.as_bytes().chunks(chunk) {
                sock.write_all(part).await.expect("part");
                sock.flush().await.expect("flush");
                tokio::time::sleep(Duration::from_millis(2)).await;
            }
        });
        format!("http://{addr}")
    }

    #[tokio::test]
    async fn chat_stream_emits_tokens_in_order_and_returns_full_text() {
        let body = concat!(
            "data: {\"choices\":[{\"delta\":{\"content\":\"def \"}}]}\n\n",
            "data: {\"choices\":[{\"delta\":{\"content\":\"año\"}}]}\n\n",
            "data: [DONE]\n\n",
        );
        let url = fake_sse_server(body, 7).await;
        let client = InferenceClient::new(url, Duration::from_secs(5)).expect("build");
        let mut seen = Vec::new();
        let full = client
            .chat_stream(&[Message::user("x")], &SamplingParams::default(), |t| {
                seen.push(t.to_string())
            })
            .await
            .expect("stream");
        assert_eq!(seen, vec!["def ", "año"]);
        assert_eq!(full, "def año");
    }

    #[tokio::test]
    async fn chat_stream_propagates_server_error_event() {
        let body = "data: {\"error\":{\"message\":\"boom\"}}\n\n";
        let url = fake_sse_server(body, 64).await;
        let client = InferenceClient::new(url, Duration::from_secs(5)).expect("build");
        let res = client
            .chat_stream(&[Message::user("x")], &SamplingParams::default(), |_| {})
            .await;
        assert!(matches!(res, Err(InferenceError::Protocol(_))));
    }
}
