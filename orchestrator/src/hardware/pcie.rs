//! Enlace PCIe de cada GPU (RF-06b, HU-00b).
//!
//! Sin NVLink en la serie 3060, todo lo que cruza entre GPUs o entre GPU y RAM
//! va por PCIe. `nvidia-smi` informa la generación y el ancho del enlace
//! actual; con eso se estima el ancho de banda y se avisa de enlaces estrechos.

/// Ancho máximo de enlace (carriles) a partir del cual se avisa (SDD §2: x4).
const NARROW_WIDTH: u8 = 4;

/// GB/s útiles por carril según la generación (codificación 8b/10b o 128b/130b).
fn lane_gbps(gen: u8) -> f64 {
    match gen {
        1 => 0.25,
        2 => 0.5,
        3 => 0.985,
        4 => 1.969,
        _ => 3.938,
    }
}

/// Generación y ancho del enlace PCIe de una GPU.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PcieLink {
    /// Generación PCIe (1–5).
    pub gen: u8,
    /// Número de carriles (x1, x4, x8, x16).
    pub width: u8,
}

impl PcieLink {
    /// Ancho de banda teórico por sentido en GB/s, redondeado a una décima.
    pub fn bandwidth_gbps(self) -> f64 {
        (lane_gbps(self.gen) * f64::from(self.width) * 10.0).round() / 10.0
    }

    /// `true` si el enlace tiene x4 carriles o menos.
    pub fn is_narrow(self) -> bool {
        self.width <= NARROW_WIDTH
    }
}

/// Aviso para la GPU `index` si su enlace es estrecho.
pub fn link_warning(index: usize, link: PcieLink) -> Option<String> {
    link.is_narrow().then(|| {
        format!(
            "GPU {index} en PCIe Gen{} x{} (~{:.1} GB/s): la carga del modelo y el \
             reparto entre GPUs serán más lentos; usa una ranura x8/x16 si es posible",
            link.gen,
            link.width,
            link.bandwidth_gbps()
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn narrow_means_x4_or_less() {
        assert!(PcieLink { gen: 4, width: 4 }.is_narrow());
        assert!(!PcieLink { gen: 3, width: 8 }.is_narrow());
    }

    #[test]
    fn no_warning_for_wide_link() {
        assert_eq!(link_warning(1, PcieLink { gen: 4, width: 16 }), None);
    }
}
