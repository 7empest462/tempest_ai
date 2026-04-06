use serde_json::Value;
use anyhow::Result;
use std::sync::{Arc, Mutex};
use ollama_rs::Ollama;
use ollama_rs::generation::chat::ChatMessage;

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
    #[allow(dead_code)] pub recent_tool_calls: Arc<Mutex<std::collections::VecDeque<String>>>,
    pub brain_path: std::path::PathBuf,
}

use ollama_rs::generation::tools::{ToolInfo, ToolFunctionInfo, ToolType};
use schemars::Schema;

/// A trait representing an autonomous tool the agent can use in its plugin-like system.
#[async_trait::async_trait]
pub trait AgentTool: Send + Sync {
    fn name(&self) -> &'static str;
    fn description(&self) -> &'static str;
    
    /// LEGACY: Used by tools that have not yet migrated to typed schemas.
    fn parameters(&self) -> Value {
        serde_json::json!({})
    }

    /// NEW: Provides the exact native ToolInfo expected by ollama-rs 0.3.4 Native Tool Calling.
    /// Default implementation automatically converts old JSON literal schemas into native types.
    fn tool_info(&self) -> ToolInfo {
        let param_value = self.parameters();
        let parameters = serde_json::from_value::<Schema>(param_value)
            .unwrap_or_else(|_| serde_json::from_str("{}").unwrap());
        
        ToolInfo {
            tool_type: ToolType::Function,
            function: ToolFunctionInfo {
                name: self.name().to_string(),
                description: self.description().to_string(),
                parameters,
            }
        }
    }

    async fn execute(&self, args: &Value, context: ToolContext) -> Result<String>;
    
    fn requires_confirmation(&self) -> bool { false }
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
