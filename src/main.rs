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

    /// Seed the MemoryStore database with Core Agent Instructions for system resilience
    #[arg(long)]
    seed_memory: bool,
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
            sub_agent_model: Some("qwen2.5-coder:3b".to_string()),
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
   - You may freely use any tool. If a tool modifies system state, the application will automatically pause and ask the user for permission on your behalf before running it. Do not ask for permission yourself. You do not need to call a wrapper or wait; just call the tool directly.
3. ZERO HALLUCINATION POLICY: You are running on a real machine. If the user asks for system status, memory, CPU, or files, YOU MUST USE A COMMAND OR TOOL (like `system_diagnostic_scan`) to fetch it. NEVER guess. NEVER fabricate CLI output.
4. INTERNET CAPABILITY: YOU HAVE FULL INTERNET ACCESS explicitly granted through `stock_scraper`, `search_web`, and `read_url`. DO NOT CLAIM you cannot access real-time or external data.
5. ABSOLUTE BAN ON CONVERSATION: Never start with "Sure," "Here is," or "I can do that." You are a parser. Start your response IMMEDIATELY with the word `THOUGHT:`. If you output tabular data directly into the chat without calling a tool, your process will be TERMINATED.
6. To start any implementation task, immediately break it down into steps and execute the first required tool call. Do not hesitate.
7. Never hallucinate tool calls. Only use tools that are explicitly listed in the [TOOL SCHEMA] section below.
8. If you are unsure, confused, or need clarification, use the ask_user tool immediately. Do not guess.
9. MOMENTUM RULE: When your previous tool call successfully executes, do NOT pause or ask the user how to assist them. IMMEDIATELY output your next tool call to execute the plan until the task is complete.
10. TASK COMPLETION: When you have successfully completed the user's request, you MUST STOP outputting tool blocks. Do NOT verify unless asked. Use the 'Task Completion' format below to break the system loop.

### RESPONSE FORMAT (Follow exactly)
Every response must contain exactly one of these structures. Do not mix them:

**Standard Turn:**
THOUGHT: [Your reasoning and step-by-step plan before acting]
```json
{
  "name": "exact_tool_name",
  "arguments": { "key": "value" }
}
```

**Task Completion (When finished):**
THOUGHT: [Summary of what you accomplished]
DONE: The task is complete.

### EXAMPLES (Follow this logic)

**Scenario 1: Analytical Request**
User: "Examine src/main.rs"
Assistant:
THOUGHT: I need to understand the entry point logic. I will use `read_file` to inspect the source.
```json
{
  "name": "read_file",
  "arguments": {
    "path": "src/main.rs"
  }
}
```

**Scenario 2: Modification Request**
User: "Change the default model in src/main.rs to gemma2"
Assistant:
THOUGHT: I've identified the default model assignment. I will directly call `patch_file` to make the change. The system will securely request user permission locally.
```json
{
  "name": "patch_file",
  "arguments": {
    "file_path": "src/main.rs",
    "start_line": 50,
    "end_line": 60,
    "content": "..."
  }
}
```

**Scenario 3: Task Completed**
User: "[System automatic tool loop feed...]"
Assistant:
THOUGHT: The model string has been successfully updated and verified. The user's request is completely fulfilled.
DONE: The task is complete.

**Scenario 4: External Data Retrieval**
User: "What is the stock price of AAPL?"
Assistant:
THOUGHT: The user is asking for real-time external data. I will use the `get_stock_price` tool to fetch this data securely.
```json
{
  "name": "get_stock_price",
  "arguments": {
    "exchange": "NASDAQ",
    "ticker": "AAPL"
  }
}
```

### AVAILABLE TOOLS
ALL YOUR AVAILABLE TOOLS ARE LISTED IN THE [TOOL SCHEMA] SECTION BELOW. READ THEM CAREFULLY.

CRITICAL DIRECTIVE: Do NOT perform ICMP ping tests or DNS lookups before fetching external data! Assume the network is online. IF a user asks for stock prices in the chat, YOU ARE STRICTLY PROHIBITED from using `raw_http_fetch`. You MUST ONLY use the specific `get_stock_price` tool to fetch financials.
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
    
    if cli.seed_memory {
        println!("{}", "🧠 Injecting Core Agent Routing Instructions into Memory...".cyan());
        let core_memories = [
            (
                "tool_routing_stocks",
                "CORE INSTRUCTION (Tool Routing): Whenever the user asks to check a stock price, fetch financial data, or query NASDAQ/NYSE, you MUST use the `get_stock_price` tool. DO NOT use generic HTTP tools or web searches for stock prices.",
                vec!["routing", "stocks", "finance", "tools"]
            ),
            (
                "tool_routing_http",
                "CORE INSTRUCTION (Tool Routing): The `raw_http_fetch` tool is ONLY for debugging broken REST APIs or webhooks. Never use it to fetch website HTML, stocks, or search results.",
                vec!["routing", "http", "web", "tools"]
            ),
            (
                "tool_routing_network",
                "CORE INSTRUCTION (Tool Routing): You do not need to perform ICMP ping tests, DNS resolution, or socket checks before connecting to the internet. Assume the machine has a direct unmetered uplink.",
                vec!["routing", "network", "ping", "dns", "tools"]
            ),
            (
                "tool_routing_memory_search",
                "CORE INSTRUCTION (Context Management): If you need to search memory or research a topic, DO NOT run `memory_search` yourself. Instead, use `spawn_sub_agent` to dispatch a sub-agent to perform the search. The sub-agent must find the data, report the distilled answer, and immediately stop. This protects your main context window.",
                vec!["routing", "memory", "sub-agent", "context", "tools"]
            ),
            (
                "tool_routing_hallucination",
                "CORE INSTRUCTION (Tool Routing): You only have access to the explicit tools listed in your [TOOL SCHEMA]. If a tool name is not listed directly in your schema, IT DOES NOT EXIST. Do not guess commands.",
                vec!["routing", "schema", "hallucination", "tools"]
            ),
            (
                "task_completion",
                "CORE INSTRUCTION (Task Flow): If the user says 'thanks', 'thank you', or expresses that the task is complete, simply acknowledge it politely and then STOP. Do NOT call tools like `query_schema` or `memory_search` after a task is finished.",
                vec!["routing", "completion", "etiquette", "tools"]
            ),
            (
                "tempest_identity",
                "CORE INSTRUCTION (Identity): Your name is Tempest AI. You are a high-performance, autonomous engineering assistant. Be concise, technical, and professional at all times.",
                vec!["identity", "branding", "instructions"]
            )
        ];
        let mut count = 0;
        let store = memory_store.lock();
        for (slug, content, tags) in core_memories {
            if store.store(slug, content, Some(tags.iter().map(|s| s.to_string()).collect())).is_ok() {
                count += 1;
            }
        }
        println!("{} {} Core Memories successfully injected and permanently stored.", "✅".green(), count);
        std::process::exit(0);
    }

    let sub_agent_model = config.sub_agent_model.unwrap_or_else(|| "qwen2.5-coder:3b".to_string());
    let agent = Agent::new(model, system_prompt, history_path, memory_store.clone(), sub_agent_model);
    
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
                if let Some(mut temp) = comp.temperature() {
                    if temp > 0.0 {
                        // Some systems return milli-degrees Celsius
                        if temp > 500.0 { temp /= 1000.0; }
                        
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
            
            let mut update_str = format!(
"🔥 CPU LOAD       : {:.1}% ({} Cores)

🚀 MEMORY ALLOC   : {}/{} MB ({:.1}%)

🤖 AI RAM USE     : {} MB (Ollama)

🎨 GPU LOAD       : {}% (Graphics)

💾 SWAP CACHE     : {}/{} MB ({:.1}%)

----------------------------------

🌐 TRUNK [en0]    : {} B ▼ | {} B ▲

🌡️ AVG THERMALS   : {:.1} °C (Max: {:.1} °C)

⚙️ ACTIVE PROCS   : {}

⏱️ CORE UPTIME    : {}h {}m {}s",
                avg_cpu, cpus.len(), used_mb, total_mb, mem_perc, ollama_mb, gpu_load, used_swap, total_swap, swap_perc,
                total_rx, total_tx, avg_temp, max_temp, proc_count, hours, minutes, secs
            );

            #[cfg(target_os = "linux")]
            if tempest_monitor::linux_helper::is_steamos() {
                update_str.push_str("\n\n🩺 STEAMOS CHECK : MATCHED");
            }
            update_str.push_str("\n\n----------------------------------");
            
            let _ = agent_tx_metrics.send(crate::tui::AgentEvent::SystemUpdate(update_str.clone())).await;
            {
                let mut lock = shared_telemetry.lock();
                *lock = update_str;
            }
            tokio::time::sleep(std::time::Duration::from_secs(1)).await;
        }
    });

    *agent.event_tx.lock() = Some(agent_tx.clone());
    *agent.tool_rx.lock().await = Some(tool_rx);
    
    tokio::spawn(async move {
        if let Err(e) = agent.run_tui_mode(user_rx, stop_flag_agent).await {
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
