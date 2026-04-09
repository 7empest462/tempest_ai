use miette::{Result, IntoDiagnostic, miette};
use async_trait::async_trait;
use serde_json::Value;
use std::process::Command;
use super::{AgentTool, ToolContext};
use schemars::JsonSchema;
use serde::Deserialize;
use ollama_rs::generation::tools::{ToolInfo, ToolFunctionInfo, ToolType};

#[derive(Deserialize, JsonSchema)]
pub struct ClipboardArgs {
    /// 'read' to get clipboard contents, 'write' to set them
    pub action: String,
    /// Text to copy to clipboard (required for 'write')
    pub content: Option<String>,
}

pub struct ClipboardTool;

#[async_trait]
impl AgentTool for ClipboardTool {
    fn name(&self) -> &'static str { "clipboard" }
    fn description(&self) -> &'static str { "Read from or write to the system clipboard. Use 'read' to get clipboard contents, or 'write' to copy text to the clipboard so the user can paste it." }
    fn tool_info(&self) -> ToolInfo {
        let mut settings = schemars::generate::SchemaSettings::draft07();
        settings.inline_subschemas = true;
        let payload = settings.into_generator().into_root_schema_for::<ClipboardArgs>();
        
        ToolInfo {
            tool_type: ToolType::Function,
            function: ToolFunctionInfo {
                name: self.name().to_string(),
                description: self.description().to_string(),
                parameters: payload.into(),
            }
        }
    }

    async fn execute(&self, args: &Value, _context: ToolContext) -> Result<String> {
        let typed_args: ClipboardArgs = serde_json::from_value(args.clone()).into_diagnostic()?;
        let action = typed_args.action;

        match action.as_str() {
            "write" => {
                let content = typed_args.content
                    .ok_or_else(|| miette!("Missing 'content' for clipboard write"))?;
                let mut clipboard = arboard::Clipboard::new()
                    .map_err(|e| miette!("Failed to access clipboard: {}", e))?;
                clipboard.set_text(content.to_string())
                    .map_err(|e| miette!("Failed to write to clipboard: {}", e))?;
                Ok(format!("✅ Copied {} characters to clipboard.", content.len()))
            },
            "read" => {
                let mut clipboard = arboard::Clipboard::new()
                    .map_err(|e| miette!("Failed to access clipboard: {}", e))?;
                let text = clipboard.get_text()
                    .map_err(|e| miette!("Failed to read clipboard: {}", e))?;
                Ok(format!("Clipboard contents:\n{}", text))
            },
            _ => Err(miette!("Unknown clipboard action. Use 'read' or 'write'.")),
        }
    }
}

#[derive(Deserialize, JsonSchema)]
pub struct NotifyArgs {
    /// Notification title
    pub title: String,
    /// Notification message body
    pub message: String,
}

pub struct NotifyTool;

#[async_trait]
impl AgentTool for NotifyTool {
    fn name(&self) -> &'static str { "notify" }
    fn description(&self) -> &'static str { "Sends a native macOS/Linux desktop notification. Use this to alert the user when a long-running task completes." }
    fn tool_info(&self) -> ToolInfo {
        let mut settings = schemars::generate::SchemaSettings::draft07();
        settings.inline_subschemas = true;
        let payload = settings.into_generator().into_root_schema_for::<NotifyArgs>();
        
        ToolInfo {
            tool_type: ToolType::Function,
            function: ToolFunctionInfo {
                name: self.name().to_string(),
                description: self.description().to_string(),
                parameters: payload.into(),
            }
        }
    }

    async fn execute(&self, args: &Value, _context: ToolContext) -> Result<String> {
        let typed_args: NotifyArgs = serde_json::from_value(args.clone()).into_diagnostic()?;
        let title = typed_args.title;
        let message = typed_args.message;

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
                .output().into_diagnostic()?;
            if output.status.success() {
                Ok(format!("🔔 Notification sent: {} — {}", title, message))
            } else {
                let err = String::from_utf8_lossy(&output.stderr);
                Err(miette!("Failed to send notification: {}", err))
            }
        }

        #[cfg(target_os = "linux")]
        {
            let output = Command::new("notify-send")
                .arg(&title)
                .arg(&message)
                .output();
            match output {
                Ok(o) if o.status.success() => Ok(format!("🔔 Notification sent: {} — {}", title, message)),
                Ok(o) => {
                    let err = String::from_utf8_lossy(&o.stderr);
                    Err(miette!("Failed to send notification (is libnotify installed?): {}", err))
                }
                Err(_) => Err(miette!("notify-send not found. Install with: sudo apt install libnotify-bin")),
            }
        }

        #[cfg(not(any(target_os = "macos", target_os = "linux")))]
        {
            let _ = title;
            let _ = message;
            Ok("Notification not supported on this platform.".to_string())
        }
    }
}

#[derive(Deserialize, JsonSchema)]
pub struct EnvVarArgs {
    /// 'get' or 'list'
    pub action: String,
    /// Variable name (required for get)
    pub name: Option<String>,
}

pub struct EnvVarTool;

#[async_trait]
impl AgentTool for EnvVarTool {
    fn name(&self) -> &'static str { "env_var" }
    fn description(&self) -> &'static str { "Read environment variables. Use 'get' to read a specific variable or 'list' to show all exported variables." }
    fn tool_info(&self) -> ToolInfo {
        let mut settings = schemars::generate::SchemaSettings::draft07();
        settings.inline_subschemas = true;
        let payload = settings.into_generator().into_root_schema_for::<EnvVarArgs>();
        
        ToolInfo {
            tool_type: ToolType::Function,
            function: ToolFunctionInfo {
                name: self.name().to_string(),
                description: self.description().to_string(),
                parameters: payload.into(),
            }
        }
    }

    async fn execute(&self, args: &Value, _context: ToolContext) -> Result<String> {
        let typed_args: EnvVarArgs = serde_json::from_value(args.clone()).into_diagnostic()?;
        let action = typed_args.action;

        match action.as_str() {
            "get" => {
                let name = typed_args.name
                    .ok_or_else(|| miette!("Missing 'name' for env get"))?;
                match std::env::var(&name) {
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
            _ => Err(miette!("Unknown env_var action. Use 'get' or 'list'.")),
        }
    }
}

#[derive(Deserialize, JsonSchema)]
pub struct ChmodArgs {
    /// Path to file or directory
    pub path: String,
    /// Permission mode (e.g., '755', '644', '+x', 'u+rwx')
    pub mode: String,
}

pub struct ChmodTool;

#[async_trait]
impl AgentTool for ChmodTool {
    fn name(&self) -> &'static str { "chmod" }
    fn description(&self) -> &'static str { "Change file or directory permissions using standard Unix mode strings (e.g., '755', '644', '+x')." }
    fn is_modifying(&self) -> bool { true }
    fn tool_info(&self) -> ToolInfo {
        let mut settings = schemars::generate::SchemaSettings::draft07();
        settings.inline_subschemas = true;
        let payload = settings.into_generator().into_root_schema_for::<ChmodArgs>();
        
        ToolInfo {
            tool_type: ToolType::Function,
            function: ToolFunctionInfo {
                name: self.name().to_string(),
                description: self.description().to_string(),
                parameters: payload.into(),
            }
        }
    }

    async fn execute(&self, args: &Value, _context: ToolContext) -> Result<String> {
        let typed_args: ChmodArgs = serde_json::from_value(args.clone()).into_diagnostic()?;
        let path = shellexpand::tilde(&typed_args.path).to_string();
        let mode = typed_args.mode;

        let output = Command::new("chmod")
            .arg(&mode)
            .arg(&path)
            .output().into_diagnostic()?;
        
        if output.status.success() {
            Ok(format!("✅ Changed permissions of '{}' to '{}'", path, mode))
        } else {
            let err = String::from_utf8_lossy(&output.stderr);
            Err(miette!("chmod failed: {}", err.trim()))
        }
    }
}
