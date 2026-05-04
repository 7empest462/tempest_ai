// Copyright (c) 2026 Robert Simens. All Rights Reserved.
// Licensed under the Tempest AI Source-Available License.
// See LICENSE in the project root for full license information.

use miette::{Result, miette};
use colored::*;
use ollama_rs::{
    generation::chat::{ChatMessage, MessageRole},
    Ollama,
};
use serde_json::Value;
use syntect::parsing::SyntaxSet;
use syntect::highlighting::ThemeSet;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::sync::OnceLock;
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
            AgentPhase::Planning => "deepseek-r1:8b".to_string(),
            AgentPhase::Execution => "qwen2.5-coder:7b".to_string(),
            AgentPhase::Testing => "deepseek-r1:8b".to_string(),
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
    reasoning: String,
    native_tool_calls: Vec<ollama_rs::generation::tools::ToolCall>,
    /// Tool results that were already executed mid-stream (real-time approval)
    executed_mid_stream: Vec<(String, String, bool)>,
}

static TOOL_BLOCK_RE: OnceLock<regex::Regex> = OnceLock::new();

#[derive(Debug, Clone)]
pub struct ToolResult {
    pub tool_name: String,
    pub result: String,
    pub is_success: bool,
}

#[derive(Debug, Clone)]
pub enum AgentStreamState {
    /// DeepSeek-R1 is generating its internal <think> block
    Thinking { accumulated: String },
    /// Model is generating the main response content
    StreamingContent { content: String },
    /// Model has suggested tool calls, waiting for approval
    PendingTools { tool_calls: Vec<ollama_rs::generation::tools::ToolCall> },
    /// Actively running the approved tool batch
    ExecutingTools { 
        tool_calls: Vec<ollama_rs::generation::tools::ToolCall>,
        results: Vec<ToolResult> 
    },
    /// Turn completed
    Done,
}

#[derive(Clone)]
pub struct Agent {
    #[allow(dead_code)]
    pub mode: AgentMode,
    pub phase: Arc<Mutex<AgentPhase>>,
    backend: Arc<tokio::sync::RwLock<Backend>>,
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
    pub editor_context: Arc<Mutex<Option<Value>>>,
    pub safe_mode: Arc<AtomicBool>,
    pub tool_stats: Arc<DashMap<String, (usize, usize)>>,
    pub tool_repetition_stack: Arc<Mutex<Vec<(String, String, Option<String>)>>>,
    pub planner_model: Option<String>,
    pub executor_model: Option<String>,
    pub verifier_model: Option<String>,
    pub mlx_presets: Arc<DashMap<String, crate::MlxPreset>>,
    pub temp_planning: f32,
    pub temp_execution: f32,
    pub top_p_planning: f32,
    pub top_p_execution: f32,
    pub repeat_penalty_planning: f32,
    pub repeat_penalty_execution: f32,
    pub ctx_planning: u64,
    pub ctx_execution: u64,
    pub mlx_temp_planning: Option<f32>,
    pub mlx_temp_execution: Option<f32>,
    pub mlx_top_p_planning: Option<f32>,
    pub mlx_top_p_execution: Option<f32>,
    pub mlx_repeat_penalty_planning: Option<f32>,
    pub mlx_repeat_penalty_execution: Option<f32>,
    pub paged_attn: bool,
    pub planning_enabled: bool,
    pub checkpoint_mgr: crate::checkpoint::SharedCheckpointManager,
    pub mcp_clients: Arc<DashMap<String, Arc<tokio::sync::Mutex<crate::mcp::McpClient>>>>,
}

pub struct AgentStream<'a> {
    pub agent: &'a Agent,
    pub state: AgentStreamState,
    pub stop: Arc<AtomicBool>,
    pub iteration: usize,
}

impl<'a> AgentStream<'a> {
    pub fn new(agent: &'a Agent, stop: Arc<AtomicBool>) -> Self {
        Self {
            agent,
            state: AgentStreamState::Thinking { accumulated: String::new() },
            stop,
            iteration: 0,
        }
    }

    pub async fn transition(&mut self) -> Result<()> {
        let current_state = self.state.clone();
        match current_state {
            AgentStreamState::Thinking { accumulated } => {
                // If we have accumulated reasoning from a previous step, broadcast it
                if !accumulated.is_empty() {
                    let tx_opt = self.agent.event_tx.lock().clone();
                    if let Some(tx) = tx_opt {
                        let _ = tx.try_send(crate::tui::AgentEvent::SystemUpdate(format!("🧠 Thought Process finalized ({} chars)", accumulated.len())));
                    }
                }

                self.agent.inject_state_context();
                let ctx_limit = self.agent.calculate_optimal_ctx().await;
                self.agent.run_sentinel_stage(ctx_limit).await?;
                if !self.agent.planning_enabled {
                    // Bypass planning: Jump straight to Execution Turn
                    self.agent.switch_phase(AgentPhase::Execution).await?;
                    let output = self.agent.planner_turn(self.stop.clone()).await?;
                    
                    self.state = AgentStreamState::Thinking { 
                        accumulated: output.reasoning.clone() 
                    };

                    let mut all_tool_calls = Vec::new();
                    for native_call in output.native_tool_calls {
                        all_tool_calls.push(native_call);
                    }
                    if all_tool_calls.is_empty() {
                        if let Ok(legacy_calls) = self.agent.extract_tool_calls(&output.content) {
                            for call in legacy_calls {
                                 let name = call.get("name").or(call.get("tool")).or(call.get("function")).and_then(|v| v.as_str());
                                 let args = call.get("arguments").or(call.get("args")).cloned().unwrap_or(serde_json::json!({}));
                                 if let Some(n) = name {
                                     all_tool_calls.push(ollama_rs::generation::tools::ToolCall {
                                         function: ollama_rs::generation::tools::ToolCallFunction {
                                             name: n.to_string(),
                                             arguments: args,
                                         }
                                     });
                                 }
                            }
                        }
                    }

                    // Deduplicate
                    let mut unique_calls = Vec::new();
                    for call in all_tool_calls {
                        let is_duplicate = unique_calls.iter().any(|u: &ollama_rs::generation::tools::ToolCall| {
                            u.function.name == call.function.name && u.function.arguments == call.function.arguments
                        });
                        if !is_duplicate {
                            unique_calls.push(call);
                        }
                    }
                    all_tool_calls = unique_calls;

                    if !all_tool_calls.is_empty() {
                        self.state = AgentStreamState::PendingTools { tool_calls: all_tool_calls };
                    } else if !output.content.trim().is_empty() {
                        self.state = AgentStreamState::StreamingContent { content: output.content };
                    } else {
                        self.state = AgentStreamState::Done;
                    }
                    return Ok(());
                }

                self.agent.switch_phase(AgentPhase::Planning).await?;
                
                let planner_output = self.agent.planner_turn(self.stop.clone()).await?;
                
                // Update state with new reasoning
                self.state = AgentStreamState::Thinking { 
                    accumulated: planner_output.reasoning.clone() 
                };
                
                let mut all_tool_calls = Vec::new();
                for native_call in planner_output.native_tool_calls {
                    all_tool_calls.push(native_call);
                }
                if all_tool_calls.is_empty() {
                    if let Ok(legacy_calls) = self.agent.extract_tool_calls(&planner_output.content) {
                        for call in legacy_calls {
                             // Normalize legacy calls to the native format
                             let name = call.get("name").or(call.get("tool")).or(call.get("function")).and_then(|v| v.as_str());
                             let args = call.get("arguments").or(call.get("args")).cloned().unwrap_or(serde_json::json!({}));
                             
                             if let Some(n) = name {
                                 all_tool_calls.push(ollama_rs::generation::tools::ToolCall {
                                     function: ollama_rs::generation::tools::ToolCallFunction {
                                         name: n.to_string(),
                                         arguments: args,
                                     }
                                 });
                             }
                        }
                    }
                }

                // --- 🛡️ DEDUPLICATION GUARD ---
                let mut unique_calls = Vec::new();
                for call in all_tool_calls {
                    let is_duplicate = unique_calls.iter().any(|u: &ollama_rs::generation::tools::ToolCall| {
                        u.function.name == call.function.name && u.function.arguments == call.function.arguments
                    });
                    if !is_duplicate {
                        unique_calls.push(call);
                    }
                }
                all_tool_calls = unique_calls;

                // Handle mid-stream results
                let mid_stream_results = planner_output.executed_mid_stream;
                if !mid_stream_results.is_empty() {
                    self.agent.process_tool_feedback_stage(mid_stream_results).await?;
                }

                if !all_tool_calls.is_empty() {
                    self.state = AgentStreamState::PendingTools { tool_calls: all_tool_calls };
                } else if !planner_output.content.trim().is_empty() {
                    self.state = AgentStreamState::StreamingContent { content: planner_output.content };
                } else {
                    // --- ⚡ AUTOMATIC EXECUTION HANDOVER (Hardened) ---
                    let tx_opt = self.agent.event_tx.lock().clone();
                    if let Some(ref tx) = tx_opt {
                        let _ = tx.try_send(crate::tui::AgentEvent::SystemUpdate("⚡ HANDOVER: Handing context to EXECUTOR...".to_string()));
                    }
                    
                    // INJECT REASONING INTO HISTORY: This links the phases!
                    if !planner_output.reasoning.is_empty() {
                        let mut history = self.agent.history.lock();
                        history.push(ollama_rs::generation::chat::ChatMessage::new(
                            ollama_rs::generation::chat::MessageRole::Assistant,
                            format!("<think>\n{}\n</think>", planner_output.reasoning),
                        ));
                    }

                    self.agent.switch_phase(AgentPhase::Execution).await?;
                    
                    // Small buffer for Ollama to settle VRAM swap
                    tokio::time::sleep(std::time::Duration::from_millis(200)).await;
                    
                    let mut output = self.agent.planner_turn(self.stop.clone()).await?;
                    
                    // If the first handover turn returns nothing, try one more time
                    if output.content.trim().is_empty() && !self.stop.load(std::sync::atomic::Ordering::Relaxed) {
                        if let Some(ref tx) = tx_opt {
                            let _ = tx.try_send(crate::tui::AgentEvent::SubagentStatus(Some("📡 Retrying Handover...".to_string())));
                        }
                        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
                        output = self.agent.planner_turn(self.stop.clone()).await?;
                    }
                    
                    if !output.content.trim().is_empty() {
                        self.state = AgentStreamState::StreamingContent { content: output.content };
                    } else {
                        self.state = AgentStreamState::Done;
                    }
                }
            }
            AgentStreamState::PendingTools { tool_calls: calls } => {
                self.state = AgentStreamState::ExecutingTools { 
                    tool_calls: calls.clone(), 
                    results: Vec::new() 
                };
                
                let execution_results = self.agent.executor_dispatch(calls.clone()).await?;
                let mut tool_results = Vec::new();
                for (name, result, success) in execution_results {
                    tool_results.push(ToolResult { tool_name: name, result, is_success: success });
                }
                
                // Update state with results
                self.state = AgentStreamState::ExecutingTools { 
                    tool_calls: calls.clone(), 
                    results: tool_results.clone() 
                };
                
                let feedback_batch: Vec<_> = tool_results.into_iter()
                    .map(|r| (r.tool_name, r.result, r.is_success))
                    .collect();
                self.agent.process_tool_feedback_stage(feedback_batch).await?;
                
                self.iteration += 1;
                self.state = AgentStreamState::Thinking { accumulated: String::new() };
            }
            AgentStreamState::StreamingContent { content } => {
                let content = content.clone();
                // Expanded detection to catch .py, .json, and naked blocks
                let contains_raw_code = content.contains("```rust") || 
                                      content.contains("```python") || 
                                      content.contains("```py") || 
                                      content.contains("```javascript") || 
                                      content.contains("```js") || 
                                      content.contains("```sh") || 
                                      content.contains("```bash") || 
                                      content.contains("```json") ||
                                      (content.contains("```") && content.len() > 20); // Catch naked blocks with actual content

                let lower_content = content.to_lowercase();
                let is_delegating = lower_content.contains("you generate") || 
                                    lower_content.contains("you write") || 
                                    lower_content.contains("you create") || 
                                    lower_content.contains("let me know when you") ||
                                    (lower_content.contains("please use the tool") && !lower_content.contains("i will"));

                let model_name = self.agent.model.lock().to_lowercase();
                let planner_name = self.agent.planner_model.clone().unwrap_or_default().to_lowercase();
                let is_r1 = model_name.contains("r1") || model_name.contains("deepseek") ||
                           planner_name.contains("r1") || planner_name.contains("deepseek");

                if (contains_raw_code && !is_r1) || is_delegating {
                    let reprimand = if is_delegating {
                        "⚠️ [ROLE REMINDER]: Assistant, YOU are the engineer with the tools. The User cannot help you with file operations. Please re-issue your response and use the correct `write_file` or `run_command` JSON tool call yourself.".to_string()
                    } else {
                        "🛑 CRITICAL ERROR: Your previous response was REJECTED because it contained raw markdown code blocks. YOU ARE FORBIDDEN from using backticks for code. Use the `write_file` tool call ONLY. Please re-think your strategy and use the tool now.".to_string()
                    };
                    
                    let sentinel_name = if is_delegating { "Identity Guard" } else { "Tool Guard" };
                    let log_msg = if is_delegating { "Blocked delegation to user" } else { "Blocked raw code output" };

                    let tx_opt = self.agent.event_tx.lock().clone();
                    if let Some(tx) = tx_opt {
                        let _ = tx.try_send(crate::tui::AgentEvent::SentinelUpdate { 
                            active: vec![sentinel_name.to_string()],
                            log: log_msg.to_string() 
                        });
                    }

                    // System role prevents AI from blaming the User
                    self.agent.history.lock().push(ChatMessage::new(MessageRole::System, reprimand));
                    self.agent.save_history()?;
                    
                    // BREAK THE HABIT: Clear accumulated reasoning so it doesn't just repeat the same thought
                    self.state = AgentStreamState::Thinking { accumulated: String::new() };
                } else {
                    let is_done = lower_content.contains("done:") || 
                                 lower_content.contains("task complete") || 
                                 lower_content.contains("all tasks finished");
                    
                    if is_done {
                        self.state = AgentStreamState::Done;
                    } else {
                        // FIX: Maintain momentum! If the model is reporting progress, give it one more turn to verify or finish.
                        let tx_opt = self.agent.event_tx.lock().clone();
                        
                        // RESTORE SILENT FAILURE NUDGE:
                        let is_silent_failure = content.len() < 15 && !self.agent.history.lock().is_empty() && {
                            let last_msg = self.agent.history.lock().last().cloned();
                            let reasoning_len = last_msg.and_then(|m| m.thinking).map(|s| s.len()).unwrap_or(0);
                            reasoning_len > 100
                        };

                        if is_silent_failure {
                             let nudge = "⚠️ [SILENT FAILURE]: You reasoned about an action but didn't output a tool call. YOU must output the JSON tool call now to finish the task.".to_string();
                             self.agent.history.lock().push(ollama_rs::generation::chat::ChatMessage::new(ollama_rs::generation::chat::MessageRole::System, nudge));
                        } else if let Some(tx) = tx_opt {
                             let _ = tx.try_send(crate::tui::AgentEvent::SystemUpdate("🔄 MOMENTUM: Technical report received. Re-assessing next steps...".to_string()));
                        }
                        
                        self.state = AgentStreamState::Thinking { accumulated: String::new() };
                    }
                }
            }
            AgentStreamState::ExecutingTools { tool_calls, results } => {
                // Log the execution summary to ensure fields are read
                let tx_opt = self.agent.event_tx.lock().clone();
                if let Some(tx) = tx_opt {
                    let _ = tx.try_send(crate::tui::AgentEvent::SystemUpdate(format!("🛠️ Executed {} tools with {} results", tool_calls.len(), results.len())));
                }
                // FIX: Transition back to Thinking so the model can verify results and continue its plan
                self.state = AgentStreamState::Thinking { accumulated: String::new() };
            }
            AgentStreamState::Done => {}
        }
        Ok(())
    }
}

impl Agent {
    pub fn get_tools(&self) -> &DashMap<String, Arc<dyn crate::tools::AgentTool>> {
        &self.tools
    }

    pub async fn execute_tool_by_name(&self, name: &str, arguments: &serde_json::Value) -> Result<String> {
        let tool_call = ollama_rs::generation::tools::ToolCall {
            function: ollama_rs::generation::tools::ToolCallFunction {
                name: name.to_string(),
                arguments: arguments.clone(),
            }
        };
        
        // Execute through the unified pipeline to ensure metrics/telemetry/stats are captured
        let (_, res, success) = self.process_single_tool_call(tool_call, true).await;
        
        if success {
            Ok(res)
        } else {
            Err(miette::miette!(res))
        }
    }

    pub fn get_model(&self) -> String {
        self.model.lock().clone()
    }

    pub async fn new(
        mode: AgentMode, 
        mut model: String, 
        mut quant: String, 
        system_prompt: String, 
        history_path: String, 
        memory_store: Arc<Mutex<MemoryStore>>, 
        sub_agent_model: String,
        planner_model: Option<String>,
        executor_model: Option<String>,
        verifier_model: Option<String>,
        mlx_presets: std::collections::HashMap<String, crate::MlxPreset>,
        temp_planning: f32,
        temp_execution: f32,
        top_p_planning: f32,
        top_p_execution: f32,
        repeat_penalty_planning: f32,
        repeat_penalty_execution: f32,
        ctx_planning: u64,
        ctx_execution: u64,
        mlx_temp_planning: Option<f32>,
        mlx_temp_execution: Option<f32>,
        mlx_top_p_planning: Option<f32>,
        mlx_top_p_execution: Option<f32>,
        mlx_repeat_penalty_planning: Option<f32>,
        mlx_repeat_penalty_execution: Option<f32>,
        paged_attn: bool,
        planning_enabled: bool,
        event_tx: Arc<Mutex<Option<tokio::sync::mpsc::Sender<crate::tui::AgentEvent>>>>,
    ) -> Result<Self> {
        // Resolve preset if model name matches a key in mlx_presets
        if mode == AgentMode::MLX {
            if let Some(preset) = mlx_presets.get(&model) {
                if let Some(path) = &preset.path {
                    model = path.clone();
                    quant = "None".to_string(); // Native models don't use GGUF quant strings here
                } else if let Some(repo) = &preset.repo {
                    model = repo.clone();
                    if let Some(q) = &preset.quant {
                        quant = q.clone();
                    }
                }
            }
        }

        let (backend, final_model) = Backend::new(mode, model, quant, event_tx.clone(), paged_attn, ctx_execution as usize).await?;
        let backend = Arc::new(tokio::sync::RwLock::new(backend));

        let tools_vec: Vec<Arc<dyn crate::tools::AgentTool>> = vec![
            Arc::new(crate::tools::file::ReadFileTool),
            Arc::new(crate::tools::file::WriteFileTool),
            Arc::new(crate::tools::file::ListDirTool),
            Arc::new(crate::tools::file::SearchFilesTool),
            Arc::new(crate::tools::file::DiffFilesTool),
            Arc::new(crate::tools::file::AppendFileTool),
            Arc::new(crate::tools::file::PatchFileTool),
            Arc::new(crate::tools::file::FindReplaceTool),
            Arc::new(crate::tools::file::CreateDirectoryTool),
            Arc::new(crate::tools::file::DeleteFileTool),
            Arc::new(crate::tools::file::RenameFileTool),
            Arc::new(crate::tools::editing::EditFileWithDiffTool),
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

            Arc::new(crate::tools::privilege::RequestPrivilegesTool),
            Arc::new(crate::tools::rust::CargoAddTool),
            Arc::new(crate::tools::rust::CrateSearchTool),
            Arc::new(crate::tools::ast::AstOutlineTool),
            Arc::new(crate::tools::ast::AstEditTool),
        ];

        let tools_map = Arc::new(DashMap::new());
        for t in &tools_vec {
            tools_map.insert(t.name().to_string(), t.clone());
        }

        let history_path_obj = Path::new(&history_path);
        let brain_path = history_path_obj.parent().unwrap_or(Path::new(".")).join("brain_vectors.json");
        
        // --- 🛠️ TOOL PRUNING (Inference Optimization) ---
        // We only provide a "Core" set of tools to keep the prompt small (~1500 tokens instead of 9000+).
        // This prevents massive GPU prefill delays (KV Cache calculation) on the first turn.
        // The model can use `query_schema` to see the full details of other tools if needed.
        let core_tool_names = vec![
            "read_file", "write_file", "list_dir", "search_files", "grep_search", "edit_file_with_diff",
            "run_command", "run_tests", "build_project",
            "git_status", "git_diff", "git_action",
            "semantic_search", "tree", "project_atlas",
            "search_web", "read_url",
            "recall_brain", "recall_memory", "recall_skill", "list_skills",
            "ask_user", "query_schema", "update_task_context",
            "system_telemetry", "network_check",
            "cargo_search", "cargo_add"
        ];
        
        let tool_registry: Vec<ollama_rs::generation::tools::ToolInfo> = tools_vec.iter()
            .filter(|t| core_tool_names.contains(&t.name()))
            .map(|t| t.tool_info())
            .collect();

        let vector_brain = Arc::new(Mutex::new(crate::vector_brain::VectorBrain::load_from_disk(&brain_path)
            .unwrap_or_else(|_| crate::vector_brain::VectorBrain::new())));

        Ok(Agent {
            mode,
            phase: Arc::new(Mutex::new(AgentPhase::Planning)),
            backend: backend,
            model: Arc::new(Mutex::new(final_model)),
            history: Arc::new(Mutex::new(vec![])),
            tools: tools_map,
            tool_registry,
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
            editor_context: Arc::new(Mutex::new(None)),
            safe_mode: Arc::new(AtomicBool::new(false)),

            tool_stats: Arc::new(DashMap::new()),
            tool_repetition_stack: Arc::new(Mutex::new(Vec::new())),
            planner_model,
            executor_model,
            verifier_model,
            mlx_presets: {
                let dm = DashMap::new();
                for (k, v) in mlx_presets { dm.insert(k, v); }
                Arc::new(dm)
            },
            temp_planning,
            temp_execution,
            top_p_planning,
            top_p_execution,
            repeat_penalty_planning,
            repeat_penalty_execution,
            ctx_planning,
            ctx_execution,
            mlx_temp_planning,
            mlx_temp_execution,
            mlx_top_p_planning,
            mlx_top_p_execution,
            mlx_repeat_penalty_planning,
            mlx_repeat_penalty_execution,
            paged_attn,
            planning_enabled,
            checkpoint_mgr: crate::checkpoint::new_shared(50),
            mcp_clients: Arc::new(DashMap::new()),
            system_prompt: {
                let mut final_system_prompt = system_prompt.clone();
                if mode == AgentMode::MLX {
                    final_system_prompt.push_str("\n\n⚠️ AGENT OPERATIONAL RULES:
1. YOU ARE THE ACTOR: You possess the tools (`write_file`, `run_command`).
2. REASONING BOUNDARY: You MUST wrap your internal planning, logic, and thoughts inside `[MISSION]` and `[/MISSION]` tags.
3. ACTIONS ARE EXTERNAL: All JSON tool calls MUST be placed OUTSIDE and AFTER the `[/MISSION]` tag.
4. CODE DELIVERY: You are FORBIDDEN from using Markdown code blocks (```python) in your responses unless they are for explanation. 
5. THE JSON IS YOUR WORK: Your only way to 'do' work is by outputting a JSON tool call. A JSON tool call is NOT 'raw code'; it is your mandatory delivery mechanism.
6. EDITOR AWARENESS: You have direct visibility into the user's active editor. ALWAYS prioritize the file path and content provided in the `[EDITOR]` block. Never guess a path if one is provided in context.
7. If you have code to provide, YOU MUST output the `write_file` tool call. If you don't, the user gets nothing.
8. NEVER ask the user to write code. You are the engineer; they are the supervisor.");
                }
                final_system_prompt
            },
        })
    }

    pub async fn get_ollama(&self) -> Result<Ollama> {
        match &*self.backend.read().await {
            Backend::Ollama(o) => Ok(o.clone()),
            #[cfg(target_os = "macos")]
            Backend::MLX { .. } => Err(miette!("Active backend is MLX, not Ollama")),
            Backend::Bridge(_) => Err(miette!("Active backend is AI Bridge, not Ollama")),
        }
    }

    pub async fn initialize_atlas(&self, force: bool) -> Result<()> {
        let backend = self.backend.read().await;
        let tx = self.event_tx.lock().clone();
        crate::tools::atlas::run_semantic_indexing(
            &*backend, 
            self.vector_brain.clone(), 
            &self.brain_path, 
            force,
            tx
        ).await
    }
    
    /// Returns the configured context window size.
    /// Driven entirely by config (ctx_execution in config.toml, default 32768).
    async fn calculate_optimal_ctx(&self) -> u64 {
        self.ctx_execution
    }

    pub async fn check_connection(&self) -> Result<()> {
        if let Ok(ollama) = self.get_ollama().await {
            // Only enforce the multi-model fleet if we are actually using Ollama.
            // MLX uses a single loaded model and doesn't require these.
            let models_result = tokio::time::timeout(
                std::time::Duration::from_secs(10),
                ollama.list_local_models()
            ).await;

            let models = match models_result {
                Ok(Ok(m)) => m,
                Ok(Err(e)) => return Err(miette!("Ollama API Error: {:?}. Is the service running and responsive?", e)),
                Err(_) => return Err(miette!("Ollama connection TIMEOUT (10s). The API is not responding. Please check your Ollama app.")),
            };

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
        
        let mode_str = if is_planning { "PLANNING PHASE (Architectural research & strategy)" } else { "EXECUTION PHASE (Implementation & active engineering)" };

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

        let whisperer_str = if self.mode == AgentMode::MLX {
            "\n\n⚠️ MISSION ARCHITECTURE: You are the ACTOR. The User is your COLLABORATOR.
DELIVERY PROTOCOL: All project code and technical changes must be delivered via tools (`write_file`, `patch_file`). Raw markdown code blocks are for demonstration only.
STATUS REPORTING: You are expected to provide concise, technical updates on your progress in the chat. Explain what you've done and what you intend to verify next.
VERIFICATION CYCLE: No task is complete until verified. Use `read_file` or `run_command` to confirm your modifications before reporting success."
        } else {
            ""
        };

        let state_msg = format!(
            "[STATE] {} | DIRECTIVE: AUTONOMY ENGAGED. Focus on tool-driven execution and technical reporting.{} | ADVISORY: Verify all path assumptions and results before proceeding.{}",
            mode_str.to_uppercase(), competency_str, whisperer_str
        );

        let mut h_lock = self.history.lock();
        
        // Find the initial system prompt to merge state into, or prepend if missing
        if let Some(pos) = h_lock.iter().position(|m| m.role == MessageRole::System) {
            let mut content = h_lock[pos].content.clone();
            // Remove any existing state messages to prevent bloat
            if let Some(state_start) = content.find("[STATE]") {
                if let Some(state_end) = content[state_start..].find("\n") {
                    content.replace_range(state_start..state_start + state_end + 1, "");
                } else {
                    content.replace_range(state_start.., "");
                }
            }
            h_lock[pos].content = format!("{}\n\n{}", state_msg, content.trim());
        } else {
            h_lock.insert(0, ChatMessage::new(MessageRole::System, state_msg));
        }
    }

    pub async fn switch_phase(&self, new_phase: AgentPhase) -> Result<()> {
        let old_desc = {
            let mut p_lock = self.phase.lock();
            if *p_lock == new_phase {
                return Ok(());
            }
            let old = p_lock.description();
            *p_lock = new_phase.clone();
            old
        };
        
        // --- BACKEND-AWARE MODEL ROUTING ---
        // If we are in MLX mode, we only have one model pipeline loaded. 
        // Overwriting the model string here would break the MLX backend.
        if !matches!(&*self.backend.read().await, crate::inference::Backend::MLX { .. }) {
            *self.model.lock() = new_phase.default_model();
        }

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
            if let Ok(mut history) = serde_json::from_str::<Vec<ChatMessage>>(&data) {
                // PRUNING: Ensure the last message isn't a dangling tool call.
                while let Some(last) = history.last() {
                    if last.role == MessageRole::Assistant && !last.tool_calls.is_empty() {
                        history.pop();
                    } else {
                        break;
                    }
                }

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

    pub async fn resume_session(&self) -> Result<()> {
        let history_len = self.history.lock().len();
        if history_len > 0 {
            // Pulse the environment to ground the agent
            let cwd = std::env::current_dir().unwrap_or_default();
            let mut recent_files = Vec::new();
            
            // Heuristic: Find 5 most recently modified files in the CWD (shallow)
            if let Ok(entries) = std::fs::read_dir(&cwd) {
                let mut files: Vec<_> = entries
                    .filter_map(|e| e.ok())
                    .filter(|e| e.file_type().map(|ft| ft.is_file()).unwrap_or(false))
                    .filter(|e| {
                        let name = e.file_name().to_string_lossy().to_string();
                        !name.starts_with('.') && name != "Cargo.lock" && name != "history.json"
                    })
                    .collect();
                
                files.sort_by_key(|e| e.metadata().and_then(|m| m.modified()).ok());
                files.reverse();
                
                for entry in files.iter().take(5) {
                    recent_files.push(entry.file_name().to_string_lossy().into_owned());
                }
            }

            let recent_str = if recent_files.is_empty() { 
                "No recent files detected.".to_string() 
            } else { 
                recent_files.join(", ") 
            };

            let pulse = format!(
                "🔄 [SESSION RESUME]: You are continuing a previous session.\n\
                 - Current Working Directory: {}\n\
                 - Recent Files in Workspace: {}\n\n\
                 Please briefly acknowledge that you've resumed the context and are ready to continue.",
                cwd.display(),
                recent_str
            );

            let mut h_lock = self.history.lock();
            h_lock.push(ChatMessage::new(MessageRole::System, pulse));
            
            let tx_opt = self.event_tx.lock().clone();
            if let Some(tx) = tx_opt {
                let _ = tx.try_send(crate::tui::AgentEvent::SystemUpdate("✨ Session Resumed: Environment grounded.".to_string()));
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

    /// Initializes and connects to external MCP servers based on the provided configuration.
    pub async fn initialize_mcp(&self, configs: Vec<crate::McpServerConfig>) -> Result<()> {
        for config in configs {
            let tx_opt = self.event_tx.lock().clone();
            if let Some(tx) = tx_opt.as_ref() {
                let _ = tx.try_send(crate::tui::AgentEvent::SystemUpdate(format!("🔌 Connecting to MCP Server: {}...", config.name)));
            }

            match crate::mcp::McpClient::new(
                config.name.clone(),
                &config.command,
                &config.args,
                &config.env.clone().unwrap_or_default()
            ).await {
                Ok(mut client) => {
                    if let Err(e) = client.initialize().await {
                        if let Some(tx) = tx_opt.as_ref() {
                            let _ = tx.try_send(crate::tui::AgentEvent::SystemUpdate(format!("❌ MCP Init Failed ({}): {}", config.name, e)));
                        }
                        continue;
                    }

                    match client.list_tools().await {
                        Ok(tools) => {
                            let client_arc = Arc::new(tokio::sync::Mutex::new(client));
                            self.mcp_clients.insert(config.name.clone(), client_arc.clone());

                            for tool in tools {
                                // Hardening: Validate schema before registration
                                if !tool.input_schema.is_object() {
                                    if let Some(tx) = tx_opt.as_ref() {
                                        let _ = tx.try_send(crate::tui::AgentEvent::SystemUpdate(format!("⚠️ Skipping MCP Tool {}: Malformed Schema (Not an object)", tool.name)));
                                    }
                                    continue;
                                }

                                // Dynamic tool registration with 'static str leaking
                                let namespaced_name = format!("{}_{}", config.name, tool.name);
                                let leaked_name: &'static str = Box::leak(namespaced_name.into_boxed_str());
                                let leaked_desc: &'static str = Box::leak(tool.description.into_boxed_str());

                                let proxy = crate::mcp::McpToolProxy {
                                    client: client_arc.clone(),
                                    name: leaked_name,
                                    description: leaked_desc,
                                    input_schema: tool.input_schema.clone(),
                                };

                                self.tools.insert(leaked_name.to_string(), Arc::new(proxy));
                                
                                if let Some(tx) = tx_opt.as_ref() {
                                    let _ = tx.try_send(crate::tui::AgentEvent::SystemUpdate(format!("✅ Registered MCP Tool: {}", leaked_name)));
                                }
                            }
                        }
                        Err(e) => {
                            if let Some(tx) = tx_opt.as_ref() {
                                let _ = tx.try_send(crate::tui::AgentEvent::SystemUpdate(format!("❌ MCP Tools Failed ({}): {}", config.name, e)));
                            }
                        }
                    }
                }
                Err(e) => {
                    if let Some(tx) = tx_opt.as_ref() {
                        let _ = tx.try_send(crate::tui::AgentEvent::SystemUpdate(format!("❌ MCP Connection Failed ({}): {}", config.name, e)));
                    }
                }
            }
        }
        Ok(())
    }

    /// Helper to push a structured tool result back to the model history and TUI.
    pub async fn send_tool_feedback(&self, tool_name: &str, result: &str, is_success: bool) -> Result<()> {

        // Update TUI HUD
        let tx_opt = self.event_tx.lock().clone();
        if let Some(tx) = tx_opt {
            let _ = tx.try_send(crate::tui::AgentEvent::StreamToken(format!("⚡ [System]: {} completed.\n---\n{}\n---\n", tool_name, result)));
            let _ = tx.try_send(crate::tui::AgentEvent::StreamToken("".to_string())); // Finalize message
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

        self.history.lock().push(ChatMessage::new(MessageRole::User, feedback));
        self.save_history()?;
        
        // --- 📊 RESTORE TOOL RESULT TRACKER ---
        self.report_tool_stats().await;
        
        Ok(())
    }

    /// Reports current tool performance metrics to the TUI.
    pub async fn report_tool_stats(&self) {
        if self.tool_stats.is_empty() { return; }
        
        let mut stats_lines = Vec::new();
        stats_lines.push("📊 [TOOL PERFORMANCE]:".to_string());
        for item in self.tool_stats.iter() {
            let (name, (s, f)) = (item.key(), item.value());
            let total = s + f;
            if total == 0 { continue; }
            let rate = (*s as f64 / total as f64) * 100.0;
            let emoji = if rate >= 90.0 { "🟢" } else if rate >= 50.0 { "🟡" } else { "🔴" };
            stats_lines.push(format!("  {} {}: {:.1}% ({}s / {}f)", emoji, name, rate, s, f));
        }
        
        if let Some(tx) = self.event_tx.lock().clone() {
            let _ = tx.try_send(crate::tui::AgentEvent::SystemUpdate(stats_lines.join("\n")));
        }
    }

    pub fn clear_history(&self) {
        self.history.lock().clear();
        let _ = std::fs::remove_file(&self.history_path);
        
        // Clear Atlas semantic index to prevent session leakage
        let _ = std::fs::remove_file(".tempest_atlas.md");
    }

    pub async fn switch_mlx_model(&self, preset_name: String) -> Result<()> {
        let preset = self.mlx_presets.get(&preset_name)
            .ok_or_else(|| miette!("Preset {} not found", preset_name))?
            .clone();
            
        let (model_val, quant_val) = if let Some(path) = preset.path {
            (path, "None".to_string())
        } else if let Some(repo) = preset.repo {
            (repo, preset.quant.unwrap_or_else(|| "Q4_K_M".to_string()))
        } else {
            return Err(miette!("Preset {} is malformed: missing path or repo", preset_name));
        };

        let tx_opt = self.event_tx.lock().clone();
        if let Some(tx) = tx_opt {
            let _ = tx.send(crate::tui::AgentEvent::SystemUpdate(format!("🔄 Hot-swapping MLX to: {} ({})", preset_name, quant_val))).await;
        } else {
            println!("{} Hot-swapping MLX to: {} ({})", "🔄".yellow(), preset_name, quant_val);
        }
        
        let (new_backend, new_model_name) = crate::inference::Backend::new(
            crate::inference::AgentMode::MLX,
            model_val,
            quant_val,
            self.event_tx.clone(),
            self.paged_attn,
            self.ctx_execution as usize
        ).await?;
        
        {
            let mut lock = self.backend.write().await;
            *lock = new_backend;
        }
        
        {
            let mut model_lock = self.model.lock();
            *model_lock = new_model_name;
        }

        let tx_opt = self.event_tx.lock().clone();
        if let Some(tx) = tx_opt {
            let _ = tx.send(crate::tui::AgentEvent::SystemUpdate(format!("✅ MLX Switched to {}", preset_name))).await;
        } else {
            println!("{} MLX Switched to {}", "✅".green(), preset_name);
        }
        
        Ok(())
    }

    pub async fn run(&self, initial_user_prompt: String, stop: std::sync::Arc<std::sync::atomic::AtomicBool>) -> Result<()> {
        if initial_user_prompt.trim() == "/clear" {
            self.clear_history();
            return Ok(());
        }

        let prompt_trimmed = initial_user_prompt.trim();
        if prompt_trimmed.starts_with('/') {
            if prompt_trimmed == "/help" {
                let manual = include_str!("../MANUAL.md");
                let tx_opt = self.event_tx.lock().clone();
                if let Some(tx) = tx_opt {
                    let _ = tx.try_send(crate::tui::AgentEvent::ShowManual(manual.to_string()));
                }
                return Ok(());
            }

            if prompt_trimmed == "/safemode" {
                let current = self.safe_mode.load(std::sync::atomic::Ordering::SeqCst);
                self.safe_mode.store(!current, std::sync::atomic::Ordering::SeqCst);
                let tx_opt = self.event_tx.lock().clone();
                if let Some(tx) = tx_opt {
                    let _ = tx.try_send(crate::tui::AgentEvent::SystemUpdate(format!("🛡️ Safe Mode: {}", if !current { "ON" } else { "OFF" })));
                }
                return Ok(());
            }
            
            let cmd = if prompt_trimmed.starts_with("/switch ") {
                prompt_trimmed.strip_prefix("/switch ").unwrap().trim()
            } else {
                prompt_trimmed.strip_prefix("/").unwrap().trim()
            };
            
            let cmd_str = cmd.to_string();
            if self.mlx_presets.get(&cmd_str).is_some() {
                return self.switch_mlx_model(cmd_str).await;
            } else if prompt_trimmed.starts_with("/switch ") {
                println!("{} Preset not found: {}", "❌".red(), cmd_str);
                return Ok(());
            }
        }

        self.initialize_session(&initial_user_prompt).await?;

        // 🛡️ PRE-TURN CONNECTIVITY SENTINEL
        if let Ok(ollama) = self.get_ollama().await {
            let ping = tokio::time::timeout(
                std::time::Duration::from_secs(2),
                ollama.list_local_models()
            ).await;
            
            if ping.is_err() || ping.unwrap().is_err() {
                let tx_opt = self.event_tx.lock().clone();
                if let Some(tx) = tx_opt {
                    let _ = tx.send(crate::tui::AgentEvent::SystemUpdate("⚠️ CRITICAL: Cannot reach Ollama API. Handshake failed.".to_string())).await;
                }
                return Err(miette!("Ollama is not responding. Please ensure the Ollama service is running."));
            }
        }
        
        // Immediate Feedback: Signal to the UI that we are starting
        let tx_opt = self.event_tx.lock().clone();
        if let Some(tx) = tx_opt {
            let _ = tx.try_send(crate::tui::AgentEvent::Thinking(Some("Analyzing request...".to_string())));
            let _ = tx.try_send(crate::tui::AgentEvent::SubagentStatus(Some("⚡ Tempest Engine: Grounding environment and history...".to_string())));
        }

        let mut stream = AgentStream::new(self, stop.clone());
        let max_iterations = 30;

        while stream.iteration < max_iterations {
            if stop.load(std::sync::atomic::Ordering::Relaxed) {
                let tx_opt = self.event_tx.lock().clone();
                if let Some(tx) = tx_opt {
                    let _ = tx.send(crate::tui::AgentEvent::SystemUpdate("🛑 INTERRUPTED: Turn cancelled by user.".to_string())).await;
                    let _ = tx.send(crate::tui::AgentEvent::Thinking(None)).await;
                }
                break;
            }

            match &stream.state {
                AgentStreamState::Done => break,
                _ => {
                    stream.transition().await?;
                }
            }
        }

        let _ = self.save_history();
        Ok(())
    }

    async fn initialize_session(&self, initial_user_prompt: &str) -> Result<()> {
        if self.event_tx.lock().is_none() {
            println!("{}", "=".repeat(60).blue());
            println!("{} {}", "🚀".green(), "Tempest AI Agent Initialized".bold());
            println!("{} {}", "Model:".blue(), *self.model.lock());
            println!("{}", "=".repeat(60).blue());
        }

        let os_name = match std::env::consts::OS {
            "macos" => "macOS",
            "linux" => "Linux",
            "windows" => "Windows",
            _ => std::env::consts::OS,
        };

        let active_rules = self.rules.get_active_rules(&[]);
        
        {
            let mut h_lock = self.history.lock();
            
            // 1. IDENTITY + INVIOLABLE RULES (Base)
            let mut full_system_prompt = crate::prompts::SYSTEM_PROMPT_BASE.replace("{OS}", os_name);
            
            // 2. TOOL SCHEMA (Middle)
            full_system_prompt.push_str("\n\n[TOOL SCHEMA]\n");
            if let Ok(schema_json) = serde_json::to_string(&self.tool_registry) {
                full_system_prompt.push_str(&schema_json);
            }

            // 3. ACTIVE PROJECT RULES (Contextual)
            if !active_rules.is_empty() {
                full_system_prompt.push_str("\n\n[ACTIVE PROJECT RULES]\n");
                for rule in active_rules {
                    full_system_prompt.push_str(&format!("### Rule: {}\n{}\n\n", rule.name, rule.content));
                }
            }

            // 4. OUTPUT FORMAT + CRITICAL RESPONSE RULES + EXAMPLES (Tail)
            // This is the last thing the model sees, providing the strongest priming.
            let current_model = self.model.lock().to_lowercase();
            let planner_model = self.planner_model.clone().unwrap_or_default().to_lowercase();
            
            // Log model names to status for diagnostic purposes
            let tx_opt = self.event_tx.lock().clone();
            if let Some(tx) = tx_opt {
                let _ = tx.try_send(crate::tui::AgentEvent::SubagentStatus(Some(format!("🤖 Engine: {} | Planner: {}", current_model, planner_model))));
            }

            // If EITHER the main model or the planner is a reasoning model, use the minimalist prompt
            let is_reasoning = current_model.contains("r1") || current_model.contains("deepseek") || current_model.contains("deep_seat") ||
                              planner_model.contains("r1") || planner_model.contains("deepseek") || planner_model.contains("deep_seat");

            if is_reasoning {
                full_system_prompt.push_str("\n\n[ACTOR PROTOCOL]:\n- You are the ACTOR. Your response MUST start with a `<think>` block.\n- [THOUGHT BOUNDARY]: Your `<think>` block is for internal logic ONLY. Do NOT place tool calls inside thoughts.\n- [TOOL CALL FORMAT]: After `</think>`, explain your action briefly, then output the JSON tool call directly. NEVER use markdown code blocks (```json) for tool calls.\n- [FORMAT]:\n<think>\n[Reasoning]\n</think>\nExplanation text...\n{\"tool\": \"name\", \"arguments\": {}}\n\n- [COLLABORATION]: After performing actions and verifying results, provide a concise summary to the user and ask for the next step in the roadmap.");
            } else {
                full_system_prompt.push_str(&crate::prompts::SYSTEM_PROMPT_TAIL.replace("{OS}", os_name));
            }

            if let Some(pos) = h_lock.iter().position(|m| m.role == MessageRole::System) {
                h_lock[pos] = ChatMessage::new(MessageRole::System, full_system_prompt);
            } else {
                h_lock.insert(0, ChatMessage::new(MessageRole::System, full_system_prompt));
            }

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
            // We store ONLY the user's original prompt in the permanent history.
            // The editor context is prepended dynamically in 'planner_turn' but is NOT saved to disk.
            let user_msg = ChatMessage::new(MessageRole::User, initial_user_prompt.to_string());


            h_lock.push(user_msg);

            self.recent_failures.clear();

            // Ensure we always have at least one User message
            if h_lock.iter().filter(|m| m.role == MessageRole::User).count() == 0 {
                h_lock.push(ChatMessage::new(MessageRole::User, initial_user_prompt.to_string()));
            }
        }
        let _ = self.save_history();
        Ok(())
    }

    async fn run_sentinel_stage(&self, ctx_limit: u64) -> Result<()> {
        let rep_stack = self.tool_repetition_stack.lock().clone();
        let history = self.history.lock().clone();
        let sentinel = self.sentinel.clone();
        let action_opt = tokio::task::spawn_blocking(move || {
            sentinel.analyze_state(&history, ctx_limit, &rep_stack)
        }).await.into_diagnostic()?;

        if let Some(action) = action_opt {
            let tx_opt = self.event_tx.lock().clone();
            if let Some(tx) = tx_opt {
                let _ = tx.try_send(crate::tui::AgentEvent::SentinelUpdate { 
                    active: action.active_sentinels.clone(), 
                    log: action.message.clone() 
                });
                
                if !action.message.is_empty() {
                    let _ = tx.try_send(crate::tui::AgentEvent::SystemUpdate(action.message.clone()));
                    // Inject into history so the model sees the reprimand immediately
                    self.history.lock().push(ollama_rs::generation::chat::ChatMessage::new(
                        ollama_rs::generation::chat::MessageRole::System, 
                        format!("SENTINEL INTERVENTION: {}", action.message)
                    ));
                }
            }

            if action.needs_compaction {
                let history_to_compact = self.history.lock().clone();
                let before_count = crate::context_manager::estimate_tokens(&history_to_compact);
                
                let backend_guard = self.backend.read().await;
                let new_history = crate::context_manager::compact_history(
                    &*backend_guard, 
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
        Ok(())
    }

    async fn process_tool_feedback_stage(&self, results: Vec<(String, String, bool)>) -> Result<()> {
        self.switch_phase(AgentPhase::Testing).await?;
        let mut detected_loop_key = None;
        let mut feedback_to_apply = Vec::new();

        for (tool_name, result, is_success) in results {
            let (formatted_res, _hud_msg, is_success) = if is_success { 
                self.recent_failures.remove(&tool_name);
                self.recent_failures.remove("GENERIC_FILE_NOT_FOUND");
                
                if result.starts_with("BLOCKED:") {
                    (
                        format!("SYSTEM NOTIFICATION: TOOL BLOCKED for {}:\n{}\n\nACTION REQUIRED: You MUST propose a plan and ask for approval via 'ask_user' before this tool can be used.", tool_name, result),
                        format!("🚫 BLOCKED: '{}'", tool_name),
                        true
                    )
                } else {
                    let is_modifying = self.tools.get(&tool_name).map(|t| t.is_modifying()).unwrap_or(false);
                    let mut base_res = format!("SUCCESS: Tool '{}' executed.\nRESULT:\n{}", tool_name, result);
                    if is_modifying {
                        base_res.push_str("\n\n⚠️ SYSTEM DIRECTIVE: You have just modified a physical file. Before doing ANYTHING else, you MUST rigorously test and verify your changes.");
                    }
                    (base_res, format!("✅ SUCCESS: '{}'", tool_name), true)
                }
            } else { 
                let fail_key = if result.contains("os error 2") || result.contains("No such file") {
                    "GENERIC_FILE_NOT_FOUND".to_string()
                } else {
                    format!("{}:{}", tool_name, result.chars().take(50).collect::<String>())
                };

                let count = *self.recent_failures.entry(fail_key.clone()).and_modify(|c| *c += 1).or_insert(1);
                if count >= 3 {
                    detected_loop_key = Some(fail_key);
                }

                (format!("ERROR: Tool '{}' failed.\nREASON:\n{}", tool_name, result), format!("❌ ERROR: '{}'", tool_name), false)
            };
            
            feedback_to_apply.push((tool_name, formatted_res, is_success));
        }

        for (tool_name, formatted_res, is_success) in feedback_to_apply {
            let _ = self.send_tool_feedback(&tool_name, &formatted_res, is_success).await;
        }

        if let Some(key) = detected_loop_key {
            let mut h_lock = self.history.lock();
            let directive = format!("\n\n⚠️ [SENTINEL REORIENTATION DIRECTIVE]: You are looping on '{}'. I am forcing a state-synchronization check of the CURRENT WORKING DIRECTORY.", key);
            h_lock.push(ChatMessage::new(MessageRole::System, directive));
            
            if let Ok(entries) = std::fs::read_dir(".") {
                let files: Vec<_> = entries.flatten().map(|e| e.file_name().to_string_lossy().to_string()).collect();
                let resync = format!("SYSTEM NOTIFICATION: This is an automated forced-sync of your CURRENT WORKING DIRECTORY (it is not a message from the user).\n\nCONTENTS:\n- {}", files.join("\n- "));
                h_lock.push(ChatMessage::new(MessageRole::System, resync));
            }
            self.recent_failures.clear();
        }
        
        let _ = self.save_history();
        Ok(())
    }

    async fn planner_turn(&self, stop: std::sync::Arc<std::sync::atomic::AtomicBool>) -> Result<PlannerOutput> {
        let mut history_snapshot = self.history.lock().clone();

        // --- PHASE 3: TOKEN BUDGET AWARENESS ---
        let ctx_limit = self.calculate_optimal_ctx().await;
        let used = crate::context_manager::estimate_tokens(&history_snapshot);
        
        let tx_opt = self.event_tx.lock().clone();
        if let Some(tx) = tx_opt {
            let _ = tx.try_send(crate::tui::AgentEvent::ContextStatus { used, total: ctx_limit });
        }


        let runway_report = crate::context_manager::generate_runway_report(used, ctx_limit);
        
        // Merge runway report into the first system message of the snapshot
        if let Some(pos) = history_snapshot.iter().position(|m| m.role == MessageRole::System) {
            history_snapshot[pos].content = format!("{}\n\n{}", runway_report, history_snapshot[pos].content);
        } else {
            history_snapshot.insert(0, ChatMessage::new(MessageRole::System, runway_report));
        }

        // Remove the mid-history System insertion that was violating alternating role rules.
        // We'll rely on the unified system merge in inference.rs to keep these rules active.
        
        let executed_mid_stream = Arc::new(parking_lot::Mutex::new(Vec::new()));
        let (tool_tx, mut tool_rx) = tokio::sync::mpsc::unbounded_channel::<ollama_rs::generation::tools::ToolCall>();
        let executed_mid_stream_for_task = executed_mid_stream.clone();
        let agent_for_task = self.clone();

        let tool_task = tokio::spawn(async move {
            let mut join_set = tokio::task::JoinSet::new();
            while let Some(call) = tool_rx.recv().await {
                let agent_clone = agent_for_task.clone();
                join_set.spawn(async move {
                    agent_clone.process_single_tool_call(call, false).await
                });
            }
            while let Some(res) = join_set.join_next().await {
                if let Ok(r) = res {
                    executed_mid_stream_for_task.lock().push(r);
                }
            }
        });

        let mode = self.backend.read().await.mode();

        let is_mlx = mode == crate::inference::AgentMode::MLX;

        let phase = self.phase.lock().clone();
        let is_planning = matches!(phase, AgentPhase::Planning);
        let model_name = match phase {
            AgentPhase::Planning => self.planner_model.clone().unwrap_or_else(|| self.model.lock().clone()),
            AgentPhase::Execution => self.executor_model.clone().unwrap_or_else(|| self.model.lock().clone()),
            AgentPhase::Testing => self.verifier_model.clone().unwrap_or_else(|| self.model.lock().clone()),
        };

        if let Some(tx) = self.event_tx.lock().clone() {
            let phase_desc = phase.description();
            let _ = tx.try_send(crate::tui::AgentEvent::SubagentStatus(Some(format!("🔄 {} [Using {}]...", phase_desc, model_name))));
        }

        let sampling = if is_planning {
            crate::inference::SamplingConfig {
                temperature: if is_mlx { self.mlx_temp_planning.unwrap_or(0.6) } else { self.temp_planning },
                top_p: if is_mlx { self.mlx_top_p_planning.unwrap_or(0.95) } else { self.top_p_planning },
                repeat_penalty: if is_mlx { self.mlx_repeat_penalty_planning.unwrap_or(1.1) } else { self.repeat_penalty_planning },
                context_size: self.ctx_planning,
            }
        } else {
            crate::inference::SamplingConfig {
                temperature: if is_mlx { self.mlx_temp_execution.unwrap_or(0.2) } else { self.temp_execution },
                top_p: if is_mlx { self.mlx_top_p_execution.unwrap_or(0.9) } else { self.top_p_execution },
                repeat_penalty: if is_mlx { self.mlx_repeat_penalty_execution.unwrap_or(1.05) } else { self.repeat_penalty_execution },
                context_size: self.ctx_execution,
            }
        };
        
        let mut final_history = history_snapshot.clone();
        
        // --- 🧠 DYNAMIC CONTEXT INJECTION ---
        // We prepend the editor context to the LAST user message in this turn's memory ONLY.
        // This keeps the long-term history (and history.json) clean of redundant code blocks.
        if let Some(ctx) = self.editor_context.lock().as_ref() {
            let visible_code = ctx.get("visible_code").and_then(|v| v.as_str()).unwrap_or("");
            if !visible_code.is_empty() {
                if let Some(last_user_msg) = final_history.iter_mut().rev().find(|m| m.role == MessageRole::User) {
                    let file_name = ctx.get("file_name").and_then(|v| v.as_str()).unwrap_or("unknown");
                    let file_path = ctx.get("file_path").and_then(|v| v.as_str()).unwrap_or("");
                    let language = ctx.get("language_id").and_then(|v| v.as_str()).unwrap_or("text");
                    let has_selection = ctx.get("has_selection").and_then(|v| v.as_bool()).unwrap_or(false);
                    let cursor_line = ctx.get("cursor_line").and_then(|v| v.as_u64()).unwrap_or(0);
                    let lines_count = visible_code.lines().count();

                    let context_prefix = format!(
                        "### [EDITOR GROUND TRUTH] ###\n\
                        - ACTIVE FILE: {}\n\
                        - FULL PATH: {}\n\
                        - LANGUAGE: {}\n\
                        - CURSOR LINE: {}\n\
                        - STATUS: {} contains {} lines of code\n\
                        \n\
                        ```{}\n\
                        {}\n\
                        ```\n\
                        ### [END EDITOR CONTEXT] ###\n\n",
                        file_name, file_path, language, cursor_line,
                        if has_selection { "SELECTION" } else { "VISIBLE CODE" },
                        lines_count, language, visible_code
                    );
                    
                    if !last_user_msg.content.contains("[EDITOR]") {
                        last_user_msg.content = format!("{} [USER] {}", context_prefix, last_user_msg.content);
                    }
                }
            }
        }

        if let Some(tx) = self.event_tx.lock().clone() {
            let _ = tx.try_send(crate::tui::AgentEvent::SubagentStatus(Some(format!("📡 Dispatching request to Ollama ({}) [Waiting for GPU]...", model_name))));
        }

        let output = self.backend.read().await.stream_chat(
            model_name,
            final_history,
            sampling,
            self.event_tx.clone(),
            stop,
            self.system_prompt.clone(),
            Some(tool_tx),
            Some(self.tool_registry.clone()),
        ).await?;

        // Signal end of tool calls and wait for all mid-stream tools to finish
        // (though in reality the task completes as soon as tool_tx is dropped)
        // drop(tool_tx); // already dropped by being passed by value? no, it was Some(tool_tx)
        // Wait, stream_chat takes Option<UnboundedSender<...>> by value.
        // So tool_tx is dropped when stream_chat returns.
        let _ = tool_task.await;

        let full_content = output.content;
        let reasoning_content = output.reasoning;
        let native_tool_calls = output.native_tool_calls;
        let tx_opt = self.event_tx.lock().clone();
        if let Some(tx) = tx_opt {
            let _ = tx.send(crate::tui::AgentEvent::StreamToken("".to_string())).await;
        }
        if self.event_tx.lock().is_none() {
            println!();
        }

        if !full_content.trim().is_empty() || !native_tool_calls.is_empty() || !reasoning_content.is_empty() {
            let mut stored_content = full_content.clone();
            if !native_tool_calls.is_empty() && stored_content.is_empty() {
                stored_content = "<think>I am executing a structural tool call.</think>".to_string();
                // Actively notify the UI if we were silent during the stream
                if let Some(tx) = self.event_tx.lock().clone() {
                    let _ = tx.try_send(crate::tui::AgentEvent::StreamToken("⚡ [System]: Executing tool call...".to_string()));
                }
            }
            
            let mut msg = ChatMessage::new(MessageRole::Assistant, stored_content);
            msg.tool_calls = native_tool_calls.clone();
            msg.thinking = Some(reasoning_content.clone());
            
            self.history.lock().push(msg);
        }

        Ok(PlannerOutput {
            content: full_content,
            reasoning: reasoning_content,
            native_tool_calls,
            executed_mid_stream: Arc::try_unwrap(executed_mid_stream).map(|m| m.into_inner()).unwrap_or_default(),
        })
    }

    fn repair_tool_name(&self, name: &str) -> String {
        match name.to_lowercase().as_str() {
            "ask" | "ask_user_input" | "prompt_user" | "user_input" => "ask_user".to_string(),
            "stock_price" | "get_stock" | "check_stock" | "stock" => "get_stock_price".to_string(),
            "search" | "google_search" | "web_search" => "search_web".to_string(),
            "read" | "fetch_url" | "web_read" => "read_url".to_string(),
            "recall" | "recall_knowledge" | "memory" | "brain" => "recall_brain".to_string(),
            "distill" | "save_knowledge" | "save_brain" => "distill_knowledge".to_string(),
            "shell" | "exec" | "command" => "run_command".to_string(),
            "notify" | "alert" | "status" => "no_op".to_string(),
            _ => name.to_string(),
        }
    }

    pub async fn executor_dispatch(&self, tool_calls: Vec<ollama_rs::generation::tools::ToolCall>) -> Result<Vec<(String, String, bool)>> {
        let mut results = Vec::new();

        // 🛡️ REPETITION SENTINEL: Block identical back-to-back tool calls
        let mut filtered_calls = Vec::new();
        {
            let mut stack = self.tool_repetition_stack.lock();
            for call in tool_calls {
                let repaired_name = self.repair_tool_name(&call.function.name);
                let call_key = format!("{}:{}", repaired_name, call.function.arguments);
                
                // If this EXACT call was the VERY LAST one made, block it to break the loop
                if let Some((_, _, last_res)) = stack.first().filter(|(k, _, _)| k == &call_key) {
                    let tx_opt = self.event_tx.lock().clone();
                    if let Some(tx) = tx_opt {
                        let _ = tx.try_send(crate::tui::AgentEvent::SentinelUpdate { 
                            active: vec!["Loop Breaker".to_string()],
                            log: format!("Blocked duplicate {}", call.function.name) 
                        });
                    }

                    let informative_error = if let Some(res) = last_res {
                        format!("⚠️ [REPETITION ALERT]: You have already performed this exact action with these arguments. DO NOT REPEAT.\n\nSYSTEM RECALL: Here is the result of your PREVIOUS execution (provided so you don't have to call it again):\n---\n{}\n---", res)
                    } else {
                        "⚠️ [REPETITION ALERT]: You have already performed this exact action with these arguments. DO NOT REPEAT. If you are finished, acknowledge and stop.".to_string()
                    };

                    results.push((
                        call.function.name.clone(),
                        informative_error,
                        false
                    ));
                    continue;
                }
                
                // Track this call for future repetition checks (keep last 10 for better coverage)
                stack.insert(0, (call_key, repaired_name, None));
                if stack.len() > 10 { stack.pop(); }
                filtered_calls.push(call);
            }
        }

        let mut current_batch: Vec<tokio::task::JoinHandle<(String, String, bool)>> = Vec::new();
        let mut current_batch_calls = Vec::new();
        let mut resource_locks = std::collections::HashSet::new();

        for tool_call in filtered_calls {
            let tool_name = tool_call.function.name.clone();
            let is_modifying = self.tools.get(&tool_name).map(|t| t.is_modifying()).unwrap_or(false);
            
            // Extract target resource (file path) for concurrency checks
            let resource = tool_call.function.arguments
                .get("path")
                .or(tool_call.function.arguments.get("file_path"))
                .and_then(|v| v.as_str())
                .map(|s| shellexpand::tilde(s).to_string());

            let mut should_flush = false;
            
            if let Some(res) = &resource {
                if resource_locks.contains(res) {
                    should_flush = true;
                } else {
                    resource_locks.insert(res.clone());
                }
            } else if is_modifying {
                should_flush = true;
            }

            if should_flush && !current_batch_calls.is_empty() {
                // --- BATCH APPROVAL GATE ---
                let approved = self.handle_batch_approval(&current_batch_calls).await;
                
                if approved {
                    let batch_results = futures::future::join_all(current_batch).await;
                    results.extend(batch_results.into_iter().filter_map(|r| r.ok()));
                } else {
                    for call in &current_batch_calls {
                        results.push((call.function.name.clone(), "Error: Batch execution was rejected by the user.".to_string(), false));
                    }
                }
                
                current_batch = Vec::new();
                current_batch_calls = Vec::new();
                resource_locks.clear();
                if let Some(res) = &resource { resource_locks.insert(res.clone()); }
            }

            current_batch_calls.push(tool_call.clone());
            
            // Prepare parallel task
            let self_clone = self.clone();
            let call_clone = tool_call.clone();
            
            let task = async move {
                let is_mod = self_clone.tools.get(&call_clone.function.name).map(|t| t.is_modifying()).unwrap_or(false);
                if is_mod {
                    let mut cp = self_clone.checkpoint_mgr.lock();
                    cp.begin_checkpoint(&format!("Tool: {}", call_clone.function.name));
                    
                    if let Some(path_str) = call_clone.function.arguments
                        .get("path")
                        .or(call_clone.function.arguments.get("file_path"))
                        .and_then(|v| v.as_str()) 
                    {
                        let expanded = shellexpand::tilde(path_str).to_string();
                        cp.snapshot_file(std::path::Path::new(&expanded));
                    }
                }

                let res = self_clone.process_single_tool_call(call_clone, true).await;
                
                if is_mod {
                    if res.2 {
                        let _ = self_clone.checkpoint_mgr.lock().commit_checkpoint();
                    } else {
                        self_clone.checkpoint_mgr.lock().discard_pending();
                    }
                }
                res
            };
            current_batch.push(tokio::spawn(task));
        }

        if !current_batch_calls.is_empty() {
            let approved = self.handle_batch_approval(&current_batch_calls).await;
            if approved {
                let batch_results = futures::future::join_all(current_batch).await;
                results.extend(batch_results.into_iter().filter_map(|r| r.ok()));
            } else {
                for call in current_batch_calls {
                    results.push((call.function.name.clone(), "Error: Batch execution was rejected by the user.".to_string(), false));
                }
            }
        }

        Ok(results)
    }

    async fn handle_batch_approval(&self, calls: &[ollama_rs::generation::tools::ToolCall]) -> bool {
        if !self.safe_mode.load(std::sync::atomic::Ordering::SeqCst) {
            return true;
        }

        let mut modifying_previews = Vec::new();
        let mut tool_names = Vec::new();

        for call in calls {
            let tool_name = self.repair_tool_name(&call.function.name);
            if let Some(tool) = self.tools.get(&tool_name).map(|r| r.value().clone()) {
                if tool.is_modifying() {
                    let args = &call.function.arguments;
                    let preview = tool.get_approval_preview(args).await;
                    
                    let mut prompt_chunk = String::new();
                    if let Some(p) = preview {
                        prompt_chunk.push_str(&p);
                    } else {
                        let target_path = args.get("path")
                            .or(args.get("file_path"))
                            .and_then(|v| v.as_str())
                            .map(|s| shellexpand::tilde(s).to_string());
                        
                        let new_content = args.get("content")
                            .or(args.get("new_content"))
                            .and_then(|v| v.as_str());
                        
                        if let (Some(path), Some(content)) = (&target_path, new_content) {
                            let path_buf = std::path::PathBuf::from(path);
                            let modifications = vec![(path_buf, content.to_string())];
                            let diff_preview = crate::checkpoint::generate_batch_diff(&modifications);
                            prompt_chunk.push_str(&diff_preview);
                        } else {
                            let args_preview = serde_json::to_string(args)
                                .unwrap_or_default()
                                .chars().take(200).collect::<String>();
                            prompt_chunk.push_str(&format!("Arguments: {}\n", args_preview));
                        }
                    }
                    modifying_previews.push(prompt_chunk);
                    tool_names.push(tool_name.to_uppercase());
                }
            }
        }

        if modifying_previews.is_empty() {
            return true;
        }

        let mut final_prompt = String::new();
        for chunk in modifying_previews {
            final_prompt.push_str(&chunk);
            final_prompt.push_str("\n---\n");
        }

        let cp_count = self.checkpoint_mgr.lock().checkpoint_count();
        let batch_label = if tool_names.len() > 1 {
            format!("BATCH ({} actions: {})", tool_names.len(), tool_names.join(", "))
        } else {
            tool_names[0].clone()
        };

        final_prompt.push_str(&format!(
            "APPROVE {}? [ENTER/ESC]  (⏪ {} checkpoints available)",
            batch_label,
            cp_count
        ));

        let tx_opt = self.event_tx.lock().clone();
        if let Some(tx) = tx_opt {
            let _ = tx.send(crate::tui::AgentEvent::RequestInput(
                "BATCH_APPROVAL".to_string(), 
                final_prompt
            )).await;
        }

        // Wait for user response
        let mut rx_lock = self.tool_rx.lock().await;
        if let Some(rx) = rx_lock.as_mut() {
            match tokio::time::timeout(
                tokio::time::Duration::from_secs(300),
                rx.recv()
            ).await {
                Ok(Some(crate::tui::ToolResponse::Text(ans))) => {
                    let lower = ans.trim().to_lowercase();
                    lower == "y" || lower == "yes" || lower.is_empty()
                }
                _ => false
            }
        } else {
            false
        }
    }

    async fn process_single_tool_call(&self, tool_call: ollama_rs::generation::tools::ToolCall, skip_approval: bool) -> (String, String, bool) {
        let tool_name = self.repair_tool_name(&tool_call.function.name);
            
        let mut args = tool_call.function.arguments.clone();

        // Fuzzy Repair Logic continues below...

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
            if tool.is_modifying() && !skip_approval {
                // Single-tool fallback approval if not already handled by a batch
                if !self.handle_batch_approval(&[tool_call.clone()]).await {
                    return (tool_name.clone(), 
                        format!("Error: Tool '{}' was rejected by the user.", tool_name), 
                        false);
                }
            }
            
            // Log modification if in auto-mode
            if tool.is_modifying() && !self.safe_mode.load(std::sync::atomic::Ordering::SeqCst) {
                let preview = tool.get_approval_preview(&args).await;
                let tx_opt = self.event_tx.lock().clone();
                if let Some(tx) = tx_opt {
                    if let Some(p) = preview {
                        let _ = tx.send(crate::tui::AgentEvent::CommandOutput(format!("🚀 [AUTO-MODIFY]: {}\n{}", tool_name, p))).await;
                    } else {
                        let _ = tx.send(crate::tui::AgentEvent::SystemUpdate(format!("🚀 [AUTO-MODIFY]: Executing {}", tool_name))).await;
                    }
                }
            }

            let mut last_result = (tool_name.clone(), "Error: Tool execution failed and could not be retried.".to_string(), false);
            let max_attempts = 5;

            for attempt in 1..=max_attempts {
                let start = std::time::Instant::now();
                metrics::gauge!("tool.semaphore_available_permits").set(self.concurrency_semaphore.available_permits() as f64);
                
                let _permit = self.concurrency_semaphore.acquire().await.ok();
                let context = self.get_tool_context().await;
                
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
                        
                        metrics::counter!("tool.success", "tool" => tool_name.clone()).increment(1);
                            
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
                            
                            metrics::counter!("tool.failure", "tool" => tool_name.clone()).increment(1);
                        }

                        if classification == crate::error_classifier::ErrorClass::Retryable && attempt < max_attempts {
                            // Exponential backoff with jitter
                            let wait_duration = {
                                use rand::Rng;
                                let base_wait = 2u64.pow(attempt as u32 - 1);
                                let jitter_ms = rand::rng().random_range(0..1000);
                                tokio::time::Duration::from_millis(base_wait * 1000 + jitter_ms)
                            };
                            
                            let tx_opt = self.event_tx.lock().clone();
                            if let Some(tx) = tx_opt {
                                let _ = tx.send(crate::tui::AgentEvent::SystemUpdate(
                                    format!("🔄 [{}/{}] Retrying {} in {:.1}s: {}", attempt, max_attempts, tool_name, wait_duration.as_secs_f32(), err_msg)
                                )).await;
                            }
                            tokio::time::sleep(wait_duration).await;
                            last_result = (tool_name.clone(), format!("Error (Failed after {} attempts): {}", attempt, err_msg), false);
                            continue;
                        } else if classification == crate::error_classifier::ErrorClass::Recoverable {
                            let tip = if err_msg.to_lowercase().contains("permission") || err_msg.to_lowercase().contains("sudo") {
                                "\n\nSYSTEM TIP: This looks like a permission issue. You may need to ask the user for elevated privileges (root/sudo) or use a different path."
                            } else {
                                "\n\nSYSTEM TIP: This failure might be recoverable by changing your strategy or asking the user for clarification."
                            };
                            last_result = (tool_name.to_string(), format!("Error: {}{}", err_msg, tip), false);
                            break;
                        } else {
                            last_result = (tool_name.to_string(), format!("Error: {}", err_msg), false);
                            break;
                        }
                    }
                }
            }

            let final_res = last_result;
            let call_key = format!("{}:{}", tool_name, args);
            {
                let mut stack = self.tool_repetition_stack.lock();
                if let Some(entry) = stack.iter_mut().find(|(k, _, _)| k == &call_key) {
                    entry.2 = Some(final_res.1.clone());
                }
            }
            final_res
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

    pub async fn get_tool_context(&self) -> ToolContext {
        let real_tx = self.event_tx.lock().clone();

        ToolContext {
            ollama: self.get_ollama().await.unwrap_or_else(|_| ollama_rs::Ollama::from_url(reqwest::Url::parse("http://127.0.0.1:11434").unwrap())),
            backend: self.backend.clone(),
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
            all_tools: self.tools.iter().map(|kv| kv.value().tool_info()).collect(),
            checkpoint_mgr: self.checkpoint_mgr.clone(),
        }
    }

    // Removed auto_summarize_memory (unused)

    fn extract_tool_calls(&self, content: &str) -> Result<Vec<Value>, String> {
        let block_regex = TOOL_BLOCK_RE.get_or_init(|| {
            regex::Regex::new(r"```json\s*([\s\S]*?)\s*```").unwrap()
        });
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

        // 🧠 RAW JSON CATCHER: If no markdown blocks, try parsing the entire content
        if calls.is_empty() {
            let trimmed = content.trim();
            if (trimmed.starts_with('{') && trimmed.ends_with('}')) || (trimmed.starts_with('[') && trimmed.ends_with(']')) {
                if let Ok(val) = serde_json::from_str::<Value>(trimmed) {
                    if let Some(obj) = val.as_object() {
                        if obj.contains_key("tool") || obj.contains_key("name") || obj.contains_key("function_name") || obj.contains_key("function") || obj.contains_key("action") {
                            calls.push(val);
                        }
                    } else if let Some(arr) = val.as_array() {
                        // Handle batch tool calls in a single JSON array
                        for item in arr {
                            if let Some(obj) = item.as_object() {
                                if obj.contains_key("tool") || obj.contains_key("name") || obj.contains_key("function_name") || obj.contains_key("function") || obj.contains_key("action") {
                                    calls.push(item.clone());
                                }
                            }
                        }
                    }
                }
            }
        }

        // 🧠 FUZZY TOOL CATCHER: If still no formal tool calls found, look for raw code blocks.
        if calls.is_empty() {
            let code_re = regex::Regex::new(r"```(?P<lang>\w+)?\n(?P<code>[\s\S]*?)\n```").unwrap();
            let file_re = regex::Regex::new(r"(?i)(?:file|to|in|at)\s+`?([\w\-\./]+\.(?:py|rs|js|ts|c|cpp|h|go|html|css|json|toml|yaml|yml|md|sh))`?").unwrap();
            
            for caps in code_re.captures_iter(content) {
                let code = caps.name("code").map(|m| m.as_str()).unwrap_or("");
                let lang = caps.name("lang").map(|m| m.as_str()).unwrap_or("text");

                if lang == "json" || code.is_empty() { continue; }

                // Look for a filename in the preceding 200 characters
                let block_start = caps.get(0).unwrap().start();
                let search_start = block_start.saturating_sub(200);
                let context = &content[search_start..block_start];

                if let Some(f_caps) = file_re.captures_iter(context).last() {
                    let path = f_caps.get(1).unwrap().as_str();
                    calls.push(serde_json::json!({
                        "name": "write_file",
                        "arguments": {
                            "path": path,
                            "content": code
                        }
                    }));
                }
            }
        }

        Ok(calls)
    }

    pub async fn run_tui_mode(&self, mut user_rx: tokio::sync::mpsc::Receiver<String>, stop: std::sync::Arc<std::sync::atomic::AtomicBool>) -> Result<()> {
        let mut current_task: Option<tokio::task::JoinHandle<()>> = None;

        while let Some(user_msg) = user_rx.recv().await {
             // 🧟 ZOMBIE KILLER: If a turn is already running, abort it before starting a new one.
             if let Some(task) = current_task.take() {
                 task.abort();
             }

             // Always reset stop flag before starting a new turn
             stop.store(false, std::sync::atomic::Ordering::Relaxed);

             if user_msg == "/clear" {
                 self.clear_history();
                 let tx_opt = self.event_tx.lock().clone();
                 if let Some(tx) = tx_opt {
                     let _ = tx.send(crate::tui::AgentEvent::SystemUpdate("🧹 Session Hard Reset: History and Task cleared.".to_string())).await;
                 }
                 continue;
             }

             if user_msg == "/undo" {
                 let result = self.checkpoint_mgr.lock().undo();
                 let tx_opt = self.event_tx.lock().clone();
                 if let Some(tx) = tx_opt {
                     match result {
                         Ok(summary) => {
                             let _ = tx.send(crate::tui::AgentEvent::StreamToken(summary)).await;
                             let _ = tx.send(crate::tui::AgentEvent::StreamToken(String::new())).await;
                         }
                         Err(msg) => {
                             let _ = tx.send(crate::tui::AgentEvent::SystemUpdate(format!("⚠️ {}", msg))).await;
                         }
                     }
                 }
                 continue;
             }

             if user_msg == "/checkpoints" {
                 let summary = self.checkpoint_mgr.lock().list_checkpoints();
                 let tx_opt = self.event_tx.lock().clone();
                 if let Some(tx) = tx_opt {
                     let _ = tx.send(crate::tui::AgentEvent::StreamToken(summary)).await;
                     let _ = tx.send(crate::tui::AgentEvent::StreamToken(String::new())).await;
                 }
                 continue;
             }

             let agent_clone = self.clone();
             let stop_clone = stop.clone();
             let msg_clone = user_msg.clone();

             current_task = Some(tokio::spawn(async move {
                 if let Err(e) = agent_clone.run(msg_clone, stop_clone).await {
                     let tx_opt = agent_clone.event_tx.lock().clone();
                     if let Some(tx) = tx_opt {
                         let _ = tx.send(crate::tui::AgentEvent::SystemUpdate(format!("Error: {}", e))).await;
                     }
                 }
             }));
             
             // Reset stop is not needed here as it's per-task now
        }
        Ok(())
    }

    /// Warm up the engine by sending a single dummy token request.
    /// This ensures the model is loaded into VRAM and the GPU is initialized before the user speaks.
    pub async fn warmup(&self) -> Result<()> {
        let _tx_opt = self.event_tx.lock().clone();

        // Silent inference pulse
        let dummy_history = vec![ollama_rs::generation::chat::ChatMessage::new(
            ollama_rs::generation::chat::MessageRole::User,
            "warmup".to_string()
        )];
        
        let model_name = AgentPhase::Planning.default_model();

        // We use a tiny max_len for the warmup pulse
        let _ = self.backend.read().await.stream_chat(
            model_name,
            dummy_history,
            crate::inference::SamplingConfig {
                temperature: 0.1,
                top_p: 0.9,
                repeat_penalty: 1.1,
                context_size: 1024,
            },
            Arc::new(parking_lot::Mutex::new(None)),           // event_tx
            Arc::new(std::sync::atomic::AtomicBool::new(true)), // stop
            "warmup".to_string(), // system prompt
            None, // on_tool_call
            None, // tool_registry
        ).await;

        let tx_opt = self.event_tx.lock().clone();
        if let Some(tx) = tx_opt {
            let _ = tx.send(crate::tui::AgentEvent::SystemUpdate("✅ Engine ready.".to_string())).await;
        }

        Ok(())
    }

    /// Explicitly unload the model from Ollama's memory (GPU) by sending a request with keep_alive: 0.
    pub async fn shutdown(&self) {
        let backend = self.backend.read().await;
        
        // 1. Unload primary model
        let primary_model = self.model.lock().clone();
        backend.shutdown(primary_model).await;
        
        // 2. Unload planner model if different
        if let Some(planner) = &self.planner_model {
            if planner != &self.model.lock().clone() {
                backend.shutdown(planner.clone()).await;
            }
        }
    }

    #[cfg(target_os = "macos")]
    async fn _unused_placeholder() {}
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_agent_new() {
        let memory_store = Arc::new(Mutex::new(crate::memory::MemoryStore::new("test-passphrase".to_string()).unwrap()));
        let agent = Agent::new(
            crate::inference::AgentMode::Ollama,
            "test-model".to_string(),
            "Q4_K_M".to_string(),
            "test-prompt".to_string(),
            "/tmp/tempest_test_history.json".to_string(),
            memory_store,
            "test-sub-model".to_string(),
            None,
            None,
            None,
            std::collections::HashMap::new(),
            0.05,
            0.25,
            0.95,
            0.92,
            1.18,
            1.12,
            16384,
            32768,
            None,
            None,
            None,
            None,
            None,
            None,
            false,
            true,
            Arc::new(Mutex::new(None)),
        ).await;
        assert!(agent.is_ok());
        let agent = agent.unwrap();
        assert_eq!(agent.sub_agent_model, "test-sub-model");
        assert!(!agent.tool_registry.is_empty());
        assert!(!agent.tools.is_empty());
    }
}
