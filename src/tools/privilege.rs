use serde_json::Value;
use miette::{Result, IntoDiagnostic, miette};
use async_trait::async_trait;
use super::{AgentTool, ToolContext};
use schemars::JsonSchema;
use serde::Deserialize;
use ollama_rs::generation::tools::{ToolInfo, ToolFunctionInfo, ToolType};
use std::sync::atomic::Ordering;
use tokio::process::Command;

#[derive(Deserialize, JsonSchema)]
pub struct RequestPrivilegesArgs {
    /// Rationale for why privileges are needed (e.g., 'To access GPU telemetry', 'To restart systemd services').
    pub rationale: String,
}

pub struct RequestPrivilegesTool;

#[async_trait]
impl AgentTool for RequestPrivilegesTool {
    fn name(&self) -> &'static str { "request_privileges" }
    fn description(&self) -> &'static str { "SECURE ESCALATION: Requests root-level privileges (sudo) from the user. Use this when a tool fails due to permission denied or for deep system audits." }
    fn is_modifying(&self) -> bool { true }
    fn tool_info(&self) -> ToolInfo {
        let mut settings = schemars::generate::SchemaSettings::draft07();
        settings.inline_subschemas = true;
        let payload = settings.into_generator().into_root_schema_for::<RequestPrivilegesArgs>();
        
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
        let typed_args: RequestPrivilegesArgs = serde_json::from_value(args.clone()).into_diagnostic()?;
        
        if context.is_root.load(Ordering::SeqCst) {
            return Ok("✅ Agent already has root privileges.".to_string());
        }

        let (tx, mut rx) = tokio::sync::mpsc::channel(1);
        
        // Send request to TUI
        context.tx.send(crate::tui::AgentEvent::RequestPrivileges {
            rationale: typed_args.rationale.clone(),
            response_tx: tx,
        }).await.map_err(|e| miette!("Failed to send privilege request to TUI: {}", e))?;

        // Wait for user response
        match rx.recv().await {
            Some(crate::tui::ToolResponse::Confirmed(true)) => {
                // Perform validation check to see if we can actually run sudo non-interactively
                let check = Command::new("sudo")
                    .arg("-n")
                    .arg("true")
                    .status()
                    .await;

                match check {
                    Ok(status) if status.success() => {
                        context.is_root.store(true, Ordering::SeqCst);
                        Ok(format!("🚀 Privilege escalation SUCCESSFUL. Rationale: {} (Passwordless/Cached mode confirmed)", typed_args.rationale))
                    }
                    _ => {
                        // Sudo failed or requires password. For now, we set the flag anyway
                        // but warn the user.
                        context.is_root.store(true, Ordering::SeqCst);
                        Ok(format!("⚠️ Privilege escalation PARTIAL SUCCESS. Sudo appears to require a password. Commands may fail unless you run 'sudo true' in your terminal first. Rationale: {}", typed_args.rationale))
                    }
                }
            }
            Some(crate::tui::ToolResponse::Confirmed(false)) => {
                Err(miette!("Privilege escalation REJECTED by user."))
            }
            Some(crate::tui::ToolResponse::Error(e)) => {
                Err(miette!("TUI error during privilege escalation: {}", e))
            }
            _ => Err(miette!("Privilege escalation FAILED: No response or timeout.")),
        }
    }
}
