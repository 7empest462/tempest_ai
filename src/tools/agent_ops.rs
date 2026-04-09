use serde_json::Value;
use colored::Colorize;
use miette::{Result, IntoDiagnostic, miette};
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
        let payload = settings.into_generator().into_root_schema_for::<AskUserArgs>();
        
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
        let typed_args: AskUserArgs = serde_json::from_value(args.clone()).into_diagnostic()?;
        let question = typed_args.question;
        
        // 🚀 TUI HANDOFF
        if context.tx.send(crate::tui::AgentEvent::RequestInput(self.name().to_string(), question.to_string())).await.is_ok() {
            let mut rx_lock = context.tool_rx.lock().await;
            match rx_lock.recv().await {
                Some(crate::tui::ToolResponse::Text(ans)) => return Ok(format!("User responded: {}", ans)),
                _ => return Err(miette!("User cancelled input.")),
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
            Err(miette!("Failed to read input from terminal."))
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
        let payload = settings.into_generator().into_root_schema_for::<SpawnSubAgentArgs>();
        
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
        let typed_args: SpawnSubAgentArgs = serde_json::from_value(args.clone()).into_diagnostic()?;
        let task = typed_args.task;
        let model = typed_args.model.unwrap_or_else(|| context.sub_agent_model.clone());
        
        let sub_history = vec![
            ChatMessage::new(MessageRole::System, "You are a specialized Disciplined Sub-Agent. \
                 Perform the focused mission described below. \
                 1. NO HALLUCINATION: Be honest if you find no data. \
                 2. NO PREAMBLE: Output your report directly without 'Sure' or 'Here is'. \
                 3. CONCISE: Provide critical details first.".to_string()),
            ChatMessage::new(MessageRole::User, task.to_string()),
        ];
        
        let req = ChatMessageRequest::new(model, sub_history);
        match context.ollama.send_chat_messages(req).await {
            Ok(res) => Ok(format!("[SUB-AGENT REPORT]: {}", res.message.content)),
            Err(e) => Err(miette!("Sub-agent error: {}", e)),
        }
    }
}

pub struct TogglePlanningTool;

#[async_trait]
impl AgentTool for TogglePlanningTool {
    fn name(&self) -> &'static str { "toggle_planning" }
    fn description(&self) -> &'static str { "The Master Switch. Sets/Toggles between PLANNING and EXECUTION mode. You MUST set this to 'active: false' to enter EXECUTION mode before any state-modifying tools (write_file, run_command, etc.) will work." }
    fn tool_info(&self) -> ToolInfo {
        let mut settings = schemars::generate::SchemaSettings::draft07();
        settings.inline_subschemas = true;
        let payload = settings.into_generator().into_root_schema_for::<TogglePlanningArgs>();
        
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
        let mut planning_lock = context.planning_mode.lock();
        
        if let Some(active) = typed_args.active {
            *planning_lock = active;
        } else {
            *planning_lock = !*planning_lock;
        }
        
        let mode = if *planning_lock { "PLANNING" } else { "EXECUTION" };
        Ok(format!("Agent is now in {} mode.", mode))
    }
}

pub struct UpdateTaskContextTool;

#[async_trait]
impl AgentTool for UpdateTaskContextTool {
    fn name(&self) -> &'static str { "update_task_context" }
    fn description(&self) -> &'static str { "Updates the agent's internal reflective memory (sketchpad) to track progress across multiple turns." }
    fn tool_info(&self) -> ToolInfo {
        let mut settings = schemars::generate::SchemaSettings::draft07();
        settings.inline_subschemas = true;
        let payload = settings.into_generator().into_root_schema_for::<UpdateTaskContextArgs>();
        
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
        let typed_args: UpdateTaskContextArgs = serde_json::from_value(args.clone()).into_diagnostic()?;
        let new_ctx = typed_args.context;
        let mut ctx_lock = context.task_context.lock();
        *ctx_lock = new_ctx.to_string();
        Ok("Task context successfully updated.".to_string())
    }
}

#[derive(Deserialize, JsonSchema)]
pub struct TogglePlanningArgs {
    /// Explicitly set the mode. true for PLANNING, false for EXECUTION. If omitted, toggles current state.
    pub active: Option<bool>,
}

#[derive(Deserialize, JsonSchema)]
pub struct UpdateTaskContextArgs {
    /// The updated status or plan.
    pub context: String,
}
