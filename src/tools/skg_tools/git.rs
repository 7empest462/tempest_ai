// ==========================================
// 🌿 SKG GIT TOOLS — Native Skelegent Implementations
// ==========================================
// Replaces the legacy AgentTool git tools.

use skg_tool::{ToolCallContext, ToolError};
use skg_tool_macro::skg_tool;
use super::execution::run_command;

// ── git_status ─────────────────────────────────────────────────────────────────

#[skg_tool(
    name = "git_status",
    description = "Lists all changed and untracked files in the current repository."
)]
pub async fn git_status(
    ctx: &ToolCallContext,
) -> Result<serde_json::Value, ToolError> {
    let out = run_command("git status -s".to_string(), None, Some(30), ctx).await?;
    
    if let Some(out_str) = out.as_str()
        && (out_str.contains("clean") || out_str.trim().is_empty())
    {
        return Ok(serde_json::Value::String("No changes detected.".to_string()));
    }
    Ok(out)
}

// ── git_diff ───────────────────────────────────────────────────────────────────

#[skg_tool(
    name = "git_diff",
    description = "Shows changes for a specific file or the entire repository."
)]
pub async fn git_diff(
    path: Option<String>,
    ctx: &ToolCallContext,
) -> Result<serde_json::Value, ToolError> {
    let path_val = path.unwrap_or_default();
    let cmd = format!("git diff {}", path_val);
    run_command(cmd, None, Some(30), ctx).await
}

// ── git_commit ─────────────────────────────────────────────────────────────────

#[skg_tool(
    name = "git_commit",
    description = "Stages and commits changes with a given message."
)]
pub async fn git_commit(
    message: String,
    ctx: &ToolCallContext,
) -> Result<serde_json::Value, ToolError> {
    // Stage all changes
    let _ = run_command("git add .".to_string(), None, Some(30), ctx).await?;
    
    // Escape single quotes for shell execution
    let safe_message = message.replace("'", "'\\''");
    let cmd = format!("git commit -m '{}'", safe_message);
    
    run_command(cmd, None, Some(30), ctx).await
}

// ── git_action ─────────────────────────────────────────────────────────────────

#[skg_tool(
    name = "git_action",
    description = "Natively executes a secure 'git' command. Provide arguments as an array of strings (e.g., ['push', 'origin', 'main'])."
)]
pub async fn git_action(
    args: Vec<String>,
    ctx: &ToolCallContext,
) -> Result<serde_json::Value, ToolError> {
    let safe_cmd = format!(
        "git {}",
        args.iter()
            .map(|a| format!("'{}'", a.replace("'", "'\\''")))
            .collect::<Vec<_>>()
            .join(" ")
    );

    run_command(safe_cmd, None, Some(60), ctx).await
}
