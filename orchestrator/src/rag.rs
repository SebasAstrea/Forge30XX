//! Integración con el servicio RAG externo (port-rag-potty) vía HTTP.
//!
//! Ver docs/adr/0001-integracion-rag-portable.md (Opción C). El motor del RAG
//! corre como contenedor aparte; aquí vive la decisión de router por perfil
//! (RF-15) y la forma de las peticiones. El transporte HTTP concreto se añade
//! en Sprint S7 con la dependencia de cliente que apruebe el PO.

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

#[cfg(test)]
mod tests {
    use super::*;

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
}
