use miette::{Result, IntoDiagnostic, miette};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, Command};
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
    #[allow(dead_code)]
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
        let request = json!({
            "jsonrpc": "2.0",
            "id": self.request_id,
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
        let _response = self.read_response().await?;
        Ok(())
    }

    pub async fn list_tools(&mut self) -> Result<Vec<McpTool>> {
        let request = json!({
            "jsonrpc": "2.0",
            "id": self.request_id,
            "method": "tools/list",
            "params": {}
        });
        self.request_id += 1;
        self.send_request(request).await?;
        let response = self.read_response().await?;
        
        let tools_val = response.get("result").and_then(|r| r.get("tools")).ok_or_else(|| miette!("Invalid tools/list response"))?;
        let tools: Vec<McpTool> = serde_json::from_value(tools_val.clone()).into_diagnostic()?;
        Ok(tools)
    }

    pub async fn call_tool(&mut self, name: &str, arguments: Value) -> Result<String> {
        let request = json!({
            "jsonrpc": "2.0",
            "id": self.request_id,
            "method": "tools/call",
            "params": {
                "name": name,
                "arguments": arguments
            }
        });
        self.request_id += 1;
        self.send_request(request).await?;
        let response = self.read_response().await?;
        
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
        self.stdin.write_all(line.as_bytes()).await.into_diagnostic()?;
        self.stdin.flush().await.into_diagnostic()?;
        Ok(())
    }

    async fn read_response(&mut self) -> Result<Value> {
        let mut line = String::new();
        self.reader.read_line(&mut line).await.into_diagnostic()?;
        serde_json::from_str(&line).into_diagnostic()
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
        ToolInfo {
            tool_type: ToolType::Function,
            function: ToolFunctionInfo {
                name: self.name.to_string(),
                description: self.description.to_string(),
                parameters: serde_json::from_value(self.input_schema.clone()).unwrap_or_else(|_| schemars::generate::SchemaSettings::draft07().into_generator().into_root_schema_for::<()>()),
            },
        }
    }

    async fn execute(&self, args: &Value, _context: ToolContext) -> Result<String> {
        let mut client = self.client.lock().await;
        let original_name = self.name.split('_').skip(1).collect::<Vec<_>>().join("_");
        client.call_tool(&original_name, args.clone()).await
    }
}
