//! Carga de configuración desde `forge.config.toml` y variables de entorno
//! con prefijo `FORGE_` (figment). Sin hardcode de rutas ni puertos.

use figment::{
    providers::{Env, Format, Toml},
    Figment,
};
use serde::Deserialize;

/// Configuración de hardware y selección de perfil.
#[derive(Debug, Clone, Deserialize)]
pub struct Hardware {
    /// Perfil de hardware: "A", "B" o "auto".
    pub profile: String,
    /// Índice de la GPU principal.
    pub gpu_index: u32,
}

/// Configuración de la UI.
#[derive(Debug, Clone, Deserialize)]
pub struct Ui {
    /// Puerto HTTP de la UI/API local.
    pub port: u16,
}

/// Configuración del router de contexto (RF-15).
///
/// `rag_threshold_tokens` por perfil: A 1500, B 3000.
#[derive(Debug, Clone, Deserialize)]
pub struct ContextRouter {
    /// Umbral de tokens a partir del cual conviene consultar el RAG.
    pub rag_threshold_tokens: u32,
    /// Porcentaje de la ventana al que se resume el historial (RF-16).
    pub summarize_at_percent: u8,
}

/// Configuración del servicio RAG externo (ADR-0001, Opción C).
#[derive(Debug, Clone, Deserialize)]
pub struct Rag {
    /// URL base del servicio port-rag-potty (p. ej. http://localhost:8765).
    pub base_url: String,
    /// Timeout de las peticiones HTTP en milisegundos.
    #[serde(default = "default_rag_timeout_ms")]
    pub timeout_ms: u64,
}

fn default_rag_timeout_ms() -> u64 {
    5_000
}

/// Configuración de inferencia (RF-07). Apunta al llama-server externo.
#[derive(Debug, Clone, Deserialize)]
pub struct Inference {
    /// URL base del llama-server (API OpenAI).
    #[serde(default = "default_inference_url")]
    pub base_url: String,
    /// Temperatura de muestreo.
    pub temperature: f32,
    /// Top-p (núcleo de probabilidad).
    pub top_p: f32,
    /// Máximo de tokens a generar.
    pub max_tokens: u32,
    /// Timeout de las peticiones en milisegundos.
    #[serde(default = "default_inference_timeout_ms")]
    pub timeout_ms: u64,
}

fn default_inference_url() -> String {
    "http://127.0.0.1:8080".to_string()
}

fn default_inference_timeout_ms() -> u64 {
    120_000
}

/// Configuración raíz de Forge30XX.
#[derive(Debug, Clone, Deserialize)]
pub struct Config {
    /// Sección de hardware.
    pub hardware: Hardware,
    /// Sección de UI.
    pub ui: Ui,
    /// Router de contexto.
    pub context_router: ContextRouter,
    /// Servicio RAG externo.
    pub rag: Rag,
    /// Inferencia (llama-server).
    pub inference: Inference,
}

impl Config {
    /// Carga la configuración desde un archivo TOML y variables `FORGE_`.
    ///
    /// Las variables de entorno sobreescriben el archivo.
    pub fn load(path: &str) -> crate::Result<Self> {
        Figment::new()
            .merge(Toml::file(path))
            .merge(Env::prefixed("FORGE_").split("__"))
            .extract()
            .map_err(|e| crate::Error::Config(e.to_string()))
    }
}

/// Config mínima para tests de otros módulos (servidor, etc.).
#[cfg(test)]
pub fn test_config() -> Config {
    Config {
        hardware: Hardware {
            profile: "A".to_string(),
            gpu_index: 0,
        },
        ui: Ui { port: 7860 },
        context_router: ContextRouter {
            rag_threshold_tokens: 1500,
            summarize_at_percent: 70,
        },
        rag: Rag {
            base_url: "http://localhost:8765".to_string(),
            timeout_ms: 5_000,
        },
        inference: Inference {
            base_url: "http://127.0.0.1:8080".to_string(),
            temperature: 0.2,
            top_p: 0.95,
            max_tokens: 4096,
            timeout_ms: 120_000,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn env_overrides_file_port() {
        // El tipo de retorno del closure lo impone figment::Jail::expect_with
        // (Result<_, figment::Error>); no controlamos su tamaño, de ahí el allow.
        #[allow(clippy::result_large_err)]
        fn body(jail: &mut figment::Jail) -> Result<(), figment::Error> {
            jail.create_file(
                "forge.config.toml",
                r#"
                [hardware]
                profile = "auto"
                gpu_index = 0
                [ui]
                port = 7860
                [context_router]
                rag_threshold_tokens = 1500
                summarize_at_percent = 70
                [rag]
                base_url = "http://localhost:8765"
                [inference]
                temperature = 0.2
                top_p = 0.95
                max_tokens = 4096
                "#,
            )?;
            jail.set_env("FORGE_UI__PORT", "9000");
            let cfg = Config::load("forge.config.toml").expect("config loads");
            assert_eq!(cfg.ui.port, 9000);
            assert_eq!(cfg.hardware.profile, "auto");
            assert_eq!(cfg.context_router.rag_threshold_tokens, 1500);
            assert_eq!(cfg.rag.timeout_ms, 5_000);
            assert_eq!(cfg.inference.base_url, "http://127.0.0.1:8080");
            assert_eq!(cfg.inference.max_tokens, 4096);
            Ok(())
        }
        figment::Jail::expect_with(body);
    }
}
