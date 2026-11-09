//! HU-03 / HU-03b: velocidad y VRAM con el contexto lleno (RNF-01, RNF-03).
//!
//! Requiere un llama-server real del fork arrancado con la configuración del
//! perfil (no se mockea la GPU, AGENTS.md 11.11). Ver `docs/inference.md`.
//!
//! ```text
//! # Perfil A (por defecto): 32K, ≥12 tok/s, ≤5837 MiB en la GPU 0
//! FORGE_TEST_LLAMA_URL=http://127.0.0.1:8080 \
//!   cargo test --release --test hu03_full_context -- --ignored --nocapture
//! # Perfil B: FORGE_TEST_CTX=65536 FORGE_TEST_MIN_TPS=22 FORGE_TEST_MAX_VRAM_MIB=11673
//! ```
//!
//! `FORGE_TEST_GPUS` (por defecto "0") lista los índices de GPU a vigilar.

// Crate de test: todo su código es test, donde AGENTS.md 11.1 permite expect().
#![allow(clippy::expect_used)]

use std::process::Command;
use std::time::Duration;

use serde_json::{json, Value};

/// Texto de relleno: código real del propio orquestador.
const FILLER: &str = concat!(
    include_str!("../src/inference.rs"),
    include_str!("../src/hardware.rs"),
    include_str!("../src/rag.rs"),
);
/// Tokens que se reservan para la pregunta, la plantilla y la respuesta.
const HEADROOM_TOKENS: u64 = 600;
const GEN_TOKENS: u64 = 128;

fn env_or<T: std::str::FromStr>(key: &str, default: T) -> T {
    std::env::var(key)
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(default)
}

async fn post(http: &reqwest::Client, url: String, body: Value) -> Value {
    let resp = http.post(url).json(&body).send().await.expect("petición");
    assert!(resp.status().is_success(), "HTTP {}", resp.status());
    resp.json().await.expect("json")
}

/// Repite `FILLER` y lo recorta para acercarse a `ctx - HEADROOM_TOKENS` tokens.
async fn filler_prompt(http: &reqwest::Client, base: &str, ctx: u64) -> String {
    let tok = post(
        http,
        format!("{base}/tokenize"),
        json!({ "content": FILLER }),
    )
    .await;
    let per_copy = tok["tokens"].as_array().expect("tokens").len().max(1) as u64;
    let target_chars = FILLER.chars().count() as u64 * (ctx - HEADROOM_TOKENS) / per_copy;
    let copies = target_chars as usize / FILLER.chars().count().max(1) + 1;
    FILLER
        .repeat(copies)
        .chars()
        .take(target_chars as usize)
        .collect()
}

/// MiB usados en cada GPU indicada, según `nvidia-smi`.
fn vram_used_mib(gpus: &str) -> Vec<(String, u64)> {
    let out = Command::new("nvidia-smi")
        .args([
            "--query-gpu=index,memory.used",
            "--format=csv,noheader,nounits",
        ])
        .output()
        .expect("nvidia-smi");
    let wanted: Vec<&str> = gpus.split(',').map(str::trim).collect();
    String::from_utf8_lossy(&out.stdout)
        .lines()
        .filter_map(|l| l.split_once(','))
        .map(|(i, m)| (i.trim().to_string(), m.trim().parse().unwrap_or(u64::MAX)))
        .filter(|(i, _)| wanted.contains(&i.as_str()))
        .collect()
}

#[tokio::test]
#[ignore = "requiere llama-server real del fork (FORGE_TEST_LLAMA_URL)"]
async fn hu03_full_context_meets_speed_and_vram() {
    let base: String = env_or("FORGE_TEST_LLAMA_URL", "http://127.0.0.1:8080".to_string());
    let ctx: u64 = env_or("FORGE_TEST_CTX", 32_768);
    let min_tps: f64 = env_or("FORGE_TEST_MIN_TPS", 12.0);
    let max_vram: u64 = env_or("FORGE_TEST_MAX_VRAM_MIB", 5_837);
    let gpus: String = env_or("FORGE_TEST_GPUS", "0".to_string());
    let http = reqwest::Client::builder()
        .timeout(Duration::from_secs(900))
        .build()
        .expect("cliente");

    let code = filler_prompt(&http, &base, ctx).await;
    let body = json!({
        "messages": [{"role": "user", "content": format!("{code}\n\nResume este código.")}],
        "max_tokens": GEN_TOKENS, "temperature": 0, "ignore_eos": true,
    });
    let reply = post(&http, format!("{base}/v1/chat/completions"), body).await;
    let t = &reply["timings"];
    let prompt_n = t["prompt_n"].as_u64().expect("prompt_n");
    let tps = t["predicted_per_second"]
        .as_f64()
        .expect("predicted_per_second");
    let vram = vram_used_mib(&gpus);

    println!(
        "HU-03: prompt {prompt_n} tok (ctx {ctx}), generación {tps:.1} tok/s, VRAM {vram:?} MiB"
    );
    assert!(
        prompt_n * 10 >= ctx * 9,
        "el prompt no llena el 90% del contexto"
    );
    assert!(tps >= min_tps, "{tps:.1} tok/s < {min_tps}");
    assert!(!vram.is_empty(), "nvidia-smi no devolvió las GPUs {gpus}");
    assert!(
        vram.iter().all(|(_, m)| *m <= max_vram),
        "VRAM > {max_vram} MiB"
    );
}
