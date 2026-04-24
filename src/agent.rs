use miette::{Result, miette};
use colored::*;
use ollama_rs::{
    generation::chat::{ChatMessage, MessageRole},
    models::ModelOptions,
    Ollama,
};
use serde_json::Value;
use syntect::parsing::SyntaxSet;
use syntect::highlighting::ThemeSet;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use parking_lot::Mutex;
use dashmap::DashMap;
use miette::IntoDiagnostic;
use std::path::Path;
use crate::tools::ToolContext;
use crate::memory::MemoryStore;
use crate::inference::{Backend, AgentMode};

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum AgentPhase {
    Planning,     // Strong reasoning model (DeepSeek R1)
    Execution,    // Fast & accurate coding model (Qwen2.5-Coder)
    Testing,      // Verification & error-catching model (Ministral 8B)
}

impl AgentPhase {
    pub fn default_model(&self) -> String {
        match self {
            AgentPhase::Planning => "deepseek-r1:14b".to_string(),
            AgentPhase::Execution => "qwen2.5-coder:14b".to_string(),
            AgentPhase::Testing => "ministral-3:8b".to_string(),
        }
    }

    pub fn description(&self) -> &'static str {
        match self {
            AgentPhase::Planning => "Planning Phase (Reasoning)",
            AgentPhase::Execution => "Execution Phase (Coding)",
            AgentPhase::Testing => "Testing Phase (Verification)",
        }
    }
}

struct PlannerOutput {
    content: String,
    native_tool_calls: Vec<ollama_rs::generation::tools::ToolCall>,
    /// Tool results that were already executed mid-stream (real-time approval)
    executed_mid_stream: Vec<(String, String, bool)>,
}

#[derive(Clone)]
pub struct Agent {
    #[allow(dead_code)]
    pub mode: AgentMode,
    pub phase: Arc<Mutex<AgentPhase>>,
    backend: Arc<Backend>,
    model: Arc<Mutex<String>>,
    history: Arc<Mutex<Vec<ChatMessage>>>,
    tools: Arc<DashMap<String, Arc<dyn crate::tools::AgentTool>>>,
    tool_registry: Vec<ollama_rs::generation::tools::ToolInfo>,
    system_prompt: String,
    recent_tool_calls: Arc<DashMap<String, String>>,
    history_path: String,
    brain_path: std::path::PathBuf,
    pub planning_mode: Arc<Mutex<bool>>,
    pub task_context: Arc<Mutex<String>>,
    pub vector_brain: Arc<Mutex<crate::vector_brain::VectorBrain>>,
    #[allow(dead_code)]
    pub sub_agent_model: String,
    #[allow(dead_code)]
    syntax_set: SyntaxSet,
    #[allow(dead_code)]
    theme_set: Arc<ThemeSet>,
    pub telemetry: Arc<Mutex<String>>,
    pub is_root: Arc<AtomicBool>,
    pub concurrency_semaphore: Arc<tokio::sync::Semaphore>,
    pub event_tx: Arc<Mutex<Option<tokio::sync::mpsc::Sender<crate::tui::AgentEvent>>>>,
    pub tool_rx: Arc<tokio::sync::Mutex<Option<tokio::sync::mpsc::Receiver<crate::tui::ToolResponse>>>>,
    pub rules: crate::rules::RuleEngine,
    pub recent_failures: Arc<DashMap<String, usize>>,
    pub sentinel: crate::sentinel::SentinelManager,
    pub tool_stats: Arc<DashMap<String, (usize, usize)>>,
}

impl Agent {
    pub async fn new(mode: AgentMode, model: String, quant: String, system_prompt: String, history_path: String, memory_store: Arc<Mutex<MemoryStore>>, sub_agent_model: String) -> Self {
        let event_tx = Arc::new(Mutex::new(None));
        let (backend, final_model) = Backend::new(mode, model, quant, event_tx.clone()).await;

        let tools_vec: Vec<Arc<dyn crate::tools::AgentTool>> = vec![
            Arc::new(crate::tools::file::ReadFileTool),
            Arc::new(crate::tools::file::WriteFileTool),
            Arc::new(crate::tools::file::ListDirTool),
            Arc::new(crate::tools::file::SearchFilesTool),
            Arc::new(crate::tools::file::AppendFileTool),
            Arc::new(crate::tools::file::PatchFileTool),
            Arc::new(crate::tools::file::FindReplaceTool),
            Arc::new(crate::tools::file::CreateDirectoryTool),
            Arc::new(crate::tools::file::DeleteFileTool),
            Arc::new(crate::tools::file::RenameFileTool),
            Arc::new(crate::tools::execution::RunCommandTool),
            Arc::new(crate::tools::execution::RunTestsTool),
            Arc::new(crate::tools::execution::BuildProjectTool),
            Arc::new(crate::tools::git::GitStatusTool),
            Arc::new(crate::tools::git::GitDiffTool),
            Arc::new(crate::tools::git::GitCommitTool),
            Arc::new(crate::tools::search::SemanticSearchTool),
            Arc::new(crate::tools::search::GrepSearchTool),
            Arc::new(crate::tools::memory::StoreMemoryTool::new(memory_store.clone())),
            Arc::new(crate::tools::memory::RecallMemoryTool::new(memory_store.clone())),
            Arc::new(crate::tools::agent_ops::AskUserTool),
            Arc::new(crate::tools::agent_ops::SpawnSubAgentTool),

            Arc::new(crate::tools::agent_ops::UpdateTaskContextTool),
            Arc::new(crate::tools::agent_ops::NoOpTool),
            Arc::new(crate::tools::telemetry::SystemTelemetryTool),
            Arc::new(crate::tools::network_manager::ListSocketsTool),
            Arc::new(crate::tools::service_manager::ListServicesTool),
            Arc::new(crate::tools::developer::InitializeRustProjectTool),
            Arc::new(crate::tools::web::SearchWebTool),
            Arc::new(crate::tools::web::ReadUrlTool),
            Arc::new(crate::tools::web::HttpRequestTool),
            Arc::new(crate::tools::web::DownloadFileTool),
            Arc::new(crate::tools::web::StockScraperTool),
            Arc::new(crate::tools::process::RunBackgroundTool),
            Arc::new(crate::tools::process::ReadProcessLogsTool),
            Arc::new(crate::tools::process::KillProcessTool),
            Arc::new(crate::tools::process::WatchDirectoryTool),
            Arc::new(crate::tools::utilities::ClipboardTool),
            Arc::new(crate::tools::utilities::NotifyTool),
            Arc::new(crate::tools::utilities::EnvVarTool),
            Arc::new(crate::tools::utilities::ChmodTool),
            Arc::new(crate::tools::utilities::CalculatorTool),
            Arc::new(crate::tools::knowledge::DistillKnowledgeTool),
            Arc::new(crate::tools::knowledge::SkillRecallTool),
            Arc::new(crate::tools::knowledge::RecallBrainTool),
            Arc::new(crate::tools::knowledge::ListSkillsTool),
            Arc::new(crate::tools::agent_ops::QuerySchemaTool),
            Arc::new(crate::tools::database::SqliteQueryTool),
            Arc::new(crate::tools::network::NetworkCheckTool),
            Arc::new(crate::tools::atlas::TreeTool),
            Arc::new(crate::tools::atlas::ProjectAtlasTool),
            Arc::new(crate::tools::file::ExtractAndWriteTool),
            Arc::new(crate::tools::git::GitActionTool),
            Arc::new(crate::tools::search::IndexFileSemanticallyTool),
            Arc::new(crate::tools::memory::MemorySearchTool::new(memory_store.clone())),
            Arc::new(crate::tools::system::SystemdManagerTool),
            Arc::new(crate::tools::system::CurrentProcessTool),
            Arc::new(crate::tools::system::SystemTelemetryTool),
            Arc::new(crate::tools::privilege::RequestPrivilegesTool),
            Arc::new(crate::tools::rust::CargoAddTool),
            Arc::new(crate::tools::rust::CrateSearchTool),
        ];

        let tools_map = Arc::new(DashMap::new());
        for t in &tools_vec {
            tools_map.insert(t.name().to_string(), t.clone());
        }

        let history_path_obj = Path::new(&history_path);
        let brain_path = history_path_obj.parent().unwrap_or(Path::new(".")).join("brain_vectors.json");
        let tool_registry: Vec<ollama_rs::generation::tools::ToolInfo> = tools_vec.iter().map(|t| t.tool_info()).collect();

        let vector_brain = Arc::new(Mutex::new(crate::vector_brain::VectorBrain::load_from_disk(&brain_path)
            .unwrap_or_else(|_| crate::vector_brain::VectorBrain::new())));

        Agent {
            mode,
            phase: Arc::new(Mutex::new(AgentPhase::Planning)),
            backend: Arc::new(backend),
            model: Arc::new(Mutex::new(final_model)),
            history: Arc::new(Mutex::new(vec![])),
            tools: tools_map,
            tool_registry,
            system_prompt,
            recent_tool_calls: Arc::new(DashMap::new()),
            history_path,
            brain_path,
            planning_mode: Arc::new(Mutex::new(true)),
            task_context: Arc::new(Mutex::new("Not started yet.".to_string())),
            vector_brain,
            sub_agent_model,
            syntax_set: SyntaxSet::load_defaults_newlines(),
            theme_set: Arc::new(ThemeSet::load_defaults()),
            rules: crate::rules::RuleEngine::new(),
            recent_failures: Arc::new(DashMap::new()),
            telemetry: Arc::new(Mutex::new(String::new())),
            is_root: Arc::new(AtomicBool::new(nix::unistd::getuid().is_root())),
            concurrency_semaphore: Arc::new(tokio::sync::Semaphore::new(5)),
            event_tx,
            tool_rx: Arc::new(tokio::sync::Mutex::new(None)),
            sentinel: crate::sentinel::SentinelManager::new(),

            tool_stats: Arc::new(DashMap::new()),
        }
    }

    pub fn get_ollama(&self) -> Result<&Ollama> {
        match &*self.backend {
            Backend::Ollama(o) => Ok(o),
            #[cfg(feature = "mlx")]
            Backend::MLX(_) => Err(miette!("Active backend is MLX, not Ollama")),
        }
    }

    pub async fn initialize_atlas(&self, force: bool) -> Result<()> {
        if let Ok(ollama) = self.get_ollama() {
            crate::tools::atlas::run_semantic_indexing(
                ollama, 
                self.vector_brain.clone(), 
                &self.brain_path, 
                force
            ).await
        } else {
            Ok(())
        }
    }
    
    fn calculate_optimal_ctx(&self) -> u64 {
        let model = self.model.lock().to_lowercase();
        
        // If we are using MLX, the current config is locked to 16,384 for performance/VRAM safety on 16GB Macs.
        if matches!(&*self.backend, Backend::MLX(_)) {
            return 16384;
        }

        if model.contains("20b") || model.contains("27b") || model.contains("30b") || model.contains("deepseek-r1:32b") {
            2048
        } else if model.contains("13b") || model.contains("16b") || model.contains("12b") {
            4096
        } else if model.contains("14b") {
            12288
        } else if model.contains("7b") || model.contains("8b") || model.contains("9b") {
            32768
        } else {
             12288
        }
    }

    pub async fn check_connection(&self) -> Result<()> {
        if let Ok(ollama) = self.get_ollama() {
            let models = ollama.list_local_models().await.into_diagnostic()?;
            let model_names: std::collections::HashSet<String> = models.into_iter().map(|m| m.name).collect();

            let required = vec![
                AgentPhase::Planning.default_model(),
                AgentPhase::Execution.default_model(),
                AgentPhase::Testing.default_model(),
            ];

            for req in required {
                if !model_names.contains(&req) {
                    return Err(miette!("Required model '{}' not found in Ollama. Please run: ollama pull {}", req, req));
                }
            }
        }
        Ok(())
    }

    /// Injects a high-priority state message into the context to ensure the model knows its current boundaries.
    fn inject_state_context(&self) {
        let is_planning = *self.planning_mode.lock();
        
        let mode_str = if is_planning { "PLANNING MODE (Read-only research & architecture)" } else { "EXECUTION MODE (Full autonomy granted)" };

        // --- 🧠 COMPETENCY REPORTING ---
        let mut competency_warnings = Vec::new();
        for item in self.tool_stats.iter() {
            let (name, (s, f)) = (item.key(), item.value());
            let total = s + f;
            if total >= 3 && *f > *s {
                competency_warnings.push(format!("- {}: High failure rate ({}/{} failed). REASON: Likely incorrect arguments or path assumptions. BE MORE PRECISE.", name, f, total));
            }
        }

        let competency_str = if competency_warnings.is_empty() {
            "".to_string()
        } else {
            format!("\n### COMPETENCY WARNINGS ###\n{}\n", competency_warnings.join("\n"))
        };

        let state_msg = format!(
            "### TEMPEST INTERNAL STATE ###\n- MODE: {}\n- DIRECTIVE: You have FULL AUTONOMY to execute tools. Write code using write_file tool calls ONLY. Never dump raw code blocks into chat.{}\n- ADVISORY: If you see high failure rates, stop and verify your assumptions using read_file or list_dir before retrying.",
            mode_str, competency_str
        );

        let mut h_lock = self.history.lock();
        // Remove old state message if it exists to keep context clean
        h_lock.retain(|m| !m.content.starts_with("### TEMPEST INTERNAL STATE ###"));
        h_lock.push(ChatMessage::new(MessageRole::System, state_msg));
    }

    pub async fn switch_phase(&self, new_phase: AgentPhase) -> Result<()> {
        let mut p_lock = self.phase.lock();
        if *p_lock == new_phase {
            return Ok(());
        }

        let old_desc = p_lock.description();
        *p_lock = new_phase.clone();
        *self.model.lock() = new_phase.default_model();

        // Notify TUI
        let tx_opt = self.event_tx.lock().clone();
        if let Some(tx) = tx_opt {
            let _ = tx.try_send(crate::tui::AgentEvent::SystemUpdate(
                format!("🔄 Switched from {} -> {}", old_desc, new_phase.description())
            ));
        }

        // Save history to ensure current state is persisted
        let _ = self.save_history();
        
        Ok(())
    }

    pub fn load_history(&self) -> Result<()> {
        let history_path = std::path::Path::new(&self.history_path);
        if history_path.exists() {
            let data = std::fs::read_to_string(history_path).into_diagnostic()?;
            if let Ok(history) = serde_json::from_str::<Vec<ChatMessage>>(&data) {
                let mut h_lock = self.history.lock();
                for msg in history {
                    if msg.role != MessageRole::System {
                        h_lock.push(msg);
                    }
                }
            }
        }
        Ok(())
    }

    pub fn save_history(&self) -> Result<()> {
        let history = self.history.lock();
        let path = std::path::Path::new(&self.history_path);
        let file = std::fs::File::create(path).into_diagnostic()?;
        serde_json::to_writer_pretty(file, &*history).into_diagnostic()?;
        Ok(())
    }

    /// Helper to push a structured tool result back to the model history and TUI.
    pub async fn send_tool_feedback(&self, tool_name: &str, result: &str, is_success: bool) -> Result<()> {
        let hud_msg = if is_success {
            format!("✅ SUCCESS: '{}' executed", tool_name)
        } else {
            format!("❌ ERROR: '{}' failed", tool_name)
        };

        // Update TUI HUD
        let tx_opt = self.event_tx.lock().clone();
        if let Some(tx) = tx_opt {
            let _ = tx.try_send(crate::tui::AgentEvent::SystemUpdate(hud_msg));
        }

        let feedback = if is_success {
            format!(
                "=== TOOL RESULT ===\nTool: {}\nResult: {}\n\nYou MUST use the information above exactly as shown. Do not override it with your own knowledge or guess versions.",
                tool_name, result
            )
        } else {
            format!(
                "=== TOOL ERROR ===\nTool: {}\nError: {}\n\nPlease analyze this error carefully and adjust your strategy. Do NOT repeat the same mistake.",
                tool_name, result
            )
        };

        self.history.lock().push(ChatMessage::new(MessageRole::System, feedback));
        self.save_history()?;
        Ok(())
    }

    pub fn clear_history(&self) {
        self.history.lock().clear();
        let _ = std::fs::remove_file(&self.history_path);
    }

    pub async fn run(&self, initial_user_prompt: String, stop: std::sync::Arc<std::sync::atomic::AtomicBool>) -> Result<()> {
        if initial_user_prompt.trim() == "/clear" {
            self.clear_history();
            return Ok(());
        }
        if self.event_tx.lock().is_none() {
            println!("{}", "=".repeat(60).blue());
            println!("{} {}", "🚀".green(), "Tempest AI Agent Initialized".bold());
            println!("{} {}", "Model:".blue(), *self.model.lock());
            println!("{}", "=".repeat(60).blue());
        }

        let active_rules = self.rules.get_active_rules(&[]); // Empty for now, will refine
        
        {
            let mut h_lock = self.history.lock();
            let mut full_system_prompt = self.system_prompt.clone();
            
            if !active_rules.is_empty() {
                full_system_prompt.push_str("\n\n[ACTIVE PROJECT RULES]\n");
                for rule in active_rules {
                    full_system_prompt.push_str(&format!("### Rule: {}\n{}\n\n", rule.name, rule.content));
                }
            }

            full_system_prompt.push_str("\n\n[TOOL SCHEMA]\n");
            // Use dense JSON instead of pretty to eliminate thousands of whitespace tokens
            if let Ok(schema_json) = serde_json::to_string(&self.tool_registry) {
                full_system_prompt.push_str(&schema_json);
            }
            

            if let Some(pos) = h_lock.iter().position(|m| m.role == MessageRole::System) {
                h_lock[pos] = ChatMessage::new(MessageRole::System, full_system_prompt);
            } else {
                h_lock.insert(0, ChatMessage::new(MessageRole::System, full_system_prompt));
            }

            // --- 🧹 CONTEXT INERTIA BREAKER ---
            // If the user manually inputs a new command, it means they are pivoting.
            // We prune unresolved tool errors and hanging thoughts from the end of the history.
            while let Some(last) = h_lock.last() {
                if last.role == MessageRole::User && (
                    last.content.starts_with("ERROR: Tool") || 
                    last.content.starts_with("SYSTEM NOTIFICATION:") ||
                    last.content.starts_with("BLOCKED:") ||
                    last.content.contains("ACTION REQUIRED")
                ) {
                    h_lock.pop();
                } else {
                    break;
                }
            }

            // Reset failure counters to prevent legacy errors tracking into the new topic
            self.recent_failures.clear();

            let safe_prompt = if h_lock.len() > 1 {
                format!(
                    "### ⚠️ USER OVERRIDE DIRECTIVE ###\nThe user has explicitly submitted a new command/topic. YOU MUST completely abandon any previous uncompleted tool loops, errors, or planning states. Pivot your absolute focus to fulfilling this new request.\n\nNEW USER REQUEST:\n{}",
                    initial_user_prompt
                )
            } else {
                initial_user_prompt
            };

            h_lock.push(ChatMessage::new(MessageRole::User, safe_prompt));
        }
        let _ = self.save_history();
        
        // Reset thinking just in case
        let tx_opt = self.event_tx.lock().clone();
        if let Some(tx) = tx_opt {
            let _ = tx.try_send(crate::tui::AgentEvent::Thinking(None));
        }

        let max_iterations = 30;
        let mut iteration_count = 0;
        let mut reprimand_issued = false;

        loop {
            // --- STAGE 0: STATE INJECTION ---
            self.inject_state_context();

            if stop.load(std::sync::atomic::Ordering::Relaxed) {
                let tx_opt = self.event_tx.lock().clone();
                if let Some(tx) = tx_opt {
                    let _ = tx.send(crate::tui::AgentEvent::SystemUpdate("🛑 INTERRUPTED: Turn cancelled by user.".to_string())).await;
                    let _ = tx.send(crate::tui::AgentEvent::Thinking(None)).await;
                }
                break;
            }
            iteration_count += 1;
            if iteration_count > max_iterations {
                if self.event_tx.lock().is_none() {
                    println!("\n{}", "🛑 Execution limit reached (30 turns). Stopping.".red());
                }
                break;
            }

            // --- STAGE 1: SENTINEL FLEET CHECK ---
            let ctx_limit = self.calculate_optimal_ctx();
            let action_opt = {
                self.sentinel.analyze_state(&self.history.lock(), ctx_limit)
            };

            if let Some(action) = action_opt {
                let tx_opt = self.event_tx.lock().clone();
                if let Some(tx) = tx_opt {
                    // Update the HUD
                    let _ = tx.try_send(crate::tui::AgentEvent::SentinelUpdate { 
                        active: action.active_sentinels.clone(), 
                        log: action.message.clone() 
                    });
                    
                    if !action.message.is_empty() {
                        let _ = tx.try_send(crate::tui::AgentEvent::SystemUpdate(action.message.clone()));
                    }
                }

                if action.needs_compaction {
                    let history_to_compact = self.history.lock().clone();
                    let before_count = crate::context_manager::estimate_tokens(&history_to_compact);
                    
                    let new_history = crate::context_manager::compact_history(
                        self.get_ollama().unwrap_or(&Ollama::default()), 
                        &self.sub_agent_model, 
                        history_to_compact, 
                        ctx_limit
                    ).await?;
                    
                    let after_count = crate::context_manager::estimate_tokens(&new_history);
                    
                    *self.history.lock() = new_history;
                    let _ = self.save_history();

                    let tx_opt = self.event_tx.lock().clone();
                    if let Some(tx) = tx_opt {
                        let _ = tx.try_send(crate::tui::AgentEvent::SystemUpdate(format!(
                            "🌪️ [CONTEXT COMPACTION]: Successfully condensed history ({} -> {} tokens)",
                            before_count, after_count
                        )));
                    }
                }

                if action.needs_privilege {
                    let mut h_lock = self.history.lock();
                    h_lock.push(ChatMessage::new(MessageRole::System, "⚠️ [SENTINEL]: You are attempting to access a protected area. If this is required, you MUST use 'request_privileges' or prefix the command with sudo.".to_string()));
                }
            }

            // --- STAGE 1: PLANNING ---
            self.switch_phase(AgentPhase::Planning).await?;
            let planner_output = self.planner_turn(stop.clone()).await?;
            
            // --- STAGE 2: EXTRACTION ---
            let mut all_tool_calls = Vec::new();
            for native_call in planner_output.native_tool_calls {
                all_tool_calls.push(serde_json::json!({
                    "tool": native_call.function.name,
                    "arguments": native_call.function.arguments,
                }));
            }

            if all_tool_calls.is_empty() {
                if let Ok(legacy_calls) = self.extract_tool_calls(&planner_output.content) {
                    all_tool_calls.extend(legacy_calls);
                }
            }

            // Merge: mid-stream results are already done; only dispatch remaining tools
            let mid_stream_results = planner_output.executed_mid_stream;
            let executed_names: std::collections::HashSet<String> = mid_stream_results.iter()
                .map(|(name, _, _)| name.clone())
                .collect();
            let has_mid_stream = !executed_names.is_empty();
            
            // Filter out tools that were already executed mid-stream
            let remaining_tool_calls: Vec<_> = if has_mid_stream {
                all_tool_calls.into_iter().filter(|tc| {
                    let name = tc.get("name").or_else(|| tc.get("tool"))
                        .and_then(|v| v.as_str()).unwrap_or("unknown");
                    !executed_names.contains(name)
                }).collect()
            } else {
                all_tool_calls
            };

            if remaining_tool_calls.is_empty() && !has_mid_stream {
                // --- 🛡️ WATCHDOG: Detect silent completion after tool result ---
                let needs_reprimand = {
                    let h_lock = self.history.lock();
                    let last_msg_is_tool_result = h_lock.last()
                        .map(|m| m.role == MessageRole::User && m.content.contains("SYSTEM NOTIFICATION: TOOL RESULT"))
                        .unwrap_or(false);
                    
                    let content_is_weak = {
                        let trimmed = planner_output.content.trim();
                        let lower = trimmed.to_lowercase();
                        trimmed.is_empty() || (lower.contains("thought") && trimmed.len() < 60)
                    };
                    
                    last_msg_is_tool_result && content_is_weak
                };

                if needs_reprimand && !reprimand_issued {
                    reprimand_issued = true;
                    let reprimand = "CRITICAL REPRIMAND: You just received data from the system but you FAILED to report it to the human! DO NOT call another tool. Analyze the tool results in your history and give the final answer to the user NOW. You MUST provide a summary.".to_string();
                    
                    let tx_opt = self.event_tx.lock().clone();
                    if let Some(tx) = tx_opt {
                        let _ = tx.try_send(crate::tui::AgentEvent::SystemUpdate("Watchdog: Forcing response...".to_string()));
                    }

                    self.history.lock().push(ChatMessage::new(MessageRole::User, reprimand));
                    let _ = self.save_history();
                    continue; // Force another turn
                }

                if !planner_output.content.is_empty() {
                    if self.event_tx.lock().is_none() {
                        println!("\n{} {}", "✅".green(), "Turn complete.".dimmed());
                    }
                }
                break;
            }

            // --- STAGE 3: EXECUTION (remaining non-modifying tools) ---
            let mut results = mid_stream_results; // Start with already-executed mid-stream tools
            if !remaining_tool_calls.is_empty() {
                self.switch_phase(AgentPhase::Execution).await?;
                let dispatch_results = self.executor_dispatch(remaining_tool_calls).await?;
                results.extend(dispatch_results);
            }

            // --- STAGE 4: COLLECTION & STRUCTURED FEEDBACK ---
            self.switch_phase(AgentPhase::Testing).await?;
            let mut detected_loop_key = None;
            let mut feedback_to_apply = Vec::new();

            for (tool_name, result, is_success) in results {
                let (formatted_res, hud_msg) = if is_success { 
                    // Reset failure counters on any success
                    self.recent_failures.remove(&tool_name);
                    self.recent_failures.remove("GENERIC_FILE_NOT_FOUND");
                    
                    if result.starts_with("BLOCKED:") {
                        (
                            format!("SYSTEM NOTIFICATION: TOOL BLOCKED for {}:\n{}\n\nACTION REQUIRED: You MUST propose a plan and ask for approval via 'ask_user' before this tool can be used.", tool_name, result),
                            format!("🚫 BLOCKED: '{}' - Plan requires approval", tool_name)
                        )
                    } else {
                        let is_modifying = self.tools.get(&tool_name).map(|t| t.is_modifying()).unwrap_or(false);
                        let mut base_res = format!("SUCCESS: Tool '{}' executed.\nRESULT:\n{}", tool_name, result);
                        
                        if is_modifying {
                            base_res.push_str("\n\n⚠️ SYSTEM DIRECTIVE: You have just modified a physical file. Before doing ANYTHING else, you MUST rigorously test and verify your changes. If it is code, run it or its tests using 'run_command'. Do NOT assume your modifications work correctly. Verify them immediately and report the outcome.");
                        }

                        (
                            base_res,
                            format!("✅ SUCCESS: '{}' executed", tool_name)
                        )
                    }
                } else { 
                    let fail_key = if result.contains("os error 2") || result.contains("No such file") || result.contains("No files found") {
                        "GENERIC_FILE_NOT_FOUND".to_string()
                    } else {
                        format!("{}:{}", tool_name, result.chars().take(50).collect::<String>())
                    };

                    let count = *self.recent_failures.entry(fail_key.clone()).and_modify(|c| *c += 1).or_insert(1);
                    if count >= 3 {
                        detected_loop_key = Some(fail_key);
                    }

                    (
                        format!("ERROR: Tool '{}' failed.\nREASON:\n{}", tool_name, result),
                        format!("❌ ERROR: '{}' failed", tool_name)
                    )
                };
                
                feedback_to_apply.push((tool_name, formatted_res, hud_msg, is_success));
            }

            for (tool_name, formatted_res, _hud_msg, is_success) in feedback_to_apply {
                // Use the structured feedback helper for each result
                let _ = self.send_tool_feedback(&tool_name, &formatted_res, is_success).await;
            }
            let _ = self.save_history();

            if let Some(key) = detected_loop_key {
                let mut h_lock = self.history.lock();
                let directive = format!(
                    "\n\n⚠️ [SENTINEL REORIENTATION DIRECTIVE]:\nYou have encountered a pattern of missing resources: '{}'.\nYour current strategy is LOOPING and HALLUCINATING files that do NOT exist.\n1. STOP this path immediately.\n2. I am forcing a 'list_dir' for you below. Study it carefully.\n3. Do NOT attempt to access any file not explicitly listed in the output below.\n4. Acknowledge this directive and apologize for the hallucination loop.",
                    key
                );
                h_lock.push(ChatMessage::new(MessageRole::System, directive));
                
                // --- AUTO-RESYNC: Run an ls and inject it immediately ---
                if let Ok(entries) = std::fs::read_dir(".") {
                    let mut files = vec![];
                    for e in entries.flatten() {
                        files.push(e.file_name().to_string_lossy().to_string());
                    }
                    let resync = format!("SENTINEL FORCED SYNC (Current Directory Contents):\n- {}", files.join("\n- "));
                    h_lock.push(ChatMessage::new(MessageRole::User, resync));
                }

                self.recent_failures.clear(); // Reset after intervention
            }
            
            let _ = self.save_history();
        }
        
        // Ensure final state is persisted even if loop broke early
        let _ = self.save_history();
        Ok(())
    }

    async fn planner_turn(&self, stop: std::sync::Arc<std::sync::atomic::AtomicBool>) -> Result<PlannerOutput> {
        let ctx_limit = self.calculate_optimal_ctx();
        let is_planning = *self.planning_mode.lock();
        let temp = if is_planning { 0.1 } else { 0.4 };

        let options = ModelOptions::default()
            .num_ctx(ctx_limit)
            .num_predict(8192)
            .temperature(temp);

        let mut history_snapshot = self.history.lock().clone();

        // --- PHASE 3: TOKEN BUDGET AWARENESS ---
        let ctx_limit = self.calculate_optimal_ctx();
        let used = crate::context_manager::estimate_tokens(&history_snapshot);
        
        let tx_opt = self.event_tx.lock().clone();
        if let Some(tx) = tx_opt {
            let _ = tx.send(crate::tui::AgentEvent::ContextStatus { used, total: ctx_limit }).await;
        }

        let runway_report = crate::context_manager::generate_runway_report(used, ctx_limit);
        history_snapshot.push(ChatMessage::new(MessageRole::System, runway_report));

        let pos = history_snapshot.len().saturating_sub(2); // Insert before the directive
        history_snapshot.insert(pos, ChatMessage::new(
            MessageRole::System,
            "CRITICAL RULES REMINDER (re-read every turn):\n\
             1. Begin with THOUGHT: always. No preamble, no 'Sure', no 'Here is'.\n\
             2. ALL code MUST go through the `write_file` tool. NEVER output ```rust or ```python blocks into chat. That does NOT save files.\n\
             3. Your tool call MUST be valid JSON inside a ```json block with keys 'name' and 'arguments'.\n\
             4. After writing code, IMMEDIATELY use `run_command` to test/compile it. Do not ask the user if they want you to verify.\n\
             5. Do NOT ask the user how you can help. Execute the next step of your plan autonomously.\n\
             6. If you just received tool results, analyze them and take the next action. Do NOT stop to summarize unless the task is DONE.".to_string()
        ));
        
        let executed_mid_stream: Vec<(String, String, bool)> = Vec::new();

        // Transition to Execution phase for inference
        *self.phase.lock() = AgentPhase::Execution;

        let model_name = self.model.lock().clone();
        let output = self.backend.stream_chat(
            model_name,
            history_snapshot,
            options,
            self.event_tx.clone(),
            stop,
            self.system_prompt.clone(),
        ).await?;

        let full_content = output.content;
        let _reasoning_content = output.reasoning;
        let native_tool_calls = output.native_tool_calls;
        let tx_opt = self.event_tx.lock().clone();
        if let Some(tx) = tx_opt {
            let _ = tx.send(crate::tui::AgentEvent::StreamToken("".to_string())).await;
        }
        if self.event_tx.lock().is_none() {
            println!();
        }

        if !full_content.trim().is_empty() || !native_tool_calls.is_empty() {
            let mut stored_content = full_content.clone();
            if !native_tool_calls.is_empty() && stored_content.is_empty() {
                stored_content = "THOUGHT: I am executing a structural tool call.".to_string();
            }
            let mut msg = ChatMessage::new(MessageRole::Assistant, stored_content);
            msg.tool_calls = native_tool_calls.clone();
            self.history.lock().push(msg);
        }

        Ok(PlannerOutput {
            content: full_content,
            native_tool_calls,
            executed_mid_stream,
        })
    }

    async fn executor_dispatch(&self, tool_calls: Vec<Value>) -> Result<Vec<(String, String, bool)>> {
        let mut results = Vec::new();
        for tool_req in tool_calls {
            let tool_name = tool_req.get("name").or_else(|| tool_req.get("tool")).and_then(|v| v.as_str()).unwrap_or("unknown");
            let is_modifying = self.tools.get(tool_name).map(|t| t.is_modifying()).unwrap_or(false);
            
            let result = self.process_single_tool_call(tool_req).await;
            results.push(result);
            
            if is_modifying {
                // BREAK AFTER ONE MODIFYING TOOL
                // This ensures the AI sees the result of a write/patch before continuing
                break;
            }
        }
        Ok(results)
    }

    async fn process_single_tool_call(&self, tool_req: Value) -> (String, String, bool) {
        let tool_name = tool_req.get("tool")
            .or_else(|| tool_req.get("action"))
            .or_else(|| tool_req.get("name"))
            .or_else(|| tool_req.get("function_name"))
            .or_else(|| tool_req.get("function"))
            .and_then(|v| v.as_str())
            .unwrap_or("unknown")
            .to_string();

        // 🧠 FUZZY TOOL REPAIR
        let tool_name = match tool_name.to_lowercase().as_str() {
            "ask" | "ask_user_input" | "prompt_user" | "user_input" => "ask_user".to_string(),
            "stock_price" | "get_stock" | "check_stock" | "stock" => "get_stock_price".to_string(),
            "search" | "google_search" | "web_search" => "search_web".to_string(),
            "read" | "fetch_url" | "web_read" => "read_url".to_string(),
            "recall" | "recall_knowledge" | "memory" | "brain" => "recall_brain".to_string(),
            "distill" | "save_knowledge" | "save_brain" => "distill_knowledge".to_string(),
            "shell" | "exec" | "command" => "run_command".to_string(),
            "notify" | "alert" | "status" => "no_op".to_string(),
            _ => tool_name,
        };
            
        let mut args = tool_req.get("arguments")
            .or_else(|| tool_req.get("args"))
            .or_else(|| tool_req.get("params"))
            .or_else(|| tool_req.get("parameters"))
            .cloned()
            .unwrap_or(Value::Null);

        // 🧠 FUZZY ARGUMENT REPAIR
        if tool_name == "get_stock_price" {
            if let Some(obj) = args.as_object_mut() {
                if obj.contains_key("symbol") && !obj.contains_key("ticker") {
                    if let Some(sym) = obj.remove("symbol") {
                        obj.insert("ticker".to_string(), sym);
                    }
                }
            }
        }

        if tool_name == "run_command" {
            if let Some(obj) = args.as_object_mut() {
                if obj.contains_key("command") && obj.contains_key("options") {
                    let cmd = obj.get("command").and_then(|v| v.as_str()).unwrap_or("");
                    let opts = obj.get("options").and_then(|v| v.as_str()).unwrap_or("");
                    if !opts.is_empty() {
                        obj.insert("command".to_string(), serde_json::json!(format!("{} {}", cmd, opts)));
                        obj.remove("options");
                    }
                }
            }
        }

        if let Some(tool) = self.tools.get(&tool_name).map(|r| r.value().clone()) {
            // --- TRANSPARENT APPROVAL GATE for modifying tools ---
            // The AI never knows this exists. If rejected, it sees a generic error.
            if tool.is_modifying() {
                let tx_opt = self.event_tx.lock().clone();
                if let Some(tx) = tx_opt {
                    let args_preview = serde_json::to_string(&args)
                        .unwrap_or_default()
                        .chars().take(100).collect::<String>();
                    let prompt = format!(
                        "APPROVE {} ({})? [ENTER/ESC]",
                        tool_name.to_uppercase(),
                        args_preview
                    );
                    let _ = tx.send(crate::tui::AgentEvent::RequestInput(
                        tool_name.clone(), 
                        prompt
                    )).await;

                    // Wait for user response through the existing tool_rx channel
                    let mut rx_lock = self.tool_rx.lock().await;
                    if let Some(rx) = rx_lock.as_mut() {
                        match tokio::time::timeout(
                            tokio::time::Duration::from_secs(300), // 5 min timeout
                            rx.recv()
                        ).await {
                            Ok(Some(crate::tui::ToolResponse::Text(ans))) => {
                                let lower = ans.trim().to_lowercase();
                                if lower != "y" && lower != "yes" {
                                    // User rejected — AI sees a generic error, not "BLOCKED"
                                    return (tool_name.clone(), 
                                        format!("Error: Tool '{}' could not be executed at this time. Try a different approach.", tool_name), 
                                        false);
                                }
                                // User approved — fall through silently
                            }
                            _ => {
                                // Timeout or channel error — auto-reject for safety
                                return (tool_name.clone(),
                                    format!("Error: Tool '{}' timed out waiting for system resources.", tool_name),
                                    false);
                            }
                        }
                    }
                }
                // If no TUI (CLI mode), auto-approve
            }

            let mut last_result = (tool_name.clone(), "Error: Tool execution failed and could not be retried.".to_string(), false);
            let max_attempts = 3;

            for attempt in 1..=max_attempts {
                let start = std::time::Instant::now();
                metrics::gauge!("tool.semaphore_available_permits").set(self.concurrency_semaphore.available_permits() as f64);
                
                let _permit = self.concurrency_semaphore.acquire().await.ok();
                let context = self.get_tool_context();
                
                match tool.execute(&args, context).await {
                    Ok(res) => {
                        let duration = start.elapsed();
                        metrics::histogram!("tool.execution_ms", "tool" => tool_name.clone()).record(duration.as_millis() as f64);
                        
                        let result = (tool_name.to_string(), res, true);
                        self.recent_tool_calls.insert(tool_name.to_string(), result.1.chars().take(100).collect());
                        
                        // Increment success stats
                        self.tool_stats.entry(tool_name.to_string())
                            .and_modify(|(s, _)| *s += 1)
                            .or_insert((1, 0));
                            
                        return result;
                    }
                    Err(e) => {
                        let err_msg = format!("{}", e);
                        
                        // Increment failure stats on final attempt or if non-retryable
                        let classification = crate::error_classifier::classify_error(&tool_name, &err_msg);
                        if classification != crate::error_classifier::ErrorClass::Retryable || attempt == max_attempts {
                            self.tool_stats.entry(tool_name.to_string())
                                .and_modify(|(_, f)| *f += 1)
                                .or_insert((0, 1));
                        }

                        if classification == crate::error_classifier::ErrorClass::Retryable && attempt < max_attempts {
                            let wait_secs = 2u64.pow(attempt as u32 - 1);
                            let tx_opt = self.event_tx.lock().clone();
                            if let Some(tx) = tx_opt {
                                let _ = tx.try_send(crate::tui::AgentEvent::SystemUpdate(
                                    format!("🔄 Retrying {} ({}/{}) - Wait {}s: {}", tool_name, attempt, max_attempts, wait_secs, err_msg)
                                ));
                            }
                            tokio::time::sleep(tokio::time::Duration::from_secs(wait_secs)).await;
                            last_result = (tool_name.clone(), format!("Error (Failed after {} attempts): {}", attempt, err_msg), false);
                            continue;
                        } else {
                            last_result = (tool_name.to_string(), format!("Error: {}", err_msg), false);
                            break;
                        }
                    }
                }
            }

            last_result
        } else {
            (
                tool_name.to_string(), 
                format!(
                    "SYSTEM ADVISORY: Tool '{}' not found in registry. You likely hallucinated a capability. VALID ALTERNATIVES: 'read_file', 'grep_search', 'run_command', 'ask_user'. Verify the [TOOL SCHEMA] and try again.", 
                    tool_name
                ), 
                false
            )
        }
    }

    pub fn get_tool_context(&self) -> ToolContext {
        let (tx, _) = tokio::sync::mpsc::channel(1); // Placeholder for non-TUI runs

        let real_tx = match &*self.event_tx.lock() {
            Some(t) => t.clone(),
            None => tx,
        };

        ToolContext {
            ollama: self.get_ollama().unwrap_or(&Ollama::default()).clone(),
            model: self.model.lock().clone(),
            sub_agent_model: self.sub_agent_model.clone(),
            history: self.history.clone(),
            task_context: self.task_context.clone(),
            vector_brain: self.vector_brain.clone(),
            telemetry: self.telemetry.clone(),
            tx: real_tx,
            tool_rx: self.tool_rx.clone(),
            recent_tool_calls: self.recent_tool_calls.clone(),
            brain_path: self.brain_path.clone(),
            is_root: self.is_root.clone(),
            all_tools: self.tool_registry.clone(),
        }
    }

    // Removed auto_summarize_memory (unused)

    fn extract_tool_calls(&self, content: &str) -> Result<Vec<Value>, String> {
        let block_regex = regex::Regex::new(r"```json\s*([\s\S]*?)\s*```").unwrap();
        let mut calls = Vec::new();
        for caps in block_regex.captures_iter(content) {
            if let Some(m) = caps.get(1) {
                if let Ok(val) = serde_json::from_str::<Value>(m.as_str().trim()) {
                    if let Some(obj) = val.as_object() {
                        if obj.contains_key("tool") || obj.contains_key("name") || obj.contains_key("function_name") || obj.contains_key("function") {
                            calls.push(val);
                        }
                    }
                }
            }
        }
        Ok(calls)
    }

    pub async fn run_tui_mode(&self, mut user_rx: tokio::sync::mpsc::Receiver<String>, stop: std::sync::Arc<std::sync::atomic::AtomicBool>) -> Result<()> {
        while !stop.load(std::sync::atomic::Ordering::Relaxed) || true { // We want the TUI to stay alive even if stop was true
             if let Ok(user_msg) = user_rx.try_recv() {
                 // Run one full turn
                 if let Err(e) = self.run(user_msg, stop.clone()).await {
                     let tx_opt = self.event_tx.lock().clone();
                     if let Some(tx) = tx_opt {
                         let _ = tx.send(crate::tui::AgentEvent::SystemUpdate(format!("Error: {}", e))).await;
                     }
                 }
                 // Auto-reset stop flag after a turn finishes/is interrupted
                 stop.store(false, std::sync::atomic::Ordering::Relaxed);
             }
             tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
        }
        Ok(())
    }

    /// Explicitly unload the model from Ollama's memory (GPU) by sending a request with keep_alive: 0.
    pub async fn shutdown(&self) {
        self.backend.shutdown(self.model.lock().clone()).await;
    }

    #[cfg(feature = "mlx")]
    async fn _unused_placeholder() {}
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_agent_new() {
        // Basic sanity check
    }
}
