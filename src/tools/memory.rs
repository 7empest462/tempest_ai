use serde_json::Value;
use anyhow::Result;
use async_trait::async_trait;
use super::{AgentTool, ToolContext};
use std::sync::{Arc, Mutex};
use crate::memory::MemoryStore;
use schemars::JsonSchema;
use serde::Deserialize;
use ollama_rs::generation::tools::{ToolInfo, ToolFunctionInfo, ToolType};

#[derive(Deserialize, JsonSchema)]
pub struct StoreMemoryArgs {
    /// The information to remember.
    pub fact: String,
    /// Optional search tags.
    pub tags: Option<Vec<String>>,
}

#[derive(Deserialize, JsonSchema)]
pub struct RecallMemoryArgs {
    /// Search keywords or query.
    pub query: String,
}

pub struct StoreMemoryTool {
    memory_store: Arc<Mutex<MemoryStore>>,
}

impl StoreMemoryTool {
    pub fn new(memory_store: Arc<Mutex<MemoryStore>>) -> Self { Self { memory_store } }
}

#[async_trait]
impl AgentTool for StoreMemoryTool {
    fn name(&self) -> &'static str { "store_memory" }
    fn description(&self) -> &'static str { "Stores an important fact in long-term memory for later recall across sessions." }
    fn tool_info(&self) -> ToolInfo {
        let mut settings = schemars::generate::SchemaSettings::draft07();
        settings.inline_subschemas = true;
        let generator = settings.into_generator();
        let payload = generator.into_root_schema_for::<StoreMemoryArgs>();
        
        ToolInfo {
            tool_type: ToolType::Function,
            function: ToolFunctionInfo {
                name: self.name().to_string(),
                description: self.description().to_string(),
                parameters: payload.into(),
            }
        }
    }

    async fn execute(&self, args: &Value, _context: ToolContext) -> Result<String> {
        let typed_args: StoreMemoryArgs = serde_json::from_value(args.clone())?;
        
        self.memory_store.lock().expect("Memory Store Poisoned").store(&typed_args.fact, &typed_args.fact, typed_args.tags)?;
        Ok("Fact stored successfully in long-term memory.".to_string())
    }
}

pub struct RecallMemoryTool {
    memory_store: Arc<Mutex<MemoryStore>>,
}

impl RecallMemoryTool {
    pub fn new(memory_store: Arc<Mutex<MemoryStore>>) -> Self { Self { memory_store } }
}

#[async_trait]
impl AgentTool for RecallMemoryTool {
    fn name(&self) -> &'static str { "recall_memory" }
    fn description(&self) -> &'static str { "Searches long-term memory for relevant facts based on keywords." }
    fn tool_info(&self) -> ToolInfo {
        let mut settings = schemars::generate::SchemaSettings::draft07();
        settings.inline_subschemas = true;
        let generator = settings.into_generator();
        let payload = generator.into_root_schema_for::<RecallMemoryArgs>();
        
        ToolInfo {
            tool_type: ToolType::Function,
            function: ToolFunctionInfo {
                name: self.name().to_string(),
                description: self.description().to_string(),
                parameters: payload.into(),
            }
        }
    }

    async fn execute(&self, args: &Value, _context: ToolContext) -> Result<String> {
        let typed_args: RecallMemoryArgs = serde_json::from_value(args.clone())?;
        let memories = self.memory_store.lock().expect("Memory Store Poisoned").recall(&typed_args.query)?;
        
        if memories.is_empty() {
            Ok("No matching memories found.".to_string())
        } else {
            let mut out = String::from("Relevant memories discovered:\n");
            for (i, (topic, content)) in memories.iter().enumerate() {
                out.push_str(&format!("{}. [Topic: {}] {}\n", i + 1, topic, content));
            }
            Ok(out)
        }
    }
}
