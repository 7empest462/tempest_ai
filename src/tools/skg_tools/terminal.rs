// ==========================================
// 🖥️ SKG TERMINAL TOOLS — Native Skelegent Implementations
// ==========================================
// Replaces the legacy AgentTool terminal tools.

use skg_tool::{ToolCallContext, ToolError, ToolDyn};
use skg_tool_macro::skg_tool;
use portable_pty::{CommandBuilder, MasterPty, PtySize, native_pty_system};
use uuid::Uuid;
use std::collections::HashMap;
use std::io::{Read, Write};
use std::sync::{Arc, Mutex, OnceLock};
use std::thread;

pub struct TerminalSession {
    pub writer: Box<dyn Write + Send>,
    pub master: Box<dyn MasterPty + Send>,
    pub output_buffer: Arc<Mutex<String>>,
}

fn terminal_registry() -> &'static Mutex<HashMap<String, TerminalSession>> {
    static REGISTRY: OnceLock<Mutex<HashMap<String, TerminalSession>>> = OnceLock::new();
    REGISTRY.get_or_init(|| Mutex::new(HashMap::new()))
}

// ── terminal_spawn ─────────────────────────────────────────────────────────────

#[skg_tool(
    name = "terminal_spawn",
    description = "Spawns a new interactive pseudo-terminal (PTY) session. Returns a session_id. Use this to maintain persistent shell sessions, run REPLs, or execute commands that require state to be maintained between calls."
)]
pub async fn terminal_spawn(
    shell: Option<String>,
) -> Result<serde_json::Value, ToolError> {
    let shell_cmd = shell
        .unwrap_or_else(|| std::env::var("SHELL").unwrap_or_else(|_| "zsh".to_string()));

    let pty_system = native_pty_system();
    let pair = pty_system
        .openpty(PtySize {
            rows: 24,
            cols: 80,
            pixel_width: 0,
            pixel_height: 0,
        })
        .map_err(|e| ToolError::ExecutionFailed(format!("Failed to open PTY: {}", e)))?;

    let mut cmd = CommandBuilder::new(&shell_cmd);
    if shell_cmd.ends_with("zsh") || shell_cmd.ends_with("bash") {
        cmd.arg("-l"); // Login shell
    }

    cmd.cwd(std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("/")));
    cmd.env("TERM", "xterm-256color");
    cmd.env("COLORTERM", "truecolor");

    if let Ok(home) = std::env::var("HOME") {
        cmd.env("HOME", &home);
    }
    if let Ok(path) = std::env::var("PATH") {
        cmd.env("PATH", &path);
    }

    let mut child = pair
        .slave
        .spawn_command(cmd)
        .map_err(|e| ToolError::ExecutionFailed(format!("Failed to spawn shell: {}", e)))?;

    // Dropping slave ensures the master reads EOF when child exits.
    drop(pair.slave);

    let mut reader = pair
        .master
        .try_clone_reader()
        .map_err(|e| ToolError::ExecutionFailed(format!("Failed to clone reader: {}", e)))?;
    let writer = pair.master.take_writer().map_err(|e| {
        ToolError::ExecutionFailed(format!("Failed to take PTY writer: {}", e))
    })?;

    let output_buffer = Arc::new(Mutex::new(String::new()));
    let buffer_clone = Arc::clone(&output_buffer);

    // Spawn reader thread
    thread::spawn(move || {
        let mut buf = [0u8; 4096];
        loop {
            match reader.read(&mut buf) {
                Ok(0) => break, // EOF
                Ok(n) => {
                    let text = String::from_utf8_lossy(&buf[..n]).to_string();
                    if let Ok(mut lock) = buffer_clone.lock() {
                        lock.push_str(&text);
                    }
                }
                Err(_) => break, // Error or closed
            }
        }
    });

    // Spawn waiter thread
    thread::spawn(move || {
        let _ = child.wait();
    });

    let session_id = Uuid::new_v4().to_string();

    let session = TerminalSession {
        writer,
        master: pair.master,
        output_buffer,
    };

    terminal_registry()
        .lock()
        .map_err(|_| ToolError::ExecutionFailed("Terminal registry poisoned".to_string()))?
        .insert(session_id.clone(), session);

    // Wait a tiny bit for the initial shell prompt to appear
    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

    Ok(serde_json::Value::String(format!(
        "Terminal spawned successfully with session_id: {}",
        session_id
    )))
}

// ── terminal_input ─────────────────────────────────────────────────────────────
// Note: We manually implement ToolDyn for TerminalInputTool to bypass the
// skg-tool-macro parameter shadowing bug with parameters named "input".

pub struct TerminalInputTool;

impl TerminalInputTool {
    pub fn new() -> Self {
        Self
    }
}

impl Default for TerminalInputTool {
    fn default() -> Self {
        Self::new()
    }
}

impl ToolDyn for TerminalInputTool {
    fn name(&self) -> &str {
        "terminal_input"
    }

    fn description(&self) -> &str {
        "Sends text input to an active terminal session and returns the resulting output. Don't forget to add a trailing '\\n' to execute a command."
    }

    fn input_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "session_id": {
                    "type": "string"
                },
                "input": {
                    "type": "string"
                },
                "wait_ms": {
                    "type": "integer"
                }
            },
            "required": ["session_id", "input"]
        })
    }

    fn call(
        &self,
        input_val: serde_json::Value,
        _ctx: &ToolCallContext,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<serde_json::Value, ToolError>> + Send + '_>>
    {
        Box::pin(async move {
            let session_id: String = serde_json::from_value(
                input_val.get("session_id").cloned().unwrap_or(serde_json::Value::Null)
            ).map_err(|e| ToolError::InvalidInput(e.to_string()))?;

            let input: String = serde_json::from_value(
                input_val.get("input").cloned().unwrap_or(serde_json::Value::Null)
            ).map_err(|e| ToolError::InvalidInput(e.to_string()))?;

            let wait_ms: Option<u64> = serde_json::from_value(
                input_val.get("wait_ms").cloned().unwrap_or(serde_json::Value::Null)
            ).map_err(|e| ToolError::InvalidInput(e.to_string()))?;

            terminal_input_impl(session_id, input, wait_ms).await
        })
    }
}

async fn terminal_input_impl(
    session_id: String,
    input: String,
    wait_ms: Option<u64>,
) -> Result<serde_json::Value, ToolError> {
    let wait_ms = wait_ms.unwrap_or(500);

    let output_buffer = {
        let mut registry = terminal_registry()
            .lock()
            .map_err(|_| ToolError::ExecutionFailed("Registry poisoned".to_string()))?;
        let session = registry
            .get_mut(&session_id)
            .ok_or_else(|| ToolError::ExecutionFailed(format!("Invalid session_id: {}", session_id)))?;

        // Clear buffer BEFORE sending input so we only capture new output
        if let Ok(mut buf) = session.output_buffer.lock() {
            buf.clear();
        }

        session
            .writer
            .write_all(input.as_bytes())
            .map_err(|e| ToolError::ExecutionFailed(format!("Failed to write PTY input: {}", e)))?;
        session
            .writer
            .flush()
            .map_err(|e| ToolError::ExecutionFailed(format!("Failed to flush PTY writer: {}", e)))?;

        Arc::clone(&session.output_buffer)
    };

    // Wait for output to accumulate
    tokio::time::sleep(tokio::time::Duration::from_millis(wait_ms)).await;

    let output = {
        let mut buf = output_buffer
            .lock()
            .map_err(|_| ToolError::ExecutionFailed("Buffer poisoned".to_string()))?;
        let result = buf.clone();
        buf.clear(); // Clear it so subsequent reads are fresh
        result
    };

    if output.is_empty() {
        Ok(serde_json::Value::String(
            "Command sent, but no output was received within the wait time. Use terminal_read to check later.".to_string()
        ))
    } else {
        Ok(serde_json::Value::String(output))
    }
}

// ── terminal_read ──────────────────────────────────────────────────────────────

#[skg_tool(
    name = "terminal_read",
    description = "Reads any accumulated output from a terminal session without sending new input."
)]
pub async fn terminal_read(
    session_id: String,
) -> Result<serde_json::Value, ToolError> {
    let output = {
        let mut registry = terminal_registry()
            .lock()
            .map_err(|_| ToolError::ExecutionFailed("Registry poisoned".to_string()))?;
        let session = registry
            .get_mut(&session_id)
            .ok_or_else(|| ToolError::ExecutionFailed(format!("Invalid session_id: {}", session_id)))?;

        let mut buf = session
            .output_buffer
            .lock()
            .map_err(|_| ToolError::ExecutionFailed("Buffer poisoned".to_string()))?;
        let result = buf.clone();
        buf.clear();
        result
    };

    if output.is_empty() {
        Ok(serde_json::Value::String("No new output available.".to_string()))
    } else {
        Ok(serde_json::Value::String(output))
    }
}

// ── terminal_close ─────────────────────────────────────────────────────────────

#[skg_tool(
    name = "terminal_close",
    description = "Closes an active terminal session."
)]
pub async fn terminal_close(
    session_id: String,
) -> Result<serde_json::Value, ToolError> {
    let mut registry = terminal_registry()
        .lock()
        .map_err(|_| ToolError::ExecutionFailed("Registry poisoned".to_string()))?;

    if registry.remove(&session_id).is_some() {
        Ok(serde_json::Value::String(format!(
            "Terminal session {} closed successfully.",
            session_id
        )))
    } else {
        Err(ToolError::ExecutionFailed(format!(
            "Invalid session_id: {}",
            session_id
        )))
    }
}
