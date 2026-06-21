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
    pub embedding_model: String,
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

#[derive(serde::Deserialize)]
pub struct GenericParams(pub Value);

impl schemars::JsonSchema for GenericParams {
    fn schema_name() -> std::borrow::Cow<'static, str> {
        std::borrow::Cow::Borrowed("GenericParams")
    }
    fn json_schema(_gen: &mut schemars::SchemaGenerator) -> schemars::Schema {
        true.into()
    }
}

#[macro_export]
macro_rules! define_ollama_tool_adapter {
    ($name:ident, $name_str:expr, $desc_str:expr) => {
        pub struct $name {
            pub tool: std::sync::Arc<dyn $crate::tools::AgentTool>,
            pub context: $crate::tools::ToolContext,
        }
        impl ollama_rs::generation::tools::Tool for $name {
            type Params = $crate::tools::GenericParams;
            fn name() -> &'static str {
                $name_str
            }
            fn description() -> &'static str {
                $desc_str
            }
            fn call(
                &mut self,
                parameters: Self::Params,
            ) -> impl std::future::Future<
                Output = std::result::Result<String, Box<dyn std::error::Error + Send + Sync>>,
            > + Send {
                let tool = self.tool.clone();
                let context = self.context.clone();
                async move {
                    tool.execute(&parameters.0, context)
                        .await
                        .map_err(|e| e.into())
                }
            }
        }
    };
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
