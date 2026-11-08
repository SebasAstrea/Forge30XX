# Inferencia mínima (RF-01, RF-07)

El modelo lo sirve `llama-server` (del fork llama.cpp) como proceso externo que
expone la **API OpenAI** (`/v1/chat/completions`, `/v1/models`). El orquestador
Rust habla con él por HTTP vía `inference::InferenceClient` (reqwest, ADR-0002).

La gramática GBNF para tool-calls (RF-08) se añadirá al integrar el
tool-calling; por ahora el cliente hace chat de texto plano.

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
(system + user + assistant) entre turnos.

## Verificación

```powershell
cd orchestrator; cargo test inference     # (de)serialización OpenAI, cliente
```

Probado end-to-end contra `llama-server` con Qwen2.5-Coder-3B Q8_0 en una
RTX 3050 6 GB: `forge chat` envía el prompt y muestra la respuesta del modelo.
