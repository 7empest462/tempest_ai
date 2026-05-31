use miette::{Result, IntoDiagnostic, miette};
use async_trait::async_trait;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};
use portable_pty::{CommandBuilder, PtySize, native_pty_system, MasterPty};
use std::io::{Read, Write};
use schemars::JsonSchema;
use serde::Deserialize;
use ollama_rs::generation::tools::{ToolInfo, ToolFunctionInfo, ToolType};
use super::{AgentTool, ToolContext};
use uuid::Uuid;
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

// =========================================================================
// TerminalSpawnTool
// =========================================================================

#[derive(Deserialize, JsonSchema)]
pub struct TerminalSpawnArgs {
    /// Optional custom shell to spawn (e.g., "zsh", "bash", "python"). Defaults to the user's default shell.
    pub shell: Option<String>,
}

pub struct TerminalSpawnTool;

#[async_trait]
impl AgentTool for TerminalSpawnTool {
    fn name(&self) -> &'static str { "terminal_spawn" }
    fn description(&self) -> &'static str { "Spawns a new interactive pseudo-terminal (PTY) session. Returns a session_id. Use this to maintain persistent shell sessions, run REPLs, or execute commands that require state to be maintained between calls." }
    fn is_modifying(&self) -> bool { true }
    
    fn tool_info(&self) -> ToolInfo {
        let mut settings = schemars::generate::SchemaSettings::draft07();
        settings.inline_subschemas = true;
        let payload = settings.into_generator().into_root_schema_for::<TerminalSpawnArgs>();
        
        ToolInfo {
            tool_type: ToolType::Function,
            function: ToolFunctionInfo {
                name: self.name().to_string(),
                description: self.description().to_string(),
                parameters: payload.into(),
            }
        }
    }

    async fn execute(&self, args: &Value, _context: ToolContext) -> Result<String> {
        let typed_args: TerminalSpawnArgs = serde_json::from_value(args.clone()).into_diagnostic()?;
        
        let shell = typed_args.shell.unwrap_or_else(|| {
            std::env::var("SHELL").unwrap_or_else(|_| "zsh".to_string())
        });

        let pty_system = native_pty_system();
        let pair = pty_system.openpty(PtySize {
            rows: 24,
            cols: 80,
            pixel_width: 0,
            pixel_height: 0,
        }).map_err(|e| miette!("{}", e))?;

        let mut cmd = CommandBuilder::new(&shell);
        if shell.ends_with("zsh") || shell.ends_with("bash") {
            cmd.arg("-l"); // Login shell
        }
        
        cmd.cwd(std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("/")));
        cmd.env("TERM", "xterm-256color");
        cmd.env("COLORTERM", "truecolor");
        
        if let Ok(home) = std::env::var("HOME") { cmd.env("HOME", &home); }
        if let Ok(path) = std::env::var("PATH") { cmd.env("PATH", &path); }

        let mut child = pair.slave.spawn_command(cmd).map_err(|e| miette!("{}", e))?;
        
        // Dropping slave ensures the master reads EOF when child exits.
        drop(pair.slave);

        let mut reader = pair.master.try_clone_reader().map_err(|e| miette!("{}", e))?;
        let writer = pair.master.take_writer().map_err(|e| miette!("{}", e))?;

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
                        // Filter out basic ANSI escape codes to make it easier for the AI to read
                        // We use a simple regex or just string replacement for the most common ones.
                        let clean_text = text;
                            
                        if let Ok(mut lock) = buffer_clone.lock() {
                            lock.push_str(&clean_text);
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

        terminal_registry().lock().map_err(|_| miette!("Terminal registry poisoned"))?
            .insert(session_id.clone(), session);

        // Wait a tiny bit for the initial shell prompt to appear
        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
        
        Ok(format!("Terminal spawned successfully with session_id: {}", session_id))
    }
}

// =========================================================================
// TerminalInputTool
// =========================================================================

#[derive(Deserialize, JsonSchema)]
pub struct TerminalInputArgs {
    /// The session_id returned by terminal_spawn.
    pub session_id: String,
    /// The input string or command to send to the terminal. IMPORTANT: You must include a trailing newline (`\n`) if you want to execute a command!
    pub input: String,
    /// Optional delay in milliseconds to wait for output after sending the input. Default is 500ms. Increase this for long-running commands.
    pub wait_ms: Option<u64>,
}

pub struct TerminalInputTool;

#[async_trait]
impl AgentTool for TerminalInputTool {
    fn name(&self) -> &'static str { "terminal_input" }
    fn description(&self) -> &'static str { "Sends text input to an active terminal session and returns the resulting output. Don't forget to add a trailing '\\n' to execute a command." }
    fn is_modifying(&self) -> bool { true }
    
    fn tool_info(&self) -> ToolInfo {
        let mut settings = schemars::generate::SchemaSettings::draft07();
        settings.inline_subschemas = true;
        let payload = settings.into_generator().into_root_schema_for::<TerminalInputArgs>();
        
        ToolInfo {
            tool_type: ToolType::Function,
            function: ToolFunctionInfo {
                name: self.name().to_string(),
                description: self.description().to_string(),
                parameters: payload.into(),
            }
        }
    }

    async fn execute(&self, args: &Value, _context: ToolContext) -> Result<String> {
        let typed_args: TerminalInputArgs = serde_json::from_value(args.clone()).into_diagnostic()?;
        let id = &typed_args.session_id;
        
        let wait_ms = typed_args.wait_ms.unwrap_or(500);

        let output_buffer = {
            let mut registry = terminal_registry().lock().map_err(|_| miette!("Registry poisoned"))?;
            let session = registry.get_mut(id).ok_or_else(|| miette!("Invalid session_id: {}", id))?;
            
            // Clear buffer BEFORE sending input so we only capture new output
            if let Ok(mut buf) = session.output_buffer.lock() {
                buf.clear();
            }

            session.writer.write_all(typed_args.input.as_bytes()).into_diagnostic()?;
            session.writer.flush().into_diagnostic()?;
            
            Arc::clone(&session.output_buffer)
        };

        // Wait for output to accumulate
        tokio::time::sleep(tokio::time::Duration::from_millis(wait_ms)).await;

        let output = {
            let mut buf = output_buffer.lock().map_err(|_| miette!("Buffer poisoned"))?;
            let result = buf.clone();
            buf.clear(); // Clear it so subsequent reads are fresh
            result
        };

        if output.is_empty() {
            Ok("Command sent, but no output was received within the wait time. Use terminal_read to check later.".to_string())
        } else {
            Ok(output)
        }
    }
}

// =========================================================================
// TerminalReadTool
// =========================================================================

#[derive(Deserialize, JsonSchema)]
pub struct TerminalReadArgs {
    /// The session_id to read from.
    pub session_id: String,
}

pub struct TerminalReadTool;

#[async_trait]
impl AgentTool for TerminalReadTool {
    fn name(&self) -> &'static str { "terminal_read" }
    fn description(&self) -> &'static str { "Reads any accumulated output from a terminal session without sending new input." }
    
    fn tool_info(&self) -> ToolInfo {
        let mut settings = schemars::generate::SchemaSettings::draft07();
        settings.inline_subschemas = true;
        let payload = settings.into_generator().into_root_schema_for::<TerminalReadArgs>();
        
        ToolInfo {
            tool_type: ToolType::Function,
            function: ToolFunctionInfo {
                name: self.name().to_string(),
                description: self.description().to_string(),
                parameters: payload.into(),
            }
        }
    }

    async fn execute(&self, args: &Value, _context: ToolContext) -> Result<String> {
        let typed_args: TerminalReadArgs = serde_json::from_value(args.clone()).into_diagnostic()?;
        let id = &typed_args.session_id;
        
        let output = {
            let mut registry = terminal_registry().lock().map_err(|_| miette!("Registry poisoned"))?;
            let session = registry.get_mut(id).ok_or_else(|| miette!("Invalid session_id: {}", id))?;
            
            let mut buf = session.output_buffer.lock().map_err(|_| miette!("Buffer poisoned"))?;
            let result = buf.clone();
            buf.clear();
            result
        };

        if output.is_empty() {
            Ok("No new output available.".to_string())
        } else {
            Ok(output)
        }
    }
}

// =========================================================================
// TerminalCloseTool
// =========================================================================

#[derive(Deserialize, JsonSchema)]
pub struct TerminalCloseArgs {
    /// The session_id to close.
    pub session_id: String,
}

pub struct TerminalCloseTool;

#[async_trait]
impl AgentTool for TerminalCloseTool {
    fn name(&self) -> &'static str { "terminal_close" }
    fn description(&self) -> &'static str { "Closes an active terminal session." }
    fn is_modifying(&self) -> bool { true }
    
    fn tool_info(&self) -> ToolInfo {
        let mut settings = schemars::generate::SchemaSettings::draft07();
        settings.inline_subschemas = true;
        let payload = settings.into_generator().into_root_schema_for::<TerminalCloseArgs>();
        
        ToolInfo {
            tool_type: ToolType::Function,
            function: ToolFunctionInfo {
                name: self.name().to_string(),
                description: self.description().to_string(),
                parameters: payload.into(),
            }
        }
    }

    async fn execute(&self, args: &Value, _context: ToolContext) -> Result<String> {
        let typed_args: TerminalCloseArgs = serde_json::from_value(args.clone()).into_diagnostic()?;
        
        let mut registry = terminal_registry().lock().map_err(|_| miette!("Registry poisoned"))?;
        if registry.remove(&typed_args.session_id).is_some() {
            Ok(format!("Terminal session {} closed successfully.", typed_args.session_id))
        } else {
            Err(miette!("Invalid session_id: {}", typed_args.session_id))
        }
    }
}
