// Copyright (c) 2026 Robert Simens. All Rights Reserved.
// Licensed under the Tempest AI Source-Available License.
// See LICENSE in the project root for full license information.

use crate::mcp_protocol::{JsonRpcRequest, TempestRequest, TempestResponse, ChatPayload};
use miette::{Result, IntoDiagnostic, miette};
use serde_json::{Value, json};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use crate::agent::Agent;

pub struct McpServer {
    agent: Agent,
}

impl McpServer {
    pub fn new(agent: Agent) -> Self {
        Self { agent }
    }

    pub async fn run(&mut self) -> Result<()> {
        let mut reader = BufReader::new(tokio::io::stdin());
        let mut stdout = tokio::io::stdout();

        loop {
            let mut line = String::new();
            let bytes_read = reader.read_line(&mut line).await.into_diagnostic()?;
            if bytes_read == 0 { break; }

            let request: Value = match serde_json::from_str(&line) {
                Ok(v) => v,
                Err(_) => continue,
            };

            let id = request.get("id").cloned();
            let method = request.get("method").and_then(|m| m.as_str()).unwrap_or("");

            // Handle semantic tempest/* methods
            if method.starts_with("tempest/") {
                let tempest_req: JsonRpcRequest = match serde_json::from_value(request) {
                    Ok(r) => r,
                    Err(e) => {
                        let error_resp = json!({
                            "jsonrpc": "2.0",
                            "id": id,
                            "error": { "code": -32602, "message": format!("Invalid semantic request: {}", e) }
                        });
                        let _ = stdout.write_all((serde_json::to_string(&error_resp).unwrap() + "\n").as_bytes()).await;
                        continue;
                    }
                };

                self.handle_tempest_request(id, tempest_req.payload, &mut stdout).await?;
                continue;
            }

            // Fallback to standard MCP
            let response = match method {
                "initialize" => {
                    json!({
                        "jsonrpc": "2.0",
                        "id": id,
                        "result": {
                            "protocolVersion": "2024-11-05",
                            "capabilities": { "tools": { "listChanged": false } },
                            "serverInfo": { "name": "TempestAI-Server", "version": "0.1.0" }
                        }
                    })
                }
                "tools/list" => {
                    let mut tool_list = Vec::new();
                    for entry in self.agent.get_tools().iter() {
                        let tool = entry.value();
                        let info = tool.tool_info();
                        tool_list.push(json!({
                            "name": info.function.name,
                            "description": info.function.description,
                            "inputSchema": info.function.parameters
                        }));
                    }
                    json!({ "jsonrpc": "2.0", "id": id, "result": { "tools": tool_list } })
                }
                "tools/call" => {
                    let params = request.get("params").ok_or_else(|| miette!("Missing params"))?;
                    let name = params.get("name").and_then(|n| n.as_str()).ok_or_else(|| miette!("Missing tool name"))?;
                    let arguments = params.get("arguments").cloned().unwrap_or(json!({}));

                    let result = self.agent.execute_tool_by_name(name, &arguments).await;
                    match result {
                        Ok(output) => json!({ "jsonrpc": "2.0", "id": id, "result": { "content": [{"type": "text", "text": output}] } }),
                        Err(e) => json!({ "jsonrpc": "2.0", "id": id, "error": { "code": -32000, "message": e.to_string() } }),
                    }
                }
                _ => json!({ "jsonrpc": "2.0", "id": id, "error": { "code": -32601, "message": "Method not found" } }),
            };

            let response_line = serde_json::to_string(&response).into_diagnostic()? + "\n";
            stdout.write_all(response_line.as_bytes()).await.into_diagnostic()?;
            stdout.flush().await.into_diagnostic()?;
        }

        Ok(())
    }

    async fn handle_tempest_request(&self, id: Option<Value>, req: TempestRequest, stdout: &mut tokio::io::Stdout) -> Result<()> {
        match req {
            TempestRequest::Chat { message, editor_context, .. } => {
                let (tx, mut rx) = tokio::sync::mpsc::channel(100);
                *self.agent.event_tx.lock() = Some(tx);
                
                // Update editor context in agent
                if let Some(ctx) = editor_context {
                    *self.agent.editor_context.lock() = Some(ctx);
                }

                let agent_clone = self.agent.clone();
                let message_final = message.to_string();
                let stop_flag = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
                let mut agent_handle = tokio::spawn(async move { agent_clone.run(message_final, stop_flag).await });

                loop {
                    tokio::select! {
                        Some(event) = rx.recv() => {
                            match event {
                                crate::tui::AgentEvent::StreamToken(token) => {
                                    let resp = TempestResponse::ChatResponse {
                                        payload: ChatPayload { content: token, reasoning: None, is_streaming: true, done: false }
                                    };
                                    let envelope = json!({ "jsonrpc": "2.0", "id": id, "result": resp });
                                    let _ = stdout.write_all((serde_json::to_string(&envelope).unwrap() + "\n").as_bytes()).await;
                                }
                                crate::tui::AgentEvent::ReasoningToken(token) => {
                                    let resp = TempestResponse::ChatResponse {
                                        payload: ChatPayload { content: String::new(), reasoning: Some(token), is_streaming: true, done: false }
                                    };
                                    let envelope = json!({ "jsonrpc": "2.0", "id": id, "result": resp });
                                    let _ = stdout.write_all((serde_json::to_string(&envelope).unwrap() + "\n").as_bytes()).await;
                                }
                                crate::tui::AgentEvent::SentinelUpdate { log, .. } => {
                                    let resp = TempestResponse::ChatResponse {
                                        payload: ChatPayload { content: String::new(), reasoning: Some(log), is_streaming: true, done: false }
                                    };
                                    let envelope = json!({ "jsonrpc": "2.0", "id": id, "result": resp });
                                    let _ = stdout.write_all((serde_json::to_string(&envelope).unwrap() + "\n").as_bytes()).await;
                                }
                                crate::tui::AgentEvent::SubagentStatus(status) => {
                                    if let Some(text) = status {
                                        let envelope = json!({ "jsonrpc": "2.0", "method": "tempest/status", "params": { "text": text } });
                                        let _ = stdout.write_all((serde_json::to_string(&envelope).unwrap() + "\n").as_bytes()).await;
                                    }
                                }
                                crate::tui::AgentEvent::Thinking(text) => {
                                    if let Some(t) = text {
                                        let envelope = json!({ "jsonrpc": "2.0", "method": "tempest/thought", "params": { "text": t } });
                                        let _ = stdout.write_all((serde_json::to_string(&envelope).unwrap() + "\n").as_bytes()).await;
                                    }
                                }
                                _ => {}
                            }
                            let _ = stdout.flush().await;
                        }
                        res = &mut agent_handle => {
                            let _res = res.unwrap_or_else(|_| Err(miette!("Agent task panicked")));
                            let final_resp = TempestResponse::ChatResponse {
                                payload: ChatPayload { content: String::new(), reasoning: None, is_streaming: false, done: true }
                            };
                            let envelope = json!({ "jsonrpc": "2.0", "id": id, "result": final_resp });
                            let _ = stdout.write_all((serde_json::to_string(&envelope).unwrap() + "\n").as_bytes()).await;
                            let _ = stdout.flush().await;
                            break;
                        }
                    }
                }
            }
            TempestRequest::Status => {
                let model = self.agent.get_model();
                let phase = format!("{:?}", *self.agent.phase.lock());
                let resp = TempestResponse::StatusResponse {
                    backend: "mlx".to_string(),
                    phase,
                    model,
                    ram_usage_mb: 0,
                    context_tokens: 0,
                };
                let envelope = json!({ "jsonrpc": "2.0", "id": id, "result": resp });
                let _ = stdout.write_all((serde_json::to_string(&envelope).unwrap() + "\n").as_bytes()).await;
                let _ = stdout.flush().await;
            }
            TempestRequest::SwitchBackend { backend } => {
                let resp = TempestResponse::SwitchBackendResponse {
                    success: true,
                    message: format!("Switched to {}", backend),
                };
                let envelope = json!({ "jsonrpc": "2.0", "id": id, "result": resp });
                let _ = stdout.write_all((serde_json::to_string(&envelope).unwrap() + "\n").as_bytes()).await;
                let _ = stdout.flush().await;
            }
            TempestRequest::ClearHistory => {
                self.agent.clear_history();
                let resp = TempestResponse::ClearHistoryResponse { success: true };
                let envelope = json!({ "jsonrpc": "2.0", "id": id, "result": resp });
                let _ = stdout.write_all((serde_json::to_string(&envelope).unwrap() + "\n").as_bytes()).await;
                let _ = stdout.flush().await;
            }
            TempestRequest::GetState => {
                let phase = format!("{:?}", *self.agent.phase.lock());
                let resp = TempestResponse::StateResponse {
                    phase,
                    planning_enabled: self.agent.planning_enabled,
                    recent_tool_calls: vec![],
                };
                let envelope = json!({ "jsonrpc": "2.0", "id": id, "result": resp });
                let _ = stdout.write_all((serde_json::to_string(&envelope).unwrap() + "\n").as_bytes()).await;
                let _ = stdout.flush().await;
            }
        }
        Ok(())
    }
}
