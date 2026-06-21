// ==========================================
// ⚙️ SKG PROCESS TOOLS — Native Skelegent Implementations
// ==========================================
// Replaces the legacy AgentTool process tools.

use skg_tool::ToolError;
use skg_tool_macro::skg_tool;
use std::collections::HashMap;
use std::io::{BufReader, Read};
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex, OnceLock};

type ProcessLogs = Arc<Mutex<String>>;

fn process_registry() -> &'static Mutex<HashMap<String, ProcessLogs>> {
    static REGISTRY: OnceLock<Mutex<HashMap<String, ProcessLogs>>> = OnceLock::new();
    REGISTRY.get_or_init(|| Mutex::new(HashMap::new()))
}

// ── run_background ─────────────────────────────────────────────────────────────

#[skg_tool(
    name = "run_background",
    description = "Spawns a long-running bash/zsh command in the background (like starting a web server). Returns a process_id immediately. Use read_process_logs to check its output."
)]
pub async fn run_background(command: String) -> Result<serde_json::Value, ToolError> {
    let current_path = std::env::var("PATH").unwrap_or_default();
    let new_path = format!("/opt/homebrew/bin:/usr/local/bin:{}", current_path);

    let mut child = Command::new("sh")
        .env("PATH", new_path)
        .arg("-c")
        .arg(&command)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| ToolError::ExecutionFailed(format!("Failed to spawn process: {}", e)))?;

    let process_id = child.id().to_string();

    // Setup shared log buffer
    let logs = Arc::new(Mutex::new(String::new()));

    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| ToolError::ExecutionFailed("Failed to open stdout".to_string()))?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| ToolError::ExecutionFailed("Failed to open stderr".to_string()))?;

    let logs_clone1 = Arc::clone(&logs);
    std::thread::spawn(move || {
        let mut reader = BufReader::new(stdout);
        let mut buf = [0; 1024];
        while let Ok(n) = reader.read(&mut buf) {
            if n == 0 {
                break;
            }
            if let Ok(mut l) = logs_clone1.lock() {
                l.push_str(&String::from_utf8_lossy(&buf[..n]));
            }
        }
    });

    let logs_clone2 = Arc::clone(&logs);
    std::thread::spawn(move || {
        let mut reader = BufReader::new(stderr);
        let mut buf = [0; 1024];
        while let Ok(n) = reader.read(&mut buf) {
            if n == 0 {
                break;
            }
            if let Ok(mut l) = logs_clone2.lock() {
                l.push_str(&String::from_utf8_lossy(&buf[..n]));
            }
        }
    });

    process_registry()
        .lock()
        .map_err(|_| ToolError::ExecutionFailed("Registry Poisoned".to_string()))?
        .insert(process_id.clone(), logs);

    Ok(serde_json::Value::String(format!(
        "Background process spawned successfully with ID: {}",
        process_id
    )))
}

// ── read_process_logs ──────────────────────────────────────────────────────────

#[skg_tool(
    name = "read_process_logs",
    description = "Reads the stdout and stderr of a background process using its process_id."
)]
pub async fn read_process_logs(process_id: String) -> Result<serde_json::Value, ToolError> {
    let registry = process_registry()
        .lock()
        .map_err(|_| ToolError::ExecutionFailed("Registry Poisoned".to_string()))?;

    if let Some(logs) = registry.get(&process_id) {
        let log_text = logs
            .lock()
            .map_err(|_| ToolError::ExecutionFailed("Logs Poisoned".to_string()))?
            .clone();

        if log_text.is_empty() {
            Ok(serde_json::Value::String(
                "Process has produced no output yet.".to_string(),
            ))
        } else {
            let max_len = 4000;
            if log_text.len() > max_len {
                let safe_start = log_text
                    .char_indices()
                    .rev()
                    .nth(max_len)
                    .map(|(i, _)| i)
                    .unwrap_or(0);
                Ok(serde_json::Value::String(format!(
                    "...[truncated]...\n{}",
                    &log_text[safe_start..]
                )))
            } else {
                Ok(serde_json::Value::String(log_text))
            }
        }
    } else {
        Ok(serde_json::Value::String(format!(
            "Error: No background process found with ID '{}'",
            process_id
        )))
    }
}

// ── kill_process ───────────────────────────────────────────────────────────────

#[skg_tool(
    name = "kill_process",
    description = "Kill a running background process by its process ID."
)]
pub async fn kill_process(
    pid: String,
    signal: Option<String>,
) -> Result<serde_json::Value, ToolError> {
    let signal_owned = signal.unwrap_or_else(|| "TERM".to_string());
    let signal_str = signal_owned.as_str();

    let output = std::process::Command::new("kill")
        .args([&format!("-{}", signal_str), &pid])
        .output()
        .map_err(|e| ToolError::ExecutionFailed(format!("Failed to run kill command: {}", e)))?;

    if output.status.success() {
        process_registry()
            .lock()
            .map_err(|_| ToolError::ExecutionFailed("Registry Poisoned".to_string()))?
            .remove(&pid);
        Ok(serde_json::Value::String(format!(
            "✅ Sent {} signal to process {}",
            signal_str, pid
        )))
    } else {
        let err = String::from_utf8_lossy(&output.stderr);
        Ok(serde_json::Value::String(format!(
            "❌ Failed to kill process {}: {}",
            pid,
            err.trim()
        )))
    }
}

// ── watch_directory ────────────────────────────────────────────────────────────

#[skg_tool(
    name = "watch_directory",
    description = "Starts a persistent background daemon that watches a directory for file modifications. When you make changes to files, it will instantly run the 'trigger_command' provided."
)]
pub async fn watch_directory(
    path: String,
    trigger_command: String,
) -> Result<serde_json::Value, ToolError> {
    use notify::Watcher;

    let path_expanded = shellexpand::tilde(&path).to_string();
    let success_msg = format!(
        "Successfully spawned File-Watching Daemon on directory: '{}'. It will automatically execute '{}' upon any file modifications.",
        path_expanded, trigger_command
    );

    std::thread::spawn(move || {
        let (tx, rx) = std::sync::mpsc::channel();

        let mut watcher = match notify::recommended_watcher(tx) {
            Ok(w) => w,
            Err(e) => {
                eprintln!("Failed to initialize watcher: {}", e);
                return;
            }
        };

        if let Err(e) = watcher.watch(
            std::path::Path::new(&path_expanded),
            notify::RecursiveMode::Recursive,
        ) {
            eprintln!("Failed to watch path {}: {}", path_expanded, e);
            return;
        }

        let mut last_trigger = std::time::Instant::now();

        loop {
            match rx.recv() {
                Ok(Ok(event)) => {
                    if let notify::EventKind::Modify(_) = event.kind
                        && last_trigger.elapsed() > std::time::Duration::from_millis(1500)
                    {
                        let _ = std::process::Command::new("sh")
                            .arg("-c")
                            .arg(&trigger_command)
                            .current_dir(&path_expanded)
                            .spawn();
                        last_trigger = std::time::Instant::now();
                    }
                }
                Ok(Err(e)) => eprintln!("Watch error: {:?}", e),
                Err(_) => break,
            }
        }
    });

    Ok(serde_json::Value::String(success_msg))
}
