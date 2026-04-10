use serde_json::{json, Value};
use miette::{Result, IntoDiagnostic, miette};
use std::process::Stdio;
use tokio::process::Command;
use tokio::time::{timeout, Duration};
use async_trait::async_trait;
use super::{AgentTool, ToolContext};
use schemars::JsonSchema;
use serde::Deserialize;
use ollama_rs::generation::tools::{ToolInfo, ToolFunctionInfo, ToolType};
use crate::error::ExecutionError;

#[derive(Deserialize, JsonSchema)]
pub struct RunCommandArgs {
    /// The command string to execute.
    pub command: String,
    /// Working directory (default '.')
    pub cwd: Option<String>,
    /// Timeout in seconds (default 30)
    pub timeout_seconds: Option<u64>,
}

pub struct RunCommandTool;

#[async_trait]
impl AgentTool for RunCommandTool {
    fn name(&self) -> &'static str { "run_command" }
    fn description(&self) -> &'static str { "Executes a shell command. Features safety timeout and output capture." }
    fn is_modifying(&self) -> bool { true }
    fn tool_info(&self) -> ToolInfo {
        let mut settings = schemars::generate::SchemaSettings::draft07();
        settings.inline_subschemas = true;
        let generator = settings.into_generator();
        let payload = generator.into_root_schema_for::<RunCommandArgs>();
        
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
        let typed_args: RunCommandArgs = serde_json::from_value(args.clone()).into_diagnostic()?;
        let cmd_str = typed_args.command;
        let cwd = typed_args.cwd.unwrap_or_else(|| ".".to_string());
        let timeout_secs = typed_args.timeout_seconds.unwrap_or(30);

        use std::sync::atomic::Ordering;
        let is_elevated = context.is_root.load(Ordering::SeqCst);
        let final_cmd = if is_elevated && !cmd_str.starts_with("sudo ") {
            format!("sudo -n {}", cmd_str)
        } else {
            cmd_str.clone()
        };

        let child = Command::new("sh")
            .arg("-c")
            .arg(&final_cmd)
            .current_dir(shellexpand::tilde(&cwd).to_string())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| ExecutionError::CommandFailed { command: final_cmd.clone(), message: e.to_string() })?;

        let res = timeout(Duration::from_secs(timeout_secs), child.wait_with_output()).await;
        
        match res {
            Ok(Ok(output)) => {
                let stdout = String::from_utf8_lossy(&output.stdout);
                let stderr = String::from_utf8_lossy(&output.stderr);
                let status = output.status;
                
                let mut full_output = format!("Exit Status: {}\n", status);
                if !stdout.is_empty() {
                    full_output.push_str(&format!("--- STDOUT ---\n{}\n", stdout));
                }
                if !stderr.is_empty() {
                    full_output.push_str(&format!("--- STDERR ---\n{}\n", stderr));
                }
                
                if full_output.len() > 10000 {
                    full_output.truncate(10000);
                    full_output.push_str("\n...[output truncated]...");
                }
                Ok(full_output)
            }
            Ok(Err(e)) => Err(ExecutionError::CommandFailed { command: cmd_str, message: e.to_string() }.into()),
            Err(_) => {
                Err(ExecutionError::Timeout { command: cmd_str }.into())
            }
        }
    }
}

#[derive(Deserialize, JsonSchema)]
pub struct RunTestsArgs {
    /// Optional filter for specific tests.
    pub filter: Option<String>,
}

pub struct RunTestsTool;

#[async_trait]
impl AgentTool for RunTestsTool {
    fn name(&self) -> &'static str { "run_tests" }
    fn description(&self) -> &'static str { "Runs project tests. Detects language and runs appropriate test command (e.g., cargo test, npm test)." }
    fn tool_info(&self) -> ToolInfo {
        let mut settings = schemars::generate::SchemaSettings::draft07();
        settings.inline_subschemas = true;
        let generator = settings.into_generator();
        let payload = generator.into_root_schema_for::<RunTestsArgs>();
        
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
        let typed_args: RunTestsArgs = serde_json::from_value(args.clone()).into_diagnostic()?;
        let filter = typed_args.filter.unwrap_or_else(String::new);
        
        let cmd = if std::path::Path::new("Cargo.toml").exists() {
            format!("cargo test {} -- --nocapture", filter)
        } else if std::path::Path::new("package.json").exists() {
            format!("npm test -- {}", filter)
        } else if std::path::Path::new("pytest.ini").exists() || std::path::Path::new("tests").exists() {
            format!("pytest {}", filter)
        } else {
            return Err(miette!("No supported test suite detected."));
        };

        let exec_args = json!({ "command": cmd, "timeout_seconds": 300 });
        RunCommandTool.execute(&exec_args, context).await
    }
}

#[derive(Deserialize, JsonSchema)]
pub struct BuildProjectArgs {}

pub struct BuildProjectTool;

#[async_trait]
impl AgentTool for BuildProjectTool {
    fn name(&self) -> &'static str { "build_project" }
    fn description(&self) -> &'static str { "Builds the current project using the detected build system." }
    fn tool_info(&self) -> ToolInfo {
        let mut settings = schemars::generate::SchemaSettings::draft07();
        settings.inline_subschemas = true;
        let generator = settings.into_generator();
        let payload = generator.into_root_schema_for::<BuildProjectArgs>();
        
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
        let cmd = if std::path::Path::new("Cargo.toml").exists() {
            "cargo build"
        } else if std::path::Path::new("package.json").exists() {
            "npm run build"
        } else if std::path::Path::new("Makefile").exists() {
            "make"
        } else {
            return Err(miette!("No supported build system detected."));
        };

        let exec_args = json!({ "command": cmd, "timeout_seconds": 600 });
        RunCommandTool.execute(&exec_args, context).await
    }
}
