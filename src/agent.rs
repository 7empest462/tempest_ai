use miette::Result;
use colored::*;
use ollama_rs::{
    generation::{
        chat::{request::ChatMessageRequest, ChatMessage, MessageRole},
    },
    models::ModelOptions,
    Ollama,
};
use serde_json::Value;
use futures::StreamExt;
use syntect::parsing::SyntaxSet;
use syntect::highlighting::ThemeSet;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::io::Write;
use parking_lot::Mutex;
use dashmap::DashMap;
use miette::IntoDiagnostic;
use std::path::Path;

use crate::tools::ToolContext;
use crate::memory::MemoryStore;

struct PlannerOutput {
    content: String,
    native_tool_calls: Vec<ollama_rs::generation::tools::ToolCall>,
}

#[derive(Clone)]
pub struct Agent {
    ollama: Ollama,
    model: String,
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
}

impl Agent {
    pub fn new(model: String, system_prompt: String, history_path: String, memory_store: Arc<Mutex<MemoryStore>>, sub_agent_model: String) -> Self {
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
            ollama: Ollama::default(),
            model: model.clone(),
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
            event_tx: Arc::new(Mutex::new(None)),
            tool_rx: Arc::new(tokio::sync::Mutex::new(None)),
            sentinel: crate::sentinel::SentinelManager::new(),
        }
    }

    pub async fn initialize_atlas(&self, force: bool) -> Result<()> {
        crate::tools::atlas::run_semantic_indexing(
            &self.ollama, 
            self.vector_brain.clone(), 
            &self.brain_path, 
            force
        ).await
    }
    
    fn calculate_optimal_ctx(&self) -> u64 {
        let model = self.model.to_lowercase();
        if model.contains("20b") || model.contains("27b") || model.contains("30b") || model.contains("deepseek-r1:32b") {
            2048
        } else if model.contains("13b") || model.contains("16b") || model.contains("12b") {
            4096
        } else if model.contains("14b") || model.contains("7b") || model.contains("8b") || model.contains("9b") {
            16384
        } else {
             16384
        }
    }

    pub async fn check_connection(&self) -> Result<()> {
        self.ollama.list_local_models().await.into_diagnostic()?;
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
        let history_path = std::path::Path::new(&self.history_path);
        let h_lock = self.history.lock();
        let data = serde_json::to_string_pretty(&*h_lock).into_diagnostic()?;
        std::fs::write(history_path, data).into_diagnostic()?;
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
            println!("{} {}", "Model:".blue(), self.model);
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
            if let Ok(schema_json) = serde_json::to_string_pretty(&self.tool_registry) {
                full_system_prompt.push_str(&schema_json);
            }
            if let Some(pos) = h_lock.iter().position(|m| m.role == MessageRole::System) {
                h_lock[pos] = ChatMessage::new(MessageRole::System, full_system_prompt);
            } else {
                h_lock.insert(0, ChatMessage::new(MessageRole::System, full_system_prompt));
            }
            h_lock.push(ChatMessage::new(MessageRole::User, initial_user_prompt));
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
                    let new_history = crate::context_manager::compact_history(
                        &self.ollama, 
                        &self.sub_agent_model, 
                        history_to_compact, 
                        ctx_limit
                    ).await?;
                    *self.history.lock() = new_history;
                    let _ = self.save_history();
                }

                if action.needs_privilege {
                    let mut h_lock = self.history.lock();
                    h_lock.push(ChatMessage::new(MessageRole::System, "⚠️ [SENTINEL]: You are attempting to access a protected area. If this is required, you MUST use 'request_privileges' or prefix the command with sudo.".to_string()));
                }
            }

            // --- STAGE 1: PLANNING ---
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

            if all_tool_calls.is_empty() {
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

            // --- STAGE 3: EXECUTION ---
            let results = self.executor_dispatch(all_tool_calls).await?;

            // --- STAGE 4: COLLECTION ---
            let mut detected_loop_key = None;
            {
                let mut h_lock = self.history.lock();
                for (tool_name, result, is_success) in results {
                    let formatted_res = if is_success { 
                        // Reset failure counters on any success
                        self.recent_failures.remove(&tool_name);
                        self.recent_failures.remove("GENERIC_FILE_NOT_FOUND");
                        format!("SYSTEM NOTIFICATION: TOOL RESULT for {}:\n{}\n\nCRITICAL: Analyze this result and formulate your final output to the user clearly. You MUST respond.", tool_name, result) 
                    } else { 
                        // Broad matching: If it's a "Not Found" error, we group it
                        let fail_key = if result.contains("os error 2") || result.contains("No such file") || result.contains("No files found") {
                            "GENERIC_FILE_NOT_FOUND".to_string()
                        } else {
                            format!("{}:{}", tool_name, result.chars().take(50).collect::<String>())
                        };

                        let count = *self.recent_failures.entry(fail_key.clone()).and_modify(|c| *c += 1).or_insert(1);
                        
                        if count >= 3 {
                            detected_loop_key = Some(fail_key);
                        }

                        format!("SYSTEM NOTIFICATION: TOOL ERROR for {}:\n{}\n\nCRITICAL: This tool failed. Analyze the error and determine your next step. You MUST respond.", tool_name, result) 
                    };
                    h_lock.push(ChatMessage::new(MessageRole::User, formatted_res));
                }
            }

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
        Ok(())
    }

    async fn planner_turn(&self, stop: std::sync::Arc<std::sync::atomic::AtomicBool>) -> Result<PlannerOutput> {
        let options = ModelOptions::default()
            .num_ctx(self.calculate_optimal_ctx())
            .num_predict(4096)
            .temperature(if *self.planning_mode.lock() { 0.05 } else { 0.30 });

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
            "CRITICAL: You are an autonomous agent. DO NOT ask the user how you can help. You MUST begin exactly with THOUGHT:, and you MUST output your next tool call as pure JSON enclosed in a ```json block. Do NOT use bullet points or conversational preamble.".to_string()
        ));
        
        let request = ChatMessageRequest::new(self.model.clone(), history_snapshot)
            .options(options);

        let tx_opt = self.event_tx.lock().clone();
        if let Some(tx) = tx_opt {
            let _ = tx.send(crate::tui::AgentEvent::Thinking(Some("Thinking...".to_string()))).await;
        }
        
        let mut stream = self.ollama.send_chat_messages_stream(request).await.into_diagnostic()?;
        let mut full_content = String::new();
        let mut reasoning_content = String::new();
        let mut native_tool_calls = Vec::new();
        let mut first_token = true;
        let mut is_thinking = false;

        let mut last_segments: Vec<String> = Vec::new();

        while let Some(res) = stream.next().await {
            if stop.load(std::sync::atomic::Ordering::Relaxed) {
                let tx_opt = self.event_tx.lock().clone();
                if let Some(tx) = tx_opt {
                    let _ = tx.send(crate::tui::AgentEvent::SystemUpdate("🛑 INTERRUPTED: Prompt stream halted.".to_string())).await;
                }
                break;
            }
            if let Ok(chunk) = res {
                let text = chunk.message.content.clone();
                
                // --- 🛡️ REPETITION SENTINEL ---
                let trimmed = text.trim();
                if !trimmed.is_empty() && trimmed.len() > 3 {
                    last_segments.push(trimmed.to_string());
                    if last_segments.len() > 15 { last_segments.remove(0); }
                    
                    // Trigger only if 80% of recent segments are identical (higher threshold for reasoning)
                    if last_segments.iter().filter(|&s| s == trimmed).count() >= 8 {
                        let warning = "\n\n⚠️ [REPETITION SENTINEL]: Breaking loop to prevent hallucination plateau.";
                        full_content.push_str(warning);
                        
                        let tx_opt = self.event_tx.lock().clone();
                        if let Some(tx) = tx_opt {
                            let _ = tx.send(crate::tui::AgentEvent::StreamToken(warning.to_string())).await;
                        }
                        break; 
                    }
                }
                // --- END SENTINEL ---

                // --- 🧠 REASONING EXTRACTION (Field or Tags) ---
                // 1. Check for dedicated reasoning fields (Native Ollama or OpenAI-compatible)
                let val = serde_json::to_value(&chunk.message).unwrap_or(serde_json::Value::Null);
                let reasoning_field = val.get("thinking")
                    .or_else(|| val.get("reasoning"))
                    .or_else(|| val.get("reasoning_content"))
                    .and_then(|v| v.as_str());

                if let Some(reasoning) = reasoning_field {
                    if !reasoning.is_empty() {
                        reasoning_content.push_str(reasoning);
                        let tx_opt = self.event_tx.lock().clone();
                        if let Some(tx) = tx_opt {
                            let _ = tx.send(crate::tui::AgentEvent::ReasoningToken(reasoning.to_string())).await;
                        }
                    }
                }

                // 2. Tag-based detection
                if text.contains("<think>") {
                    is_thinking = true;
                    if let Some(idx) = text.find("<think>") {
                        let after = &text[idx + 7..];
                        if !after.is_empty() {
                            reasoning_content.push_str(after);
                            let tx_opt = self.event_tx.lock().clone();
                            if let Some(tx) = tx_opt {
                                let _ = tx.send(crate::tui::AgentEvent::ReasoningToken(after.to_string())).await;
                            }
                        }
                    }
                    continue;
                }
                
                if text.contains("</think>") {
                    is_thinking = false;
                    if let Some(idx) = text.find("</think>") {
                        let before = &text[..idx];
                        if !before.is_empty() {
                            reasoning_content.push_str(before);
                            let tx_opt = self.event_tx.lock().clone();
                            if let Some(tx) = tx_opt {
                                let _ = tx.send(crate::tui::AgentEvent::ReasoningToken(before.to_string())).await;
                            }
                        }
                    }
                    continue;
                }

                if is_thinking {
                    reasoning_content.push_str(&text);
                    let tx_opt = self.event_tx.lock().clone();
                    if let Some(tx) = tx_opt {
                        let _ = tx.send(crate::tui::AgentEvent::ReasoningToken(text.clone())).await;
                    }
                    continue;
                }

                if !chunk.message.tool_calls.is_empty() {
                    native_tool_calls.extend(chunk.message.tool_calls.clone());
                }
                if first_token && !text.trim().is_empty() {
                    let tx_opt = self.event_tx.lock().clone();
                    if let Some(tx) = tx_opt {
                        let _ = tx.send(crate::tui::AgentEvent::Thinking(None)).await;
                    }
                    first_token = false;
                }
                full_content.push_str(&text);

                if self.event_tx.lock().is_none() {
                    print!("{}", text);
                    let _ = std::io::stdout().flush();
                }

                let tx_opt = self.event_tx.lock().clone();
                if let Some(tx) = tx_opt {
                    let _ = tx.send(crate::tui::AgentEvent::StreamToken(text)).await;
                }
            } else if let Err(_) = res {
                let error_msg = "\n\n❌ [OLLAMA STREAM ERROR]: The model inference stream was interrupted or timed out. Check Ollama server status.".to_string();
                let tx_opt = self.event_tx.lock().clone();
                if let Some(tx) = tx_opt {
                    let _ = tx.send(crate::tui::AgentEvent::SystemUpdate(error_msg.clone())).await;
                    let _ = tx.send(crate::tui::AgentEvent::StreamToken(error_msg)).await;
                }
                break;
            }
        }
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
        })
    }

    async fn executor_dispatch(&self, tool_calls: Vec<Value>) -> Result<Vec<(String, String, bool)>> {
        let mut futures = Vec::new();
        for tool_req in tool_calls {
            let agent_worker = self.clone();
            futures.push(tokio::spawn(async move {
                agent_worker.process_single_tool_call(tool_req).await
            }));
        }

        let mut results = Vec::new();
        for res in futures::future::join_all(futures).await {
            if let Ok(tool_res) = res {
                results.push(tool_res);
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
            if *self.planning_mode.lock() && tool.is_modifying() {
                let context = self.get_tool_context();
                let prompt = format!("Agent requests permission to execute '{}'. Proceed? (y/n/a)", tool_name);
                let mut approved = false;
                
                if context.tx.send(crate::tui::AgentEvent::RequestInput("System".to_string(), prompt.clone())).await.is_ok() {
                    let mut rx_lock = context.tool_rx.lock().await;
                    if let Some(rx) = rx_lock.as_mut() {
                        match rx.recv().await {
                            Some(crate::tui::ToolResponse::Text(ans)) if {
                                let ans_lower = ans.trim().to_lowercase();
                                ans_lower == "y" || ans_lower == "yes" || ans_lower.is_empty()
                            } => {
                                approved = true;
                            }
                            Some(crate::tui::ToolResponse::Text(ans)) if {
                                let ans_lower = ans.trim().to_lowercase();
                                ans_lower == "a" || ans_lower == "all"
                            } => {
                                approved = true;
                                *self.planning_mode.lock() = false;
                            }
                            _ => {}
                        }
                    }
                } else {
                    use std::io::{self, Write};
                    use colored::Colorize;
                    println!("\n{} {}", "⚠️ System:".yellow().bold(), prompt.cyan());
                    print!(">> ");
                    let _ = io::stdout().flush();
                    let mut input = String::new();
                    if io::stdin().read_line(&mut input).is_ok() {
                        let ans_lower = input.trim().to_lowercase();
                        if ans_lower == "y" || ans_lower == "yes" || ans_lower.is_empty() {
                            approved = true;
                        } else if ans_lower == "a" || ans_lower == "all" {
                            approved = true;
                            *self.planning_mode.lock() = false;
                        }
                    }
                }
                
                if !approved {
                    return (tool_name.clone(), format!("User REJECTED execution of {}. Formulate a new plan using non-modifying tools.", tool_name), false);
                }
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
                        return result;
                    }
                    Err(e) => {
                        let err_msg = format!("{}", e);
                        let classification = crate::error_classifier::classify_error(&tool_name, &err_msg);

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
            (tool_name.to_string(), format!("Tool '{}' not found. CRITICAL RULE: You MUST NEVER invent tools! Read the [TOOL SCHEMA]. For external data or stocks, use 'get_stock_price' or 'read_url'. For memory/research, use 'spawn_sub_agent' to preserve context and stop immediately when done.", tool_name), false)
        }
    }

    pub fn get_tool_context(&self) -> ToolContext {
        let (tx, _) = tokio::sync::mpsc::channel(1); // Placeholder for non-TUI runs

        let real_tx = match &*self.event_tx.lock() {
            Some(t) => t.clone(),
            None => tx,
        };

        ToolContext {
            ollama: self.ollama.clone(),
            model: self.model.clone(),
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
        let options = ModelOptions::default()
            .num_ctx(1)
            .num_predict(1);

        let request = ChatMessageRequest::new(
            self.model.clone(),
            vec![ChatMessage::new(MessageRole::User, "UNLOAD".to_string())]
        )
        .options(options)
        .keep_alive(ollama_rs::generation::parameters::KeepAlive::UnloadOnCompletion);

        // Silent fire-and-forget unload request
        let _ = self.ollama.send_chat_messages(request).await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_agent_new() {
        // Basic sanity check
    }
}
