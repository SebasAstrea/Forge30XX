//! RF-08: porcentaje de tool-calls parseables con la gramática GBNF.
//!
//! Requiere un llama-server real (no se mockea el modelo, AGENTS.md 11.11):
//!
//! ```text
//! FORGE_TEST_LLAMA_URL=http://127.0.0.1:8080 cargo test --test rf08_tool_calls -- --ignored --nocapture
//! ```
//!
//! Umbral: ≥90% (perfil A). Para perfil B usar `FORGE_TEST_MIN_RATE=0.95`.

use std::time::Duration;

use forge::inference::{InferenceClient, Message, SamplingParams};
use forge::tool_call::ToolCall;

const SYSTEM: &str = "Eres un agente de codificación. Responde SOLO con un tool-call JSON \
{\"name\": <tool>, \"arguments\": {...}}. Tools: read_file(path), \
edit_file(path, old, new), create_file(path, content), delete_file(path), \
search_web(query), run_shell(command).";

const PROMPTS: [&str; 30] = [
    "Lee src/main.rs",
    "Muéstrame el contenido de Cargo.toml",
    "Abre README.md",
    "Lee el archivo python/forge_rag/client.py",
    "¿Qué hay en docs/SDD.md?",
    "En src/lib.rs cambia `pub mod rag;` por `pub mod rag2;`",
    "Renombra la función foo a bar en src/util.rs",
    "En config.toml cambia port = 8080 por port = 9000",
    "Sustituye println! por tracing::info! en src/main.rs",
    "Corrige el typo 'recieve' por 'receive' en docs/api.md",
    "Crea tests/test_sum.py con una función test_sum que compruebe 1+1==2",
    "Crea un .gitignore que ignore target/",
    "Crea src/math.rs con una función add(a: i32, b: i32) -> i32",
    "Crea notes.txt con el texto 'pendiente: S2'",
    "Crea frontend/src/lib/api.ts que exporte const BASE = '/api'",
    "Borra el archivo tmp/output.log",
    "Elimina old_config.toml",
    "Borra build/cache.bin",
    "Busca en la web la documentación de tokio::select!",
    "Busca cómo usar reqwest con streaming",
    "Busca la sintaxis de GBNF de llama.cpp",
    "Busca qué es un mutex en Rust",
    "Ejecuta cargo test",
    "Lista los archivos del directorio actual",
    "Ejecuta git status",
    "Corre npm run lint en frontend",
    "Ejecuta python -m pytest -q",
    "Lee el archivo con espacios 'docs/mi archivo.md'",
    "Crea data.json con {\"k\": [1, 2, 3]}",
    "Busca \"error E0599\" de rustc",
];

fn grammar() -> String {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../grammars/tool_calls.gbnf");
    std::fs::read_to_string(path).unwrap_or_else(|e| panic!("leer {path}: {e}"))
}

async fn parse_rate(client: &InferenceClient, params: &SamplingParams) -> f64 {
    let mut ok = 0usize;
    for prompt in PROMPTS {
        let msgs = [Message::system(SYSTEM), Message::user(prompt)];
        let reply = client.chat(&msgs, params).await.expect("inferencia");
        match ToolCall::parse(&reply) {
            Ok(_) => ok += 1,
            Err(e) => println!("  inválido ({e}): {prompt} -> {reply:?}"),
        }
    }
    ok as f64 / PROMPTS.len() as f64
}

#[tokio::test]
#[ignore = "requiere llama-server real (FORGE_TEST_LLAMA_URL)"]
async fn rf08_grammar_meets_parse_threshold() {
    let url = std::env::var("FORGE_TEST_LLAMA_URL").unwrap_or("http://127.0.0.1:8080".into());
    let min_rate: f64 = std::env::var("FORGE_TEST_MIN_RATE")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(0.90);
    let client = InferenceClient::new(url, Duration::from_secs(120)).expect("cliente");
    let mut params = SamplingParams {
        max_tokens: 256,
        ..SamplingParams::default()
    };

    let baseline = parse_rate(&client, &params).await;
    params.grammar = Some(grammar());
    let constrained = parse_rate(&client, &params).await;

    println!(
        "RF-08 sobre {} prompts: sin gramática {:.0}%, con gramática {:.0}% (umbral {:.0}%)",
        PROMPTS.len(),
        baseline * 100.0,
        constrained * 100.0,
        min_rate * 100.0
    );
    assert!(constrained >= min_rate);
}
