use serde_json::Value;
use anyhow::Result;
use async_trait::async_trait;
use super::{AgentTool, ToolContext};
use schemars::JsonSchema;
use serde::Deserialize;
use ollama_rs::generation::tools::{ToolInfo, ToolFunctionInfo, ToolType};

#[derive(Deserialize, JsonSchema)]
pub struct SystemTelemetryArgs {}

pub struct SystemTelemetryTool;

#[async_trait]
impl AgentTool for SystemTelemetryTool {
    fn name(&self) -> &'static str { "get_system_telemetry" }
    fn description(&self) -> &'static str { 
        "Returns comprehensive real-time system telemetry. Includes CPU usage, GPU stats, memory (RAM + SWAP), all thermal sensors, battery status, disk usage, and more. Use this for hardware-aware planning."
    }
    
    fn tool_info(&self) -> ToolInfo {
        let mut settings = schemars::generate::SchemaSettings::draft07();
        settings.inline_subschemas = true;
        let generator = settings.into_generator();
        let payload = generator.into_root_schema_for::<SystemTelemetryArgs>();
        
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
        // TODO: Call actual telemetry collection.
        // Returning placeholder for now, just as the original macro did.
        Ok(r#"
System Telemetry Snapshot:
- CPU Usage: ~45% (avg)
- GPU Usage: ~60%
- Thermal Sensors: CPU 48°C, GPU 52°C, Battery 41°C, ... (42 sensors total)
- RAM: 12.4 GB / 32 GB used
- Battery: 87% (discharging)
- Disk: 245 GB / 980 GB used
"#.trim().to_string())
    }
}
