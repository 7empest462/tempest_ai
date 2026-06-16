use crate::agent::Agent;
use axum::{
    Router,
    extract::ws::{Message, WebSocket, WebSocketUpgrade},
    response::IntoResponse,
    routing::get,
};
use futures::{sink::SinkExt, stream::StreamExt};
use include_dir::{Dir, include_dir};
use ollama_rs::generation::chat::{ChatMessage, MessageRole};
use portable_pty::{CommandBuilder, PtySize, native_pty_system};
use serde::{Deserialize, Serialize};
use std::io::{Read, Write};
use std::sync::Arc;
use sysinfo::System;
use tower_http::cors::CorsLayer;

static WEB_DIR: Dir<'_> = include_dir!("tempest-web/dist");

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type", content = "payload")]
pub enum NexusRequest {
    Chat {
        message: String,
        #[serde(default)]
        editor_context: Option<String>,
    },
    ListFiles {
        path: String,
    },
    ReadFile {
        path: String,
    },
    WriteFile {
        path: String,
        content: String,
    },
    CreateFile {
        path: String,
    },
    CreateFolder {
        path: String,
    },
    DeleteItem {
        path: String,
    },
    RenameItem {
        old_path: String,
        new_path: String,
    },
    TerminalSpawn {},
    TerminalInput {
        data: String,
    },
    TerminalResize {
        cols: u16,
        rows: u16,
    },
    SearchFiles {
        query: String,
        path: String,
    },
    SafeModeApprove {},
    SafeModeReject {},
    StopStream {},
    AskUserResponse {
        answer: String,
    },
    GetHistory {},
    RollbackHistory {
        user_message_index: usize,
    },
    GetMemories {},
    ReviewApprove {},
    ReviewReject {},
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type", content = "payload")]
pub enum NexusResponse {
    Token {
        text: String,
    },
    Done,
    FileTree {
        items: Vec<FileItem>,
        current_path: String,
    },
    FileContent {
        path: String,
        content: String,
    },
    Telemetry {
        cpu: f32,
        gpu: f32,
        ram: String,
    },
    Error {
        message: String,
    },
    TerminalOutput {
        data: String,
    },
    SearchResults {
        matches: Vec<SearchMatch>,
    },
    BackendInfo {
        backend: String,
        planner: String,
        executor: String,
        verifier: String,
    },
    AgentStateChange {
        state: String,
    },
    ActiveTools {
        tools: Vec<String>,
    },
    SafeModeRequest {
        rationale: String,
        diff: String,
    },
    TaskUpdate {
        task: String,
    },
    ReasoningToken {
        token: String,
    },
    StreamToken {
        token: String,
    },
    InferenceMetrics {
        tps: Option<u64>,
        ctx_used: Option<usize>,
        ctx_total: Option<u64>,
    },
    SentinelLog {
        sentinel: String,
        message: String,
    },
    ToolResult {
        name: String,
        args: Option<String>,
        output: Option<String>,
        success: bool,
    },
    AskUserRequest {
        question: String,
    },
    History {
        messages: Vec<WebChatMessage>,
    },
    Memories {
        memories: Vec<WebMemoryItem>,
    },
    ToolStart {
        name: String,
        args: Option<String>,
    },
    TurnReviewRequest {
        diff: String,
        files: Vec<WebFileDiff>,
    },
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct WebMemoryItem {
    pub topic: String,
    pub content: String,
    pub tags: Option<String>,
    pub updated_at: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct WebFileDiff {
    pub path: String,
    pub original: String,
    pub modified: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct WebChatMessage {
    pub id: String,
    pub role: String, // "system" | "ai" | "user"
    pub content: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reasoning: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tools: Option<Vec<WebToolCallResult>>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct WebToolCallResult {
    pub name: String,
    pub args: Option<String>,
    pub output: Option<String>,
    pub success: bool,
}

pub fn reconstruct_web_history(history: &[ChatMessage]) -> Vec<WebChatMessage> {
    let mut web_msgs = Vec::new();
    let mut current_ai_msg: Option<WebChatMessage> = None;

    for (idx, msg) in history.iter().enumerate() {
        match msg.role {
            MessageRole::User => {
                if let Some(ai_msg) = current_ai_msg.take() {
                    web_msgs.push(ai_msg);
                }
                web_msgs.push(WebChatMessage {
                    id: format!("user-{}", idx),
                    role: "user".to_string(),
                    content: msg.content.clone(),
                    reasoning: None,
                    tools: None,
                });
            }
            MessageRole::Assistant => {
                let mut content = msg.content.clone();
                let mut reasoning = msg.thinking.clone();

                if reasoning.is_none()
                    && let Some(start) = content.find("<think>")
                    && let Some(end) = content[start..].find("</think>")
                {
                    let absolute_end = start + end;
                    let extracted_reasoning = content[start + 7..absolute_end].trim().to_string();
                    reasoning = Some(extracted_reasoning);
                    content = (content[..start].to_string() + &content[absolute_end + 8..])
                        .trim()
                        .to_string();
                }

                let mut tools = Vec::new();
                for tool_call in &msg.tool_calls {
                    tools.push(WebToolCallResult {
                        name: tool_call.function.name.clone(),
                        args: Some(tool_call.function.arguments.to_string()),
                        output: None,
                        success: true,
                    });
                }

                if let Some(ref mut ai_msg) = current_ai_msg {
                    if !content.is_empty() {
                        if ai_msg.content.is_empty() {
                            ai_msg.content = content;
                        } else {
                            ai_msg.content.push('\n');
                            ai_msg.content.push_str(&content);
                        }
                    }
                    if let Some(r) = reasoning {
                        if let Some(ref mut existing_r) = ai_msg.reasoning {
                            existing_r.push('\n');
                            existing_r.push_str(&r);
                        } else {
                            ai_msg.reasoning = Some(r);
                        }
                    }
                    if !tools.is_empty() {
                        let existing_tools = ai_msg.tools.get_or_insert_with(Vec::new);
                        existing_tools.extend(tools);
                    }
                } else {
                    current_ai_msg = Some(WebChatMessage {
                        id: format!("ai-{}", idx),
                        role: "ai".to_string(),
                        content,
                        reasoning,
                        tools: if tools.is_empty() { None } else { Some(tools) },
                    });
                }
            }
            MessageRole::System | MessageRole::Tool => {
                let content = &msg.content;
                let is_observation = content.starts_with("=== SYSTEM OBSERVATION ===")
                    || content.starts_with("=== SYSTEM ERROR ===");

                if is_observation {
                    let is_success = content.starts_with("=== SYSTEM OBSERVATION ===");
                    let lines: Vec<&str> = content.lines().collect();
                    let mut tool_name = "";
                    let mut tool_output = String::new();

                    if lines.len() >= 3 {
                        if let Some(stripped) = lines[1].strip_prefix("Tool: ") {
                            tool_name = stripped.trim();
                        }
                        let mut in_result = false;
                        for line in &lines[2..] {
                            if let Some(stripped) = line.strip_prefix("Result: ") {
                                tool_output.push_str(stripped);
                                in_result = true;
                            } else if let Some(stripped) = line.strip_prefix("Error: ") {
                                tool_output.push_str(stripped);
                                in_result = true;
                            } else if in_result {
                                if line.trim()
                                    == "(Verify this data against your plan and proceed to the next step.)"
                                    || line
                                        .trim()
                                        .starts_with("Please analyze this error carefully")
                                {
                                    break;
                                }
                                tool_output.push('\n');
                                tool_output.push_str(line);
                            }
                        }
                    }

                    if let Some(ref mut ai_msg) = current_ai_msg {
                        if let Some(ref mut tools) = ai_msg.tools {
                            if let Some(t) = tools
                                .iter_mut()
                                .find(|t| t.name == tool_name && t.output.is_none())
                            {
                                t.output = Some(tool_output.trim().to_string());
                                t.success = is_success;
                            } else {
                                tools.push(WebToolCallResult {
                                    name: tool_name.to_string(),
                                    args: None,
                                    output: Some(tool_output.trim().to_string()),
                                    success: is_success,
                                });
                            }
                        } else {
                            ai_msg.tools = Some(vec![WebToolCallResult {
                                name: tool_name.to_string(),
                                args: None,
                                output: Some(tool_output.trim().to_string()),
                                success: is_success,
                            }]);
                        }
                    }
                } else {
                    if let Some(ai_msg) = current_ai_msg.take() {
                        web_msgs.push(ai_msg);
                    }
                    web_msgs.push(WebChatMessage {
                        id: format!("system-{}", idx),
                        role: "system".to_string(),
                        content: content.clone(),
                        reasoning: None,
                        tools: None,
                    });
                }
            }
        }
    }

    if let Some(ai_msg) = current_ai_msg {
        web_msgs.push(ai_msg);
    }

    web_msgs
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
    pub pending_review_checkpoints: std::sync::Arc<tokio::sync::Mutex<Option<usize>>>,
}

pub async fn run_nexus(
    agent: Agent,
    port: u16,
    backend_id: String,
    event_rx: Option<tokio::sync::mpsc::Receiver<crate::tui::AgentEvent>>,
    tool_tx: Option<tokio::sync::mpsc::Sender<crate::tui::ToolResponse>>,
) {
    let (b_tx, _b_rx) = tokio::sync::broadcast::channel(4096);

    // When VRAM time-sharing is active, all three phases are pinned to the
    // unified primary model. Override the model names so the web UI shows the
    // actual model being used rather than the dormant 3-tier config values.
    let (p_model, e_model, v_model, effective_backend_id) = if agent.vram_time_sharing {
        let unified = agent.get_model();
        (
            unified.clone(),
            unified.clone(),
            unified,
            format!("{} (VRAM Sharing)", backend_id),
        )
    } else {
        (
            agent.planner_model.clone().unwrap_or_default(),
            agent.executor_model.clone().unwrap_or_default(),
            agent.verifier_model.clone().unwrap_or_default(),
            backend_id,
        )
    };

    let state = Arc::new(NexusState {
        planner_model: p_model,
        executor_model: e_model,
        verifier_model: v_model,
        agent,
        backend_id: effective_backend_id,
        broadcast_tx: b_tx.clone(),
        tool_tx,
        stop_flag: std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),
        pending_review_checkpoints: std::sync::Arc::new(tokio::sync::Mutex::new(None)),
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
                        } else if id == "ask_user" {
                            let res = NexusResponse::AskUserRequest { question: prompt };
                            if let Ok(json) = serde_json::to_string(&res) {
                                let _ = b_tx_clone.send(json);
                            }
                        }
                    }
                    crate::tui::AgentEvent::RequestPrivileges { rationale, .. } => {
                        let res = NexusResponse::SafeModeRequest {
                            rationale: format!("Privilege Escalation Requested: {}", rationale),
                            diff: "PERMISSION REQUEST".to_string(),
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
                            let res = NexusResponse::InferenceMetrics {
                                tps,
                                ctx_used: None,
                                ctx_total: None,
                            };
                            if let Ok(json) = serde_json::to_string(&res) {
                                let _ = b_tx_clone.send(json);
                            }
                        }
                    }
                    crate::tui::AgentEvent::ContextStatus { used, total } => {
                        let res = NexusResponse::InferenceMetrics {
                            tps: None,
                            ctx_used: Some(used),
                            ctx_total: Some(total),
                        };
                        if let Ok(json) = serde_json::to_string(&res) {
                            let _ = b_tx_clone.send(json);
                        }
                    }
                    crate::tui::AgentEvent::SentinelUpdate { active, log } => {
                        // Only forward to WebSocket when a sentinel actually triggered
                        if !log.is_empty() {
                            let sentinel_name = active
                                .first()
                                .cloned()
                                .unwrap_or_else(|| "Unknown".to_string());
                            let res = NexusResponse::SentinelLog {
                                sentinel: sentinel_name,
                                message: log,
                            };
                            if let Ok(json) = serde_json::to_string(&res) {
                                let _ = b_tx_clone.send(json);
                            }
                        }
                    }
                    crate::tui::AgentEvent::ToolSuccess { name, args, output } => {
                        let res = NexusResponse::ToolResult {
                            name,
                            args,
                            output,
                            success: true,
                        };
                        if let Ok(json) = serde_json::to_string(&res) {
                            let _ = b_tx_clone.send(json);
                        }
                    }
                    crate::tui::AgentEvent::ToolError { name, error, args } => {
                        let res = NexusResponse::ToolResult {
                            name,
                            args,
                            output: Some(error),
                            success: false,
                        };
                        if let Ok(json) = serde_json::to_string(&res) {
                            let _ = b_tx_clone.send(json);
                        }
                    }
                    crate::tui::AgentEvent::ToolStart { name, args } => {
                        let res = NexusResponse::ToolStart { name, args };
                        if let Ok(json) = serde_json::to_string(&res) {
                            let _ = b_tx_clone.send(json);
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
    let (ws_tx, mut ws_rx) = tokio::sync::mpsc::channel::<Message>(4096);

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
        loop {
            match broadcast_rx.recv().await {
                Ok(json) => {
                    let _ = ws_tx_broadcast.send(Message::Text(json.into())).await;
                }
                Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => continue,
                Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
            }
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

    // Send initial chat history if available
    let history = state.agent.history.lock().clone();
    let web_msgs = reconstruct_web_history(&history);
    let history_res = NexusResponse::History { messages: web_msgs };
    if let Ok(json) = serde_json::to_string(&history_res) {
        let _ = ws_tx.send(Message::Text(json.into())).await;
    }

    // Telemetry Task
    let ws_tx_tele = ws_tx.clone();
    tokio::spawn(async move {
        let mut sys = System::new_all();
        loop {
            sys.refresh_cpu_all();
            sys.refresh_memory();
            let cpu = sys.global_cpu_usage();
            let used_ram = sys.used_memory() / 1024 / 1024;
            let total_ram = sys.total_memory() / 1024 / 1024;

            let gpu = {
                #[cfg(target_os = "macos")]
                {
                    tokio::task::spawn_blocking(|| {
                        tempest_monitor::macos_helper::get_macos_gpu_info(false).usage_pct as f32
                    })
                    .await
                    .unwrap_or(0.0)
                }
                #[cfg(target_os = "linux")]
                {
                    tokio::task::spawn_blocking(|| {
                        tempest_monitor::linux_helper::collect_gpu_telemetry().usage_pct as f32
                    })
                    .await
                    .unwrap_or(0.0)
                }
                #[cfg(not(any(target_os = "macos", target_os = "linux")))]
                {
                    0.0
                }
            };

            let res = NexusResponse::Telemetry {
                cpu,
                gpu,
                ram: format!("{}/{} MB", used_ram, total_ram),
            };
            if let Ok(json) = serde_json::to_string(&res)
                && ws_tx_tele.send(Message::Text(json.into())).await.is_err()
            {
                break;
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
                    NexusRequest::Chat {
                        message,
                        editor_context,
                    } => {
                        // Abort any previous active runs gracefully by setting the stop flag
                        state
                            .stop_flag
                            .store(true, std::sync::atomic::Ordering::Relaxed);
                        // Wait a brief moment to allow active stream and execution loops to exit cleanly
                        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
                        // Reset stop flag for the new run
                        state
                            .stop_flag
                            .store(false, std::sync::atomic::Ordering::Relaxed);
                        let stop_flag = state.stop_flag.clone();

                        // Inject editor context if user has a file open
                        let full_message = if let Some(ref path) = editor_context {
                            format!("[EDITOR] Active File: {}\n\n{}", path, message)
                        } else {
                            message
                        };
                        let ws_tx_clone = ws_tx_req.clone();
                        let pending_review_checkpoints_clone =
                            state.pending_review_checkpoints.clone();
                        tokio::spawn(async move {
                            let initial_cp = agent.checkpoint_mgr.lock().checkpoint_count();
                            let run_result = agent.run(full_message, stop_flag).await;
                            let final_cp = agent.checkpoint_mgr.lock().checkpoint_count();
                            let cp_diff = final_cp.saturating_sub(initial_cp);

                            if let Err(e) = run_result {
                                let res = NexusResponse::Error {
                                    message: e.to_string(),
                                };
                                let _ = ws_tx_clone
                                    .send(Message::Text(
                                        serde_json::to_string(&res).unwrap().into(),
                                    ))
                                    .await;
                            }

                            if cp_diff > 0 {
                                // Store the diff count in state so we know how many checkpoints to pop if rejected
                                {
                                    let mut pending = pending_review_checkpoints_clone.lock().await;
                                    *pending = Some(cp_diff);
                                }

                                // Generate review details: original content, modified content, paths, diff text
                                let mods =
                                    agent.checkpoint_mgr.lock().get_turn_modifications(cp_diff);
                                let mut files = Vec::new();
                                for (path, original, current) in mods {
                                    files.push(WebFileDiff {
                                        path: path.to_string_lossy().to_string(),
                                        original: original.unwrap_or_default(),
                                        modified: current.unwrap_or_default(),
                                    });
                                }

                                let mut diff_mods = Vec::new();
                                for file_diff in &files {
                                    diff_mods.push((
                                        std::path::PathBuf::from(&file_diff.path),
                                        file_diff.modified.clone(),
                                    ));
                                }
                                let diff_preview =
                                    crate::checkpoint::generate_batch_diff(&diff_mods);

                                let res = NexusResponse::TurnReviewRequest {
                                    diff: diff_preview,
                                    files,
                                };
                                let _ = ws_tx_clone
                                    .send(Message::Text(
                                        serde_json::to_string(&res).unwrap().into(),
                                    ))
                                    .await;
                            } else {
                                let _ = ws_tx_clone
                                    .send(Message::Text(
                                        serde_json::to_string(&NexusResponse::Done).unwrap().into(),
                                    ))
                                    .await;
                            }
                        });
                    }
                    NexusRequest::SafeModeApprove {} => {
                        if let Some(tx) = &state.tool_tx {
                            let _ = tx
                                .send(crate::tui::ToolResponse::Text("yes".to_string()))
                                .await;
                        }
                    }
                    NexusRequest::SafeModeReject {} => {
                        if let Some(tx) = &state.tool_tx {
                            let _ = tx
                                .send(crate::tui::ToolResponse::Text("no".to_string()))
                                .await;
                        }
                    }
                    NexusRequest::StopStream {} => {
                        state
                            .stop_flag
                            .store(true, std::sync::atomic::Ordering::Relaxed);
                    }
                    NexusRequest::AskUserResponse { answer } => {
                        if let Some(tx) = &state.tool_tx {
                            let _ = tx.send(crate::tui::ToolResponse::Text(answer)).await;
                        }
                    }
                    NexusRequest::ListFiles { path } => {
                        let dir_path = if path.is_empty() || path == "." {
                            "."
                        } else {
                            &path
                        };
                        let mut items = Vec::new();

                        let current_path = std::fs::canonicalize(dir_path)
                            .unwrap_or_else(|_| std::path::PathBuf::from(dir_path))
                            .to_string_lossy()
                            .to_string();

                        if let Ok(entries) = std::fs::read_dir(dir_path) {
                            for entry in entries.flatten() {
                                if let Ok(name) = entry.file_name().into_string()
                                    && !name.starts_with('.')
                                {
                                    items.push(FileItem {
                                        name,
                                        is_dir: entry.path().is_dir(),
                                    });
                                }
                            }
                        }
                        let res = NexusResponse::FileTree {
                            items,
                            current_path,
                        };
                        let _ = ws_tx_req
                            .send(Message::Text(serde_json::to_string(&res).unwrap().into()))
                            .await;
                    }
                    NexusRequest::ReadFile { path } => match std::fs::read_to_string(&path) {
                        Ok(content) => {
                            let res = NexusResponse::FileContent {
                                path: path.clone(),
                                content,
                            };
                            let _ = ws_tx_req
                                .send(Message::Text(serde_json::to_string(&res).unwrap().into()))
                                .await;
                        }
                        Err(e) => {
                            let res = NexusResponse::Error {
                                message: e.to_string(),
                            };
                            let _ = ws_tx_req
                                .send(Message::Text(serde_json::to_string(&res).unwrap().into()))
                                .await;
                        }
                    },
                    NexusRequest::WriteFile { path, content } => {
                        if std::fs::write(&path, content).is_ok() {
                            let res = NexusResponse::Done;
                            let _ = ws_tx_req
                                .send(Message::Text(serde_json::to_string(&res).unwrap().into()))
                                .await;
                        } else {
                            let res = NexusResponse::Error {
                                message: format!("Failed to write to {}", path),
                            };
                            let _ = ws_tx_req
                                .send(Message::Text(serde_json::to_string(&res).unwrap().into()))
                                .await;
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
                                let _ = ws_tx_req
                                    .send(Message::Text(
                                        serde_json::to_string(&res).unwrap().into(),
                                    ))
                                    .await;
                            }
                            Err(e) => {
                                let res = NexusResponse::Error {
                                    message: format!("Failed to create file: {}", e),
                                };
                                let _ = ws_tx_req
                                    .send(Message::Text(
                                        serde_json::to_string(&res).unwrap().into(),
                                    ))
                                    .await;
                            }
                        }
                    }
                    NexusRequest::CreateFolder { path } => match std::fs::create_dir_all(&path) {
                        Ok(_) => {
                            let res = NexusResponse::Done;
                            let _ = ws_tx_req
                                .send(Message::Text(serde_json::to_string(&res).unwrap().into()))
                                .await;
                        }
                        Err(e) => {
                            let res = NexusResponse::Error {
                                message: format!("Failed to create folder: {}", e),
                            };
                            let _ = ws_tx_req
                                .send(Message::Text(serde_json::to_string(&res).unwrap().into()))
                                .await;
                        }
                    },
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
                                let _ = ws_tx_req
                                    .send(Message::Text(
                                        serde_json::to_string(&res).unwrap().into(),
                                    ))
                                    .await;
                            }
                            Err(e) => {
                                let res = NexusResponse::Error {
                                    message: format!("Failed to delete: {}", e),
                                };
                                let _ = ws_tx_req
                                    .send(Message::Text(
                                        serde_json::to_string(&res).unwrap().into(),
                                    ))
                                    .await;
                            }
                        }
                    }
                    NexusRequest::RenameItem { old_path, new_path } => {
                        match std::fs::rename(&old_path, &new_path) {
                            Ok(_) => {
                                let res = NexusResponse::Done;
                                let _ = ws_tx_req
                                    .send(Message::Text(
                                        serde_json::to_string(&res).unwrap().into(),
                                    ))
                                    .await;
                            }
                            Err(e) => {
                                let res = NexusResponse::Error {
                                    message: format!("Failed to rename: {}", e),
                                };
                                let _ = ws_tx_req
                                    .send(Message::Text(
                                        serde_json::to_string(&res).unwrap().into(),
                                    ))
                                    .await;
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
                                cmd.cwd(
                                    std::env::current_dir()
                                        .unwrap_or_else(|_| std::path::PathBuf::from("/")),
                                );
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
                                        let res = NexusResponse::Error {
                                            message: format!("Failed to spawn PTY: {}", e),
                                        };
                                        let _ = ws_tx_req
                                            .send(Message::Text(
                                                serde_json::to_string(&res).unwrap().into(),
                                            ))
                                            .await;
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
                                                let data =
                                                    String::from_utf8_lossy(&buf[..n]).to_string();
                                                let res = NexusResponse::TerminalOutput { data };
                                                if let Ok(json) = serde_json::to_string(&res) {
                                                    let _ = ws_tx_pty
                                                        .blocking_send(Message::Text(json.into()));
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
                                let _ = ws_tx_req
                                    .send(Message::Text(
                                        serde_json::to_string(&res).unwrap().into(),
                                    ))
                                    .await;
                            }
                            Err(e) => {
                                let res = NexusResponse::Error {
                                    message: format!("Failed to open PTY: {}", e),
                                };
                                let _ = ws_tx_req
                                    .send(Message::Text(
                                        serde_json::to_string(&res).unwrap().into(),
                                    ))
                                    .await;
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
                            let search_path = if path.is_empty() || path == "." {
                                "."
                            } else {
                                &path
                            };
                            let mut matches = Vec::new();

                            // Use grep for fast search
                            let output = std::process::Command::new("grep")
                                .args([
                                    "-rnI",
                                    "--include=*.rs",
                                    "--include=*.ts",
                                    "--include=*.js",
                                    "--include=*.json",
                                    "--include=*.toml",
                                    "--include=*.css",
                                    "--include=*.html",
                                    "--include=*.py",
                                    "--include=*.sh",
                                    "--include=*.md",
                                    "--include=*.yaml",
                                    "--include=*.yml",
                                    "--include=*.zig",
                                    "--include=*.nix",
                                    "--include=*.c",
                                    "--include=*.cpp",
                                    "--include=*.h",
                                    &query,
                                    search_path,
                                ])
                                .output();

                            if let Ok(output) = output {
                                let stdout = String::from_utf8_lossy(&output.stdout);
                                for line in stdout.lines().take(100) {
                                    // Format: file:line:content
                                    let parts: Vec<&str> = line.splitn(3, ':').collect();
                                    if parts.len() == 3
                                        && let Ok(line_num) = parts[1].parse::<u32>()
                                    {
                                        matches.push(SearchMatch {
                                            file: parts[0].to_string(),
                                            line: line_num,
                                            content: parts[2].trim().to_string(),
                                        });
                                    }
                                }
                            }

                            let res = NexusResponse::SearchResults { matches };
                            let _ = ws_tx_req
                                .send(Message::Text(serde_json::to_string(&res).unwrap().into()))
                                .await;
                        });
                    }
                    NexusRequest::GetHistory {} => {
                        let history = agent.history.lock().clone();
                        let web_msgs = reconstruct_web_history(&history);
                        let res = NexusResponse::History { messages: web_msgs };
                        if let Ok(json) = serde_json::to_string(&res) {
                            let _ = ws_tx_req.send(Message::Text(json.into())).await;
                        }
                    }
                    NexusRequest::RollbackHistory { user_message_index } => {
                        let success = {
                            let mut h_lock = agent.history.lock();
                            let mut user_count = 0;
                            let mut target_idx = None;
                            for (idx, msg) in h_lock.iter().enumerate() {
                                if msg.role == MessageRole::User {
                                    if user_count == user_message_index {
                                        target_idx = Some(idx);
                                        break;
                                    }
                                    user_count += 1;
                                }
                            }

                            if let Some(idx) = target_idx {
                                h_lock.truncate(idx + 1);
                                true
                            } else {
                                false
                            }
                        };

                        if success {
                            if let Some(raw_hist_arc) =
                                agent.backend.try_read().ok().and_then(|b| b.raw_history())
                            {
                                let mut raw_hist = raw_hist_arc.lock();
                                let mut raw_user_count = 0;
                                let mut raw_target_idx = None;
                                for (raw_idx, msg_val) in raw_hist.iter().enumerate() {
                                    if msg_val.get("role").and_then(|r| r.as_str()) == Some("user")
                                    {
                                        if raw_user_count == user_message_index {
                                            raw_target_idx = Some(raw_idx);
                                            break;
                                        }
                                        raw_user_count += 1;
                                    }
                                }
                                if let Some(raw_idx) = raw_target_idx {
                                    raw_hist.truncate(raw_idx + 1);
                                }
                            }

                            let _ = agent.save_history();

                            let history = agent.history.lock().clone();
                            let web_msgs = reconstruct_web_history(&history);
                            let res = NexusResponse::History { messages: web_msgs };
                            if let Ok(json) = serde_json::to_string(&res) {
                                let _ = ws_tx_req.send(Message::Text(json.into())).await;
                            }
                        }
                    }
                    NexusRequest::GetMemories {} => {
                        let items = {
                            let m_store = agent.memory_store.lock();
                            m_store.list_all()
                        };
                        match items {
                            Ok(items) => {
                                let web_items: Vec<WebMemoryItem> = items
                                    .into_iter()
                                    .map(|record| WebMemoryItem {
                                        topic: record.topic,
                                        content: record.content,
                                        tags: record.tags,
                                        updated_at: record.updated_at,
                                    })
                                    .collect();
                                let res = NexusResponse::Memories {
                                    memories: web_items,
                                };
                                if let Ok(json) = serde_json::to_string(&res) {
                                    let _ = ws_tx_req.send(Message::Text(json.into())).await;
                                }
                            }
                            Err(e) => {
                                let res = NexusResponse::Error {
                                    message: format!("Failed to read memories: {}", e),
                                };
                                let _ = ws_tx_req
                                    .send(Message::Text(
                                        serde_json::to_string(&res).unwrap().into(),
                                    ))
                                    .await;
                            }
                        }
                    }
                    NexusRequest::ReviewApprove {} => {
                        let mut pending = state.pending_review_checkpoints.lock().await;
                        *pending = None;
                        let res = NexusResponse::Done;
                        let _ = ws_tx_req
                            .send(Message::Text(serde_json::to_string(&res).unwrap().into()))
                            .await;
                    }
                    NexusRequest::ReviewReject {} => {
                        let mut pending = state.pending_review_checkpoints.lock().await;
                        if let Some(cp_count) = pending.take() {
                            let mut cp_mgr = agent.checkpoint_mgr.lock();
                            for _ in 0..cp_count {
                                if let Err(e) = cp_mgr.undo() {
                                    eprintln!("Error during review undo: {}", e);
                                }
                            }
                        }
                        let res = NexusResponse::Done;
                        let _ = ws_tx_req
                            .send(Message::Text(serde_json::to_string(&res).unwrap().into()))
                            .await;
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
            )
                .into_response()
        }
        None => {
            // SPA Fallback: If file not found, serve index.html
            if let Some(index) = WEB_DIR.get_file("index.html") {
                (
                    axum::http::StatusCode::OK,
                    [(axum::http::header::CONTENT_TYPE, "text/html")],
                    index.contents(),
                )
                    .into_response()
            } else {
                axum::http::StatusCode::NOT_FOUND.into_response()
            }
        }
    }
}
