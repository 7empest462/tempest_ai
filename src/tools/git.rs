use serde_json::{json, Value};
use miette::{Result, IntoDiagnostic};
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
        let payload = settings.into_generator().into_root_schema_for::<GitStatusArgs>();
        
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
        let payload = settings.into_generator().into_root_schema_for::<GitDiffArgs>();
        
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
        let typed_args: GitDiffArgs = serde_json::from_value(args.clone()).into_diagnostic()?;
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
    fn is_modifying(&self) -> bool { true }
    fn tool_info(&self) -> ToolInfo {
        let mut settings = schemars::generate::SchemaSettings::draft07();
        settings.inline_subschemas = true;
        let payload = settings.into_generator().into_root_schema_for::<GitCommitArgs>();
        
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
        let typed_args: GitCommitArgs = serde_json::from_value(args.clone()).into_diagnostic()?;
        let message = typed_args.message;
        
        let add_args = json!({ "command": "git add .", "timeout_seconds": 30 });
        RunCommandTool.execute(&add_args, context.clone()).await?;

        // Use shell escaping or array-based approach via git_action logic if possible
        // For now, we'll use a safer format or just use GitActionTool logic
        let commit_args = json!({ "args": ["commit", "-m", &message] });
        GitActionTool.execute(&commit_args, context).await
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
        let payload = settings.into_generator().into_root_schema_for::<GitActionArgs>();
        
        ToolInfo {
            tool_type: ToolType::Function,
            function: ToolFunctionInfo {
                name: self.name().to_string(),
                description: self.description().to_string(),
                parameters: payload.into(),
            }
        }
    }

    async fn execute(&self, json_args: &Value, context: ToolContext) -> Result<String> {
        let typed_args: GitActionArgs = serde_json::from_value(json_args.clone()).into_diagnostic()?;
        let string_args = typed_args.args;
        
        // Escape args for shell execution
        let safe_cmd = format!("git {}", string_args.iter()
            .map(|a| format!("'{}'", a.replace("'", "'\\''")))
            .collect::<Vec<_>>().join(" "));

        let exec_args = json!({ "command": safe_cmd, "timeout_seconds": 60 });
        RunCommandTool.execute(&exec_args, context).await
    }
}
