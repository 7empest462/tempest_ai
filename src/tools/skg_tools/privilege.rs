// ==========================================
// 🔑 SKG PRIVILEGE TOOL — Native Skelegent Implementations
// ==========================================

use skg_tool::{ToolCallContext, ToolError};
use skg_tool_macro::skg_tool;
use std::sync::atomic::Ordering;
use tokio::process::Command;

// ── request_privileges ─────────────────────────────────────────────────────────

#[skg_tool(
    name = "request_privileges",
    description = "SECURE ESCALATION: Requests root-level privileges (sudo) from the user. Use this when a tool fails due to permission denied or for deep system audits."
)]
pub async fn request_privileges(
    rationale: String,
    ctx: &ToolCallContext,
) -> Result<serde_json::Value, ToolError> {
    let tool_ctx = ctx.deps::<std::sync::Arc<crate::tools::ToolContext>>()
        .ok_or_else(|| ToolError::ExecutionFailed("Missing ToolContext dependency".to_string()))?;

    if tool_ctx.is_root.load(Ordering::SeqCst) {
        return Ok(serde_json::Value::String("✅ Agent already has root privileges.".to_string()));
    }

    // Proposed Improvement: Check if passwordless or cached sudo is already active BEFORE requesting TUI approval
    let check = Command::new("sudo")
        .arg("-n")
        .arg("true")
        .status()
        .await;

    if let Ok(status) = check && status.success() {
        tool_ctx.is_root.store(true, Ordering::SeqCst);
        return Ok(serde_json::Value::String(format!(
            "🚀 Privilege escalation SUCCESSFUL. Rationale: {} (Passwordless/Cached mode confirmed)",
            rationale
        )));
    }

    // Otherwise, fall back to TUI approval channel if available
    let (tx, mut rx) = tokio::sync::mpsc::channel(1);

    if let Some(ref tx_sender) = tool_ctx.tx {
        tx_sender
            .send(crate::tui::AgentEvent::RequestPrivileges {
                rationale: rationale.clone(),
                response_tx: tx,
            })
            .await
            .map_err(|e| ToolError::ExecutionFailed(format!("Failed to send privilege request to TUI: {}", e)))?;
    } else {
        return Err(ToolError::ExecutionFailed(
            "Privilege escalation requires interactive password entry, but agent is running in non-TUI mode and no sudo credentials are cached.".to_string()
        ));
    }

    // Wait for user response
    match rx.recv().await {
        Some(crate::tui::ToolResponse::Confirmed(true)) => {
            // Perform validation check to see if we can actually run sudo non-interactively now
            let check = Command::new("sudo").arg("-n").arg("true").status().await;

            match check {
                Ok(status) if status.success() => {
                    tool_ctx.is_root.store(true, Ordering::SeqCst);
                    Ok(serde_json::Value::String(format!(
                        "🚀 Privilege escalation SUCCESSFUL. Rationale: {} (Passwordless/Cached mode confirmed)",
                        rationale
                    )))
                }
                _ => {
                    Ok(serde_json::Value::String(format!(
                        "⚠️ Privilege escalation approved but requires a password. Commands will fail until you run 'sudo -v' (or similar) in your primary terminal to cache credentials. Rationale: {}",
                        rationale
                    )))
                }
            }
        }
        Some(crate::tui::ToolResponse::Confirmed(false)) => {
            Err(ToolError::ExecutionFailed("Privilege escalation REJECTED by user.".to_string()))
        }
        Some(crate::tui::ToolResponse::Error(e)) => {
            Err(ToolError::ExecutionFailed(format!("TUI error during privilege escalation: {}", e)))
        }
        _ => Err(ToolError::ExecutionFailed("Privilege escalation FAILED: No response or timeout.".to_string())),
    }
}
