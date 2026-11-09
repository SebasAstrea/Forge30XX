//! Detección de GPUs con `nvidia-smi` (RF-00, RF-06b).
//!
//! `parse_nvidia_smi` es pura y se prueba con salidas reales; `query` es la
//! única función que toca el sistema y solo se ejercita con GPU real.

use std::process::Command;

use super::pcie::PcieLink;
use super::{Gpu, HardwareError};

/// Columnas pedidas a `nvidia-smi`, en el orden que espera el parser.
///
/// La generación es la máxima negociada (`gen.max`): en reposo la GPU baja a
/// Gen1 para ahorrar energía y `gen.current` engaña. El ancho es el actual: no
/// cambia en reposo y refleja la ranura real (una GPU x16 en ranura x1 da 1).
const QUERY: &str = "--query-gpu=name,memory.total,power.max_limit,\
pcie.link.gen.max,pcie.link.width.current";

/// Ejecuta `nvidia-smi` y devuelve las GPUs detectadas.
pub fn query() -> Result<Vec<Gpu>, HardwareError> {
    let output = Command::new("nvidia-smi")
        .args([QUERY, "--format=csv,noheader,nounits"])
        .output()
        .map_err(|e| HardwareError::NvidiaSmi(e.to_string()))?;
    if !output.status.success() {
        let err = String::from_utf8_lossy(&output.stderr).to_string();
        return Err(HardwareError::NvidiaSmi(err));
    }
    Ok(parse_nvidia_smi(&String::from_utf8_lossy(&output.stdout)))
}

/// Parsea el CSV de `nvidia-smi` (name, memory.total, power.max_limit y,
/// opcionalmente, pcie.link.gen.max y pcie.link.width.current).
///
/// Los valores `[N/A]` se tratan como ausentes.
pub fn parse_nvidia_smi(csv: &str) -> Vec<Gpu> {
    csv.lines().filter_map(parse_line).collect()
}

fn parse_line(line: &str) -> Option<Gpu> {
    let cols: Vec<&str> = line.trim().split(',').map(str::trim).collect();
    let name = cols.first().filter(|n| !n.is_empty())?.to_string();
    let vram_mib = cols.get(1)?.parse::<u32>().ok()?;
    let power_limit_w = cols.get(2).and_then(|v| int_prefix(v));
    let pcie = match (cols.get(3), cols.get(4)) {
        (Some(g), Some(w)) => parse_link(g, w),
        _ => None,
    };
    Some(Gpu {
        name,
        vram_mib,
        power_limit_w,
        pcie,
    })
}

fn parse_link(gen: &str, width: &str) -> Option<PcieLink> {
    Some(PcieLink {
        gen: int_prefix(gen)?.try_into().ok()?,
        width: int_prefix(width)?.try_into().ok()?,
    })
}

/// Parte entera de un valor como "170.00"; `None` para "[N/A]".
fn int_prefix(value: &str) -> Option<u32> {
    value.split('.').next()?.trim().parse().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_nvidia_smi_csv() {
        let csv = "NVIDIA GeForce RTX 3060 Laptop GPU, 6144, 80\n\
                   NVIDIA GeForce RTX 3060, 12288, 170.00\n";
        let gpus = parse_nvidia_smi(csv);
        assert_eq!(gpus.len(), 2);
        assert_eq!(gpus[0].vram_mib, 6144);
        assert_eq!(gpus[0].power_limit_w, Some(80));
        assert_eq!(gpus[1].power_limit_w, Some(170));
        assert_eq!(gpus[1].pcie, None);
    }

    #[test]
    fn parses_nvidia_smi_without_power() {
        // power.max_limit puede venir como "[N/A]" en algunos portátiles.
        let csv = "NVIDIA GeForce RTX 3060 Laptop GPU, 6144, [N/A]\n";
        let gpus = parse_nvidia_smi(csv);
        assert_eq!(gpus.len(), 1);
        assert_eq!(gpus[0].power_limit_w, None);
    }

    #[test]
    fn skips_blank_and_malformed_lines() {
        let csv = "\n, 6144, 80\nNVIDIA GeForce RTX 3060, abc, 80\n";
        assert!(parse_nvidia_smi(csv).is_empty());
    }
}
