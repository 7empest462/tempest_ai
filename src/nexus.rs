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
use portable_pty::{CommandBuilder, PtySize, native_pty_system};
use std::io::{Read, Write};
use include_dir::{include_dir, Dir};

static WEB_DIR: Dir<'_> = include_dir!("tempest-web/dist");

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type", content = "payload")]
pub enum NexusRequest {
    Chat { message: String, #[serde(default)] editor_context: Option<String> },
    ListFiles { path: String },
    ReadFile { path: String },
    WriteFile { path: String, content: String },
    CreateFile { path: String },
    CreateFolder { path: String },
    DeleteItem { path: String },
    RenameItem { old_path: String, new_path: String },
    TerminalSpawn {},
    TerminalInput { data: String },
    TerminalResize { cols: u16, rows: u16 },
    SearchFiles { query: String, path: String },
    SafeModeApprove {},
    SafeModeReject {},
    StopStream {},
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
    TerminalOutput { data: String },
    SearchResults { matches: Vec<SearchMatch> },
    BackendInfo { 
        backend: String,
        planner: String,
        executor: String,
        verifier: String,
    },
    AgentStateChange { state: String },
    ActiveTools { tools: Vec<String> },
    SafeModeRequest { rationale: String, diff: String },
    TaskUpdate { task: String },
    ReasoningToken { token: String },
    StreamToken { token: String },
    InferenceMetrics { tps: Option<u64>, ctx_used: Option<usize>, ctx_total: Option<u64> },
    SentinelLog { sentinel: String, message: String },
    ToolResult { name: String, success: bool },
}

#[derive(Debug, Serialize, Deserialize)]
pub struct FileItem {
    pub name: String,
    pub is_dir: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchMatch {
    pub file: String,
    pub line: u32,
    pub content: String,
}

pub struct NexusState {
    pub agent: Agent,
    pub backend_id: String,
    pub planner_model: String,
    pub executor_model: String,
    pub verifier_model: String,
    pub broadcast_tx: tokio::sync::broadcast::Sender<String>,
    pub tool_tx: Option<tokio::sync::mpsc::Sender<crate::tui::ToolResponse>>,
    pub stop_flag: std::sync::Arc<std::sync::atomic::AtomicBool>,
}

pub async fn run_nexus(
    agent: Agent, 
    port: u16, 
    backend_id: String,
    event_rx: Option<tokio::sync::mpsc::Receiver<crate::tui::AgentEvent>>,
    tool_tx: Option<tokio::sync::mpsc::Sender<crate::tui::ToolResponse>>,
) {
    let (b_tx, _b_rx) = tokio::sync::broadcast::channel(100);
    
    let state = Arc::new(NexusState { 
        planner_model: agent.planner_model.clone().unwrap_or_default(),
        executor_model: agent.executor_model.clone().unwrap_or_default(),
        verifier_model: agent.verifier_model.clone().unwrap_or_default(),
        agent, 
        backend_id,
        broadcast_tx: b_tx.clone(),
        tool_tx,
        stop_flag: std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),
    });

    if let Some(mut rx) = event_rx {
        let b_tx_clone = b_tx.clone();
        tokio::spawn(async move {
            while let Some(evt) = rx.recv().await {
                match evt {
                    crate::tui::AgentEvent::AgentStateChange(s) => {
                        let res = NexusResponse::AgentStateChange { state: s };
                        if let Ok(json) = serde_json::to_string(&res) {
                            let _ = b_tx_clone.send(json);
                        }
                    }
                    crate::tui::AgentEvent::ActiveTools(t) => {
                        let res = NexusResponse::ActiveTools { tools: t };
                        if let Ok(json) = serde_json::to_string(&res) {
                            let _ = b_tx_clone.send(json);
                        }
                    }
                    crate::tui::AgentEvent::TaskUpdate(t) => {
                        let res = NexusResponse::TaskUpdate { task: t };
                        if let Ok(json) = serde_json::to_string(&res) {
                            let _ = b_tx_clone.send(json);
                        }
                    }
                    crate::tui::AgentEvent::RequestInput(id, prompt) => {
                        if id == "BATCH_APPROVAL" {
                            let res = NexusResponse::SafeModeRequest { 
                                rationale: "The agent is requesting batch approval for the following changes.".to_string(),
                                diff: prompt 
                            };
                            if let Ok(json) = serde_json::to_string(&res) {
                                let _ = b_tx_clone.send(json);
                            }
                        }
                    }
                    crate::tui::AgentEvent::RequestPrivileges { rationale, .. } => {
                        let res = NexusResponse::SafeModeRequest { 
                            rationale: format!("Privilege Escalation Requested: {}", rationale),
                            diff: "PERMISSION REQUEST".to_string()
                        };
                        if let Ok(json) = serde_json::to_string(&res) {
                            let _ = b_tx_clone.send(json);
                        }
                    }
                    crate::tui::AgentEvent::ReasoningToken(t) => {
                        let res = NexusResponse::ReasoningToken { token: t };
                        if let Ok(json) = serde_json::to_string(&res) {
                            let _ = b_tx_clone.send(json);
                        }
                    }
                    crate::tui::AgentEvent::StreamToken(t) => {
                        let res = NexusResponse::StreamToken { token: t };
                        if let Ok(json) = serde_json::to_string(&res) {
                            let _ = b_tx_clone.send(json);
                        }
                    }
                    crate::tui::AgentEvent::TelemetryMetrics { tps, .. } => {
                        if tps.is_some() {
                            let res = NexusResponse::InferenceMetrics { tps, ctx_used: None, ctx_total: None };
                            if let Ok(json) = serde_json::to_string(&res) {
                                let _ = b_tx_clone.send(json);
                            }
                        }
                    }
                    crate::tui::AgentEvent::ContextStatus { used, total } => {
                        let res = NexusResponse::InferenceMetrics { tps: None, ctx_used: Some(used), ctx_total: Some(total) };
                        if let Ok(json) = serde_json::to_string(&res) {
                            let _ = b_tx_clone.send(json);
                        }
                    }
                    crate::tui::AgentEvent::SentinelUpdate { active, log } => {
                        // Only forward to WebSocket when a sentinel actually triggered
                        if !log.is_empty() {
                            let sentinel_name = active.first().cloned().unwrap_or_else(|| "Unknown".to_string());
                            let res = NexusResponse::SentinelLog { sentinel: sentinel_name, message: log };
                            if let Ok(json) = serde_json::to_string(&res) {
                                let _ = b_tx_clone.send(json);
                            }
                        }
                    }
                    _ => {}
                }
            }
        });
    }

    let app = Router::new()
        .route("/ws", get(ws_handler))
        .fallback(static_handler)
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

    // Mux Task 2: Send broadcast events to WebSocket
    let mut broadcast_rx = state.broadcast_tx.subscribe();
    let ws_tx_broadcast = ws_tx.clone();
    tokio::spawn(async move {
        while let Ok(json) = broadcast_rx.recv().await {
            let _ = ws_tx_broadcast.send(Message::Text(json.into())).await;
        }
    });

    // Send initial backend info
    let backend_info = NexusResponse::BackendInfo { 
        backend: state.backend_id.clone(),
        planner: state.planner_model.clone(),
        executor: state.executor_model.clone(),
        verifier: state.verifier_model.clone(),
    };
    if let Ok(json) = serde_json::to_string(&backend_info) {
        let _ = ws_tx.send(Message::Text(json.into())).await;
    }

    // Telemetry Task
    let ws_tx_tele = ws_tx.clone();
    tokio::spawn(async move {
        let mut sys = System::new_all();
        loop {
            sys.refresh_all();
            let cpu = sys.global_cpu_usage();
            let used_ram = sys.used_memory() / 1024 / 1024;
            let total_ram = sys.total_memory() / 1024 / 1024;
            
            let gpu = {
                #[cfg(target_os = "macos")]
                {
                    tempest_monitor::macos_helper::get_macos_gpu_info(false).usage_pct as f32
                }
                #[cfg(target_os = "linux")]
                {
                    tempest_monitor::linux_helper::collect_gpu_telemetry().usage_pct as f32
                }
                #[cfg(not(any(target_os = "macos", target_os = "linux")))]
                {
                    0.0
                }
            };

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

    // Shared PTY writer handle — set when terminal is spawned
    let pty_writer: Arc<tokio::sync::Mutex<Option<Box<dyn Write + Send>>>> =
        Arc::new(tokio::sync::Mutex::new(None));
    let pty_pair: Arc<tokio::sync::Mutex<Option<Box<dyn portable_pty::MasterPty + Send>>>> =
        Arc::new(tokio::sync::Mutex::new(None));

    // Receiver Loop
    while let Some(Ok(msg)) = receiver.next().await {
        if let Message::Text(text) = msg {
            if let Ok(req) = serde_json::from_str::<NexusRequest>(&text) {
                let ws_tx_req = ws_tx.clone();
                let agent = state.agent.clone();
                let pty_writer_clone = pty_writer.clone();
                let pty_pair_clone = pty_pair.clone();
                match req {
                    NexusRequest::Chat { message, editor_context } => {
                        // Reset stop flag before starting a new run
                        state.stop_flag.store(false, std::sync::atomic::Ordering::Relaxed);
                        let stop_flag = state.stop_flag.clone();
                        
                        // Inject editor context if user has a file open
                        let full_message = if let Some(ref path) = editor_context {
                            format!("[EDITOR] Active File: {}\n\n{}", path, message)
                        } else {
                            message
                        };
                        tokio::spawn(async move {
                            if let Err(e) = agent.run(full_message, stop_flag).await {
                                let res = NexusResponse::Error { message: e.to_string() };
                                let _ = ws_tx_req.send(Message::Text(serde_json::to_string(&res).unwrap().into())).await;
                            }
                            let _ = ws_tx_req.send(Message::Text(serde_json::to_string(&NexusResponse::Done).unwrap().into())).await;
                        });
                    }
                    NexusRequest::SafeModeApprove {} => {
                        if let Some(tx) = &state.tool_tx {
                            let _ = tx.send(crate::tui::ToolResponse::Text("yes".to_string())).await;
                        }
                    }
                    NexusRequest::SafeModeReject {} => {
                        if let Some(tx) = &state.tool_tx {
                            let _ = tx.send(crate::tui::ToolResponse::Text("no".to_string())).await;
                        }
                    }
                    NexusRequest::StopStream {} => {
                        state.stop_flag.store(true, std::sync::atomic::Ordering::Relaxed);
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
                        if std::fs::write(&path, content).is_ok() {
                            let res = NexusResponse::Done;
                            let _ = ws_tx_req.send(Message::Text(serde_json::to_string(&res).unwrap().into())).await;
                        } else {
                            let res = NexusResponse::Error { message: format!("Failed to write to {}", path) };
                            let _ = ws_tx_req.send(Message::Text(serde_json::to_string(&res).unwrap().into())).await;
                        }
                    }
                    NexusRequest::CreateFile { path } => {
                        // Create parent dirs if needed, then create empty file
                        if let Some(parent) = std::path::Path::new(&path).parent() {
                            let _ = std::fs::create_dir_all(parent);
                        }
                        match std::fs::File::create(&path) {
                            Ok(_) => {
                                let res = NexusResponse::Done;
                                let _ = ws_tx_req.send(Message::Text(serde_json::to_string(&res).unwrap().into())).await;
                            }
                            Err(e) => {
                                let res = NexusResponse::Error { message: format!("Failed to create file: {}", e) };
                                let _ = ws_tx_req.send(Message::Text(serde_json::to_string(&res).unwrap().into())).await;
                            }
                        }
                    }
                    NexusRequest::CreateFolder { path } => {
                        match std::fs::create_dir_all(&path) {
                            Ok(_) => {
                                let res = NexusResponse::Done;
                                let _ = ws_tx_req.send(Message::Text(serde_json::to_string(&res).unwrap().into())).await;
                            }
                            Err(e) => {
                                let res = NexusResponse::Error { message: format!("Failed to create folder: {}", e) };
                                let _ = ws_tx_req.send(Message::Text(serde_json::to_string(&res).unwrap().into())).await;
                            }
                        }
                    }
                    NexusRequest::DeleteItem { path } => {
                        let target = std::path::Path::new(&path);
                        let result = if target.is_dir() {
                            std::fs::remove_dir_all(target)
                        } else {
                            std::fs::remove_file(target)
                        };
                        match result {
                            Ok(_) => {
                                let res = NexusResponse::Done;
                                let _ = ws_tx_req.send(Message::Text(serde_json::to_string(&res).unwrap().into())).await;
                            }
                            Err(e) => {
                                let res = NexusResponse::Error { message: format!("Failed to delete: {}", e) };
                                let _ = ws_tx_req.send(Message::Text(serde_json::to_string(&res).unwrap().into())).await;
                            }
                        }
                    }
                    NexusRequest::RenameItem { old_path, new_path } => {
                        match std::fs::rename(&old_path, &new_path) {
                            Ok(_) => {
                                let res = NexusResponse::Done;
                                let _ = ws_tx_req.send(Message::Text(serde_json::to_string(&res).unwrap().into())).await;
                            }
                            Err(e) => {
                                let res = NexusResponse::Error { message: format!("Failed to rename: {}", e) };
                                let _ = ws_tx_req.send(Message::Text(serde_json::to_string(&res).unwrap().into())).await;
                            }
                        }
                    }
                    NexusRequest::TerminalSpawn {} => {
                        eprintln!("🖥️  [NEXUS]: TerminalSpawn received, opening PTY...");
                        let pty_system = native_pty_system();
                        let pair = pty_system.openpty(PtySize {
                            rows: 24,
                            cols: 80,
                            pixel_width: 0,
                            pixel_height: 0,
                        });

                        match pair {
                            Ok(pair) => {
                                let mut cmd = CommandBuilder::new("zsh");
                                cmd.arg("-l"); // Login shell
                                cmd.cwd(std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("/")));
                                cmd.env("TERM", "xterm-256color");
                                cmd.env("COLORTERM", "truecolor");
                                if let Ok(home) = std::env::var("HOME") {
                                    cmd.env("HOME", &home);
                                }
                                if let Ok(path) = std::env::var("PATH") {
                                    cmd.env("PATH", &path);
                                }

                                let mut child = match pair.slave.spawn_command(cmd) {
                                    Ok(child) => child,
                                    Err(e) => {
                                        let res = NexusResponse::Error { message: format!("Failed to spawn PTY: {}", e) };
                                        let _ = ws_tx_req.send(Message::Text(serde_json::to_string(&res).unwrap().into())).await;
                                        continue;
                                    }
                                };

                                // Drop the slave — critical! The master won't receive
                                // output until the slave side is closed in our process.
                                drop(pair.slave);

                                // Get reader and writer from master
                                let mut reader = pair.master.try_clone_reader().unwrap();
                                let writer = pair.master.take_writer().unwrap();

                                // Store the writer for future TerminalInput messages
                                {
                                    let mut w = pty_writer_clone.lock().await;
                                    *w = Some(writer);
                                }
                                // Store the master for resize
                                {
                                    let mut p = pty_pair_clone.lock().await;
                                    *p = Some(pair.master);
                                }

                                // Spawn reader task — reads PTY stdout and sends to WebSocket
                                let ws_tx_pty = ws_tx.clone();
                                std::thread::spawn(move || {
                                    let mut buf = [0u8; 4096];
                                    loop {
                                        match reader.read(&mut buf) {
                                            Ok(0) => break,
                                            Ok(n) => {
                                                let data = String::from_utf8_lossy(&buf[..n]).to_string();
                                                let res = NexusResponse::TerminalOutput { data };
                                                if let Ok(json) = serde_json::to_string(&res) {
                                                    let _ = ws_tx_pty.blocking_send(Message::Text(json.into()));
                                                }
                                            }
                                            Err(_) => break,
                                        }
                                    }
                                });

                                // Spawn child waiter (so it doesn't become a zombie)
                                std::thread::spawn(move || {
                                    let _ = child.wait();
                                });

                                eprintln!("🖥️  [NEXUS]: PTY spawned successfully");
                                let res = NexusResponse::Done;
                                let _ = ws_tx_req.send(Message::Text(serde_json::to_string(&res).unwrap().into())).await;
                            }
                            Err(e) => {
                                let res = NexusResponse::Error { message: format!("Failed to open PTY: {}", e) };
                                let _ = ws_tx_req.send(Message::Text(serde_json::to_string(&res).unwrap().into())).await;
                            }
                        }
                    }
                    NexusRequest::TerminalInput { data } => {
                        let mut w = pty_writer_clone.lock().await;
                        if let Some(ref mut writer) = *w {
                            let _ = writer.write_all(data.as_bytes());
                            let _ = writer.flush();
                        }
                    }
                    NexusRequest::TerminalResize { cols, rows } => {
                        let mut p = pty_pair_clone.lock().await;
                        if let Some(ref mut master) = *p {
                            let _ = master.resize(PtySize {
                                rows,
                                cols,
                                pixel_width: 0,
                                pixel_height: 0,
                            });
                        }
                    }
                    NexusRequest::SearchFiles { query, path } => {
                        tokio::spawn(async move {
                            let search_path = if path.is_empty() || path == "." { "." } else { &path };
                            let mut matches = Vec::new();
                            
                            // Use grep for fast search
                            let output = std::process::Command::new("grep")
                                .args(["-rnI", "--include=*.rs", "--include=*.ts", "--include=*.js", 
                                       "--include=*.json", "--include=*.toml", "--include=*.css",
                                       "--include=*.html", "--include=*.py", "--include=*.sh",
                                       "--include=*.md", "--include=*.yaml", "--include=*.yml",
                                       "--include=*.zig", "--include=*.nix", "--include=*.c",
                                       "--include=*.cpp", "--include=*.h",
                                       &query, search_path])
                                .output();

                            if let Ok(output) = output {
                                let stdout = String::from_utf8_lossy(&output.stdout);
                                for line in stdout.lines().take(100) {
                                    // Format: file:line:content
                                    let parts: Vec<&str> = line.splitn(3, ':').collect();
                                    if parts.len() == 3 {
                                        if let Ok(line_num) = parts[1].parse::<u32>() {
                                            matches.push(SearchMatch {
                                                file: parts[0].to_string(),
                                                line: line_num,
                                                content: parts[2].trim().to_string(),
                                            });
                                        }
                                    }
                                }
                            }

                            let res = NexusResponse::SearchResults { matches };
                            let _ = ws_tx_req.send(Message::Text(serde_json::to_string(&res).unwrap().into())).await;
                        });
                    }
                }
            } else {
                eprintln!("⚠️ [NEXUS]: Failed to parse NexusRequest: {}", text);
            }
        }
    }
}
async fn static_handler(uri: axum::http::Uri) -> impl IntoResponse {
    let path = uri.path().trim_start_matches('/');
    
    // Default to index.html if path is empty
    let path = if path.is_empty() { "index.html" } else { path };
    
    match WEB_DIR.get_file(path) {
        Some(file) => {
            let mime = mime_guess::from_path(path).first_or_octet_stream();
            (
                axum::http::StatusCode::OK,
                [(axum::http::header::CONTENT_TYPE, mime.as_ref())],
                file.contents(),
            ).into_response()
        }
        None => {
            // SPA Fallback: If file not found, serve index.html
            if let Some(index) = WEB_DIR.get_file("index.html") {
                (
                    axum::http::StatusCode::OK,
                    [(axum::http::header::CONTENT_TYPE, "text/html")],
                    index.contents(),
                ).into_response()
            } else {
                axum::http::StatusCode::NOT_FOUND.into_response()
            }
        }
    }
}
