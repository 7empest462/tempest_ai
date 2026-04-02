use serde_json::{json, Value};
use anyhow::Result;
use async_trait::async_trait;
use super::{AgentTool, ToolContext};
use std::sync::{Arc, Mutex};
use crate::memory::MemoryStore;

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
    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "fact": { "type": "string", "description": "The information to remember." },
                "tags": { "type": "array", "items": { "type": "string" }, "description": "Optional search tags." }
            },
            "required": ["fact"]
        })
    }

    async fn execute(&self, args: &Value, _context: ToolContext) -> Result<String> {
        let fact = args.get("fact").and_then(|f| f.as_str()).unwrap();
        
        self.memory_store.lock().expect("Memory Store Poisoned").store(fact, fact)?;
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
    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "query": { "type": "string", "description": "Search keywords or query." }
            },
            "required": ["query"]
        })
    }

    async fn execute(&self, args: &Value, _context: ToolContext) -> Result<String> {
        let query = args.get("query").and_then(|q| q.as_str()).unwrap();
        let memories = self.memory_store.lock().expect("Memory Store Poisoned").recall(query)?;
        
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
