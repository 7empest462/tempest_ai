use serde_json::{json, Value};
use anyhow::Result;
use async_trait::async_trait;
use super::{AgentTool, ToolContext};
use sysinfo::System;

pub struct SystemInfoTool;

#[async_trait]
impl AgentTool for SystemInfoTool {
    fn name(&self) -> &'static str { "system_info" }
    fn description(&self) -> &'static str { "Provides a high-level overview of the host's OS, CPU, and RAM usage." }
    fn parameters(&self) -> Value { json!({}) }

    async fn execute(&self, _args: &Value, _context: ToolContext) -> Result<String> {
        let mut sys = System::new_all();
        sys.refresh_all();
        
        Ok(format!(
            "OS: {} {}\nCPU: {:.1}%\nRAM: {} / {} MB",
            System::name().unwrap_or_default(),
            System::os_version().unwrap_or_default(),
            sys.global_cpu_usage(),
            sys.used_memory() / 1024 / 1024,
            sys.total_memory() / 1024 / 1024
        ))
    }
}

// Consolidation of hardware and telemetry into this module for modularity
pub use crate::hardware::{LinuxProcessAnalyzerTool, GpuDiagnosticsTool, TelemetryChartTool};
pub use crate::telemetry::{AdvancedSystemOracleTool, KernelDiagnosticTool, NetworkSnifferTool};
pub struct SystemdManagerTool;

#[async_trait]
impl AgentTool for SystemdManagerTool {
    fn name(&self) -> &'static str { "systemd_manager" }
    fn description(&self) -> &'static str { "Natively monitor and manage Systemd services on Linux. Use 'action': 'list' to see all units, or 'start'/'stop'/'restart'/'status' with a 'unit' name. REQUIRES LINUX HOST." }
    fn is_modifying(&self) -> bool { true }
    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "action": { "type": "string", "enum": ["list", "start", "stop", "restart", "status"], "description": "The systemctl action to perform." },
                "unit": { "type": "string", "description": "The name of the service unit (e.g. 'nginx.service')." }
            },
            "required": ["action"]
        })
    }

    async fn execute(&self, args: &Value, _context: ToolContext) -> Result<String> {
        #[cfg(target_os = "linux")]
        {
            let action = args.get("action").and_then(|a| a.as_str()).unwrap_or("list");
            let unit = args.get("unit").and_then(|u| u.as_str()).unwrap_or("");

            let mut cmd = std::process::Command::new("systemctl");
            match action {
                "list" => {
                    cmd.args(["list-units", "--type=service", "--all", "--no-pager"]);
                }
                "start" | "stop" | "restart" | "status" => {
                    if unit.is_empty() { anyhow::bail!("Unit name required for this action."); }
                    cmd.args([action, unit]);
                }
                _ => anyhow::bail!("Unsupported action."),
            }

            let output = cmd.output()?;
            let stdout = String::from_utf8_lossy(&output.stdout);
            let stderr = String::from_utf8_lossy(&output.stderr);

            if !stderr.is_empty() {
                Ok(format!("Systemd Output:\n{}\nErrors:\n{}", stdout, stderr))
            } else {
                Ok(format!("Systemd Output:\n{}", stdout))
            }
        }
        #[cfg(not(target_os = "linux"))]
        {
            let _ = args;
            Ok("Error: The systemd_manager tool is exclusive to Linux environments.".to_string())
        }
    }
}
