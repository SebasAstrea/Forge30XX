//! Binario `forge`: CLI del orquestador Forge30XX.
//!
//! Subcomandos según README (RF-21). `doctor` y `chat` están implementados;
//! `run` y `profile` se completan en sus sprints (S9/S10).
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
        Command::Run { ui } => cmd_run(&cli.config, ui),
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
        Err(e) => print_no_gpu(&e),
    }

    let elapsed = started.elapsed();
    println!(
        "\nDiagnóstico completado en {} ms (objetivo RF-00: <3000 ms).",
        elapsed.as_millis()
    );
    Ok(())
}

fn print_no_gpu(e: &hardware::HardwareError) {
    println!("forge doctor — no se detectó GPU NVIDIA.");
    println!("  motivo: {e}");
    println!("  (requiere driver NVIDIA y nvidia-smi en PATH)");
    tracing::warn!(error = %e, "doctor: sin GPU");
}

fn print_recommendation(rec: &Recommendation) {
    println!("forge doctor — configuración recomendada");
    println!("  Perfil         : {}", rec.profile.label());
    println!("  Modelo         : {}", rec.model);
    println!("  Contexto       : {} tokens", rec.context_tokens);
    println!("  Caché KV (K/V) : {} / {}", rec.kv_type_k, rec.kv_type_v);
    println!("  llama-server   : {}", rec.llama_server_args().join(" "));
    if rec.warnings.is_empty() {
        println!("  Avisos         : ninguno");
    } else {
        println!("  Avisos         :");
        for w in &rec.warnings {
            println!("    - {w}");
        }
    }
}

fn cmd_run(config_path: &str, ui: bool) -> Result<()> {
    if !ui {
        println!("Nada que hacer sin --ui (el agente headless llega en otro sprint).");
        return Ok(());
    }
    let cfg = Config::load(config_path)?;
    // Estáticos de SvelteKit; configurable por si se sirve desde otra ruta.
    let static_dir = std::env::var("FORGE_UI_STATIC_DIR")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|_| std::path::PathBuf::from("frontend/build"));
    let runtime = tokio::runtime::Runtime::new()?;
    runtime.block_on(forge::server::serve(&cfg, static_dir))?;
    Ok(())
}

fn cmd_chat(config_path: &str) -> Result<()> {
    let cfg = Config::load(config_path)?;
    let inf = &cfg.inference;
    let (client, params) = build_inference(inf)?;
    let runtime = tokio::runtime::Runtime::new()?;

    // Verifica que el llama-server responde antes de abrir el REPL. Un servidor
    // caído o que no contesta 2xx se informa y no es un error del CLI.
    let healthy = runtime.block_on(client.health()).unwrap_or_else(|e| {
        tracing::warn!(error = %e, "chat: llama-server inalcanzable");
        false
    });
    if !healthy {
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
