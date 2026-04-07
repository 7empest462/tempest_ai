use serde_json::Value;
use anyhow::Result;
use async_trait::async_trait;
use super::{AgentTool, ToolContext};
use sysinfo::System;
use schemars::JsonSchema;
use serde::Deserialize;
use ollama_rs::generation::tools::{ToolInfo, ToolFunctionInfo, ToolType};

#[derive(Deserialize, JsonSchema)]
#[allow(dead_code)]
pub struct SystemInfoArgs {}

#[allow(dead_code)]
pub struct SystemInfoTool;

#[async_trait]
impl AgentTool for SystemInfoTool {
    fn name(&self) -> &'static str { "system_info" }
    fn description(&self) -> &'static str { "Provides a high-level overview of the host's OS, CPU, and RAM usage." }
    
    fn tool_info(&self) -> ToolInfo {
        let mut settings = schemars::generate::SchemaSettings::draft07();
        settings.inline_subschemas = true;
        let generator = settings.into_generator();
        let payload = generator.into_root_schema_for::<SystemInfoArgs>();
        
        ToolInfo {
            tool_type: ToolType::Function,
            function: ToolFunctionInfo {
                name: self.name().to_string(),
                description: self.description().to_string(),
                parameters: payload.into(),
            }
        }
    }

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
#[allow(unused_imports)]
pub use crate::hardware::TelemetryChartTool;
#[derive(Deserialize, JsonSchema)]
#[allow(dead_code)]
pub struct SystemdManagerArgs {
    /// The systemctl action to perform.
    pub action: String,
    /// The name of the service unit (e.g. 'nginx.service').
    pub unit: Option<String>,
}

#[allow(dead_code)]
pub struct SystemdManagerTool;

#[async_trait]
impl AgentTool for SystemdManagerTool {
    fn name(&self) -> &'static str { "systemd_manager" }
    fn description(&self) -> &'static str { "Natively monitor and manage Systemd services on Linux. Use 'action': 'list' to see all units, or 'start'/'stop'/'restart'/'status' with a 'unit' name. REQUIRES LINUX HOST." }
    fn is_modifying(&self) -> bool { true }
    
    fn tool_info(&self) -> ToolInfo {
        let mut settings = schemars::generate::SchemaSettings::draft07();
        settings.inline_subschemas = true;
        let payload = settings.into_generator().into_root_schema_for::<SystemdManagerArgs>();
        
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
        let typed_args: SystemdManagerArgs = serde_json::from_value(args.clone())
            .map_err(|e| anyhow::anyhow!("Invalid parameters: {}", e))?;
            
        #[cfg(target_os = "linux")]
        {
            let action = typed_args.action.as_str();
            let unit_opt = typed_args.unit;
            let unit = unit_opt.as_deref().unwrap_or("");

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
            let _ = typed_args;
            Ok("Error: The systemd_manager tool is exclusive to Linux environments.".to_string())
        }
    }
}
