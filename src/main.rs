mod agent;
mod crypto;
mod error;
mod memory;
mod tools;
mod hardware;

use agent::Agent;
use anyhow::Result;
use clap::Parser;
use colored::*;

/// Tempest AI — An autonomous AI pair-programmer and system assistant
#[derive(Parser, Debug)]
#[command(name = "tempest_ai", version, about)]
struct Cli {
    /// Ollama model to use (overrides config and OLLAMA_MODEL env var)
    #[arg(short, long)]
    model: Option<String>,

    /// Disable colored output
    #[arg(long)]
    no_color: bool,

    /// Path to a TOML config file
    #[arg(short, long)]
    config: Option<String>,
}

#[allow(dead_code)]
#[derive(serde::Deserialize, Debug)]
struct AppConfig {
    model: Option<String>,
    history_path: Option<String>,
    db_path: Option<String>,
    encrypt_history: Option<bool>,
}

impl Default for AppConfig {
    fn default() -> Self {
        AppConfig {
            model: Some("qwen2.5-coder:7b".to_string()),
            history_path: Some("history.json".to_string()),
            db_path: Some("~/fleet.db".to_string()),
            encrypt_history: Some(false),
        }
    }
}

fn load_config(cli_config_path: Option<&str>) -> AppConfig {
    // Priority: CLI --config > ~/.config/tempest_ai/config.toml > defaults
    let mut paths_to_try: Vec<std::path::PathBuf> = vec![];
    
    if let Some(p) = cli_config_path {
        paths_to_try.push(std::path::PathBuf::from(p));
    }
    
    // Check platform-standard config dir (~/Library/Application Support on macOS)
    if let Some(config_dir) = dirs::config_dir() {
        paths_to_try.push(config_dir.join("tempest_ai").join("config.toml"));
    }
    // Also check ~/.config (XDG convention, common on macOS too)
    if let Some(home) = dirs::home_dir() {
        paths_to_try.push(home.join(".config").join("tempest_ai").join("config.toml"));
    }

    for path in &paths_to_try {
        if path.exists() {
            if let Ok(content) = std::fs::read_to_string(path) {
                if let Ok(config) = toml::from_str::<AppConfig>(&content) {
                    println!("{} Loaded config from: {}", "⚙️".blue(), path.display());
                    return config;
                }
            }
        }
    }

    AppConfig::default()
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    if cli.no_color {
        colored::control::set_override(false);
    }

    // Initialize structured logging
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("warn"))
        )
        .with_target(false)
        .compact()
        .init();

    let config = load_config(cli.config.as_deref());

    let system_prompt = format!(r#"You are Tempest AI, an autonomous assistant running on {os}/{arch}. You have direct access to tools. 
YOU MUST USE TOOLS TO COMPLETE TASKS. Never tell the user to run commands themselves. Never output code without saving it via a tool.

RULES:
- Think first using <think>...</think> tags, then act with tool calls.  
- Rule A: To save code files or scripts, write the code in a ```language block, then call `extract_and_write` with ONLY the path. 
- NEVER use shell commands like `cat <<EOF` or `echo >` inside a tool's JSON arguments. This will fail.
- NEVER use placeholders like `{{ output }}` expecting dynamic replacement. You must wait for the actual tool completion to read its data.
- You may output ```sh blocks as suggestions WITHOUT a tool call. These are informational only.

TOOLS (call via JSON):
{{tool_descriptions}}

FORMAT: Output a JSON block to call a tool:
```json
{{ "tool": "tool_name", "arguments": {{}} }}
```
"#, os = std::env::consts::OS, arch = std::env::consts::ARCH).to_string();


    use sysinfo::System;
    let mut sys = System::new_all();
    sys.refresh_all();
    let used_mem = sys.used_memory() as f64 / sys.total_memory() as f64;
    if used_mem > 0.90 {
        println!("\n{}", "⚠️  WARNING: System Memory is critically low (>90% full).".yellow().bold());
        println!("{}", "Reasoning models (DeepSeek-R1) may hang or respond very slowly in this state.".yellow());
        println!("{}\n", "HINT: Close heavy apps (Chrome, Xcode) or switch to a smaller model (phi4-mini).".dimmed());
    }

    // Model priority: CLI flag > env var > config file > default
    let model = cli.model
        .or_else(|| std::env::var("OLLAMA_MODEL").ok())
        .or(config.model)
        .unwrap_or_else(|| "qwen2.5-coder:7b".to_string());

    let history_path = config.history_path.unwrap_or_else(|| "history.json".to_string());
    
    let mut config_dir = dirs::config_dir().unwrap_or_else(|| std::path::PathBuf::from("."));
    config_dir.push("tempest_ai");
    let key_path = config_dir.join("master.key");
    
    let passphrase = if key_path.exists() {
        std::fs::read_to_string(&key_path).unwrap_or_else(|_| "fallback_key".to_string())
    } else {
        let new_key = uuid::Uuid::new_v4().to_string() + &uuid::Uuid::new_v4().to_string();
        let _ = std::fs::create_dir_all(&config_dir);
        let _ = std::fs::write(&key_path, &new_key);
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            if let Ok(mut perms) = std::fs::metadata(&key_path).map(|m| m.permissions()) {
                perms.set_mode(0o600);
                let _ = std::fs::set_permissions(&key_path, perms);
            }
        }
        new_key
    };

    let memory_store = std::sync::Arc::new(std::sync::Mutex::new(crate::memory::MemoryStore::new(passphrase).expect("Failed to initialize SQLite Memory Store")));

    let mut agent = Agent::new(model, system_prompt, history_path, memory_store.clone());
    
    if let Err(e) = agent.check_connection().await {
        println!("{}", format!("Agent Error: {}", e).red());
        std::process::exit(1);
    }
    
    let _ = agent.load_history();
    
    let mut rl = rustyline::DefaultEditor::new()?;
    loop {
        let readline = rl.readline(">> ");
        match readline {
            Ok(line) => {
                let input = line.trim();
                if input == "exit" || input == "quit" {
                    break;
                }
                if input.is_empty() {
                    continue;
                }
                if input == "help" {
                    println!("{}", "═".repeat(60).blue());
                    println!("{}", "🌪️  TEMPEST AI — Quick Reference".green().bold());
                    println!("{}", "═".repeat(60).blue());
                    println!("{}", "\n📦 BUILT-IN TOOLS:".yellow().bold());
                    println!("  run_command      — Execute bash/zsh commands");
                    println!("  read_file        — Read file contents");
                    println!("  write_file       — Write/create files");
                    println!("  patch_file       — Surgically edit files");
                    println!("  extract_and_write— Write code from markdown blocks");
                    println!("  list_dir         — List directory contents");
                    println!("  search_dir       — Ripgrep search in directories");
                    println!("  search_web       — DuckDuckGo web search");
                    println!("  read_url         — Fetch and read web pages");
                    println!("  run_background   — Spawn long-running processes");
                    println!("  read_process_logs— Check background process output");
                    println!("  system_info      — Hardware diagnostics (CPU/RAM/OS)");
                    println!("  sqlite_query     — Direct SQLite database access");
                    println!("  git_action       — Native Git operations");
                    println!("  watch_directory  — File-watching daemon");
                    println!("  ask_user         — Ask for human input");
                    println!("  http_request     — REST API calls (GET/POST/PUT/DELETE)");
                    println!("  clipboard        — Read/write system clipboard");
                    println!("  notify           — macOS desktop notifications");
                    println!("  find_replace     — Regex find-and-replace across files");
                    println!("  tree             — Recursive directory tree view");
                    println!("  network_check    — Safe ping/DNS/port check");
                    println!("{}", "\n⌨️  SHELL COMMANDS:".yellow().bold());
                    println!("  help             — Show this reference card");
                    println!("  clear            — Clear conversation history");
                    println!("  exit / quit      — Exit Tempest AI");
                    println!("{}", "\n🚀 CLI FLAGS:".yellow().bold());
                    println!("  --model <name>   — Swap Ollama model");
                    println!("  --no-color       — Disable colored output");
                    println!("  --config <path>  — Custom TOML config file");
                    println!("{}", "\n📁 CONFIG:".yellow().bold());
                    println!("  ~/.config/tempest_ai/config.toml");
                    println!("{}", "═".repeat(60).blue());
                    continue;
                }
                if input == "clear" {
                    agent.clear_history();
                    println!("{}", "🧹 Conversation history cleared.".green());
                    continue;
                }
                let _ = rl.add_history_entry(input);
                if let Err(e) = agent.run(input.to_string()).await {
                    println!("{}", format!("Agent Error: {}", e).red());
                }
            },
            Err(_) => break,
        }
    }
    
    Ok(())
}
