// Copyright (c) 2026 Robert Simens. All Rights Reserved.
// Licensed under the Tempest AI Source-Available License.
// See LICENSE in the project root for full license information.

use crate::inference::{AgentMode, Backend};
use crate::memory::MemoryStore;
use crate::tools::ToolContext;
use colored::*;
use dashmap::DashMap;
use miette::IntoDiagnostic;
use miette::{Result, miette};
use ollama_rs::{
    Ollama,
    generation::chat::{ChatMessage, MessageRole},
};
use parking_lot::Mutex;
use serde_json::Value;
use std::path::Path;
use std::sync::Arc;
use std::sync::OnceLock;
use std::sync::atomic::AtomicBool;
use syntect::highlighting::ThemeSet;
use syntect::parsing::SyntaxSet;

/// Stack tracking tool repetitions: (tool_name, args_hash, result_snippet)
pub type ToolRepetitionStack = Arc<Mutex<Vec<(String, String, Option<String>)>>>;

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum AgentPhase {
    Planning,  // Strong reasoning model (DeepSeek R1)
    Execution, // Fast & accurate coding model (Qwen2.5-Coder)
    Testing,   // Verification & error-catching model (Ministral 8B)
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
    executed_mid_stream: Vec<(
        ollama_rs::generation::tools::ToolCall,
        (String, String, bool),
    )>,
    kv_cache_hit_pct: Option<f32>,
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
    PendingTools {
        tool_calls: Vec<ollama_rs::generation::tools::ToolCall>,
    },
    /// Actively running the approved tool batch
    ExecutingTools {
        tool_calls: Vec<ollama_rs::generation::tools::ToolCall>,
        results: Vec<ToolResult>,
    },
    /// Turn completed
    Done,
}

impl AgentStreamState {
    pub fn name(&self) -> &'static str {
        match self {
            AgentStreamState::Thinking { .. } => "Thinking",
            AgentStreamState::StreamingContent { .. } => "StreamingContent",
            AgentStreamState::PendingTools { .. } => "PendingTools",
            AgentStreamState::ExecutingTools { .. } => "ExecutingTools",
            AgentStreamState::Done => "Done",
        }
    }
}

#[derive(Clone)]
pub struct Agent {
    #[allow(dead_code)]
    pub mode: AgentMode,
    pub phase: Arc<Mutex<AgentPhase>>,
    pub backend: Arc<tokio::sync::RwLock<Backend>>,
    model: Arc<Mutex<String>>,
    pub history: Arc<Mutex<Vec<ChatMessage>>>,
    tools: Arc<DashMap<String, Arc<dyn crate::tools::AgentTool>>>,
    #[allow(dead_code)]
    tool_registry: Vec<ollama_rs::generation::tools::ToolInfo>,
    tool_rag_index: Arc<tokio::sync::RwLock<crate::tool_rag::ToolVectorIndex>>,
    system_prompt: String,
    recent_tool_calls: Arc<DashMap<String, String>>,
    history_path: String,
    pub state_store: Arc<dyn layer0::StateStore + Send + Sync>,
    brain_path: std::path::PathBuf,
    pub planning_mode: Arc<Mutex<bool>>,
    pub task_context: Arc<Mutex<String>>,
    pub vector_brain: Arc<Mutex<crate::vector_brain::VectorBrain>>,
    #[allow(dead_code)]
    pub sub_agent_model: String,
    pub embedding_model: String,
    #[allow(dead_code)]
    syntax_set: SyntaxSet,
    #[allow(dead_code)]
    theme_set: Arc<ThemeSet>,
    pub telemetry: Arc<Mutex<String>>,
    pub is_root: Arc<AtomicBool>,
    pub concurrency_semaphore: Arc<tokio::sync::Semaphore>,
    pub event_tx: Arc<Mutex<Option<tokio::sync::mpsc::Sender<crate::tui::AgentEvent>>>>,
    pub tool_rx:
        Arc<tokio::sync::Mutex<Option<tokio::sync::mpsc::Receiver<crate::tui::ToolResponse>>>>,
    pub rules: crate::rules::RuleEngine,
    pub recent_failures: Arc<DashMap<String, usize>>,
    pub sentinel: crate::sentinel::SentinelManager,
    pub editor_context: Arc<Mutex<Option<Value>>>,
    pub safe_mode: Arc<AtomicBool>,
    pub hardcore_mode: Arc<AtomicBool>,
    pub tool_stats: Arc<DashMap<String, (usize, usize)>>,
    pub tool_repetition_stack: ToolRepetitionStack,
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
    pub pa_memory_mb: Option<usize>,
    pub llm_extract_retries: usize,
    pub ollama_remote: Option<crate::OllamaRemoteConfig>,
    pub planning_enabled: bool,
    pub overwatch: crate::overwatch::OverwatchEngine,
    pub checkpoint_mgr: crate::checkpoint::SharedCheckpointManager,
    pub memory_store: Arc<Mutex<MemoryStore>>,
    pub mcp_clients: Arc<DashMap<String, Arc<tokio::sync::Mutex<crate::mcp::McpClient>>>>,
    pub tool_registry_skg: Arc<skg_tool::ToolRegistry>,
    pub vram_time_sharing: bool,
    pub session_id: String,
    pub start_time: std::time::Instant,
    pub api_time_ms: Arc<std::sync::atomic::AtomicU64>,
    pub tool_time_ms: Arc<std::sync::atomic::AtomicU64>,
    pub total_tokens: Arc<std::sync::atomic::AtomicU64>,
    pub tool_engine: String,
    pub temp_override: Arc<Mutex<Option<f32>>>,
    pub ctx_override: Arc<Mutex<Option<u64>>>,
    pub role_override: Arc<Mutex<Option<String>>>,
    pub kv_cache_hit_history: Arc<Mutex<Vec<f32>>>,
}

pub struct AgentStream<'a> {
    pub agent: &'a Agent,
    pub state: AgentStreamState,
    pub stop: Arc<AtomicBool>,
    pub iteration: usize,
    pub decomposer: crate::turn_kit::TempestTurnDecomposer,
    pub silent_failure_count: usize,
}

impl<'a> AgentStream<'a> {
    pub fn new(agent: &'a Agent, stop: Arc<AtomicBool>) -> Self {
        let mut decomposer = crate::turn_kit::TempestTurnDecomposer::new();
        // Auto-detect project verification environment
        if std::path::Path::new("Cargo.toml").exists() {
            decomposer.register_hook(crate::turn_kit::VerificationHook {
                name: "Cargo Check".to_string(),
                command: "cargo check".to_string(),
            });
            // If the project has a tests directory or test files, auto-run tests too
            if std::path::Path::new("tests").exists()
                || std::path::Path::new("src/tests.rs").exists()
            {
                decomposer.register_hook(crate::turn_kit::VerificationHook {
                    name: "Cargo Test".to_string(),
                    command: "cargo test".to_string(),
                });
            }
        } else if std::path::Path::new("package.json").exists() {
            decomposer.register_hook(crate::turn_kit::VerificationHook {
                name: "NPM Build".to_string(),
                command: "npm run build".to_string(),
            });
        }

        Self {
            agent,
            state: AgentStreamState::Thinking {
                accumulated: String::new(),
            },
            stop,
            iteration: 0,
            decomposer,
            silent_failure_count: 0,
        }
    }

    fn handle_silent_failure(&mut self) {
        let is_silent_failure = !self.agent.history.lock().is_empty();
        if is_silent_failure && self.silent_failure_count < 3 {
            self.silent_failure_count += 1;
            let nudge = "⚠️ [SILENT FAILURE]: You reasoned about an action but didn't output a tool call. YOU must output the JSON tool call now to finish the task.".to_string();
            self.agent
                .history
                .lock()
                .push(ollama_rs::generation::chat::ChatMessage::new(
                    ollama_rs::generation::chat::MessageRole::System,
                    nudge,
                ));
            self.state = AgentStreamState::Thinking {
                accumulated: String::new(),
            };
        } else {
            if is_silent_failure {
                let tx_opt = self.agent.event_tx.lock().clone();
                if let Some(tx) = tx_opt {
                    let _ = tx.try_send(crate::tui::AgentEvent::SystemUpdate(
                        "⚠️ [SILENT FAILURE]: Hit maximum silent failure retry limit. Ending turn."
                            .to_string(),
                    ));
                }
            }
            self.state = AgentStreamState::Done;
        }
    }

    pub async fn transition(&mut self) -> Result<()> {
        let current_state = self.state.clone();
        match current_state {
            AgentStreamState::Thinking { accumulated } => {
                self.agent.inject_state_context();
                let ctx_limit = self.agent.calculate_optimal_ctx().await;
                self.agent.run_sentinel_stage(ctx_limit).await?;

                let active_phase = self.agent.phase.lock().clone();
                let run_phase = if !self.agent.planning_enabled {
                    AgentPhase::Execution
                } else {
                    active_phase
                };

                let decomposer_phase = match run_phase {
                    AgentPhase::Planning => crate::turn_kit::TurnPhase::Planning,
                    AgentPhase::Execution => crate::turn_kit::TurnPhase::Executing,
                    AgentPhase::Testing => crate::turn_kit::TurnPhase::Verifying,
                };

                let phase_lbl = self.decomposer.transition_phase(decomposer_phase);
                let tx_opt = self.agent.event_tx.lock().clone();
                if let Some(ref tx) = tx_opt {
                    let _ = tx.try_send(crate::tui::AgentEvent::SystemUpdate(format!(
                        "🌪️ [SKELEGENT TURN-KIT]: Entering {}",
                        phase_lbl
                    )));
                }

                // If we have accumulated reasoning from a previous step, broadcast it
                if !accumulated.is_empty() {
                    let tx_opt = self.agent.event_tx.lock().clone();
                    if let Some(tx) = tx_opt {
                        let _ = tx.try_send(crate::tui::AgentEvent::SystemUpdate(format!(
                            "🧠 Thought Process finalized ({} chars)",
                            accumulated.len()
                        )));
                    }
                }

                self.agent.switch_phase(run_phase.clone()).await?;

                let planner_output = self.agent.planner_turn(self.stop.clone()).await?;
                if let Some(hit) = planner_output.kv_cache_hit_pct {
                    self.decomposer.kv_cache_hit_pct = Some(hit);
                }

                // Update state with new reasoning
                self.state = AgentStreamState::Thinking {
                    accumulated: planner_output.reasoning.clone(),
                };

                let mut all_tool_calls = Vec::new();
                for native_call in planner_output.native_tool_calls {
                    all_tool_calls.push(native_call);
                }
                if all_tool_calls.is_empty() {
                    // Try content first, then fall through to reasoning
                    // (models sometimes emit tool calls inside <think> blocks)
                    let sources = [&planner_output.content, &planner_output.reasoning];
                    for source in sources {
                        if !all_tool_calls.is_empty() {
                            break;
                        }
                        if source.is_empty() {
                            continue;
                        }
                        if let Ok(legacy_calls) = self.agent.extract_tool_calls(source) {
                            for call in legacy_calls {
                                // Normalize legacy calls to the native format
                                let name = call
                                    .get("name")
                                    .or(call.get("tool"))
                                    .or(call.get("function"))
                                    .and_then(|v| v.as_str());
                                let args = call
                                    .get("arguments")
                                    .or(call.get("args"))
                                    .cloned()
                                    .unwrap_or(serde_json::json!({}));

                                if let Some(n) = name {
                                    // 🛡️ REASONING RESCUE: Tool call found in thinking block
                                    if std::ptr::eq(source, &planner_output.reasoning) {
                                        let tx_opt = self.agent.event_tx.lock().clone();
                                        if let Some(tx) = tx_opt {
                                            let _ = tx.try_send(
                                                crate::tui::AgentEvent::SentinelUpdate {
                                                    active: vec!["Reasoning Rescue".to_string()],
                                                    log: format!(
                                                        "Rescued tool call '{}' from <think> block",
                                                        n
                                                    ),
                                                },
                                            );
                                        }
                                    }
                                    let tc = ollama_rs::generation::tools::ToolCall {
                                        function: ollama_rs::generation::tools::ToolCallFunction {
                                            name: n.to_string(),
                                            arguments: args,
                                        },
                                    };
                                    {
                                        let mut hist = self.agent.history.lock();
                                        if let Some(last_msg) = hist.last_mut()
                                            && last_msg.role == MessageRole::Assistant
                                            && !last_msg.tool_calls.iter().any(|c| {
                                                c.function.name == tc.function.name
                                                    && c.function.arguments == tc.function.arguments
                                            })
                                        {
                                            last_msg.tool_calls.push(tc.clone());
                                        }
                                    }
                                    all_tool_calls.push(tc);
                                }
                            }
                        }
                    }
                }

                // --- 🛡️ DEDUPLICATION GUARD ---
                let mut unique_calls = Vec::new();
                for call in all_tool_calls {
                    let is_duplicate =
                        unique_calls
                            .iter()
                            .any(|u: &ollama_rs::generation::tools::ToolCall| {
                                u.function.name == call.function.name
                                    && u.function.arguments == call.function.arguments
                            });
                    if !is_duplicate {
                        unique_calls.push(call);
                    }
                }
                all_tool_calls = unique_calls;

                // Handle mid-stream results
                let mid_stream_results = planner_output.executed_mid_stream;
                if !mid_stream_results.is_empty() {
                    let feedback: Vec<_> = mid_stream_results
                        .iter()
                        .map(|(_, res)| res.clone())
                        .collect();
                    self.agent.process_tool_feedback_stage(feedback).await?;

                    // Filter out already executed mid-stream tool calls from all_tool_calls
                    all_tool_calls.retain(|call| {
                        !mid_stream_results.iter().any(|(mid_call, _)| {
                            mid_call.function.name == call.function.name
                                && mid_call.function.arguments == call.function.arguments
                        })
                    });
                }

                if !all_tool_calls.is_empty() {
                    self.state = AgentStreamState::PendingTools {
                        tool_calls: all_tool_calls,
                    };
                } else if !planner_output.content.trim().is_empty() {
                    // Since we finished planning, switch the phase to Execution for the next turn
                    if matches!(run_phase, AgentPhase::Planning) {
                        self.agent.switch_phase(AgentPhase::Execution).await?;
                    }
                    self.state = AgentStreamState::StreamingContent {
                        content: planner_output.content,
                    };
                } else {
                    if matches!(run_phase, AgentPhase::Planning) {
                        // --- ⚡ AUTOMATIC EXECUTION HANDOVER (Hardened) ---
                        let tx_opt = self.agent.event_tx.lock().clone();
                        if let Some(ref tx) = tx_opt {
                            let _ = tx.try_send(crate::tui::AgentEvent::SystemUpdate(
                                "⚡ HANDOVER: Handing context to EXECUTOR...".to_string(),
                            ));
                        }

                        // INJECT REASONING INTO HISTORY: This links the phases!
                        // CAP REASONING: Prevent unbounded accumulation causing thinking loops
                        if !planner_output.reasoning.is_empty() {
                            let max_reasoning_chars = 2000; // ~500 tokens max, keeps thinking bounded
                            let reasoning_to_inject =
                                if planner_output.reasoning.len() > max_reasoning_chars {
                                    format!(
                                        "{}...[truncated]",
                                        &planner_output.reasoning[..max_reasoning_chars]
                                    )
                                } else {
                                    planner_output.reasoning.clone()
                                };

                            let mut history = self.agent.history.lock();
                            history.push(ollama_rs::generation::chat::ChatMessage::new(
                                ollama_rs::generation::chat::MessageRole::Assistant,
                                format!("<think>\n{}\n</think>", reasoning_to_inject),
                            ));
                        }

                        self.agent.switch_phase(AgentPhase::Execution).await?;

                        // Small buffer for Ollama to settle VRAM swap
                        tokio::time::sleep(std::time::Duration::from_millis(200)).await;

                        let mut output = self.agent.planner_turn(self.stop.clone()).await?;
                        if let Some(hit) = output.kv_cache_hit_pct {
                            self.decomposer.kv_cache_hit_pct = Some(hit);
                        }

                        // If the first handover turn returns nothing, try one more time
                        if output.content.trim().is_empty()
                            && !self.stop.load(std::sync::atomic::Ordering::Relaxed)
                        {
                            if let Some(ref tx) = tx_opt {
                                let _ = tx.try_send(crate::tui::AgentEvent::SubagentStatus(Some(
                                    "📡 Retrying Handover...".to_string(),
                                )));
                            }
                            tokio::time::sleep(std::time::Duration::from_millis(500)).await;
                            output = self.agent.planner_turn(self.stop.clone()).await?;
                            if let Some(hit) = output.kv_cache_hit_pct {
                                self.decomposer.kv_cache_hit_pct = Some(hit);
                            }
                        }

                        let mut all_tool_calls = Vec::new();
                        for native_call in output.native_tool_calls {
                            all_tool_calls.push(native_call);
                        }
                        if all_tool_calls.is_empty() {
                            let sources = [&output.content, &output.reasoning];
                            for source in sources {
                                if !all_tool_calls.is_empty() {
                                    break;
                                }
                                if source.is_empty() {
                                    continue;
                                }
                                if let Ok(legacy_calls) = self.agent.extract_tool_calls(source) {
                                    for call in legacy_calls {
                                        let name = call
                                            .get("name")
                                            .or(call.get("tool"))
                                            .or(call.get("function"))
                                            .and_then(|v| v.as_str());
                                        let args = call
                                            .get("arguments")
                                            .or(call.get("args"))
                                            .cloned()
                                            .unwrap_or(serde_json::json!({}));
                                        if let Some(n) = name {
                                            let tc = ollama_rs::generation::tools::ToolCall {
                                                function:
                                                    ollama_rs::generation::tools::ToolCallFunction {
                                                        name: n.to_string(),
                                                        arguments: args,
                                                    },
                                            };
                                            {
                                                let mut hist = self.agent.history.lock();
                                                if let Some(last_msg) = hist.last_mut()
                                                    && last_msg.role == MessageRole::Assistant
                                                    && !last_msg.tool_calls.iter().any(|c| {
                                                        c.function.name == tc.function.name
                                                            && c.function.arguments
                                                                == tc.function.arguments
                                                    })
                                                {
                                                    last_msg.tool_calls.push(tc.clone());
                                                }
                                            }
                                            all_tool_calls.push(tc);
                                        }
                                    }
                                }
                            }
                        }

                        // Deduplicate
                        let mut unique_calls = Vec::new();
                        for call in all_tool_calls {
                            let is_duplicate = unique_calls.iter().any(
                                |u: &ollama_rs::generation::tools::ToolCall| {
                                    u.function.name == call.function.name
                                        && u.function.arguments == call.function.arguments
                                },
                            );
                            if !is_duplicate {
                                unique_calls.push(call);
                            }
                        }
                        all_tool_calls = unique_calls;

                        // Handle mid-stream results
                        let mid_stream_results = output.executed_mid_stream;
                        if !mid_stream_results.is_empty() {
                            let feedback: Vec<_> = mid_stream_results
                                .iter()
                                .map(|(_, res)| res.clone())
                                .collect();
                            self.agent.process_tool_feedback_stage(feedback).await?;

                            all_tool_calls.retain(|call| {
                                !mid_stream_results.iter().any(|(mid_call, _)| {
                                    mid_call.function.name == call.function.name
                                        && mid_call.function.arguments == call.function.arguments
                                })
                            });
                        }

                        if !all_tool_calls.is_empty() {
                            self.state = AgentStreamState::PendingTools {
                                tool_calls: all_tool_calls,
                            };
                        } else if !output.content.trim().is_empty() {
                            self.state = AgentStreamState::StreamingContent {
                                content: output.content,
                            };
                        } else {
                            self.handle_silent_failure();
                        }
                    } else {
                        self.handle_silent_failure();
                    }
                }
            }
            AgentStreamState::PendingTools { tool_calls: calls } => {
                let phase_lbl = self
                    .decomposer
                    .transition_phase(crate::turn_kit::TurnPhase::Executing);
                let tx_opt = self.agent.event_tx.lock().clone();
                if let Some(ref tx) = tx_opt {
                    let _ = tx.try_send(crate::tui::AgentEvent::SystemUpdate(format!(
                        "🌪️ [SKELEGENT TURN-KIT]: Entering {}",
                        phase_lbl
                    )));
                }

                let backend = self.agent.backend.read().await.clone();
                let is_local = matches!(
                    backend.mode(),
                    crate::inference::AgentMode::Ollama
                        | crate::inference::AgentMode::MLX
                        | crate::inference::AgentMode::LMStudio
                );

                if !is_local {
                    // === Structured validation for cloud/bridge models ===
                    let mut ctx = skg_context_engine::Context::new();
                    crate::overwatch::register_overwatch_rules(&mut ctx);

                    let model = self.agent.model.lock().clone();
                    let provider = crate::skg_adapter::SkgBackendProvider { backend, model };

                    let prompt = format!(
                        "Please validate the following tool calls to ensure they are safe and properly formatted: {:?}",
                        calls
                    );
                    ctx.messages.push(layer0::context::Message::new(
                        layer0::context::Role::User,
                        layer0::content::Content::text(prompt),
                    ));
                    let schema_val = serde_json::json!({
                        "type": "object",
                        "properties": {
                            "is_valid": { "type": "boolean", "description": "Whether the tool calls are valid and safe" },
                            "reason": { "type": "string", "description": "Reason for validation decision" }
                        },
                        "required": ["is_valid", "reason"]
                    });

                    let schema =
                        skg_context_engine::OutputSchema::text_json(schema_val, |v| Ok(v.clone()));

                    let config = skg_context_engine::react::ReactLoopConfig {
                        system_prompt: "You are a tool validator. You must approve the tool call if it is well-formed. Assume all tools are registered and valid. Return is_valid: true unless it's severely malicious.".into(),
                        model: None,
                        max_tokens: Some(1024),
                        temperature: Some(0.0),
                        tool_filter: None,
                    };

                    let tools = self.agent.tool_registry_skg.as_ref();
                    let tool_ctx =
                        skg_tool::ToolCallContext::new(layer0::id::OperatorId::from("val"));

                    // Notify UI we are validating
                    if let Some(tx) = self.agent.event_tx.lock().clone() {
                        let _ = tx.try_send(crate::tui::AgentEvent::SubagentStatus(Some(
                            "🛡️ [SKG]: Running structured tool validation...".to_string(),
                        )));
                    }

                    let validation_res = skg_context_engine::react::react_loop_structured(
                        &mut ctx, &provider, tools, &tool_ctx, &config, &schema,
                    )
                    .await;

                    match validation_res {
                        Ok((value, _)) => {
                            let is_valid = value
                                .get("is_valid")
                                .and_then(|v| v.as_bool())
                                .unwrap_or(true);
                            if !is_valid {
                                let reason = value
                                    .get("reason")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("Validation failed");
                                self.state = AgentStreamState::StreamingContent {
                                    content: format!("\n🛡️ SKG Validation Intercept: {}\n", reason),
                                };
                                return Ok(());
                            }
                        }
                        Err(e) => {
                            self.state = AgentStreamState::StreamingContent {
                                content: format!("\n⚠️ SKG Validation Error: {}\n", e),
                            };
                            return Ok(());
                        }
                    }
                }

                // ============================================================
                // 🛡️ FAST-PATH OVERWATCH — Tool Call Validation (ALL MODELS)
                // ============================================================
                // This runs for BOTH local and cloud models. It catches:
                // - Batch destructive writes (≥3 write_file in one turn)
                // - Tiny writes to critical files (Cargo.toml, main.rs) without user intent
                // Added after the incident where a hallucinating model overwrote the project.
                {
                    // Convert tool calls to JSON values for the overwatch engine
                    let tool_call_values: Vec<serde_json::Value> = calls
                        .iter()
                        .map(|call| {
                            serde_json::json!({
                                "tool": call.function.name,
                                "arguments": call.function.arguments
                            })
                        })
                        .collect();

                    // Combine recent user messages for intent matching to prevent guard clipping on simple confirmations
                    let last_user_msg = {
                        let history = self.agent.history.lock();
                        let user_msgs: Vec<String> = history
                            .iter()
                            .filter(|m| m.role == ollama_rs::generation::chat::MessageRole::User)
                            .map(|m| m.content.clone())
                            .collect();
                        if user_msgs.is_empty() {
                            None
                        } else {
                            let start = user_msgs.len().saturating_sub(5);
                            Some(user_msgs[start..].join("\n"))
                        }
                    };

                    let overwatch_verdict = self
                        .agent
                        .overwatch
                        .validate_tool_calls(&tool_call_values, last_user_msg.as_deref());

                    if let crate::overwatch::OverwatchVerdict::Intercept {
                        correction,
                        log,
                        rule_name,
                    } = overwatch_verdict
                    {
                        let tx_opt = self.agent.event_tx.lock().clone();
                        if let Some(tx) = tx_opt {
                            let _ = tx.try_send(crate::tui::AgentEvent::SentinelUpdate {
                                active: vec![rule_name.clone()],
                                log: log.clone(),
                            });
                        }

                        // Inject the correction into history and force re-think
                        self.agent
                            .history
                            .lock()
                            .push(ChatMessage::new(MessageRole::System, correction));
                        self.agent.save_history()?;
                        self.state = AgentStreamState::Thinking {
                            accumulated: String::new(),
                        };
                        return Ok(());
                    }
                }

                self.state = AgentStreamState::ExecutingTools {
                    tool_calls: calls.clone(),
                    results: Vec::new(),
                };

                let execution_results = self.agent.executor_dispatch(calls.clone()).await?;
                let mut tool_results = Vec::new();
                for (name, _args, result, success) in execution_results {
                    tool_results.push(ToolResult {
                        tool_name: name,
                        result,
                        is_success: success,
                    });
                }

                // Update state with results
                self.state = AgentStreamState::ExecutingTools {
                    tool_calls: calls.clone(),
                    results: tool_results.clone(),
                };

                let feedback_batch: Vec<_> = tool_results
                    .clone()
                    .into_iter()
                    .map(|r| (r.tool_name, r.result, r.is_success))
                    .collect();
                self.agent
                    .process_tool_feedback_stage(feedback_batch)
                    .await?;

                let phase_lbl = self
                    .decomposer
                    .transition_phase(crate::turn_kit::TurnPhase::Verifying);
                let tx_opt = self.agent.event_tx.lock().clone();
                if let Some(ref tx) = tx_opt {
                    let _ = tx.try_send(crate::tui::AgentEvent::SystemUpdate(format!(
                        "🌪️ [SKELEGENT TURN-KIT]: Entering {}",
                        phase_lbl
                    )));
                }

                // --- 🛡️ VERIFICATION SENTINEL ---
                // If any modifications were made, we automatically trigger a verification check
                let mut was_modifying = false;
                for res in &tool_results {
                    if res.is_success
                        && let Some(tool) = self
                            .agent
                            .tools
                            .get(&res.tool_name)
                            .map(|r| r.value().clone())
                        && tool.is_modifying()
                    {
                        was_modifying = true;
                        break;
                    }
                }

                if was_modifying {
                    let hooks_to_run = if !self.decomposer.hooks.is_empty() {
                        self.decomposer.hooks.clone()
                    } else {
                        // Fallback logic
                        let mut fallbacks = Vec::new();
                        if std::path::Path::new("Cargo.toml").exists() {
                            fallbacks.push(crate::turn_kit::VerificationHook {
                                name: "Cargo Check".to_string(),
                                command: "cargo check".to_string(),
                            });
                        }
                        fallbacks
                    };

                    let mut verification_failures = Vec::new();

                    for hook in hooks_to_run {
                        let tx_opt = self.agent.event_tx.lock().clone();
                        if let Some(tx) = tx_opt {
                            let _ = tx.try_send(crate::tui::AgentEvent::SubagentStatus(Some(
                                format!("🔬 [SENTINEL]: Verifying via {}...", hook.name),
                            )));
                        }

                        // Parse command and args
                        let parts: Vec<&str> = hook.command.split_whitespace().collect();
                        if parts.is_empty() {
                            continue;
                        }

                        let mut cmd = tokio::process::Command::new(parts[0]);
                        if parts.len() > 1 {
                            cmd.args(&parts[1..]);
                        }

                        // Fallback/standard arg handling: if the command is cargo, disable colors
                        if parts[0] == "cargo" {
                            cmd.arg("--color").arg("never");
                        }

                        let output = cmd.output().await;
                        match output {
                            Ok(out) => {
                                if !out.status.success() {
                                    let stdout = String::from_utf8_lossy(&out.stdout);
                                    let stderr = String::from_utf8_lossy(&out.stderr);

                                    let mut error_details = String::new();
                                    if !stderr.trim().is_empty() {
                                        error_details.push_str(&stderr);
                                    }
                                    if !stdout.trim().is_empty() {
                                        if !error_details.is_empty() {
                                            error_details.push_str("\n--- STDOUT Output ---\n");
                                        }
                                        error_details.push_str(&stdout);
                                    }

                                    verification_failures.push((hook.name, error_details));
                                }
                            }
                            Err(e) => {
                                verification_failures
                                    .push((hook.name, format!("Failed to execute command: {}", e)));
                            }
                        }
                    }

                    if !verification_failures.is_empty() {
                        // INJECT FAILURE DETAILS DIRECTLY INTO HISTORY
                        let mut reprimand = "🛑 [VERIFICATION FAILED]: The verification suite has identified issues. You MUST resolve these errors before reporting completion:\n".to_string();
                        for (name, error) in verification_failures {
                            reprimand.push_str(&format!(
                                "\n### [Failure] {}\n```\n{}\n```\n",
                                name, error
                            ));
                        }

                        self.agent
                            .history
                            .lock()
                            .push(ChatMessage::new(MessageRole::System, reprimand));

                        if let Some(tx) = self.agent.event_tx.lock().clone() {
                            let _ = tx.try_send(crate::tui::AgentEvent::SystemUpdate(
                                "⚠️ [SENTINEL]: Verification failed. Force-correcting..."
                                    .to_string(),
                            ));
                        }
                    } else {
                        if let Some(tx) = self.agent.event_tx.lock().clone() {
                            let _ = tx.try_send(crate::tui::AgentEvent::SystemUpdate(
                                "✅ [SENTINEL]: All verification checks passed.".to_string(),
                            ));
                        }
                    }
                }

                self.iteration += 1;
                self.state = AgentStreamState::Thinking {
                    accumulated: String::new(),
                };
            }
            AgentStreamState::StreamingContent { content } => {
                let content = content.clone();

                // ============================================================
                // 🛡️ OVERWATCH ENGINE — Pre-Reaction Context Rules
                // ============================================================
                // Run the Overwatch engine FIRST. If it intercepts, we force
                // the model to re-roll immediately without any further checks.
                let overwatch_verdict = self.agent.overwatch.evaluate_pre_reaction(&content);

                if let crate::overwatch::OverwatchVerdict::Intercept {
                    correction,
                    log,
                    rule_name,
                } = overwatch_verdict
                {
                    let tx_opt = self.agent.event_tx.lock().clone();
                    if let Some(tx) = tx_opt {
                        let _ = tx.try_send(crate::tui::AgentEvent::SentinelUpdate {
                            active: vec![rule_name],
                            log,
                        });
                    }

                    // Inject harsh backpressure into history
                    self.agent
                        .history
                        .lock()
                        .push(ChatMessage::new(MessageRole::System, correction));
                    self.agent.save_history()?;

                    // Force re-roll: clear accumulated reasoning
                    self.state = AgentStreamState::Thinking {
                        accumulated: String::new(),
                    };
                } else {
                    // ============================================================
                    // 🔧 LEGACY GUARDS — Raw Code & Delegation Detection
                    // ============================================================
                    let json_is_tool_call = if content.contains("```json") {
                        let lower = content.to_lowercase();
                        lower.contains("\"tool\"")
                            || lower.contains("\"name\"")
                            || lower.contains("\"function\"")
                    } else {
                        false
                    };

                    let contains_raw_code = content.contains("```rust")
                        || content.contains("```python")
                        || content.contains("```py")
                        || content.contains("```javascript")
                        || content.contains("```js")
                        || content.contains("```sh")
                        || content.contains("```bash")
                        || (content.contains("```json") && !json_is_tool_call)
                        || (content.contains("```") && content.len() > 20 && !json_is_tool_call);

                    let mut stripped_content = content.clone();
                    if let Some(start) = stripped_content.find("<think>") {
                        if let Some(end) = stripped_content.find("</think>") {
                            if end > start {
                                stripped_content.replace_range(start..end + 8, "");
                            }
                        } else {
                            stripped_content.replace_range(start.., "");
                        }
                    }

                    let lower_content = stripped_content.to_lowercase();
                    let is_delegating = lower_content.contains("you generate")
                        || lower_content.contains("you write")
                        || lower_content.contains("you create")
                        || lower_content.contains("let me know when you")
                        || (lower_content.contains("please use the tool")
                            && !lower_content.contains("i will"));

                    let model_name_raw = self.agent.model.lock().clone();
                    let model_name = model_name_raw.to_lowercase();

                    let mlx_path = if let Some(preset) = self.agent.mlx_presets.get(&model_name_raw)
                    {
                        if let Some(path) = &preset.path {
                            path.to_lowercase()
                        } else if let Some(repo) = &preset.repo {
                            repo.to_lowercase()
                        } else {
                            String::new()
                        }
                    } else {
                        String::new()
                    };

                    let planner_name = self
                        .agent
                        .planner_model
                        .clone()
                        .unwrap_or_default()
                        .to_lowercase();
                    let is_r1 = model_name.contains("r1")
                        || model_name.contains("deepseek")
                        || model_name.contains("deep-seek")
                        || model_name.contains("qwq")
                        || model_name.contains("centurion")
                        || model_name.contains("battle")
                        || model_name.contains("reasoning")
                        || mlx_path.contains("r1")
                        || mlx_path.contains("deepseek")
                        || mlx_path.contains("deep-seek")
                        || mlx_path.contains("qwq")
                        || mlx_path.contains("centurion")
                        || mlx_path.contains("battle")
                        || mlx_path.contains("reasoning")
                        || planner_name.contains("r1")
                        || planner_name.contains("deepseek")
                        || planner_name.contains("deep-seek")
                        || planner_name.contains("qwq")
                        || planner_name.contains("centurion")
                        || planner_name.contains("battle")
                        || planner_name.contains("reasoning");

                    if (contains_raw_code && !is_r1) || is_delegating {
                        let reprimand = if is_delegating {
                            "⚠️ [ROLE REMINDER]: Assistant, YOU are the engineer with the tools. The User cannot help you with file operations. Please re-issue your response and use the correct `write_file` or `run_command` JSON tool call yourself.".to_string()
                        } else {
                            "🛑 CRITICAL ERROR: Your previous response was REJECTED because it contained raw markdown code blocks. YOU ARE FORBIDDEN from using backticks for code. Use the `write_file` tool call ONLY. Please re-think your strategy and use the tool now.".to_string()
                        };

                        let sentinel_name = if is_delegating {
                            "Identity Guard"
                        } else {
                            "Tool Guard"
                        };
                        let log_msg = if is_delegating {
                            "Blocked delegation to user"
                        } else {
                            "Blocked raw code output"
                        };

                        let tx_opt = self.agent.event_tx.lock().clone();
                        if let Some(tx) = tx_opt {
                            let _ = tx.try_send(crate::tui::AgentEvent::SentinelUpdate {
                                active: vec![sentinel_name.to_string()],
                                log: log_msg.to_string(),
                            });
                        }

                        self.agent
                            .history
                            .lock()
                            .push(ChatMessage::new(MessageRole::System, reprimand));
                        self.agent.save_history()?;

                        self.state = AgentStreamState::Thinking {
                            accumulated: String::new(),
                        };
                    } else {
                        let is_done = lower_content.contains("done:")
                            || lower_content.contains("task complete")
                            || lower_content.contains("all tasks finished");

                        if is_done {
                            self.state = AgentStreamState::Done;
                        } else {
                            let tx_opt = self.agent.event_tx.lock().clone();

                            // Silent failure nudge
                            let active_phase = self.agent.phase.lock().clone();
                            let is_silent_failure =
                                (content.len() < 15 && !self.agent.history.lock().is_empty() && {
                                    let last_msg = self.agent.history.lock().last().cloned();
                                    let reasoning_len = last_msg
                                        .and_then(|m| m.thinking)
                                        .map(|s| s.len())
                                        .unwrap_or(0);
                                    reasoning_len > 100
                                }) || ((matches!(active_phase, AgentPhase::Execution)
                                    || matches!(active_phase, AgentPhase::Testing))
                                    && !is_done);

                            if is_silent_failure && self.silent_failure_count < 3 {
                                self.silent_failure_count += 1;
                                let nudge = "⚠️ [SILENT FAILURE]: You reasoned about an action but didn't output a tool call. YOU must output the JSON tool call now to finish the task.".to_string();
                                self.agent.history.lock().push(
                                    ollama_rs::generation::chat::ChatMessage::new(
                                        ollama_rs::generation::chat::MessageRole::System,
                                        nudge,
                                    ),
                                );
                                self.state = AgentStreamState::Thinking {
                                    accumulated: String::new(),
                                };
                            } else {
                                if is_silent_failure {
                                    if let Some(tx) = tx_opt {
                                        let _ = tx.try_send(crate::tui::AgentEvent::SystemUpdate(
                                            "⚠️ [SILENT FAILURE]: Hit maximum silent failure retry limit. Ending turn."
                                                .to_string(),
                                        ));
                                    }
                                } else {
                                    if let Some(tx) = tx_opt {
                                        let _ = tx.try_send(crate::tui::AgentEvent::SystemUpdate(
                                            "🔄 MOMENTUM: Response received. Turn completed."
                                                .to_string(),
                                        ));
                                    }
                                }
                                self.state = AgentStreamState::Done;
                            }
                        }
                    }
                }
            }

            AgentStreamState::ExecutingTools {
                tool_calls,
                results,
            } => {
                // Log the execution summary to ensure fields are read
                let tx_opt = self.agent.event_tx.lock().clone();
                if let Some(tx) = tx_opt {
                    let _ = tx.try_send(crate::tui::AgentEvent::SystemUpdate(format!(
                        "🛠️ Executed {} tools with {} results",
                        tool_calls.len(),
                        results.len()
                    )));
                }
                // FIX: Transition back to Thinking so the model can verify results and continue its plan
                self.state = AgentStreamState::Thinking {
                    accumulated: String::new(),
                };
            }
            AgentStreamState::Done => {}
        }
        Ok(())
    }
}

pub struct AgentConfig {
    pub planner_model: Option<String>,
    pub executor_model: Option<String>,
    pub verifier_model: Option<String>,
    pub mlx_presets: std::collections::HashMap<String, crate::MlxPreset>,
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
    pub lmstudio_url: Option<String>,
    pub pa_memory_mb: Option<usize>,
    pub vram_time_sharing: bool,
    pub ollama_remote: Option<crate::OllamaRemoteConfig>,
    pub tool_engine: String,
}

impl Agent {
    pub fn get_tools(&self) -> &DashMap<String, Arc<dyn crate::tools::AgentTool>> {
        &self.tools
    }

    pub async fn consolidate_memories(&self) -> Result<String> {
        use crate::inference::SamplingConfig;
        use ollama_rs::models::ModelOptions;

        let records = {
            let store = self.memory_store.lock();
            store.list_all()?
        };

        if records.len() < 5 {
            return Ok("No need to dream yet. Long-term memory is clean and compact.".to_string());
        }

        let mut text_list = String::new();
        for (i, r) in records.iter().enumerate() {
            text_list.push_str(&format!(
                "{}. Topic: \"{}\" | Tags: \"{}\" | Fact: \"{}\"\n",
                i + 1,
                r.topic,
                r.tags.as_deref().unwrap_or("none"),
                r.content
            ));
        }

        let prompt = format!(
            "### TASK: LONG-TERM MEMORY CONSOLIDATION\n\
             You are the agent's subconscious mind, running during sleep/dreaming phase.\n\
             Consolidate the following facts. Merge overlapping facts, remove obsolete/redundant information, and compile a clean, unified list.\n\n\
             ### RULES:\n\
             1. Output MUST be a valid JSON array of objects, with NO surrounding text, markdown blocks, or preamble.\n\
             2. Output format:\n\
                [\n\
                  {{\n\
                    \"topic\": \"concise category or topic name\",\n\
                    \"tags\": [\"tag1\", \"tag2\"],\n\
                    \"fact\": \"synthesized fact statement\"\n\
                  }}\n\
                ]\n\n\
             ### CURRENT FACTS:\n{}",
            text_list
        );

        let response_text = match &*self.backend.read().await {
            Backend::Ollama(ollama, _) => {
                let options = ModelOptions::default()
                    .temperature(0.1)
                    .top_p(0.9)
                    .repeat_penalty(1.1)
                    .num_ctx(4096);
                let mut coordinator = ollama_rs::coordinator::Coordinator::new(
                    ollama.clone(),
                    self.get_model(),
                    vec![],
                )
                .options(options);

                let chat_fut =
                    coordinator.chat(vec![ollama_rs::generation::chat::ChatMessage::new(
                        ollama_rs::generation::chat::MessageRole::User,
                        prompt,
                    )]);
                match tokio::time::timeout(tokio::time::Duration::from_secs(45), chat_fut).await {
                    Ok(Ok(response)) => Some(response.message.content),
                    _ => None,
                }
            }
            backend => {
                let sampling = SamplingConfig {
                    temperature: 0.1,
                    top_p: 0.9,
                    repeat_penalty: 1.1,
                    context_size: 4096,
                };
                let stop = Arc::new(std::sync::atomic::AtomicBool::new(false));
                let event_tx = Arc::new(parking_lot::Mutex::new(None));
                let chat_fut = backend.stream_chat(crate::inference::ChatRequest {
                    model: self.get_model(),
                    history: vec![ollama_rs::generation::chat::ChatMessage::new(
                        ollama_rs::generation::chat::MessageRole::User,
                        prompt,
                    )],
                    sampling,
                    event_tx,
                    stop,
                    system_prompt: "".to_string(),
                    on_tool_call: None,
                    tool_registry: None,
                });
                match tokio::time::timeout(tokio::time::Duration::from_secs(45), chat_fut).await {
                    Ok(Ok(response)) => Some(response.content),
                    _ => None,
                }
            }
        };

        let response_text = response_text.ok_or_else(|| {
            miette::miette!("Memory consolidation model invocation timed out or failed")
        })?;

        let clean_json = response_text
            .trim()
            .strip_prefix("```json")
            .unwrap_or(&response_text)
            .strip_suffix("```")
            .unwrap_or(&response_text)
            .trim();

        #[derive(serde::Deserialize)]
        struct ConsFact {
            topic: String,
            tags: Vec<String>,
            fact: String,
        }

        let consolidated: Vec<ConsFact> = serde_json::from_str(clean_json).map_err(|e| {
            miette::miette!(
                "Failed to parse consolidated memory JSON: {}. Raw response: {}",
                e,
                response_text
            )
        })?;

        let mut repopulate_records = Vec::new();
        for item in consolidated {
            repopulate_records.push((item.topic, item.fact, Some(item.tags)));
        }

        let original_count = records.len();
        let new_count = repopulate_records.len();

        self.memory_store
            .lock()
            .clear_and_repopulate(repopulate_records)?;

        Ok(format!(
            "Memory consolidation complete (dreamed {} facts down to {} facts).",
            original_count, new_count
        ))
    }

    pub async fn execute_tool_by_name(
        &self,
        name: &str,
        arguments: &serde_json::Value,
    ) -> Result<String> {
        let tool_call = ollama_rs::generation::tools::ToolCall {
            function: ollama_rs::generation::tools::ToolCallFunction {
                name: name.to_string(),
                arguments: arguments.clone(),
            },
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

    #[allow(clippy::too_many_arguments)]
    pub async fn new(
        mode: AgentMode,
        mut model: String,
        mut quant: String,
        system_prompt: String,
        history_path: String,
        session_id: String,
        memory_store: Arc<Mutex<MemoryStore>>,
        sub_agent_model: String,
        embedding_model: String,
        event_tx: Arc<Mutex<Option<tokio::sync::mpsc::Sender<crate::tui::AgentEvent>>>>,
        config: AgentConfig,
    ) -> Result<Self> {
        // Resolve preset if model name matches a key in mlx_presets
        if mode == AgentMode::MLX
            && let Some(preset) = config.mlx_presets.get(&model)
        {
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

        // If we are not in Ollama mode, or if we are using a remote Ollama instance,
        // we override the tiered models to None so they fallback to the unified primary model.
        let (p_model, e_model, v_model) = if mode != AgentMode::Ollama
            || config
                .ollama_remote
                .as_ref()
                .is_some_and(|r| r.enabled.unwrap_or(false))
        {
            (None, None, None)
        } else {
            (
                config.planner_model.clone(),
                config.executor_model.clone(),
                config.verifier_model.clone(),
            )
        };

        let b_url = if mode == AgentMode::LMStudio {
            config.lmstudio_url.clone()
        } else {
            None
        };
        let backend_config = crate::inference::BackendConfig {
            mode,
            model: model.clone(),
            quant: quant.clone(),
            event_tx: event_tx.clone(),
            paged_attn: config.paged_attn,
            ctx_limit: config.ctx_execution as usize,
            base_url: b_url,
            pa_memory_mb: config.pa_memory_mb,
            ollama_remote: config.ollama_remote.clone(),
            embedding_model: Some(embedding_model.clone()),
        };
        let (backend, final_model) = Backend::new(backend_config).await?;
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
            Arc::new(crate::tools::editing::MultiEditTool),
            Arc::new(crate::tools::execution::RunCommandTool),
            Arc::new(crate::tools::execution::RunTestsTool),
            Arc::new(crate::tools::execution::BuildProjectTool),
            Arc::new(crate::tools::git::GitStatusTool),
            Arc::new(crate::tools::git::GitDiffTool),
            Arc::new(crate::tools::git::GitCommitTool),
            Arc::new(crate::tools::search::SemanticSearchTool),
            Arc::new(crate::tools::search::GrepSearchTool),
            Arc::new(crate::tools::memory::StoreMemoryTool::new(
                memory_store.clone(),
            )),
            Arc::new(crate::tools::memory::RecallMemoryTool::new(
                memory_store.clone(),
            )),
            Arc::new(crate::tools::agent_ops::AskUserTool),
            Arc::new(crate::tools::agent_ops::SpawnSubAgentTool),
            Arc::new(crate::tools::wasm_sandbox::WasmSafeCalculatorTool),
            Arc::new(crate::tools::threat_scanner::ThreatScannerTool),
            Arc::new(crate::tools::csv::QueryCsvTool),
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
            Arc::new(crate::tools::terminal::TerminalSpawnTool),
            Arc::new(crate::tools::terminal::TerminalInputTool),
            Arc::new(crate::tools::terminal::TerminalReadTool),
            Arc::new(crate::tools::terminal::TerminalCloseTool),
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
            Arc::new(crate::tools::search::IndexFileConceptuallyTool),
            Arc::new(crate::tools::memory::MemorySearchTool::new(
                memory_store.clone(),
            )),
            Arc::new(crate::tools::system::SystemdManagerTool),
            Arc::new(crate::tools::system::CurrentProcessTool),
            Arc::new(crate::tools::process::ListProcessesTool),
            Arc::new(crate::tools::privilege::RequestPrivilegesTool),
            Arc::new(crate::tools::rust::CargoAddTool),
            Arc::new(crate::tools::rust::CrateSearchTool),
            Arc::new(crate::tools::ast::AstOutlineTool),
            Arc::new(crate::tools::ast::AstQueryTool),
            Arc::new(crate::tools::ast::AstEditTool),
            Arc::new(crate::tools::visualization::GenerateGraphTool),
        ];

        let tools_map = Arc::new(DashMap::new());
        for t in &tools_vec {
            tools_map.insert(t.name().to_string(), t.clone());
        }

        // --- 🌪️ SKELEGENT TOOL REGISTRY INITIALIZATION ---
        let mut tool_registry_skg = skg_tool::ToolRegistry::new();

        // 1. Register Adapted Tools (Bridge) FIRST
        for t in &tools_vec {
            tool_registry_skg.register(Arc::new(crate::tools::skg_adapter::SkgToolAdapter {
                inner: t.clone(),
            }));
        }

        // 2. Register Native Skelegent Tools SECOND (overriding legacy adapted tools with their native SKG versions)
        tool_registry_skg.register(Arc::new(crate::tools::skg_tools::echo::EchoTool::new()));
        tool_registry_skg.register(Arc::new(crate::tools::skg_tools::demo::DemoTool::new()));

        // File Tools
        tool_registry_skg.register(Arc::new(crate::tools::skg_tools::file::ReadFileTool::new()));
        tool_registry_skg.register(Arc::new(crate::tools::skg_tools::file::WriteFileTool::new()));
        tool_registry_skg.register(Arc::new(crate::tools::skg_tools::file::ListDirTool::new()));
        tool_registry_skg.register(Arc::new(
            crate::tools::skg_tools::file::SearchFilesTool::new(),
        ));
        tool_registry_skg.register(Arc::new(crate::tools::skg_tools::file::DiffFilesTool::new()));
        tool_registry_skg.register(Arc::new(
            crate::tools::skg_tools::file::AppendFileTool::new(),
        ));
        tool_registry_skg.register(Arc::new(crate::tools::skg_tools::file::PatchFileTool::new()));
        tool_registry_skg.register(Arc::new(
            crate::tools::skg_tools::file::FindReplaceTool::new(),
        ));
        tool_registry_skg.register(Arc::new(
            crate::tools::skg_tools::file::CreateDirectoryTool::new(),
        ));
        tool_registry_skg.register(Arc::new(
            crate::tools::skg_tools::file::DeleteFileTool::new(),
        ));
        tool_registry_skg.register(Arc::new(
            crate::tools::skg_tools::file::RenameFileTool::new(),
        ));

        // Execution Tools
        tool_registry_skg.register(Arc::new(
            crate::tools::skg_tools::execution::RunCommandTool::new(),
        ));
        tool_registry_skg.register(Arc::new(
            crate::tools::skg_tools::execution::RunTestsTool::new(),
        ));
        tool_registry_skg.register(Arc::new(
            crate::tools::skg_tools::execution::BuildProjectTool::new(),
        ));

        // Search Tools
        tool_registry_skg.register(Arc::new(
            crate::tools::skg_tools::search::GrepSearchTool::new(),
        ));
        tool_registry_skg.register(Arc::new(
            crate::tools::skg_tools::search::SemanticSearchTool::new(),
        ));
        tool_registry_skg.register(Arc::new(
            crate::tools::skg_tools::search::IndexFileSemanticallyTool::new(),
        ));

        // Wave 2: Git Tools
        tool_registry_skg.register(Arc::new(crate::tools::skg_tools::git::GitStatusTool::new()));
        tool_registry_skg.register(Arc::new(crate::tools::skg_tools::git::GitDiffTool::new()));
        tool_registry_skg.register(Arc::new(crate::tools::skg_tools::git::GitCommitTool::new()));
        tool_registry_skg.register(Arc::new(crate::tools::skg_tools::git::GitActionTool::new()));

        // Wave 2: Web Tools
        tool_registry_skg.register(Arc::new(crate::tools::skg_tools::web::SearchWebTool::new()));
        tool_registry_skg.register(Arc::new(crate::tools::skg_tools::web::ReadUrlTool::new()));
        tool_registry_skg.register(Arc::new(
            crate::tools::skg_tools::web::RawHttpFetchTool::new(),
        ));
        tool_registry_skg.register(Arc::new(
            crate::tools::skg_tools::web::DownloadFileTool::new(),
        ));
        tool_registry_skg.register(Arc::new(
            crate::tools::skg_tools::web::GetStockPriceTool::new(),
        ));

        // Wave 2: Memory Tools
        tool_registry_skg.register(Arc::new(
            crate::tools::skg_tools::memory::StoreMemoryTool::new(),
        ));
        tool_registry_skg.register(Arc::new(
            crate::tools::skg_tools::memory::RecallMemoryTool::new(),
        ));
        tool_registry_skg.register(Arc::new(
            crate::tools::skg_tools::memory::MemorySearchTool::new(),
        ));

        // Wave 3: Editing, Agent Ops, Process, Terminal Tools
        tool_registry_skg.register(Arc::new(
            crate::tools::skg_tools::editing::EditFileWithDiffTool::new(),
        ));
        tool_registry_skg.register(Arc::new(
            crate::tools::skg_tools::editing::MultiEditTool::new(),
        ));

        tool_registry_skg.register(Arc::new(
            crate::tools::skg_tools::agent_ops::AskUserTool::new(),
        ));
        tool_registry_skg.register(Arc::new(
            crate::tools::skg_tools::agent_ops::SpawnSubAgentTool::new(),
        ));
        tool_registry_skg.register(Arc::new(
            crate::tools::skg_tools::agent_ops::UpdateTaskContextTool::new(),
        ));
        tool_registry_skg.register(Arc::new(
            crate::tools::skg_tools::agent_ops::QuerySchemaTool::new(),
        ));
        tool_registry_skg.register(Arc::new(crate::tools::skg_tools::agent_ops::NoOpTool::new()));

        tool_registry_skg.register(Arc::new(
            crate::tools::skg_tools::process::RunBackgroundTool::new(),
        ));
        tool_registry_skg.register(Arc::new(
            crate::tools::skg_tools::process::ReadProcessLogsTool::new(),
        ));
        tool_registry_skg.register(Arc::new(
            crate::tools::skg_tools::process::KillProcessTool::new(),
        ));
        tool_registry_skg.register(Arc::new(
            crate::tools::skg_tools::process::WatchDirectoryTool::new(),
        ));
        tool_registry_skg.register(Arc::new(
            crate::tools::skg_tools::process::ListProcessesTool::new(),
        ));

        tool_registry_skg.register(Arc::new(
            crate::tools::skg_tools::terminal::TerminalSpawnTool::new(),
        ));
        tool_registry_skg.register(Arc::new(
            crate::tools::skg_tools::terminal::TerminalInputTool::new(),
        ));
        tool_registry_skg.register(Arc::new(
            crate::tools::skg_tools::terminal::TerminalReadTool::new(),
        ));
        tool_registry_skg.register(Arc::new(
            crate::tools::skg_tools::terminal::TerminalCloseTool::new(),
        ));

        // Wave 4: Knowledge, Utilities, AST, Rust Tools
        tool_registry_skg.register(Arc::new(
            crate::tools::skg_tools::knowledge::ListSkillsTool::new(),
        ));
        tool_registry_skg.register(Arc::new(
            crate::tools::skg_tools::knowledge::RecallSkillTool::new(),
        ));
        tool_registry_skg.register(Arc::new(
            crate::tools::skg_tools::knowledge::DistillKnowledgeTool::new(),
        ));
        tool_registry_skg.register(Arc::new(
            crate::tools::skg_tools::knowledge::RecallBrainTool::new(),
        ));

        tool_registry_skg.register(Arc::new(
            crate::tools::skg_tools::utilities::ClipboardTool::new(),
        ));
        tool_registry_skg.register(Arc::new(
            crate::tools::skg_tools::utilities::NotifyTool::new(),
        ));
        tool_registry_skg.register(Arc::new(
            crate::tools::skg_tools::utilities::EnvVarTool::new(),
        ));
        tool_registry_skg.register(Arc::new(
            crate::tools::skg_tools::utilities::ChmodTool::new(),
        ));
        tool_registry_skg.register(Arc::new(
            crate::tools::skg_tools::utilities::CalculatorTool::new(),
        ));

        tool_registry_skg.register(Arc::new(crate::tools::skg_tools::ast::AstOutlineTool::new()));
        tool_registry_skg.register(Arc::new(crate::tools::skg_tools::ast::AstEditTool::new()));
        tool_registry_skg.register(Arc::new(crate::tools::skg_tools::ast::AstQueryTool::new()));

        tool_registry_skg.register(Arc::new(crate::tools::skg_tools::rust::CargoAddTool::new()));
        tool_registry_skg.register(Arc::new(
            crate::tools::skg_tools::rust::CargoSearchTool::new(),
        ));

        // Wave 6: Remaining system/development/database/visualization/safety tools
        tool_registry_skg.register(Arc::new(
            crate::tools::skg_tools::wasm_sandbox::WasmSafeCalcTool::new(),
        ));
        tool_registry_skg.register(Arc::new(
            crate::tools::skg_tools::threat_scanner::ThreatScanTool::new(),
        ));
        tool_registry_skg.register(Arc::new(crate::tools::skg_tools::csv::QueryCsvTool::new()));
        tool_registry_skg.register(Arc::new(
            crate::tools::skg_tools::telemetry::SystemTelemetryTool::new(),
        ));
        tool_registry_skg.register(Arc::new(
            crate::tools::skg_tools::network_manager::ListNetworkSocketsTool::new(),
        ));
        tool_registry_skg.register(Arc::new(
            crate::tools::skg_tools::service_manager::ListSystemServicesTool::new(),
        ));
        tool_registry_skg.register(Arc::new(
            crate::tools::skg_tools::developer::InitializeRustProjectTool::new(),
        ));
        tool_registry_skg.register(Arc::new(
            crate::tools::skg_tools::database::SqliteQueryTool::new(),
        ));
        tool_registry_skg.register(Arc::new(
            crate::tools::skg_tools::network::NetworkCheckTool::new(),
        ));
        tool_registry_skg.register(Arc::new(crate::tools::skg_tools::atlas::TreeTool::new()));
        tool_registry_skg.register(Arc::new(
            crate::tools::skg_tools::atlas::ProjectAtlasTool::new(),
        ));
        tool_registry_skg.register(Arc::new(
            crate::tools::skg_tools::file::ExtractAndWriteTool::new(),
        ));
        tool_registry_skg.register(Arc::new(
            crate::tools::skg_tools::system::SystemdManagerTool::new(),
        ));
        tool_registry_skg.register(Arc::new(
            crate::tools::skg_tools::system::CurrentProcessInfoTool::new(),
        ));
        tool_registry_skg.register(Arc::new(
            crate::tools::skg_tools::privilege::RequestPrivilegesTool::new(),
        ));
        tool_registry_skg.register(Arc::new(
            crate::tools::skg_tools::visualization::GenerateGraphTool::new(),
        ));

        let tool_registry_skg = Arc::new(tool_registry_skg);

        let history_path_obj = Path::new(&history_path);
        let brain_path = history_path_obj
            .parent()
            .unwrap_or(Path::new("."))
            .join("brain_vectors.json");

        // --- 🎯 TOOL RAG INDEX (Dynamic Tool Selection) ---
        // Instead of a static core_tool_names whitelist, we embed all tool descriptions
        // into a vector index using nomic-embed-text. On each user turn, only the
        // top-K most relevant tools are injected into the schema.
        // The full toolbox remains discoverable via `query_schema`.
        let skg_tools_vec: Vec<Arc<dyn skg_tool::ToolDyn>> =
            tool_registry_skg.iter().cloned().collect();
        let tool_rag_index = crate::tool_rag::ToolVectorIndex::build(
            &skg_tools_vec,
            &*backend.read().await,
            event_tx.clone(),
        )
        .await
        .unwrap_or_else(|e| {
            eprintln!(
                "⚠️ Tool RAG index build failed ({}), falling back to full toolset",
                e
            );
            // Fallback: we'll handle this in resolve() by returning all tools
            // For now, build with an empty state — resolve will still work via always-on
            crate::tool_rag::ToolVectorIndex::build_fallback(&skg_tools_vec)
        });
        let tool_rag_index = Arc::new(tokio::sync::RwLock::new(tool_rag_index));

        // Initialize tool_registry as empty — it will be populated dynamically per-turn
        // by the Tool RAG system in planner_turn(). We keep the field for backward
        // compatibility with stream_chat's tool schema parameter.
        let tool_registry: Vec<ollama_rs::generation::tools::ToolInfo> = Vec::new();

        let passphrase = memory_store.lock().passphrase().to_string();
        let vector_brain = Arc::new(Mutex::new(
            crate::vector_brain::VectorBrain::load_from_disk(&brain_path, Some(&passphrase))
                .unwrap_or_else(|_| crate::vector_brain::VectorBrain::new()),
        ));

        let start_time = std::time::Instant::now();
        let api_time_ms = Arc::new(std::sync::atomic::AtomicU64::new(0));
        let tool_time_ms = Arc::new(std::sync::atomic::AtomicU64::new(0));
        let total_tokens = Arc::new(std::sync::atomic::AtomicU64::new(0));

        let state_store: Arc<dyn layer0::StateStore + Send + Sync> = if history_path == ":memory:" {
            Arc::new(skg_state_memory::MemoryStore::new())
        } else {
            let path = std::path::Path::new(&history_path);
            let parent = path
                .parent()
                .unwrap_or_else(|| std::path::Path::new("."))
                .to_path_buf();
            Arc::new(crate::state_store::FileStateStore::new(parent))
        };

        Ok(Agent {
            mode,
            phase: Arc::new(Mutex::new(AgentPhase::Planning)),
            backend,
            model: Arc::new(Mutex::new(final_model)),
            history: Arc::new(Mutex::new(vec![])),
            tools: tools_map,
            tool_registry,
            tool_rag_index,
            recent_tool_calls: Arc::new(DashMap::new()),
            history_path,
            state_store,
            brain_path,
            planning_mode: Arc::new(Mutex::new(true)),
            task_context: Arc::new(Mutex::new("Not started yet.".to_string())),
            vector_brain,
            sub_agent_model,
            embedding_model,
            syntax_set: SyntaxSet::load_defaults_newlines(),
            theme_set: Arc::new(ThemeSet::load_defaults()),
            rules: crate::rules::RuleEngine::new(),
            recent_failures: Arc::new(DashMap::new()),
            telemetry: Arc::new(Mutex::new(String::new())),
            is_root: Arc::new(AtomicBool::new({
                #[cfg(unix)]
                {
                    nix::unistd::getuid().is_root()
                }
                #[cfg(not(unix))]
                {
                    false
                }
            })),
            concurrency_semaphore: Arc::new(tokio::sync::Semaphore::new(5)),
            event_tx,
            tool_rx: Arc::new(tokio::sync::Mutex::new(None)),
            sentinel: crate::sentinel::SentinelManager::new(),
            editor_context: Arc::new(Mutex::new(None)),
            safe_mode: Arc::new(AtomicBool::new(false)),
            hardcore_mode: Arc::new(AtomicBool::new(false)),

            tool_stats: Arc::new(DashMap::new()),
            tool_repetition_stack: Arc::new(Mutex::new(Vec::new())),
            planner_model: p_model,
            executor_model: e_model,
            verifier_model: v_model,
            mlx_presets: {
                let dm = DashMap::new();
                for (k, v) in config.mlx_presets {
                    dm.insert(k, v);
                }
                Arc::new(dm)
            },
            temp_planning: config.temp_planning,
            temp_execution: config.temp_execution,
            top_p_planning: config.top_p_planning,
            top_p_execution: config.top_p_execution,
            repeat_penalty_planning: config.repeat_penalty_planning,
            repeat_penalty_execution: config.repeat_penalty_execution,
            ctx_planning: config.ctx_planning,
            ctx_execution: config.ctx_execution,
            mlx_temp_planning: config.mlx_temp_planning,
            mlx_temp_execution: config.mlx_temp_execution,
            mlx_top_p_planning: config.mlx_top_p_planning,
            mlx_top_p_execution: config.mlx_top_p_execution,
            mlx_repeat_penalty_planning: config.mlx_repeat_penalty_planning,
            mlx_repeat_penalty_execution: config.mlx_repeat_penalty_execution,
            paged_attn: config.paged_attn,
            pa_memory_mb: config.pa_memory_mb,
            llm_extract_retries: 3,
            ollama_remote: config.ollama_remote.clone(),
            planning_enabled: config.planning_enabled,
            overwatch: crate::overwatch::OverwatchEngine::new(),
            checkpoint_mgr: crate::checkpoint::new_shared(50),
            memory_store,
            mcp_clients: Arc::new(DashMap::new()),
            tool_registry_skg,
            temp_override: Arc::new(Mutex::new(None)),
            ctx_override: Arc::new(Mutex::new(None)),
            role_override: Arc::new(Mutex::new(None)),
            vram_time_sharing: config.vram_time_sharing,
            session_id,
            start_time,
            api_time_ms,
            tool_time_ms,
            total_tokens,
            tool_engine: config.tool_engine.clone(),
            kv_cache_hit_history: Arc::new(Mutex::new(Vec::new())),
            system_prompt: {
                let mut final_system_prompt = system_prompt.clone();
                if mode == AgentMode::MLX {
                    final_system_prompt.push_str("\n\n⚠️ AGENT OPERATIONAL RULES:
1. YOU ARE THE ACTOR: You possess the tools (`write_file`, `run_command`).
2. REASONING BOUNDARY: You MUST wrap your internal planning, logic, and thoughts inside `<think>` and `</think>` tags.
3. ACTIONS ARE EXTERNAL: All JSON tool calls and conversational responses MUST be placed OUTSIDE and AFTER the `</think>` tag.
4. TOOL CALL FORMAT: Deliver tool calls as ```json blocks containing valid JSON with a `tool` key and `arguments` key. Multiple tool calls may be placed in a single block as a comma-separated list.
5. CODE DISCIPLINE: You are FORBIDDEN from dumping raw source code in markdown blocks (```python, ```rust, ```sh, etc.). If you have code to write, deliver it via the `write_file` tool call — never as a bare code block.
6. EDITOR AWARENESS: You have direct visibility into the user's active editor. ALWAYS prioritize the file path and content provided in the `[EDITOR]` block. Never guess a path if one is provided in context.
7. DELIVERY GUARANTEE: If you have code to provide, YOU MUST output a `write_file` tool call. If you don't, the user gets nothing.
8. NEVER ask the user to write code. You are the engineer; they are the supervisor.");
                }
                final_system_prompt
            },
        })
    }

    pub async fn get_ollama(&self) -> Result<Ollama> {
        match &*self.backend.read().await {
            Backend::Ollama(o, _) => Ok(o.clone()),
            #[cfg(target_os = "macos")]
            Backend::MLX { .. } => Err(miette!("Active backend is MLX, not Ollama")),
            Backend::Bridge(_) => Err(miette!("Active backend is AI Bridge, not Ollama")),
            Backend::Kalosm { .. } => Err(miette!("Active backend is Kalosm, not Ollama")),
        }
    }

    pub async fn initialize_atlas(&self, force: bool) -> Result<()> {
        let backend = self.backend.read().await;
        let tx = self.event_tx.lock().clone();
        crate::tools::atlas::run_semantic_indexing(
            &backend,
            self.vector_brain.clone(),
            &self.brain_path,
            force,
            tx,
        )
        .await
    }

    /// Returns the configured context window size.
    /// Driven entirely by config (ctx_execution in config.toml, default 32768).
    async fn calculate_optimal_ctx(&self) -> u64 {
        let backend = self.backend.read().await;
        if backend.mode() == crate::inference::AgentMode::Gemini {
            // Gemini 1.5 Pro has up to 2 million token context, so we bypass the local config limits.
            return 2_000_000;
        }
        self.ctx_execution
    }

    pub async fn check_connection(&self) -> Result<()> {
        let backend = self.backend.read().await;
        match &*backend {
            crate::inference::Backend::Ollama(_, _) => {
                if let Ok(ollama) = self.get_ollama().await {
                    let models_result = tokio::time::timeout(
                        std::time::Duration::from_secs(10),
                        ollama.list_local_models(),
                    )
                    .await;

                    let models = match models_result {
                        Ok(Ok(m)) => m,
                        Ok(Err(e)) => {
                            return Err(miette!(
                                "Ollama API Error: {:?}. Is the service running and responsive?",
                                e
                            ));
                        }
                        Err(_) => {
                            return Err(miette!(
                                "Ollama connection TIMEOUT (10s). The API is not responding. Please check your Ollama app."
                            ));
                        }
                    };

                    let model_names: std::collections::HashSet<String> =
                        models.into_iter().map(|m| m.name).collect();

                    let required = vec![
                        AgentPhase::Planning.default_model(),
                        AgentPhase::Execution.default_model(),
                        AgentPhase::Testing.default_model(),
                    ];

                    for req in required {
                        if !model_names.contains(&req) {
                            // If we're using a specific unified model override, we might not need the tiered ones
                            let current_model = self.model.lock().clone();
                            if model_names.contains(&current_model) {
                                break;
                            }
                            return Err(miette!(
                                "Required model '{}' not found in Ollama. Please run: ollama pull {}",
                                req,
                                req
                            ));
                        }
                    }
                }
            }
            crate::inference::Backend::Kalosm { .. } => {}
            crate::inference::Backend::Bridge(bridge) => {
                use ollama_rs::generation::chat::{ChatMessage, MessageRole};

                let tx_opt = self.event_tx.lock().clone();
                let is_gemini = bridge.models[0].starts_with("gemini");
                if !is_gemini {
                    if let Some(tx) = &tx_opt {
                        let _ = tx.try_send(crate::tui::AgentEvent::SubagentStatus(Some(format!(
                            "🔌 Connecting to: {}...",
                            bridge.models[0]
                        ))));
                    }
                    if let Err(e) = bridge
                        .chat(
                            vec![ChatMessage::new(MessageRole::User, "ping".to_string())],
                            None,
                        )
                        .await
                    {
                        return Err(miette!(
                            "Bridge connection failed: {}. Ensure your local server (LM Studio/OpenAI) is running and the model '{}' is loaded.",
                            e,
                            self.model.lock()
                        ));
                    }
                    if let Some(tx) = &tx_opt {
                        let _ = tx.try_send(crate::tui::AgentEvent::SubagentStatus(Some(format!(
                            "🟢 Connected: {}",
                            bridge.models[0]
                        ))));
                    }
                } else {
                    let fallbacks = vec![
                        "gemini-3.5-flash",
                        "gemini-3.1-pro-preview",
                        "gemini-2.5-flash",
                        "gemini-1.5-pro-latest",
                    ];

                    let mut models_to_try = vec![bridge.models[0].clone()];
                    for f in &fallbacks {
                        if !models_to_try.contains(&f.to_string()) {
                            models_to_try.push(f.to_string());
                        }
                    }

                    let mut success = false;
                    let mut last_err = String::new();

                    for model_name in models_to_try {
                        if let Some(tx) = &tx_opt {
                            let _ = tx.try_send(crate::tui::AgentEvent::SubagentStatus(Some(
                                format!("🔌 Connecting to: {}...", model_name),
                            )));
                        }
                        let test_bridge = if model_name == bridge.models[0] {
                            bridge.clone()
                        } else {
                            let api_key = std::env::var("GEMINI_API_KEY").unwrap_or_default();
                            crate::ai_bridge::TempestAiBridge::new(
                                crate::ai_bridge::ModelProvider::Gemini { api_key },
                                vec![model_name.clone()],
                            )?
                        };

                        match test_bridge
                            .chat(
                                vec![ChatMessage::new(MessageRole::User, "ping".to_string())],
                                None,
                            )
                            .await
                        {
                            Ok(_) => {
                                if model_name != bridge.models[0] {
                                    let update_msg =
                                        format!("Automatically falling back to {}...", model_name);
                                    if let Some(tx) = &tx_opt {
                                        let _ = tx.try_send(crate::tui::AgentEvent::SystemUpdate(
                                            format!("⚠️ {}", update_msg),
                                        ));
                                    } else {
                                        println!("{} {}", "⚠️".yellow(), update_msg);
                                    }
                                    drop(backend);
                                    *self.model.lock() = model_name.clone();
                                    let mut b_write = self.backend.write().await;
                                    *b_write = crate::inference::Backend::Bridge(test_bridge);
                                }
                                if let Some(tx) = &tx_opt {
                                    let _ = tx.try_send(crate::tui::AgentEvent::SubagentStatus(
                                        Some(format!("🟢 Connected: {}", model_name)),
                                    ));
                                }
                                success = true;
                                break;
                            }
                            Err(e) => {
                                last_err = e.to_string();
                                if last_err.contains("503")
                                    || last_err.contains("UNAVAILABLE")
                                    || last_err.contains("404")
                                {
                                    continue; // Try next fallback
                                } else {
                                    return Err(miette!(
                                        "Bridge connection failed on model {}: {}",
                                        model_name,
                                        e
                                    ));
                                }
                            }
                        }
                    }

                    if !success {
                        return Err(miette!(
                            "All Gemini fallback models failed or are unavailable. Last error: {}",
                            last_err
                        ));
                    }
                }
            }
            #[cfg(target_os = "macos")]
            crate::inference::Backend::MLX { .. } => {
                // MLX is local and verified during Backend::new
            }
        }
        Ok(())
    }

    /// Injects a high-priority state message into the context to ensure the model knows its current boundaries.
    fn inject_state_context(&self) {
        let is_planning = *self.planning_mode.lock();

        let mode_str = if is_planning {
            "PLANNING PHASE (Architectural research & strategy)"
        } else {
            "EXECUTION PHASE (Implementation & active engineering)"
        };

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
            format!(
                "\n### COMPETENCY WARNINGS ###\n{}\n",
                competency_warnings.join("\n")
            )
        };

        let whisperer_str = if self.mode == AgentMode::MLX {
            "\n\n⚠️ MISSION ARCHITECTURE: You are the ACTOR. The User is your COLLABORATOR.
DELIVERY PROTOCOL: All project code and technical changes must be delivered via tools (`write_file`, `patch_file`). Raw markdown code blocks are for demonstration only.
STATUS REPORTING: You are expected to provide concise, technical updates on your progress in the chat. Explain what you've done and what you intend to verify next.
VERIFICATION CYCLE: No task is complete until verified. Use `read_file` or `run_command` to confirm your modifications before reporting success."
        } else {
            ""
        };

        let hardcore_str = if self.hardcore_mode.load(std::sync::atomic::Ordering::SeqCst) {
            "\n\n[HARDCORE MODE ACTIVE]: STRICT EXECUTION ONLY. DO NOT use conversational filler, apologies, or pleasantries. Output ONLY raw technical data, code diffs, and tool calls. Any conversational deviation will result in immediate termination."
        } else {
            ""
        };

        let state_msg = format!(
            "[STATE] {} | DIRECTIVE: AUTONOMY ENGAGED. Focus on tool-driven execution and technical reporting.{} | ADVISORY: Verify all path assumptions and results before proceeding.{}{}",
            mode_str.to_uppercase(),
            competency_str,
            whisperer_str,
            hardcore_str
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
        #[cfg(target_os = "macos")]
        let is_not_mlx = !matches!(
            &*self.backend.read().await,
            crate::inference::Backend::MLX { .. }
        );
        #[cfg(not(target_os = "macos"))]
        let is_not_mlx = true;

        if is_not_mlx {
            let is_ollama = matches!(
                self.backend.read().await.mode(),
                crate::inference::AgentMode::Ollama
            );
            if is_ollama && !self.vram_time_sharing {
                *self.model.lock() = new_phase.default_model();
            }
        }

        // Notify TUI
        let tx_opt = self.event_tx.lock().clone();
        if let Some(tx) = tx_opt {
            let _ = tx.try_send(crate::tui::AgentEvent::SystemUpdate(format!(
                "🔄 Switched from {} -> {}",
                old_desc,
                new_phase.description()
            )));
        }

        // Save history to ensure current state is persisted
        let _ = self.save_history();

        Ok(())
    }

    pub fn load_history(&self) -> Result<()> {
        let scope = layer0::effect::Scope::Session(layer0::id::SessionId::new(&self.session_id));

        let history_value = tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current()
                .block_on(async { self.state_store.read(&scope, "history").await })
        })
        .map_err(|e| miette::miette!("Failed to read state: {:?}", e))?;

        if let Some(val) = history_value
            && let Ok(mut history) = serde_json::from_value::<Vec<ChatMessage>>(val)
        {
            // PRUNING: Ensure the last message isn't a dangling tool call.
            while let Some(last) = history.last() {
                if last.role == MessageRole::Assistant && !last.tool_calls.is_empty() {
                    history.pop();
                } else {
                    break;
                }
            }

            // 🔄 Historical demangling for standard history
            for msg in &mut history {
                for call in &mut msg.tool_calls {
                    let mut name = call.function.name.clone();
                    if name.starts_with("__idx_")
                        && let Some(end) = name[6..].find("__")
                    {
                        let absolute_end = 6 + end;
                        name = name[absolute_end + 2..].to_string();
                        call.function.name = name;
                    }
                }
            }

            let mut h_lock = self.history.lock();
            for msg in history {
                if msg.role != MessageRole::System {
                    h_lock.push(msg);
                }
            }
        }

        // Load raw history if it exists to preserve thought signatures and extra fields
        let raw_value = tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current()
                .block_on(async { self.state_store.read(&scope, "raw_history").await })
        })
        .map_err(|e| miette::miette!("Failed to read raw state: {:?}", e))?;

        if let Some(val) = raw_value
            && let Ok(mut raw_history_vec) = serde_json::from_value::<Vec<serde_json::Value>>(val)
        {
            // 🔄 Historical demangling for raw history
            for msg_val in &mut raw_history_vec {
                if msg_val.get("role").and_then(|r| r.as_str()) == Some("assistant") {
                    if let Some(tool_calls) = msg_val
                        .get_mut("tool_calls")
                        .and_then(|tc| tc.as_array_mut())
                    {
                        for tc in tool_calls {
                            if let Some(func) =
                                tc.get_mut("function").and_then(|f| f.as_object_mut())
                                && let Some(name_val) = func.get_mut("name")
                                && let Some(name_str) = name_val.as_str()
                                && name_str.starts_with("__idx_")
                                && let Some(end) = name_str[6..].find("__")
                            {
                                let absolute_end = 6 + end;
                                let actual_name = name_str[absolute_end + 2..].to_string();
                                *name_val = serde_json::Value::String(actual_name);
                            }
                        }
                    }
                } else if msg_val.get("role").and_then(|r| r.as_str()) == Some("tool")
                    && let Some(name_val) = msg_val.get_mut("name")
                    && let Some(name_str) = name_val.as_str()
                    && name_str.starts_with("__idx_")
                    && let Some(end) = name_str[6..].find("__")
                {
                    let absolute_end = 6 + end;
                    let actual_name = name_str[absolute_end + 2..].to_string();
                    *name_val = serde_json::Value::String(actual_name);
                }
            }

            if let Some(raw_hist_arc) = self.backend.try_read().ok().and_then(|b| b.raw_history()) {
                let mut raw_hist = raw_hist_arc.lock();
                *raw_hist = raw_history_vec;
            }
        }

        Ok(())
    }

    pub fn set_safe_mode(&self, enabled: bool) {
        self.safe_mode
            .store(enabled, std::sync::atomic::Ordering::SeqCst);
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
                let _ = tx.try_send(crate::tui::AgentEvent::SystemUpdate(
                    "✨ Session Resumed: Environment grounded.".to_string(),
                ));
            }
        }
        Ok(())
    }

    pub fn save_history(&self) -> Result<()> {
        let history = self.history.lock();
        let scope = layer0::effect::Scope::Session(layer0::id::SessionId::new(&self.session_id));
        let val = serde_json::to_value(&*history).into_diagnostic()?;

        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current()
                .block_on(async { self.state_store.write(&scope, "history", val).await })
        })
        .map_err(|e| miette::miette!("Failed to write state: {:?}", e))?;

        // Save raw history to preserve thought signatures and extra fields
        if let Some(raw_hist_arc) = self.backend.try_read().ok().and_then(|b| b.raw_history()) {
            let raw_hist = raw_hist_arc.lock();
            let raw_val = serde_json::to_value(&*raw_hist).into_diagnostic()?;
            tokio::task::block_in_place(|| {
                tokio::runtime::Handle::current().block_on(async {
                    self.state_store.write(&scope, "raw_history", raw_val).await
                })
            })
            .map_err(|e| miette::miette!("Failed to write raw state: {:?}", e))?;
        }

        Ok(())
    }

    /// Initializes and connects to external MCP servers based on the provided configuration.
    pub async fn initialize_mcp(&self, configs: Vec<crate::McpServerConfig>) -> Result<()> {
        for config in configs {
            let tx_opt = self.event_tx.lock().clone();
            if let Some(tx) = tx_opt.as_ref() {
                let _ = tx.try_send(crate::tui::AgentEvent::SystemUpdate(format!(
                    "🔌 Connecting to MCP Server: {}...",
                    config.name
                )));
            }

            match crate::mcp::McpClient::new(
                config.name.clone(),
                &config.command,
                &config.args,
                &config.env.clone().unwrap_or_default(),
            )
            .await
            {
                Ok(mut client) => {
                    if let Err(e) = client.initialize().await {
                        if let Some(tx) = tx_opt.as_ref() {
                            let _ = tx.try_send(crate::tui::AgentEvent::SystemUpdate(format!(
                                "❌ MCP Init Failed ({}): {}",
                                config.name, e
                            )));
                        }
                        continue;
                    }

                    match client.list_tools().await {
                        Ok(tools) => {
                            let client_arc = Arc::new(tokio::sync::Mutex::new(client));
                            self.mcp_clients
                                .insert(config.name.clone(), client_arc.clone());

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
                                let leaked_name: &'static str =
                                    Box::leak(namespaced_name.into_boxed_str());
                                let leaked_desc: &'static str =
                                    Box::leak(tool.description.into_boxed_str());

                                let proxy = crate::mcp::McpToolProxy {
                                    client: client_arc.clone(),
                                    name: leaked_name,
                                    description: leaked_desc,
                                    input_schema: tool.input_schema.clone(),
                                };

                                self.tools.insert(
                                    leaked_name.to_string(),
                                    Arc::new(proxy) as Arc<dyn crate::tools::AgentTool>,
                                );

                                if let Some(tx) = tx_opt.as_ref() {
                                    let _ = tx.try_send(crate::tui::AgentEvent::SystemUpdate(
                                        format!("✅ Registered MCP Tool: {}", leaked_name),
                                    ));
                                }
                            }
                        }
                        Err(e) => {
                            if let Some(tx) = tx_opt.as_ref() {
                                let _ = tx.try_send(crate::tui::AgentEvent::SystemUpdate(format!(
                                    "❌ MCP Tools Failed ({}): {}",
                                    config.name, e
                                )));
                            }
                        }
                    }
                }
                Err(e) => {
                    if let Some(tx) = tx_opt.as_ref() {
                        let _ = tx.try_send(crate::tui::AgentEvent::SystemUpdate(format!(
                            "❌ MCP Connection Failed ({}): {}",
                            config.name, e
                        )));
                    }
                }
            }
        }
        Ok(())
    }

    /// Helper to push a structured tool result back to the model history and TUI.
    pub async fn send_tool_feedback(
        &self,
        tool_name: &str,
        result: &str,
        is_success: bool,
    ) -> Result<()> {
        // Update TUI HUD
        let tx_opt = self.event_tx.lock().clone();
        if let Some(tx) = tx_opt {
            let _ = tx.try_send(crate::tui::AgentEvent::SystemUpdate(format!(
                "⚡ [System]: {} completed.",
                tool_name
            )));
        }

        let feedback = if is_success {
            format!(
                "=== SYSTEM OBSERVATION ===\nTool: {}\nResult: {}\n\n(Verify this data against your plan and proceed to the next step.)",
                tool_name, result
            )
        } else {
            format!(
                "=== SYSTEM ERROR ===\nTool: {}\nError: {}\n\nPlease analyze this error carefully and adjust your strategy. Do NOT repeat the same mistake.",
                tool_name, result
            )
        };

        // CRITICAL: Tool results must be Tool/Observation role, not User.
        self.history
            .lock()
            .push(ChatMessage::new(MessageRole::Tool, feedback));
        self.save_history()?;

        // --- 📊 RESTORE TOOL RESULT TRACKER ---
        self.report_tool_stats().await;

        Ok(())
    }

    /// Reports current tool performance metrics to the TUI.
    pub async fn report_tool_stats(&self) {
        if self.tool_stats.is_empty() {
            return;
        }

        let mut stats_lines = Vec::new();
        stats_lines.push("📊 [TOOL PERFORMANCE]:".to_string());
        for item in self.tool_stats.iter() {
            let (name, (s, f)) = (item.key(), item.value());
            let total = s + f;
            if total == 0 {
                continue;
            }
            let rate = (*s as f64 / total as f64) * 100.0;
            let emoji = if rate >= 90.0 {
                "🟢"
            } else if rate >= 50.0 {
                "🟡"
            } else {
                "🔴"
            };
            stats_lines.push(format!(
                "  {} {}: {:.1}% ({}s / {}f)",
                emoji, name, rate, s, f
            ));
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
        let preset = self
            .mlx_presets
            .get(&preset_name)
            .ok_or_else(|| miette!("Preset {} not found", preset_name))?
            .clone();

        let (model_val, quant_val) = if let Some(path) = preset.path {
            (path, "None".to_string())
        } else if let Some(repo) = preset.repo {
            (repo, preset.quant.unwrap_or_else(|| "Q4_K_M".to_string()))
        } else {
            return Err(miette!(
                "Preset {} is malformed: missing path or repo",
                preset_name
            ));
        };

        let tx_opt = self.event_tx.lock().clone();
        if let Some(tx) = tx_opt {
            let _ = tx
                .send(crate::tui::AgentEvent::SystemUpdate(format!(
                    "🔄 Hot-swapping MLX to: {} ({})",
                    preset_name, quant_val
                )))
                .await;
        } else {
            println!(
                "{} Hot-swapping MLX to: {} ({})",
                "🔄".yellow(),
                preset_name,
                quant_val
            );
        }

        let config = crate::inference::BackendConfig {
            mode: crate::inference::AgentMode::MLX,
            model: model_val.clone(),
            quant: quant_val.clone(),
            event_tx: self.event_tx.clone(),
            paged_attn: self.paged_attn,
            ctx_limit: self.ctx_execution as usize,
            base_url: None,
            pa_memory_mb: self.pa_memory_mb,
            ollama_remote: self.ollama_remote.clone(),
            embedding_model: Some(self.embedding_model.clone()),
        };

        let (new_backend, new_model_name) = crate::inference::Backend::new(config).await?;

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
            let _ = tx
                .send(crate::tui::AgentEvent::SystemUpdate(format!(
                    "✅ MLX Switched to {}",
                    preset_name
                )))
                .await;
        } else {
            println!("{} MLX Switched to {}", "✅".green(), preset_name);
        }

        Ok(())
    }

    pub async fn run(
        &self,
        initial_user_prompt: String,
        stop: std::sync::Arc<std::sync::atomic::AtomicBool>,
    ) -> Result<()> {
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
                self.safe_mode
                    .store(!current, std::sync::atomic::Ordering::SeqCst);
                let tx_opt = self.event_tx.lock().clone();
                if let Some(tx) = tx_opt {
                    let _ = tx.try_send(crate::tui::AgentEvent::SystemUpdate(format!(
                        "🛡️ Safe Mode: {}",
                        if !current { "ON" } else { "OFF" }
                    )));
                }
                return Ok(());
            }

            if prompt_trimmed == "/compact" {
                let tx_opt = self.event_tx.lock().clone();
                if let Some(ref tx) = tx_opt {
                    let _ = tx.try_send(crate::tui::AgentEvent::AgentStateChange(
                        "Compacting".to_string(),
                    ));
                    let _ = tx.try_send(crate::tui::AgentEvent::Thinking(Some(
                        "agent is compacting history. Please wait one moment.".to_string(),
                    )));
                }

                let history_to_compact = self.history.lock().clone();
                let before_count = crate::context_manager::estimate_tokens(&history_to_compact);

                let backend_guard = self.backend.read().await;
                let ctx_limit = self.ctx_execution;
                let result = crate::context_manager::compact_history(
                    &backend_guard,
                    &self.sub_agent_model,
                    history_to_compact,
                    ctx_limit,
                    &self.vector_brain,
                    &self.brain_path,
                )
                .await;

                if let Some(ref tx) = tx_opt {
                    let _ = tx.try_send(crate::tui::AgentEvent::Thinking(None));
                    let _ =
                        tx.try_send(crate::tui::AgentEvent::AgentStateChange("Done".to_string()));
                    match &result {
                        Ok(new_history) => {
                            let after_count = crate::context_manager::estimate_tokens(new_history);
                            let _ = tx.try_send(crate::tui::AgentEvent::SystemUpdate(format!(
                                "🌪️ [CONTEXT COMPACTION]: Successfully condensed history ({} -> {} tokens)",
                                before_count, after_count
                            )));
                        }
                        Err(e) => {
                            let _ = tx.try_send(crate::tui::AgentEvent::SystemUpdate(format!(
                                "⚠️ Context compaction failed: {}",
                                e
                            )));
                        }
                    }
                }
                if let Ok(new_history) = result {
                    *self.history.lock() = new_history;
                    let _ = self.save_history();
                }
                return Ok(());
            }

            if prompt_trimmed == "/recall" {
                let tx_opt = self.event_tx.lock().clone();
                if let Some(tx) = tx_opt {
                    let memory_res = self.memory_store.lock().recall_latest();
                    match memory_res {
                        Ok(Some((topic, content))) => {
                            let _ = tx.try_send(crate::tui::AgentEvent::SystemUpdate(format!(
                                "🧠 [RECALL MEMORY] Topic: {}\n\n{}",
                                topic, content
                            )));
                        }
                        Ok(None) => {
                            let _ = tx.try_send(crate::tui::AgentEvent::SystemUpdate(
                                "🧠 [RECALL MEMORY]: No memories found in database.".to_string(),
                            ));
                        }
                        Err(e) => {
                            let _ = tx.try_send(crate::tui::AgentEvent::SystemUpdate(format!(
                                "⚠️ Memory recall failed: {}",
                                e
                            )));
                        }
                    }
                }
                return Ok(());
            }

            if prompt_trimmed == "/toggle_hardcore" {
                let current = self.hardcore_mode.load(std::sync::atomic::Ordering::SeqCst);
                self.hardcore_mode
                    .store(!current, std::sync::atomic::Ordering::SeqCst);
                let tx_opt = self.event_tx.lock().clone();
                if let Some(tx) = tx_opt {
                    let _ = tx.try_send(crate::tui::AgentEvent::SystemUpdate(format!(
                        "⚠️ [SENTINEL]: Hardcore Mode is now {}",
                        if !current {
                            "ON (Fast-blocking warnings active)"
                        } else {
                            "OFF"
                        }
                    )));
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

        if self.planning_enabled {
            let mut p_lock = self.phase.lock();
            if *p_lock == AgentPhase::Testing {
                *p_lock = AgentPhase::Planning;
            }
        }

        self.initialize_session(&initial_user_prompt).await?;

        // Notify Web/HUD of initial task
        let task_preview = initial_user_prompt.chars().take(200).collect::<String>();
        {
            let mut ctx_lock = self.task_context.lock();
            *ctx_lock = task_preview.clone();
        }
        let tx_opt = self.event_tx.lock().clone();
        if let Some(tx) = tx_opt {
            let _ = tx
                .send(crate::tui::AgentEvent::TaskUpdate(task_preview))
                .await;
        }

        // 🛡️ PRE-TURN CONNECTIVITY SENTINEL
        if let Ok(ollama) = self.get_ollama().await {
            let ping = tokio::time::timeout(
                std::time::Duration::from_secs(2),
                ollama.list_local_models(),
            )
            .await;

            if ping.is_err() || ping.unwrap().is_err() {
                let tx_opt = self.event_tx.lock().clone();
                if let Some(tx) = tx_opt {
                    let _ = tx
                        .send(crate::tui::AgentEvent::SystemUpdate(
                            "⚠️ CRITICAL: Cannot reach Ollama API. Handshake failed.".to_string(),
                        ))
                        .await;
                }
                return Err(miette!(
                    "Ollama is not responding. Please ensure the Ollama service is running."
                ));
            }
        }

        // Immediate Feedback: Signal to the UI that we are starting
        let tx_opt = self.event_tx.lock().clone();
        if let Some(tx) = tx_opt {
            let _ = tx.try_send(crate::tui::AgentEvent::Thinking(Some(
                "Analyzing request...".to_string(),
            )));
            let _ = tx.try_send(crate::tui::AgentEvent::SubagentStatus(Some(
                "⚡ Tempest Engine: Grounding environment and history...".to_string(),
            )));
        }

        let mut stream = AgentStream::new(self, stop.clone());
        let max_iterations = 30;

        while stream.iteration < max_iterations {
            if stop.load(std::sync::atomic::Ordering::Relaxed) {
                let tx_opt = self.event_tx.lock().clone();
                if let Some(tx) = tx_opt {
                    let _ = tx
                        .send(crate::tui::AgentEvent::SystemUpdate(
                            "🛑 INTERRUPTED: Turn cancelled by user.".to_string(),
                        ))
                        .await;
                    let _ = tx.send(crate::tui::AgentEvent::Thinking(None)).await;
                }
                break;
            }

            match &stream.state {
                AgentStreamState::Done => break,
                _ => {
                    // 📊 TELEMETRY PRE-EMPTION: Send ActiveTools BEFORE transition if we are about to execute
                    if let AgentStreamState::ExecutingTools { tool_calls, .. } = &stream.state {
                        let tx_opt = self.event_tx.lock().clone();
                        if let Some(tx) = tx_opt {
                            let tool_names: Vec<String> =
                                tool_calls.iter().map(|t| t.function.name.clone()).collect();
                            let _ = tx
                                .send(crate::tui::AgentEvent::ActiveTools(tool_names))
                                .await;
                        }
                    }

                    // Send state CHANGE BEFORE transition so UI shows Thinking while LLM is running
                    let tx_opt = self.event_tx.lock().clone();
                    if let Some(ref tx) = tx_opt {
                        let _ = tx
                            .send(crate::tui::AgentEvent::AgentStateChange(
                                stream.state.name().to_string(),
                            ))
                            .await;
                    }

                    stream.transition().await?;
                }
            }
        }

        stream.decomposer.finalize();
        let telemetry_str = stream.decomposer.format_telemetry();

        let tx_opt = self.event_tx.lock().clone();
        if let Some(tx) = tx_opt {
            let _ = tx.try_send(crate::tui::AgentEvent::SystemUpdate(format!(
                "📊 [SKELEGENT TURN-KIT] {}",
                telemetry_str
            )));
            let _ = tx.try_send(crate::tui::AgentEvent::PhaseDurations {
                planning_ms: stream.decomposer.planning_duration_ms,
                executing_ms: stream.decomposer.executing_duration_ms,
                verifying_ms: stream.decomposer.verifying_duration_ms,
            });
        }
        println!("📊 [SKELEGENT TURN-KIT] {}", telemetry_str);

        let _ = self.save_history();
        self.overwatch.mark_success();

        // Force the final state to Done when the run loop finishes (even if it hits max_iterations)
        let tx_opt = self.event_tx.lock().clone();
        if let Some(tx) = tx_opt {
            let _ = tx.try_send(crate::tui::AgentEvent::AgentStateChange("Done".to_string()));
        }

        Ok(())
    }

    pub fn build_system_prompt(&self, phase: AgentPhase, model_name: &str) -> String {
        let os_name = match std::env::consts::OS {
            "macos" => "macOS",
            "linux" => "Linux",
            "windows" => "Windows",
            _ => std::env::consts::OS,
        };

        // 1. IDENTITY + INVIOLABLE RULES (Base)
        let mut full_system_prompt = crate::prompts::SYSTEM_PROMPT_BASE.replace("{OS}", os_name);

        // 2. PHASE-SPECIFIC INSTRUCTIONS
        let phase_instruction = match phase {
            AgentPhase::Planning => crate::prompts::SYSTEM_PROMPT_PLANNING,
            AgentPhase::Execution => crate::prompts::SYSTEM_PROMPT_EXECUTION,
            AgentPhase::Testing => crate::prompts::SYSTEM_PROMPT_TESTING,
        };
        full_system_prompt.push_str("\n\n");
        full_system_prompt.push_str(phase_instruction);

        // 3. TOOL SCHEMA PLACEHOLDER
        full_system_prompt.push_str("\n\n[TOOL_SCHEMA_PLACEHOLDER]");

        // 4. ACTIVE PROJECT RULES (Contextual)
        let mut active_files = std::collections::HashSet::new();
        {
            let history = self.history.lock();
            let re_file = regex::Regex::new(r"(?i)[a-zA-Z0-9_\-\./]+\.[a-zA-Z]{2,6}").ok();
            for msg in history.iter() {
                for call in &msg.tool_calls {
                    for key in ["path", "file_path", "TargetFile", "TargetDirectory"] {
                        if let Some(val) = call.function.arguments.get(key)
                            && let Some(path_str) = val.as_str()
                        {
                            active_files.insert(path_str.to_string());
                        }
                    }
                }
                if let Some(ref re) = re_file {
                    for mat in re.find_iter(&msg.content) {
                        active_files.insert(mat.as_str().to_string());
                    }
                }
            }
        }
        let files_list: Vec<String> = active_files.into_iter().collect();
        let active_rules = self.rules.get_active_rules(&files_list);

        if !active_rules.is_empty() {
            full_system_prompt.push_str("\n\n[ACTIVE PROJECT RULES]\n");
            for rule in active_rules {
                full_system_prompt
                    .push_str(&format!("### Rule: {}\n{}\n\n", rule.name, rule.content));
            }
        }

        if let Some(ref role) = *self.role_override.lock() {
            let role_instruction = match role.as_str() {
                "senior-editor" => {
                    "\n\n🎓 ROLE: Senior Editor\n\
                     Your core objective is code quality, clear structure, clean architecture, and strict professional design guidelines. \
                     Focus heavily on readability, modularity, comments, and eliminating code duplication."
                }
                "security-auditor" => {
                    "\n\n🛡️ ROLE: Security Auditor\n\
                     Your core objective is finding and fixing vulnerabilities. Perform rigorous input validation, check buffer boundaries, \
                     hunt for race conditions, verify permissions, and eliminate security flaws."
                }
                "code-poet" => {
                    "\n\n✍️ ROLE: Code Poet\n\
                     Your core objective is writing beautifully structured, highly expressive, and artistic code. Use elegant comments, clear naming conventions, \
                     and make the code feel crafted like poetry."
                }
                "refactor-ninja" => {
                    "\n\n🥷 ROLE: Refactor Ninja\n\
                     Your core objective is refactoring and optimization. Eliminate duplication, decrease cognitive load, simplify logic, \
                     and improve performance while making minimal, surgical changes."
                }
                _ => "",
            };
            if !role_instruction.is_empty() {
                full_system_prompt.push_str(role_instruction);
            }
        }

        // 5. OUTPUT FORMAT + CRITICAL RESPONSE RULES + EXAMPLES (Tail)
        let model_lower = model_name.to_lowercase();

        let mut is_reasoning = model_lower.contains("r1")
            || model_lower.contains("deepseek")
            || model_lower.contains("deep-seek")
            || model_lower.contains("qwq")
            || model_lower.contains("centurion")
            || model_lower.contains("battle")
            || model_lower.contains("reasoning");

        if let Some(preset) = self.mlx_presets.get(&model_lower) {
            let path_lower = preset.path.clone().unwrap_or_default().to_lowercase();
            let repo_lower = preset.repo.clone().unwrap_or_default().to_lowercase();
            if path_lower.contains("r1")
                || path_lower.contains("deepseek")
                || path_lower.contains("deep-seek")
                || path_lower.contains("battle")
                || path_lower.contains("centurion")
                || path_lower.contains("qwq")
                || path_lower.contains("reasoning")
                || repo_lower.contains("r1")
                || repo_lower.contains("deepseek")
                || repo_lower.contains("deep-seek")
                || repo_lower.contains("battle")
                || repo_lower.contains("centurion")
                || repo_lower.contains("qwq")
                || repo_lower.contains("reasoning")
            {
                is_reasoning = true;
            }
        }

        if is_reasoning {
            full_system_prompt.push_str("\n\n[ACTOR PROTOCOL]:\n- You are the ACTOR. Your response MUST start with a `<think>` block.\n- [THOUGHT BOUNDARY]: Your `<think>` block is for internal logic ONLY. Do NOT place tool calls inside thoughts.\n- [TOOL CALL FORMAT]: After `</think>`, explain your action briefly, then output the JSON tool call directly. NEVER use markdown code blocks (```json) for tool calls.\n- [FORMAT]:\n<think>\n[Reasoning]\n</think>\nExplanation text...\n{\"tool\": \"name\", \"arguments\": {}}\n\n- [COLLABORATION]: After performing actions and verifying results, provide a concise summary to the user and ask for the next step in the roadmap.");
            full_system_prompt
                .push_str(&crate::prompts::SYSTEM_PROMPT_TAIL.replace("{OS}", os_name));
        } else {
            full_system_prompt.push_str(
                &crate::prompts::SYSTEM_PROMPT_NON_REASONING_TAIL.replace("{OS}", os_name),
            );
        }

        full_system_prompt
    }

    async fn initialize_session(&self, initial_user_prompt: &str) -> Result<()> {
        if self.event_tx.lock().is_none() {
            println!("{}", "=".repeat(60).blue());
            println!("{} {}", "🚀".green(), "Tempest AI Agent Initialized".bold());
            println!("{} {}", "Model:".blue(), *self.model.lock());
            println!("{}", "=".repeat(60).blue());
        }

        let current_phase = self.phase.lock().clone();
        let current_model = self.model.lock().clone();

        let tx_opt = self.event_tx.lock().clone();
        if let Some(tx) = tx_opt {
            let status = if let Some(planner) = &self.planner_model {
                format!("🤖 Engine: {} | Planner: {}", current_model, planner)
            } else {
                format!("🤖 Engine: {}", current_model)
            };
            let _ = tx.try_send(crate::tui::AgentEvent::SubagentStatus(Some(status)));
        }

        let full_system_prompt = self.build_system_prompt(current_phase, &current_model);

        {
            let mut h_lock = self.history.lock();

            if let Some(pos) = h_lock.iter().position(|m| m.role == MessageRole::System) {
                h_lock[pos] = ChatMessage::new(MessageRole::System, full_system_prompt);
            } else {
                h_lock.insert(0, ChatMessage::new(MessageRole::System, full_system_prompt));
            }

            while let Some(last) = h_lock.last() {
                if last.role == MessageRole::User
                    && (last.content.starts_with("ERROR: Tool")
                        || last.content.starts_with("SYSTEM NOTIFICATION:")
                        || last.content.starts_with("BLOCKED:")
                        || last.content.contains("ACTION REQUIRED"))
                {
                    h_lock.pop();
                } else {
                    break;
                }
            }

            let user_msg = ChatMessage::new(MessageRole::User, initial_user_prompt.to_string());
            h_lock.push(user_msg);
            self.recent_failures.clear();
        }

        let _ = self.save_history();
        Ok(())
    }

    async fn run_sentinel_stage(&self, ctx_limit: u64) -> Result<()> {
        let rep_stack = self.tool_repetition_stack.lock().clone();
        let history = self.history.lock().clone();
        let sentinel = self.sentinel.clone();
        let is_hardcore = self.hardcore_mode.load(std::sync::atomic::Ordering::SeqCst);
        let action_opt = tokio::task::spawn_blocking(move || {
            sentinel.analyze_state(&history, ctx_limit, &rep_stack, is_hardcore)
        })
        .await
        .into_diagnostic()?;

        if let Some(action) = action_opt {
            let tx_opt = self.event_tx.lock().clone();
            if let Some(tx) = tx_opt {
                let _ = tx.try_send(crate::tui::AgentEvent::SentinelUpdate {
                    active: action.active_sentinels.clone(),
                    log: action.message.clone(),
                });

                if !action.message.is_empty() {
                    let _ =
                        tx.try_send(crate::tui::AgentEvent::SystemUpdate(action.message.clone()));
                    // Inject into history so the model sees the reprimand immediately
                    self.history
                        .lock()
                        .push(ollama_rs::generation::chat::ChatMessage::new(
                            ollama_rs::generation::chat::MessageRole::System,
                            format!("SENTINEL INTERVENTION: {}", action.message),
                        ));
                }
            }

            if action.hardcore_kill {
                return Err(miette::miette!(
                    "HARDCORE KILL TRIGGERED: {}",
                    action.message
                ));
            }

            if action.needs_compaction {
                let tx_opt = self.event_tx.lock().clone();
                if let Some(ref tx) = tx_opt {
                    let _ = tx.try_send(crate::tui::AgentEvent::AgentStateChange(
                        "Compacting".to_string(),
                    ));
                    let _ = tx.try_send(crate::tui::AgentEvent::Thinking(Some(
                        "agent is compacting history. Please wait one moment.".to_string(),
                    )));
                }

                let history_to_compact = self.history.lock().clone();
                let before_count = crate::context_manager::estimate_tokens(&history_to_compact);

                let backend_guard = self.backend.read().await;
                let new_history = crate::context_manager::compact_history(
                    &backend_guard,
                    &self.sub_agent_model,
                    history_to_compact,
                    ctx_limit,
                    &self.vector_brain,
                    &self.brain_path,
                )
                .await?;

                let after_count = crate::context_manager::estimate_tokens(&new_history);
                *self.history.lock() = new_history;
                let _ = self.save_history();

                if let Some(ref tx) = tx_opt {
                    let _ = tx.try_send(crate::tui::AgentEvent::Thinking(None));
                    let _ = tx.try_send(crate::tui::AgentEvent::AgentStateChange(
                        "Thinking".to_string(),
                    ));
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

    async fn process_tool_feedback_stage(
        &self,
        results: Vec<(String, String, bool)>,
    ) -> Result<()> {
        let mut was_modifying = false;
        for (tool_name, _, is_success) in &results {
            if *is_success
                && let Some(tool) = self.tools.get(tool_name).map(|r| r.value().clone())
                && tool.is_modifying()
            {
                was_modifying = true;
                break;
            }
        }

        if was_modifying {
            self.switch_phase(AgentPhase::Testing).await?;
        }
        let mut detected_loop_key = None;
        let mut feedback_to_apply = Vec::new();

        for (tool_name, result, is_success) in results {
            let (formatted_res, _hud_msg, is_success) = if is_success {
                self.recent_failures.remove(&tool_name);
                self.recent_failures.remove("GENERIC_FILE_NOT_FOUND");

                if result.starts_with("BLOCKED:") {
                    (
                        format!(
                            "SYSTEM NOTIFICATION: TOOL BLOCKED for {}:\n{}\n\nACTION REQUIRED: You MUST propose a plan and ask for approval via 'ask_user' before this tool can be used.",
                            tool_name, result
                        ),
                        format!("🚫 BLOCKED: '{}'", tool_name),
                        true,
                    )
                } else {
                    (result, format!("✅ SUCCESS: '{}'", tool_name), true)
                }
            } else {
                let fail_key = if result.contains("os error 2") || result.contains("No such file") {
                    "GENERIC_FILE_NOT_FOUND".to_string()
                } else {
                    format!(
                        "{}:{}",
                        tool_name,
                        result.chars().take(50).collect::<String>()
                    )
                };

                let count = *self
                    .recent_failures
                    .entry(fail_key.clone())
                    .and_modify(|c| *c += 1)
                    .or_insert(1);
                if count >= 3 {
                    detected_loop_key = Some(fail_key);
                }

                (
                    format!("ERROR: Tool '{}' failed.\nREASON:\n{}", tool_name, result),
                    format!("❌ ERROR: '{}'", tool_name),
                    false,
                )
            };

            feedback_to_apply.push((tool_name, formatted_res, is_success));
        }

        for (tool_name, formatted_res, is_success) in feedback_to_apply {
            let _ = self
                .send_tool_feedback(&tool_name, &formatted_res, is_success)
                .await;
        }

        if let Some(key) = detected_loop_key {
            let mut h_lock = self.history.lock();
            let directive = format!(
                "\n\n⚠️ [SENTINEL REORIENTATION DIRECTIVE]: You are looping on '{}'. I am forcing a state-synchronization check of the CURRENT WORKING DIRECTORY.",
                key
            );
            h_lock.push(ChatMessage::new(MessageRole::System, directive));

            if let Ok(entries) = std::fs::read_dir(".") {
                let files: Vec<_> = entries
                    .flatten()
                    .map(|e| e.file_name().to_string_lossy().to_string())
                    .collect();
                let resync = format!(
                    "SYSTEM NOTIFICATION: This is an automated forced-sync of your CURRENT WORKING DIRECTORY (it is not a message from the user).\n\nCONTENTS:\n- {}",
                    files.join("\n- ")
                );
                h_lock.push(ChatMessage::new(MessageRole::System, resync));
            }
            self.recent_failures.clear();
        }

        let _ = self.save_history();
        Ok(())
    }

    async fn planner_turn(
        &self,
        stop: std::sync::Arc<std::sync::atomic::AtomicBool>,
    ) -> Result<PlannerOutput> {
        let mut history_snapshot = self.history.lock().clone();

        let mode = self.backend.read().await.mode();
        let phase = self.phase.lock().clone();
        let is_planning = matches!(phase, AgentPhase::Planning);
        let is_mlx = mode == crate::inference::AgentMode::MLX;

        // Resolve model name. If VRAM sharing is active with Ollama, or if we are not using Ollama backend, pin to the primary model
        let model_name = if (self.vram_time_sharing && mode == crate::inference::AgentMode::Ollama)
            || mode != crate::inference::AgentMode::Ollama
        {
            self.model.lock().clone()
        } else {
            match phase {
                AgentPhase::Planning => self
                    .planner_model
                    .clone()
                    .unwrap_or_else(|| self.model.lock().clone()),
                AgentPhase::Execution => self
                    .executor_model
                    .clone()
                    .unwrap_or_else(|| self.model.lock().clone()),
                AgentPhase::Testing => self
                    .verifier_model
                    .clone()
                    .unwrap_or_else(|| self.model.lock().clone()),
            }
        };

        // --- CONTEXT SWAPPING: Hot-swap system prompt dynamically for the active phase ---
        let phase_system_prompt = self.build_system_prompt(phase.clone(), &model_name);
        if let Some(pos) = history_snapshot
            .iter()
            .position(|m| m.role == MessageRole::System)
        {
            history_snapshot[pos].content = phase_system_prompt;
        } else {
            history_snapshot.insert(
                0,
                ChatMessage::new(MessageRole::System, phase_system_prompt),
            );
        }

        // --- PHASE 3: TOKEN BUDGET AWARENESS (SKG Context Pipeline) ---
        let ctx_limit = self.calculate_optimal_ctx().await;
        let used = crate::context_manager::estimate_tokens(&history_snapshot);

        let tx_opt = self.event_tx.lock().clone();
        if let Some(tx) = tx_opt {
            let _ = tx.try_send(crate::tui::AgentEvent::ContextStatus {
                used,
                total: ctx_limit,
                kv_cache_hit_pct: None,
            });
        }

        let mut skg_ctx = skg_context_engine::Context::new();
        skg_ctx.messages = crate::context_manager::to_layer0_messages(&history_snapshot);
        skg_ctx
            .extensions
            .insert(crate::context_manager::ContextLimit(ctx_limit as usize));

        let runway_rule = skg_context_engine::Rule::when(
            "Context Runway Monitor",
            100,
            |_| true,
            crate::context_manager::RunwayReportOp,
        );
        skg_ctx.add_rule(runway_rule);

        struct NoOp;
        #[async_trait::async_trait]
        impl skg_context_engine::ContextOp for NoOp {
            type Output = ();
            async fn execute(
                &self,
                _ctx: &mut skg_context_engine::Context,
            ) -> std::result::Result<(), skg_context_engine::EngineError> {
                Ok(())
            }
        }

        if skg_ctx.run(NoOp).await.is_ok() {
            history_snapshot = crate::context_manager::to_chat_messages(&skg_ctx.messages);
        }

        let executed_mid_stream = Arc::new(parking_lot::Mutex::new(Vec::new()));
        let (tool_tx, mut tool_rx) =
            tokio::sync::mpsc::unbounded_channel::<ollama_rs::generation::tools::ToolCall>();
        let executed_mid_stream_for_task = executed_mid_stream.clone();
        let agent_for_task = self.clone();

        let tool_task = tokio::spawn(async move {
            let mut join_set = tokio::task::JoinSet::new();
            while let Some(call) = tool_rx.recv().await {
                let agent_clone = agent_for_task.clone();
                let call_clone = call.clone();
                join_set.spawn(async move {
                    let res = agent_clone.process_single_tool_call(call, false).await;
                    (call_clone, res)
                });
            }
            while let Some(res) = join_set.join_next().await {
                if let Ok(r) = res {
                    executed_mid_stream_for_task.lock().push(r);
                }
            }
        });

        if let Some(tx) = self.event_tx.lock().clone() {
            let phase_desc = phase.description();
            let _ = tx.try_send(crate::tui::AgentEvent::SubagentStatus(Some(format!(
                "🔄 {} [Using {}]...",
                phase_desc, model_name
            ))));
        }

        let sampling = if is_planning {
            crate::inference::SamplingConfig {
                temperature: (*self.temp_override.lock()).unwrap_or(if is_mlx {
                    self.mlx_temp_planning.unwrap_or(0.6)
                } else {
                    self.temp_planning
                }),
                top_p: if is_mlx {
                    self.mlx_top_p_planning.unwrap_or(0.95)
                } else {
                    self.top_p_planning
                },
                repeat_penalty: if is_mlx {
                    self.mlx_repeat_penalty_planning.unwrap_or(1.1)
                } else {
                    self.repeat_penalty_planning
                },
                context_size: (*self.ctx_override.lock()).unwrap_or(self.ctx_planning),
            }
        } else {
            crate::inference::SamplingConfig {
                temperature: (*self.temp_override.lock()).unwrap_or(if is_mlx {
                    self.mlx_temp_execution.unwrap_or(0.2)
                } else {
                    self.temp_execution
                }),
                top_p: if is_mlx {
                    self.mlx_top_p_execution.unwrap_or(0.9)
                } else {
                    self.top_p_execution
                },
                repeat_penalty: if is_mlx {
                    self.mlx_repeat_penalty_execution.unwrap_or(1.05)
                } else {
                    self.repeat_penalty_execution
                },
                context_size: (*self.ctx_override.lock()).unwrap_or(self.ctx_execution),
            }
        };

        let mut final_history = history_snapshot;

        // --- 🎯 TOOL RAG: DYNAMIC TOOL SCHEMA RESOLUTION ---
        // Resolve the most relevant tools for this prompt using vector similarity.
        // This replaces the old static core_tool_names whitelist.
        let user_prompt_for_rag = final_history
            .iter()
            .rev()
            .find(|m| m.role == MessageRole::User)
            .map(|m| m.content.clone())
            .unwrap_or_default();

        let (dynamic_tool_registry, tool_rag_log) = match self
            .tool_rag_index
            .read()
            .await
            .resolve(&user_prompt_for_rag, &*self.backend.read().await, None)
            .await
        {
            Ok((tools, log)) => (tools, log),
            Err(e) => {
                // Fallback: use always-on core tools
                if let Some(tx) = self.event_tx.lock().clone() {
                    let _ = tx.try_send(crate::tui::AgentEvent::SystemUpdate(format!(
                        "⚠️ [TOOL RAG]: Resolution failed ({}), using core tools fallback",
                        e
                    )));
                }
                (
                    self.tool_rag_index.read().await.always_on_tools(),
                    Vec::new(),
                )
            }
        };

        // Emit telemetry about which tools were selected
        if let Some(tx) = self.event_tx.lock().clone() {
            let rag_tools_str: Vec<String> = tool_rag_log
                .iter()
                .map(|(name, sim)| format!("{} ({:.2})", name, sim))
                .collect();
            let always_on_str = crate::tool_rag::ALWAYS_ON_TOOLS.join(", ");
            let _ = tx.try_send(crate::tui::AgentEvent::SystemUpdate(format!(
                "🎯 [TOOL RAG]: {} tools selected | RAG: [{}] | Always-on: [{}]",
                dynamic_tool_registry.len(),
                rag_tools_str.join(", "),
                always_on_str,
            )));
        }

        // Inject the dynamic tool schema into the system prompt placeholder
        if let Ok(schema_json) = serde_json::to_string(&dynamic_tool_registry)
            && let Some(sys_msg) = final_history
                .iter_mut()
                .find(|m| m.role == MessageRole::System)
        {
            sys_msg.content = sys_msg.content.replace(
                "[TOOL_SCHEMA_PLACEHOLDER]",
                &format!("\n\n[TOOL SCHEMA]\n{}", schema_json),
            );
        }

        // --- 🧠 DYNAMIC CONTEXT INJECTION ---
        // We prepend the editor context to the LAST user message in this turn's memory ONLY.
        // This keeps the long-term history (and history.json) clean of redundant code blocks.
        if let Some(ctx) = self.editor_context.lock().as_ref() {
            let visible_code = ctx
                .get("visible_code")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            if !visible_code.is_empty()
                && let Some(last_user_msg) = final_history
                    .iter_mut()
                    .rev()
                    .find(|m| m.role == MessageRole::User)
            {
                let file_name = ctx
                    .get("file_name")
                    .and_then(|v| v.as_str())
                    .unwrap_or("unknown");
                let file_path = ctx.get("file_path").and_then(|v| v.as_str()).unwrap_or("");
                let language = ctx
                    .get("language_id")
                    .and_then(|v| v.as_str())
                    .unwrap_or("text");
                let has_selection = ctx
                    .get("has_selection")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);
                let cursor_line = ctx.get("cursor_line").and_then(|v| v.as_u64()).unwrap_or(0);
                let lines_count = visible_code.lines().count();

                let context_prefix = crate::templates::render_editor_context(
                    file_name,
                    file_path,
                    language,
                    cursor_line,
                    has_selection,
                    lines_count,
                    visible_code,
                )
                .expect("Failed to render editor context template");

                if !last_user_msg.content.contains("[EDITOR]") {
                    last_user_msg.content =
                        format!("{} [USER] {}", context_prefix, last_user_msg.content);
                }
            }
        }

        // --- 🧠 DYNAMIC SEMANTIC MEMORY INJECTION ---
        // Retrieve relevant context from past compacted turns using VectorBrain.
        let last_user_content = final_history
            .iter()
            .rev()
            .find(|m| m.role == MessageRole::User)
            .map(|m| m.content.clone());

        if let Some(query_text) = last_user_content
            && let Ok(query_vec) = self
                .backend
                .read()
                .await
                .generate_embeddings(&query_text)
                .await
        {
            let hits = self.vector_brain.lock().search(&query_vec, 3);
            let mut retrieved_memories = Vec::new();
            for (entry, sim) in hits {
                if entry.source == "context_compaction" && sim >= 0.70 {
                    retrieved_memories.push(entry.text);
                }
            }
            if !retrieved_memories.is_empty() {
                let injection = crate::templates::render_historical_context(&retrieved_memories)
                    .expect("Failed to render historical context template");
                if let Some(pos) = final_history
                    .iter()
                    .rposition(|m| m.role == MessageRole::User)
                {
                    final_history.insert(pos, ChatMessage::new(MessageRole::System, injection));
                }
            }
        }

        let tx_opt = self.event_tx.lock().clone();
        if let Some(tx) = tx_opt {
            let mode = self.backend.read().await.mode();
            let backend_name = match mode {
                crate::inference::AgentMode::MLX => "MLX Engine",
                crate::inference::AgentMode::Ollama => "Ollama",
                crate::inference::AgentMode::Bridge => "AI Bridge",
                crate::inference::AgentMode::LMStudio => "LM Studio",
                crate::inference::AgentMode::Kalosm => "Kalosm Native",
                crate::inference::AgentMode::Gemini => "Google Gemini",
            };
            let _ = tx.try_send(crate::tui::AgentEvent::SubagentStatus(Some(format!(
                "📡 Dispatching request to {} ({}) [Waiting for GPU]...",
                backend_name, model_name
            ))));
        }

        let input_tokens = crate::context_manager::estimate_tokens(&final_history);
        let start_api = std::time::Instant::now();
        let output = self
            .backend
            .read()
            .await
            .stream_chat(crate::inference::ChatRequest {
                model: model_name,
                history: final_history,
                sampling,
                event_tx: self.event_tx.clone(),
                stop,
                system_prompt: self.system_prompt.clone(),
                on_tool_call: Some(tool_tx),
                tool_registry: Some(dynamic_tool_registry),
            })
            .await?;
        self.api_time_ms.fetch_add(
            start_api.elapsed().as_millis() as u64,
            std::sync::atomic::Ordering::Relaxed,
        );
        let output_tokens = crate::context_manager::count_tokens(&output.content)
            + crate::context_manager::count_tokens(&output.reasoning);
        self.total_tokens.fetch_add(
            (input_tokens + output_tokens) as u64,
            std::sync::atomic::Ordering::Relaxed,
        );

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
            let _ = tx
                .send(crate::tui::AgentEvent::StreamToken("".to_string()))
                .await;
        }
        if self.event_tx.lock().is_none() {
            println!();
        }

        if !full_content.trim().is_empty()
            || !native_tool_calls.is_empty()
            || !reasoning_content.is_empty()
        {
            let mut stored_content = full_content.clone();
            if !native_tool_calls.is_empty() && stored_content.is_empty() {
                stored_content =
                    "<think>I am executing a structural tool call.</think>".to_string();
                // Actively notify the UI if we were silent during the stream
                if let Some(tx) = self.event_tx.lock().clone() {
                    let _ = tx.try_send(crate::tui::AgentEvent::SystemUpdate(
                        "⚡ [System]: Executing tool call...".to_string(),
                    ));
                }
            }

            let mut msg = ChatMessage::new(MessageRole::Assistant, stored_content);
            msg.tool_calls = native_tool_calls.clone();
            msg.thinking = Some(reasoning_content.clone());

            self.history.lock().push(msg);
        }

        // 🧠 KALOSM MEMORY MANAGEMENT: Auto-prune history to prevent unbounded growth
        let mode = self.backend.read().await.mode();
        if mode == crate::inference::AgentMode::Kalosm {
            self.auto_prune_history_for_kalosm().await;
        }

        if let Some(hit) = output.kv_cache_hit_pct {
            self.kv_cache_hit_history.lock().push(hit);
        }

        Ok(PlannerOutput {
            content: full_content,
            reasoning: reasoning_content,
            native_tool_calls,
            executed_mid_stream: Arc::try_unwrap(executed_mid_stream)
                .map(|m| m.into_inner())
                .unwrap_or_default(),
            kv_cache_hit_pct: output.kv_cache_hit_pct,
        })
    }

    /// 🧠 AUTO-PRUNE HISTORY FOR KALOSM
    /// Keeps history bounded to prevent swap creep.
    /// - Always keeps system messages
    /// - Keeps only the last N message pairs (configurable)
    /// - Runs automatically after each turn for Kalosm users
    async fn auto_prune_history_for_kalosm(&self) {
        let max_history_depth = 15; // More aggressive: keep only last 15 messages

        let mut h_lock = self.history.lock();

        // Separate system messages from conversation messages
        let (system_msgs, conversation_msgs): (Vec<_>, Vec<_>) = h_lock
            .iter()
            .enumerate()
            .partition(|(_, m)| m.role == MessageRole::System);

        // If conversation exceeds max, keep only the most recent
        if conversation_msgs.len() > max_history_depth {
            let kept_indices: Vec<usize> = system_msgs.iter().map(|(i, _)| *i).collect::<Vec<_>>();
            let recent_indices: Vec<usize> = conversation_msgs
                .iter()
                .skip(conversation_msgs.len() - max_history_depth)
                .map(|(i, _)| *i)
                .collect();

            let all_kept: Vec<usize> = {
                let mut v = kept_indices;
                v.extend(recent_indices);
                v.sort();
                v
            };

            // Rebuild history with only kept indices
            let new_history: Vec<ChatMessage> = h_lock
                .iter()
                .enumerate()
                .filter(|(i, _)| all_kept.contains(i))
                .map(|(_, m)| m.clone())
                .collect();

            let before = h_lock.len();
            *h_lock = new_history;
            let after = h_lock.len();

            if before != after {
                drop(h_lock); // Release lock before sending event
                let tx_opt = self.event_tx.lock().clone();
                if let Some(tx) = tx_opt {
                    let _ = tx.try_send(crate::tui::AgentEvent::SystemUpdate(format!(
                        "🧠 [KALOSM AUTO-PRUNE]: History reduced from {} to {} messages to manage swap usage",
                        before, after
                    )));
                }
                let _ = self.save_history();
            }
        }
    }

    fn repair_tool_name(&self, name: &str) -> String {
        let name_lower = name.to_lowercase();
        match name_lower.as_str() {
            "ask" | "ask_user_input" | "prompt_user" | "user_input" => "ask_user".to_string(),
            "stock_price" | "get_stock" | "check_stock" | "stock" => "get_stock_price".to_string(),
            "search" | "google_search" | "web_search" => "search_web".to_string(),
            "read" | "fetch_url" | "web_read" => "read_url".to_string(),
            "recall" | "recall_knowledge" | "memory" | "brain" => "recall_brain".to_string(),
            "distill" | "save_knowledge" | "save_brain" => "distill_knowledge".to_string(),
            "shell" | "exec" | "command" => "run_command".to_string(),
            "notify" | "alert" | "status" => "no_op".to_string(),
            _ => {
                if self.tools.contains_key(name) || self.tool_registry_skg.get(name).is_some() {
                    name.to_string()
                } else if self.tools.contains_key(&name_lower)
                    || self.tool_registry_skg.get(&name_lower).is_some()
                {
                    name_lower
                } else {
                    name.to_string()
                }
            }
        }
    }

    pub async fn executor_dispatch(
        &self,
        tool_calls: Vec<ollama_rs::generation::tools::ToolCall>,
    ) -> Result<Vec<(String, String, String, bool)>> {
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
                    let informative_error = if let Some(res) = last_res {
                        format!(
                            "⚠️ [REPETITION ALERT]: You have already performed this exact action with these arguments. DO NOT REPEAT.\n\nSYSTEM RECALL: Here is the result of your PREVIOUS execution (provided so you don't have to call it again):\n---\n{}\n---",
                            res
                        )
                    } else {
                        "⚠️ [REPETITION ALERT]: You have already performed this exact action with these arguments. DO NOT REPEAT. If you are finished, acknowledge and stop.".to_string()
                    };

                    let tx_opt = self.event_tx.lock().clone();
                    if let Some(tx) = tx_opt {
                        let _ = tx.try_send(crate::tui::AgentEvent::SentinelUpdate {
                            active: vec!["Loop Breaker".to_string()],
                            log: format!("Blocked duplicate {}", call.function.name),
                        });
                        let _ = tx.try_send(crate::tui::AgentEvent::ToolStart {
                            name: call.function.name.clone(),
                            args: Some(call.function.arguments.to_string()),
                        });
                        let _ = tx.try_send(crate::tui::AgentEvent::ToolError {
                            name: repaired_name.clone(),
                            error: informative_error.clone(),
                            args: Some(call.function.arguments.to_string()),
                        });
                    }

                    results.push((
                        call.function.name.clone(),
                        call.function.arguments.to_string(),
                        informative_error,
                        false,
                    ));
                    continue;
                }

                // Track this call for future repetition checks (keep last 10 for better coverage)
                stack.insert(0, (call_key, repaired_name, None));
                if stack.len() > 10 {
                    stack.pop();
                }
                filtered_calls.push(call);
            }
        }

        let mut current_batch: Vec<tokio::task::JoinHandle<(String, String, String, bool)>> =
            Vec::new();
        let mut current_batch_calls = Vec::new();
        let mut resource_locks = std::collections::HashSet::new();

        for tool_call in filtered_calls {
            let tool_name = tool_call.function.name.clone();
            let is_modifying = self
                .tools
                .get(&tool_name)
                .map(|t| t.is_modifying())
                .unwrap_or(false);

            // Extract target resource (file path) for concurrency checks
            let resource = tool_call
                .function
                .arguments
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
                    for (i, res) in batch_results.into_iter().enumerate() {
                        match res {
                            Ok(tool_res) => results.push(tool_res),
                            Err(e) => {
                                let call = &current_batch_calls[i];
                                let err_msg = if e.is_panic() {
                                    format!(
                                        "Skelegent Error: Tool '{}' panicked during execution (likely missing or invalid required arguments). Please check the tool schema.",
                                        call.function.name
                                    )
                                } else {
                                    format!("Skelegent Error: Tool execution failed: {}", e)
                                };
                                results.push((
                                    call.function.name.clone(),
                                    call.function.arguments.to_string(),
                                    err_msg,
                                    false,
                                ));
                            }
                        }
                    }
                } else {
                    for call in &current_batch_calls {
                        results.push((
                            call.function.name.clone(),
                            call.function.arguments.to_string(),
                            "Error: Batch execution was rejected by the user.".to_string(),
                            false,
                        ));
                    }
                }

                current_batch = Vec::new();
                current_batch_calls = Vec::new();
                resource_locks.clear();
                if let Some(res) = &resource {
                    resource_locks.insert(res.clone());
                }
            }

            current_batch_calls.push(tool_call.clone());

            // Prepare parallel task
            let self_clone = self.clone();
            let call_clone = tool_call.clone();

            let task = async move {
                let is_mod = self_clone
                    .tools
                    .get(&call_clone.function.name)
                    .map(|t| t.is_modifying())
                    .unwrap_or(false);
                if is_mod {
                    let mut cp = self_clone.checkpoint_mgr.lock();
                    cp.begin_checkpoint(&format!("Tool: {}", call_clone.function.name));

                    for param in &["path", "file_path", "old_path", "new_path"] {
                        if let Some(path_str) = call_clone
                            .function
                            .arguments
                            .get(*param)
                            .and_then(|v| v.as_str())
                        {
                            let expanded = shellexpand::tilde(path_str).to_string();
                            cp.snapshot_file(std::path::Path::new(&expanded));
                        }
                    }
                }

                let start_tool = std::time::Instant::now();
                let (res_name, res_out, res_succ) = self_clone
                    .process_single_tool_call(call_clone.clone(), true)
                    .await;
                self_clone.tool_time_ms.fetch_add(
                    start_tool.elapsed().as_millis() as u64,
                    std::sync::atomic::Ordering::Relaxed,
                );

                if is_mod {
                    if res_succ {
                        let _ = self_clone.checkpoint_mgr.lock().commit_checkpoint();
                    } else {
                        let rollback_res = self_clone.checkpoint_mgr.lock().rollback_pending();
                        match rollback_res {
                            Ok(summary) => {
                                let tx_opt = self_clone.event_tx.lock().clone();
                                if let Some(tx) = tx_opt {
                                    let _ = tx
                                        .send(crate::tui::AgentEvent::SystemUpdate(format!(
                                            "⚠️ Tool failed. Rolled back modifications:\n{}",
                                            summary
                                        )))
                                        .await;
                                }
                            }
                            Err(_) => {
                                self_clone.checkpoint_mgr.lock().discard_pending();
                            }
                        }
                    }
                }
                (
                    res_name,
                    call_clone.function.arguments.to_string(),
                    res_out,
                    res_succ,
                )
            };
            current_batch.push(tokio::spawn(task));
        }

        if !current_batch_calls.is_empty() {
            let approved = self.handle_batch_approval(&current_batch_calls).await;
            if approved {
                let batch_results = futures::future::join_all(current_batch).await;
                for (i, res) in batch_results.into_iter().enumerate() {
                    match res {
                        Ok(tool_res) => results.push(tool_res),
                        Err(e) => {
                            let call = &current_batch_calls[i];
                            let err_msg = if e.is_panic() {
                                format!(
                                    "Skelegent Error: Tool '{}' panicked during execution (likely missing or invalid required arguments). Please check the tool schema.",
                                    call.function.name
                                )
                            } else {
                                format!("Skelegent Error: Tool execution failed: {}", e)
                            };
                            results.push((
                                call.function.name.clone(),
                                call.function.arguments.to_string(),
                                err_msg,
                                false,
                            ));
                        }
                    }
                }
            } else {
                for call in current_batch_calls {
                    results.push((
                        call.function.name.clone(),
                        call.function.arguments.to_string(),
                        "Error: Batch execution was rejected by the user.".to_string(),
                        false,
                    ));
                }
            }
        }

        Ok(results)
    }

    async fn handle_batch_approval(
        &self,
        calls: &[ollama_rs::generation::tools::ToolCall],
    ) -> bool {
        if !self.safe_mode.load(std::sync::atomic::Ordering::SeqCst) {
            return true;
        }

        let mut modifying_previews = Vec::new();
        let mut tool_names = Vec::new();

        for call in calls {
            let tool_name = self.repair_tool_name(&call.function.name);
            if let Some(tool) = self.tools.get(&tool_name).map(|r| r.value().clone())
                && tool.is_modifying()
            {
                let args = &call.function.arguments;
                let preview = tool.get_approval_preview(args).await;

                let mut prompt_chunk = String::new();
                if let Some(p) = preview {
                    prompt_chunk.push_str(&p);
                } else {
                    let target_path = args
                        .get("path")
                        .or(args.get("file_path"))
                        .and_then(|v| v.as_str())
                        .map(|s| shellexpand::tilde(s).to_string());

                    let new_content = args
                        .get("content")
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
                            .chars()
                            .take(200)
                            .collect::<String>();
                        prompt_chunk.push_str(&format!("Arguments: {}\n", args_preview));
                    }
                }
                modifying_previews.push(prompt_chunk);
                tool_names.push(tool_name.to_uppercase());
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
            format!(
                "BATCH ({} actions: {})",
                tool_names.len(),
                tool_names.join(", ")
            )
        } else {
            tool_names[0].clone()
        };

        final_prompt.push_str(&format!(
            "APPROVE {}? [ENTER/ESC]  (⏪ {} checkpoints available)",
            batch_label, cp_count
        ));

        let tx_opt = self.event_tx.lock().clone();
        if let Some(tx) = tx_opt {
            let _ = tx
                .send(crate::tui::AgentEvent::RequestInput(
                    "BATCH_APPROVAL".to_string(),
                    final_prompt,
                ))
                .await;
        }

        // Wait for user response
        let mut rx_lock = self.tool_rx.lock().await;
        if let Some(rx) = rx_lock.as_mut() {
            match tokio::time::timeout(tokio::time::Duration::from_secs(300), rx.recv()).await {
                Ok(Some(crate::tui::ToolResponse::Text(ans))) => {
                    let lower = ans.trim().to_lowercase();
                    lower == "y" || lower == "yes" || lower.is_empty()
                }
                _ => false,
            }
        } else {
            false
        }
    }

    fn map_tool_to_effect(
        &self,
        name: &str,
        args: &serde_json::Value,
    ) -> Option<crate::effects::TempestEffect> {
        match name {
            "skg_read_file" | "read_file" => {
                let path = args
                    .get("path")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                if !path.is_empty() {
                    Some(crate::effects::TempestEffect::ReadFile { path })
                } else {
                    None
                }
            }
            "skg_write_file" | "write_file" => {
                let path = args
                    .get("path")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                let content = args
                    .get("content")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                let force = args
                    .get("force_overwrite")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);
                if !path.is_empty() {
                    Some(crate::effects::TempestEffect::WriteFile {
                        path,
                        content,
                        force_overwrite: force,
                    })
                } else {
                    None
                }
            }
            "run_command" => {
                let command = args
                    .get("command")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                let cwd = args
                    .get("cwd")
                    .and_then(|v| v.as_str())
                    .unwrap_or(".")
                    .to_string();
                if !command.is_empty() {
                    Some(crate::effects::TempestEffect::RunCommand { command, cwd })
                } else {
                    None
                }
            }
            _ => None,
        }
    }

    async fn process_single_tool_call(
        &self,
        tool_call: ollama_rs::generation::tools::ToolCall,
        skip_approval: bool,
    ) -> (String, String, bool) {
        let tx_opt = self.event_tx.lock().clone();
        if let Some(ref tx) = tx_opt {
            let _ = tx.try_send(crate::tui::AgentEvent::ToolStart {
                name: tool_call.function.name.clone(),
                args: Some(tool_call.function.arguments.to_string()),
            });
        }

        let result = self
            .process_single_tool_call_internal(tool_call.clone(), skip_approval)
            .await;

        if let Some(ref tx) = tx_opt {
            if result.2 {
                let _ = tx.try_send(crate::tui::AgentEvent::ToolSuccess {
                    name: result.0.clone(),
                    args: Some(tool_call.function.arguments.to_string()),
                    output: Some(result.1.clone()),
                });
            } else {
                let _ = tx.try_send(crate::tui::AgentEvent::ToolError {
                    name: result.0.clone(),
                    error: if result.1.is_empty() {
                        "Execution failed".to_string()
                    } else {
                        result.1.clone()
                    },
                    args: Some(tool_call.function.arguments.to_string()),
                });
            }
        }

        result
    }

    async fn process_single_tool_call_internal(
        &self,
        tool_call: ollama_rs::generation::tools::ToolCall,
        skip_approval: bool,
    ) -> (String, String, bool) {
        let tool_name = self.repair_tool_name(&tool_call.function.name);
        let mut args = tool_call.function.arguments.clone();

        // 🛡️ ALGEBRAIC EFFECT SANDBOX DISPATCH
        if let Some(effect) = self.map_tool_to_effect(&tool_name, &args) {
            // Combine recent user messages for intent matching to prevent guard clipping on simple confirmations
            let last_user_msg = {
                let history = self.history.lock();
                let user_msgs: Vec<String> = history
                    .iter()
                    .filter(|m| m.role == ollama_rs::generation::chat::MessageRole::User)
                    .map(|m| m.content.clone())
                    .collect();
                if user_msgs.is_empty() {
                    None
                } else {
                    let start = user_msgs.len().saturating_sub(5);
                    Some(user_msgs[start..].join("\n"))
                }
            };

            let overwatch_verdict = self
                .overwatch
                .validate_effects(std::slice::from_ref(&effect), last_user_msg.as_deref());
            if let crate::overwatch::OverwatchVerdict::Intercept { correction, .. } =
                overwatch_verdict
            {
                let tx_opt = self.event_tx.lock().clone();
                if let Some(tx) = tx_opt {
                    let _ = tx.try_send(crate::tui::AgentEvent::SystemUpdate(format!(
                        "🛑 [OVERWATCH EFFECT INTERCEPT]: Blocked execution of {}",
                        tool_name
                    )));
                }
                return (tool_name.clone(), correction, false);
            }

            let tx_opt = self.event_tx.lock().clone();
            if let Some(tx) = tx_opt {
                let _ = tx.try_send(crate::tui::AgentEvent::SystemUpdate(format!(
                    "🌪️ [ALGEBRAIC EFFECT RUNNER]: Executing {}",
                    tool_name
                )));
            }

            let executor = crate::effects::TempestEffectExecutor::new();
            match executor.execute_effect(effect).await {
                Ok(res) => {
                    metrics::counter!("tool.success", "tool" => tool_name.clone()).increment(1);
                    self.tool_stats
                        .entry(tool_name.to_string())
                        .and_modify(|(s, _)| *s += 1)
                        .or_insert((1, 0));
                    return (tool_name.clone(), res, true);
                }
                Err(e) => {
                    metrics::counter!("tool.failure", "tool" => tool_name.clone()).increment(1);
                    self.tool_stats
                        .entry(tool_name.to_string())
                        .and_modify(|(_, f)| *f += 1)
                        .or_insert((0, 1));
                    return (
                        tool_name.clone(),
                        format!("Algebraic Effect Error: {}", e),
                        false,
                    );
                }
            }
        }

        // Fuzzy Repair Logic continues below...

        // Map mutual query/keyword argument discrepancies for memory recall/search tools
        if (tool_name == "recall_memory" || tool_name == "memory_search")
            && let Some(obj) = args.as_object_mut()
            && !obj.contains_key("query")
        {
            if let Some(keyword) = obj.remove("keyword") {
                obj.insert("query".to_string(), keyword);
            } else if let Some(keywords) = obj.remove("keywords") {
                obj.insert("query".to_string(), keywords);
            }
        }

        if tool_name == "recall_brain"
            && let Some(obj) = args.as_object_mut()
            && !obj.contains_key("keyword")
        {
            if let Some(query) = obj.remove("query") {
                obj.insert("keyword".to_string(), query);
            } else if let Some(keywords) = obj.remove("keywords") {
                obj.insert("keyword".to_string(), keywords);
            }
        }

        if tool_name == "get_stock_price"
            && let Some(obj) = args.as_object_mut()
            && obj.contains_key("symbol")
            && !obj.contains_key("ticker")
            && let Some(sym) = obj.remove("symbol")
        {
            obj.insert("ticker".to_string(), sym);
        }

        if tool_name == "run_command"
            && let Some(obj) = args.as_object_mut()
            && obj.contains_key("command")
            && obj.contains_key("options")
        {
            let cmd = obj.get("command").and_then(|v| v.as_str()).unwrap_or("");
            let opts = obj.get("options").and_then(|v| v.as_str()).unwrap_or("");
            if !opts.is_empty() {
                obj.insert(
                    "command".to_string(),
                    serde_json::json!(format!("{} {}", cmd, opts)),
                );
                obj.remove("options");
            }
        }

        // --- 🌪️ SKELEGENT HYBRID DISPATCH ---
        // We check the Skelegent registry. If tool_engine is "skg", we execute the tool
        // via the Skelegent pipeline if it is present. Otherwise, we only execute via Skelegent
        // if the tool is not present in the legacy self.tools map.
        let run_skg = if self.tool_engine == "skg" {
            self.tool_registry_skg.get(&tool_name).is_some()
        } else {
            self.tool_registry_skg.get(&tool_name).is_some() && !self.tools.contains_key(&tool_name)
        };

        if run_skg {
            let skg_tool = self.tool_registry_skg.get(&tool_name).unwrap();

            if skg_tool.requires_approval()
                && !skip_approval
                && !self
                    .handle_batch_approval(std::slice::from_ref(&tool_call))
                    .await
            {
                return (
                    tool_name.clone(),
                    format!("Error: Tool '{}' was rejected by the user.", tool_name),
                    false,
                );
            }

            let tx_opt = self.event_tx.lock().clone();
            if let Some(tx) = tx_opt {
                let _ = tx.try_send(crate::tui::AgentEvent::SystemUpdate(format!(
                    "🌪️ [SKELEGENT]: Executing native tool: {}",
                    tool_name
                )));
            }

            let start = std::time::Instant::now();
            metrics::gauge!("tool.semaphore_available_permits")
                .set(self.concurrency_semaphore.available_permits() as f64);

            let _permit = self.concurrency_semaphore.acquire().await.ok();
            let context = self.get_tool_context().await;
            let skg_ctx = skg_tool::ToolCallContext::with_deps(
                layer0::id::OperatorId::new("tempest-agent"),
                Arc::new(Arc::new(context)),
            );

            match skg_tool.call(args.clone(), &skg_ctx).await {
                Ok(res) => {
                    let duration = start.elapsed();
                    metrics::histogram!("tool.execution_ms", "tool" => tool_name.clone())
                        .record(duration.as_millis() as f64);

                    let res_str = match res {
                        Value::String(s) => s,
                        _ => res.to_string(),
                    };
                    metrics::counter!("tool.success", "tool" => tool_name.clone()).increment(1);
                    self.tool_stats
                        .entry(tool_name.to_string())
                        .and_modify(|(s, _)| *s += 1)
                        .or_insert((1, 0));
                    return (tool_name.to_string(), res_str, true);
                }
                Err(e) => {
                    let duration = start.elapsed();
                    metrics::histogram!("tool.execution_ms", "tool" => tool_name.clone())
                        .record(duration.as_millis() as f64);

                    metrics::counter!("tool.failure", "tool" => tool_name.clone()).increment(1);
                    self.tool_stats
                        .entry(tool_name.to_string())
                        .and_modify(|(_, f)| *f += 1)
                        .or_insert((0, 1));
                    return (
                        tool_name.to_string(),
                        format!("Skelegent Error: {}", e),
                        false,
                    );
                }
            }
        }

        if let Some(tool) = self.tools.get(&tool_name).map(|r| r.value().clone()) {
            if tool.is_modifying() && !skip_approval {
                // Single-tool fallback approval if not already handled by a batch
                if !self
                    .handle_batch_approval(std::slice::from_ref(&tool_call))
                    .await
                {
                    return (
                        tool_name.clone(),
                        format!("Error: Tool '{}' was rejected by the user.", tool_name),
                        false,
                    );
                }
            }

            // Log modification if in auto-mode
            if tool.is_modifying() && !self.safe_mode.load(std::sync::atomic::Ordering::SeqCst) {
                let preview = tool.get_approval_preview(&args).await;
                let tx_opt = self.event_tx.lock().clone();
                if let Some(tx) = tx_opt {
                    if let Some(p) = preview {
                        let _ = tx
                            .send(crate::tui::AgentEvent::CommandOutput(format!(
                                "🚀 [AUTO-MODIFY]: {}\n{}",
                                tool_name, p
                            )))
                            .await;
                    } else {
                        let _ = tx
                            .send(crate::tui::AgentEvent::SystemUpdate(format!(
                                "🚀 [AUTO-MODIFY]: Executing {}",
                                tool_name
                            )))
                            .await;
                    }
                }
            }

            let mut last_result = (
                tool_name.clone(),
                "Error: Tool execution failed and could not be retried.".to_string(),
                false,
            );
            let max_attempts = 5;

            for attempt in 1..=max_attempts {
                let start = std::time::Instant::now();
                metrics::gauge!("tool.semaphore_available_permits")
                    .set(self.concurrency_semaphore.available_permits() as f64);

                let _permit = self.concurrency_semaphore.acquire().await.ok();
                let context = self.get_tool_context().await;

                match tool.execute(&args, context).await {
                    Ok(res) => {
                        let duration = start.elapsed();
                        metrics::histogram!("tool.execution_ms", "tool" => tool_name.clone())
                            .record(duration.as_millis() as f64);

                        let result = (tool_name.to_string(), res, true);
                        self.recent_tool_calls
                            .insert(tool_name.to_string(), result.1.chars().take(100).collect());

                        // Increment success stats
                        self.tool_stats
                            .entry(tool_name.to_string())
                            .and_modify(|(s, _)| *s += 1)
                            .or_insert((1, 0));

                        // Prune tracking maps periodically to prevent unbounded growth
                        if (self.recent_tool_calls.len() + self.tool_stats.len()).is_multiple_of(20)
                        {
                            self.prune_tracking_maps();
                        }

                        metrics::counter!("tool.success", "tool" => tool_name.clone()).increment(1);

                        return result;
                    }
                    Err(e) => {
                        let err_msg = format!("{}", e);

                        // Increment failure stats on final attempt or if non-retryable
                        let classification =
                            crate::error_classifier::classify_error(&tool_name, &err_msg);
                        if classification.class != crate::error_classifier::ErrorClass::Retryable
                            || attempt == max_attempts
                        {
                            self.tool_stats
                                .entry(tool_name.to_string())
                                .and_modify(|(_, f)| *f += 1)
                                .or_insert((0, 1));

                            metrics::counter!("tool.failure", "tool" => tool_name.clone())
                                .increment(1);
                        }

                        if classification.class == crate::error_classifier::ErrorClass::Retryable
                            && attempt < max_attempts
                        {
                            // Exponential backoff with jitter
                            let wait_duration = {
                                use rand::RngExt;
                                let base_wait = 2u64.pow(attempt as u32 - 1);
                                let jitter_ms = rand::rng().random_range(0..1000);
                                tokio::time::Duration::from_millis(base_wait * 1000 + jitter_ms)
                            };

                            let tx_opt = self.event_tx.lock().clone();
                            if let Some(tx) = tx_opt {
                                let _ = tx
                                    .send(crate::tui::AgentEvent::SystemUpdate(format!(
                                        "🔄 [{}/{}] Retrying {} in {:.1}s: {}",
                                        attempt,
                                        max_attempts,
                                        tool_name,
                                        wait_duration.as_secs_f32(),
                                        err_msg
                                    )))
                                    .await;
                            }
                            tokio::time::sleep(wait_duration).await;
                            last_result = (
                                tool_name.clone(),
                                format!("Error (Failed after {} attempts): {}", attempt, err_msg),
                                false,
                            );
                            continue;
                        } else {
                            let tip = if let Some(t) = &classification.tip {
                                format!("\n\nSYSTEM TIP: {}", t)
                            } else if classification.class
                                == crate::error_classifier::ErrorClass::Recoverable
                            {
                                "\n\nSYSTEM TIP: This failure might be recoverable by changing your strategy or asking the user for clarification.".to_string()
                            } else {
                                "".to_string()
                            };
                            last_result = (
                                tool_name.to_string(),
                                format!("Error: {}{}", err_msg, tip),
                                false,
                            );
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
                false,
            )
        }
    }

    pub async fn get_tool_context(&self) -> ToolContext {
        let real_tx = self.event_tx.lock().clone();

        ToolContext {
            ollama: self.get_ollama().await.unwrap_or_else(|_| {
                ollama_rs::Ollama::from_url(reqwest::Url::parse("http://127.0.0.1:11434").unwrap())
            }),
            backend: self.backend.clone(),
            model: self.model.lock().clone(),
            sub_agent_model: self.sub_agent_model.clone(),
            embedding_model: self.embedding_model.clone(),
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
            memory_store: self.memory_store.clone(),
        }
    }

    // Removed auto_summarize_memory (unused)

    fn extract_tool_calls(&self, content: &str) -> Result<Vec<Value>, String> {
        let block_regex =
            TOOL_BLOCK_RE.get_or_init(|| regex::Regex::new(r"```json\s*([\s\S]*?)\s*```").unwrap());
        let mut calls = Vec::new();
        for caps in block_regex.captures_iter(content) {
            if let Some(m) = caps.get(1) {
                let block_text = m.as_str().trim();

                // Extract tool comment if present (e.g. // Tool: read_file)
                let mut tool_comment_name = None;
                let mut cleaned_block_text = block_text.to_string();
                if let Some(idx) = block_text.find("// Tool:") {
                    let comment_part = &block_text[idx..];
                    let line = if let Some(nl) = comment_part.find('\n') {
                        &comment_part[..nl]
                    } else {
                        comment_part
                    };
                    let name = line["// Tool:".len()..].trim();
                    if !name.is_empty() {
                        tool_comment_name = Some(name.to_string());
                    }
                    cleaned_block_text = block_text.replace(line, "");
                }

                let block_text_repaired = crate::overwatch::repair_json_str(&cleaned_block_text);
                if let Ok(mut val) = serde_json::from_str::<Value>(&block_text_repaired) {
                    if let Some(ref name) = tool_comment_name
                        && let Some(obj) = val.as_object_mut()
                        && !obj.contains_key("tool")
                        && !obj.contains_key("name")
                        && !obj.contains_key("function_name")
                        && !obj.contains_key("function")
                    {
                        obj.insert("name".to_string(), serde_json::Value::String(name.clone()));
                    }
                    // Single valid JSON value
                    if let Some(obj) = val.as_object() {
                        if obj.contains_key("tool")
                            || obj.contains_key("name")
                            || obj.contains_key("function_name")
                            || obj.contains_key("function")
                        {
                            calls.push(val);
                        }
                    } else if let Some(arr) = val.as_array() {
                        // Already a JSON array of tool calls
                        for item in arr {
                            if let Some(obj) = item.as_object()
                                && (obj.contains_key("tool")
                                    || obj.contains_key("name")
                                    || obj.contains_key("function_name")
                                    || obj.contains_key("function"))
                            {
                                calls.push(item.clone());
                            }
                        }
                    }
                } else if let Ok(mut val) =
                    serde_json::from_str::<Value>(&format!("[{}]", cleaned_block_text))
                {
                    // 🛡️ MULTI-CALL RECOVERY: LM Studio models often output multiple JSON objects
                    // separated by commas in a single ```json block (e.g., `{...}, {...}`).
                    // This is invalid JSON on its own, but valid when wrapped in array brackets.
                    if let Some(ref name) = tool_comment_name
                        && let Some(arr) = val.as_array_mut()
                    {
                        for item in arr {
                            if let Some(obj) = item.as_object_mut()
                                && !obj.contains_key("tool")
                                && !obj.contains_key("name")
                                && !obj.contains_key("function_name")
                                && !obj.contains_key("function")
                            {
                                obj.insert(
                                    "name".to_string(),
                                    serde_json::Value::String(name.clone()),
                                );
                            }
                        }
                    }
                    if let Some(arr) = val.as_array() {
                        for item in arr {
                            if let Some(obj) = item.as_object()
                                && (obj.contains_key("tool")
                                    || obj.contains_key("name")
                                    || obj.contains_key("function_name")
                                    || obj.contains_key("function"))
                            {
                                calls.push(item.clone());
                            }
                        }
                    }
                }
            }
        }

        // 🧠 ROBUST JSON CATCHER: Brace-counting extractor for raw or nested JSON objects.
        // This perfectly isolates consecutive or poorly formatted JSON objects even if they
        // are surrounded by raw text, without failing on nested braces like regex would.
        if calls.is_empty() {
            let chars: Vec<char> = content.chars().collect();
            let mut i = 0;
            while i < chars.len() {
                if chars[i] == '{' {
                    let mut brace_count = 0;
                    let mut in_string = false;
                    let mut escape = false;
                    let start_idx = i;
                    let mut end_idx = i;

                    let mut j = i;
                    while j < chars.len() {
                        let c = chars[j];
                        if !escape && c == '"' {
                            in_string = !in_string;
                        }
                        if !in_string && !escape {
                            if c == '{' {
                                brace_count += 1;
                            } else if c == '}' {
                                brace_count -= 1;
                            }
                        }

                        if c == '\\' {
                            escape = !escape;
                        } else {
                            escape = false;
                        }

                        if brace_count == 0 {
                            end_idx = j;
                            break;
                        }
                        j += 1;
                    }

                    if brace_count == 0 {
                        let json_str: String = chars[start_idx..=end_idx].iter().collect();
                        let json_str_repaired = crate::overwatch::repair_json_str(&json_str);
                        if let Ok(val) = serde_json::from_str::<Value>(&json_str_repaired)
                            && let Some(obj) = val.as_object()
                        {
                            let mut name_opt = obj
                                .get("tool")
                                .or(obj.get("name"))
                                .or(obj.get("function"))
                                .or(obj.get("function_name"))
                                .or(obj.get("action"))
                                .and_then(|v| v.as_str());

                            let extracted_name = if name_opt.is_none() {
                                let prefix: String = chars[..start_idx].iter().collect();
                                if let Some(tool_comment_idx) = prefix.rfind("// Tool:") {
                                    let comment_line = &prefix[tool_comment_idx..];
                                    let name = if let Some(newline_idx) = comment_line.find('\n') {
                                        comment_line["// Tool:".len()..newline_idx].trim()
                                    } else {
                                        comment_line["// Tool:".len()..].trim()
                                    };
                                    if !name.is_empty() {
                                        Some(name.to_string())
                                    } else {
                                        None
                                    }
                                } else {
                                    None
                                }
                            } else {
                                None
                            };

                            if let Some(ref name_str) = extracted_name {
                                name_opt = Some(name_str);
                            }

                            if let Some(name) = name_opt {
                                let mut with_name = obj.clone();
                                if !with_name.contains_key("name")
                                    && !with_name.contains_key("tool")
                                {
                                    with_name.insert(
                                        "name".to_string(),
                                        serde_json::Value::String(name.to_string()),
                                    );
                                }
                                let final_val = serde_json::Value::Object(with_name);
                                if !calls.iter().any(|existing: &Value| existing == &final_val) {
                                    calls.push(final_val);
                                }
                            }
                        }
                        // Advance i to end_idx so we don't re-parse inner objects unnecessarily
                        i = end_idx;
                    }
                }
                i += 1;
            }
        }

        // 🧠 FUZZY TOOL CATCHER: If still no formal tool calls found, look for raw code blocks.
        if calls.is_empty() {
            let code_re = regex::Regex::new(r"```(?P<lang>\w+)?\n(?P<code>[\s\S]*?)\n```").unwrap();
            let file_re = regex::Regex::new(r"(?i)(?:file|to|in|at)\s+`?([\w\-\./]+\.(?:py|rs|js|ts|c|cpp|h|go|html|css|json|toml|yaml|yml|md|sh))`?").unwrap();

            for caps in code_re.captures_iter(content) {
                let code = caps.name("code").map(|m| m.as_str()).unwrap_or("");
                let lang = caps.name("lang").map(|m| m.as_str()).unwrap_or("text");

                if lang == "json" || code.is_empty() {
                    continue;
                }

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

    pub fn get_tool_by_name(
        &self,
        name: &str,
    ) -> Option<std::sync::Arc<dyn crate::tools::AgentTool>> {
        self.tools.get(name).map(|r| r.clone())
    }

    pub async fn run_tui_mode(
        &self,
        mut user_rx: tokio::sync::mpsc::Receiver<String>,
        stop: std::sync::Arc<std::sync::atomic::AtomicBool>,
    ) -> Result<()> {
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
                    let _ = tx
                        .send(crate::tui::AgentEvent::SystemUpdate(
                            "🧹 Session Hard Reset: History and Task cleared.".to_string(),
                        ))
                        .await;
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
                            let _ = tx
                                .send(crate::tui::AgentEvent::StreamToken(String::new()))
                                .await;
                        }
                        Err(msg) => {
                            let _ = tx
                                .send(crate::tui::AgentEvent::SystemUpdate(format!("⚠️ {}", msg)))
                                .await;
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
                    let _ = tx
                        .send(crate::tui::AgentEvent::StreamToken(String::new()))
                        .await;
                }
                continue;
            }

            if user_msg == "/dream" {
                let tx_opt = self.event_tx.lock().clone();
                if let Some(tx) = tx_opt.as_ref() {
                    let _ = tx
                        .send(crate::tui::AgentEvent::SystemUpdate(
                            "💤 Entering memory consolidation phase (dreaming)...".to_string(),
                        ))
                        .await;
                }

                match self.consolidate_memories().await {
                    Ok(summary) => {
                        if let Some(tx) = tx_opt {
                            let _ = tx
                                .send(crate::tui::AgentEvent::StreamToken(format!(
                                    "✨ {}\n",
                                    summary
                                )))
                                .await;
                            let _ = tx
                                .send(crate::tui::AgentEvent::StreamToken(String::new()))
                                .await;
                        }
                    }
                    Err(e) => {
                        if let Some(tx) = tx_opt {
                            let _ = tx
                                .send(crate::tui::AgentEvent::SystemUpdate(format!(
                                    "⚠️ Memory consolidation failed: {}",
                                    e
                                )))
                                .await;
                        }
                    }
                }
                continue;
            }

            if user_msg.starts_with("/tool ") {
                let parts: Vec<&str> = user_msg.splitn(3, ' ').collect();
                if parts.len() >= 2 {
                    let tool_name = parts[1];
                    let args_str = if parts.len() == 3 { parts[2] } else { "{}" };

                    match serde_json::from_str::<serde_json::Value>(args_str) {
                        Ok(json_args) => {
                            if let Some(tool) = self.get_tool_by_name(tool_name) {
                                let ctx = self.get_tool_context().await;
                                let result = tool.execute(&json_args, ctx).await;

                                let tx_opt = self.event_tx.lock().clone();
                                if let Some(tx) = tx_opt {
                                    let output = match result {
                                        Ok(msg) => {
                                            format!("🛠️ Tool '{}' Success:\n{}", tool_name, msg)
                                        }
                                        Err(e) => format!("⚠️ Tool '{}' Error:\n{}", tool_name, e),
                                    };
                                    let _ =
                                        tx.send(crate::tui::AgentEvent::StreamToken(output)).await;
                                    let _ = tx
                                        .send(crate::tui::AgentEvent::StreamToken(String::new()))
                                        .await;
                                }
                            } else {
                                let tx_opt = self.event_tx.lock().clone();
                                if let Some(tx) = tx_opt {
                                    let _ = tx
                                        .send(crate::tui::AgentEvent::StreamToken(format!(
                                            "⚠️ Tool '{}' not found in registry.",
                                            tool_name
                                        )))
                                        .await;
                                    let _ = tx
                                        .send(crate::tui::AgentEvent::StreamToken(String::new()))
                                        .await;
                                }
                            }
                        }
                        Err(e) => {
                            let tx_opt = self.event_tx.lock().clone();
                            if let Some(tx) = tx_opt {
                                let _ = tx
                                    .send(crate::tui::AgentEvent::StreamToken(format!(
                                        "⚠️ Invalid JSON arguments: {}",
                                        e
                                    )))
                                    .await;
                                let _ = tx
                                    .send(crate::tui::AgentEvent::StreamToken(String::new()))
                                    .await;
                            }
                        }
                    }
                } else {
                    let tx_opt = self.event_tx.lock().clone();
                    if let Some(tx) = tx_opt {
                        let _ = tx
                            .send(crate::tui::AgentEvent::StreamToken(
                                "⚠️ Usage: /tool <tool_name> <json_args>".to_string(),
                            ))
                            .await;
                        let _ = tx
                            .send(crate::tui::AgentEvent::StreamToken(String::new()))
                            .await;
                    }
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
                        let _ = tx
                            .send(crate::tui::AgentEvent::AgentError(e.to_string()))
                            .await;
                    }
                }
            }));

            // Reset stop is not needed here as it's per-task now
        }
        if let Some(task) = current_task.take() {
            task.abort();
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
            "warmup".to_string(),
        )];

        let model_name = self.model.lock().clone();

        // We use a tiny max_len for the warmup pulse
        let _ = self
            .backend
            .read()
            .await
            .clone()
            .stream_chat(crate::inference::ChatRequest {
                model: model_name,
                history: dummy_history,
                sampling: crate::inference::SamplingConfig {
                    temperature: 0.1,
                    top_p: 0.9,
                    repeat_penalty: 1.1,
                    context_size: 1024,
                },
                event_tx: Arc::new(parking_lot::Mutex::new(None)),
                stop: Arc::new(std::sync::atomic::AtomicBool::new(false)),
                system_prompt: "warmup".to_string(),
                on_tool_call: None,
                tool_registry: None,
            })
            .await;

        let tx_opt = self.event_tx.lock().clone();
        if let Some(tx) = tx_opt {
            let _ = tx
                .send(crate::tui::AgentEvent::SystemUpdate(
                    "✅ Engine ready.".to_string(),
                ))
                .await;
        }

        Ok(())
    }

    /// Prune tracking maps to prevent unbounded memory growth.
    /// Called periodically to limit the size of recent_failures, recent_tool_calls, and tool_stats.
    fn prune_tracking_maps(&self) {
        const MAX_RECENT_FAILURES: usize = 50;
        const MAX_RECENT_CALLS: usize = 100;
        const MAX_TOOL_STATS: usize = 100;

        // Prune recent_failures if it exceeds max size
        if self.recent_failures.len() > MAX_RECENT_FAILURES {
            let to_remove: Vec<String> = self
                .recent_failures
                .iter()
                .take(self.recent_failures.len() - MAX_RECENT_FAILURES + 10)
                .map(|entry| entry.key().clone())
                .collect();
            for key in to_remove {
                self.recent_failures.remove(&key);
            }
        }

        // Prune recent_tool_calls if it exceeds max size
        if self.recent_tool_calls.len() > MAX_RECENT_CALLS {
            let to_remove: Vec<String> = self
                .recent_tool_calls
                .iter()
                .take(self.recent_tool_calls.len() - MAX_RECENT_CALLS + 10)
                .map(|entry| entry.key().clone())
                .collect();
            for key in to_remove {
                self.recent_tool_calls.remove(&key);
            }
        }

        // Prune tool_stats if it exceeds max size (keep highest success-to-failure ratio)
        if self.tool_stats.len() > MAX_TOOL_STATS {
            let mut stats: Vec<_> = self
                .tool_stats
                .iter()
                .map(|entry| (entry.key().clone(), *entry.value()))
                .collect();
            // Sort by success count (descending), then remove bottom entries
            stats.sort_by_key(|entry| std::cmp::Reverse(entry.1.0));
            let to_remove: Vec<String> = stats
                .into_iter()
                .skip(MAX_TOOL_STATS - 10)
                .map(|(k, _)| k)
                .collect();
            for key in to_remove {
                self.tool_stats.remove(&key);
            }
        }
    }

    /// Explicitly unload all loaded models from Ollama's memory (GPU) by sending a request with keep_alive: 0.
    pub async fn shutdown(&self) {
        let backend = self.backend.read().await;

        let mut models = std::collections::HashSet::new();
        models.insert(self.model.lock().clone());
        if let Some(planner) = &self.planner_model {
            models.insert(planner.clone());
        }
        if let Some(executor) = &self.executor_model {
            models.insert(executor.clone());
        }
        if let Some(verifier) = &self.verifier_model {
            models.insert(verifier.clone());
        }
        if !self.sub_agent_model.is_empty() {
            models.insert(self.sub_agent_model.clone());
        }
        if !self.embedding_model.is_empty() {
            models.insert(self.embedding_model.clone());
        }

        for model in models {
            if !model.is_empty() {
                backend.shutdown(model).await;
            }
        }
    }

    #[cfg(target_os = "macos")]
    async fn _unused_placeholder() {}

    pub fn print_interaction_summary(&self) {
        let uptime = self.start_time.elapsed();
        let hours = uptime.as_secs() / 3600;
        let minutes = (uptime.as_secs() % 3600) / 60;
        let secs = uptime.as_secs() % 60;

        let api_time = std::time::Duration::from_millis(
            self.api_time_ms.load(std::sync::atomic::Ordering::Relaxed),
        );
        let tool_time = std::time::Duration::from_millis(
            self.tool_time_ms.load(std::sync::atomic::Ordering::Relaxed),
        );

        let api_secs = api_time.as_secs();
        let tool_secs = tool_time.as_secs();
        let uptime_secs = uptime.as_secs().max(1);

        let mut total_success = 0;
        let mut total_fail = 0;

        for entry in self.tool_stats.iter() {
            let (s, f) = *entry.value();
            total_success += s;
            total_fail += f;
        }
        let total_tools = total_success + total_fail;

        let success_rate = if total_tools > 0 {
            (total_success as f64 / total_tools as f64) * 100.0
        } else {
            0.0
        };

        let w: usize = 130;
        let line = |s: &str| {
            let len = s.chars().count();
            let pad = w.saturating_sub(len);
            println!(" │{}{}│", s, " ".repeat(pad));
        };

        // Ensure standard terminal colors are reset before printing
        println!("\x1b[0m");
        println!(" ╭{}╮", "─".repeat(w));
        line("  Agent powering down. Goodbye!");
        line("");
        line("  Interaction Summary");
        line(&format!(
            "  Session ID:                 {}",
            self.session_id
        ));
        line(&format!(
            "  Tool Calls:                 {} ( ✓ {} x {} )",
            total_tools, total_success, total_fail
        ));
        line(&format!(
            "  Success Rate:               {:.1}%",
            success_rate
        ));
        line("");
        line("  Performance");
        line(&format!(
            "  Wall Time:                  {}h {}m {}s",
            hours, minutes, secs
        ));
        let total_tokens = self.total_tokens.load(std::sync::atomic::Ordering::Relaxed);
        let api_secs_f64 = api_time.as_secs_f64();
        let tps = if api_secs_f64 > 0.0 {
            total_tokens as f64 / api_secs_f64
        } else {
            0.0
        };
        line(&format!(
            "  Total Tokens:               {} ( {:.1} tok/s )",
            total_tokens, tps
        ));
        line(&format!(
            "  Agent Active:               {}s",
            api_secs + tool_secs
        ));
        line(&format!(
            "    » API Time:               {}s ({:.1}%)",
            api_secs,
            (api_secs as f64 / uptime_secs as f64) * 100.0
        ));
        line(&format!(
            "    » Tool Time:              {}s ({:.1}%)",
            tool_secs,
            (tool_secs as f64 / uptime_secs as f64) * 100.0
        ));
        line("");
        line(&format!(
            "  To resume this session: tempest_ai --resume {}",
            self.session_id
        ));
        println!(" ╰{}╯", "─".repeat(w));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_agent_new() {
        let memory_store = Arc::new(Mutex::new(
            crate::memory::MemoryStore::new("test-passphrase".to_string()).unwrap(),
        ));
        let agent = Agent::new(
            crate::inference::AgentMode::Ollama,
            "test-model".to_string(),
            "Q4_K_M".to_string(),
            "test-prompt".to_string(),
            "/tmp/tempest_test_history.json".to_string(),
            "test-session-id".to_string(),
            memory_store,
            "test-sub-model".to_string(),
            "test-embedding-model".to_string(),
            Arc::new(Mutex::new(None)),
            AgentConfig {
                planner_model: None,
                executor_model: None,
                verifier_model: None,
                mlx_presets: std::collections::HashMap::new(),
                temp_planning: 0.05,
                temp_execution: 0.25,
                top_p_planning: 0.95,
                top_p_execution: 0.92,
                repeat_penalty_planning: 1.18,
                repeat_penalty_execution: 1.12,
                ctx_planning: 16384,
                ctx_execution: 32768,
                mlx_temp_planning: None,
                mlx_temp_execution: None,
                mlx_top_p_planning: None,
                mlx_top_p_execution: None,
                mlx_repeat_penalty_planning: None,
                mlx_repeat_penalty_execution: None,
                paged_attn: false,
                planning_enabled: true,
                lmstudio_url: None,
                pa_memory_mb: None,
                vram_time_sharing: false,
                ollama_remote: None,
                tool_engine: "legacy".to_string(),
            },
        )
        .await;
        assert!(agent.is_ok());
        let agent = agent.unwrap();
        assert_eq!(agent.sub_agent_model, "test-sub-model");
        assert!(agent.tool_registry.is_empty());
        assert!(!agent.tools.is_empty());
        assert!(!agent.tool_rag_index.read().await.all_tools().is_empty());
    }

    #[tokio::test]
    async fn test_vram_time_sharing_and_prompts() {
        let memory_store = Arc::new(Mutex::new(
            crate::memory::MemoryStore::new("test-passphrase".to_string()).unwrap(),
        ));
        let agent = Agent::new(
            crate::inference::AgentMode::Ollama,
            "qwen2.5-coder:7b".to_string(),
            "Q4_K_M".to_string(),
            "test-prompt".to_string(),
            "/tmp/tempest_test_history_sharing.json".to_string(),
            "test-session-id-2".to_string(),
            memory_store,
            "test-sub-model".to_string(),
            "test-embedding-model".to_string(),
            Arc::new(Mutex::new(None)),
            AgentConfig {
                planner_model: Some("deepseek-r1:8b".to_string()),
                executor_model: Some("qwen2.5-coder:7b".to_string()),
                verifier_model: Some("deepseek-r1:8b".to_string()),
                mlx_presets: std::collections::HashMap::new(),
                temp_planning: 0.05,
                temp_execution: 0.25,
                top_p_planning: 0.95,
                top_p_execution: 0.92,
                repeat_penalty_planning: 1.18,
                repeat_penalty_execution: 1.12,
                ctx_planning: 16384,
                ctx_execution: 32768,
                mlx_temp_planning: None,
                mlx_temp_execution: None,
                mlx_top_p_planning: None,
                mlx_top_p_execution: None,
                mlx_repeat_penalty_planning: None,
                mlx_repeat_penalty_execution: None,
                paged_attn: false,
                planning_enabled: true,
                lmstudio_url: None,
                pa_memory_mb: None,
                vram_time_sharing: true,
                ollama_remote: None,
                tool_engine: "legacy".to_string(),
            },
        )
        .await
        .unwrap();

        assert!(agent.vram_time_sharing);

        let planning_prompt = agent.build_system_prompt(AgentPhase::Planning, "qwen2.5-coder:7b");
        assert!(planning_prompt.contains("CURRENT OPERATIONAL PHASE: PLANNING"));

        let execution_prompt = agent.build_system_prompt(AgentPhase::Execution, "qwen2.5-coder:7b");
        assert!(execution_prompt.contains("CURRENT OPERATIONAL PHASE: EXECUTION"));

        let testing_prompt = agent.build_system_prompt(AgentPhase::Testing, "qwen2.5-coder:7b");
        assert!(testing_prompt.contains("CURRENT OPERATIONAL PHASE: TESTING"));
    }

    #[tokio::test]
    async fn test_extract_tool_calls_with_comments() {
        let memory_store = Arc::new(Mutex::new(
            crate::memory::MemoryStore::new("test-passphrase".to_string()).unwrap(),
        ));
        let agent = Agent::new(
            crate::inference::AgentMode::Ollama,
            "test-model".to_string(),
            "Q4_K_M".to_string(),
            "test-prompt".to_string(),
            "/tmp/tempest_test_extract_comments.json".to_string(),
            "test-session-id-3".to_string(),
            memory_store,
            "test-sub-model".to_string(),
            "test-embedding-model".to_string(),
            Arc::new(Mutex::new(None)),
            AgentConfig {
                planner_model: None,
                executor_model: None,
                verifier_model: None,
                mlx_presets: std::collections::HashMap::new(),
                temp_planning: 0.05,
                temp_execution: 0.25,
                top_p_planning: 0.95,
                top_p_execution: 0.92,
                repeat_penalty_planning: 1.18,
                repeat_penalty_execution: 1.12,
                ctx_planning: 16384,
                ctx_execution: 32768,
                mlx_temp_planning: None,
                mlx_temp_execution: None,
                mlx_top_p_planning: None,
                mlx_top_p_execution: None,
                mlx_repeat_penalty_planning: None,
                mlx_repeat_penalty_execution: None,
                paged_attn: false,
                planning_enabled: true,
                lmstudio_url: None,
                pa_memory_mb: None,
                vram_time_sharing: false,
                ollama_remote: None,
                tool_engine: "legacy".to_string(),
            },
        )
        .await
        .unwrap();

        // 1. Raw text comment-based tool call (Brace-counting fallback)
        let raw_content = "Please run this:\n// Tool: read_file\n{\"path\": \"src/main.rs\"}";
        let calls = agent.extract_tool_calls(raw_content).unwrap();
        assert_eq!(calls.len(), 1);
        assert_eq!(
            calls[0].get("name").and_then(|v| v.as_str()),
            Some("read_file")
        );
        assert_eq!(
            calls[0].get("path").and_then(|v| v.as_str()),
            Some("src/main.rs")
        );

        // 2. Markdown code block comment-based tool call
        let md_content =
            "Here is the block:\n```json\n// Tool: run_command\n{\"command\": \"ls\"}\n```";
        let calls_md = agent.extract_tool_calls(md_content).unwrap();
        assert_eq!(calls_md.len(), 1);
        assert_eq!(
            calls_md[0].get("name").and_then(|v| v.as_str()),
            Some("run_command")
        );
        assert_eq!(
            calls_md[0].get("command").and_then(|v| v.as_str()),
            Some("ls")
        );

        // 3. Raw JSON tool call without comments (Direct tool key)
        let raw_json_content = "Let me use ast_outline to get an overview.\n{\"tool\": \"ast_outline\", \"arguments\": {\"path\": \"src/main.rs\"}}";
        let calls_raw = agent.extract_tool_calls(raw_json_content).unwrap();
        assert_eq!(calls_raw.len(), 1);
        assert_eq!(
            calls_raw[0].get("tool").and_then(|v| v.as_str()),
            Some("ast_outline")
        );
    }

    #[tokio::test]
    async fn test_tool_and_arg_repair() {
        let memory_store = Arc::new(Mutex::new(
            crate::memory::MemoryStore::new("test-passphrase".to_string()).unwrap(),
        ));
        let agent = Agent::new(
            crate::inference::AgentMode::Ollama,
            "test-model".to_string(),
            "Q4_K_M".to_string(),
            "test-prompt".to_string(),
            "/tmp/tempest_test_history_repair.json".to_string(),
            "test-session-id-repair".to_string(),
            memory_store,
            "test-sub-model".to_string(),
            "test-embedding-model".to_string(),
            Arc::new(Mutex::new(None)),
            AgentConfig {
                planner_model: None,
                executor_model: None,
                verifier_model: None,
                mlx_presets: std::collections::HashMap::new(),
                temp_planning: 0.05,
                temp_execution: 0.25,
                top_p_planning: 0.95,
                top_p_execution: 0.92,
                repeat_penalty_planning: 1.18,
                repeat_penalty_execution: 1.12,
                ctx_planning: 16384,
                ctx_execution: 32768,
                mlx_temp_planning: None,
                mlx_temp_execution: None,
                mlx_top_p_planning: None,
                mlx_top_p_execution: None,
                mlx_repeat_penalty_planning: None,
                mlx_repeat_penalty_execution: None,
                paged_attn: false,
                planning_enabled: true,
                lmstudio_url: None,
                pa_memory_mb: None,
                vram_time_sharing: false,
                ollama_remote: None,
                tool_engine: "legacy".to_string(),
            },
        )
        .await
        .unwrap();

        // Test tool name casing repair
        assert_eq!(agent.repair_tool_name("RECALL_MEMORY"), "recall_memory");
        assert_eq!(agent.repair_tool_name("recall_memory"), "recall_memory");
        assert_eq!(agent.repair_tool_name("SEARCH_WEB"), "search_web"); // via match lowercase alias
        assert_eq!(agent.repair_tool_name("Recall_Brain"), "recall_brain");

        // Test argument field repairs in process_single_tool_call
        let call = ollama_rs::generation::tools::ToolCall {
            function: ollama_rs::generation::tools::ToolCallFunction {
                name: "RECALL_MEMORY".to_string(),
                arguments: serde_json::json!({
                    "keyword": "important system facts"
                }),
            },
        };

        let (name, out, success) = agent.process_single_tool_call(call, true).await;
        assert_eq!(name, "recall_memory");
        assert!(success, "Tool execution failed: {}", out);
        assert!(out.contains("No matching memories found.") || out.contains("memories"));
    }

    #[test]
    fn test_json_extraction() {
        let content = r#"The user is asking me to list the contents of the `./src/` directory. This is a straightforward request to examine the files in the source directory. I should use the `list_dir` tool to retrieve this information. Since the user hasn't specified any particular file, I'll assume they want a general overview of the directory structure. I'll explain my action and then call the tool.
I'll list the contents of the `./src/` directory to show what files are present.
{"tool":"list_dir","arguments":{"path":"./src"}}<｜begin of sentence｜>
The user is asking me to list the contents of the `./src/` directory."#;

        let mut calls = Vec::new();
        let chars: Vec<char> = content.chars().collect();
        let mut i = 0;
        while i < chars.len() {
            if chars[i] == '{' {
                let mut brace_count = 0;
                let mut in_string = false;
                let mut escape = false;
                let start_idx = i;
                let mut end_idx = i;

                let mut j = i;
                while j < chars.len() {
                    let c = chars[j];
                    if !escape && c == '"' {
                        in_string = !in_string;
                    }
                    if !in_string && !escape {
                        if c == '{' {
                            brace_count += 1;
                        } else if c == '}' {
                            brace_count -= 1;
                        }
                    }

                    if c == '\\' {
                        escape = !escape;
                    } else {
                        escape = false;
                    }

                    if brace_count == 0 {
                        end_idx = j;
                        break;
                    }
                    j += 1;
                }

                if brace_count == 0 {
                    let json_str: String = chars[start_idx..=end_idx].iter().collect();
                    if let Ok(val) = serde_json::from_str::<serde_json::Value>(&json_str)
                        && let Some(obj) = val.as_object()
                        && (obj.contains_key("tool")
                            || obj.contains_key("name")
                            || obj.contains_key("function_name")
                            || obj.contains_key("function")
                            || obj.contains_key("action"))
                    {
                        calls.push(val);
                    }
                    i = end_idx;
                }
            }
            i += 1;
        }

        assert_eq!(calls.len(), 1);
    }

    #[tokio::test]
    async fn test_agent_overrides() {
        let memory_store = Arc::new(Mutex::new(
            crate::memory::MemoryStore::new("test-passphrase".to_string()).unwrap(),
        ));
        let agent = Agent::new(
            crate::inference::AgentMode::Ollama,
            "test-model".to_string(),
            "Q4_K_M".to_string(),
            "test-prompt".to_string(),
            "/tmp/tempest_test_history_overrides.json".to_string(),
            "test-session-id-overrides".to_string(),
            memory_store,
            "test-sub-model".to_string(),
            "test-embedding-model".to_string(),
            Arc::new(Mutex::new(None)),
            AgentConfig {
                planner_model: None,
                executor_model: None,
                verifier_model: None,
                mlx_presets: std::collections::HashMap::new(),
                temp_planning: 0.05,
                temp_execution: 0.25,
                top_p_planning: 0.95,
                top_p_execution: 0.92,
                repeat_penalty_planning: 1.18,
                repeat_penalty_execution: 1.12,
                ctx_planning: 16384,
                ctx_execution: 32768,
                mlx_temp_planning: None,
                mlx_temp_execution: None,
                mlx_top_p_planning: None,
                mlx_top_p_execution: None,
                mlx_repeat_penalty_planning: None,
                mlx_repeat_penalty_execution: None,
                paged_attn: false,
                planning_enabled: true,
                lmstudio_url: None,
                pa_memory_mb: None,
                vram_time_sharing: false,
                ollama_remote: None,
                tool_engine: "legacy".to_string(),
            },
        )
        .await
        .unwrap();

        // 1. Verify defaults are None
        assert!(agent.temp_override.lock().is_none());
        assert!(agent.ctx_override.lock().is_none());
        assert!(agent.role_override.lock().is_none());

        // 2. Set overrides
        *agent.temp_override.lock() = Some(0.85);
        *agent.ctx_override.lock() = Some(8192);
        *agent.role_override.lock() = Some("security-auditor".to_string());

        // Verify values
        assert_eq!(*agent.temp_override.lock(), Some(0.85));
        assert_eq!(*agent.ctx_override.lock(), Some(8192));
        assert_eq!(
            agent.role_override.lock().as_deref(),
            Some("security-auditor")
        );

        // 3. Test that system prompt includes role information
        let system_prompt = agent.build_system_prompt(AgentPhase::Execution, "test-model");
        assert!(system_prompt.contains("ROLE: Security Auditor"));
    }
}
