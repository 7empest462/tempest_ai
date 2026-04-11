use serde_json::Value;
use miette::Result;
use async_trait::async_trait;
use super::{AgentTool, ToolContext};
use sysinfo::System;
use schemars::JsonSchema;
use serde::Deserialize;
use ollama_rs::generation::tools::{ToolInfo, ToolFunctionInfo, ToolType};
use miette::IntoDiagnostic;

#[derive(Deserialize, JsonSchema)]
#[allow(dead_code)]
pub struct SystemdManagerArgs {
    /// The systemctl action to perform.
    pub action: String,
    /// The name of the service unit (e.g. 'nginx.service').
    pub unit: Option<String>,
}

pub struct SystemdManagerTool;

#[async_trait]
impl AgentTool for SystemdManagerTool {
    fn name(&self) -> &'static str { "systemd_manager" }
    fn description(&self) -> &'static str { "Controls systemd services (start, stop, restart, status, list) on Linux." }
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
        let typed_args: SystemdManagerArgs = serde_json::from_value(args.clone()).into_diagnostic()?;
            
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
                "start" | "stop" | "restart" | "status" | "enable" | "disable" => {
                    if unit.is_empty() {
                        return Ok("Error: Unit name is required for this action.".to_string());
                    }
                    cmd.args([action, unit]);
                }
                _ => return Ok(format!("Error: Unsupported action '{}'.", action)),
            }

            let output = cmd.output().into_diagnostic()?;
            let stdout = String::from_utf8_lossy(&output.stdout);
            let stderr = String::from_utf8_lossy(&output.stderr);

            Ok(format!("--- STDOUT ---\n{}\n\n--- STDERR ---\n{}", stdout, stderr))
        }

        #[cfg(not(target_os = "linux"))]
        {
            Ok("Systemd is only available on Linux system.".to_string())
        }
    }
}

pub struct CurrentProcessTool;

#[derive(Deserialize, JsonSchema)]
#[allow(dead_code)]
pub struct NoArgs {}

#[async_trait]
impl AgentTool for CurrentProcessTool {
    fn name(&self) -> &'static str { "current_process_info" }
    fn description(&self) -> &'static str { "Returns technical details about the agent's own process state (RAM, uptime)." }
    fn tool_info(&self) -> ToolInfo {
        let mut settings = schemars::generate::SchemaSettings::draft07();
        settings.inline_subschemas = true;
        let payload = settings.into_generator().into_root_schema_for::<NoArgs>();

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
            "Tempest AI Process Stats:\nCPU: {:.1}%\nMemory: {}/{} MiB",
            sys.global_cpu_usage(),
            sys.used_memory() / 1024 / 1024,
            sys.total_memory() / 1024 / 1024
        ))
    }
}

// Consolidation of hardware and telemetry into this module for modularity
pub use crate::telemetry::SystemTelemetryTool;
