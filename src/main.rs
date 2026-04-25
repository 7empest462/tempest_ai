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
mod context_manager;
mod error_classifier;
mod rules;
pub mod sentinel;
mod inference;

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

    /// Use the MLX Backend (Apple Silicon Neural Engine + GPU) instead of Ollama
    #[arg(long)]
    mlx: bool,

    /// MLX Quantization variant (e.g. Q4_K_M, Q8_0)
    #[arg(long)]
    quant: Option<String>,
}

#[derive(serde::Deserialize, Debug, Clone)]
pub struct MlxPreset {
    pub repo: String,
    pub quant: String,
}

#[allow(dead_code)]
#[derive(serde::Deserialize, Debug)]
struct AppConfig {
    model: Option<String>,
    history_path: Option<String>,
    db_path: Option<String>,
    encrypt_history: Option<bool>,
    pub sub_agent_model: Option<String>,
    pub mlx_model: Option<String>,
    pub mlx_quant: Option<String>,
    pub planner_model: Option<String>,
    pub executor_model: Option<String>,
    pub verifier_model: Option<String>,
    pub mlx_presets: Option<std::collections::HashMap<String, MlxPreset>>,
    pub temp_planning: Option<f32>,
    pub temp_execution: Option<f32>,
    pub top_p_planning: Option<f32>,
    pub top_p_execution: Option<f32>,
    pub repeat_penalty_planning: Option<f32>,
    pub repeat_penalty_execution: Option<f32>,
    pub ctx_planning: Option<u64>,
    pub ctx_execution: Option<u64>,
    pub mlx_temp_planning: Option<f32>,
    pub mlx_temp_execution: Option<f32>,
    pub mlx_top_p_planning: Option<f32>,
    pub mlx_top_p_execution: Option<f32>,
    pub mlx_repeat_penalty_planning: Option<f32>,
    pub mlx_repeat_penalty_execution: Option<f32>,
}

impl Default for AppConfig {
    fn default() -> Self {
        let mut mlx_presets = std::collections::HashMap::new();
        mlx_presets.insert("r1".to_string(), MlxPreset {
            repo: "bartowski/DeepSeek-R1-Distill-Qwen-7B-GGUF".to_string(),
            quant: "Q8_0".to_string(),
        });
        mlx_presets.insert("qwen_big".to_string(), MlxPreset {
            repo: "bartowski/Qwen2.5-Coder-7B-Instruct-GGUF".to_string(),
            quant: "Q8_0".to_string(),
        });
        mlx_presets.insert("qwen_small".to_string(), MlxPreset {
            repo: "bartowski/Qwen2.5-Coder-7B-Instruct-GGUF".to_string(),
            quant: "Q4_K_M".to_string(),
        });

        AppConfig {
            model: Some("qwen2.5-coder:7b".to_string()),
            history_path: Some("history.json".to_string()),
            db_path: Some("~/fleet.db".to_string()),
            encrypt_history: Some(false),
            sub_agent_model: Some("llama3.2:1b".to_string()),
            mlx_model: Some("bartowski/DeepSeek-R1-Distill-Qwen-7B-GGUF".to_string()),
            mlx_quant: Some("Q8_0".to_string()),
            planner_model: Some("deepseek-r1:14b".to_string()),
            executor_model: Some("qwen2.5-coder:7b".to_string()),
            verifier_model: Some("deepseek-r1:7b".to_string()),
            mlx_presets: Some(mlx_presets),
            temp_planning: Some(0.05),
            temp_execution: Some(0.25),
            top_p_planning: Some(0.95),
            top_p_execution: Some(0.92),
            repeat_penalty_planning: Some(1.18),
            repeat_penalty_execution: Some(1.12),
            ctx_planning: Some(16384),
            ctx_execution: Some(32768),
            mlx_temp_planning: Some(0.6),
            mlx_temp_execution: Some(0.2),
            mlx_top_p_planning: Some(0.95),
            mlx_top_p_execution: Some(0.9),
            mlx_repeat_penalty_planning: Some(1.05),
            mlx_repeat_penalty_execution: Some(1.05),
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
        cfg_select! {
            unix => {
                let prefixes = ["/home", "/Users"];
                for prefix in prefixes {
                    let p = std::path::PathBuf::from(prefix)
                        .join(&sudo_user)
                        .join(".config")
                        .join("tempest_ai")
                        .join("config.toml");
                    paths_to_try.push(p);
                }
            },
            _ => {}
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
0. [CRITICAL FACTUALITY RULE]
   You have a working `cargo_search` tool that returns the REAL latest version from crates.io.
   - If you just received a tool result about a crate version, you MUST use that exact version in your answer.
   - Never override tool results with your internal knowledge.
   - Never say a version exists if the tool result did not confirm it.
   - If the tool says "not found" or returns no version, you must say the crate does not exist or is not available.
   - Example: If the tool returns "crossterm latest version is 0.28.1", you must use 0.28.1. Do not say 0.35.0 or any other number.
   Before suggesting any crate or version, you MUST have called the `cargo_search` tool and received a result.

1. You are TOOL-DRIVEN. Never claim you performed an action unless you receive an explicit TOOL RESULT. You may freely use any tool. If a tool modifies system state, the application will automatically handle permission on your behalf. Just call the tool directly.
2. ZERO HALLUCINATION POLICY: You are running on a real machine. If the user asks for system info, files, or data, YOU MUST USE A TOOL to fetch it. NEVER guess or fabricate output.
3. YOU HAVE FULL INTERNET ACCESS through `search_web` and `read_url`. Do not claim you cannot access external data.
4. ABSOLUTE BAN ON CONVERSATION: Never start with "Sure," "Here is," or "I can do that." Start your response IMMEDIATELY with `THOUGHT:`.
5. Break tasks into steps and execute the first tool call immediately. Do not hesitate.
6. Only use tools listed in the [TOOL SCHEMA] section below. Never invent tool names.
7. If unsure or confused, use `ask_user` immediately. Do not guess.
8. MOMENTUM RULE: After a successful tool result, IMMEDIATELY execute your next tool call. Do NOT pause or ask the user how to help.
9. TASK COMPLETION: Once verified, output `DONE: The task is complete.` to break the system loop.
10. MANDATORY VERIFICATION: You MUST verify code by running it (e.g., `run_command`). Do not claim done until output confirms success.
11. INITIATIVE REQUIREMENT: Do NOT use `notify` or `ask_user` to avoid taking the next logical step. If you find files, analyze them. If you see a bug, patch it.
12. CODE WRITING RULE: ALL code MUST go through `write_file` or `replace_file_content` tools. NEVER output raw code blocks (```rust, ```python, etc.) into chat. Code in chat is NOT saved to disk.

### RESPONSE FORMAT
- **If you are a reasoning model (like DeepSeek-R1):** You MUST begin your response with native `<think>` tags. Perform all your internal planning and tool selection inside these tags. After the closing `</think>` tag, output your selected tool call in the JSON format below.
- **If you are a standard model:** Start your response immediately with `THOUGHT:` followed by your reasoning and then the JSON tool call.

**Tool Call Format:**

**Standard Turn (Standard Model):**
THOUGHT: [Your reasoning]
```json
{
  "name": "tool_name",
  "arguments": { "key": "value" }
}
```

**Task Completion:**
THOUGHT: [Summary of what you accomplished]
DONE: The task is complete.

### EXAMPLES

**Example 1: Read a file**
THOUGHT: I need to inspect the source. I will use `read_file`.
```json
{
  "name": "read_file",
  "arguments": { "path": "src/main.rs" }
}
```

**Example 2: Write code to a file**
THOUGHT: I will write the calculator logic to src/main.rs using write_file.
```json
{
  "name": "write_file",
  "arguments": { "path": "src/main.rs", "content": "fn main() {\n    println!(\"Hello\");\n}" }
}
```

**Example 3: Add a Rust dependency**
THOUGHT: I need to add `tokio` for async support. I will use `cargo_add`.
```json
{
  "name": "cargo_add",
  "arguments": { "crate_name": "tokio", "features": ["full"], "cwd": "project_dir" }
}
```

### AVAILABLE TOOLS
All tools are listed in the [TOOL SCHEMA] section below. Use them responsibly.
Never invent tool names. If you need a capability that isn't listed, use `ask_user`.

You are running on a real machine with real consequences. Be precise, safe, and professional.
"#.to_string();

    // Model priority: CLI flag > MLX Default (if flag set) > env var > config file > default
    let model = if cli.mlx {
        cli.model.clone()
            .or(config.mlx_model.clone())
            .unwrap_or_else(|| "bartowski/Qwen2.5-Coder-7B-Instruct-GGUF".to_string())
    } else {
        cli.model.clone()
            .or_else(|| std::env::var("OLLAMA_MODEL").ok())
            .or(config.model.clone())
            .unwrap_or_else(|| "qwen2.5-coder:7b".to_string())
    };

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
                "CORE INSTRUCTION (Identity): Your name is Tempest AI. You are a high-performance, autonomous engineering assistant. You operate using a dual-model architecture: a Native MLX 'Smarter' Engine (Local GPU) for reasoning/coding, and a Condensed Ollama Sub-Agent (llama3.2:1b) for administrative tasks like context summarization and semantic indexing.",
                vec!["identity", "branding", "instructions", "architecture"]
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

    let sub_agent_model = config.sub_agent_model.unwrap_or_else(|| "llama3.2:1b".to_string());
    
    let mode = if cli.mlx {
        crate::inference::AgentMode::MLX
    } else {
        crate::inference::AgentMode::Ollama
    };

    let quant = cli.quant.or(config.mlx_quant).unwrap_or_else(|| "Q4_K_M".to_string());

    if !cli.cli {
        println!("{} Initializing Tempest AI Agent (Backend: {:?}, Model: {})...", "🚀".blue(), mode, model);
    }

    let agent = Agent::new(
        mode, 
        model, 
        quant, 
        system_prompt, 
        history_path, 
        memory_store.clone(), 
        sub_agent_model,
        config.planner_model.clone(),
        config.executor_model.clone(),
        config.verifier_model.clone(),
        config.mlx_presets.clone().unwrap_or_default(),
        config.temp_planning.unwrap_or(0.05),
        config.temp_execution.unwrap_or(0.25),
        config.top_p_planning.unwrap_or(0.95),
        config.top_p_execution.unwrap_or(0.92),
        config.repeat_penalty_planning.unwrap_or(1.18),
        config.repeat_penalty_execution.unwrap_or(1.12),
        config.ctx_planning.unwrap_or(16384),
        config.ctx_execution.unwrap_or(32768),
        config.mlx_temp_planning,
        config.mlx_temp_execution,
        config.mlx_top_p_planning,
        config.mlx_top_p_execution,
        config.mlx_repeat_penalty_planning,
        config.mlx_repeat_penalty_execution,
    ).await;
    
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
    if !cli.cli {
        println!("{} Indexing local workspace and skills...", "🧠".cyan());
    }
    let _ = agent.initialize_atlas(false).await;
    if !cli.cli {
        println!("{} Startup sequence complete. Launching TUI...", "✅".green());
    }

    if cli.cli {
        run_cli_mode(agent).await?;
        return Ok(());
    }
    
    // Default to TUI mode
    let (user_tx, user_rx) = tokio::sync::mpsc::channel(32);
    let (agent_tx, agent_rx) = tokio::sync::mpsc::channel(10000);
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

        loop {
            sys.refresh_all();
            networks.refresh(true);
            components.refresh(true);
            
            let mut ollama_mem_bytes = 0;
            let mut tempest_mem_bytes = 0;
            let current_pid = std::process::id();

            for (pid, process) in sys.processes() {
                let name = process.name().to_string_lossy().to_lowercase();
                if name.contains("ollama") || name.contains("llama") {
                    ollama_mem_bytes = std::cmp::max(ollama_mem_bytes, process.memory());
                }
                if pid.as_u32() == current_pid {
                    tempest_mem_bytes = process.memory();
                }
            }

            #[cfg(target_os = "macos")]
            let mac_gpu = tempest_monitor::macos_helper::get_macos_gpu_info(false);

            // In MLX/Native mode, we need to capture the Metal/AGX memory.
            // sysinfo process.memory() often misses private Metal heaps on macOS.
            let ai_ram_mb = if mode == crate::inference::AgentMode::MLX {
                let mut vram_mb = 0;
                #[cfg(target_os = "macos")]
                {
                    // get_macos_gpu_info returns usage, but we need the VRAM 'In Use' metric.
                    // We'll peek at the PerformanceStatistics from AGX specifically.
                    if let Ok(output) = std::process::Command::new("ioreg").args(["-r", "-d", "1", "-c", "AGXAccelerator"]).output() {
                        let s = String::from_utf8_lossy(&output.stdout);
                        // Greedy sum of all system memory keys (Alloc, In Use, Driver, etc.)
                        let vram_re = regex::Regex::new(r#""(?:Alloc|In use) system memory(?:\s*\(driver\))?"\s*=\s*(\d+)"#).unwrap();
                        for caps in vram_re.captures_iter(&s) {
                            if let Some(m) = caps.get(1) {
                                vram_mb += m.as_str().parse::<u64>().unwrap_or(0) / 1024 / 1024;
                            }
                        }
                    }
                }
                (tempest_mem_bytes / 1024 / 1024) + vram_mb
            } else {
                (ollama_mem_bytes + tempest_mem_bytes) / 1024 / 1024
            };
            let engine_label = match mode {
                crate::inference::AgentMode::MLX => "(Native Engine)",
                crate::inference::AgentMode::Ollama => "(Ollama)",
            };

            let gpu_load = {
                #[cfg(target_os = "macos")]
                {
                    mac_gpu.usage_pct as i32
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
                        // Some systems return milli-degrees Celsius (e.g. 45000)
                        if temp > 500.0 { temp /= 1000.0; }
                        
                        // Safety cap for invalid sensors
                        if temp > 150.0 { continue; }
                        
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

🤖 AI RAM USE     : {} MB {}

🎨 GPU LOAD       : {}% (Graphics)

💾 SWAP CACHE     : {}/{} MB ({:.1}%)

----------------------------------

🌐 TRUNK [en0]    : {} B ▼ | {} B ▲

🌡️ AVG THERMALS   : {:.1} °C (Max: {:.1} °C)

⚙️ ACTIVE PROCS   : {}

⏱️ CORE UPTIME    : {}h {}m {}s",
                avg_cpu, cpus.len(), used_mb, total_mb, mem_perc, ai_ram_mb, engine_label, gpu_load, used_swap, total_swap, swap_perc,
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
    
    let agent_tui = agent.clone();
    tokio::spawn(async move {
        if let Err(e) = agent_tui.run_tui_mode(user_rx, stop_flag_agent).await {
            let _ = agent_tx.send(crate::tui::AgentEvent::SystemUpdate(format!("Agent crashed: {}", e))).await;
        }
    });

    if let Err(e) = crate::tui::run_tui(agent_rx, user_tx, tool_tx, stop_flag).await {
        println!("{}", format!("TUI Render Error: {}", e).red());
    }
    
    // KILL SWITCH: Signal Ollama to unload the model from VRAM immediately on exit
    agent.shutdown().await;
    
    Ok(())
}

async fn run_cli_mode(agent: Agent) -> Result<()> {
    use rustyline::DefaultEditor;
    let mut rl = DefaultEditor::new().into_diagnostic()?;
    let stop_flag = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    
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
                
                if let Err(e) = agent.run(p.to_string(), stop_flag.clone()).await {
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
