// ==========================================
// 🦀 SKG RUST TOOLS — Native Skelegent Implementations
// ==========================================
// Replaces the legacy AgentTool rust tools.

use skg_tool::ToolError;
use skg_tool_macro::skg_tool;
use tokio::process::Command;

// ── cargo_add ──────────────────────────────────────────────────────────────────

#[skg_tool(
    name = "cargo_add",
    description = "Helps with Rust and Cargo. Use this tool to add dependencies with 'cargo add'. ALWAYS use cargo_search first to verify the crate and get the correct version. Never guess crate versions."
)]
pub async fn cargo_add(
    crate_name: String,
    version: Option<String>,
    features: Option<Vec<String>>,
    dev: Option<bool>,
    cwd: Option<String>,
) -> Result<serde_json::Value, ToolError> {
    let mut cmd = Command::new("cargo");
    cmd.arg("add");

    if let Some(v) = version {
        cmd.arg(format!("{}@{}", crate_name, v));
    } else {
        cmd.arg(&crate_name);
    }

    if let Some(feats) = features
        && !feats.is_empty()
    {
        cmd.arg("--features").arg(feats.join(","));
    }

    if dev.unwrap_or(false) {
        cmd.arg("--dev");
    }

    if let Some(dir) = cwd {
        let path = std::path::Path::new(&dir);
        if !path.exists() {
            return Err(ToolError::ExecutionFailed(format!(
                "The directory '{}' does not exist. Please provide a valid path.",
                dir
            )));
        }
        cmd.current_dir(dir);
    }

    let output_future = cmd.output();
    let timeout_duration = std::time::Duration::from_secs(15);
    let output = match tokio::time::timeout(timeout_duration, output_future).await {
        Ok(Ok(out)) => out,
        Ok(Err(e)) => return Err(ToolError::ExecutionFailed(format!("Failed to execute cargo add: {}", e))),
        Err(_) => return Err(ToolError::ExecutionFailed("Cargo add timed out after 15 seconds. This can happen if another cargo process holds the index/package lock.".to_string())),
    };

    if output.status.success() {
        Ok(serde_json::Value::String(format!(
            "Successfully added '{}' to dependencies.\n{}",
            crate_name,
            String::from_utf8_lossy(&output.stderr)
        )))
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let error_msg = if stderr.contains("could not be found in registry index") {
            format!(
                "⚠️ SYSTEM DIRECTIVE: The version or crate name '{}' was not found. YOU JUST HALLUCINATED. Stop immediately. Use 'cargo_search' or 'search_web' to find the CORRECT name and version before retrying.",
                crate_name
            )
        } else {
            format!("Failed to add dependency: {}", stderr)
        };
        Err(ToolError::ExecutionFailed(error_msg))
    }
}

// ── cargo_search ───────────────────────────────────────────────────────────────

#[skg_tool(
    name = "cargo_search",
    description = "Helps with Rust and Cargo. Use this tool to search for crates on crates.io, get the latest version of any crate. ALWAYS use this tool before suggesting any crate name or version in your response. Never guess crate versions."
)]
pub async fn cargo_search(
    query: String,
    cwd: Option<String>,
) -> Result<serde_json::Value, ToolError> {
    let mut cmd = Command::new("cargo");
    cmd.arg("search")
        .arg(&query)
        .arg("--limit")
        .arg("10");

    if let Some(dir) = cwd {
        let path = std::path::Path::new(&dir);
        if !path.exists() {
            return Err(ToolError::ExecutionFailed(format!(
                "The directory '{}' does not exist. Please provide a valid path.",
                dir
            )));
        }
        cmd.current_dir(dir);
    }

    let output_future = cmd.output();
    let timeout_duration = std::time::Duration::from_secs(15);
    let output = match tokio::time::timeout(timeout_duration, output_future).await {
        Ok(Ok(out)) => out,
        Ok(Err(e)) => return Err(ToolError::ExecutionFailed(format!("Failed to execute cargo search: {}", e))),
        Err(_) => return Err(ToolError::ExecutionFailed("Cargo search timed out after 15 seconds. This can happen if another cargo process holds the index lock or there is no network connection.".to_string())),
    };

    if output.status.success() {
        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        if stdout.trim().is_empty() {
            Ok(serde_json::Value::String("No crates found matching your query. Check spelling or try a broader search. You may also use 'search_web' to find the crate on crates.io if cargo search fails.".to_string()))
        } else {
            Ok(serde_json::Value::String(stdout))
        }
    } else {
        Err(ToolError::ExecutionFailed(format!(
            "Cargo search failed: {}",
            String::from_utf8_lossy(&output.stderr)
        )))
    }
}
