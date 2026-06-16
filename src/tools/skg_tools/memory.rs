// ==========================================
// 🧠 SKG MEMORY TOOLS — Native Skelegent Implementations
// ==========================================
// Replaces the legacy AgentTool memory tools.

use skg_tool::{ToolCallContext, ToolError};
use skg_tool_macro::skg_tool;

// ── store_memory ───────────────────────────────────────────────────────────────

#[skg_tool(
    name = "store_memory",
    description = "Stores an important fact in long-term memory for later recall across sessions."
)]
pub async fn store_memory(
    fact: String,
    tags: Option<Vec<String>>,
    ctx: &ToolCallContext,
) -> Result<serde_json::Value, ToolError> {
    let tool_ctx = ctx.deps::<std::sync::Arc<crate::tools::ToolContext>>()
        .ok_or_else(|| ToolError::ExecutionFailed("Missing ToolContext dependency".to_string()))?;
        
    let memory_store = tool_ctx.memory_store.clone();
    
    memory_store.lock().store(&fact, &fact, tags)
        .map_err(|e| ToolError::ExecutionFailed(format!("Failed to store memory: {}", e)))?;
        
    Ok(serde_json::Value::String("Fact stored successfully in long-term memory.".to_string()))
}

// ── recall_memory ──────────────────────────────────────────────────────────────

#[skg_tool(
    name = "recall_memory",
    description = "Searches long-term memory for relevant facts based on keywords."
)]
pub async fn recall_memory(
    query: String,
    ctx: &ToolCallContext,
) -> Result<serde_json::Value, ToolError> {
    let tool_ctx = ctx.deps::<std::sync::Arc<crate::tools::ToolContext>>()
        .ok_or_else(|| ToolError::ExecutionFailed("Missing ToolContext dependency".to_string()))?;
        
    let memory_store = tool_ctx.memory_store.clone();
    
    let memories = memory_store.lock().recall(&query)
        .map_err(|e| ToolError::ExecutionFailed(format!("Failed to recall memory: {}", e)))?;
        
    if memories.is_empty() {
        Ok(serde_json::Value::String("No matching memories found.".to_string()))
    } else {
        let mut out = String::from("Relevant memories discovered:\n");
        for (i, (topic, content)) in memories.iter().enumerate() {
            out.push_str(&format!("{}. [Topic: {}] {}\n", i + 1, topic, content));
        }
        Ok(serde_json::Value::String(out))
    }
}

// ── memory_search ──────────────────────────────────────────────────────────────

#[skg_tool(
    name = "memory_search",
    description = "Performs a global search across all stored memories and facts."
)]
pub async fn memory_search(
    query: String,
    ctx: &ToolCallContext,
) -> Result<serde_json::Value, ToolError> {
    let tool_ctx = ctx.deps::<std::sync::Arc<crate::tools::ToolContext>>()
        .ok_or_else(|| ToolError::ExecutionFailed("Missing ToolContext dependency".to_string()))?;
        
    let memory_store = tool_ctx.memory_store.clone();
    
    let memories = memory_store.lock().recall(&query)
        .map_err(|e| ToolError::ExecutionFailed(format!("Failed to search memory: {}", e)))?;
        
    if memories.is_empty() {
        Ok(serde_json::Value::String("No matching memories found in search.".to_string()))
    } else {
        let mut out = String::from("Global memory search results:\n");
        for (i, (topic, content)) in memories.iter().enumerate() {
            out.push_str(&format!("{}. [Topic: {}] {}\n", i + 1, topic, content));
        }
        Ok(serde_json::Value::String(out))
    }
}
