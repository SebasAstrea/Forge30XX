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
make train-lora PROFILE=A        # perfil A: Qwen3.5-4B, una GPU
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

### Perfil A (Qwen3.5-4B)

Prueba corta, perfil A, 2026-11-19, una RTX 3060 12 GB de escritorio:

| Métrica | Valor |
|---|---:|
| Pasos | 60 |
| Pérdida de entrenamiento | 0.067 |
| Pérdida de evaluación (450 ejemplos) | 0.045 |
| Tiempo | 237 s |
| Pico de VRAM | 4110 MiB |

Época completa, perfil A (`artifacts/lora/A-20261119T230149Z`):

| Métrica | Valor |
|---|---:|
| Pasos | 1069 (batch efectivo 8) |
| Pérdida de entrenamiento / evaluación | 0.036 / 0.025 |
| Tiempo | 43 min 9 s (límite RF-03: 4 h) |
| Pico de VRAM | 4130 MiB |

RF-08 con el adaptador sobre el GGUF base Q4_K_M (30 prompts, 32K, turbo2):

| | Sin gramática | Con gramática |
|---|---:|---:|
| Qwen3.5-4B Q4_K_M, sin adaptador | 57% | 100% |
| Qwen3.5-4B Q4_K_M + adaptador A | **100%** | **100%** |

HU-01 pide ≥90% de tool-calls válidos: se cumple incluso sin gramática.

El GGUF fusionado también pasa HU-03: 32 196 tok de prompt, 56.3 tok/s y
3738 MiB a 32K con turbo2.

### Perfil A histórico (Qwen2.5-Coder-3B)

Medido antes de ADR-0013 (2026-11-09), una RTX 3060 12 GB de escritorio.
Prueba de 60 pasos:

| Métrica | Valor |
|---|---:|
| Pérdida de entrenamiento | 0.085 |
| Pérdida de evaluación (450 ejemplos) | 0.055 |
| Tiempo | 111 s (1.85 s/paso) |
| Pico de VRAM | 2490 MiB |

Con ese ritmo, una época completa (1069 pasos) tarda unos 33 min, lejos del
límite de 4 h de RF-03. El pico de 2.5 GB cabe en 6 GB, pero hay que
confirmarlo en la 3060 Laptop (HU-01).

Época completa, perfil A histórico (`artifacts/lora/A-20261109T172555Z`):

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
  --base-model-id Qwen/Qwen3.5-4B --outtype f16 \
  --outfile <run>/adapter-f16.gguf <run>/adapter
# Servir el modelo base Q4_K_M con el adaptador
llama-server -m models/Qwen3.5-4B-Q4_K_M.gguf --lora <run>/adapter-f16.gguf \
  -c 32768 -ngl 99 -fa on -np 1 -ctk turbo2 -ctv turbo2 --reasoning off --port 8080
```

Test RF-08 (30 prompts que no están en el entrenamiento, system prompt
distinto del de entrenamiento) en el mismo servidor, con y sin `--lora`. Los
resultados del perfil A con Qwen3.5-4B están en
[Resultados](#perfil-a-qwen35-4b).

Riesgo para S6: el adaptador solo ha visto respuestas que son tool-calls. El
bucle del agente también necesita turnos de respuesta final en texto, que
habrá que añadir al dataset cuando exista ese flujo.

## Perfil B (HU-02)

Época completa, 7B repartido entre las dos GPUs con `device_map="auto"`
(`artifacts/lora/B-20261109T182023Z`):

| Métrica | Valor |
|---|---:|
| Pasos | 1069 (batch efectivo 8) |
| Pérdida de entrenamiento / evaluación | 0.038 / 0.027 |
| Tiempo | 2 h 20 min (límite RF-03: 6 h) |
| Pico de VRAM | 4746 MiB por GPU |

El paso es ~4.5 veces más lento que en A (7.9 s frente a 1.85 s):
`device_map="auto"` reparte las capas pero solo trabaja una GPU cada vez.

RF-08 con 7B Q4_K_M, K f16 / V turbo3, 64K, dos GPUs:

| | Sin gramática | Con gramática |
|---|---:|---:|
| Sin adaptador | 20% | 100% |
| Con adaptador | **97%** | **100%** |
| Fusionado y recuantizado a Q4_K_M | 97% | 100% |

HU-02 pide ≥95%: se cumple sin gramática.

## Fusión y exportación a GGUF (RF-04)

Criterio de RF-04: ±2% frente al adaptador separado, medido con la
perplejidad de `eval.jsonl` (450 filas renderizadas con la plantilla de chat,
20 trozos de 4096 tokens). Las mediciones usan el ejecutable del fork
(`llama-perplexity -f <eval-chat.txt> -c 4096 --chunks 20 -ngl 99 -fa on`).

### Perfil A (Qwen3.5-4B)

`convert_hf_to_gguf.py` del fork convierte la base de 16 bits a GGUF f16
(8.65 GB, 441 tensores), y el adaptador se fusiona sobre ese f16 para
cuantizar a Q4_K_M una sola vez:

```bash
convert_hf_to_gguf.py <snapshot>/ --outtype f16 --outfile a-base-f16.gguf
llama-export-lora -m a-base-f16.gguf --lora <run>/adapter-f16.gguf -o a-merged-f16.gguf
llama-quantize a-merged-f16.gguf a-merged-q4_k_m.gguf Q4_K_M
```

| Modelo | PPL | Frente al adaptador |
|---|---:|---:|
| Base Q4_K_M sin adaptador | 1.1912 | +0.73% |
| Base Q4_K_M + adaptador separado | 1.1825 | — |
| Fusionado f16 (8.1 GiB) | 1.1794 | −0.26% ✅ |
| Fusionado y recuantizado a Q4_K_M (2.6 GiB) | 1.1796 | −0.25% ✅ |

El fusionado Q4_K_M también pasa RF-08: 100% sin gramática y 100% con
gramática en los 30 prompts.

### Perfil B (Qwen2.5-Coder-7B, histórico)

El 7B se fusionó sobre su propio GGUF Q4_K_M (el export de 16 bits nunca se
completó antes de ADR-0014), y `llama-export-lora` rechaza las claves split.*
del GGUF:

```bash
PYTHONPATH=gguf-py python gguf-py/gguf/scripts/gguf_new_metadata.py --force \
  --remove-metadata split.no --remove-metadata split.count --remove-metadata split.tensors.count \
  models/qwen2.5-coder-7b-instruct-q4_k_m.gguf base-nosplit.gguf
llama-export-lora -m base-nosplit.gguf --lora <run>/adapter-f16.gguf -o merged-f16.gguf
llama-quantize --allow-requantize merged-f16.gguf merged-q4_k_m.gguf Q4_K_M
```

| Modelo | PPL | Frente al adaptador |
|---|---:|---:|
| Base Q4_K_M sin adaptador | 1.3143 | +5.7% |
| Base Q4_K_M + adaptador separado | 1.2432 | — |
| Fusionado f16 (13.8 GB) | 1.2421 | −0.1% ✅ |
| Fusionado y recuantizado a Q4_K_M (4.7 GB) | 1.2830 | **+3.2% ❌** |

La fusión es correcta (f16 coincide con el adaptador), pero recuantizar un
modelo que ya era Q4_K_M cuantiza dos veces y pierde un 3.2%. HU-02 se retiró
en ADR-0014, así que ese export no bloquea RF-04: el criterio se cumple con
el perfil A.
