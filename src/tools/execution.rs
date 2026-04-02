use serde_json::{json, Value};
use anyhow::Result;
use std::process::Stdio;
use tokio::process::Command;
use tokio::time::{timeout, Duration};
use async_trait::async_trait;
use super::{AgentTool, ToolContext};

pub struct RunCommandTool;

#[async_trait]
impl AgentTool for RunCommandTool {
    fn name(&self) -> &'static str { "run_command" }
    fn description(&self) -> &'static str { "Executes a shell command. Features safety timeout and output capture." }
    fn requires_confirmation(&self) -> bool { true }
    fn is_modifying(&self) -> bool { true }
    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "command": { "type": "string", "description": "The command string to execute." },
                "cwd": { "type": "string", "description": "Working directory (default '.')" },
                "timeout_seconds": { "type": "integer", "description": "Timeout in seconds (default 30)" }
            },
            "required": ["command"]
        })
    }

    async fn execute(&self, args: &Value, _context: ToolContext) -> Result<String> {
        let cmd_str = args.get("command").and_then(|c| c.as_str()).ok_or_else(|| anyhow::anyhow!("Missing 'command'"))?;
        let cwd = args.get("cwd").and_then(|c| c.as_str()).unwrap_or(".");
        let timeout_secs = args.get("timeout_seconds").and_then(|t| t.as_u64()).unwrap_or(30);

        let child = Command::new("sh")
            .arg("-c")
            .arg(cmd_str)
            .current_dir(shellexpand::tilde(cwd).to_string())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?;

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
            Ok(Err(e)) => anyhow::bail!("Command error: {}", e),
            Err(_) => {
                Ok(format!("Error: Command timed out after {}s.", timeout_secs))
            }
        }
    }
}

pub struct RunTestsTool;

#[async_trait]
impl AgentTool for RunTestsTool {
    fn name(&self) -> &'static str { "run_tests" }
    fn description(&self) -> &'static str { "Runs project tests. Detects language and runs appropriate test command (e.g., cargo test, npm test)." }
    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "filter": { "type": "string", "description": "Optional filter for specific tests." }
            }
        })
    }

    async fn execute(&self, args: &Value, context: ToolContext) -> Result<String> {
        let filter = args.get("filter").and_then(|f| f.as_str()).unwrap_or("");
        
        let cmd = if std::path::Path::new("Cargo.toml").exists() {
            format!("cargo test {} -- --nocapture", filter)
        } else if std::path::Path::new("package.json").exists() {
            format!("npm test -- {}", filter)
        } else if std::path::Path::new("pytest.ini").exists() || std::path::Path::new("tests").exists() {
            format!("pytest {}", filter)
        } else {
            anyhow::bail!("No supported test suite detected.");
        };

        let exec_args = json!({ "command": cmd, "timeout_seconds": 300 });
        RunCommandTool.execute(&exec_args, context).await
    }
}

pub struct BuildProjectTool;

#[async_trait]
impl AgentTool for BuildProjectTool {
    fn name(&self) -> &'static str { "build_project" }
    fn description(&self) -> &'static str { "Builds the current project using the detected build system." }
    fn parameters(&self) -> Value { json!({}) }

    async fn execute(&self, _args: &Value, context: ToolContext) -> Result<String> {
        let cmd = if std::path::Path::new("Cargo.toml").exists() {
            "cargo build"
        } else if std::path::Path::new("package.json").exists() {
            "npm run build"
        } else if std::path::Path::new("Makefile").exists() {
            "make"
        } else {
            anyhow::bail!("No supported build system detected.");
        };

        let exec_args = json!({ "command": cmd, "timeout_seconds": 600 });
        RunCommandTool.execute(&exec_args, context).await
    }
}
