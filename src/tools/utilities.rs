use anyhow::Result;
use async_trait::async_trait;
use serde_json::Value;
use std::process::Command;
use super::{AgentTool, ToolContext};

pub struct ClipboardTool;

#[async_trait]
impl AgentTool for ClipboardTool {
    fn name(&self) -> &'static str { "clipboard" }
    fn description(&self) -> &'static str { "Read from or write to the system clipboard. Use 'read' to get clipboard contents, or 'write' to copy text to the clipboard so the user can paste it." }
    fn parameters(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "action": { "type": "string", "description": "'read' to get clipboard contents, 'write' to set them" },
                "content": { "type": "string", "description": "Text to copy to clipboard (required for 'write')" }
            },
            "required": ["action"]
        })
    }

    async fn execute(&self, args: &Value, _context: ToolContext) -> Result<String> {
        let action = args.get("action").and_then(|a| a.as_str()).unwrap_or("read");

        match action {
            "write" => {
                let content = args.get("content").and_then(|c| c.as_str())
                    .ok_or_else(|| anyhow::anyhow!("Missing 'content' for clipboard write"))?;
                let mut clipboard = arboard::Clipboard::new()
                    .map_err(|e| anyhow::anyhow!("Failed to access clipboard: {}", e))?;
                clipboard.set_text(content.to_string())
                    .map_err(|e| anyhow::anyhow!("Failed to write to clipboard: {}", e))?;
                Ok(format!("✅ Copied {} characters to clipboard.", content.len()))
            },
            "read" => {
                let mut clipboard = arboard::Clipboard::new()
                    .map_err(|e| anyhow::anyhow!("Failed to access clipboard: {}", e))?;
                let text = clipboard.get_text()
                    .map_err(|e| anyhow::anyhow!("Failed to read clipboard: {}", e))?;
                Ok(format!("Clipboard contents:\n{}", text))
            },
            _ => anyhow::bail!("Unknown clipboard action '{}'. Use 'read' or 'write'.", action),
        }
    }
}

pub struct NotifyTool;

#[async_trait]
impl AgentTool for NotifyTool {
    fn name(&self) -> &'static str { "notify" }
    fn description(&self) -> &'static str { "Sends a native macOS/Linux desktop notification. Use this to alert the user when a long-running task completes." }
    fn parameters(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "title": { "type": "string", "description": "Notification title" },
                "message": { "type": "string", "description": "Notification message body" }
            },
            "required": ["title", "message"]
        })
    }

    async fn execute(&self, args: &Value, _context: ToolContext) -> Result<String> {
        let title = args.get("title").and_then(|t| t.as_str()).unwrap_or("Tempest AI");
        let message = args.get("message").and_then(|m| m.as_str()).unwrap_or("Task complete.");

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
                .output()?;
            if output.status.success() {
                Ok(format!("🔔 Notification sent: {} — {}", title, message))
            } else {
                let err = String::from_utf8_lossy(&output.stderr);
                anyhow::bail!("Failed to send notification: {}", err)
            }
        }

        #[cfg(target_os = "linux")]
        {
            let output = Command::new("notify-send")
                .arg(title)
                .arg(message)
                .output();
            match output {
                Ok(o) if o.status.success() => Ok(format!("🔔 Notification sent: {} — {}", title, message)),
                Ok(o) => {
                    let err = String::from_utf8_lossy(&o.stderr);
                    anyhow::bail!("Failed to send notification (is libnotify installed?): {}", err)
                }
                Err(_) => anyhow::bail!("notify-send not found. Install with: sudo apt install libnotify-bin"),
            }
        }

        #[cfg(not(any(target_os = "macos", target_os = "linux")))]
        {
            Ok(format!("Notification not supported on this platform. Title: {} Message: {}", title, message))
        }
    }
}

pub struct EnvVarTool;

#[async_trait]
impl AgentTool for EnvVarTool {
    fn name(&self) -> &'static str { "env_var" }
    fn description(&self) -> &'static str { "Read environment variables. Use 'get' to read a specific variable or 'list' to show all exported variables." }
    fn parameters(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "action": { "type": "string", "description": "'get' or 'list'" },
                "name": { "type": "string", "description": "Variable name (required for get)" }
            },
            "required": ["action"]
        })
    }

    async fn execute(&self, args: &Value, _context: ToolContext) -> Result<String> {
        let action = args.get("action").and_then(|a| a.as_str()).unwrap_or("get");

        match action {
            "get" => {
                let name = args.get("name").and_then(|n| n.as_str())
                    .ok_or_else(|| anyhow::anyhow!("Missing 'name' for env get"))?;
                match std::env::var(name) {
                    Ok(val) => Ok(format!("{}={}", name, val)),
                    Err(_) => Ok(format!("Variable '{}' is not set.", name)),
                }
            },
            "list" => {
                let vars: Vec<String> = std::env::vars()
                    .take(50)
                    .map(|(k, v)| {
                        let truncated = if v.len() > 100 { format!("{}...", &v[..100]) } else { v };
                        format!("{}={}", k, truncated)
                    })
                    .collect();
                Ok(format!("Environment variables (first 50):\n{}", vars.join("\n")))
            },
            _ => anyhow::bail!("Unknown env_var action '{}'. Use 'get' or 'list'.", action),
        }
    }
}

pub struct ChmodTool;

#[async_trait]
impl AgentTool for ChmodTool {
    fn name(&self) -> &'static str { "chmod" }
    fn description(&self) -> &'static str { "Change file or directory permissions using standard Unix mode strings (e.g., '755', '644', '+x')." }
    fn requires_confirmation(&self) -> bool { true }
    fn is_modifying(&self) -> bool { true }
    fn parameters(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "path": { "type": "string", "description": "Path to file or directory" },
                "mode": { "type": "string", "description": "Permission mode (e.g., '755', '644', '+x', 'u+rwx')" }
            },
            "required": ["path", "mode"]
        })
    }

    async fn execute(&self, args: &Value, _context: ToolContext) -> Result<String> {
        let path_str = args.get("path").and_then(|p| p.as_str()).ok_or_else(|| anyhow::anyhow!("Missing 'path'"))?;
        let path = shellexpand::tilde(path_str).to_string();
        let mode = args.get("mode").and_then(|m| m.as_str()).ok_or_else(|| anyhow::anyhow!("Missing 'mode'"))?;

        let output = Command::new("chmod")
            .args([mode, &path])
            .output()?;
        
        if output.status.success() {
            Ok(format!("✅ Changed permissions of '{}' to '{}'", path, mode))
        } else {
            let err = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!("chmod failed: {}", err.trim())
        }
    }
}
