use serde_json::Value;
use miette::Result;
use std::sync::Arc;
use parking_lot::Mutex;
use ollama_rs::Ollama;
use ollama_rs::generation::chat::ChatMessage;
use std::sync::atomic::AtomicBool;

/// The context passed to every tool, providing safe, thread-safe access to agent state.
#[derive(Clone)]
pub struct ToolContext {
    pub ollama: Ollama,
    #[allow(dead_code)] pub model: String,
    pub sub_agent_model: String,
    #[allow(dead_code)] pub history: Arc<Mutex<Vec<ChatMessage>>>,
    pub planning_mode: Arc<Mutex<bool>>,
    pub task_context: Arc<Mutex<String>>,
    pub vector_brain: Arc<Mutex<crate::vector_brain::VectorBrain>>,
    #[allow(dead_code)] pub telemetry: Arc<Mutex<String>>,
    pub tx: tokio::sync::mpsc::Sender<crate::tui::AgentEvent>,
    pub tool_rx: Arc<tokio::sync::Mutex<tokio::sync::mpsc::Receiver<crate::tui::ToolResponse>>>,
    #[allow(dead_code)] pub recent_tool_calls: Arc<dashmap::DashMap<String, String>>,
    #[allow(dead_code)] pub brain_path: std::path::PathBuf,
    pub is_root: Arc<AtomicBool>,
    pub all_tools: Vec<ToolInfo>,
}

use ollama_rs::generation::tools::ToolInfo;

/// A trait representing an autonomous tool the agent can use in its plugin-like system.
#[async_trait::async_trait]
pub trait AgentTool: Send + Sync {
    fn name(&self) -> &'static str;
    fn description(&self) -> &'static str;
    
    /// Provides the exact native ToolInfo expected by ollama-rs 0.3.4 Native Tool Calling.
    fn tool_info(&self) -> ToolInfo;

    async fn execute(&self, args: &Value, context: ToolContext) -> Result<String>;
    
    fn is_modifying(&self) -> bool { false }
}

pub mod file;
pub mod execution;
pub mod git;
pub mod search;
pub mod editing;
pub mod system;
pub mod memory;
pub mod agent_ops;
pub mod web;
pub mod process;
pub mod utilities;
pub mod knowledge;
pub mod database;
pub mod network;
pub mod atlas;
pub mod telemetry;
pub mod network_manager;
pub mod service_manager;
pub mod developer;
pub mod privilege;
