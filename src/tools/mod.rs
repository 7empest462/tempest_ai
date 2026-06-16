use miette::Result;
use ollama_rs::Ollama;
use ollama_rs::generation::chat::ChatMessage;
use parking_lot::Mutex;
use serde_json::Value;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

/// The context passed to every tool, providing safe, thread-safe access to agent state.
#[derive(Clone)]
pub struct ToolContext {
    pub ollama: Ollama,
    pub backend: Arc<tokio::sync::RwLock<crate::inference::Backend>>,
    #[allow(dead_code)]
    pub model: String,
    pub sub_agent_model: String,
    #[allow(dead_code)]
    pub history: Arc<Mutex<Vec<ChatMessage>>>,

    pub task_context: Arc<Mutex<String>>,
    pub vector_brain: Arc<Mutex<crate::vector_brain::VectorBrain>>,
    #[allow(dead_code)]
    pub telemetry: Arc<Mutex<String>>,
    pub tx: Option<tokio::sync::mpsc::Sender<crate::tui::AgentEvent>>,
    pub tool_rx:
        Arc<tokio::sync::Mutex<Option<tokio::sync::mpsc::Receiver<crate::tui::ToolResponse>>>>,
    #[allow(dead_code)]
    pub recent_tool_calls: Arc<dashmap::DashMap<String, String>>,
    #[allow(dead_code)]
    pub brain_path: std::path::PathBuf,
    pub is_root: Arc<AtomicBool>,
    pub all_tools: Vec<ToolInfo>,
    #[allow(dead_code)]
    pub checkpoint_mgr: crate::checkpoint::SharedCheckpointManager,
    pub memory_store: Arc<Mutex<crate::memory::MemoryStore>>,
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

    fn is_modifying(&self) -> bool {
        false
    }

    /// Returns an optional preview of the intended action (e.g. a diff) to show during approval.
    async fn get_approval_preview(&self, _args: &Value) -> Option<String> {
        None
    }
}

pub mod agent_ops;
pub mod ast;
pub mod atlas;
pub mod csv;
pub mod database;
pub mod developer;
pub mod editing;
pub mod execution;
pub mod file;
pub mod git;
pub mod knowledge;
pub mod memory;
pub mod network;
pub mod network_manager;
pub mod privilege;
pub mod process;
pub mod rust;
pub mod search;
pub mod service_manager;
pub mod skg_adapter;
pub mod system;
pub mod telemetry;
pub mod terminal;
pub mod threat_scanner;
pub mod utilities;
pub mod visualization;
pub mod wasm_sandbox;
pub mod web;

// Native Skelegent tool implementations (controlled by tool_engine config)
pub mod skg_tools;
