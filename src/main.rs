mod agent;
mod crypto;
mod error;
mod memory;
mod tools;
mod hardware;
mod telemetry;
mod daemon;
mod tui;
mod vector_brain;
mod skills;

use agent::Agent;
use clap::Parser;
use colored::*;
use miette::{Result, IntoDiagnostic};
use parking_lot::Mutex;
use std::sync::Arc;

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
    #[arg(short = 'C', long)]
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
    pub sub_agent_model: Option<String>,
}

impl Default for AppConfig {
    fn default() -> Self {
        AppConfig {
            model: Some("qwen2.5-coder:7b".to_string()),
            history_path: Some("history.json".to_string()),
            db_path: Some("~/fleet.db".to_string()),
            encrypt_history: Some(false),
            sub_agent_model: Some("phi3:latest".to_string()),
        }
    }
}

fn load_config(cli_config_path: Option<&str>, tui_mode: bool) -> AppConfig {
    let mut paths_to_try: Vec<std::path::PathBuf> = vec![];
    
    if let Some(p) = cli_config_path {
        paths_to_try.push(std::path::PathBuf::from(p));
    }
    
    if let Ok(sudo_user) = std::env::var("SUDO_USER") {
        if !sudo_user.is_empty() && sudo_user != "root" {
            #[cfg(unix)]
            {
                let prefixes = ["/home", "/Users"];
                for prefix in prefixes {
                    let p = std::path::PathBuf::from(prefix)
                        .join(&sudo_user)
                        .join(".config")
                        .join("tempest_ai")
                        .join("config.toml");
                    paths_to_try.push(p);
                }
            }
        }
    }

    if let Some(config_dir) = dirs::config_dir() {
        paths_to_try.push(config_dir.join("tempest_ai").join("config.toml"));
    }
    if let Some(home) = dirs::home_dir() {
        paths_to_try.push(home.join(".config").join("tempest_ai").join("config.toml"));
    }

    for path in &paths_to_try {
        if path.exists() {
            if let Ok(content) = std::fs::read_to_string(path) {
                if let Ok(config) = toml::from_str::<AppConfig>(&content) {
                    if !tui_mode {
                        println!("{} Loaded config from: {}", "⚙️".blue(), path.display());
                    }
                    return config;
                }
            }
        }
    }

    if !tui_mode {
        println!("{} No valid config found. Using default settings.", "ℹ️".dimmed());
    }
    AppConfig::default()
}

use std::net::SocketAddr;
use metrics_exporter_prometheus::PrometheusBuilder;

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize Prometheus metrics exporter on port 7777
    let addr: SocketAddr = "0.0.0.0:7777".parse().expect("Invalid metrics address");
    PrometheusBuilder::new()
        .with_http_listener(addr)
        .install()
        .expect("failed to install Prometheus recorder");

    // Install error handlers
    color_eyre::install().map_err(|e| miette::miette!("Failed to install color-eyre: {}", e))?;

    let cli = Cli::parse();

    // Initialize tracing for performance monitoring
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

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

    let config = load_config(cli.config.as_deref(), !cli.cli);


    let system_prompt = r#"You are Tempest AI — a disciplined, production-grade Principal Engineer running inside a real TUI environment.

You follow a strict engineering workflow and never deviate from it.

### CORE RULES (Never break these)
1. You are TOOL-DRIVEN. Never claim you performed an action unless you receive an explicit TOOL RESULT.
2. You are in one of two modes at all times:
   - PLANNING MODE (default): You may only read, analyze, ask questions, or use no_op. You MUST NOT use any modifying tools.
   - EXECUTION MODE: You may use modifying tools only after the user has explicitly toggled planning mode off.
3. If you are in PLANNING MODE and the user asks you to do something that modifies the system, your response must be:
   - Use the ask_user tool to clarify, or
   - Use the toggle_planning tool to request permission to enter execution mode.
4. Never hallucinate tool calls. Only use tools that are explicitly listed in the [TOOL SCHEMA] section below.
5. If you are unsure, confused, or need clarification, use the ask_user tool immediately. Do not guess.

### RESPONSE FORMAT (Follow exactly)
Every response must contain exactly one of these structures:

**In Planning Mode:**
THOUGHT: [Your reasoning]
PLAN: [Numbered list of next steps]
NEXT: [Either a tool call or a clear question to the user]

**In Execution Mode:**
THOUGHT: [Your reasoning]
ACTION: [The single tool call you are making]
NEXT: [What you will do after receiving the result]

### AVAILABLE TOOLS
{{tool_descriptions}}

You have a powerful set of tools including file operations, git, execution, telemetry, web search, memory, and more. Use them responsibly and only when appropriate.

Never invent tool names. If you need a capability that isn't listed, ask the user instead of hallucinating.

You are running on a real machine with real consequences. Be precise, safe, and professional.
"#.to_string();

    // Model priority: CLI flag > env var > config file > default
    let model = cli.model
        .or_else(|| std::env::var("OLLAMA_MODEL").ok())
        .or(config.model.clone())
        .unwrap_or_else(|| "qwen2.5-coder:7b".to_string());

    let mut config_dir = dirs::config_dir().unwrap_or_else(|| std::path::PathBuf::from("."));
    config_dir.push("tempest_ai");
    let _ = std::fs::create_dir_all(&config_dir);

    let history_raw = config.history_path
        .unwrap_or_else(|| "history.json".to_string());
    
    let history_path = if std::path::Path::new(&history_raw).is_absolute() {
        history_raw
    } else {
        config_dir.join(&history_raw).to_string_lossy().to_string()
    };
    
    let key_path = config_dir.join("master.key");
    
    let passphrase = if key_path.exists() {
        std::fs::read_to_string(&key_path).unwrap_or_else(|_| "fallback_key".to_string())
    } else {
        let new_key = uuid::Uuid::new_v4().to_string() + &uuid::Uuid::new_v4().to_string();
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

    let memory_store = Arc::new(Mutex::new(crate::memory::MemoryStore::new(passphrase).expect("Failed to initialize SQLite Memory Store")));
    let sub_agent_model = config.sub_agent_model.unwrap_or_else(|| "phi3:latest".to_string());
    let agent = Agent::new(model, system_prompt, history_path, memory_store, sub_agent_model);
    
    if let Err(e) = agent.check_connection().await {
        if !cli.cli {
            // In TUI mode, we might want to just exit or show an error later?
            // But since TUI hasn't started, plain print is ok, 
            // but we can make it cleaner.
        }
        println!("{}", format!("Agent Error: {}", e).red());
        std::process::exit(1);
    }
    
    let _ = agent.load_history();
    let _ = agent.initialize_atlas(false).await;

    if cli.cli {
        run_cli_mode(agent).await?;
        return Ok(());
    }
    
    // Default to TUI mode
    let (user_tx, user_rx) = tokio::sync::mpsc::channel(32);
    let (agent_tx, agent_rx) = tokio::sync::mpsc::channel(100);
    let (tool_tx, tool_rx) = tokio::sync::mpsc::channel::<crate::tui::ToolResponse>(1);

    let stop_flag = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let stop_flag_agent = stop_flag.clone();

    let agent_tx_metrics = agent_tx.clone();
    let shared_telemetry = agent.telemetry.clone();
    tokio::spawn(async move {
        use sysinfo::{System, Networks, Components};
        let mut sys = System::new_all();
        let mut networks = Networks::new_with_refreshed_list();
        let mut components = Components::new_with_refreshed_list();
        
        #[cfg(target_os = "macos")]
        let gpu_re = regex::Regex::new(r#""Device Utilization %"=(\d+)"#).unwrap();

        loop {
            sys.refresh_all();
            networks.refresh(true);
            components.refresh(true);
            
            let mut ollama_mem_bytes = 0;
            for process in sys.processes().values() {
                let name = process.name().to_string_lossy().to_lowercase();
                if name.contains("ollama") || name.contains("llama") {
                    ollama_mem_bytes = std::cmp::max(ollama_mem_bytes, process.memory());
                }
            }
            let ollama_mb = ollama_mem_bytes / 1024 / 1024;

            let gpu_load = {
                #[cfg(target_os = "macos")]
                {
                    let mut current_load = 0;
                    let output = std::process::Command::new("ioreg")
                        .args(["-r", "-c", "AGXAccelerator"])
                        .output()
                        .ok();
                    if let Some(out) = output {
                        let s = String::from_utf8_lossy(&out.stdout);
                        if let Some(caps) = gpu_re.captures(&s) {
                            if let Some(m) = caps.get(1) {
                                current_load = m.as_str().parse::<i32>().unwrap_or(0);
                            }
                        }
                    }
                    current_load
                }
                #[cfg(target_os = "linux")]
                {
                    crate::hardware::get_linux_gpu_usage()
                }
                #[cfg(not(any(target_os = "macos", target_os = "linux")))]
                {
                    0
                }
            };
            
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
                "🔥 CPU LOAD      : {:.1}% ({} Cores)\n\n🚀 MEMORY ALLOC  : {}/{} MB ({:.1}%)\n\n🤖 AI RAM USE    : {} MB (Ollama)\n\n🎨 GPU LOAD      : {}% (Graphics)\n\n💾 SWAP CACHE    : {}/{} MB ({:.1}%)\n\n----------------------------------\n\n🌐 TRUNK [en0]   : {} B ▼ | {} B ▲\n\n🌡️ AVG THERMALS  : {:.1} °C (Max: {:.1} °C)\n\n⚙️ ACTIVE PROCS  : {}\n\n⏱️ CORE UPTIME   : {}h {}m {}s\n\n----------------------------------\n\n[ Live Topology Sweep: Active ]",
                avg_cpu, cpus.len(), used_mb, total_mb, mem_perc, ollama_mb, gpu_load, used_swap, total_swap, swap_perc,
                total_rx, total_tx, avg_temp, max_temp, proc_count, hours, minutes, secs
            );
            
            let _ = agent_tx_metrics.send(crate::tui::AgentEvent::SystemUpdate(update_str.clone())).await;
            {
                let mut lock = shared_telemetry.lock();
                *lock = update_str;
            }
            tokio::time::sleep(std::time::Duration::from_secs(1)).await;
        }
    });

    let agent_tx_agent = agent_tx.clone();
    *agent.event_tx.lock() = Some(agent_tx.clone());
    
    tokio::spawn(async move {
        if let Err(e) = agent.run_tui_mode(user_rx, agent_tx_agent, tool_rx, stop_flag_agent).await {
            let _ = agent_tx.send(crate::tui::AgentEvent::SystemUpdate(format!("Agent crashed: {}", e))).await;
        }
    });

    if let Err(e) = crate::tui::run_tui(agent_rx, user_tx, tool_tx).await {
        println!("{}", format!("TUI Render Error: {}", e).red());
    }
    
    Ok(())
}

async fn run_cli_mode(agent: Agent) -> Result<()> {
    use rustyline::DefaultEditor;
    let mut rl = DefaultEditor::new().into_diagnostic()?;
    
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
