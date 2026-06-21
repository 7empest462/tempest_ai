// ==========================================
// 🤖 SKG AGENT OPS TOOLS — Native Skelegent Implementations
// ==========================================
// Replaces the legacy AgentTool agent_ops tools.

use colored::Colorize;
use ollama_rs::generation::chat::{ChatMessage, MessageRole};
use skg_tool::{ToolCallContext, ToolError};
use skg_tool_macro::skg_tool;

// ── ask_user ───────────────────────────────────────────────────────────────────

#[skg_tool(
    name = "ask_user",
    description = "Stops execution to ask the user a question. Use this for clarifying ambiguous tasks."
)]
pub async fn ask_user(
    question: String,
    ctx: &ToolCallContext,
) -> Result<serde_json::Value, ToolError> {
    let tool_ctx = ctx
        .deps::<std::sync::Arc<crate::tools::ToolContext>>()
        .ok_or_else(|| ToolError::ExecutionFailed("Missing ToolContext dependency".to_string()))?;

    // 🚀 TUI HANDOFF
    if let Some(ref tx) = tool_ctx.tx
        && tx
            .send(crate::tui::AgentEvent::RequestInput(
                "ask_user".to_string(),
                question.to_string(),
            ))
            .await
            .is_ok()
    {
        let mut rx_lock = tool_ctx.tool_rx.lock().await;
        if let Some(rx) = rx_lock.as_mut() {
            match rx.recv().await {
                Some(crate::tui::ToolResponse::Text(ans)) => {
                    return Ok(serde_json::Value::String(format!(
                        "User responded: {}",
                        ans
                    )));
                }
                _ => {
                    return Err(ToolError::ExecutionFailed(
                        "User cancelled input.".to_string(),
                    ));
                }
            }
        } else {
            return Err(ToolError::ExecutionFailed(
                "No live TUI input channel configured.".to_string(),
            ));
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
            Ok(serde_json::Value::String(
                "User provided an empty response.".to_string(),
            ))
        } else {
            Ok(serde_json::Value::String(format!(
                "User responded: {}",
                ans
            )))
        }
    } else {
        Err(ToolError::ExecutionFailed(
            "Failed to read input from terminal.".to_string(),
        ))
    }
}

// ── spawn_sub_agent ────────────────────────────────────────────────────────────

#[skg_tool(
    name = "spawn_sub_agent",
    description = "Spawns a specialized sub-agent for a localized task. Best for research or isolated debugging."
)]
pub async fn spawn_sub_agent(
    task: String,
    model: Option<String>,
    ctx: &ToolCallContext,
) -> Result<serde_json::Value, ToolError> {
    let tool_ctx = ctx
        .deps::<std::sync::Arc<crate::tools::ToolContext>>()
        .ok_or_else(|| ToolError::ExecutionFailed("Missing ToolContext dependency".to_string()))?;

    let model = model.unwrap_or_else(|| tool_ctx.sub_agent_model.clone());

    // Notify HUD
    if let Some(ref tx) = tool_ctx.tx {
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

    let backend = tool_ctx.backend.read().await.clone();
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
    if let Some(ref tx) = tool_ctx.tx {
        let _ = tx.send(crate::tui::AgentEvent::SubagentStatus(None)).await;
    }

    match response {
        Ok(res) => Ok(serde_json::Value::String(format!(
            "[SUB-AGENT ACP REPORT]: {}",
            res.content
        ))),
        Err(e) => Err(ToolError::ExecutionFailed(format!(
            "Sub-agent error: {}",
            e
        ))),
    }
}

// ── update_task_context ────────────────────────────────────────────────────────

#[skg_tool(
    name = "update_task_context",
    description = "Updates the agent's internal reflective memory (sketchpad) to track progress across multiple turns."
)]
pub async fn update_task_context(
    context: String,
    ctx: &ToolCallContext,
) -> Result<serde_json::Value, ToolError> {
    let tool_ctx = ctx
        .deps::<std::sync::Arc<crate::tools::ToolContext>>()
        .ok_or_else(|| ToolError::ExecutionFailed("Missing ToolContext dependency".to_string()))?;

    {
        let mut ctx_lock = tool_ctx.task_context.lock();
        *ctx_lock = context.to_string();
    }

    // Notify HUD/Web
    if let Some(ref tx) = tool_ctx.tx {
        let _ = tx
            .send(crate::tui::AgentEvent::TaskUpdate(context.to_string()))
            .await;
    }

    Ok(serde_json::Value::String(
        "Task context successfully updated.".to_string(),
    ))
}

// ── query_schema ───────────────────────────────────────────────────────────────

#[skg_tool(
    name = "query_schema",
    description = "META-TOOL: Inspects the agent's current capabilities. Returns a list of all available tools and their descriptions. Use this if you are unsure what tools you have or if a tool call returns 'unknown'."
)]
pub async fn query_schema(
    tool_name: Option<String>,
    ctx: &ToolCallContext,
) -> Result<serde_json::Value, ToolError> {
    let tool_ctx = ctx
        .deps::<std::sync::Arc<crate::tools::ToolContext>>()
        .ok_or_else(|| ToolError::ExecutionFailed("Missing ToolContext dependency".to_string()))?;

    if let Some(target) = tool_name {
        if let Some(info) = tool_ctx
            .all_tools
            .iter()
            .find(|t| t.function.name == target)
        {
            return Ok(serde_json::Value::String(format!(
                "Full Tool Schema for {}:\n{}",
                target,
                serde_json::to_string_pretty(info).unwrap_or_default()
            )));
        }
        return Err(ToolError::ExecutionFailed(format!(
            "Tool '{}' not found in current schema.",
            target
        )));
    }

    let mut summary = "🌪️ TEMPEST INDUSTRIAL TOOLBOX SCHEMA:\n".to_string();
    for tool in &tool_ctx.all_tools {
        summary.push_str(&format!(
            "- {}: {}\n",
            tool.function.name, tool.function.description
        ));
    }
    summary.push_str(
        "\nUse query_schema(tool_name: \"name\") for detailed JSON parameters of a specific tool.",
    );
    Ok(serde_json::Value::String(summary))
}

// ── no_op ──────────────────────────────────────────────────────────────────────

#[skg_tool(
    name = "no_op",
    description = "Does nothing. Use this when you just want to think or continue planning without taking any action."
)]
pub async fn no_op() -> Result<serde_json::Value, ToolError> {
    Ok(serde_json::Value::String(
        "No operation performed. Continuing in planning mode.".to_string(),
    ))
}
