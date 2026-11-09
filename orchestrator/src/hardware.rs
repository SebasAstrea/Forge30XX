//! Detección de hardware y selección de perfil (RF-00, RF-21b, HU-00).
//!
//! La selección de perfil es **lógica pura** sobre una lista de GPUs
//! detectadas (`recommend`), testeable sin GPU. La detección real
//! (`detect`) invoca `nvidia-smi` y delega en `recommend`; se ejercita en
//! CI hardware (ver AGENTS.md 11.11: no mockear la GPU en integración real).
//!
//! Matriz de referencia: docs/SDD.md §2.

mod nvidia_smi;
mod pcie;

pub use nvidia_smi::parse_nvidia_smi;
pub use pcie::PcieLink;

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
    /// Enlace PCIe (generación máxima, ancho actual), si el driver lo reporta (RF-06b).
    pub pcie: Option<PcieLink>,
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
    /// Tipo de la caché K (`-ctk` del llama-server).
    pub kv_type_k: String,
    /// Tipo de la caché V (`-ctv` del llama-server).
    pub kv_type_v: String,
    /// Flags adicionales del llama-server propios del modelo (p. ej. MoE).
    pub extra_args: Vec<String>,
    /// Avisos para el usuario (p. ej. TGP bajo, GPU no soportada).
    pub warnings: Vec<String>,
}

impl Recommendation {
    /// Flags del llama-server del fork para esta recomendación (sin `-m`).
    ///
    /// Todas las capas en GPU; con varias GPUs, llama.cpp reparte capas y su KV
    /// según la VRAM libre (`-sm layer`), que rinde igual que el mejor `-ts`
    /// medido (docs/inference.md, RF-06b).
    pub fn llama_server_args(&self) -> Vec<String> {
        let ctx = self.context_tokens.to_string();
        let base = [
            "-c",
            &ctx,
            "-ngl",
            "99",
            "-fa",
            "on",
            "-np",
            "1",
            "-ctk",
            &self.kv_type_k,
            "-ctv",
            &self.kv_type_v,
        ];
        base.map(str::to_string)
            .into_iter()
            .chain(self.extra_args.iter().cloned())
            .collect()
    }
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
        kv_type_k: tier.kv_type_k.to_string(),
        kv_type_v: tier.kv_type_v.to_string(),
        extra_args: tier.extra_args.iter().map(|a| a.to_string()).collect(),
        warnings,
    })
}

/// Fila de la matriz de compatibilidad (SDD §2).
struct Tier {
    profile: Profile,
    model: &'static str,
    context_tokens: u32,
    kv_type_k: &'static str,
    kv_type_v: &'static str,
    extra_args: &'static [&'static str],
}

const TIER_B: Tier = Tier {
    profile: Profile::B,
    // ADR-0013: MoE 35B/3B activos. Con todas las capas en GPU no cabe; 2 capas
    // de expertos en RAM dejan 11.1/10.7 GB por GPU a 64K y 34.8 tok/s.
    model: "Qwen3.6-35B-A3B Q4_K_M",
    context_tokens: 65_536,
    // ADR-0006: K en f16 para mantener ≥22 tok/s a 64K.
    kv_type_k: "f16",
    kv_type_v: "turbo3",
    // Sin modo thinking: con él la gramática de tool-calls da 0% (RF-08).
    extra_args: &["--n-cpu-moe", "2", "--reasoning", "off"],
};
const TIER_PREMIUM: Tier = Tier {
    profile: Profile::Premium,
    model: "Qwen2.5-Coder-7B Q5_K_M",
    context_tokens: 100_000,
    kv_type_k: "turbo3",
    kv_type_v: "turbo3",
    extra_args: &[],
};
const TIER_A_PLUS_10: Tier = Tier {
    profile: Profile::APlus,
    model: "Qwen2.5-Coder-7B Q4_K_M",
    context_tokens: 49_152,
    kv_type_k: "turbo3",
    kv_type_v: "turbo3",
    extra_args: &[],
};
const TIER_A_PLUS_8: Tier = Tier {
    profile: Profile::APlus,
    model: "Qwen2.5-Coder-3B Q5_K_M",
    context_tokens: 40_960,
    kv_type_k: "turbo2",
    kv_type_v: "turbo2",
    extra_args: &[],
};
const TIER_A: Tier = Tier {
    profile: Profile::A,
    // ADR-0013: Apache-2.0, sustituye a Qwen2.5-Coder-3B (licencia qwen-research).
    model: "Qwen3.5-4B Q4_K_M",
    context_tokens: 32_768,
    kv_type_k: "turbo2",
    kv_type_v: "turbo2",
    extra_args: &[],
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

/// Avisos por GPU: arquitectura no soportada, TGP bajo o PCIe estrecho.
fn gpu_warnings(gpus: &[Gpu]) -> Vec<String> {
    let mut warnings = Vec::new();
    for (i, g) in gpus.iter().enumerate() {
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
        warnings.extend(g.pcie.and_then(|link| pcie::link_warning(i, link)));
    }
    warnings
}

/// Detecta las GPUs del sistema con `nvidia-smi` y recomienda configuración.
///
/// Toca el sistema: solo se ejercita con GPU real (CI hardware).
pub fn detect() -> Result<Recommendation, HardwareError> {
    recommend(&nvidia_smi::query()?)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn gpu(name: &str, vram_mib: u32, tgp: Option<u32>) -> Gpu {
        Gpu {
            name: name.to_string(),
            vram_mib,
            power_limit_w: tgp,
            pcie: None,
        }
    }

    fn with_link(mut g: Gpu, gen: u8, width: u8) -> Gpu {
        g.pcie = Some(PcieLink { gen, width });
        g
    }

    #[test]
    fn llama_server_args_match_profile() {
        let rec = recommend(&[
            gpu("NVIDIA GeForce RTX 3060", 12288, Some(170)),
            gpu("NVIDIA GeForce RTX 3060", 12288, Some(170)),
        ])
        .expect("rec");
        // ADR-0013: Qwen3.6-35B-A3B con 2 capas de expertos en RAM y sin thinking.
        assert_eq!(rec.model, "Qwen3.6-35B-A3B Q4_K_M");
        assert_eq!(
            rec.llama_server_args().join(" "),
            "-c 65536 -ngl 99 -fa on -np 1 -ctk f16 -ctv turbo3 --n-cpu-moe 2 --reasoning off"
        );
    }

    #[test]
    fn narrow_pcie_link_warns_with_bandwidth() {
        // Caso real de warmachine: GPU0 en Gen3 x1, GPU1 en Gen4 x16.
        let rec = recommend(&[
            with_link(gpu("NVIDIA GeForce RTX 3060", 12288, Some(170)), 3, 1),
            with_link(gpu("NVIDIA GeForce RTX 3060", 12288, Some(170)), 4, 16),
        ])
        .expect("rec");
        let pcie: Vec<_> = rec.warnings.iter().filter(|w| w.contains("PCIe")).collect();
        assert_eq!(pcie.len(), 1, "{:?}", rec.warnings);
        assert!(pcie[0].contains("GPU 0") && pcie[0].contains("Gen3 x1"));
        assert!(pcie[0].contains("1.0 GB/s"));
    }

    #[test]
    fn x8_link_does_not_warn() {
        let g = with_link(gpu("NVIDIA GeForce RTX 3060", 12288, Some(170)), 3, 8);
        let rec = recommend(&[g]).expect("rec");
        assert!(rec.warnings.is_empty(), "{:?}", rec.warnings);
    }

    #[test]
    fn link_bandwidth_per_generation() {
        assert_eq!(PcieLink { gen: 4, width: 16 }.bandwidth_gbps(), 31.5);
        assert_eq!(PcieLink { gen: 3, width: 4 }.bandwidth_gbps(), 3.9);
        assert_eq!(PcieLink { gen: 1, width: 1 }.bandwidth_gbps(), 0.3);
    }

    #[test]
    fn parses_nvidia_smi_pcie_columns() {
        let csv = "NVIDIA GeForce RTX 3060, 12288, 170.00, 3, 1\n\
                   NVIDIA GeForce RTX 3060, 12288, 170.00, [N/A], [N/A]\n";
        let gpus = parse_nvidia_smi(csv);
        assert_eq!(gpus[0].pcie, Some(PcieLink { gen: 3, width: 1 }));
        assert_eq!(gpus[1].pcie, None);
    }

    #[test]
    fn laptop_3060_6gb_is_profile_a() {
        let rec = recommend(&[gpu("NVIDIA GeForce RTX 3060 Laptop GPU", 6144, Some(80))])
            .expect("recommend");
        assert_eq!(rec.profile, Profile::A);
        // ADR-0013: Qwen3.5-4B (Apache-2.0) sustituye a Qwen2.5-Coder-3B.
        assert_eq!(rec.model, "Qwen3.5-4B Q4_K_M");
        assert_eq!(rec.context_tokens, 32_768);
        assert_eq!(
            (rec.kv_type_k.as_str(), rec.kv_type_v.as_str()),
            ("turbo2", "turbo2")
        );
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
        // ADR-0006: K f16 + V turbo3 es la única combinación medida que cumple
        // KV <3 GB y ≥22 tok/s a 64K.
        assert_eq!(
            (rec.kv_type_k.as_str(), rec.kv_type_v.as_str()),
            ("f16", "turbo3")
        );
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
}
