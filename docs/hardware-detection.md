# Detección de hardware y selección de perfil (RF-00, RF-21b, HU-00)

`forge doctor` detecta las GPUs NVIDIA del sistema y recomienda una
configuración óptima (perfil, modelo, contexto y tipos de caché K/V) según la matriz del SDD §2, avisando de condiciones subóptimas.

## Diseño

- **`hardware::recommend(&[Gpu]) -> Recommendation`**: lógica pura que aplica la
  matriz del SDD. Totalmente testeable sin GPU (10 tests unitarios).
- **`hardware::parse_nvidia_smi(&str) -> Vec<Gpu>`**: parseo de la salida CSV de
  `nvidia-smi` (nombre, VRAM, TGP y enlace PCIe), puro y testeable con muestras
  reales (`hardware/nvidia_smi.rs`).
- **`hardware::PcieLink`** (`hardware/pcie.rs`): generación y ancho del enlace,
  ancho de banda estimado y aviso si el enlace es x4 o menor (RF-06b). Se usa
  la generación máxima (en reposo la GPU baja a Gen1) y el ancho actual (el de
  la ranura real).
- **`Recommendation::llama_server_args()`**: flags del llama-server del fork
  para el perfil (`-c`, `-ngl 99`, `-fa on`, `-np 1`, `-ctk`, `-ctv`).
- **`hardware::detect() -> Recommendation`**: invoca `nvidia-smi` y delega en
  `recommend`. Única parte que toca el sistema; se ejercita en CI hardware
  (no se mockea la GPU — AGENTS.md 11.11).

## Matriz de decisión (SDD §2)

| GPU detectada            | VRAM  | Perfil  | Modelo                  | Contexto | KV (K / V)      |
| ------------------------ | ----- | ------- | ----------------------- | -------- | --------------- |
| 3060 Laptop              | 6 GB  | A       | Qwen3.5-4B Q4_K_M       | 32 768   | turbo2 / turbo2 |
| 3070 / 3070 Ti           | 8 GB  | A+      | 3B Q5_K_M               | 40 960   | turbo2 / turbo2 |
| 3060 12 GB / 3080 10 GB  | 10-12 | A+      | 7B Q4_K_M               | 49 152   | turbo3 / turbo3 |
| 2x 3060 12 GB            | 24    | B       | Qwen3.6-35B-A3B Q4_K_M  | 65 536   | f16 / turbo3    |
| 3080 Ti / 3090           | ≥24   | Premium | 7B Q5_K_M               | 100 000  | turbo3 / turbo3 |

El perfil B usa K en f16 (ADR-0006). No hay ventana KV: el contexto completo
vive en VRAM (ADR-0005).

Avisos (RF-21b, RF-06b): TGP < 100 W, GPU fuera de la serie 30xx, VRAM < 6 GiB y
enlace PCIe x4 o menor.

## Uso

```bash
# Nativo (requiere driver NVIDIA + nvidia-smi en PATH)
cargo run --release --bin forge -- doctor   # desde orchestrator/
# o, vía Makefile desde la raíz:
make doctor
```

Salida de ejemplo (perfil A, 3060 Laptop con TGP bajo):

```
forge doctor — configuración recomendada
  Perfil         : A
  Modelo         : Qwen3.5-4B Q4_K_M
  Contexto       : 32768 tokens
  Caché KV (K/V) : turbo2 / turbo2
  llama-server   : -c 32768 -ngl 99 -fa on -np 1 -ctk turbo2 -ctv turbo2
  Avisos         :
    - GPU 'NVIDIA GeForce RTX 3060 Laptop GPU' con TGP 80 W (<100 W): ...

Diagnóstico completado en 42 ms (objetivo RF-00: <3000 ms).
```

Salida real en `warmachine` (2x RTX 3060 12 GB, GPU0 en una ranura x1):

```
  Perfil         : B
  Caché KV (K/V) : f16 / turbo3
  llama-server   : -c 65536 -ngl 99 -fa on -np 1 -ctk f16 -ctv turbo3 --n-cpu-moe 2 --reasoning off
  Avisos         :
    - GPU 0 en PCIe Gen3 x1 (~1.0 GB/s): la carga del modelo y el reparto entre GPUs serán más lentos; ...
```

Sin GPU, `forge doctor` no falla: informa que no detectó hardware NVIDIA y el
motivo.

## Verificación

```bash
# Tests unitarios de la lógica de perfil (sin GPU)
cd orchestrator && cargo test hardware
```

Los tests cubren: 3060 Laptop → A, 3060 12 GB → A+, 2x3060 → B, 3070 → A+,
3090 → Premium, aviso de GPU no-30xx, error sin GPU, el parseo de `nvidia-smi`
(con y sin `power.max_limit` y PCIe), los avisos de PCIe estrecho y los flags
del llama-server.
