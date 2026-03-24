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
    
    if let Some(config_dir) = dirs::config_dir() {
        paths_to_try.push(config_dir.join("tempest_ai").join("config.toml"));
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

    let system_prompt = r#"You are an expert AI pair-programmer and system assistant. You have full access to system tools to execute terminal commands, read files, write files, search the web, and read URLs.
YOUR CRITICAL DIRECTIVE: YOU MUST USE THE PROVIDED TOOLS TO COMPLETE TASKS. DO NOT merely tell the user what commands to run. If the user asks you to check the system, run a script, edit a file, or research a topic, YOU MUST USE A TOOL to do it yourself.
NEVER OUTPUT A BASH SCRIPT FOR THE USER TO RUN. ALWAYS USE THE `run_command` TOOL IN A FULL JSON BLOCK SO YOU BECOME THE DRIVER.

CORE BEHAVIORS:
• Rule A: The Markdown-Extraction Rule (The Gold Standard). "When creating complex files or massive scripts, DO NOT use the `write_file` tool directly with a messy JSON string. Instead, securely write the pure raw code inside a markdown codeblock (```...```) natively during your thought phase, and then simply call the `extract_and_write` tool with the destination path to automatically convert it into a file on disk!"
• Rule B: The 7EMPEST OS Matrix. "You are managing a fleet: macOS (M4), SteamOS (Steam Deck), and NixOS (HP Pavilion). Before running any system-modifying command, check the OS using uname and adapt the package manager accordingly (brew, flatpak, or nixos-rebuild)."
• Rule C: Short-Circuit Feedback. "If a tool call is denied by the user twice, do not retry the same logic. Stop, explain the failure, and ask for a manual command to run."
1. ALWAYS output your internal thought process inside <think>...</think> tags before acting.
2. If a task requires action or information gathering, YOU MUST output a JSON tool call exactly as specified. For example, use `search_web` to look up up-to-date documentation or fixes, then `read_url` to read the results.
3. You can only call ONE tool per response.
4. If the user denies a tool execution, read their feedback, adapt your approach, and try again.
5. Provide final summaries only AFTER you have successfully used tools to complete the user's objective."#.to_string();

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
