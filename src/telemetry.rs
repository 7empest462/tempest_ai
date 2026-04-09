use async_trait::async_trait;
use serde_json::Value;
use miette::{Result, IntoDiagnostic};
use super::tools::{AgentTool, ToolContext};
use schemars::JsonSchema;
use serde::Deserialize;
use ollama_rs::generation::tools::{ToolInfo, ToolFunctionInfo, ToolType};
use sysinfo::{System, RefreshKind};

#[derive(Deserialize, JsonSchema)]
#[allow(dead_code)]
pub struct TelemetryArgs {
    /// If true, returns detailed per-core and per-disk stats.
    pub detailed: Option<bool>,
}

pub struct SystemTelemetryTool;

#[async_trait]
impl AgentTool for SystemTelemetryTool {
    fn name(&self) -> &'static str { "system_telemetry" }
    fn description(&self) -> &'static str { "Returns real-time hardware metrics (CPU, Memory, Disk, Temp). Use this to diagnose performance issues or verify system state." }
    fn tool_info(&self) -> ToolInfo {
        let mut settings = schemars::generate::SchemaSettings::draft07();
        settings.inline_subschemas = true;
        let payload = settings.into_generator().into_root_schema_for::<TelemetryArgs>();
        
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
        let typed_args: TelemetryArgs = serde_json::from_value(args.clone()).into_diagnostic()?;
        let detailed = typed_args.detailed.unwrap_or(false);

        let mut sys = System::new_with_specifics(
            RefreshKind::everything()
        );
        sys.refresh_all();

        let mut report = format!("🖥️ SYSTEM TELEMETRY (Detailed: {})\n", detailed);
        report.push_str(&format!("CPU Usage: {:.1}%\n", sys.global_cpu_usage()));
        report.push_str(&format!("Memory: {}/{} MB used\n", sys.used_memory() / 1024 / 1024, sys.total_memory() / 1024 / 1024));
        
        if detailed {
            report.push_str("\n--- CPU CORES ---\n");
            for (i, cpu) in sys.cpus().iter().enumerate() {
                report.push_str(&format!(" Core {}: {:.1}% @ {}MHz\n", i, cpu.cpu_usage(), cpu.frequency()));
            }
        }

        Ok(report)
    }
}
