#![recursion_limit = "1024"]
// Copyright (c) 2026 Robert Simens. All Rights Reserved.
// Licensed under the Tempest AI Source-Available License.
// See LICENSE in the project root for full license information.
// Modules are now exported via src/lib.rs

use clap::Parser;
use colored::*;
use miette::{IntoDiagnostic, Result};
use parking_lot::Mutex;
use std::sync::Arc;
use tempest_ai::agent::Agent;

/// Tempest AI — An autonomous AI pair-programmer and system assistant.
///
/// Once started, you can use Slash Commands in the TUI:
///   /help      - Show the full user manual
///   /undo      - Revert the last file modifications
///   /safemode  - Toggle blocking approvals (ON/OFF)
///   /switch    - Hot-swap the inference model
///   /tool      - Test a tool directly (Diagnostic Mode)
#[derive(Parser, Debug)]
#[command(
    name = "tempest_ai",
    version,
    about = "🌪️ Tempest AI: The Hardware-Aware, Local-Inference Autonomous Engineer.",
    after_help = "LAUNCH MODES:\n  ./tempest_ai          Launch high-fidelity TUI (Ollama)\n  ./tempest_ai --mlx    Launch high-fidelity TUI (Native Apple Silicon)\n  ./tempest_ai --web    Launch Standalone Web Command Center\n  ./tempest_ai --cli    Launch standard command-line interface\n\nFor the full v0.3.5 Operational Manual, launch the TUI and type '/help'."
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
    pub seed_memory: bool,

    /// Connect to remote Ollama cloud instance via SSH tunnel
    #[arg(long)]
    pub cloud: bool,

    /// Use the MLX Backend (Apple Silicon Neural Engine + GPU) instead of Ollama
    #[arg(long)]
    mlx: bool,

    /// Use Google Gemini API as the Backend (requires GEMINI_API_KEY env var)
    #[arg(long)]
    gemini: bool,

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

    /// Use Kalosm (Native GPU) as the inference backend
    #[arg(long)]
    kalosm: bool,

    /// Start the Tempest Nexus Web Server (WebSocket API)
    #[arg(long)]
    web: bool,

    /// Port for the Tempest Nexus Web Server (Default: 8080)
    #[arg(short, long)]
    pub port: Option<u16>,

    /// Port for the Prometheus Metrics Exporter (Default: 7777)
    #[arg(long)]
    pub metrics_port: Option<u16>,

    /// Hard cap for the PagedAttention memory budget (MB). E.g., 2048 for 2GB.
    #[arg(long)]
    pub pa_memory_mb: Option<usize>,

    /// Enable Safe Mode (block for approvals on file modifications)
    #[arg(short, long)]
    pub safe: bool,

    /// Resume a previous session by providing its Session ID
    #[arg(long)]
    pub resume: Option<String>,
}

use tempest_ai::AppConfig;

fn load_config(cli_config_path: Option<&str>, tui_mode: bool) -> AppConfig {
    let mut paths_to_try: Vec<std::path::PathBuf> = vec![];

    if let Some(p) = cli_config_path {
        paths_to_try.push(std::path::PathBuf::from(p));
    }

    // Check local directory first for developer-centric overrides
    paths_to_try.push(std::path::PathBuf::from("config.toml"));

    if let Some(sudo_user) = std::env::var("SUDO_USER")
        .ok()
        .filter(|s| !s.is_empty() && s != "root")
    {
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

    if let Some(config_dir) = dirs::config_dir() {
        paths_to_try.push(config_dir.join("tempest_ai").join("config.toml"));
    }
    if let Some(home) = dirs::home_dir() {
        paths_to_try.push(home.join(".config").join("tempest_ai").join("config.toml"));
    }

    for path in &paths_to_try {
        if let Some(config) = std::fs::read_to_string(path)
            .ok()
            .and_then(|content| toml::from_str::<AppConfig>(&content).ok())
        {
            if !tui_mode {
                println!("{} Loaded config from: {}", "⚙️".blue(), path.display());
            }
            return config;
        }
    }

    if !tui_mode {
        println!(
            "{} No valid config found. Using default settings.",
            "ℹ️".dimmed()
        );
    }
    AppConfig::default()
}

use metrics_exporter_prometheus::PrometheusBuilder;
use std::net::SocketAddr;

#[tokio::main(flavor = "multi_thread")]
async fn main() -> Result<()> {
    // Initialize tracing/console subscriber first for performance monitoring and tokio-console
    #[cfg(feature = "console")]
    {
        println!("📡 [CONSOLE]: Initializing console-subscriber on 0.0.0.0:6669...");
        console_subscriber::ConsoleLayer::builder()
            .server_addr(([0, 0, 0, 0], 6669))
            .init();
    }

    #[cfg(not(feature = "console"))]
    {
        tracing_subscriber::fmt()
            .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
            .init();
    }

    let cli = Cli::parse();

    // Determine base ports from CLI -> Config -> Defaults
    let config = load_config(
        cli.config.as_deref(),
        !cli.web && !cli.cli && !cli.mcp_server,
    );

    // 1. Resolve Nexus Port with automatic fallback
    let nexus_pref = cli.port.or(config.nexus_port).unwrap_or(8080);
    let mut nexus_port = nexus_pref;
    let mut nexus_found = false;
    while !nexus_found && nexus_port < nexus_pref + 20 {
        if std::net::TcpListener::bind(format!("0.0.0.0:{}", nexus_port)).is_ok() {
            nexus_found = true;
        } else {
            nexus_port += 1;
        }
    }
    if nexus_port != nexus_pref {
        println!(
            "{} Port {} occupied. Nexus live on: http://localhost:{}",
            "⚠️".yellow(),
            nexus_pref,
            nexus_port
        );
    }

    // 2. Resolve Metrics Port with automatic fallback
    let metrics_pref = cli.metrics_port.or(config.metrics_port).unwrap_or(7777);
    let mut metrics_port = metrics_pref;
    let mut metrics_found = false;
    while !metrics_found && metrics_port < metrics_pref + 20 {
        if std::net::TcpListener::bind(format!("0.0.0.0:{}", metrics_port)).is_ok() {
            metrics_found = true;
        } else {
            metrics_port += 1;
        }
    }
    if metrics_port != metrics_pref {
        println!(
            "{} Port {} occupied. Metrics live on: {}",
            "⚠️".yellow(),
            metrics_pref,
            metrics_port
        );
    }

    // 3. Initialize Prometheus metrics exporter on the resolved port
    let addr: SocketAddr = format!("0.0.0.0:{}", metrics_port)
        .parse()
        .expect("Invalid metrics address");
    PrometheusBuilder::new()
        .with_http_listener(addr)
        .install()
        .expect("failed to install Prometheus recorder");

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
    let system_prompt = tempest_ai::templates::render_system_prompt(
        tempest_ai::prompts::SYSTEM_PROMPT_BASE,
        os_name,
        tempest_ai::prompts::SYSTEM_PROMPT_TAIL,
    )
    .expect("Failed to render system prompt template");

    // Model priority: CLI flag > Backend Default (if flag set) > env var > config file > default
    let model = if cli.mlx {
        cli.model
            .clone()
            .or(config.mlx_model.clone())
            .unwrap_or_else(|| "bartowski/Qwen2.5-Coder-7B-Instruct-GGUF".to_string())
    } else if cli.lmstudio {
        cli.model
            .clone()
            .or(config.lmstudio_model.clone())
            .unwrap_or_else(|| "LM Studio (External Inference)".to_string())
    } else if cli.kalosm {
        cli.model
            .clone()
            .or(config.kalosm_model.clone())
            .unwrap_or_else(|| "kalosm_default".to_string())
    } else if cli.gemini {
        cli.model
            .clone()
            .or(config.gemini_model.clone())
            .unwrap_or_else(|| "gemini-3.1-pro-preview-customtools".to_string())
    } else if cli.cloud {
        cli.model
            .clone()
            .or_else(|| std::env::var("OLLAMA_MODEL").ok())
            .unwrap_or_else(|| "gemma4:31b-cloud".to_string())
    } else {
        cli.model
            .clone()
            .or_else(|| std::env::var("OLLAMA_MODEL").ok())
            .or(config.model.clone())
            .unwrap_or_else(|| "qwen2.5-coder:7b".to_string())
    };

    let mut config_dir = dirs::config_dir().unwrap_or_else(|| std::path::PathBuf::from("."));
    config_dir.push("tempest_ai");
    let _ = std::fs::create_dir_all(&config_dir);

    let session_id = cli
        .resume
        .clone()
        .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());

    let sessions_dir = config_dir.join("sessions");
    let _ = std::fs::create_dir_all(&sessions_dir);

    let history_path = if cli.resume.is_some() {
        sessions_dir
            .join(format!("{}.json", session_id))
            .to_string_lossy()
            .to_string()
    } else {
        match &config.history_path {
            Some(history_raw) => {
                if std::path::Path::new(history_raw).is_absolute() {
                    history_raw.clone()
                } else {
                    config_dir.join(history_raw).to_string_lossy().to_string()
                }
            }
            None => sessions_dir
                .join(format!("{}.json", session_id))
                .to_string_lossy()
                .to_string(),
        }
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

    let memory_store = Arc::new(Mutex::new(
        tempest_ai::memory::MemoryStore::new(passphrase)
            .expect("Failed to initialize SQLite Memory Store"),
    ));

    if cli.seed_memory {
        println!(
            "{}",
            "🧠 Injecting Core Agent Routing Instructions into Memory...".cyan()
        );
        let core_memories = [
            (
                "tempest_identity",
                "CORE INSTRUCTION (Identity): Your name is Tempest AI `v0.3.5` — \"Cyber-Orchestrator\". You are a high-performance, autonomous engineering assistant. You operate using a multi-model architecture: a Native MLX 'Smarter' Engine (Local GPU) or AI Bridge for reasoning/coding, and a Condensed Ollama Sub-Agent (llama3.2:1b) for administrative tasks like context summarization, semantic indexing, and search.",
                vec!["identity", "branding", "instructions", "architecture"],
            ),
            (
                "code_quality_guideline",
                "CORE INSTRUCTION (Code Quality): When writing or modifying Rust code, ALWAYS ensure that the code compiles successfully by running `cargo check` or `cargo clippy`. Never leave placeholders, stubs, or comments like `// TODO` in code modifications. Implement the requested logic completely and professionally.",
                vec!["coding", "quality", "rust", "verification"],
            ),
            (
                "tool_routing_http",
                "CORE INSTRUCTION (Tool Routing): Use high-level tools like `search_web` and `read_url` for gathering web data and reading documentation. The `raw_http_fetch` tool is ONLY a last resort for debugging broken REST APIs or webhooks. Never use it to fetch website HTML, stocks, or search results.",
                vec!["routing", "http", "web", "tools"],
            ),
            (
                "context_management",
                "CORE INSTRUCTION (Context Management): To protect your main context window from token pressure, use the `skg-context` package. For long-running tasks or heavy search queries, delegate work to a sub-agent using `spawn_sub_agent`. The sub-agent will execute the task, report a distilled answer, and terminate, preserving the primary agent's token budget.",
                vec!["context", "compaction", "sub-agent", "performance"],
            ),
            (
                "file_modification_safety",
                "CORE INSTRUCTION (Safety): Before making non-trivial modifications to files, check the sentinel constraints and verify build safety. Modify files incrementally and verify compilation at each step. Use the checkpoint and undo stack (`/undo` in TUI/CLI) if a change breaks the build.",
                vec!["safety", "checkpoint", "undo", "files"],
            ),
            (
                "tool_routing_hallucination",
                "CORE INSTRUCTION (Tool Routing): You only have access to the explicit tools listed in your [TOOL SCHEMA]. If a tool name is not listed directly in your schema, IT DOES NOT EXIST. Do not guess or call non-existent tools. Check tool definitions to verify arguments.",
                vec!["routing", "schema", "hallucination", "tools"],
            ),
            (
                "task_completion",
                "CORE INSTRUCTION (Task Flow): If the user says 'thanks', 'thank you', or indicates that the task is complete, simply acknowledge it politely and then STOP. Do NOT call tools like `query_schema` or `memory_search` after a task is finished.",
                vec!["routing", "completion", "etiquette", "tools"],
            ),
        ];
        let mut count = 0;
        let store = memory_store.lock();
        for (slug, content, tags) in core_memories {
            if store
                .store(
                    slug,
                    content,
                    Some(tags.iter().map(|s| s.to_string()).collect()),
                )
                .is_ok()
            {
                count += 1;
            }
        }
        println!(
            "{} {} Core Memories successfully injected and permanently stored.",
            "✅".green(),
            count
        );
        std::process::exit(0);
    }

    let sub_agent_model = config
        .sub_agent_model
        .unwrap_or_else(|| "llama3.2:1b".to_string());

    let mode = if cli.bridge {
        tempest_ai::inference::AgentMode::Bridge
    } else if cli.lmstudio {
        tempest_ai::inference::AgentMode::LMStudio
    } else if cli.kalosm {
        tempest_ai::inference::AgentMode::Kalosm
    } else {
        #[cfg(target_os = "macos")]
        {
            if cli.mlx {
                tempest_ai::inference::AgentMode::MLX
            } else if cli.gemini {
                tempest_ai::inference::AgentMode::Gemini
            } else {
                tempest_ai::inference::AgentMode::Ollama
            }
        }
        #[cfg(not(target_os = "macos"))]
        {
            if cli.gemini {
                tempest_ai::inference::AgentMode::Gemini
            } else {
                tempest_ai::inference::AgentMode::Ollama
            }
        }
    };

    if cli.mlx && cfg!(not(target_os = "macos")) {
        println!(
            "{} MLX Backend is only available on macOS (Apple Silicon). Defaulting to Ollama...",
            "⚠️".yellow()
        );
    }

    let quant = cli
        .quant
        .or(config.mlx_quant)
        .unwrap_or_else(|| "Q4_K_M".to_string());

    if !cli.cli {
        println!("{}", "=".repeat(60).blue());
        println!("{} {}", "🌪️".cyan(), "TEMPEST AI • ENGINE ONLINE".bold());
        let backend_name = if cli.bridge {
            "AI Bridge (Unified)".to_string()
        } else if cli.lmstudio {
            format!(
                "LM Studio (Local) • {}",
                config.lmstudio_url.as_deref().unwrap_or("localhost:1234")
            )
        } else if cli.kalosm {
            "Kalosm (Native GPU)".to_string()
        } else if cli.gemini {
            "Google Gemini (API)".to_string()
        } else if cli.mlx {
            format!("MLX (Native Apple Silicon) • {}", quant)
        } else {
            "Ollama (Cross-Platform)".to_string()
        };
        println!("{} {}", "⚡ Backend:".blue(), backend_name);

        if cli.mlx {
            println!("{} {}", "🤖 Unified:".blue(), model);
        } else if cli.kalosm {
            println!(
                "{} {}",
                "🤖 Unified:".blue(),
                config.kalosm_model.as_deref().unwrap_or(&model)
            );
        } else if cli.gemini {
            println!("{} {}", "🤖 Unified:".blue(), model);
        } else if cli.lmstudio {
            println!("{} {}", "🧠 Planner:".blue(), model);
            println!("{} {}", "💻 Executor:".blue(), model);
            println!("{} {}", "🔬 Verifier:".blue(), model);
        } else {
            if config.vram_time_sharing.unwrap_or(false) {
                println!("{} {}", "🤖 Unified (VRAM Sharing):".blue(), model);
            } else {
                println!(
                    "{} {}",
                    "🧠 Planner:".blue(),
                    config.planner_model.as_deref().unwrap_or(&model)
                );
                println!(
                    "{} {}",
                    "💻 Executor:".blue(),
                    config.executor_model.as_deref().unwrap_or(&model)
                );
                println!(
                    "{} {}",
                    "🔬 Verifier:".blue(),
                    config.verifier_model.as_deref().unwrap_or(&model)
                );
            }
        }
        println!("{}", "=".repeat(60).blue());
    }

    // Pre-initialize event channel for MCP mode to capture startup logs
    let event_tx = Arc::new(parking_lot::Mutex::new(None));
    let mut event_rx = None;
    let (tool_tx, tool_rx_internal) =
        tokio::sync::mpsc::channel::<tempest_ai::tui::ToolResponse>(1);
    let mut tool_tx_opt = None;

    if cli.mcp_server || cli.web {
        let (tx, rx) = tokio::sync::mpsc::channel(100);
        *event_tx.lock() = Some(tx);
        event_rx = Some(rx);
        tool_tx_opt = Some(tool_tx);
    }

    let agent = Agent::new(
        mode,
        model,
        quant,
        system_prompt,
        history_path,
        session_id,
        memory_store.clone(),
        sub_agent_model,
        event_tx.clone(),
        tempest_ai::agent::AgentConfig {
            planner_model: config.planner_model.clone(),
            executor_model: config.executor_model.clone(),
            verifier_model: config.verifier_model.clone(),
            mlx_presets: config.mlx_presets.clone().unwrap_or_default(),
            temp_planning: config.temp_planning.unwrap_or(0.05),
            temp_execution: config.temp_execution.unwrap_or(0.25),
            top_p_planning: config.top_p_planning.unwrap_or(0.95),
            top_p_execution: config.top_p_execution.unwrap_or(0.92),
            repeat_penalty_planning: config.repeat_penalty_planning.unwrap_or(1.18),
            repeat_penalty_execution: config.repeat_penalty_execution.unwrap_or(1.12),
            ctx_planning: config.ctx_planning.unwrap_or(12288),
            ctx_execution: config.ctx_execution.unwrap_or(32768),
            mlx_temp_planning: config.mlx_temp_planning,
            mlx_temp_execution: config.mlx_temp_execution,
            mlx_top_p_planning: config.mlx_top_p_planning,
            mlx_top_p_execution: config.mlx_top_p_execution,
            mlx_repeat_penalty_planning: config.mlx_repeat_penalty_planning,
            mlx_repeat_penalty_execution: config.mlx_repeat_penalty_execution,
            paged_attn: cli.paged_attn || config.paged_attn.unwrap_or(false),
            planning_enabled: config.planning_enabled.unwrap_or(true),
            lmstudio_url: config.lmstudio_url.clone(),
            pa_memory_mb: cli.pa_memory_mb.or(config.pa_memory_mb),
            vram_time_sharing: config.vram_time_sharing.unwrap_or(false),
            ollama_remote: {
                if cli.cloud {
                    let mut remote = config.ollama_remote.clone();
                    if let Some(r) = &mut remote {
                        r.enabled = Some(true);
                    } else {
                        println!(
                            "{} {} is set but no ollama_remote block in config.toml",
                            "⚠️".yellow(),
                            "--cloud".blue()
                        );
                    }
                    remote
                } else {
                    None
                }
            },
            tool_engine: config.tool_engine.clone().unwrap_or_else(|| "legacy".to_string()),
        },
    )
    .await?;

    if cli.web || cli.mcp_server {
        *agent.tool_rx.lock().await = Some(tool_rx_internal);
    }

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
    let _ = agent
        .initialize_mcp(config.mcp_servers.unwrap_or_default())
        .await;
    let _ = agent.resume_session().await;

    // Explicitly set safe mode from CLI flag
    if cli.safe {
        agent.set_safe_mode(true);
    }
    if !cli.cli && !cli.web && !cli.mcp_server {
        let agent_init = agent.clone();
        let backend_id = if cli.bridge {
            "bridge".to_string()
        } else if cli.lmstudio {
            "lmstudio".to_string()
        } else if cli.kalosm {
            "kalosm".to_string()
        } else if cli.mlx {
            "mlx".to_string()
        } else {
            "ollama".to_string()
        };
        let final_nexus_port = nexus_port;
        let agent_nexus = agent_init.clone();
        tokio::spawn(async move {
            tempest_ai::nexus::run_nexus(agent_nexus, final_nexus_port, backend_id, None, None)
                .await;
        });
        tokio::spawn(async move {
            let _ = agent_init.initialize_atlas(false).await;
            let _ = agent_init.warmup().await;
        });
    } else if !cli.cli {
        let agent_init = agent.clone();
        tokio::spawn(async move {
            let _ = agent_init.initialize_atlas(false).await;
            let _ = agent_init.warmup().await;
        });
    }
    if !cli.cli && !cli.mcp_server {
        println!("{} Launching TUI...", "🚀".green());
    }

    if cli.web {
        println!("{} Launching Tempest Nexus...", "🌐".green());
        let backend_id = if cli.bridge {
            "bridge"
        } else if cli.lmstudio {
            "lmstudio"
        } else if cli.kalosm {
            "kalosm"
        } else if cli.mlx {
            "mlx"
        } else if cli.gemini {
            "gemini"
        } else {
            "ollama"
        };
        let agent_clone = agent.clone();

        tokio::select! {
            _ = tempest_ai::nexus::run_nexus(agent, nexus_port, backend_id.to_string(), event_rx, tool_tx_opt) => {}
            _ = tokio::signal::ctrl_c() => {
                println!("\n{} Shutting down Nexus...", "🛑".red());
                agent_clone.print_interaction_summary();
            }
        }
        return Ok(());
    }

    if cli.mcp_server {
        if let Some(tx) = event_tx.lock().clone() {
            let collector = tempest_ai::telemetry::TelemetryCollector::new(tx);
            tokio::spawn(async move {
                collector.run().await;
            });
        }
        let mut server = tempest_ai::mcp_server::McpServer::new(agent, event_rx);
        if let Err(e) = server.run().await {
            eprintln!("MCP Server error: {}", e);
        }
        return Ok(());
    }

    if cli.cli {
        run_cli_mode(agent.clone()).await?;
        agent.print_interaction_summary();
        return Ok(());
    }

    // Default to TUI mode
    let (user_tx, user_rx) = tokio::sync::mpsc::channel(32);
    let (agent_tx, agent_rx) = tokio::sync::mpsc::channel(10000);
    let (tool_tx, tool_rx) = tokio::sync::mpsc::channel::<tempest_ai::tui::ToolResponse>(1);

    let stop_flag = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let stop_flag_agent = stop_flag.clone();

    let (_telemetry_actor, _actor_handle) = ractor::Actor::spawn(
        Some("telemetry_actor".to_string()),
        tempest_ai::actors::TelemetryActor,
        tempest_ai::actors::TelemetryArgs {
            agent_tx: agent_tx.clone(),
            shared_telemetry: agent.telemetry.clone(),
            mode: agent.mode,
        },
    )
    .await
    .expect("Failed to spawn TelemetryActor");

    *agent.event_tx.lock() = Some(agent_tx.clone());
    *agent.tool_rx.lock().await = Some(tool_rx);

    {
        let current_model = agent.get_model();
        let status_msg = match agent.mode {
            tempest_ai::inference::AgentMode::Gemini => format!("🟢 Connected: {}", current_model),
            tempest_ai::inference::AgentMode::MLX => {
                format!("🟢 MLX Engine Loaded: {}", current_model)
            }
            tempest_ai::inference::AgentMode::Kalosm => {
                format!("🟢 Kalosm Loaded: {}", current_model)
            }
            tempest_ai::inference::AgentMode::LMStudio => {
                format!("🟢 LM Studio Connected: {}", current_model)
            }
            tempest_ai::inference::AgentMode::Ollama => format!("🟢 Connected: {}", current_model),
            _ => format!("🟢 Connected: {}", current_model),
        };
        let _ = agent_tx
            .send(tempest_ai::tui::AgentEvent::SubagentStatus(Some(
                status_msg,
            )))
            .await;
    }

    let agent_tui = agent.clone();
    tokio::spawn(async move {
        if let Err(e) = agent_tui.run_tui_mode(user_rx, stop_flag_agent).await {
            let _ = agent_tx
                .send(tempest_ai::tui::AgentEvent::AgentError(format!(
                    "Agent crashed: {}",
                    e
                )))
                .await;
        }
    });

    let initial_theme = config
        .tui_theme
        .clone()
        .unwrap_or_else(|| "base16-ocean.dark".to_string());

    if let Err(e) =
        tempest_ai::tui::run_tui(agent_rx, user_tx, tool_tx, stop_flag, initial_theme).await
    {
        println!("{}", format!("TUI Render Error: {}", e).red());
    }

    // KILL SWITCH: Signal Ollama to unload the model from VRAM immediately on exit
    agent.shutdown().await;

    agent.print_interaction_summary();

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
                if p.is_empty() {
                    continue;
                }
                let _ = rl.add_history_entry(p);

                if p == "/quit" || p == "/exit" {
                    break;
                }
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
                    println!("  /tool        — Test a tool directly (usage: /tool <name> <json>)");
                    println!("  /quit        — Exit Tempest");
                    continue;
                }

                if p.starts_with("/tool ") {
                    let parts: Vec<&str> = p.splitn(3, ' ').collect();
                    if parts.len() >= 2 {
                        let tool_name = parts[1];
                        let args_str = if parts.len() == 3 { parts[2] } else { "{}" };
                        match serde_json::from_str::<serde_json::Value>(args_str) {
                            Ok(json_args) => {
                                if let Some(tool) = agent.get_tool_by_name(tool_name) {
                                    let ctx = agent.get_tool_context().await;
                                    match tool.execute(&json_args, ctx).await {
                                        Ok(msg) => {
                                            println!("{} {}", "🛠️ Tool Success:".green(), msg)
                                        }
                                        Err(e) => println!("{} {}", "⚠️ Tool Error:".red(), e),
                                    }
                                } else {
                                    println!(
                                        "{} Tool '{}' not found in registry.",
                                        "⚠️".yellow(),
                                        tool_name
                                    );
                                }
                            }
                            Err(e) => println!("{} Invalid JSON arguments: {}", "⚠️".red(), e),
                        }
                    } else {
                        println!("{} Usage: /tool <tool_name> <json_args>", "⚠️".yellow());
                    }
                    continue;
                }

                if let Err(e) = agent.run(p.to_string(), stop_flag.clone()).await {
                    println!("{} {}", "❌ Error:".red().bold(), e);
                }
            }
            Err(rustyline::error::ReadlineError::Interrupted)
            | Err(rustyline::error::ReadlineError::Eof) => break,
            Err(err) => {
                println!("Error: {:?}", err);
                break;
            }
        }
    }
    Ok(())
}
