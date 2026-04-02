use serde_json::{json, Value};
use anyhow::Result;
use async_trait::async_trait;
use super::{AgentTool, ToolContext};
use crate::tools::execution::RunCommandTool;

pub struct GitStatusTool;

#[async_trait]
impl AgentTool for GitStatusTool {
    fn name(&self) -> &'static str { "git_status" }
    fn description(&self) -> &'static str { "Lists all changed and untracked files in the current repository." }
    fn parameters(&self) -> Value { json!({}) }

    async fn execute(&self, _args: &Value, context: ToolContext) -> Result<String> {
        let exec_args = json!({ "command": "git status -s" });
        let out = RunCommandTool.execute(&exec_args, context).await?;
        if out.contains("clean") || out.trim().is_empty() {
            Ok("No changes detected.".to_string())
        } else {
            Ok(out)
        }
    }
}

pub struct GitDiffTool;

#[async_trait]
impl AgentTool for GitDiffTool {
    fn name(&self) -> &'static str { "git_diff" }
    fn description(&self) -> &'static str { "Shows changes for a specific file or the entire repository." }
    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": { "type": "string", "description": "Optional path to a specific file to diff." }
            }
        })
    }

    async fn execute(&self, args: &Value, context: ToolContext) -> Result<String> {
        let path = args.get("path").and_then(|p| p.as_str()).unwrap_or("");
        let cmd = format!("git diff {}", path);
        let exec_args = json!({ "command": cmd });
        RunCommandTool.execute(&exec_args, context).await
    }
}

pub struct GitCommitTool;

#[async_trait]
impl AgentTool for GitCommitTool {
    fn name(&self) -> &'static str { "git_commit" }
    fn description(&self) -> &'static str { "Stages and commits changes with a given message." }
    fn requires_confirmation(&self) -> bool { true }
    fn is_modifying(&self) -> bool { true }
    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "message": { "type": "string", "description": "Commit message" }
            },
            "required": ["message"]
        })
    }

    async fn execute(&self, args: &Value, context: ToolContext) -> Result<String> {
        let message = args.get("message").and_then(|m| m.as_str()).ok_or_else(|| anyhow::anyhow!("Missing message"))?;
        
        let add_args = json!({ "command": "git add ." });
        RunCommandTool.execute(&add_args, context.clone()).await?;

        let commit_args = json!({ "command": format!("git commit -m \"{}\"", message) });
        RunCommandTool.execute(&commit_args, context).await
    }
}
pub struct GitActionTool;

#[async_trait]
impl AgentTool for GitActionTool {
    fn name(&self) -> &'static str { "git_action" }
    fn description(&self) -> &'static str { "Natively executes a secure 'git' command. Provide arguments as an array of strings (e.g., ['push', 'origin', 'main'])." }
    fn is_modifying(&self) -> bool { true }
    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "args": { "type": "array", "items": { "type": "string" }, "description": "Array of string arguments for git." }
            },
            "required": ["args"]
        })
    }

    async fn execute(&self, json_args: &Value, _context: ToolContext) -> Result<String> {
        let raw_args = json_args.get("args").and_then(|a| a.as_array()).ok_or_else(|| anyhow::anyhow!("Missing 'args'"))?;
        let string_args: Vec<String> = raw_args.iter().filter_map(|v| v.as_str().map(|s| s.to_string())).collect();

        let output = std::process::Command::new("git")
            .args(&string_args)
            .output()?;

        let mut result = String::from_utf8_lossy(&output.stdout).to_string();
        let err_result = String::from_utf8_lossy(&output.stderr).to_string();
        
        if !err_result.is_empty() {
            result.push_str("\n--- STDERR ---\n");
            result.push_str(&err_result);
        }

        if !output.status.success() {
            anyhow::bail!("Git command failed with status {}:\n{}", output.status, result);
        }

        Ok(result)
    }
}
