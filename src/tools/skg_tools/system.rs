// ==========================================
// ⚙️ SKG SYSTEM TOOLS — Native Skelegent Implementations
// ==========================================

use skg_tool::{ToolCallContext, ToolError};
use skg_tool_macro::skg_tool;
use sysinfo::System;

// ── systemd_manager ────────────────────────────────────────────────────────────

#[skg_tool(
    name = "systemd_manager",
    description = "Controls systemd services (start, stop, restart, status, list) on Linux."
)]
pub async fn systemd_manager(
    action: String,
    unit: Option<String>,
    _ctx: &ToolCallContext,
) -> Result<serde_json::Value, ToolError> {
    #[cfg(target_os = "linux")]
    {
        let unit_name = unit.as_deref().unwrap_or("");
        let mut cmd = std::process::Command::new("systemctl");
        match action.as_str() {
            "list" => {
                cmd.args(["list-units", "--type=service", "--all", "--no-pager"]);
            }
            "start" | "stop" | "restart" | "status" | "enable" | "disable" => {
                if unit_name.is_empty() {
                    return Err(ToolError::ExecutionFailed("Unit name is required for this action.".to_string()));
                }
                cmd.args([action.as_str(), unit_name]);
            }
            _ => return Err(ToolError::ExecutionFailed(format!("Unsupported action '{}'.", action))),
        }

        let output = cmd.output()
            .map_err(|e| ToolError::ExecutionFailed(format!("Failed to run systemctl: {}", e)))?;
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);

        Ok(serde_json::Value::String(format!(
            "--- STDOUT ---\n{}\n\n--- STDERR ---\n{}",
            stdout, stderr
        )))
    }

    #[cfg(not(target_os = "linux"))]
    {
        let _ = action;
        let _ = unit;
        Ok(serde_json::Value::String("Systemd is only available on Linux system.".to_string()))
    }
}

// ── current_process_info ───────────────────────────────────────────────────────

#[skg_tool(
    name = "current_process_info",
    description = "Returns technical details about the agent's own process state (RAM, uptime)."
)]
pub async fn current_process_info(
    _ctx: &ToolCallContext,
) -> Result<serde_json::Value, ToolError> {
    let mut sys = System::new_all();
    sys.refresh_all();

    let report = format!(
        "Tempest AI Process Stats:\nCPU: {:.1}%\nMemory: {}/{} MiB",
        sys.global_cpu_usage(),
        sys.used_memory() / 1024 / 1024,
        sys.total_memory() / 1024 / 1024
    );

    Ok(serde_json::Value::String(report))
}
