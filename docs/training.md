# Entrenamiento QLoRA para tool-calling (RF-03, RF-03b, HU-01, HU-02)

El adaptador LoRA enseña al modelo base a responder con un tool-call
`{"name": "<tool>", "arguments": {...}}`, el formato de
`grammars/tool_calls.gbnf`. Los datos se describen en ADR-0012.

## Entorno

El extra `[train]` (torch, unsloth, peft, bitsandbytes, safetensors) va en un
venv aparte, `python/.venv-ml`, que `make train-lora` usa si existe:

```bash
cd python
python3.13 -m venv .venv-ml           # requiere las cabeceras (Python.h): triton compila un módulo C
.venv-ml/bin/pip install "torch>=2.4,<2.15" torchvision   # primero torch
.venv-ml/bin/pip install -e ".[train,dev]"
.venv-ml/bin/pip uninstall -y xformers  # unsloth trae uno compilado para otro torch
```

Notas verificadas el 2026-11-09 en Fedora 44:

- Sin instalar torch primero, pip prueba cientos de versiones de unsloth y no
  termina.
- Con Python 3.12 sin `python3.12-devel`, triton no compila y unsloth no
  arranca. Python 3.13 trae las cabeceras en este equipo.
- Si `~/.cache/huggingface` no es escribible, exporta `HF_HOME` a otra ruta.

## Datos

```bash
# Glaive function-calling v2 (Apache-2.0), fuera del repo (~271 MB)
curl -L -o glaive-v2.json \
  https://huggingface.co/datasets/glaiveai/glaive-function-calling-v2/resolve/main/glaive-function-calling-v2.json
make dataset GLAIVE=$PWD/glaive-v2.json   # → artifacts/dataset-v1/{train,eval}.jsonl + manifest.json
```

Por defecto: 6000 ejemplos de Glaive y 3000 sintéticos para las tools del
proyecto, con semilla 42 y un 5% para evaluación. De los 112 960 registros de
Glaive, 60 687 tienen una llamada a función utilizable. Los 30 prompts del test
RF-08 nunca entran en el entrenamiento (`test_dataset.py` lo comprueba).

## Entrenar

```bash
make train-lora PROFILE=A        # perfil A: 3B, una GPU
make train-lora PROFILE=B        # perfil B: 7B, device_map="auto" entre las dos GPUs
# Prueba corta del pipeline:
cd python && .venv-ml/bin/python -m forge_ml.train --profile A --max-steps 60
```

Hiperparámetros en `[training.A]` / `[training.B]` de `forge.config.toml`:
QLoRA 4-bit, r=16, alpha=32, lr 2e-4 con coseno, batch 2 x acumulación 4
(batch efectivo 8, RF-03b), `paged_adamw_8bit` y gradient checkpointing de
Unsloth. La pérdida solo cuenta la respuesta del asistente.

Salida en `artifacts/lora/<perfil>-<fecha>/`: `adapter/` (safetensors) y
`manifest.json` con la semilla, el modelo base y su revisión, el manifest del
dataset y las métricas, incluido el pico de VRAM (RNF-09).

## Resultados

Prueba de 60 pasos, perfil A, 2026-11-09, una RTX 3060 12 GB de escritorio:

| Métrica | Valor |
|---|---:|
| Pérdida de entrenamiento | 0.085 |
| Pérdida de evaluación (450 ejemplos) | 0.055 |
| Tiempo | 111 s (1.85 s/paso) |
| Pico de VRAM | 2490 MiB |

Con ese ritmo, una época completa (1069 pasos) tarda unos 33 min, lejos del
límite de 4 h de RF-03. El pico de 2.5 GB cabe en 6 GB, pero hay que
confirmarlo en la 3060 Laptop (HU-01).

Época completa, perfil A (`artifacts/lora/A-20261109T172555Z`):

| Métrica | Valor |
|---|---:|
| Pasos | 1069 (batch efectivo 8) |
| Pérdida de entrenamiento / evaluación | 0.040 / 0.030 |
| Tiempo | 33 min 35 s |
| Pico de VRAM | 2490 MiB |

## Usar el adaptador en el llama-server

```bash
# Convertir a GGUF con el script del fork. --base-model-id toma la config del
# modelo sin cuantizar: el script no admite bases bitsandbytes.
cd llama-cpp-turboquant
PYTHONPATH=gguf-py <venv-ml>/bin/python convert_lora_to_gguf.py \
  --base-model-id Qwen/Qwen2.5-Coder-3B-Instruct --outtype f16 \
  --outfile <run>/adapter-f16.gguf <run>/adapter
# Servir el modelo base Q4_K_M con el adaptador
llama-server -m models/qwen2.5-coder-3b-instruct-q4_k_m.gguf --lora <run>/adapter-f16.gguf \
  -c 32768 -ngl 99 -fa on -np 1 -ctk turbo2 -ctv turbo2 --port 8080
```

Test RF-08 (30 prompts que no están en el entrenamiento, system prompt
distinto del de entrenamiento) en el mismo servidor, con y sin `--lora`:

| | Sin gramática | Con gramática |
|---|---:|---:|
| 3B Q4_K_M, sin adaptador | 30% | 97% |
| 3B Q4_K_M + adaptador A | **100%** | **100%** |

HU-01 pide ≥90% de tool-calls válidos: se cumple incluso sin gramática.

Riesgo para S6: el adaptador solo ha visto respuestas que son tool-calls. El
bucle del agente también necesita turnos de respuesta final en texto, que
habrá que añadir al dataset cuando exista ese flujo.
