mod agent;
mod crypto;
mod error;
mod memory;
mod tools;
mod hardware;
mod telemetry;
mod daemon;
mod tui;

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

    /// Run the Tempest Headless Background Watcher
    #[arg(long)]
    daemon: bool,

    /// Install the Tempest Daemon to run automatically on system boot (requires Root)
    #[arg(long)]
    install_daemon: bool,

    /// Run the original Tempest CLI (Command Line Interface) mode
    #[arg(short = 'c', long)]
    cli: bool,
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

    if cli.install_daemon {
        crate::daemon::install_daemon();
        return Ok(());
    }

    if cli.daemon {
        crate::daemon::run_daemon().await;
        return Ok(());
    }

    if cli.no_color {
        colored::control::set_override(false);
    }

    let config = load_config(cli.config.as_deref());

    let current_user = std::env::var("USER").unwrap_or_else(|_| "unknown_user".to_string());
    let home_dir = dirs::home_dir().map(|p| p.to_string_lossy().to_string()).unwrap_or_else(|| "/".to_string());
    let cwd = std::env::current_dir().map(|p| p.to_string_lossy().to_string()).unwrap_or_else(|_| ".".to_string());

    let system_prompt = format!(r#"You are Tempest AI, an autonomous assistant running on {os}/{arch}. You have direct access to tools. 
YOU MUST USE TOOLS TO COMPLETE TASKS. Never tell the user to run commands themselves. Never output code without saving it via a tool.

[ENVIRONMENT]:
- Current User: {user}
- Home Directory: {home}
- Current Working Directory: {cwd}

RULES:
- Think first using <think>...</think> tags, then act with tool calls.  
- Rule A: To save code files or scripts, write the code in a ```language block, then call `extract_and_write` with ONLY the path. 
- NEVER use shell commands like `cat <<EOF` or `echo >` inside a tool's JSON arguments. This will fail.
- NEVER use placeholders like `{{ output }}` expecting dynamic replacement. You must wait for the actual tool completion to read its data.
- You may output ```sh blocks as suggestions WITHOUT a tool call. These are informational only.

[COGNITIVE LIFECYCLE]:
You operate in two modes: PLANNING and EXECUTING.

1. PLANNING MODE (DEFAULT): You start every session in this mode.
   - Use read-only tools (read_file, list_dir, search_dir, search_web, system_info) to research.
   - Present your implementation plan to the user as a numbered list.
   - Once the user approves, you MUST call `toggle_planning` with mode="off" to enter execution mode.

2. EXECUTING MODE: Full tool access.
   - After every file write or command, you MUST verify your work (read_file, cat, compile).
   - If you need to stop and research a new approach, call `toggle_planning` with mode="on" to re-lock the system.

For simple, single-step tasks (e.g. "make a test file", "what time is it"), skip planning and execute directly.

TOOLS (call via JSON):
{{tool_descriptions}}

FORMAT: Output a JSON block to call a tool:
```json
{{ "tool": "tool_name", "arguments": {{}} }}
```
"#, 
    os = std::env::consts::OS, 
    arch = std::env::consts::ARCH,
    user = current_user,
    home = home_dir,
    cwd = cwd).to_string();


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

    if cli.cli {
        run_cli_mode(agent).await?;
        return Ok(());
    }
    
    // Default to TUI mode
    let (user_tx, user_rx) = tokio::sync::mpsc::channel(32);
    let (agent_tx, agent_rx) = tokio::sync::mpsc::channel(100);
    let (tool_tx, tool_rx) = tokio::sync::mpsc::channel::<crate::tui::ToolResponse>(1);

    let app = crate::tui::App::new();

    let stop_flag = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let stop_flag_agent = stop_flag.clone();
    let stop_flag_tui = stop_flag.clone();

    let agent_tx_metrics = agent_tx.clone();
    tokio::spawn(async move {
        use sysinfo::{System, Networks, Components};
        let mut sys = System::new_all();
        let mut networks = Networks::new_with_refreshed_list();
        let mut components = Components::new_with_refreshed_list();
        
        loop {
            sys.refresh_all();
            networks.refresh(true);
            components.refresh(true);
            
            let cpus = sys.cpus();
            let mut total_cpu = 0.0;
            for cpu in cpus { total_cpu += cpu.cpu_usage(); }
            let avg_cpu = if !cpus.is_empty() { total_cpu / cpus.len() as f32 } else { 0.0 };
            
            let used_mb = sys.used_memory() / 1024 / 1024;
            let total_mb = sys.total_memory() / 1024 / 1024;
            let mem_perc = if total_mb > 0 { (used_mb as f32 / total_mb as f32) * 100.0 } else { 0.0 };

            let used_swap = sys.used_swap() / 1024 / 1024;
            let total_swap = sys.total_swap() / 1024 / 1024;
            let swap_perc = if total_swap > 0 { (used_swap as f32 / total_swap as f32) * 100.0 } else { 0.0 };
            
            let mut total_rx = 0;
            let mut total_tx = 0;
            for (interface_name, data) in &networks {
                if interface_name == "en0" || interface_name.starts_with("eth") || interface_name.starts_with("wlan") {
                    total_rx += data.received();
                    total_tx += data.transmitted();
                }
            }
            
            let mut max_temp = 0.0;
            let mut sum_temp = 0.0;
            let mut count_temp = 0;
            for comp in &components {
                if let Some(temp) = comp.temperature() {
                    if temp > 0.0 {
                        if temp > max_temp { max_temp = temp; }
                        sum_temp += temp;
                        count_temp += 1;
                    }
                }
            }
            let avg_temp = if count_temp > 0 { sum_temp / count_temp as f32 } else { 0.0 };
            
            let uptime = System::uptime();
            let hours = uptime / 3600;
            let minutes = (uptime % 3600) / 60;
            let secs = uptime % 60;
            
            let proc_count = sys.processes().len();
            
            let update_str = format!(
                "🔥 CPU LOAD      : {:.1}% ({} Cores)\n\n🚀 MEMORY ALLOC  : {}/{} MB ({:.1}%)\n\n💾 SWAP CACHE    : {}/{} MB ({:.1}%)\n\n----------------------------------\n\n🌐 TRUNK [en0]   : {} B ▼ | {} B ▲\n\n🌡️ AVG THERMALS  : {:.1} °C (Max: {:.1} °C)\n\n⚙️ ACTIVE PROCS  : {}\n\n⏱️ CORE UPTIME   : {}h {}m {}s\n\n----------------------------------\n\n[ Live Topology Sweep: Active ]",
                avg_cpu, cpus.len(), used_mb, total_mb, mem_perc, used_swap, total_swap, swap_perc,
                total_rx, total_tx, avg_temp, max_temp, proc_count, hours, minutes, secs
            );
            
            let _ = agent_tx_metrics.send(crate::tui::AgentEvent::SystemUpdate(update_str)).await;
            tokio::time::sleep(std::time::Duration::from_secs(1)).await;
        }
    });

    let agent_tx_agent = agent_tx.clone();
    tokio::spawn(async move {
        if let Err(e) = agent.run_tui_mode(user_rx, agent_tx_agent, tool_rx, stop_flag_agent).await {
            let _ = agent_tx.send(crate::tui::AgentEvent::SystemUpdate(format!("Agent crashed: {}", e))).await;
        }
    });

    if let Err(e) = crate::tui::run_tui(app, agent_rx, user_tx, tool_tx, stop_flag_tui).await {
        println!("{}", format!("TUI Render Error: {}", e).red());
    }
    
    Ok(())
}

async fn run_cli_mode(mut agent: Agent) -> Result<()> {
    use rustyline::DefaultEditor;
    let mut rl = DefaultEditor::new()?;
    
    println!("{}", "=".repeat(60).blue());
    println!("{} {} Mode: ON", "🚀".green(), "Tempest Command".bold());
    println!("Type /help for internal commands or /quit to exit.");
    println!("{}", "=".repeat(60).blue());

    loop {
        let readline = rl.readline(">> ");
        match readline {
            Ok(line) => {
                let p = line.trim();
                if p.is_empty() { continue; }
                let _ = rl.add_history_entry(p);
                
                if p == "/quit" || p == "/exit" { break; }
                if p == "/clear" {
                    agent.clear_history();
                    println!("{} History wiped.", "🧹".yellow());
                    continue;
                }
                
                if let Err(e) = agent.run(p.to_string()).await {
                    println!("{} {}", "❌ Error:".red().bold(), e);
                }
            }
            Err(rustyline::error::ReadlineError::Interrupted) | Err(rustyline::error::ReadlineError::Eof) => break,
            Err(err) => {
                println!("Error: {:?}", err);
                break;
            }
        }
    }
    Ok(())
}
