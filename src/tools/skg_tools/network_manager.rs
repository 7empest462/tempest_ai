// ==========================================
// 🕸️ SKG NETWORK SOCKETS TOOL — Native Skelegent Implementations
// ==========================================

use skg_tool::{ToolCallContext, ToolError};
use skg_tool_macro::skg_tool;
use sysinfo::System;
use tempest_monitor::system_helper::get_sockets;

// ── list_network_sockets ───────────────────────────────────────────────────────

#[skg_tool(
    name = "list_network_sockets",
    description = "Lists active network connections. Highly Recommended: Use 'pid' to filter results for a specific process to avoid overwhelming context."
)]
pub async fn list_network_sockets(
    pid: Option<i32>,
    limit: Option<usize>,
    _ctx: &ToolCallContext,
) -> Result<serde_json::Value, ToolError> {
    let mut sys = System::new_with_specifics(sysinfo::RefreshKind::everything());
    sys.refresh_processes(sysinfo::ProcessesToUpdate::All, true);

    let mut sockets = get_sockets(&sys);

    if let Some(p) = pid {
        sockets.retain(|s| s.pid == Some(p));
    }

    let limit_val = limit.unwrap_or(50);
    sockets.truncate(limit_val);

    if sockets.is_empty() {
        return Ok(serde_json::Value::String("No matching active network sockets found.".to_string()));
    }

    let mut report = format!("Found {} matching network sockets:\n\n", sockets.len());
    report.push_str("| Proto | Local Address | Foreign Address | State | PID | Process |\n");
    report.push_str("|-------|---------------|-----------------|-------|-----|---------|\n");

    for s in sockets {
        let pid_str = s
            .pid
            .map(|p| p.to_string())
            .unwrap_or_else(|| "-".to_string());
        report.push_str(&format!(
            "| {} | {} | {} | {} | {} | {} |\n",
            s.proto, s.local_addr, s.foreign_addr, s.state, pid_str, s.process_name
        ));
    }

    Ok(serde_json::Value::String(report))
}
