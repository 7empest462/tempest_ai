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
            Arc::new(crate::tools::agent_ops::TogglePlanningTool),
            Arc::new(crate::tools::agent_ops::UpdateTaskContextTool),
            Arc::new(crate::tools::telemetry::SystemTelemetryTool),
            Arc::new(crate::tools::network_manager::ListSocketsTool),
            Arc::new(crate::tools::service_manager::ListServicesTool),
            Arc::new(crate::tools::developer::InitializeRustProjectTool),
            Arc::new(crate::tools::web::SearchWebTool),
            Arc::new(crate::tools::web::ReadUrlTool),
            Arc::new(crate::tools::web::HttpRequestTool),
            Arc::new(crate::tools::web::DownloadFileTool),
            Arc::new(crate::tools::process::RunBackgroundTool),
            Arc::new(crate::tools::process::ReadProcessLogsTool),
            Arc::new(crate::tools::process::KillProcessTool),
            Arc::new(crate::tools::process::WatchDirectoryTool),
            Arc::new(crate::tools::utilities::ClipboardTool),
            Arc::new(crate::tools::utilities::NotifyTool),
            Arc::new(crate::tools::utilities::EnvVarTool),
            Arc::new(crate::tools::utilities::ChmodTool),
            Arc::new(crate::tools::knowledge::DistillKnowledgeTool),
            Arc::new(crate::tools::knowledge::RecallBrainTool),
            Arc::new(crate::tools::knowledge::ListSkillsTool),
            Arc::new(crate::tools::knowledge::SkillRecallTool),
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
            telemetry: Arc::new(Mutex::new(String::new())),
            is_root: Arc::new(AtomicBool::new(nix::unistd::getuid().is_root())),
            concurrency_semaphore: Arc::new(tokio::sync::Semaphore::new(5)),
            event_tx: Arc::new(Mutex::new(None)),
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
        } else if model.contains("14b") || model.contains("13b") || model.contains("16b") || model.contains("12b") {
            4096
        } else if model.contains("7b") || model.contains("8b") || model.contains("9b") {
            8192
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

    pub async fn run(&self, initial_user_prompt: String) -> Result<()> {
        if self.event_tx.lock().is_none() {
            println!("{}", "=".repeat(60).blue());
            println!("{} {}", "🚀".green(), "Tempest AI Agent Initialized".bold());
            println!("{} {}", "Model:".blue(), self.model);
            println!("{}", "=".repeat(60).blue());
        }

        {
            let mut h_lock = self.history.lock();
            let full_system_prompt = self.system_prompt.clone();
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

        loop {
            iteration_count += 1;
            if iteration_count > max_iterations {
                if self.event_tx.lock().is_none() {
                    println!("\n{}", "🛑 Execution limit reached (30 turns). Stopping.".red());
                }
                break;
            }

            // --- STAGE 1: PLANNING ---
            let planner_output = self.planner_turn().await?;
            
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
            {
                let mut h_lock = self.history.lock();
                for (tool_name, result, is_success) in results {
                    let formatted_res = if is_success { 
                        format!("TOOL RESULT for {}:\n{}", tool_name, result) 
                    } else { 
                        format!("TOOL ERROR for {}:\n{}", tool_name, result) 
                    };
                    h_lock.push(ChatMessage::new(MessageRole::Tool, formatted_res));
                }
            }
            let _ = self.save_history();
        }
        Ok(())
    }

    async fn planner_turn(&self) -> Result<PlannerOutput> {
        let options = ModelOptions::default()
            .num_ctx(self.calculate_optimal_ctx())
            .num_predict(4096)
            .temperature(if *self.planning_mode.lock() { 0.15 } else { 0.05 });

        let history_snapshot = self.history.lock().clone();
        let request = ChatMessageRequest::new(self.model.clone(), history_snapshot)
            .options(options)
            .tools(self.tool_registry.clone());

        let tx_opt = self.event_tx.lock().clone();
        if let Some(tx) = tx_opt {
            let _ = tx.send(crate::tui::AgentEvent::Thinking(Some("Thinking...".to_string()))).await;
        }
        
        let mut stream = self.ollama.send_chat_messages_stream(request).await.into_diagnostic()?;
        let mut full_content = String::new();
        let mut native_tool_calls = Vec::new();
        let mut first_token = true;

        while let Some(res) = stream.next().await {
            if let Ok(chunk) = res {
                let text = chunk.message.content.clone();
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
            }
        }
        let tx_opt = self.event_tx.lock().clone();
        if let Some(tx) = tx_opt {
            let _ = tx.send(crate::tui::AgentEvent::StreamToken("".to_string())).await;
        }
        if self.event_tx.lock().is_none() {
            println!();
        }

        if !full_content.trim().is_empty() {
            self.history.lock().push(ChatMessage::new(MessageRole::Assistant, full_content.clone()));
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
        let tool_name = tool_req.get("tool").and_then(|v| v.as_str()).unwrap_or("unknown").to_string();
        let args = tool_req.get("arguments").unwrap_or(&Value::Null);

        if let Some(tool) = self.tools.get(&tool_name).map(|r| r.value().clone()) {
            if *self.planning_mode.lock() && tool.is_modifying() {
                 return (tool_name.to_string(), "Blocked: PLANNING MODE ACTIVE".to_string(), false);
            }
            let start = std::time::Instant::now();
            metrics::gauge!("tool.semaphore_available_permits").set(self.concurrency_semaphore.available_permits() as f64);
            
            let _permit = self.concurrency_semaphore.acquire().await.ok();
            let context = self.get_tool_context();
            
            let result = match tool.execute(args, context).await {
                Ok(res) => (tool_name.to_string(), res, true),
                Err(e) => (tool_name.to_string(), format!("Error: {}", e), false),
            };

            let duration = start.elapsed();
            metrics::histogram!("tool.execution_ms", "tool" => tool_name.clone()).record(duration.as_millis() as f64);
            
            // Update concurrent tracking
            self.recent_tool_calls.insert(tool_name.to_string(), result.1.chars().take(100).collect());
            
            result
        } else {
            (tool_name.to_string(), format!("Tool '{}' not found", tool_name), false)
        }
    }

    pub fn get_tool_context(&self) -> ToolContext {
        let (tx, _rx) = tokio::sync::mpsc::channel(1); // Placeholder for non-TUI runs
        let (_ttx, trx) = tokio::sync::mpsc::channel(1);

        ToolContext {
            ollama: self.ollama.clone(),
            model: self.model.clone(),
            sub_agent_model: self.sub_agent_model.clone(),
            history: self.history.clone(),
            planning_mode: self.planning_mode.clone(),
            task_context: self.task_context.clone(),
            vector_brain: self.vector_brain.clone(),
            telemetry: self.telemetry.clone(),
            tx,
            tool_rx: Arc::new(tokio::sync::Mutex::new(trx)),
            recent_tool_calls: self.recent_tool_calls.clone(),
            brain_path: self.brain_path.clone(),
            is_root: self.is_root.clone(),
        }
    }

    // Removed auto_summarize_memory (unused)

    fn extract_tool_calls(&self, content: &str) -> Result<Vec<Value>, String> {
        let block_regex = regex::Regex::new(r"```json\s*([\s\S]*?)\s*```").unwrap();
        let mut calls = Vec::new();
        for caps in block_regex.captures_iter(content) {
            if let Some(m) = caps.get(1) {
                if let Ok(val) = serde_json::from_str::<Value>(m.as_str().trim()) {
                    calls.push(val);
                }
            }
        }
        Ok(calls)
    }

    pub async fn run_tui_mode(&self, mut user_rx: tokio::sync::mpsc::Receiver<String>, _tx: tokio::sync::mpsc::Sender<crate::tui::AgentEvent>, _tool_rx: tokio::sync::mpsc::Receiver<crate::tui::ToolResponse>, stop: std::sync::Arc<std::sync::atomic::AtomicBool>) -> Result<()> {
        while !stop.load(std::sync::atomic::Ordering::Relaxed) {
             if let Ok(user_msg) = user_rx.try_recv() {
                 // Run one full turn
                 if let Err(e) = self.run(user_msg).await {
                     let tx_opt = self.event_tx.lock().clone();
                     if let Some(tx) = tx_opt {
                         let _ = tx.send(crate::tui::AgentEvent::SystemUpdate(format!("Error: {}", e))).await;
                     }
                 }
             }
             tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
        }
        Ok(())
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
