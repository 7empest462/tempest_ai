use miette::{Result, IntoDiagnostic, miette};
use async_trait::async_trait;
use serde_json::Value;
use std::process::Command;
use super::{AgentTool, ToolContext};
use schemars::JsonSchema;
use serde::Deserialize;
use ollama_rs::generation::tools::{ToolInfo, ToolFunctionInfo, ToolType};

#[derive(Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum NetworkAction {
    Ping,
    Dns,
    Port,
}

#[derive(Deserialize, JsonSchema)]
pub struct NetworkCheckArgs {
    /// Action to perform: ping, dns, or port
    pub action: NetworkAction,
    /// Hostname or IP to test
    pub host: String,
    /// Port number (required for 'port' action)
    pub port: Option<u16>,
}

pub struct NetworkCheckTool;

#[async_trait]
impl AgentTool for NetworkCheckTool {
    fn name(&self) -> &'static str { "network_check" }
    fn description(&self) -> &'static str { "Performs low-level ICMP and socket diagnostics. DO NOT USE THIS to verify general internet access. Use ONLY for targeted sysadmin debugging." }
    
    fn tool_info(&self) -> ToolInfo {
        let mut settings = schemars::generate::SchemaSettings::draft07();
        settings.inline_subschemas = true;
        let payload = settings.into_generator().into_root_schema_for::<NetworkCheckArgs>();
        
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
        let typed_args: NetworkCheckArgs = serde_json::from_value(args.clone()).into_diagnostic()?;

        match typed_args.action {
            NetworkAction::Ping => {
                let output = Command::new("ping")
                    .args(["-c", "4", "-W", "3", &typed_args.host])
                    .output().into_diagnostic()?;
                let stdout = String::from_utf8_lossy(&output.stdout);
                let stderr = String::from_utf8_lossy(&output.stderr);
                if output.status.success() {
                    Ok(format!("✅ Ping results:\n{}", stdout))
                } else {
                    Ok(format!("❌ Ping failed:\n{}{}", stdout, stderr))
                }
            },
            NetworkAction::Dns => {
                let output = Command::new("dig")
                    .args(["+short", "+time=3", "+tries=1", &typed_args.host])
                    .output().into_diagnostic()?;
                let result = String::from_utf8_lossy(&output.stdout).trim().to_string();
                if result.is_empty() {
                    Ok(format!("❌ DNS lookup failed for '{}'", typed_args.host))
                } else {
                    Ok(format!("✅ DNS results for '{}':\n{}", typed_args.host, result))
                }
            },
            NetworkAction::Port => {
                let port = typed_args.port.ok_or_else(|| miette!("Missing 'port' for port check"))?;
                let addr = format!("{}:{}", typed_args.host, port);
                // Simple TCP connect attempt
                match std::net::TcpStream::connect_timeout(
                    &format!("{}:{}", typed_args.host, port).parse().unwrap_or_else(|_| {
                        // Fallback resolution attempt if parsing as SocketAddr fails
                        std::net::SocketAddr::from(([127, 0, 0, 1], port))
                    }),
                    std::time::Duration::from_secs(3),
                ) {
                    Ok(_) => Ok(format!("✅ Port {} is OPEN on {}", port, typed_args.host)),
                    Err(e) => {
                        // Better fallback for hostnames
                        use std::net::ToSocketAddrs;
                        if let Ok(mut addrs) = addr.to_socket_addrs() {
                            if let Some(socket_addr) = addrs.next() {
                                if let Ok(_) = std::net::TcpStream::connect_timeout(&socket_addr, std::time::Duration::from_secs(3)) {
                                    return Ok(format!("✅ Port {} is OPEN on {}", port, typed_args.host));
                                }
                            }
                        }
                        Ok(format!("❌ Port {} is CLOSED on {} — {}", port, typed_args.host, e))
                    }
                }
            }
        }
    }
}
