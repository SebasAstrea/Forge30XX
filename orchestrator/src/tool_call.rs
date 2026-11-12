//! Formato de tool-call que emite el modelo bajo la gramática GBNF (RF-08).
//!
//! Forma: `{"name": "<tool>", "arguments": {...}}`. La gramática vive en
//! `grammars/tool_calls.gbnf` y restringe la salida a exactamente esta forma.

use serde::Deserialize;
use serde_json::{Map, Value};

/// Error al interpretar un tool-call.
#[derive(Debug, thiserror::Error)]
pub enum ToolCallError {
    /// El texto no es un objeto JSON con `name` y `arguments`.
    #[error("tool-call JSON inválido: {0}")]
    Invalid(String),
    /// El nombre no cumple `[a-z][a-z0-9_]{0,63}`.
    #[error("nombre de tool inválido: {0}")]
    InvalidName(String),
}

/// Llamada a herramienta ya parseada.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ToolCall {
    /// Nombre de la herramienta.
    pub name: String,
    /// Argumentos como objeto JSON.
    pub arguments: Map<String, Value>,
}

impl ToolCall {
    /// Parsea la salida del modelo de forma estricta (sin limpiar ``` ni texto extra).
    pub fn parse(raw: &str) -> Result<Self, ToolCallError> {
        let call: Self =
            serde_json::from_str(raw.trim()).map_err(|e| ToolCallError::Invalid(e.to_string()))?;
        if !is_valid_name(&call.name) {
            return Err(ToolCallError::InvalidName(call.name));
        }
        Ok(call)
    }

    /// `true` si la tool requiere aprobación del usuario (RF-20, ADR-0015).
    ///
    /// Lista blanca *fail-safe*: solo las tools explícitamente de solo-lectura
    /// son no destructivas; cualquier otra (incluida una desconocida) se trata
    /// como destructiva y no se ejecuta sin aprobación.
    pub fn is_destructive(&self) -> bool {
        !READ_ONLY_TOOLS.contains(&self.name.as_str())
    }
}

/// Tools de solo-lectura que no requieren aprobación (ADR-0015). Toda tool
/// fuera de esta lista es destructiva por defecto.
const READ_ONLY_TOOLS: &[&str] = &["read_file", "ls", "grep", "find", "search", "web_search"];

fn is_valid_name(name: &str) -> bool {
    let mut chars = name.chars();
    let starts_ok = chars.next().is_some_and(|c| c.is_ascii_lowercase());
    starts_ok
        && name.len() <= 64
        && chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_valid_call() {
        let call = ToolCall::parse(r#"{"name": "read_file", "arguments": {"path": "src/a.rs"}}"#)
            .expect("valid");
        assert_eq!(call.name, "read_file");
        assert_eq!(call.arguments["path"], "src/a.rs");
    }

    #[test]
    fn accepts_surrounding_whitespace() {
        assert!(ToolCall::parse("\n {\"name\":\"ls\",\"arguments\":{}} \n").is_ok());
    }

    #[test]
    fn rejects_code_fences_and_prose() {
        let fenced = "```json\n{\"name\":\"ls\",\"arguments\":{}}\n```";
        assert!(matches!(
            ToolCall::parse(fenced),
            Err(ToolCallError::Invalid(_))
        ));
        let prose = "Claro: {\"name\":\"ls\",\"arguments\":{}}";
        assert!(matches!(
            ToolCall::parse(prose),
            Err(ToolCallError::Invalid(_))
        ));
    }

    #[test]
    fn rejects_missing_or_extra_fields() {
        assert!(ToolCall::parse(r#"{"name":"ls"}"#).is_err());
        assert!(ToolCall::parse(r#"{"name":"ls","arguments":{},"x":1}"#).is_err());
        assert!(ToolCall::parse(r#"{"name":"ls","arguments":[]}"#).is_err());
    }

    #[test]
    fn rejects_names_outside_grammar() {
        for bad in ["", "Read", "1ls", "rm -rf", &"a".repeat(65)] {
            let raw = format!("{{\"name\":\"{bad}\",\"arguments\":{{}}}}");
            assert!(
                matches!(ToolCall::parse(&raw), Err(ToolCallError::InvalidName(_))),
                "debería rechazar {bad:?}"
            );
        }
    }

    #[test]
    fn read_only_tools_are_not_destructive() {
        for name in ["read_file", "ls", "grep", "find", "search", "web_search"] {
            let raw = format!("{{\"name\":\"{name}\",\"arguments\":{{}}}}");
            let call = ToolCall::parse(&raw).expect("valid");
            assert!(!call.is_destructive(), "{name} no debería ser destructiva");
        }
    }

    #[test]
    fn write_and_unknown_tools_are_destructive() {
        // Tools de escritura conocidas y una desconocida: todas destructivas
        // (lista blanca fail-safe, ADR-0015).
        for name in ["delete_file", "edit", "shell", "write_file", "tool_nueva"] {
            let raw = format!("{{\"name\":\"{name}\",\"arguments\":{{}}}}");
            let call = ToolCall::parse(&raw).expect("valid");
            assert!(call.is_destructive(), "{name} debería ser destructiva");
        }
    }
}
