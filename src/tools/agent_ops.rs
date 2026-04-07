use serde_json::Value;
use colored::Colorize;
use anyhow::Result;
use async_trait::async_trait;
use super::{AgentTool, ToolContext};
use ollama_rs::generation::chat::{request::ChatMessageRequest, ChatMessage, MessageRole};
use schemars::JsonSchema;
use serde::Deserialize;
use ollama_rs::generation::tools::{ToolInfo, ToolFunctionInfo, ToolType};

#[derive(Deserialize, JsonSchema)]
pub struct AskUserArgs {
    /// The question to ask the user.
    pub question: String,
}

pub struct AskUserTool;

#[async_trait]
impl AgentTool for AskUserTool {
    fn name(&self) -> &'static str { "ask_user" }
    fn description(&self) -> &'static str { "Stops execution to ask the user a question. Use this for clarifying ambiguous tasks." }
    fn tool_info(&self) -> ToolInfo {
        let mut settings = schemars::generate::SchemaSettings::draft07();
        settings.inline_subschemas = true;
        let generator = settings.into_generator();
        let payload = generator.into_root_schema_for::<AskUserArgs>();
        
        ToolInfo {
            tool_type: ToolType::Function,
            function: ToolFunctionInfo {
                name: self.name().to_string(),
                description: self.description().to_string(),
                parameters: payload.into(),
            }
        }
    }

    async fn execute(&self, args: &Value, context: ToolContext) -> Result<String> {
        let typed_args: AskUserArgs = serde_json::from_value(args.clone())?;
        let question = typed_args.question;
        
        // 🚀 TUI HANDOFF
        if context.tx.send(crate::tui::AgentEvent::RequestInput(self.name().to_string(), question.to_string())).await.is_ok() {
            let mut rx_lock = context.tool_rx.lock().await;
            match rx_lock.recv().await {
                Some(crate::tui::ToolResponse::Text(ans)) => return Ok(format!("User responded: {}", ans)),
                _ => anyhow::bail!("User cancelled input."),
            }
        }

        // 🚑 CLI FALLBACK (if TUI is not running or channel is dead)
        use std::io::{self, Write};
        println!("\n{} \n{}", "❓ Agent Question:".yellow().bold(), question.cyan());
        print!(">> ");
        let _ = io::stdout().flush();
        
        let mut input = String::new();
        if io::stdin().read_line(&mut input).is_ok() {
            let ans = input.trim().to_string();
            if ans.is_empty() {
                Ok("User provided an empty response.".to_string())
            } else {
                Ok(format!("User responded: {}", ans))
            }
        } else {
            anyhow::bail!("Failed to read input from terminal.")
        }
    }
}

#[derive(Deserialize, JsonSchema)]
pub struct SpawnSubAgentArgs {
    /// The specific mission for the sub-agent.
    pub task: String,
    /// Optional model name to override.
    pub model: Option<String>,
}

pub struct SpawnSubAgentTool;

#[async_trait]
impl AgentTool for SpawnSubAgentTool {
    fn name(&self) -> &'static str { "spawn_sub_agent" }
    fn description(&self) -> &'static str { "Spawns a specialized sub-agent for a localized task. Best for research or isolated debugging." }
    fn tool_info(&self) -> ToolInfo {
        let mut settings = schemars::generate::SchemaSettings::draft07();
        settings.inline_subschemas = true;
        let generator = settings.into_generator();
        let payload = generator.into_root_schema_for::<SpawnSubAgentArgs>();
        
        ToolInfo {
            tool_type: ToolType::Function,
            function: ToolFunctionInfo {
                name: self.name().to_string(),
                description: self.description().to_string(),
                parameters: payload.into(),
            }
        }
    }

    async fn execute(&self, args: &Value, context: ToolContext) -> Result<String> {
        let typed_args: SpawnSubAgentArgs = serde_json::from_value(args.clone())?;
        let task = typed_args.task;
        let model = typed_args.model.unwrap_or_else(|| context.sub_agent_model.clone());
        
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

#[derive(Deserialize, JsonSchema)]
pub struct TogglePlanningArgs {
    /// Explicitly set the mode. true for PLANNING, false for EXECUTION. If omitted, toggles current state.
    pub active: Option<bool>,
}

pub struct TogglePlanningTool;

#[async_trait]
impl AgentTool for TogglePlanningTool {
    fn name(&self) -> &'static str { "toggle_planning" }
    fn description(&self) -> &'static str { "The Master Switch. Sets/Toggles between PLANNING and EXECUTION mode. You MUST set this to 'active: false' to enter EXECUTION mode before any state-modifying tools (write_file, run_command, etc.) will work." }
    fn tool_info(&self) -> ToolInfo {
        let mut settings = schemars::generate::SchemaSettings::draft07();
        settings.inline_subschemas = true;
        let generator = settings.into_generator();
        let payload = generator.into_root_schema_for::<TogglePlanningArgs>();
        
        ToolInfo {
            tool_type: ToolType::Function,
            function: ToolFunctionInfo {
                name: self.name().to_string(),
                description: self.description().to_string(),
                parameters: payload.into(),
            }
        }
    }

    async fn execute(&self, args: &Value, context: ToolContext) -> Result<String> {
        let typed_args: TogglePlanningArgs = serde_json::from_value(args.clone()).unwrap_or(TogglePlanningArgs { active: None });
        let mut planning_lock = context.planning_mode.lock().unwrap();
        
        if let Some(active) = typed_args.active {
            *planning_lock = active;
        } else {
            *planning_lock = !*planning_lock;
        }
        
        let mode = if *planning_lock { "PLANNING" } else { "EXECUTION" };
        Ok(format!("Agent is now in {} mode.", mode))
    }
}

#[derive(Deserialize, JsonSchema)]
pub struct UpdateTaskContextArgs {
    /// The updated status or plan.
    pub context: String,
}

pub struct UpdateTaskContextTool;

#[async_trait]
impl AgentTool for UpdateTaskContextTool {
    fn name(&self) -> &'static str { "update_task_context" }
    fn description(&self) -> &'static str { "Updates the agent's internal reflective memory (sketchpad) to track progress across multiple turns." }
    fn tool_info(&self) -> ToolInfo {
        let mut settings = schemars::generate::SchemaSettings::draft07();
        settings.inline_subschemas = true;
        let generator = settings.into_generator();
        let payload = generator.into_root_schema_for::<UpdateTaskContextArgs>();
        
        ToolInfo {
            tool_type: ToolType::Function,
            function: ToolFunctionInfo {
                name: self.name().to_string(),
                description: self.description().to_string(),
                parameters: payload.into(),
            }
        }
    }

    async fn execute(&self, args: &Value, context: ToolContext) -> Result<String> {
        let typed_args: UpdateTaskContextArgs = serde_json::from_value(args.clone())?;
        let new_ctx = typed_args.context;
        let mut ctx_lock = context.task_context.lock().unwrap();
        *ctx_lock = new_ctx.to_string();
        Ok("Task context successfully updated.".to_string())
    }
}
#[derive(Deserialize, JsonSchema)]
#[allow(dead_code)]
pub struct ExtractAndWriteArgs {
    /// The path to the file to create or overwrite.
    pub path: String,
}

#[allow(dead_code)]
pub struct ExtractAndWriteTool;

#[async_trait]
impl AgentTool for ExtractAndWriteTool {
    fn name(&self) -> &'static str { "extract_and_write" }
    fn description(&self) -> &'static str { "Extracts the latest markdown code block from your thought process and writes it to a file. MUST wrap your code in triple backticks BEFORE calling this tool." }
    fn is_modifying(&self) -> bool { true }

    fn tool_info(&self) -> ToolInfo {
        let mut settings = schemars::generate::SchemaSettings::draft07();
        settings.inline_subschemas = true;
        let generator = settings.into_generator();
        let payload = generator.into_root_schema_for::<ExtractAndWriteArgs>();
        
        ToolInfo {
            tool_type: ToolType::Function,
            function: ToolFunctionInfo {
                name: self.name().to_string(),
                description: self.description().to_string(),
                parameters: payload.into(),
            }
        }
    }

    async fn execute(&self, args: &Value, context: ToolContext) -> Result<String> {
        let typed_args: ExtractAndWriteArgs = serde_json::from_value(args.clone())?;
        let path_owned = shellexpand::tilde(&typed_args.path).to_string();

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
