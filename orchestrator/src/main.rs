//! Binario `forge`: CLI del orquestador Forge30XX.
//!
//! Subcomandos según README. En fase bootstrap los comandos están declarados
//! pero su lógica se implementa en sprints posteriores (RF correspondientes).
#![deny(unsafe_code)]

use std::io::{self, Write};

use anyhow::Result;
use clap::{Parser, Subcommand};
use forge::config::Config;
use forge::hardware::{self, Recommendation};
use forge::inference::{InferenceClient, Message, SamplingParams};

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
        Command::Chat => cmd_chat(&cli.config),
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

fn cmd_chat(config_path: &str) -> Result<()> {
    let cfg = Config::load(config_path)?;
    let inf = &cfg.inference;
    let (client, params) = build_inference(inf)?;
    let runtime = tokio::runtime::Runtime::new()?;

    // Verifica que el llama-server responde antes de abrir el REPL.
    if !runtime.block_on(client.health())? {
        println!("No se pudo contactar al llama-server en {}.", inf.base_url);
        println!("Arranca llama-server con un modelo GGUF y vuelve a intentarlo.");
        return Ok(());
    }

    println!(
        "Forge30XX chat — modelo vía {}. Escribe 'exit' para salir.",
        inf.base_url
    );
    let mut history = vec![Message::system(
        "Eres Forge30XX, un asistente de codificación conciso y correcto.",
    )];
    while let Some(prompt) = read_prompt()? {
        history.push(Message::user(prompt));
        let turn = client.chat_stream(&history, &params, print_token);
        match runtime.block_on(turn) {
            Ok(reply) => {
                println!("\n");
                history.push(Message {
                    role: forge::inference::Role::Assistant,
                    content: reply,
                });
            }
            Err(e) => {
                tracing::error!(error = %e, "fallo de inferencia");
                println!("\n[error] {e}\n");
                history.pop(); // descarta el turno fallido
            }
        }
    }
    Ok(())
}

fn build_inference(inf: &forge::config::Inference) -> Result<(InferenceClient, SamplingParams)> {
    let client = InferenceClient::new(
        inf.base_url.clone(),
        std::time::Duration::from_millis(inf.timeout_ms),
    )?;
    let params = SamplingParams {
        model: "local".to_string(),
        temperature: inf.temperature,
        top_p: inf.top_p,
        max_tokens: inf.max_tokens,
        grammar: None, // chat libre; la gramática de tool-calls se usa en S6
    };
    Ok((client, params))
}

/// Lee la siguiente línea no vacía. `None` en EOF, `exit` o `quit`.
fn read_prompt() -> Result<Option<String>> {
    loop {
        print!("> ");
        io::stdout().flush()?;
        let mut line = String::new();
        if io::stdin().read_line(&mut line)? == 0 {
            return Ok(None);
        }
        match line.trim() {
            "" => continue,
            "exit" | "quit" => return Ok(None),
            prompt => return Ok(Some(prompt.to_string())),
        }
    }
}

fn print_token(token: &str) {
    let mut out = io::stdout().lock();
    // Un fallo de stdout no debe abortar la generación; se registra y sigue.
    if let Err(e) = out.write_all(token.as_bytes()).and_then(|()| out.flush()) {
        tracing::warn!(error = %e, "no se pudo escribir el token");
    }
}

fn cmd_profile(which: &str) -> Result<()> {
    tracing::info!(requested = %which, "profile");
    println!("perfil solicitado: {which}");
    Ok(())
}
