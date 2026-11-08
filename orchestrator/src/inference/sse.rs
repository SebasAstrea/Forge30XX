//! Decodificador de los eventos SSE que emite `llama-server` con `stream: true`.

use serde::Deserialize;

use super::InferenceError;

/// Evento útil extraído del stream.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum SseEvent {
    /// Fragmento de texto generado.
    Delta(String),
    /// Fin del stream (`data: [DONE]`).
    Done,
}

#[derive(Deserialize)]
struct StreamChunk {
    #[serde(default)]
    choices: Vec<StreamChoice>,
    error: Option<serde_json::Value>,
}

#[derive(Deserialize)]
struct StreamChoice {
    #[serde(default)]
    delta: Delta,
}

#[derive(Default, Deserialize)]
struct Delta {
    content: Option<String>,
}

/// Acumula bytes y devuelve eventos por cada línea `data:` completa.
///
/// Se bufferiza en bytes (no `String`) porque un trozo HTTP puede cortar un
/// carácter UTF-8 multibyte por la mitad.
#[derive(Debug, Default)]
pub(crate) struct SseDecoder {
    buf: Vec<u8>,
}

impl SseDecoder {
    pub(crate) fn push(&mut self, chunk: &[u8]) -> Result<Vec<SseEvent>, InferenceError> {
        self.buf.extend_from_slice(chunk);
        let mut events = Vec::new();
        while let Some(pos) = self.buf.iter().position(|&b| b == b'\n') {
            let line: Vec<u8> = self.buf.drain(..=pos).collect();
            let line =
                std::str::from_utf8(&line).map_err(|e| InferenceError::Protocol(e.to_string()))?;
            if let Some(event) = parse_line(line.trim_end_matches(['\r', '\n']))? {
                events.push(event);
            }
        }
        Ok(events)
    }
}

fn parse_line(line: &str) -> Result<Option<SseEvent>, InferenceError> {
    let Some(data) = line.strip_prefix("data:") else {
        return Ok(None); // líneas vacías, comentarios ":" y otros campos SSE
    };
    let data = data.trim_start();
    if data == "[DONE]" {
        return Ok(Some(SseEvent::Done));
    }
    let chunk: StreamChunk =
        serde_json::from_str(data).map_err(|e| InferenceError::Protocol(e.to_string()))?;
    if let Some(err) = chunk.error {
        return Err(InferenceError::Protocol(err.to_string()));
    }
    let text: String = chunk
        .choices
        .into_iter()
        .filter_map(|c| c.delta.content)
        .collect();
    Ok((!text.is_empty()).then_some(SseEvent::Delta(text)))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn delta(text: &str) -> String {
        format!("data: {{\"choices\":[{{\"delta\":{{\"content\":\"{text}\"}}}}]}}\n\n")
    }

    #[test]
    fn decodes_deltas_and_done() {
        let mut d = SseDecoder::default();
        let input = format!("{}{}data: [DONE]\n\n", delta("fn "), delta("a"));
        let events = d.push(input.as_bytes()).expect("decode");
        assert_eq!(
            events,
            vec![
                SseEvent::Delta("fn ".into()),
                SseEvent::Delta("a".into()),
                SseEvent::Done
            ]
        );
    }

    #[test]
    fn handles_line_split_across_chunks() {
        let mut d = SseDecoder::default();
        let input = delta("hola");
        let (a, b) = input.as_bytes().split_at(10);
        assert!(d.push(a).expect("first half").is_empty());
        assert_eq!(
            d.push(b).expect("second half"),
            vec![SseEvent::Delta("hola".into())]
        );
    }

    #[test]
    fn handles_utf8_split_across_chunks() {
        let mut d = SseDecoder::default();
        let input = delta("año");
        let cut = input.find('ñ').expect("ñ") + 1; // a mitad del carácter de 2 bytes
        let (a, b) = input.as_bytes().split_at(cut);
        assert!(d.push(a).expect("first half").is_empty());
        assert_eq!(
            d.push(b).expect("second half"),
            vec![SseEvent::Delta("año".into())]
        );
    }

    #[test]
    fn accepts_crlf_and_ignores_comments_and_empty_deltas() {
        let mut d = SseDecoder::default();
        let input = ": keep-alive\r\n\r\ndata: {\"choices\":[{\"delta\":{\"role\":\"assistant\"}}]}\r\n\r\ndata: {\"choices\":[{\"delta\":{\"content\":\"x\"}}]}\r\n\r\n";
        assert_eq!(
            d.push(input.as_bytes()).expect("decode"),
            vec![SseEvent::Delta("x".into())]
        );
    }

    #[test]
    fn server_error_event_is_an_error() {
        let mut d = SseDecoder::default();
        let input = "data: {\"error\":{\"message\":\"context full\"}}\n\n";
        assert!(matches!(
            d.push(input.as_bytes()),
            Err(InferenceError::Protocol(_))
        ));
    }

    #[test]
    fn malformed_json_is_an_error() {
        let mut d = SseDecoder::default();
        assert!(matches!(
            d.push(b"data: {no json\n\n"),
            Err(InferenceError::Protocol(_))
        ));
    }
}
