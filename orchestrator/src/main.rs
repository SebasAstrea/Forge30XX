//! Binario `forge`: CLI del orquestador Forge30XX.
//!
//! Subcomandos según README. En fase bootstrap los comandos están declarados
//! pero su lógica se implementa en sprints posteriores (RF correspondientes).
#![deny(unsafe_code)]

use anyhow::Result;
use clap::{Parser, Subcommand};
use forge::config::Config;
use forge::hardware::{self, Recommendation};

/// Agente de codificación local para GPUs Nvidia serie 30xx.
#[derive(Parser)]
#[command(name = "forge", version, about)]
struct Cli {
    /// Ruta al archivo de configuración.
    #[arg(long, default_value = "forge.config.toml")]
    config: String,

    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Detecta hardware y recomienda configuración (RF-00, RF-21b).
    Doctor,
    /// Inicia el agente (con o sin UI).
    Run {
        /// Lanza la UI web.
        #[arg(long)]
        ui: bool,
    },
    /// Chat interactivo en terminal.
    Chat,
    /// Fuerza o detecta el perfil de hardware.
    Profile {
        /// "A", "B" o "auto".
        #[arg(default_value = "auto")]
        which: String,
    },
    /// Muestra versión y build info.
    Version,
}

fn main() -> Result<()> {
    tracing_subscriber::fmt().json().init();
    let cli = Cli::parse();

    match cli.command {
        Command::Doctor => cmd_doctor(&cli.config),
        Command::Run { ui } => cmd_run(ui),
        Command::Chat => cmd_chat(),
        Command::Profile { which } => cmd_profile(&which),
        Command::Version => {
            println!("forge {}", env!("CARGO_PKG_VERSION"));
            Ok(())
        }
    }
}

fn cmd_doctor(config_path: &str) -> Result<()> {
    let started = std::time::Instant::now();
    // La configuración se carga para conocer overrides futuros; su ausencia no
    // impide el diagnóstico de hardware.
    if let Ok(cfg) = Config::load(config_path) {
        tracing::info!(profile = %cfg.hardware.profile, "doctor: config cargada");
    }

    match hardware::detect() {
        Ok(rec) => {
            print_recommendation(&rec);
            tracing::info!(profile = rec.profile.label(), "doctor ok");
        }
        Err(e) => {
            println!("forge doctor — no se detectó GPU NVIDIA.");
            println!("  motivo: {e}");
            println!("  (requiere driver NVIDIA y nvidia-smi en PATH)");
            tracing::warn!(error = %e, "doctor: sin GPU");
        }
    }

    let elapsed = started.elapsed();
    println!(
        "\nDiagnóstico completado en {} ms (objetivo RF-00: <3000 ms).",
        elapsed.as_millis()
    );
    Ok(())
}

fn print_recommendation(rec: &Recommendation) {
    println!("forge doctor — configuración recomendada");
    println!("  Perfil         : {}", rec.profile.label());
    println!("  Modelo         : {}", rec.model);
    println!("  Contexto       : {} tokens", rec.context_tokens);
    println!("  Cuantización KV: {}", rec.kv_quant);
    println!("  Ventana KVFlash: {} tokens", rec.kvflash_window);
    if rec.warnings.is_empty() {
        println!("  Avisos         : ninguno");
    } else {
        println!("  Avisos         :");
        for w in &rec.warnings {
            println!("    - {w}");
        }
    }
}

fn cmd_run(_ui: bool) -> Result<()> {
    tracing::warn!("run: pendiente Sprint S1 (RF-07 API OpenAI)");
    Ok(())
}

fn cmd_chat() -> Result<()> {
    tracing::warn!("chat: pendiente Sprint S1");
    Ok(())
}

fn cmd_profile(which: &str) -> Result<()> {
    tracing::info!(requested = %which, "profile");
    println!("perfil solicitado: {which}");
    Ok(())
}
