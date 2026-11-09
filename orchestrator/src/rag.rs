//! Integración con el servicio RAG externo (port-rag-potty) vía HTTP.
//!
//! Ver docs/adr/0001-integracion-rag-portable.md (Opción C) y ADR-0002. El
//! motor del RAG corre como contenedor aparte; aquí viven la decisión de router
//! por perfil (RF-15, `RouterPolicy`) y el cliente HTTP (`RagClient`).

use std::time::Duration;

use serde::{Deserialize, Serialize};

/// Error de comunicación con el servicio RAG.
#[derive(Debug, thiserror::Error)]
pub enum RagError {
    /// Fallo de transporte HTTP (conexión, timeout, estado no 2xx).
    #[error("transporte RAG: {0}")]
    Transport(String),
}

/// Acción recomendada por el router de contexto.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RouteAction {
    /// Leer directamente los archivos candidatos (destino pequeño).
    Direct,
    /// Consultar el RAG por búsqueda semántica (destino grande).
    Rag,
    /// Localizar con RAG y leer el top-1.
    Hybrid,
}

/// Política de enrutado dependiente del perfil de hardware (RF-15).
///
/// `rag_threshold_tokens`: A = 1500, B = 3000. Por debajo del umbral conviene
/// leer el archivo directamente; por encima, el RAG ahorra contexto.
#[derive(Debug, Clone, Copy)]
pub struct RouterPolicy {
    threshold_tokens: u32,
}

impl RouterPolicy {
    /// Crea una política a partir del umbral de tokens configurado.
    pub fn new(threshold_tokens: u32) -> Self {
        Self { threshold_tokens }
    }

    /// Decide la acción según el tamaño estimado del destino en tokens.
    ///
    /// - destino < umbral  → `Direct`
    /// - destino >= umbral y hay candidatos concretos → `Hybrid`
    /// - destino >= umbral sin candidatos → `Rag`
    pub fn decide(&self, estimated_target_tokens: u32, has_candidates: bool) -> RouteAction {
        if estimated_target_tokens < self.threshold_tokens {
            RouteAction::Direct
        } else if has_candidates {
            RouteAction::Hybrid
        } else {
            RouteAction::Rag
        }
    }
}

/// Respuesta de `/route`: decisión del router.
#[derive(Debug, Clone, Deserialize)]
pub struct RouteResponse {
    /// "rag" | "direct" | "hybrid".
    pub action: String,
    /// Confianza [0,1].
    #[serde(default)]
    pub conf: f32,
    /// Ahorro neto estimado de tokens.
    #[serde(default)]
    pub save: i64,
    /// Archivos a leer si la acción es `direct`/`hybrid`.
    #[serde(default)]
    pub read: Vec<RouteRead>,
}

/// Un archivo propuesto por el router para lectura directa.
#[derive(Debug, Clone, Deserialize)]
pub struct RouteRead {
    /// Ruta del archivo a leer.
    pub path: String,
    /// Tokens estimados del archivo.
    #[serde(default)]
    pub tok: u32,
}

/// Un chunk recuperado por `/query`.
#[derive(Debug, Clone, Deserialize)]
pub struct QueryHit {
    /// Ruta del archivo de origen del chunk.
    #[serde(default)]
    pub path: String,
    /// Score coseno del chunk.
    #[serde(default)]
    pub score: f32,
    /// Texto del chunk.
    #[serde(default)]
    pub text: String,
}

#[derive(Debug, Deserialize)]
struct QueryResponse {
    #[serde(default)]
    results: Vec<QueryHit>,
}

#[derive(Serialize)]
struct RouteRequest<'a> {
    q: &'a str,
    k: u32,
    candidate_files: &'a [String],
    methodology_loaded: bool,
    loaded_files: &'a [String],
    compact: bool,
}

#[derive(Serialize)]
struct QueryRequest<'a> {
    q: &'a str,
    k: u32,
    include_methodology: bool,
}

/// Cliente HTTP asíncrono del servicio RAG (ADR-0002, `reqwest` sin TLS).
#[derive(Debug, Clone)]
pub struct RagClient {
    base_url: String,
    http: reqwest::Client,
}

impl RagClient {
    /// Crea un cliente contra `base_url` (p. ej. `http://localhost:8765`).
    pub fn new(base_url: impl Into<String>, timeout: Duration) -> Result<Self, RagError> {
        let http = reqwest::Client::builder()
            .timeout(timeout)
            .build()
            .map_err(|e| RagError::Transport(e.to_string()))?;
        Ok(Self {
            base_url: base_url.into().trim_end_matches('/').to_string(),
            http,
        })
    }

    /// `true` si el servicio responde en `/health`.
    pub async fn health(&self) -> Result<bool, RagError> {
        let url = format!("{}/health", self.base_url);
        let resp = self
            .http
            .get(url)
            .send()
            .await
            .map_err(|e| RagError::Transport(e.to_string()))?;
        Ok(resp.status().is_success())
    }

    /// Pregunta al router si conviene RAG, lectura directa o híbrido (RF-15).
    pub async fn route(
        &self,
        query: &str,
        k: u32,
        candidate_files: &[String],
    ) -> Result<RouteResponse, RagError> {
        let body = RouteRequest {
            q: query,
            k,
            candidate_files,
            methodology_loaded: false,
            loaded_files: &[],
            compact: true,
        };
        self.post_json("/route", &body).await
    }

    /// Búsqueda semántica: devuelve los chunks rankeados (RF-14).
    pub async fn query(&self, query: &str, k: u32) -> Result<Vec<QueryHit>, RagError> {
        let body = QueryRequest {
            q: query,
            k,
            include_methodology: true,
        };
        let resp: QueryResponse = self.post_json("/query", &body).await?;
        Ok(resp.results)
    }

    async fn post_json<B, R>(&self, path: &str, body: &B) -> Result<R, RagError>
    where
        B: Serialize + ?Sized,
        R: for<'de> Deserialize<'de>,
    {
        let url = format!("{}{}", self.base_url, path);
        let resp = self
            .http
            .post(url)
            .json(body)
            .send()
            .await
            .map_err(|e| RagError::Transport(e.to_string()))?;
        if !resp.status().is_success() {
            return Err(RagError::Transport(format!(
                "{path}: HTTP {}",
                resp.status()
            )));
        }
        resp.json::<R>()
            .await
            .map_err(|e| RagError::Transport(e.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::{closed_port_url, serve, Reply};

    #[test]
    fn small_target_reads_directly() {
        let policy = RouterPolicy::new(1500);
        assert_eq!(policy.decide(420, true), RouteAction::Direct);
    }

    #[test]
    fn large_target_without_candidates_uses_rag() {
        let policy = RouterPolicy::new(1500);
        assert_eq!(policy.decide(8000, false), RouteAction::Rag);
    }

    #[test]
    fn large_target_with_candidates_is_hybrid() {
        let policy = RouterPolicy::new(1500);
        assert_eq!(policy.decide(8000, true), RouteAction::Hybrid);
    }

    #[test]
    fn profile_b_threshold_is_higher() {
        let profile_b = RouterPolicy::new(3000);
        // 2500 tok: en A (1500) iría a RAG/híbrido; en B (3000) se lee directo.
        assert_eq!(profile_b.decide(2500, false), RouteAction::Direct);
    }

    #[test]
    fn client_builds_and_trims_trailing_slash() {
        let c = RagClient::new("http://localhost:8765/", Duration::from_secs(5)).expect("build");
        assert_eq!(c.base_url, "http://localhost:8765");
    }

    #[test]
    fn deserializes_route_response() {
        let json = r#"{"action":"direct","conf":0.86,"save":1180,
            "read":[{"path":"src/x.py","tok":420}]}"#;
        let r: RouteResponse = serde_json::from_str(json).expect("parse");
        assert_eq!(r.action, "direct");
        assert_eq!(r.read.len(), 1);
        assert_eq!(r.read[0].path, "src/x.py");
        assert_eq!(r.read[0].tok, 420);
    }

    #[test]
    fn deserializes_query_response() {
        let json = r#"{"results":[{"path":"a.rs","score":0.9,"text":"fn a(){}"}],
            "methodology_chunks":[]}"#;
        let r: QueryResponse = serde_json::from_str(json).expect("parse");
        assert_eq!(r.results.len(), 1);
        assert_eq!(r.results[0].path, "a.rs");
    }

    fn client(url: String) -> RagClient {
        RagClient::new(url, Duration::from_secs(5)).expect("build")
    }

    #[tokio::test]
    async fn health_reflects_service_status() {
        let up = serve(vec![("GET /health", Reply::json("{\"ok\":true}"))]).await;
        assert!(client(up).health().await.expect("health"));
        let down = serve(vec![("GET /health", Reply::status(503))]).await;
        assert!(!client(down).health().await.expect("health"));
    }

    #[tokio::test]
    async fn health_fails_when_service_is_unreachable() {
        let res = client(closed_port_url().await).health().await;
        assert!(matches!(res, Err(RagError::Transport(_))));
    }

    #[tokio::test]
    async fn route_parses_service_decision() {
        let body = r#"{"action":"hybrid","conf":0.7,"save":900,"read":[{"path":"a.rs"}]}"#;
        let url = serve(vec![("POST /route", Reply::json(body))]).await;
        let r = client(url)
            .route("dónde se parsea el SSE", 5, &["a.rs".to_string()])
            .await
            .expect("route");
        assert_eq!(r.action, "hybrid");
        assert_eq!(r.read[0].path, "a.rs");
        assert_eq!(r.read[0].tok, 0);
    }

    #[tokio::test]
    async fn query_returns_ranked_hits() {
        let body = r#"{"results":[{"path":"a.rs","score":0.9,"text":"x"},{"path":"b.rs"}]}"#;
        let url = serve(vec![("POST /query", Reply::json(body))]).await;
        let hits = client(url).query("parser", 2).await.expect("query");
        assert_eq!(hits.len(), 2);
        assert_eq!(hits[1].path, "b.rs");
    }

    #[tokio::test]
    async fn non_success_status_is_transport_error_with_path() {
        let url = serve(vec![("POST /query", Reply::status(500))]).await;
        let res = client(url).query("parser", 2).await;
        assert!(matches!(res, Err(RagError::Transport(m)) if m.contains("/query")));
    }

    #[tokio::test]
    async fn malformed_body_is_transport_error() {
        let url = serve(vec![("POST /route", Reply::json("no-json"))]).await;
        let res = client(url).route("q", 1, &[]).await;
        assert!(matches!(res, Err(RagError::Transport(_))));
    }
}
