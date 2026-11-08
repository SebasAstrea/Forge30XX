//! Forge30XX orchestrator library.
//!
//! Expone la configuración y los módulos que el binario `forge` compone.
//! Reglas: ver AGENTS.md 11.3 (capa Rust).
#![deny(unsafe_code)]
#![warn(missing_docs)]

pub mod config;
pub mod hardware;
pub mod inference;
pub mod rag;
pub mod tool_call;

/// Error raíz de la librería del orquestador.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// Fallo al cargar o validar la configuración.
    #[error("config: {0}")]
    Config(String),
}

/// Alias de resultado para la librería.
pub type Result<T> = std::result::Result<T, Error>;
