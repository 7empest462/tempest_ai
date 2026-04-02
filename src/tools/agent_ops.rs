use serde_json::{json, Value};
use anyhow::Result;
use async_trait::async_trait;
use super::{AgentTool, ToolContext};
use ollama_rs::generation::chat::{request::ChatMessageRequest, ChatMessage, MessageRole};

pub struct AskUserTool;

#[async_trait]
impl AgentTool for AskUserTool {
    fn name(&self) -> &'static str { "ask_user" }
    fn description(&self) -> &'static str { "Stops execution to ask the user a question. Use this for clarifying ambiguous tasks." }
    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "question": { "type": "string", "description": "The question to ask the user." }
            },
            "required": ["question"]
        })
    }

    async fn execute(&self, args: &Value, context: ToolContext) -> Result<String> {
        let question = args.get("question").and_then(|q| q.as_str()).unwrap_or("(No question)");
        
        let _ = context.tx.send(crate::tui::AgentEvent::RequestInput(self.name().to_string(), question.to_string())).await;
        
        let mut rx_lock = context.tool_rx.lock().await;
        match rx_lock.recv().await {
            Some(crate::tui::ToolResponse::Text(ans)) => Ok(format!("User responded: {}", ans)),
            _ => anyhow::bail!("User cancelled input."),
        }
    }
}

pub struct SpawnSubAgentTool;

#[async_trait]
impl AgentTool for SpawnSubAgentTool {
    fn name(&self) -> &'static str { "spawn_sub_agent" }
    fn description(&self) -> &'static str { "Spawns a specialized sub-agent for a localized task. Best for research or isolated debugging." }
    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "task": { "type": "string", "description": "The specific mission for the sub-agent." },
                "model": { "type": "string", "description": "Optional model name to override." }
            },
            "required": ["task"]
        })
    }

    async fn execute(&self, args: &Value, context: ToolContext) -> Result<String> {
        let task = args.get("task").and_then(|t| t.as_str()).unwrap();
        let model = args.get("model").and_then(|m| m.as_str()).map(|s| s.to_string()).unwrap_or_else(|| context.sub_agent_model.clone());
        
        let sub_history = vec![
            ChatMessage::new(MessageRole::System, "You are a specialized Sub-Agent. Perform the task and provide a CONCISE summary.".to_string()),
            ChatMessage::new(MessageRole::User, task.to_string()),
        ];
        
        let req = ChatMessageRequest::new(model, sub_history);
        match context.ollama.send_chat_messages(req).await {
            Ok(res) => Ok(format!("[SUB-AGENT REPORT]: {}", res.message.content)),
            Err(e) => anyhow::bail!("Sub-agent error: {}", e),
        }
    }
}

pub struct TogglePlanningTool;

#[async_trait]
impl AgentTool for TogglePlanningTool {
    fn name(&self) -> &'static str { "toggle_planning" }
    fn description(&self) -> &'static str { "Toggles between PLANNING and EXECUTION mode. Use PLANNING for high-level architectural proposals." }
    fn parameters(&self) -> Value { json!({}) }

    async fn execute(&self, _args: &Value, context: ToolContext) -> Result<String> {
        let mut planning_lock = context.planning_mode.lock().unwrap();
        *planning_lock = !*planning_lock;
        let mode = if *planning_lock { "PLANNING" } else { "EXECUTION" };
        Ok(format!("Agent is now in {} mode.", mode))
    }
}

pub struct UpdateTaskContextTool;

#[async_trait]
impl AgentTool for UpdateTaskContextTool {
    fn name(&self) -> &'static str { "update_task_context" }
    fn description(&self) -> &'static str { "Updates the agent's internal reflective memory (sketchpad) to track progress across multiple turns." }
    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "context": { "type": "string", "description": "The updated status or plan." }
            },
            "required": ["context"]
        })
    }

    async fn execute(&self, args: &Value, context: ToolContext) -> Result<String> {
        let new_ctx = args.get("context").and_then(|c| c.as_str()).unwrap_or("");
        let mut ctx_lock = context.task_context.lock().unwrap();
        *ctx_lock = new_ctx.to_string();
        Ok("Task context successfully updated.".to_string())
    }
}
pub struct ExtractAndWriteTool;

#[async_trait]
impl AgentTool for ExtractAndWriteTool {
    fn name(&self) -> &'static str { "extract_and_write" }
    fn description(&self) -> &'static str { "Extracts the latest markdown code block from your thought process and writes it to a file. MUST wrap your code in triple backticks BEFORE calling this tool." }
    fn is_modifying(&self) -> bool { true }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": { "type": "string", "description": "The path to the file to create or overwrite." }
            },
            "required": ["path"]
        })
    }

    async fn execute(&self, args: &Value, context: ToolContext) -> Result<String> {
        let path_str = args.get("path").and_then(|p| p.as_str()).ok_or_else(|| anyhow::anyhow!("Missing 'path'"))?;
        let path_owned = shellexpand::tilde(path_str).to_string();

        let history = context.history.lock().unwrap();
        let last_assistant_msg = history.iter().rev()
            .find(|m| m.role == MessageRole::Assistant)
            .ok_or_else(|| anyhow::anyhow!("No assistant message found in history."))?;

        let content = &last_assistant_msg.content;
        let blocks: Vec<&str> = content.split("```").collect();
        
        // Code blocks are at odd indices
        let mut code_block = "";
        for i in (1..blocks.len()).step_by(2).rev() {
            let b = blocks[i].trim();
            // Skip the tool call block itself (which is JSON)
            if !b.to_lowercase().starts_with("json") {
                code_block = blocks[i];
                break;
            }
        }

        if code_block.is_empty() {
             // Fallback: take the last block if we didn't find a non-json one
             if blocks.len() >= 2 {
                 code_block = blocks[blocks.len() - 2];
             } else {
                 anyhow::bail!("No markdown code block found in your previous response.");
             }
        }

        // Clean up the language tag if present
        let clean_code = if let Some(first_newline) = code_block.find('\n') {
            let first_line = &code_block[0..first_newline];
            if !first_line.contains(' ') && !first_line.is_empty() {
                &code_block[first_newline + 1..]
            } else {
                code_block
            }
        } else {
            code_block
        };

        let path = std::path::Path::new(&path_owned);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(path, clean_code)?;

        Ok(format!("Successfully extracted and wrote {} bytes to {}", clean_code.len(), path_owned))
    }
}
