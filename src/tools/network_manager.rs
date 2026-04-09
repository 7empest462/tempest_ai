use serde_json::Value;
use miette::{Result, IntoDiagnostic};
use async_trait::async_trait;
use super::{AgentTool, ToolContext};
use schemars::JsonSchema;
use serde::Deserialize;
use ollama_rs::generation::tools::{ToolInfo, ToolFunctionInfo, ToolType};
use sysinfo::System;
use tempest_monitor::system_helper::get_sockets;

#[derive(Deserialize, JsonSchema)]
pub struct ListSocketsArgs {
    /// Optional: Filter results for a specific Process ID (PID).
    pub pid: Option<i32>,
    /// Optional: Limit the number of results (default 50).
    pub limit: Option<usize>,
}

pub struct ListSocketsTool;

#[async_trait]
impl AgentTool for ListSocketsTool {
    fn name(&self) -> &'static str { "list_network_sockets" }
    fn description(&self) -> &'static str { 
        "Lists active network connections. Highly Recommended: Use 'pid' to filter results for a specific process to avoid overwhelming context."
    }
    
    fn tool_info(&self) -> ToolInfo {
        let mut settings = schemars::generate::SchemaSettings::draft07();
        settings.inline_subschemas = true;
        let payload = settings.into_generator().into_root_schema_for::<ListSocketsArgs>();
        
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
        let typed_args: ListSocketsArgs = serde_json::from_value(args.clone()).into_diagnostic()?;
        
        let mut sys = System::new_with_specifics(sysinfo::RefreshKind::everything());
        sys.refresh_processes(sysinfo::ProcessesToUpdate::All, true);

        let mut sockets = get_sockets(&sys);
        
        // 🔬 FILTRATION
        if let Some(pid) = typed_args.pid {
            sockets.retain(|s| s.pid == Some(pid));
        }

        let limit = typed_args.limit.unwrap_or(50);
        sockets.truncate(limit);

        if sockets.is_empty() {
            return Ok("No matching active network sockets found.".to_string());
        }

        let mut report = format!("Found {} matching network sockets:\n\n", sockets.len());
        report.push_str("| Proto | Local Address | Foreign Address | State | PID | Process |\n");
        report.push_str("|-------|---------------|-----------------|-------|-----|---------|\n");
        
        for s in sockets {
            let pid_str = s.pid.map(|p| p.to_string()).unwrap_or_else(|| "-".to_string());
            report.push_str(&format!(
                "| {} | {} | {} | {} | {} | {} |\n",
                s.proto, s.local_addr, s.foreign_addr, s.state, pid_str, s.process_name
            ));
        }

        Ok(report)
    }
}
