// ==========================================
// ⚡ SKG EXECUTION TOOLS — Native Skelegent Implementations
// ==========================================
// Replaces the legacy RunCommandTool, RunTestsTool, and BuildProjectTool.

use skg_tool::{ToolCallContext, ToolError};
use skg_tool_macro::skg_tool;
use std::process::Stdio;
use tokio::process::Command;
use tokio::time::{Duration, timeout};

// ── run_command ────────────────────────────────────────────────────────────────

#[skg_tool(
    name = "run_command",
    description = "Executes a shell command. Features safety timeout and output capture."
)]
pub async fn run_command(
    command: String,
    cwd: Option<String>,
    timeout_seconds: Option<u64>,
    ctx: &ToolCallContext,
) -> Result<serde_json::Value, ToolError> {
    let mut working_dir = cwd.unwrap_or_else(|| ".".to_string());
    if working_dir.is_empty() {
        working_dir = ".".to_string();
    }
    let timeout_secs = timeout_seconds.unwrap_or(30);

    // Check for elevated privileges via ToolContext deps injection
    let is_elevated =
        if let Some(tool_ctx) = ctx.deps::<std::sync::Arc<crate::tools::ToolContext>>() {
            tool_ctx.is_root.load(std::sync::atomic::Ordering::SeqCst)
        } else {
            false
        };

    let final_cmd = if is_elevated && !command.starts_with("sudo ") {
        format!("sudo -n {}", command)
    } else {
        command.clone()
    };

    let mut child_cmd = if cfg!(target_os = "windows") {
        let mut c = Command::new("cmd");
        c.arg("/C").arg(&final_cmd);
        c
    } else {
        let mut c = Command::new("sh");
        c.arg("-c").arg(&final_cmd);
        c
    };

    let child = child_cmd
        .current_dir(shellexpand::tilde(&working_dir).to_string())
        .env("TERM", "dumb")
        .env("DEBIAN_FRONTEND", "noninteractive")
        .env("GIT_EDITOR", "true")
        .env("PAGER", "cat")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .map_err(|e| {
            ToolError::ExecutionFailed(format!("Failed to spawn command '{}': {}", final_cmd, e))
        })?;

    let res = timeout(Duration::from_secs(timeout_secs), child.wait_with_output()).await;

    match res {
        Ok(Ok(output)) => {
            let stdout = String::from_utf8_lossy(&output.stdout);
            let stderr = String::from_utf8_lossy(&output.stderr);
            let status = output.status;

            let mut combined = String::new();
            if !stdout.is_empty() {
                combined.push_str(&format!("--- STDOUT ---\n{}\n", stdout));
            }
            if !stderr.is_empty() {
                combined.push_str(&format!("--- STDERR ---\n{}\n", stderr));
            }

            let mut full_output = format!("Exit Status: {}\n{}", status, combined);

            if full_output.len() > 10000 {
                let head = &full_output[..2000];
                let tail = &full_output[full_output.len() - 8000..];
                full_output = format!(
                    "{}\n\n...[OUTPUT TRUNCATED - Showing first 2k and last 8k bytes]...\n\n{}",
                    head, tail
                );
            }
            Ok(serde_json::Value::String(full_output))
        }
        Ok(Err(e)) => Err(ToolError::ExecutionFailed(format!(
            "Command '{}' failed: {}",
            command, e
        ))),
        Err(_) => Err(ToolError::ExecutionFailed(format!(
            "Command '{}' timed out after {} seconds",
            command, timeout_secs
        ))),
    }
}

// ── run_tests ──────────────────────────────────────────────────────────────────

#[skg_tool(
    name = "run_tests",
    description = "Runs project tests. Detects language and runs appropriate test command (e.g., cargo test, npm test)."
)]
pub async fn run_tests(
    filter: Option<String>,
    ctx: &ToolCallContext,
) -> Result<serde_json::Value, ToolError> {
    let filter = filter.unwrap_or_default();

    let cmd = if std::path::Path::new("Cargo.toml").exists() {
        format!("cargo test {} -- --nocapture", filter)
    } else if std::path::Path::new("package.json").exists() {
        format!("npm test -- {}", filter)
    } else if std::path::Path::new("pytest.ini").exists() || std::path::Path::new("tests").exists()
    {
        format!("pytest {}", filter)
    } else {
        return Err(ToolError::ExecutionFailed(
            "No supported test suite detected.".to_string(),
        ));
    };

    run_command(cmd, None, Some(300), ctx).await
}

// ── build_project ──────────────────────────────────────────────────────────────

#[skg_tool(
    name = "build_project",
    description = "Builds the current project using the detected build system."
)]
pub async fn build_project(ctx: &ToolCallContext) -> Result<serde_json::Value, ToolError> {
    let cmd = if std::path::Path::new("Cargo.toml").exists() {
        "cargo build"
    } else if std::path::Path::new("package.json").exists() {
        "npm run build"
    } else if std::path::Path::new("Makefile").exists() {
        "make"
    } else {
        return Err(ToolError::ExecutionFailed(
            "No supported build system detected.".to_string(),
        ));
    };

    run_command(cmd.to_string(), None, Some(600), ctx).await
}
