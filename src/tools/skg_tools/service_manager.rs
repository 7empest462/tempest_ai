// ==========================================
// 💼 SKG SERVICES TOOL — Native Skelegent Implementations
// ==========================================

use skg_tool::{ToolCallContext, ToolError};
use skg_tool_macro::skg_tool;
use tempest_monitor::system_helper::get_services;

// ── list_system_services ───────────────────────────────────────────────────────

#[skg_tool(
    name = "list_system_services",
    description = "Lists all background system services (Launchd on macOS, Systemd on Linux) and their current status (running, stopped, status code). Default: Summary + Top 15. Use limit=0 for full list (NOT RECOMMENDED for large systems)."
)]
pub async fn list_system_services(
    limit: Option<usize>,
    filter: Option<String>,
    _ctx: &ToolCallContext,
) -> Result<serde_json::Value, ToolError> {
    let filter_lower = filter.as_deref().map(|s| s.to_lowercase());
    let mut services = get_services();

    if let Some(ref f) = filter_lower {
        services.retain(|s| s.label.to_lowercase().contains(f));
    }

    if services.is_empty() {
        return Ok(serde_json::Value::String(
            "No services found matching criteria.".to_string(),
        ));
    }

    let total_found = services.len();
    let limit_val = limit.unwrap_or(15);

    if limit_val > 0 && total_found > limit_val {
        services.truncate(limit_val);
    }

    let mut report = format!(
        "Found {} system services{}:\n\n",
        total_found,
        if limit_val > 0 && total_found > limit_val {
            format!(" (showing top {})", limit_val)
        } else {
            "".to_string()
        }
    );
    report.push_str("| Status | PID | Label |\n");
    report.push_str("|--------|-----|-------|\n");

    for svc in services {
        let is_ok = if cfg!(target_os = "macos") {
            svc.status == 0 || svc.status == 1 || svc.status == 78
        } else {
            svc.status == 0
        };
        let status_icon = if is_ok {
            "✅".to_string()
        } else {
            format!("❌ ({})", svc.status)
        };
        let pid_str = svc
            .pid
            .map(|p| p.to_string())
            .unwrap_or_else(|| "-".to_string());
        report.push_str(&format!(
            "| {} | {} | {} |\n",
            status_icon, pid_str, svc.label
        ));
    }

    if limit_val > 0 && total_found > limit_val {
        report
            .push_str("\n💡 [TRUNCATED] Use 'filter' parameter to narrow down specific services.");
    }

    Ok(serde_json::Value::String(report))
}
