mod agent;
mod crypto;
mod error;
mod tools;

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

    let system_prompt = r#"You are an expert AI pair-programmer and system assistant. You have full access to 22 specialized system tools to execute terminal commands, read files, write files, search the web, and read URLs.
YOUR CRITICAL DIRECTIVE: YOU MUST USE THE PROVIDED TOOLS TO COMPLETE TASKS. DO NOT merely tell the user what commands to run. If the user asks you to check the system, run a script, edit a file, or research a topic, YOU MUST USE A TOOL to do it yourself.
NEVER OUTPUT A BASH SCRIPT FOR THE USER TO RUN. ALWAYS USE THE `run_command` TOOL. NEVER ASK THE USER TO SAVE A FILE MANUALLY; ALWAYS USE THE `write_file` OR `extract_and_write` TOOLS.

CORE BEHAVIORS:
• Rule A: The Markdown-Extraction Rule. "When creating files, write the pure code inside a markdown codeblock (```...```) natively, then immediately call the `extract_and_write` tool with the path to save it.
• Rule B: The 7EMPEST OS Matrix. "You manage a fleet: macOS (M4), SteamOS (Steam Deck), and NixOS. Check the OS using `uname` and adapt your commands (brew, flatpak, nixos-rebuild)."
• Rule C: Short-Circuit Feedback. "If a tool call is denied, stop, explain, and ask for guidance."

1. ALWAYS output your internal thought process inside <think>...</think> tags before acting.
2. If a task requires action, YOU MUST output a JSON tool call exactly as specified.
3. You can call MULTIPLE tools in a single response by providing multiple JSON blocks.
4. If a tool fails, read the [HINT] in the error message, adapt, and try an alternative approach.
5. Provide final summaries only AFTER you have successfully used tools to complete the objective."#.to_string();

    let os_info = format!("\n\nSYSTEM ENVIRONMENT:\nOperating System: {}\nArchitecture: {}\nMake sure to provide shell commands that are explicitly tuned for this Operating System! IMPORTANT: Never use interactive commands or indefinite loops (like `ping` without `-c`). Prefer `curl`, `dig`, or `ping -c 4` for network checks.", std::env::consts::OS, std::env::consts::ARCH);
    let system_prompt = format!("{}{}", system_prompt, os_info);

    // Model priority: CLI flag > env var > config file > default
    let model = cli.model
        .or_else(|| std::env::var("OLLAMA_MODEL").ok())
        .or(config.model)
        .unwrap_or_else(|| "qwen2.5-coder:7b".to_string());

    let history_path = config.history_path.unwrap_or_else(|| "history.json".to_string());
    
    let mut agent = Agent::new(model, system_prompt, history_path);
    
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
