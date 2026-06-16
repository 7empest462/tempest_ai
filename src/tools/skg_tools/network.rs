// ==========================================
// 🕸️ SKG NETWORK CHECK TOOL — Native Skelegent Implementations
// ==========================================

use skg_tool::{ToolCallContext, ToolError};
use skg_tool_macro::skg_tool;
use std::process::Command;

#[derive(serde::Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum NetworkAction {
    Ping,
    Dns,
    Port,
}

// ── network_check ─────────────────────────────────────────────────────────────

#[skg_tool(
    name = "network_check",
    description = "Performs low-level ICMP and socket diagnostics. DO NOT USE THIS to verify general internet access. Use ONLY for targeted sysadmin debugging."
)]
pub async fn network_check(
    action: NetworkAction,
    host: String,
    port: Option<u16>,
    _ctx: &ToolCallContext,
) -> Result<serde_json::Value, ToolError> {
    let res = match action {
        NetworkAction::Ping => {
            let output = Command::new("ping")
                .args(["-c", "4", "-W", "3", &host])
                .output()
                .map_err(|e| ToolError::ExecutionFailed(format!("Ping spawn failed: {}", e)))?;
            let stdout = String::from_utf8_lossy(&output.stdout);
            let stderr = String::from_utf8_lossy(&output.stderr);
            if output.status.success() {
                format!("✅ Ping results:\n{}", stdout)
            } else {
                format!("❌ Ping failed:\n{}{}", stdout, stderr)
            }
        }
        NetworkAction::Dns => {
            let output = Command::new("dig")
                .args(["+short", "+time=3", "+tries=1", &host])
                .output()
                .map_err(|e| ToolError::ExecutionFailed(format!("Dig spawn failed: {}", e)))?;
            let result = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if result.is_empty() {
                format!("❌ DNS lookup failed for '{}'", host)
            } else {
                format!("✅ DNS results for '{}':\n{}", host, result)
            }
        }
        NetworkAction::Port => {
            let p_num = port.ok_or_else(|| ToolError::ExecutionFailed("Missing 'port' for port check".to_string()))?;
            let addr = format!("{}:{}", host, p_num);
            match std::net::TcpStream::connect_timeout(
                &format!("{}:{}", host, p_num)
                    .parse()
                    .unwrap_or_else(|_| {
                        std::net::SocketAddr::from(([127, 0, 0, 1], p_num))
                    }),
                std::time::Duration::from_secs(3),
            ) {
                Ok(_) => format!("✅ Port {} is OPEN on {}", p_num, host),
                Err(e) => {
                    use std::net::ToSocketAddrs;
                    if let Ok(mut addrs) = addr.to_socket_addrs()
                        && let Some(socket_addr) = addrs.next()
                        && let Ok(_) = std::net::TcpStream::connect_timeout(
                            &socket_addr,
                            std::time::Duration::from_secs(3),
                        )
                    {
                        format!("✅ Port {} is OPEN on {}", p_num, host)
                    } else {
                        format!("❌ Port {} is CLOSED on {} — {}", p_num, host, e)
                    }
                }
            }
        }
    };

    Ok(serde_json::Value::String(res))
}
