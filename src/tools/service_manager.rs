use serde_json::Value;
use miette::Result;
use async_trait::async_trait;
use super::{AgentTool, ToolContext};
use schemars::JsonSchema;
use serde::Deserialize;
use ollama_rs::generation::tools::{ToolInfo, ToolFunctionInfo, ToolType};
use tempest_monitor::system_helper::get_services;

#[derive(Deserialize, JsonSchema)]
pub struct ListServicesArgs {
    /// Optional limit on the number of services to show (default: 15).
    pub limit: Option<usize>,
    /// Optional filter string (checks if service label contains this).
    pub filter: Option<String>,
}

pub struct ListServicesTool;

#[async_trait]
impl AgentTool for ListServicesTool {
    fn name(&self) -> &'static str { "list_system_services" }
    fn description(&self) -> &'static str { 
        "Lists all background system services (Launchd on macOS, Systemd on Linux) and their current status (running, stopped, status code). Default: Summary + Top 15. Use limit=0 for full list (NOT RECOMMENDED for large systems)."
    }
    
    fn tool_info(&self) -> ToolInfo {
        let mut settings = schemars::generate::SchemaSettings::draft07();
        settings.inline_subschemas = true;
        let generator = settings.into_generator();
        let payload = generator.into_root_schema_for::<ListServicesArgs>();
        
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
        let typed_args: ListServicesArgs = serde_json::from_value(args.clone()).unwrap_or(ListServicesArgs { limit: None, filter: None });
        let filter = typed_args.filter.as_deref().map(|s| s.to_lowercase());
        let mut services = get_services();

        if let Some(f) = filter {
            services.retain(|s| s.label.to_lowercase().contains(&f));
        }

        if services.is_empty() {
            return Ok("No services found matching criteria.".to_string());
        }

        let total_found = services.len();
        let limit = typed_args.limit.unwrap_or(15);
        
        if limit > 0 && total_found > limit {
            services.truncate(limit);
        }

        let mut report = format!("Found {} system services{}:\n\n", total_found, if limit > 0 && total_found > limit { format!(" (showing top {})", limit) } else { "".to_string() });
        report.push_str("| Status | PID | Label |\n");
        report.push_str("|--------|-----|-------|\n");
        
        for svc in services {
            let is_ok = if cfg!(target_os = "macos") {
                svc.status == 0 || svc.status == 1 || svc.status == 78
            } else {
                svc.status == 0
            };
            let status_icon = if is_ok { "✅".to_string() } else { format!("❌ ({})", svc.status) };
            let pid_str = svc.pid.map(|p| p.to_string()).unwrap_or_else(|| "-".to_string());
            report.push_str(&format!("| {} | {} | {} |\n", status_icon, pid_str, svc.label));
        }

        if limit > 0 && total_found > limit {
            report.push_str("\n💡 [TRUNCATED] Use 'filter' parameter to narrow down specific services.");
        }

        Ok(report)
    }
}
