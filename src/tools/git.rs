use serde_json::{json, Value};
use anyhow::Result;
use async_trait::async_trait;
use super::{AgentTool, ToolContext};
use crate::tools::execution::RunCommandTool;
use schemars::JsonSchema;
use serde::Deserialize;
use ollama_rs::generation::tools::{ToolInfo, ToolFunctionInfo, ToolType};

#[derive(Deserialize, JsonSchema)]
pub struct GitStatusArgs {}

pub struct GitStatusTool;

#[async_trait]
impl AgentTool for GitStatusTool {
    fn name(&self) -> &'static str { "git_status" }
    fn description(&self) -> &'static str { "Lists all changed and untracked files in the current repository." }
    fn tool_info(&self) -> ToolInfo {
        let mut settings = schemars::generate::SchemaSettings::draft07();
        settings.inline_subschemas = true;
        let generator = settings.into_generator();
        let payload = generator.into_root_schema_for::<GitStatusArgs>();
        
        ToolInfo {
            tool_type: ToolType::Function,
            function: ToolFunctionInfo {
                name: self.name().to_string(),
                description: self.description().to_string(),
                parameters: payload.into(),
            }
        }
    }

    async fn execute(&self, _args: &Value, context: ToolContext) -> Result<String> {
        let _typed_args: GitStatusArgs = serde_json::from_value(_args.clone()).unwrap_or(GitStatusArgs {});
        let exec_args = json!({ "command": "git status -s" });
        let out = RunCommandTool.execute(&exec_args, context).await?;
        if out.contains("clean") || out.trim().is_empty() {
            Ok("No changes detected.".to_string())
        } else {
            Ok(out)
        }
    }
}

#[derive(Deserialize, JsonSchema)]
pub struct GitDiffArgs {
    /// Optional path to a specific file to diff.
    pub path: Option<String>,
}

pub struct GitDiffTool;

#[async_trait]
impl AgentTool for GitDiffTool {
    fn name(&self) -> &'static str { "git_diff" }
    fn description(&self) -> &'static str { "Shows changes for a specific file or the entire repository." }
    fn tool_info(&self) -> ToolInfo {
        let mut settings = schemars::generate::SchemaSettings::draft07();
        settings.inline_subschemas = true;
        let generator = settings.into_generator();
        let payload = generator.into_root_schema_for::<GitDiffArgs>();
        
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
        let typed_args: GitDiffArgs = serde_json::from_value(args.clone())?;
        let path = typed_args.path.unwrap_or_else(String::new);
        let cmd = format!("git diff {}", path);
        let exec_args = json!({ "command": cmd });
        RunCommandTool.execute(&exec_args, context).await
    }
}

#[derive(Deserialize, JsonSchema)]
pub struct GitCommitArgs {
    /// Commit message
    pub message: String,
}

pub struct GitCommitTool;

#[async_trait]
impl AgentTool for GitCommitTool {
    fn name(&self) -> &'static str { "git_commit" }
    fn description(&self) -> &'static str { "Stages and commits changes with a given message." }
    fn requires_confirmation(&self) -> bool { true }
    fn is_modifying(&self) -> bool { true }
    fn tool_info(&self) -> ToolInfo {
        let mut settings = schemars::generate::SchemaSettings::draft07();
        settings.inline_subschemas = true;
        let generator = settings.into_generator();
        let payload = generator.into_root_schema_for::<GitCommitArgs>();
        
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
        let typed_args: GitCommitArgs = serde_json::from_value(args.clone())?;
        let message = typed_args.message;
        
        let add_args = json!({ "command": "git add ." });
        RunCommandTool.execute(&add_args, context.clone()).await?;

        let commit_args = json!({ "command": format!("git commit -m \"{}\"", message) });
        RunCommandTool.execute(&commit_args, context).await
    }
}
#[derive(Deserialize, JsonSchema)]
pub struct GitActionArgs {
    /// Array of string arguments for git.
    pub args: Vec<String>,
}

pub struct GitActionTool;

#[async_trait]
impl AgentTool for GitActionTool {
    fn name(&self) -> &'static str { "git_action" }
    fn description(&self) -> &'static str { "Natively executes a secure 'git' command. Provide arguments as an array of strings (e.g., ['push', 'origin', 'main'])." }
    fn is_modifying(&self) -> bool { true }
    fn tool_info(&self) -> ToolInfo {
        let mut settings = schemars::generate::SchemaSettings::draft07();
        settings.inline_subschemas = true;
        let generator = settings.into_generator();
        let payload = generator.into_root_schema_for::<GitActionArgs>();
        
        ToolInfo {
            tool_type: ToolType::Function,
            function: ToolFunctionInfo {
                name: self.name().to_string(),
                description: self.description().to_string(),
                parameters: payload.into(),
            }
        }
    }

    async fn execute(&self, json_args: &Value, _context: ToolContext) -> Result<String> {
        let typed_args: GitActionArgs = serde_json::from_value(json_args.clone())?;
        let string_args = typed_args.args;

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
