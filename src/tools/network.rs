use anyhow::Result;
use async_trait::async_trait;
use serde_json::Value;
use std::process::Command;
use super::{AgentTool, ToolContext};

pub struct NetworkCheckTool;

#[async_trait]
impl AgentTool for NetworkCheckTool {
    fn name(&self) -> &'static str { "network_check" }
    fn description(&self) -> &'static str { "Performs safe, non-hanging network diagnostics (ping, dns, port check)." }
    fn parameters(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "action": { "type": "string", "description": "'ping', 'dns', or 'port'" },
                "host": { "type": "string", "description": "Hostname or IP to test" },
                "port": { "type": "integer", "description": "Port number (required for 'port' action)" }
            },
            "required": ["action", "host"]
        })
    }

    async fn execute(&self, args: &Value, _context: ToolContext) -> Result<String> {
        let action = args.get("action").and_then(|a| a.as_str()).unwrap_or("ping");
        let host = args.get("host").and_then(|h| h.as_str()).ok_or_else(|| anyhow::anyhow!("Missing 'host'"))?;

        match action {
            "ping" => {
                let output = Command::new("ping")
                    .args(["-c", "4", "-W", "3", host])
                    .output()?;
                let stdout = String::from_utf8_lossy(&output.stdout);
                let stderr = String::from_utf8_lossy(&output.stderr);
                if output.status.success() {
                    Ok(format!("✅ Ping results:\n{}", stdout))
                } else {
                    Ok(format!("❌ Ping failed:\n{}{}", stdout, stderr))
                }
            },
            "dns" => {
                let output = Command::new("dig")
                    .args(["+short", "+time=3", "+tries=1", host])
                    .output()?;
                let result = String::from_utf8_lossy(&output.stdout).trim().to_string();
                if result.is_empty() {
                    Ok(format!("❌ DNS lookup failed for '{}'", host))
                } else {
                    Ok(format!("✅ DNS results for '{}':\n{}", host, result))
                }
            },
            "port" => {
                let port = args.get("port").and_then(|p| p.as_u64())
                    .ok_or_else(|| anyhow::anyhow!("Missing 'port' for port check"))? as u16;
                let addr = format!("{}:{}", host, port);
                // Simple TCP connect attempt
                match std::net::TcpStream::connect_timeout(
                    &format!("{}:{}", host, port).parse().unwrap_or_else(|_| {
                        // Fallback resolution attempt if parsing as SocketAddr fails
                        std::net::SocketAddr::from(([127, 0, 0, 1], port))
                    }),
                    std::time::Duration::from_secs(3),
                ) {
                    Ok(_) => Ok(format!("✅ Port {} is OPEN on {}", port, host)),
                    Err(e) => {
                        // Better fallback for hostnames
                        use std::net::ToSocketAddrs;
                        if let Ok(mut addrs) = addr.to_socket_addrs() {
                            if let Some(socket_addr) = addrs.next() {
                                if let Ok(_) = std::net::TcpStream::connect_timeout(&socket_addr, std::time::Duration::from_secs(3)) {
                                    return Ok(format!("✅ Port {} is OPEN on {}", port, host));
                                }
                            }
                        }
                        Ok(format!("❌ Port {} is CLOSED on {} — {}", port, host, e))
                    }
                }
            },
            _ => anyhow::bail!("Unknown network action '{}'. Use 'ping', 'dns', or 'port'.", action),
        }
    }
}
