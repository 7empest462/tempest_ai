use super::{AgentTool, ToolContext};
use async_trait::async_trait;
use colored::Colorize;
use miette::{IntoDiagnostic, Result, miette};
use ollama_rs::generation::chat::{ChatMessage, MessageRole};
use ollama_rs::generation::tools::{ToolFunctionInfo, ToolInfo, ToolType};
use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::Value;

#[derive(Deserialize, JsonSchema)]
pub struct AskUserArgs {
    /// The question to ask the user.
    pub question: String,
}

pub struct AskUserTool;

#[async_trait]
impl AgentTool for AskUserTool {
    fn name(&self) -> &'static str {
        "ask_user"
    }
    fn description(&self) -> &'static str {
        "Stops execution to ask the user a question. Use this for clarifying ambiguous tasks."
    }
    fn tool_info(&self) -> ToolInfo {
        let mut settings = schemars::generate::SchemaSettings::draft07();
        settings.inline_subschemas = true;
        let payload = settings
            .into_generator()
            .into_root_schema_for::<AskUserArgs>();

        ToolInfo {
            tool_type: ToolType::Function,
            function: ToolFunctionInfo {
                name: self.name().to_string(),
                description: self.description().to_string(),
                parameters: payload,
            },
        }
    }

    async fn execute(&self, args: &Value, context: ToolContext) -> Result<String> {
        let typed_args: AskUserArgs = serde_json::from_value(args.clone()).into_diagnostic()?;
        let question = typed_args.question;

        // 🚀 TUI HANDOFF
        if let Some(ref tx) = context.tx
            && tx
                .send(crate::tui::AgentEvent::RequestInput(
                    self.name().to_string(),
                    question.to_string(),
                ))
                .await
                .is_ok()
        {
            let mut rx_lock = context.tool_rx.lock().await;
            if let Some(rx) = rx_lock.as_mut() {
                match rx.recv().await {
                    Some(crate::tui::ToolResponse::Text(ans)) => {
                        return Ok(format!("User responded: {}", ans));
                    }
                    _ => return Err(miette!("User cancelled input.")),
                }
            } else {
                return Err(miette!("No live TUI input channel configured."));
            }
        }

        // 🚑 CLI FALLBACK (if TUI is not running or channel is dead)
        use std::io::{self, Write};
        println!(
            "\n{} \n{}",
            "❓ Agent Question:".yellow().bold(),
            question.cyan()
        );
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
    fn name(&self) -> &'static str {
        "spawn_sub_agent"
    }
    fn description(&self) -> &'static str {
        "Spawns a specialized sub-agent for a localized task. Best for research or isolated debugging."
    }
    fn tool_info(&self) -> ToolInfo {
        let mut settings = schemars::generate::SchemaSettings::draft07();
        settings.inline_subschemas = true;
        let payload = settings
            .into_generator()
            .into_root_schema_for::<SpawnSubAgentArgs>();

        ToolInfo {
            tool_type: ToolType::Function,
            function: ToolFunctionInfo {
                name: self.name().to_string(),
                description: self.description().to_string(),
                parameters: payload,
            },
        }
    }

    async fn execute(&self, args: &Value, context: ToolContext) -> Result<String> {
        let typed_args: SpawnSubAgentArgs =
            serde_json::from_value(args.clone()).into_diagnostic()?;
        let task = typed_args.task;
        let model = typed_args
            .model
            .unwrap_or_else(|| context.sub_agent_model.clone());

        // Notify HUD
        if let Some(ref tx) = context.tx {
            let _ = tx
                .send(crate::tui::AgentEvent::SubagentStatus(Some(format!(
                    "Calling {} for assist:\n{}",
                    model, task
                ))))
                .await;
        }

        let sub_history = vec![
            ChatMessage::new(
                MessageRole::System,
                "You are a specialized Disciplined Sub-Agent. \
                 Perform the focused mission described below. \
                 You must communicate using the Agent Client Protocol (ACP). \
                 1. NO HALLUCINATION: Be honest if you find no data. \
                 2. NO PREAMBLE: Output your report directly without 'Sure' or 'Here is'. \
                 3. CONCISE: Provide critical details first."
                    .to_string(),
            ),
            ChatMessage::new(
                MessageRole::User,
                format!(
                    "{{\"jsonrpc\": \"2.0\", \"method\": \"prompt\", \"params\": {{\"task\": \"{}\"}}}}",
                    task
                ),
            ),
        ];

        let backend = context.backend.read().await.clone();
        let sampling = crate::inference::SamplingConfig {
            temperature: 0.1,
            top_p: 0.9,
            repeat_penalty: 1.1,
            context_size: 16384,
        };
        let stop = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let event_tx = std::sync::Arc::new(parking_lot::Mutex::new(None));

        let response = backend
            .stream_chat(crate::inference::ChatRequest {
                model,
                history: sub_history,
                sampling,
                event_tx,
                stop,
                system_prompt: "".to_string(),
                on_tool_call: None,
                tool_registry: None, // No tools for sub-agent yet to avoid recursive loop
            })
            .await;

        // Clear HUD
        if let Some(ref tx) = context.tx {
            let _ = tx.send(crate::tui::AgentEvent::SubagentStatus(None)).await;
        }

        match response {
            Ok(res) => Ok(format!("[SUB-AGENT ACP REPORT]: {}", res.content)),
            Err(e) => Err(miette!("Sub-agent error: {}", e)),
        }
    }
}

pub struct UpdateTaskContextTool;

#[async_trait]
impl AgentTool for UpdateTaskContextTool {
    fn name(&self) -> &'static str {
        "update_task_context"
    }
    fn description(&self) -> &'static str {
        "Updates the agent's internal reflective memory (sketchpad) to track progress across multiple turns."
    }
    fn tool_info(&self) -> ToolInfo {
        let mut settings = schemars::generate::SchemaSettings::draft07();
        settings.inline_subschemas = true;
        let payload = settings
            .into_generator()
            .into_root_schema_for::<UpdateTaskContextArgs>();

        ToolInfo {
            tool_type: ToolType::Function,
            function: ToolFunctionInfo {
                name: self.name().to_string(),
                description: self.description().to_string(),
                parameters: payload,
            },
        }
    }

    async fn execute(&self, args: &Value, context: ToolContext) -> Result<String> {
        let typed_args: UpdateTaskContextArgs =
            serde_json::from_value(args.clone()).into_diagnostic()?;
        let new_ctx = typed_args.context;
        {
            let mut ctx_lock = context.task_context.lock();
            *ctx_lock = new_ctx.to_string();
        }

        // Notify HUD/Web
        if let Some(ref tx) = context.tx {
            let _ = tx
                .send(crate::tui::AgentEvent::TaskUpdate(new_ctx.to_string()))
                .await;
        }

        Ok("Task context successfully updated.".to_string())
    }
}

#[derive(Deserialize, JsonSchema)]
pub struct UpdateTaskContextArgs {
    /// The updated status or plan.
    pub context: String,
}

#[derive(Deserialize, JsonSchema)]
pub struct QuerySchemaArgs {
    /// Optional specific tool name to get full details for. If omitted, returns a summary of all tools.
    pub tool_name: Option<String>,
}

pub struct QuerySchemaTool;

#[async_trait]
impl AgentTool for QuerySchemaTool {
    fn name(&self) -> &'static str {
        "query_schema"
    }
    fn description(&self) -> &'static str {
        "META-TOOL: Inspects the agent's current capabilities. Returns a list of all available tools and their descriptions. Use this if you are unsure what tools you have or if a tool call returns 'unknown'."
    }
    fn tool_info(&self) -> ToolInfo {
        let mut settings = schemars::generate::SchemaSettings::draft07();
        settings.inline_subschemas = true;
        let payload = settings
            .into_generator()
            .into_root_schema_for::<QuerySchemaArgs>();

        ToolInfo {
            tool_type: ToolType::Function,
            function: ToolFunctionInfo {
                name: self.name().to_string(),
                description: self.description().to_string(),
                parameters: payload,
            },
        }
    }

    async fn execute(&self, args: &Value, context: ToolContext) -> Result<String> {
        let typed_args: QuerySchemaArgs =
            serde_json::from_value(args.clone()).unwrap_or(QuerySchemaArgs { tool_name: None });

        if let Some(target) = typed_args.tool_name {
            if let Some(info) = context.all_tools.iter().find(|t| t.function.name == target) {
                return Ok(format!(
                    "Full Tool Schema for {}:\n{}",
                    target,
                    serde_json::to_string_pretty(info).unwrap_or_default()
                ));
            }
            return Err(miette!("Tool '{}' not found in current schema.", target));
        }

        let mut summary = "🌪️ TEMPEST INDUSTRIAL TOOLBOX SCHEMA:\n".to_string();
        for tool in &context.all_tools {
            summary.push_str(&format!(
                "- {}: {}\n",
                tool.function.name, tool.function.description
            ));
        }
        summary.push_str("\nUse query_schema(tool_name: \"name\") for detailed JSON parameters of a specific tool.");
        Ok(summary)
    }
}

#[derive(Deserialize, JsonSchema)]
pub struct NoOpArgs {}

pub struct NoOpTool;

#[async_trait]
impl AgentTool for NoOpTool {
    fn name(&self) -> &'static str {
        "no_op"
    }
    fn description(&self) -> &'static str {
        "Does nothing. Use this when you just want to think or continue planning without taking any action."
    }
    fn tool_info(&self) -> ollama_rs::generation::tools::ToolInfo {
        let mut settings = schemars::generate::SchemaSettings::draft07();
        settings.inline_subschemas = true;
        let generator = settings.into_generator();
        let payload = generator.into_root_schema_for::<NoOpArgs>(); // Use explicit empty struct for no arguments

        ollama_rs::generation::tools::ToolInfo {
            tool_type: ollama_rs::generation::tools::ToolType::Function,
            function: ollama_rs::generation::tools::ToolFunctionInfo {
                name: self.name().to_string(),
                description: self.description().to_string(),
                parameters: payload,
            },
        }
    }

    async fn execute(&self, _args: &Value, _context: ToolContext) -> Result<String> {
        Ok("No operation performed. Continuing in planning mode.".to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_noop_tool_schema() {
        let tool = NoOpTool;
        let info = tool.tool_info();
        let schema_val = serde_json::to_value(&info.function.parameters).unwrap();

        // Assert that the parameters schema is an object and contains type = "object"
        assert_eq!(
            schema_val.get("type").and_then(|v| v.as_str()),
            Some("object")
        );
    }
}
