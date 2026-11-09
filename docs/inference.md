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
17%, con gramática **100%** (umbral A: 90%).

Resultado 2026-11-09, fork TurboQuant en 2x RTX 3060 12 GB (Fedora 44):

| Modelo | KV | GPUs | Sin gramática | Con gramática | Umbral |
|---|---|---|---:|---:|---:|
| 3B Q4_K_M | turbo2 (32K) | 1 | 37% | **97%** | 90% (A) |
| 7B Q4_K_M | turbo3 (64K) | 2 | 23% | **100%** | 95% (B) |

El único fallo con gramática (3B) fue un `create_file` cortado por
`max_tokens = 256` del test, no una violación de la gramática.

## Compilar el fork en Linux

ADR-0004 fija el fork. En Linux nativo (RNF-04b) con CUDA ≥12.1:

```bash
git clone --branch feature/turboquant-kv-cache --depth 1 \
  https://github.com/TheTom/llama-cpp-turboquant.git
cd llama-cpp-turboquant
cmake -S . -B build -DGGML_CUDA=ON -DCMAKE_CUDA_ARCHITECTURES=86 \
  -DCMAKE_BUILD_TYPE=Release -DLLAMA_CURL=OFF
cmake --build build -j --target llama-server llama-bench
```

`86` es Ampere (serie 30xx). Si la versión de GCC es más nueva que la que
admite nvcc (GCC 16 con CUDA 13.3, p. ej.), añade
`-DCMAKE_CUDA_FLAGS=-allow-unsupported-compiler`. Verificado con el commit
`6439568` del fork, CUDA 13.3 y GCC 16.2.

## Caché KV TurboQuant (RF-05)

Los tipos `turbo2`…`turbo6` se eligen al arrancar el servidor con
`-ctk`/`-ctv` y requieren flash attention (`-fa on`):

```bash
# Perfil A: 3B, 32K, turbo2
llama-server -m models/qwen2.5-coder-3b-instruct-q4_k_m.gguf \
  -c 32768 -ngl 99 -fa on -np 1 -ctk turbo2 -ctv turbo2 --port 8080
# Perfil B: 7B, 64K, K f16 + V turbo3 (ADR-0006), capas repartidas entre las dos GPUs
llama-server -m models/qwen2.5-coder-7b-instruct-q4_k_m.gguf \
  -c 65536 -ngl 99 -fa on -np 1 -ctk f16 -ctv turbo3 --port 8080
```

El fork cambia por su cuenta lo que se pide en dos casos, y el log lo avisa:

- **Auto-asimetría** (`TURBO_AUTO_ASYMMETRIC=1` por defecto): con GQA alto
  (Qwen2.5-3B 8:1, 7B 7:1) sube K a `q8_0` para no degradar la calidad.
- **Boundary V** (`TURBO_LAYER_ADAPTIVE`): con V `turbo2`, las dos primeras y
  dos últimas capas guardan V en `q8_0`.

Con `TURBO_AUTO_ASYMMETRIC=0 TURBO_LAYER_ADAPTIVE=0` se obtiene el tipo pedido
tal cual.

### Medición 2026-11-09 (2x RTX 3060 12 GB de escritorio)

Tamaño KV: línea `llama_kv_cache: size` del log de `llama-server -lv 4`.
Velocidad: `llama-bench -fa 1 -p 0 -n 64 -d <profundidad>` (decodificación con
la caché ya llena hasta esa profundidad).

Perfil A, 3B Q4_K_M, 32K, una GPU (`CUDA_VISIBLE_DEVICES=0`):

| K / V efectivo | KV a 32K | VRAM total | tok/s @4K | tok/s @32K |
|---|---:|---:|---:|---:|
| f16 / f16 | 1152 MiB | 3.27 GB | 116.4 | 85.8 |
| q8_0 / turbo2 (por defecto al pedir turbo2) | 408 MiB | 2.53 GB | 100.1 | 50.4 |
| turbo2 / turbo2 (auto-asimetría off) | 153 MiB | 2.28 GB | 107.9 | 79.0 |

Perfil B, 7B Q4_K_M, 64K, dos GPUs (reparto por capas por defecto):

| K / V efectivo | KV a 64K | VRAM GPU0 / GPU1 | tok/s @4K | tok/s @64K |
|---|---:|---:|---:|---:|
| f16 / f16 | 3584 MiB | 4.51 / 4.69 GB | 65.9 | 39.2 |
| f16 / turbo3 | ≈2142 MiB¹ | — | 61.0 | 23.8 |
| q8_0 / turbo3 (por defecto al pedir turbo3) | 1302 MiB | 3.31 / 3.66 GB | 57.3 | 17.7 |
| turbo3 / turbo3 (auto-asimetría off) | 700 MiB | 3.71 / 4.09 GB | 49.5 | 16.9 |

¹ Calculado (K f16 1792 MiB + V turbo3 350 MiB), no leído del log.

Lectura frente a los KPI del SDD §1:

- **KV VRAM** (A <1.5 GB a 32K, B <3 GB a 64K): se cumple con todas las
  combinaciones turbo. En A también con f16, porque el GQA 8:1 de Qwen2.5-3B
  ya deja la caché pequeña.
- **Velocidad con contexto lleno** (A ≥12, B ≥22 tok/s): A la supera con
  margen en esta GPU de escritorio. Falta medirla en una 3060 Laptop 6 GB.
  En B, a 64K solo cumplen f16/f16 (que supera el KV de 3 GB) y
  f16/turbo3 (23.8 tok/s, justo por encima).
- **RF-06** (ADR-0005): el contexto máximo cabe entero en VRAM en A y B. La
  velocidad cae entre un 26% y un 69% de 4K al máximo; se acepta mientras se
  cumpla RNF-01.
- **Perfil B** usa f16/turbo3 (ADR-0006).

### Calidad: KL-divergence frente a f16/f16 (7B, perfil B)

`llama-perplexity --kl-divergence` sobre 10 trozos de 8192 tokens de código
(C++ del fork, Rust y Python del repo) y el SDD. Primero se guardan los logits
de referencia con `-ctk f16 -ctv f16 --kl-divergence-base base.bin`; después
cada combinación se compara con `--kl-divergence-base base.bin --kl-divergence`.

| K / V | PPL | KLD media | Mismo top-1 |
|---|---:|---:|---:|
| f16 / f16 (referencia) | 2.350 | — | — |
| **f16 / turbo3** | 2.354 | **0.0040** | **98.25%** |
| q8_0 / turbo3 | 2.358 | 0.0056 | 98.10% |
| f16 / turbo2 | 2.369 | 0.0131 | 96.80% |

El LLM-juez de AGENTS.md 11.5 (≥4/5 en 25 prompts) usa opencode y puntúa cada
respuesta por sí sola, viendo el código (ADR-0009, ADR-0010):

```bash
# 1. Respuestas con la configuración KV a evaluar (servidor del fork en :8080).
#    5 archivos x 5 preguntas = 25 tareas; el código se guarda con cada respuesta.
PYTHONPATH=python python -m forge_ml.judge generate --out cand.jsonl --context <5 archivos>
# 2. Puntuación (reanudable con --cache; no aprueba con menos de 25 tareas)
PYTHONPATH=python python -m forge_ml.judge score --answers cand.jsonl \
  --cache verdicts.jsonl --out report.json
```

Si la nota sale baja, puntúa también las respuestas f16/f16 de las mismas
tareas, para separar el efecto de la caché KV del límite del modelo base.

Estado: respuestas generadas el 2026-11-09; puntuación pendiente de que se
renueve la cuota gratuita de opencode.

## Aceptación de HU-03 / HU-03b con el contexto lleno

`orchestrator/tests/hu03_full_context.rs` rellena el contexto hasta >90% con
código real, genera 128 tokens y comprueba la velocidad de generación
(`timings.predicted_per_second` del servidor) y la VRAM de `nvidia-smi`.
Necesita el llama-server del fork arrancado con la configuración del perfil:

```bash
# Perfil A (umbrales por defecto: 32K, ≥12 tok/s, ≤5837 MiB en la GPU 0)
FORGE_TEST_LLAMA_URL=http://127.0.0.1:8080 \
  cargo test --release --test hu03_full_context -- --ignored --nocapture
# Perfil B
FORGE_TEST_CTX=65536 FORGE_TEST_MIN_TPS=22 FORGE_TEST_MAX_VRAM_MIB=11673 \
FORGE_TEST_GPUS=0,1 FORGE_TEST_LLAMA_URL=http://127.0.0.1:8080 \
  cargo test --release --test hu03_full_context -- --ignored --nocapture
```

Resultado 2026-11-09 (2x RTX 3060 12 GB de escritorio):

| Perfil | Config | Prompt | tok/s | VRAM | Umbrales |
|---|---|---:|---:|---|---|
| A (1 GPU) | 3B, turbo2, 32K | 32 005 tok | 50.6 | 2538 MiB | ≥12 tok/s, ≤5837 MiB ✅ |
| B (2 GPUs) | 7B, f16/turbo3, 64K | 64 902 tok | 23.7 | 3744 / 4028 MiB | ≥22 tok/s, ≤11 673 MiB ✅ |

El perfil A se ha medido en una GPU de escritorio (360 GB/s y 170 W). En la
3060 Laptop 6 GB hay que repetirlo para dar HU-03 por cerrada.

## Verificación

```powershell
cd orchestrator; cargo test inference     # (de)serialización, SSE, cliente con servidor falso
cargo test tool_call                      # parser estricto de tool-calls
```

Probado end-to-end contra `llama-server` con Qwen2.5-Coder-3B Q8_0 en una
RTX 3050 6 GB: `forge chat` muestra el primer token a ~0.85 s y el resto
según se generan.
