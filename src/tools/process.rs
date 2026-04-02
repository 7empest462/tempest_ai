use anyhow::Result;
use async_trait::async_trait;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};
use std::process::{Command, Stdio};
use std::io::{Read, BufReader};
use super::{AgentTool, ToolContext};

type ProcessLogs = Arc<Mutex<String>>;

fn process_registry() -> &'static Mutex<HashMap<String, ProcessLogs>> {
    static REGISTRY: OnceLock<Mutex<HashMap<String, ProcessLogs>>> = OnceLock::new();
    REGISTRY.get_or_init(|| Mutex::new(HashMap::new()))
}

pub struct RunBackgroundTool;

#[async_trait]
impl AgentTool for RunBackgroundTool {
    fn name(&self) -> &'static str { "run_background" }
    fn description(&self) -> &'static str { "Spawns a long-running bash/zsh command in the background (like starting a web server). Returns a process_id immediately. Use read_process_logs to check its output." }
    fn requires_confirmation(&self) -> bool { true }
    fn is_modifying(&self) -> bool { true }
    fn parameters(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "command": { "type": "string", "description": "The command string to execute in the background." }
            },
            "required": ["command"]
        })
    }
    async fn execute(&self, args: &Value, _context: ToolContext) -> Result<String> {
        let cmd = args.get("command").and_then(|c| c.as_str()).ok_or_else(|| anyhow::anyhow!("Missing 'command' argument"))?;

        let current_path = std::env::var("PATH").unwrap_or_default();
        let new_path = format!("/opt/homebrew/bin:/usr/local/bin:{}", current_path);

        let mut child = Command::new("sh")
            .env("PATH", new_path)
            .arg("-c")
            .arg(cmd)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?;

        let process_id = child.id().to_string();
        
        // Setup shared log buffer
        let logs = Arc::new(Mutex::new(String::new()));
        
        let stdout = child.stdout.take().expect("Failed to open stdout");
        let stderr = child.stderr.take().expect("Failed to open stderr");
        
        let logs_clone1 = Arc::clone(&logs);
        std::thread::spawn(move || {
            let mut reader = BufReader::new(stdout);
            let mut buf = [0; 1024];
            while let Ok(n) = reader.read(&mut buf) {
                if n == 0 { break; }
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
                if n == 0 { break; }
                if let Ok(mut l) = logs_clone2.lock() {
                    l.push_str(&String::from_utf8_lossy(&buf[..n]));
                }
            }
        });

        process_registry().lock().unwrap().insert(process_id.clone(), logs);

        Ok(format!("Background process spawned successfully with ID: {}", process_id))
    }
}

pub struct ReadProcessLogsTool;

#[async_trait]
impl AgentTool for ReadProcessLogsTool {
    fn name(&self) -> &'static str { "read_process_logs" }
    fn description(&self) -> &'static str { "Reads the stdout and stderr of a background process using its process_id." }
    fn parameters(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "process_id": { "type": "string", "description": "The ID returned by run_background." }
            },
            "required": ["process_id"]
        })
    }
    
    async fn execute(&self, args: &Value, _context: ToolContext) -> Result<String> {
        let pid = args.get("process_id").and_then(|p| p.as_str()).ok_or_else(|| anyhow::anyhow!("Missing 'process_id' argument"))?;

        let registry = process_registry().lock().unwrap();
        if let Some(logs) = registry.get(pid) {
            let log_text = logs.lock().unwrap().clone();
            if log_text.is_empty() {
                Ok("Process has produced no output yet.".to_string())
            } else {
                let max_len = 4000;
                if log_text.len() > max_len {
                    let safe_start = log_text.char_indices().rev().nth(max_len).map(|(i, _)| i).unwrap_or(0);
                    Ok(format!("...[truncated]...\n{}", &log_text[safe_start..]))
                } else {
                    Ok(log_text)
                }
            }
        } else {
            Ok(format!("Error: No background process found with ID '{}'", pid))
        }
    }
}

pub struct KillProcessTool;

#[async_trait]
impl AgentTool for KillProcessTool {
    fn name(&self) -> &'static str { "kill_process" }
    fn description(&self) -> &'static str { "Kill a running background process by its process ID." }
    fn requires_confirmation(&self) -> bool { true }
    fn is_modifying(&self) -> bool { true }
    fn parameters(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "pid": { "type": "string", "description": "Process ID to kill" },
                "signal": { "type": "string", "description": "Signal to send (default: TERM). Options: TERM, KILL, INT" }
            },
            "required": ["pid"]
        })
    }

    async fn execute(&self, args: &Value, _context: ToolContext) -> Result<String> {
        let pid = args.get("pid").and_then(|p| p.as_str()).ok_or_else(|| anyhow::anyhow!("Missing 'pid'"))?;
        let signal = args.get("signal").and_then(|s| s.as_str()).unwrap_or("TERM");

        let output = std::process::Command::new("kill")
            .args([&format!("-{}", signal), pid])
            .output()?;
        
        if output.status.success() {
            // Also cleanup registry if it was a background process we were tracking
            process_registry().lock().unwrap().remove(pid);
            Ok(format!("✅ Sent {} signal to process {}", signal, pid))
        } else {
            let err = String::from_utf8_lossy(&output.stderr);
            Ok(format!("❌ Failed to kill process {}: {}", pid, err.trim()))
        }
    }
}
pub struct WatchDirectoryTool;

#[async_trait]
impl AgentTool for WatchDirectoryTool {
    fn name(&self) -> &'static str { "watch_directory" }
    fn description(&self) -> &'static str { "Starts a persistent background daemon that watches a directory for file modifications. When you make changes to files, it will instantly run the 'trigger_command' provided." }
    fn is_modifying(&self) -> bool { true }
    fn parameters(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "path": { "type": "string", "description": "The directory path to recursively watch." },
                "trigger_command": { "type": "string", "description": "The bash command to run whenever a file changes." }
            },
            "required": ["path", "trigger_command"]
        })
    }

    async fn execute(&self, args: &Value, _context: ToolContext) -> Result<String> {
        use notify::Watcher;
        
        let path_str = args.get("path").and_then(|p| p.as_str()).ok_or_else(|| anyhow::anyhow!("Missing 'path'"))?;
        let path = shellexpand::tilde(path_str).to_string();
        let cmd = args.get("trigger_command").and_then(|c| c.as_str()).ok_or_else(|| anyhow::anyhow!("Missing 'trigger_command'"))?.to_string();

        let success_msg = format!("Successfully spawned File-Watching Daemon on directory: '{}'. It will automatically execute '{}' upon any file modifications.", path, cmd);

        std::thread::spawn(move || {
            let (tx, rx) = std::sync::mpsc::channel();
            
            let mut watcher = match notify::recommended_watcher(tx) {
                Ok(w) => w,
                Err(e) => {
                    eprintln!("Failed to initialize watcher: {}", e);
                    return;
                }
            };

            if let Err(e) = watcher.watch(std::path::Path::new(&path), notify::RecursiveMode::Recursive) {
                eprintln!("Failed to watch path {}: {}", path, e);
                return;
            }

            let mut last_trigger = std::time::Instant::now();

            loop {
                match rx.recv() {
                    Ok(Ok(event)) => {
                        if let notify::EventKind::Modify(_) = event.kind {
                            if last_trigger.elapsed() > std::time::Duration::from_millis(1500) {
                                let _ = std::process::Command::new("sh")
                                    .arg("-c")
                                    .arg(&cmd)
                                    .current_dir(&path)
                                    .spawn();
                                last_trigger = std::time::Instant::now();
                            }
                        }
                    },
                    Ok(Err(e)) => eprintln!("Watch error: {:?}", e),
                    Err(_) => break,
                }
            }
        });

        Ok(success_msg)
    }
}
