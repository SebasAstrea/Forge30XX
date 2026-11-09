//! Detección de hardware y selección de perfil (RF-00, RF-21b, HU-00).
//!
//! La selección de perfil es **lógica pura** sobre una lista de GPUs
//! detectadas (`recommend`), testeable sin GPU. La detección real
//! (`detect`) invoca `nvidia-smi` y delega en `recommend`; se ejercita en
//! CI hardware (ver AGENTS.md 11.11: no mockear la GPU en integración real).
//!
//! Matriz de referencia: docs/SDD.md §2.

use std::process::Command;

/// Umbral de TGP por debajo del cual se avisa al usuario (SDD RF-21b).
const LOW_TGP_WATTS: u32 = 100;

/// Una GPU detectada en el sistema.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Gpu {
    /// Nombre del modelo reportado por el driver (p. ej. "NVIDIA GeForce RTX 3060").
    pub name: String,
    /// Memoria total en MiB.
    pub vram_mib: u32,
    /// Potencia máxima (TGP) en vatios, si el driver la reporta.
    pub power_limit_w: Option<u32>,
}

impl Gpu {
    /// VRAM aproximada en GiB (redondeo hacia abajo a entero).
    fn vram_gib(&self) -> u32 {
        self.vram_mib / 1024
    }

    /// `true` si el modelo pertenece a la serie 30xx (Ampere), único soportado.
    fn is_ampere_30xx(&self) -> bool {
        let n = self.name.to_ascii_lowercase();
        n.contains("rtx 30")
            || n.contains("rtx30")
            || n.contains(" 3060")
            || n.contains(" 3070")
            || n.contains(" 3080")
            || n.contains(" 3090")
    }
}

/// Perfil de hardware seleccionado.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Profile {
    /// RTX 3060 6 GB Laptop: 3B Q4, 32K.
    A,
    /// GPU single de 8-10-12 GB: 7B/3B, 40-48K.
    APlus,
    /// 2x RTX 3060 12 GB: 7B Q4 repartido, 64K.
    B,
    /// 3080 Ti / 3090 u otras >=12 GB de gama alta.
    Premium,
}

impl Profile {
    /// Etiqueta corta del perfil.
    pub fn label(self) -> &'static str {
        match self {
            Profile::A => "A",
            Profile::APlus => "A+",
            Profile::B => "B",
            Profile::Premium => "Premium",
        }
    }
}

/// Recomendación de configuración derivada del hardware (RF-21b, HU-00).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Recommendation {
    /// Perfil seleccionado.
    pub profile: Profile,
    /// Modelo recomendado.
    pub model: String,
    /// Tamaño de contexto en tokens.
    pub context_tokens: u32,
    /// Cuantización KV (turbo2/turbo3).
    pub kv_quant: String,
    /// Ventana KVFlash en tokens.
    pub kvflash_window: u32,
    /// Avisos para el usuario (p. ej. TGP bajo, GPU no soportada).
    pub warnings: Vec<String>,
}

/// Error de detección de hardware.
#[derive(Debug, thiserror::Error)]
pub enum HardwareError {
    /// No se pudo ejecutar `nvidia-smi`.
    #[error("no se pudo ejecutar nvidia-smi: {0}")]
    NvidiaSmi(String),
    /// No se detectó ninguna GPU compatible.
    #[error("no se detectaron GPUs NVIDIA")]
    NoGpu,
}

/// Selecciona perfil y configuración a partir de las GPUs detectadas.
///
/// Implementa la matriz del SDD §2. Función pura: sin I/O, totalmente testeable.
pub fn recommend(gpus: &[Gpu]) -> Result<Recommendation, HardwareError> {
    let first = gpus.first().ok_or(HardwareError::NoGpu)?;
    let mut warnings = gpu_warnings(gpus);
    let vram = first.vram_gib();
    let dual = gpus.len() >= 2 && gpus.iter().all(|g| g.vram_gib() >= 12);
    if !dual && vram < 6 {
        warnings.push(format!(
            "VRAM detectada {} GiB (<6 GiB): se degradará contexto para evitar OOM",
            vram
        ));
    }
    let tier = select_tier(vram, dual);
    Ok(Recommendation {
        profile: tier.profile,
        model: tier.model.to_string(),
        context_tokens: tier.context_tokens,
        kv_quant: tier.kv_quant.to_string(),
        kvflash_window: tier.kvflash_window,
        warnings,
    })
}

/// Fila de la matriz de compatibilidad (SDD §2).
struct Tier {
    profile: Profile,
    model: &'static str,
    context_tokens: u32,
    kv_quant: &'static str,
    kvflash_window: u32,
}

const TIER_B: Tier = Tier {
    profile: Profile::B,
    model: "Qwen2.5-Coder-7B Q4_K_M",
    context_tokens: 65_536,
    kv_quant: "turbo3",
    kvflash_window: 8_192,
};
const TIER_PREMIUM: Tier = Tier {
    profile: Profile::Premium,
    model: "Qwen2.5-Coder-7B Q5_K_M",
    context_tokens: 100_000,
    kv_quant: "turbo3",
    kvflash_window: 8_192,
};
const TIER_A_PLUS_10: Tier = Tier {
    profile: Profile::APlus,
    model: "Qwen2.5-Coder-7B Q4_K_M",
    context_tokens: 49_152,
    kv_quant: "turbo3",
    kvflash_window: 8_192,
};
const TIER_A_PLUS_8: Tier = Tier {
    profile: Profile::APlus,
    model: "Qwen2.5-Coder-3B Q5_K_M",
    context_tokens: 40_960,
    kv_quant: "turbo2",
    kvflash_window: 4_096,
};
const TIER_A: Tier = Tier {
    profile: Profile::A,
    model: "Qwen2.5-Coder-3B Q4_K_M",
    context_tokens: 32_768,
    kv_quant: "turbo2",
    kvflash_window: 4_096,
};

fn select_tier(vram_gib: u32, dual: bool) -> &'static Tier {
    match (dual, vram_gib) {
        (true, _) => &TIER_B,
        (false, 24..) => &TIER_PREMIUM,
        (false, 10..) => &TIER_A_PLUS_10,
        (false, 8..) => &TIER_A_PLUS_8,
        (false, _) => &TIER_A,
    }
}

/// Avisos por GPU: arquitectura no soportada o TGP por debajo del KPI.
fn gpu_warnings(gpus: &[Gpu]) -> Vec<String> {
    let mut warnings = Vec::new();
    for g in gpus {
        if !g.is_ampere_30xx() {
            warnings.push(format!(
                "GPU '{}' no es serie 30xx; soporte no garantizado (v1.0 solo Ampere 30xx)",
                g.name
            ));
        }
        if let Some(w) = g.power_limit_w.filter(|w| *w < LOW_TGP_WATTS) {
            warnings.push(format!(
                "GPU '{}' con TGP {} W (<{} W): rendimiento por debajo del KPI",
                g.name, w, LOW_TGP_WATTS
            ));
        }
    }
    warnings
}

/// Detecta las GPUs del sistema con `nvidia-smi` y recomienda configuración.
///
/// Toca el sistema: solo se ejercita con GPU real (CI hardware).
pub fn detect() -> Result<Recommendation, HardwareError> {
    let output = Command::new("nvidia-smi")
        .args([
            "--query-gpu=name,memory.total,power.max_limit",
            "--format=csv,noheader,nounits",
        ])
        .output()
        .map_err(|e| HardwareError::NvidiaSmi(e.to_string()))?;

    if !output.status.success() {
        let err = String::from_utf8_lossy(&output.stderr).to_string();
        return Err(HardwareError::NvidiaSmi(err));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let gpus = parse_nvidia_smi(&stdout);
    recommend(&gpus)
}

/// Parsea la salida CSV de `nvidia-smi` (name, memory.total, power.max_limit).
///
/// Pura y testeable con muestras reales de salida.
pub fn parse_nvidia_smi(csv: &str) -> Vec<Gpu> {
    csv.lines()
        .filter_map(|line| {
            let line = line.trim();
            if line.is_empty() {
                return None;
            }
            let mut cols = line.split(',').map(str::trim);
            let name = cols.next()?.to_string();
            let vram_mib = cols.next()?.parse::<u32>().ok()?;
            let power_limit_w = cols
                .next()
                .and_then(|v| v.split('.').next())
                .and_then(|v| v.trim().parse::<u32>().ok());
            Some(Gpu {
                name,
                vram_mib,
                power_limit_w,
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn gpu(name: &str, vram_mib: u32, tgp: Option<u32>) -> Gpu {
        Gpu {
            name: name.to_string(),
            vram_mib,
            power_limit_w: tgp,
        }
    }

    #[test]
    fn laptop_3060_6gb_is_profile_a() {
        let rec = recommend(&[gpu("NVIDIA GeForce RTX 3060 Laptop GPU", 6144, Some(80))])
            .expect("recommend");
        assert_eq!(rec.profile, Profile::A);
        assert_eq!(rec.context_tokens, 32_768);
        assert_eq!(rec.kv_quant, "turbo2");
        assert_eq!(rec.kvflash_window, 4_096);
        // TGP 80 W < 100 W → debe avisar (RF-21b).
        assert!(rec.warnings.iter().any(|w| w.contains("TGP")));
    }

    #[test]
    fn single_3060_12gb_is_aplus() {
        let rec = recommend(&[gpu("NVIDIA GeForce RTX 3060", 12288, Some(170))]).expect("rec");
        assert_eq!(rec.profile, Profile::APlus);
        assert_eq!(rec.context_tokens, 49_152);
        assert!(rec.warnings.is_empty());
    }

    #[test]
    fn dual_3060_12gb_is_profile_b() {
        let rec = recommend(&[
            gpu("NVIDIA GeForce RTX 3060", 12288, Some(170)),
            gpu("NVIDIA GeForce RTX 3060", 12288, Some(170)),
        ])
        .expect("rec");
        assert_eq!(rec.profile, Profile::B);
        assert_eq!(rec.context_tokens, 65_536);
        assert_eq!(rec.kv_quant, "turbo3");
        assert_eq!(rec.kvflash_window, 8_192);
    }

    #[test]
    fn rtx_3070_8gb_is_aplus() {
        let rec = recommend(&[gpu("NVIDIA GeForce RTX 3070", 8192, Some(220))]).expect("rec");
        assert_eq!(rec.profile, Profile::APlus);
        assert_eq!(rec.context_tokens, 40_960);
    }

    #[test]
    fn rtx_3090_24gb_is_premium() {
        let rec = recommend(&[gpu("NVIDIA GeForce RTX 3090", 24576, Some(350))]).expect("rec");
        assert_eq!(rec.profile, Profile::Premium);
    }

    #[test]
    fn non_ampere_gpu_warns() {
        let rec = recommend(&[gpu("NVIDIA GeForce RTX 4090", 24576, Some(450))]).expect("rec");
        assert!(rec.warnings.iter().any(|w| w.contains("no es serie 30xx")));
    }

    #[test]
    fn no_gpu_is_error() {
        assert!(matches!(recommend(&[]), Err(HardwareError::NoGpu)));
    }

    #[test]
    fn parses_nvidia_smi_csv() {
        let csv = "NVIDIA GeForce RTX 3060 Laptop GPU, 6144, 80\n\
                   NVIDIA GeForce RTX 3060, 12288, 170.00\n";
        let gpus = parse_nvidia_smi(csv);
        assert_eq!(gpus.len(), 2);
        assert_eq!(gpus[0].vram_mib, 6144);
        assert_eq!(gpus[0].power_limit_w, Some(80));
        assert_eq!(gpus[1].power_limit_w, Some(170));
    }

    #[test]
    fn parses_nvidia_smi_without_power() {
        // power.max_limit puede venir como "[N/A]" en algunos portátiles.
        let csv = "NVIDIA GeForce RTX 3060 Laptop GPU, 6144, [N/A]\n";
        let gpus = parse_nvidia_smi(csv);
        assert_eq!(gpus.len(), 1);
        assert_eq!(gpus[0].power_limit_w, None);
    }
}
