use axum::{
    extract::ws::{Message, WebSocket, WebSocketUpgrade},
    response::IntoResponse,
    routing::get,
    Router,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tower_http::cors::CorsLayer;
use crate::agent::Agent;
use futures::{sink::SinkExt, stream::StreamExt};
use sysinfo::System;

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type", content = "payload")]
pub enum NexusRequest {
    Chat { message: String },
    ListFiles { path: String },
    ReadFile { path: String },
    WriteFile { path: String, content: String },
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type", content = "payload")]
pub enum NexusResponse {
    Token { text: String },
    Done,
    FileTree { items: Vec<FileItem>, current_path: String },
    FileContent { content: String },
    Telemetry { cpu: f32, gpu: f32, ram: String },
    Error { message: String },
}

#[derive(Debug, Serialize, Deserialize)]
pub struct FileItem {
    pub name: String,
    pub is_dir: bool,
}

pub struct NexusState {
    pub agent: Agent,
}

pub async fn run_nexus(agent: Agent, port: u16) {
    let state = Arc::new(NexusState { agent });

    let app = Router::new()
        .route("/ws", get(ws_handler))
        .fallback_service(tower_http::services::ServeDir::new("tempest-web/dist"))
        .layer(CorsLayer::permissive())
        .with_state(state);

    let listener = tokio::net::TcpListener::bind(format!("0.0.0.0:{}", port))
        .await
        .unwrap();
    println!("🌪️  Tempest Nexus Online: http://localhost:{}", port);
    axum::serve(listener, app).await.unwrap();
}

async fn ws_handler(
    ws: WebSocketUpgrade,
    axum::extract::State(state): axum::extract::State<Arc<NexusState>>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_socket(socket, state))
}

async fn handle_socket(socket: WebSocket, state: Arc<NexusState>) {
    let (mut sender, mut receiver) = socket.split();
    let (ws_tx, mut ws_rx) = tokio::sync::mpsc::channel::<Message>(100);

    // Mux Task: Send messages from channel to WebSocket
    tokio::spawn(async move {
        while let Some(msg) = ws_rx.recv().await {
            if sender.send(msg).await.is_err() {
                break;
            }
        }
    });

    // Telemetry Task
    let ws_tx_tele = ws_tx.clone();
    tokio::spawn(async move {
        let mut sys = System::new_all();
        loop {
            sys.refresh_all();
            let cpu = sys.global_cpu_usage();
            let used_ram = sys.used_memory() / 1024 / 1024;
            let total_ram = sys.total_memory() / 1024 / 1024;
            
            let gpu_info = tempest_monitor::macos_helper::get_macos_gpu_info(false);
            let gpu = gpu_info.usage_pct as f32;

            let res = NexusResponse::Telemetry { 
                cpu, 
                gpu, 
                ram: format!("{}/{} MB", used_ram, total_ram) 
            };
            if let Ok(json) = serde_json::to_string(&res) {
                if ws_tx_tele.send(Message::Text(json.into())).await.is_err() {
                    break;
                }
            }
            tokio::time::sleep(std::time::Duration::from_secs(1)).await;
        }
    });

    // Receiver Loop
    while let Some(Ok(msg)) = receiver.next().await {
        if let Message::Text(text) = msg {
            if let Ok(req) = serde_json::from_str::<NexusRequest>(&text) {
                let ws_tx_req = ws_tx.clone();
                let agent = state.agent.clone();
                
                match req {
                    NexusRequest::Chat { message } => {
                        tokio::spawn(async move {
                            let stop_flag = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
                            // Here we would ideally hook into the Agent's token stream
                            // For now, we simulate the tokenized response
                            if let Err(e) = agent.run(message, stop_flag).await {
                                let res = NexusResponse::Error { message: e.to_string() };
                                let _ = ws_tx_req.send(Message::Text(serde_json::to_string(&res).unwrap().into())).await;
                            }
                            let _ = ws_tx_req.send(Message::Text(serde_json::to_string(&NexusResponse::Done).unwrap().into())).await;
                        });
                    }
                    NexusRequest::ListFiles { path } => {
                        let dir_path = if path.is_empty() || path == "." { "." } else { &path };
                        let mut items = Vec::new();
                        
                        let current_path = std::fs::canonicalize(dir_path)
                            .unwrap_or_else(|_| std::path::PathBuf::from(dir_path))
                            .to_string_lossy()
                            .to_string();

                        if let Ok(entries) = std::fs::read_dir(dir_path) {
                            for entry in entries.flatten() {
                                if let Ok(name) = entry.file_name().into_string() {
                                    if !name.starts_with('.') {
                                        items.push(FileItem { name, is_dir: entry.path().is_dir() });
                                    }
                                }
                            }
                        }
                        let res = NexusResponse::FileTree { items, current_path };
                        let _ = ws_tx_req.send(Message::Text(serde_json::to_string(&res).unwrap().into())).await;
                    }
                    NexusRequest::ReadFile { path } => {
                        match std::fs::read_to_string(&path) {
                            Ok(content) => {
                                let res = NexusResponse::FileContent { content };
                                let _ = ws_tx_req.send(Message::Text(serde_json::to_string(&res).unwrap().into())).await;
                            }
                            Err(e) => {
                                let res = NexusResponse::Error { message: e.to_string() };
                                let _ = ws_tx_req.send(Message::Text(serde_json::to_string(&res).unwrap().into())).await;
                            }
                        }
                    }
                    NexusRequest::WriteFile { path, content } => {
                        if let Ok(_) = std::fs::write(&path, content) {
                            let res = NexusResponse::Done;
                            let _ = ws_tx_req.send(Message::Text(serde_json::to_string(&res).unwrap().into())).await;
                        } else {
                            let res = NexusResponse::Error { message: format!("Failed to write to {}", path) };
                            let _ = ws_tx_req.send(Message::Text(serde_json::to_string(&res).unwrap().into())).await;
                        }
                    }
                }
            }
        }
    }
}
