#![recursion_limit = "1024"]
// Modules are now exported via src/lib.rs


use tempest_ai::agent::Agent;
use clap::Parser;
use colored::*;
// Copyright (c) 2026 Robert Simens. All Rights Reserved.
// Licensed under the Tempest AI Source-Available License.
// See LICENSE in the project root for full license information.

use miette::{Result, IntoDiagnostic};
use parking_lot::Mutex;
use std::sync::Arc;

/// Tempest AI — An autonomous AI pair-programmer and system assistant.
/// 
/// Once started, you can use Slash Commands in the TUI:
///   /help      - Show the full user manual
///   /undo      - Revert the last file modifications
///   /safemode  - Toggle blocking approvals (ON/OFF)
///   /switch    - Hot-swap the inference model
#[derive(Parser, Debug)]
#[command(
    name = "tempest_ai", 
    version, 
    about = "🌪️ Tempest AI: The Hardware-Aware, Local-Inference Autonomous Engineer.", 
    after_help = "LAUNCH MODES:\n  ./tempest_ai          Launch high-fidelity TUI (Ollama)\n  ./tempest_ai --mlx    Launch high-fidelity TUI (Native Apple Silicon)\n  ./tempest_ai --cli    Launch standard command-line interface\n\nFor the full v0.3.2 Operational Manual, launch the TUI and type '/help'."
)]
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

    /// Enable Paged Attention for the MLX Backend (Experimental)
    #[arg(long)]
    paged_attn: bool,

    /// Use the AI Bridge (Universal Model Access) to connect to any provider
    #[arg(long)]
    bridge: bool,

    /// Start as a headless MCP Server (JSON-RPC over stdio) for IDE integration
    #[arg(long)]
    mcp_server: bool,

    /// Use LM Studio (Local OpenAI-compatible API) as the inference backend
    #[arg(long)]
    lmstudio: bool,
}

use tempest_ai::AppConfig;

fn load_config(cli_config_path: Option<&str>, tui_mode: bool) -> AppConfig {
    let mut paths_to_try: Vec<std::path::PathBuf> = vec![];
    
    if let Some(p) = cli_config_path {
        paths_to_try.push(std::path::PathBuf::from(p));
    }
    
    // Check local directory first for developer-centric overrides
    paths_to_try.push(std::path::PathBuf::from("config.toml"));
    
    if let Ok(sudo_user) = std::env::var("SUDO_USER") {
        if !sudo_user.is_empty() && sudo_user != "root" {
        cfg_select! {
            unix => {
                let prefixes = ["/home", "/Users"];
                for prefix in prefixes {
                    // Check standard XDG path
                    let p = std::path::PathBuf::from(prefix)
                        .join(&sudo_user)
                        .join(".config")
                        .join("tempest_ai")
                        .join("config.toml");
                    paths_to_try.push(p);

                    // Check macOS Application Support path
                    if prefix == "/Users" {
                        let p_mac = std::path::PathBuf::from(prefix)
                            .join(&sudo_user)
                            .join("Library")
                            .join("Application Support")
                            .join("tempest_ai")
                            .join("config.toml");
                        paths_to_try.push(p_mac);
                    }
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

#[tokio::main(flavor = "multi_thread")]
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
        tempest_ai::daemon::install_daemon();
        return Ok(());
    }

    if cli.daemon {
        tempest_ai::daemon::run_daemon().await;
        return Ok(());
    }

    if cli.no_color {
        colored::control::set_override(false);
    }

    let config = load_config(cli.config.as_deref(), !cli.cli);


    let os_name = match std::env::consts::OS {
        "macos" => "macOS",
        "linux" => "Linux",
        "windows" => "Windows",
        _ => std::env::consts::OS,
    };
    let system_prompt = format!(
        "{}\n\nOPERATING SYSTEM: {}\n\n{}",
        tempest_ai::prompts::SYSTEM_PROMPT_BASE,
        os_name,
        tempest_ai::prompts::SYSTEM_PROMPT_TAIL
    );

    // Model priority: CLI flag > MLX Default (if flag set) > env var > config file > default
    let model = if cli.mlx {
        cli.model.clone()
            .or(config.mlx_model.clone())
            .unwrap_or_else(|| "bartowski/Qwen2.5-Coder-7B-Instruct-GGUF".to_string())
    } else if cli.lmstudio {
        cli.model.clone()
            .or(config.lmstudio_model.clone())
            .unwrap_or_else(|| "LM Studio (External Inference)".to_string())
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

    let memory_store = Arc::new(Mutex::new(tempest_ai::memory::MemoryStore::new(passphrase).expect("Failed to initialize SQLite Memory Store")));
    
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
                "CORE INSTRUCTION (Identity): Your name is Tempest AI `v0.3.2` — \"Cyber-Orchestrator\". You are a high-performance, autonomous engineering assistant. You operate using a dual-model architecture: a Native MLX 'Smarter' Engine (Local GPU) for reasoning/coding, and a Condensed Ollama Sub-Agent (llama3.2:1b) for administrative tasks like context summarization and semantic indexing.",
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
    
    let mode = if cli.bridge {
        tempest_ai::inference::AgentMode::Bridge
    } else if cli.lmstudio {
        tempest_ai::inference::AgentMode::LMStudio
    } else {
        #[cfg(target_os = "macos")]
        {
            if cli.mlx { tempest_ai::inference::AgentMode::MLX } else { tempest_ai::inference::AgentMode::Ollama }
        }
        #[cfg(not(target_os = "macos"))]
        {
            tempest_ai::inference::AgentMode::Ollama
        }
    };

    if cli.mlx && cfg!(not(target_os = "macos")) {
         println!("{} MLX Backend is only available on macOS (Apple Silicon). Defaulting to Ollama...", "⚠️".yellow());
    }

    let quant = cli.quant.or(config.mlx_quant).unwrap_or_else(|| "Q4_K_M".to_string());

    if !cli.cli {
        println!("{}", "=".repeat(60).blue());
        println!("{} {}", "🌪️".cyan(), "TEMPEST AI • ENGINE ONLINE".bold());
        let backend_name = if cli.bridge { 
            "AI Bridge (Unified)".to_string() 
        } else if cli.lmstudio {
            format!("LM Studio (Local) • {}", config.lmstudio_url.as_deref().unwrap_or("localhost:1234"))
        } else if cli.mlx { 
            format!("MLX (Native Apple Silicon) • {}", quant) 
        } else { 
            "Ollama (Cross-Platform)".to_string() 
        };
        println!("{} {}", "⚡ Backend:".blue(), backend_name);
        
        if cli.mlx {
            println!("{} {}", "🤖 Unified:".blue(), model);
        } else if cli.lmstudio {
            println!("{} {}", "🧠 Planner:".blue(), model);
            println!("{} {}", "💻 Executor:".blue(), model);
            println!("{} {}", "🔬 Verifier:".blue(), model);
        } else {
            println!("{} {}", "🧠 Planner:".blue(), config.planner_model.as_deref().unwrap_or(&model));
            println!("{} {}", "💻 Executor:".blue(), config.executor_model.as_deref().unwrap_or(&model));
            println!("{} {}", "🔬 Verifier:".blue(), config.verifier_model.as_deref().unwrap_or(&model));
        }
        println!("{}", "=".repeat(60).blue());
    }
    
    // Pre-initialize event channel for MCP mode to capture startup logs
    let event_tx = Arc::new(parking_lot::Mutex::new(None));
    let mut event_rx = None;
    if cli.mcp_server {
        let (tx, rx) = tokio::sync::mpsc::channel(100);
        *event_tx.lock() = Some(tx);
        event_rx = Some(rx);
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
        config.ctx_planning.unwrap_or(32768),
        config.ctx_execution.unwrap_or(65536),
        config.mlx_temp_planning,
        config.mlx_temp_execution,
        config.mlx_top_p_planning,
        config.mlx_top_p_execution,
        config.mlx_repeat_penalty_planning,
        config.mlx_repeat_penalty_execution,
        cli.paged_attn || config.paged_attn.unwrap_or(false),
        config.planning_enabled.unwrap_or(true),
        event_tx.clone(),
        config.lmstudio_url.clone(),
    ).await?;
    
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
    let _ = agent.initialize_mcp(config.mcp_servers.unwrap_or_default()).await;
    let _ = agent.resume_session().await;
    if !cli.cli {
        let agent_init = agent.clone();
        tokio::spawn(async move {
            let _ = agent_init.initialize_atlas(false).await;
            let _ = agent_init.warmup().await;
        });
    }
    if !cli.cli && !cli.mcp_server {
        println!("{} Launching TUI...", "🚀".green());
    }

    if cli.mcp_server {
        let mut server = tempest_ai::mcp_server::McpServer::new(agent, event_rx);
        if let Err(e) = server.run().await {
            eprintln!("MCP Server error: {}", e);
        }
        return Ok(());
    }

    if cli.cli {
        run_cli_mode(agent).await?;
        return Ok(());
    }
    
    // Default to TUI mode
    let (user_tx, user_rx) = tokio::sync::mpsc::channel(32);
    let (agent_tx, agent_rx) = tokio::sync::mpsc::channel(10000);
    let (tool_tx, tool_rx) = tokio::sync::mpsc::channel::<tempest_ai::tui::ToolResponse>(1);

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
            let mut lmstudio_mem_bytes = 0;

            for (_pid, process) in sys.processes() {
                let name = process.name().to_string_lossy().to_lowercase();
                let exe = process.exe().map(|p| p.to_string_lossy().to_lowercase()).unwrap_or_default();
                
                if name.contains("tempest_ai") {
                    tempest_mem_bytes += process.memory();
                } else if name.contains("ollama") {
                    ollama_mem_bytes += process.memory();
                } else if name.contains("lm studio") || name.contains("lmstudio") || exe.contains(".lmstudio") {
                    lmstudio_mem_bytes += process.memory();
                }
            }

            #[cfg(target_os = "macos")]
            let mac_gpu = tempest_monitor::macos_helper::get_macos_gpu_info(false);

            // In MLX/Native mode, we need to capture the Metal/AGX memory.
            // sysinfo process.memory() often misses private Metal heaps on macOS.
            let ai_ram_mb = match mode {
                tempest_ai::inference::AgentMode::MLX => {
                    #[cfg(target_os = "macos")]
                    let mut vram_mb = 0;
                    #[cfg(not(target_os = "macos"))]
                    let vram_mb = 0;

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
                }
                tempest_ai::inference::AgentMode::Ollama => {
                    (ollama_mem_bytes + tempest_mem_bytes) / 1024 / 1024
                }
                tempest_ai::inference::AgentMode::Bridge => {
                    tempest_mem_bytes / 1024 / 1024 // External or local but not managed here
                }
                tempest_ai::inference::AgentMode::LMStudio => {
                    (lmstudio_mem_bytes + tempest_mem_bytes) / 1024 / 1024
                }
            };

            let engine_label = match mode {
                tempest_ai::inference::AgentMode::MLX => "(Native Engine)",
                tempest_ai::inference::AgentMode::Ollama => "(Ollama)",
                tempest_ai::inference::AgentMode::Bridge => "(Bridge)",
                tempest_ai::inference::AgentMode::LMStudio => "(LM Studio)",
            };

            let gpu_load = {
                #[cfg(target_os = "macos")]
                {
                    mac_gpu.usage_pct as i32
                }
                #[cfg(target_os = "linux")]
                {
                    tempest_ai::hardware::get_linux_gpu_usage()
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
            
            #[cfg(target_os = "macos")]
            let gpu_freq_str = if let Some(f) = mac_gpu.gpu_freq_mhz { format!(" @ {:.0} MHz", f) } else { "".to_string() };
            #[cfg(not(target_os = "macos"))]
            let gpu_freq_str = "".to_string();

            #[cfg(target_os = "macos")]
            let ane_power_str = if let Some(p) = mac_gpu.ane_power_mw { format!("\n\n🧠 ANE POWER      : {:.0} mW (Neural Engine)", p) } else { "".to_string() };
            #[cfg(not(target_os = "macos"))]
            let ane_power_str = "".to_string();

            let mut update_str = format!(
"🔥 CPU LOAD       : {:.1}% ({} Cores)

🚀 MEMORY ALLOC   : {}/{} MB ({:.1}%)

🤖 AI RAM USE     : {} MB {}

🎨 GPU LOAD       : {}% (Graphics){}{}

💾 SWAP CACHE     : {}/{} MB ({:.1}%)

----------------------------------

🛰️ NETWORK [en0]    : {} B ▼ | {} B ▲

🌡️ AVG THERMALS   : {:.1} °C (Max: {:.1} °C)

⚙️ ACTIVE PROCS   : {}

⏱️ CORE UPTIME    : {}h {}m {}s",
                avg_cpu, cpus.len(), used_mb, total_mb, mem_perc, ai_ram_mb, engine_label, gpu_load, gpu_freq_str, ane_power_str, used_swap, total_swap, swap_perc,
                total_rx, total_tx, avg_temp, max_temp, proc_count, hours, minutes, secs
            );

            #[cfg(target_os = "linux")]
            if tempest_monitor::linux_helper::is_steamos() {
                update_str.push_str("\n\n🩺 STEAMOS CHECK : MATCHED");
            }
            update_str.push_str("\n\n----------------------------------");
            
            let _ = agent_tx_metrics.send(tempest_ai::tui::AgentEvent::SystemUpdate(update_str.clone())).await;
            
            // --- 📊 SPARKLINE DATA EXTRACTION ---
            // Send parsed values for Sparklines (Scaled x100 for ultra-high-resolution u64 representation)
            let _ = agent_tx_metrics.send(tempest_ai::tui::AgentEvent::TelemetryMetrics { 
                cpu: Some((avg_cpu * 100.0) as u64), 
                gpu: Some(gpu_load as u64 * 100),
                tps: None 
            }).await;

            {
                let mut lock = shared_telemetry.lock();
                *lock = update_str;
            }
            tokio::time::sleep(std::time::Duration::from_millis(500)).await;
        }
    });

    *agent.event_tx.lock() = Some(agent_tx.clone());
    *agent.tool_rx.lock().await = Some(tool_rx);
    
    let agent_tui = agent.clone();
    tokio::spawn(async move {
        if let Err(e) = agent_tui.run_tui_mode(user_rx, stop_flag_agent).await {
            let _ = agent_tx.send(tempest_ai::tui::AgentEvent::SystemUpdate(format!("Agent crashed: {}", e))).await;
        }
    });

    let initial_theme = config.tui_theme.clone().unwrap_or_else(|| "base16-ocean.dark".to_string());

    if let Err(e) = tempest_ai::tui::run_tui(agent_rx, user_tx, tool_tx, stop_flag, initial_theme).await {
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
                if p == "/undo" {
                    match agent.checkpoint_mgr.lock().undo() {
                        Ok(summary) => println!("{}", summary),
                        Err(msg) => println!("{} {}", "⚠️".yellow(), msg),
                    }
                    continue;
                }
                if p == "/checkpoints" {
                    println!("{}", agent.checkpoint_mgr.lock().list_checkpoints());
                    continue;
                }
                if p == "/help" {
                    println!("{}", "Commands:".bold());
                    println!("  /clear       — Wipe conversation history");
                    println!("  /undo        — Revert the last file modification");
                    println!("  /checkpoints — List available undo checkpoints");
                    println!("  /quit        — Exit Tempest");
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
