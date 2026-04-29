use miette::{Result, IntoDiagnostic, miette};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, Command};
use tokio::time::{timeout, Duration};
use std::collections::HashMap;
use std::sync::Arc;
use async_trait::async_trait;
use crate::tools::{AgentTool, ToolContext};
use ollama_rs::generation::tools::{ToolInfo, ToolType, ToolFunctionInfo};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpTool {
    pub name: String,
    pub description: String,
    pub input_schema: Value,
}

pub struct McpClient {
    pub name: String,
    #[allow(dead_code)]
    child: Child,
    stdin: tokio::process::ChildStdin,
    reader: BufReader<tokio::process::ChildStdout>,
    request_id: i64,
}

impl McpClient {
    pub async fn new(name: String, command: &str, args: &[String], env: &HashMap<String, String>) -> Result<Self> {
        let mut child = Command::new(command)
            .args(args)
            .envs(env)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::inherit())
            .spawn()
            .into_diagnostic()?;

        let stdin = child.stdin.take().ok_or_else(|| miette!("Failed to open stdin"))?;
        let stdout = child.stdout.take().ok_or_else(|| miette!("Failed to open stdout"))?;
        let reader = BufReader::new(stdout);

        Ok(Self {
            name,
            child,
            stdin,
            reader,
            request_id: 0,
        })
    }

    pub async fn initialize(&mut self) -> Result<()> {
        let id = self.request_id;
        let request = json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": "initialize",
            "params": {
                "protocolVersion": "2024-11-05",
                "capabilities": {},
                "clientInfo": {
                    "name": "TempestAI",
                    "version": "1.0.0"
                }
            }
        });
        self.request_id += 1;
        self.send_request(request).await?;
        let _response = self.read_response_with_id(id).await?;
        Ok(())
    }

    pub async fn list_tools(&mut self) -> Result<Vec<McpTool>> {
        let id = self.request_id;
        let request = json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": "tools/list",
            "params": {}
        });
        self.request_id += 1;
        self.send_request(request).await?;
        let response = self.read_response_with_id(id).await?;
        
        let tools_val = response.get("result").and_then(|r| r.get("tools")).ok_or_else(|| miette!("Invalid tools/list response"))?;
        let tools: Vec<McpTool> = serde_json::from_value(tools_val.clone()).into_diagnostic()?;
        Ok(tools)
    }

    pub async fn call_tool(&mut self, name: &str, arguments: Value) -> Result<String> {
        let id = self.request_id;
        let request = json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": "tools/call",
            "params": {
                "name": name,
                "arguments": arguments
            }
        });
        self.request_id += 1;
        self.send_request(request).await?;
        let response = self.read_response_with_id(id).await?;
        
        if let Some(error) = response.get("error") {
            return Err(miette!("MCP Error: {}", error));
        }

        let content = response.get("result").and_then(|r| r.get("content")).ok_or_else(|| miette!("Invalid tools/call response"))?;
        
        let mut result_text = String::new();
        if let Some(arr) = content.as_array() {
            for item in arr {
                if let Some(text) = item.get("text").and_then(|t| t.as_str()) {
                    result_text.push_str(text);
                }
            }
        }

        Ok(result_text)
    }

    async fn send_request(&mut self, request: Value) -> Result<()> {
        let mut line = serde_json::to_string(&request).into_diagnostic()?;
        line.push('\n');
        timeout(Duration::from_secs(5), self.stdin.write_all(line.as_bytes()))
            .await
            .map_err(|_| miette!("Timeout writing to MCP server stdin"))?
            .into_diagnostic()?;
        
        timeout(Duration::from_secs(2), self.stdin.flush())
            .await
            .map_err(|_| miette!("Timeout flushing MCP server stdin"))?
            .into_diagnostic()?;
        Ok(())
    }

    async fn read_response_with_id(&mut self, expected_id: i64) -> Result<Value> {
        loop {
            let mut line = String::new();
            let bytes_read = timeout(Duration::from_secs(30), self.reader.read_line(&mut line))
                .await
                .map_err(|_| miette!("Timeout reading from MCP server '{}' stdout", self.name))?
                .into_diagnostic()?;

            if bytes_read == 0 {
                return Err(miette!("MCP server '{}' closed stdout unexpectedly (EOF)", self.name));
            }

            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }

            // Attempt to parse line as JSON-RPC
            match serde_json::from_str::<Value>(trimmed) {
                Ok(val) => {
                    if val.get("jsonrpc").is_some() {
                        if let Some(resp_id) = val.get("id").and_then(|i| i.as_i64()) {
                            if resp_id == expected_id {
                                return Ok(val);
                            } else {
                                // Potentially a notification or out-of-order response
                                eprintln!("Warning [MCP {}]: Received unexpected ID {} (expected {})", self.name, resp_id, expected_id);
                                continue;
                            }
                        } else if val.get("method").is_some() {
                            // This is likely a notification (no ID)
                            continue;
                        }
                    }
                    // Not a standard JSON-RPC object or wrong ID, but valid JSON
                    continue;
                }
                Err(_) => {
                    // Stdout pollution (debug logs, warnings, etc.)
                    // In a real TUI, we might want to pipe this to a "System" log window
                    continue;
                }
            }
        }
    }
}

pub struct McpToolProxy {
    pub client: Arc<tokio::sync::Mutex<McpClient>>,
    pub name: &'static str,
    pub description: &'static str,
    pub input_schema: Value,
}

#[async_trait]
impl AgentTool for McpToolProxy {
    fn name(&self) -> &'static str { self.name }
    fn description(&self) -> &'static str { self.description }
    fn tool_info(&self) -> ToolInfo {
        // We assume input_schema is pre-validated during registration
        ToolInfo {
            tool_type: ToolType::Function,
            function: ToolFunctionInfo {
                name: self.name.to_string(),
                description: self.description.to_string(),
                parameters: serde_json::from_value(self.input_schema.clone()).unwrap(),
            },
        }
    }

    async fn execute(&self, args: &Value, _context: ToolContext) -> Result<String> {
        let mut client = self.client.lock().await;
        let original_name = self.name.split('_').skip(1).collect::<Vec<_>>().join("_");
        
        match client.call_tool(&original_name, args.clone()).await {
            Ok(res) => Ok(res),
            Err(e) => {
                // Return the error back to the LLM so it can handle/retry/fix parameters
                Ok(format!("❌ MCP Tool Error ({}): {}", self.name, e))
            }
        }
    }
}
