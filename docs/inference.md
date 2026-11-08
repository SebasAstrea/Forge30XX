# Inferencia mínima (RF-01, RF-07, RF-08, streaming)

El modelo lo sirve `llama-server` (del fork llama.cpp) como proceso externo que
expone la **API OpenAI** (`/v1/chat/completions`, `/v1/models`). El orquestador
Rust habla con él por HTTP vía `inference::InferenceClient` (reqwest, ADR-0002).
La topología HTTP (no gRPC) y el fork están fijados en ADR-0004.

- `chat`: respuesta completa, sin streaming.
- `chat_stream`: SSE (`stream: true`); llama a un callback por cada fragmento y
  devuelve el texto completo (ADR-0003). `forge chat` lo usa.
- `SamplingParams::grammar`: gramática GBNF opcional (RF-08), ver más abajo.

## Arranque del llama-server

Con un GGUF local (perfil A usa Qwen2.5-Coder-3B):

```powershell
cd <ruta-llama-cpp>
./llama-server.exe -m qwen2.5-coder-3b-instruct-q8_0.gguf `
  --host 127.0.0.1 --port 8080 -c 4096 -ngl 20 --no-webui
```

- `-ngl N`: número de capas en GPU (ajústalo a la VRAM; en 6 GB empieza por 20).
- `-c N`: tamaño de contexto.

## Configuración

Sección `[inference]` de `forge.config.toml`:

```toml
[inference]
base_url = "http://127.0.0.1:8080"
temperature = 0.2
top_p = 0.95
max_tokens = 4096
```

Override por entorno: `FORGE_INFERENCE__BASE_URL`, `FORGE_INFERENCE__MAX_TOKENS`…

## Uso

```powershell
forge chat
# > escribe una funcion factorial en python
# ```python
# def factorial(n): ...
# ```
# > exit
```

`forge chat` verifica `/v1/models` antes de abrir el REPL; si el servidor no
responde, informa y termina sin error. Mantiene el historial de la conversación
(system + user + assistant) entre turnos y muestra los tokens según llegan.

`timeout_ms` es total en `chat` y, en streaming, el tiempo máximo **entre
trozos** recibidos, para no cortar generaciones largas.

## Tool-calls con gramática GBNF (RF-08)

`grammars/tool_calls.gbnf` restringe la salida a
`{"name": "<tool>", "arguments": {...}}` con `name` en `[a-z][a-z0-9_]{0,63}`.
`tool_call::ToolCall::parse` valida esa forma de manera estricta (sin limpiar
bloques ``` ni texto extra).

```rust
let mut params = SamplingParams::default();
params.grammar = Some(std::fs::read_to_string("grammars/tool_calls.gbnf")?);
let call = ToolCall::parse(&client.chat(&msgs, &params).await?)?;
```

Medición (30 prompts de lectura/edición/creación/borrado/web/shell):

```powershell
$env:FORGE_TEST_LLAMA_URL = "http://127.0.0.1:8080"
cargo test --release --test rf08_tool_calls -- --ignored --nocapture
# FORGE_TEST_MIN_RATE=0.95 para el umbral del perfil B
```

Resultado 2026-11-08, RTX 3050 6 GB, Qwen2.5-Coder-3B **Q8_0**: sin gramática
17%, con gramática **100%** (umbral A: 90%). Pendiente repetirlo con Q4_K_M
(el modelo del KPI) y con 7B en perfil B.

## Verificación

```powershell
cd orchestrator; cargo test inference     # (de)serialización, SSE, cliente con servidor falso
cargo test tool_call                      # parser estricto de tool-calls
```

Probado end-to-end contra `llama-server` con Qwen2.5-Coder-3B Q8_0 en una
RTX 3050 6 GB: `forge chat` muestra el primer token a ~0.85 s y el resto
según se generan.
