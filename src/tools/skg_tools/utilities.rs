// ==========================================
// 🛠️ SKG UTILITIES TOOLS — Native Skelegent Implementations
// ==========================================
// Replaces the legacy AgentTool utilities tools.

use skg_tool::ToolError;
use skg_tool_macro::skg_tool;
use std::process::Command;

// ── clipboard ──────────────────────────────────────────────────────────────────

#[skg_tool(
    name = "clipboard",
    description = "Read from or write to the system clipboard. Use 'read' to get clipboard contents, or 'write' to copy text to the clipboard so the user can paste it."
)]
pub async fn clipboard(
    action: String,
    content: Option<String>,
) -> Result<serde_json::Value, ToolError> {
    match action.as_str() {
        "write" => {
            let text = content
                .ok_or_else(|| ToolError::ExecutionFailed("Missing 'content' for clipboard write".to_string()))?;
            let mut clip = arboard::Clipboard::new()
                .map_err(|e| ToolError::ExecutionFailed(format!("Failed to access clipboard: {}", e)))?;
            clip.set_text(text.to_string())
                .map_err(|e| ToolError::ExecutionFailed(format!("Failed to write to clipboard: {}", e)))?;
            Ok(serde_json::Value::String(format!(
                "✅ Copied {} characters to clipboard.",
                text.len()
            )))
        }
        "read" => {
            let mut clip = arboard::Clipboard::new()
                .map_err(|e| ToolError::ExecutionFailed(format!("Failed to access clipboard: {}", e)))?;
            let text = clip.get_text()
                .map_err(|e| ToolError::ExecutionFailed(format!("Failed to read clipboard: {}", e)))?;
            Ok(serde_json::Value::String(format!("Clipboard contents:\n{}", text)))
        }
        _ => Err(ToolError::ExecutionFailed("Unknown clipboard action. Use 'read' or 'write'.".to_string())),
    }
}

// ── notify ─────────────────────────────────────────────────────────────────────

#[skg_tool(
    name = "notify",
    description = "Sends a native macOS/Linux desktop notification. Use this to alert the user when a long-running task completes."
)]
pub async fn notify(
    title: String,
    message: String,
) -> Result<serde_json::Value, ToolError> {
    #[cfg(target_os = "macos")]
    {
        let script = format!(
            "display notification \"{}\" with title \"{}\" sound name \"Glass\"",
            message.replace('"', "\\\""),
            title.replace('"', "\\\"")
        );
        let output = Command::new("osascript")
            .arg("-e")
            .arg(&script)
            .output()
            .map_err(|e| ToolError::ExecutionFailed(format!("Failed to execute osascript: {}", e)))?;
        if output.status.success() {
            Ok(serde_json::Value::String(format!("🔔 Notification sent: {} — {}", title, message)))
        } else {
            let err = String::from_utf8_lossy(&output.stderr);
            Err(ToolError::ExecutionFailed(format!("Failed to send notification: {}", err)))
        }
    }

    #[cfg(target_os = "linux")]
    {
        let output = Command::new("notify-send")
            .arg(&title)
            .arg(&message)
            .output();
        match output {
            Ok(o) if o.status.success() => {
                Ok(serde_json::Value::String(format!("🔔 Notification sent: {} — {}", title, message)))
            }
            Ok(o) => {
                let err = String::from_utf8_lossy(&o.stderr);
                Err(ToolError::ExecutionFailed(format!(
                    "Failed to send notification (is libnotify installed?): {}",
                    err
                )))
            }
            Err(_) => Err(ToolError::ExecutionFailed(
                "notify-send not found. Install with: sudo apt install libnotify-bin".to_string()
            )),
        }
    }

    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    {
        let _ = title;
        let _ = message;
        Ok(serde_json::Value::String("Notification not supported on this platform.".to_string()))
    }
}

// ── env_var ────────────────────────────────────────────────────────────────────

#[skg_tool(
    name = "env_var",
    description = "Read environment variables. Use 'get' to read a specific variable or 'list' to show all exported variables."
)]
pub async fn env_var(
    action: String,
    name: Option<String>,
) -> Result<serde_json::Value, ToolError> {
    match action.as_str() {
        "get" => {
            let var_name = name
                .ok_or_else(|| ToolError::ExecutionFailed("Missing 'name' for env get".to_string()))?;
            match std::env::var(&var_name) {
                Ok(val) => Ok(serde_json::Value::String(format!("{}={}", var_name, val))),
                Err(_) => Ok(serde_json::Value::String(format!("Variable '{}' is not set.", var_name))),
            }
        }
        "list" => {
            let vars: Vec<String> = std::env::vars()
                .take(50)
                .map(|(k, v)| {
                    let truncated = if v.len() > 100 {
                        format!("{}...", &v[..100])
                    } else {
                        v
                    };
                    format!("{}={}", k, truncated)
                })
                .collect();
            Ok(serde_json::Value::String(format!(
                "Environment variables (first 50):\n{}",
                vars.join("\n")
            )))
        }
        _ => Err(ToolError::ExecutionFailed("Unknown env_var action. Use 'get' or 'list'.".to_string())),
    }
}

// ── chmod ──────────────────────────────────────────────────────────────────────

#[skg_tool(
    name = "chmod",
    description = "Change file or directory permissions using standard Unix mode strings (e.g., '755', '644', '+x')."
)]
pub async fn chmod(
    path: String,
    mode: String,
) -> Result<serde_json::Value, ToolError> {
    let path_expanded = shellexpand::tilde(&path).to_string();

    let output = Command::new("chmod")
        .arg(&mode)
        .arg(&path_expanded)
        .output()
        .map_err(|e| ToolError::ExecutionFailed(format!("Failed to run chmod: {}", e)))?;

    if output.status.success() {
        Ok(serde_json::Value::String(format!(
            "✅ Changed permissions of '{}' to '{}'",
            path_expanded, mode
        )))
    } else {
        let err = String::from_utf8_lossy(&output.stderr);
        Err(ToolError::ExecutionFailed(format!("chmod failed: {}", err.trim())))
    }
}

// ── calculator ─────────────────────────────────────────────────────────────────

#[skg_tool(
    name = "calculator",
    description = "Evaluates an arbitrary mathematical expression natively within the agent. Can only evaluate one expression at a time."
)]
pub async fn calculator(
    expression: String,
) -> Result<serde_json::Value, ToolError> {
    match evalexpr::eval(&expression) {
        Ok(value) => Ok(serde_json::Value::String(value.to_string())),
        Err(e) => Err(ToolError::ExecutionFailed(format!("Calc evaluation error: {:?}", e))),
    }
}
