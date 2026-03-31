use anyhow::Result;
use colored::Colorize;
use serde_json::Value;
use std::process::Command;
use std::sync::{Arc, Mutex, OnceLock};
use std::collections::HashMap;
use std::{fs, path::PathBuf};
use crate::memory::MemoryStore;

type ProcessLogs = Arc<Mutex<String>>;

fn process_registry() -> &'static Mutex<HashMap<String, ProcessLogs>> {
    static REGISTRY: OnceLock<Mutex<HashMap<String, ProcessLogs>>> = OnceLock::new();
    REGISTRY.get_or_init(|| Mutex::new(HashMap::new()))
}

/// A trait representing an autonomous tool the agent can use.
#[allow(dead_code)]
#[async_trait::async_trait]
pub trait AgentTool: Send + Sync {
    fn name(&self) -> &'static str;
    fn description(&self) -> &'static str;
    async fn execute(&self, args: &Value, _agent_content: &str) -> Result<String>;
    
    /// Define the JSON schema for this tool's parameters.
    fn parameters(&self) -> Value;

    /// Whether this tool requires explicit human confirmation before executing.
    fn requires_confirmation(&self) -> bool {
        false
    }

    /// Whether this tool modifies the system state (files, processes, services).
    fn is_modifying(&self) -> bool {
        false
    }
}

pub struct RunCommandTool;

#[async_trait::async_trait]
impl AgentTool for RunCommandTool {
    fn name(&self) -> &'static str {
        "run_command"
    }

    fn description(&self) -> &'static str {
        "Executes a bash/zsh command on the host system. Useful for exploring directories, checking git status, running tests, or managing files."
    }

    fn requires_confirmation(&self) -> bool {
        true
    }
    fn is_modifying(&self) -> bool {
        true
    }

    fn parameters(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "command": {
                    "type": "string",
                    "description": "The command string to execute (e.g., 'ls -la' or 'cat Cargo.toml')."
                },
                "cwd": {
                    "type": "string",
                    "description": "Optional absolute path to use as the current working directory for the command."
                }
            },
            "required": ["command"]
        })
    }

    async fn execute(&self, args: &Value, _agent_content: &str) -> Result<String> {
        let cmd = args.get("command")
            .and_then(|c| c.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing 'command' argument"))?;

        let cwd = args.get("cwd").and_then(|c| c.as_str());

        if let Some(_c) = cwd {
            // println!(">> [TOOL CALL: run_command] Executing: {} (in {})", cmd, _c);
        } else {
            // println!(">> [TOOL CALL: run_command] Executing: {}", cmd);
        }
        
        let current_path = std::env::var("PATH").unwrap_or_default();
        let new_path = format!("/opt/homebrew/bin:/usr/local/bin:{}", current_path);
        
        let mut command = std::process::Command::new("sh");
        command.env("PATH", new_path)
               .arg("-c")
               .arg(cmd)
               .stdout(std::process::Stdio::piped())
               .stderr(std::process::Stdio::piped());

        if let Some(dir) = cwd {
            command.current_dir(dir);
        }

        let mut child = command.spawn()?;

        // Wait with timeout
        let timeout_duration = std::time::Duration::from_secs(15);
        let start_time = std::time::Instant::now();
        
        loop {
            if let Some(status) = child.try_wait()? {
                let output = child.wait_with_output()?;
                
                let mut out = String::from_utf8_lossy(&output.stdout).to_string();
                let err = String::from_utf8_lossy(&output.stderr).to_string();

                if !err.is_empty() {
                    out.push_str("\n--- STDERR ---\n");
                    out.push_str(&err);
                }
                
                // Truncate if output is too long to prevent context overflow
                let max_len = 4000;
                if out.len() > max_len {
                    let safe_len = out.char_indices().nth(max_len).map(|(i, _)| i).unwrap_or(out.len());
                    out.truncate(safe_len);
                    out.push_str("\n...[output truncated]...");
                }

                if out.is_empty() {
                    return Ok(format!("Command executed successfully with exit code: {}", status));
                } else {
                    return Ok(out);
                }
            }

            if start_time.elapsed() > timeout_duration {
                let _ = child.kill();
                return Ok(format!("Error: Command timed out after 15 seconds and was killed. Avoid using interactive commands like 'top' (use 'top -l 1' on macOS), 'nano', or 'less' which hang indefinitely."));
            }

            std::thread::sleep(std::time::Duration::from_millis(100));
        }
    }
}

pub struct ReadFileTool;

#[async_trait::async_trait]
impl AgentTool for ReadFileTool {
    fn name(&self) -> &'static str {
        "read_file"
    }

    fn description(&self) -> &'static str {
        "Reads the contents of a file at an absolute or relative path."
    }

    fn parameters(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "The path to the file."
                }
            },
            "required": ["path"]
        })
    }

    async fn execute(&self, args: &Value, _agent_content: &str) -> Result<String> {
        let path_str = args.get("path")
            .and_then(|p| p.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing 'path' argument"))?;
        let path_owned = shellexpand::tilde(path_str).to_string();
        let path = path_owned.as_str();

        // println!(">> [TOOL CALL: read_file] Reading: {}", path);
        let content = fs::read_to_string(path)?;
        Ok(content)
    }
}

pub struct WriteFileTool;

#[async_trait::async_trait]
impl AgentTool for WriteFileTool {
    fn name(&self) -> &'static str {
        "write_file"
    }

    fn description(&self) -> &'static str {
        "Overrides or creates a file with the given content."
    }

    fn requires_confirmation(&self) -> bool {
        true
    }
    fn is_modifying(&self) -> bool {
        true
    }

    fn parameters(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "The path to the file."
                },
                "content": {
                    "type": "string",
                    "description": "The full content to write to the file."
                }
            },
            "required": ["path", "content"]
        })
    }

    async fn execute(&self, args: &Value, _agent_content: &str) -> Result<String> {
        let path_str = args.get("path")
            .and_then(|p| p.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing 'path' argument"))?;
        let path_owned = shellexpand::tilde(path_str).to_string();
        let path = PathBuf::from(&path_owned);
        let absolute_path = match path.canonicalize() {
            Ok(p) => p,
            Err(_) => path.clone(),
        };
            
        let content = args.get("content")
            .and_then(|c| c.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing 'content' argument"))?;

        if content.contains("...rest of") || content.contains("left unchanged") || content.contains("... rest of") || content.contains("... existing code ...") {
            return Err(anyhow::anyhow!("[System Guardrail] CRITICAL ERROR: You attempted to write placeholder text (e.g. '...rest of file left unchanged...'). You are a machine executing a literal file-write. Placeholders will physically delete the user's code. You MUST provide the FULL, EXACT code. Re-evaluate and call the tool properly."));
        }

        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                if let Err(e) = fs::create_dir_all(parent) {
                    anyhow::bail!("Failed to create directory structure for {}: {}. Is the path writable?", parent.display(), e);
                }
            }
        }
        
        match fs::write(&path, content) {
            Ok(_) => Ok(format!("Successfully wrote {} bytes to {}", content.len(), absolute_path.display())),
            Err(e) => anyhow::bail!("OS ERROR: Failed to write to {}: {}. HINT: Check Folder Permissions / Full Disk Access on macOS.", absolute_path.display(), e),
        }
    }
}

pub struct ListDirTool;

#[async_trait::async_trait]
impl AgentTool for ListDirTool {
    fn name(&self) -> &'static str {
        "list_dir"
    }

    fn description(&self) -> &'static str {
        "Lists the contents of a directory. Use this to explore the file system."
    }

    fn parameters(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "The directory path to list."
                }
            },
            "required": ["path"]
        })
    }

    async fn execute(&self, args: &Value, _agent_content: &str) -> Result<String> {
        let path_str = args.get("path")
            .and_then(|p| p.as_str())
            .unwrap_or(".");
        let path_owned = shellexpand::tilde(path_str).to_string();
        let path = path_owned.as_str();
            
        // println!(">> [TOOL CALL: list_dir] Listing: {}", path);
        let mut result = String::new();
        match std::fs::read_dir(path) {
            Ok(entries) => {
                for entry in entries {
                    if let Ok(entry) = entry {
                        if let Ok(metadata) = entry.metadata() {
                            let kind = if metadata.is_dir() { "DIR" } else { "FILE" };
                            result.push_str(&format!("[{}] {}\n", kind, entry.file_name().to_string_lossy()));
                        }
                    }
                }
                if result.is_empty() {
                    Ok(format!("Directory '{}' is empty.", path))
                } else {
                    Ok(result)
                }
            }
            Err(e) => Ok(format!("Error listing directory '{}': {}", path, e))
        }
    }
}

pub struct SearchWebTool;

#[async_trait::async_trait]
impl AgentTool for SearchWebTool {
    fn name(&self) -> &'static str {
        "search_web"
    }

    fn description(&self) -> &'static str {
        "Searches the web using DuckDuckGo. Returns top organic results, including titles, snippets, and URLs."
    }

    fn parameters(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "query": {
                    "type": "string",
                    "description": "The search query."
                }
            },
            "required": ["query"]
        })
    }

    async fn execute(&self, args: &Value, _agent_content: &str) -> Result<String> {
        let query = args.get("query").and_then(|q| q.as_str()).unwrap_or("").to_string();
        // println!(">> [TOOL CALL: search_web] Query: {}", query);
        
        let url = "https://lite.duckduckgo.com/lite/";
        let client = reqwest::Client::builder()
            .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64)")
            .build()?;
            
        let res = client.post(url)
            .form(&[("q", &query)])
            .send().await?
            .text().await?;
            
        let document = scraper::Html::parse_document(&res);
        let tr_selector = scraper::Selector::parse("tr").unwrap();
        let a_selector = scraper::Selector::parse("a.result-link").unwrap();
        let snippet_selector = scraper::Selector::parse("td.result-snippet").unwrap();
        
        let mut results = String::new();
        let mut current_title = String::new();
        let mut current_url = String::new();
        
        for tr in document.select(&tr_selector) {
            if let Some(a) = tr.select(&a_selector).next() {
                current_title = a.text().collect::<Vec<_>>().join(" ").trim().to_string();
                current_url = a.value().attr("href").unwrap_or("").to_string();
                
                if current_url.contains("uddg?u=") {
                    if let Some(idx) = current_url.find("uddg?u=") {
                        let extracted = &current_url[idx + 7..];
                        let clean = if let Some(end_idx) = extracted.find('&') {
                            &extracted[..end_idx]
                        } else {
                            extracted
                        };
                        if let Ok(decoded) = urlencoding::decode(clean) {
                            current_url = decoded.to_string();
                        }
                    }
                } else if current_url.starts_with("//") {
                    current_url = format!("https:{}", current_url);
                } else if current_url.starts_with('/') {
                    current_url = format!("https://lite.duckduckgo.com{}", current_url);
                }
            } else if let Some(snippet) = tr.select(&snippet_selector).next() {
                let snip = snippet.text().collect::<Vec<_>>().join(" ").trim().to_string();
                if !current_title.is_empty() && !current_url.is_empty() {
                    results.push_str(&format!("Title: {}\nURL: {}\nSnippet: {}\n\n", current_title, current_url, snip));
                    current_title.clear();
                    current_url.clear();
                }
            }
        }
        
        if results.is_empty() {
            Ok(format!("No results found for query: {}", query))
        } else {
            Ok(results)
        }
    }
}

pub struct ReadUrlTool;

#[async_trait::async_trait]
impl AgentTool for ReadUrlTool {
    fn name(&self) -> &'static str {
        "read_url"
    }

    fn description(&self) -> &'static str {
        "Fetches a URL and converts the page HTML to readable markdown text. Use this to read documentation or articles from search results."
    }

    fn parameters(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "url": {
                    "type": "string",
                    "description": "The URL to fetch."
                }
            },
            "required": ["url"]
        })
    }
    
    async fn execute(&self, args: &Value, _agent_content: &str) -> Result<String> {
        let url = args.get("url").and_then(|u| u.as_str()).unwrap_or("").to_string();
        // println!(">> [TOOL CALL: read_url] Fetching: {}", url);
        
        let client = reqwest::Client::builder()
            .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36")
            .build()?;
            
        let res = client.get(&url).send().await?;
        let html_bytes = res.bytes().await?;
        
        // Use html2text to strip HTML tags and present clean text
        let text = html2text::from_read(html_bytes.as_ref(), 100);
        
        let max_len = 15000;
        let mut truncated = text;
        if truncated.len() > max_len {
            let safe_len = truncated.char_indices().nth(max_len).map(|(i, _)| i).unwrap_or(truncated.len());
            truncated.truncate(safe_len);
            truncated.push_str("\n...[Content truncated due to length]...");
        }
        
        Ok(truncated)
    }
}

pub struct PatchFileTool;

#[async_trait::async_trait]
impl AgentTool for PatchFileTool {
    fn name(&self) -> &'static str {
        "patch_file"
    }

    fn description(&self) -> &'static str {
        "Surgically replaces a specific range of lines in a file with new content. Lines are 1-indexed. Use this to edit large files without overwriting the whole file."
    }

    fn requires_confirmation(&self) -> bool {
        true
    }
    fn is_modifying(&self) -> bool {
        true
    }

    fn parameters(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "file_path": {
                    "type": "string",
                    "description": "The path to the file."
                },
                "start_line": {
                    "type": "integer",
                    "description": "1-indexed starting line number to replace."
                },
                "end_line": {
                    "type": "integer",
                    "description": "1-indexed ending line number to replace (inclusive)."
                },
                "content": {
                    "type": "string",
                    "description": "The exact new content to insert in place of the specified lines."
                }
            },
            "required": ["file_path", "start_line", "end_line", "content"]
        })
    }

    async fn execute(&self, args: &Value, _agent_content: &str) -> Result<String> {
        let path_str = args.get("file_path").and_then(|p| p.as_str()).ok_or_else(|| anyhow::anyhow!("Missing 'file_path' argument"))?;
        let path_owned = shellexpand::tilde(path_str).to_string();
        let path = path_owned.as_str();
        let start_line = args.get("start_line").and_then(|v| v.as_u64()).ok_or_else(|| anyhow::anyhow!("Missing 'start_line' argument"))? as usize;
        let end_line = args.get("end_line").and_then(|v| v.as_u64()).ok_or_else(|| anyhow::anyhow!("Missing 'end_line' argument"))? as usize;
        let content = args.get("content").and_then(|_c| _c.as_str()).ok_or_else(|| anyhow::anyhow!("Missing 'content' argument"))?;

        if content.contains("...rest of") || content.contains("left unchanged") || content.contains("... rest of") || content.contains("... existing code ...") {
            return Err(anyhow::anyhow!("[System Guardrail] CRITICAL ERROR: You attempted to write placeholder text (e.g. '...rest of file left unchanged...'). You are a machine executing a literal file-write. Placeholders will physically delete the user's code. You MUST provide the FULL, EXACT code. Re-evaluate and call the tool properly."));
        }

        // println!(">> [TOOL CALL: patch_file] Patching: {} from line {} to {}", path, start_line, end_line);

        if start_line == 0 || end_line < start_line {
            anyhow::bail!("Invalid line range. Lines must be 1-indexed and start_line <= end_line.");
        }

        let file_content = fs::read_to_string(path)?;
        let lines: Vec<&str> = file_content.lines().collect();

        if start_line > lines.len() + 1 {
            anyhow::bail!("start_line is out of bounds (file has {} lines)", lines.len());
        }

        let mut new_lines = Vec::new();
        for i in 1..start_line {
            if i - 1 < lines.len() {
                new_lines.push(lines[i - 1].to_string());
            }
        }
        
        new_lines.push(content.to_string());
        
        for i in (end_line + 1)..=lines.len() {
            new_lines.push(lines[i - 1].to_string());
        }

        let new_file_content = new_lines.join("\n") + "\n";
        fs::write(path, new_file_content)?;

        Ok(format!("Successfully patched {} from line {} to {}", path, start_line, end_line))
    }
}

pub struct RunBackgroundTool;

#[async_trait::async_trait]
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
    async fn execute(&self, args: &Value, _agent_content: &str) -> Result<String> {
        let cmd = args.get("command").and_then(|c| c.as_str()).ok_or_else(|| anyhow::anyhow!("Missing 'command' argument"))?;
        // println!(">> [TOOL CALL: run_background] Spawning: {}", cmd);

        use std::process::{Command, Stdio};
        use std::io::{Read, BufReader};

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

#[async_trait::async_trait]
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
    
    async fn execute(&self, args: &Value, _agent_content: &str) -> Result<String> {
        let pid = args.get("process_id").and_then(|p| p.as_str()).ok_or_else(|| anyhow::anyhow!("Missing 'process_id' argument"))?;
        // println!(">> [TOOL CALL: read_process_logs] PID: {}", pid);

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

pub struct SearchDirTool;

#[async_trait::async_trait]
impl AgentTool for SearchDirTool {
    fn name(&self) -> &'static str { "search_dir" }
    fn description(&self) -> &'static str { "Recursively searches a directory for a specific text snippet or regex pattern. Returns the matching file paths and line numbers." }
    fn parameters(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "path": { "type": "string", "description": "The directory path to search in (e.g., './src' or '.')." },
                "query": { "type": "string", "description": "The text or regex pattern to search for." }
            },
            "required": ["path", "query"]
        })
    }
    
    async fn execute(&self, args: &Value, _agent_content: &str) -> Result<String> {
        let path = args.get("path").and_then(|p| p.as_str()).unwrap_or(".");
        let query = args.get("query").and_then(|q| q.as_str()).ok_or_else(|| anyhow::anyhow!("Missing 'query' argument"))?;
        
        // println!(">> [TOOL CALL: search_dir] Searching for '{}' in {}", query, path);

        // Try Ripgrep first
        let mut is_rg = true;
        let mut output = std::process::Command::new("rg")
            .arg("-n")
            .arg("-i")
            .arg(query)
            .arg(path)
            .output();

        // Fallback to standard grep if rg fails (e.g., not installed)
        if output.is_err() {
            is_rg = false;
            output = std::process::Command::new("grep")
                .arg("-rni")
                .arg("--exclude-dir=target")
                .arg("--exclude-dir=node_modules")
                .arg("--exclude-dir=.git")
                .arg(query)
                .arg(path)
                .output();
        }

        let output = output?;
        
        let mut out = String::from_utf8_lossy(&output.stdout).to_string();
        
        if out.is_empty() {
            return Ok(format!("No matches found for '{}' in {} (Using {})", query, path, if is_rg { "ripgrep" } else { "grep" }));
        }
        
        let max_len = 4000;
        if out.len() > max_len {
            let safe_len = out.char_indices().nth(max_len).map(|(i, _)| i).unwrap_or(out.len());
            out.truncate(safe_len);
            out.push_str("\n...[Results truncated due to length. Try a more specific query.]...");
        }

        Ok(out)
    }
}

pub struct AskUserTool;

#[async_trait::async_trait]
impl AgentTool for AskUserTool {
    fn name(&self) -> &'static str { "ask_user" }
    fn description(&self) -> &'static str { "Pauses execution and asks the user a clarifying question. Use this when you are blocked and need human input to proceed." }
    fn parameters(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "question": { "type": "string", "description": "The question to ask the user." }
            },
            "required": ["question"]
        })
    }
    
    async fn execute(&self, args: &Value, _agent_content: &str) -> Result<String> {
        let question = args.get("question").and_then(|q| q.as_str()).ok_or_else(|| anyhow::anyhow!("Missing 'question' argument"))?;
        
        println!("\n{} {}", "🤔 [AI Requires Input]".bold().yellow(), question.yellow());
        print!("{} ", ">> User Response:".bold().green());
        use std::io::Write;
        std::io::stdout().flush()?;
        
        let mut input = String::new();
        std::io::stdin().read_line(&mut input)?;
        
        Ok(input.trim().to_string())
    }
}

pub struct ExtractAndWriteTool;

#[async_trait::async_trait]
impl AgentTool for ExtractAndWriteTool {
    fn name(&self) -> &'static str { "extract_and_write" }
    fn description(&self) -> &'static str { "Extracts the latest markdown code block from your thought process and writes it to a file. Use this for complex files to avoid JSON escaping issues. MUST wrap your code in triple backticks BEFORE calling this tool." }
    fn is_modifying(&self) -> bool { true }

    fn parameters(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "The path to the file to create or overwrite."
                }
            },
            "required": ["path"]
        })
    }

    async fn execute(&self, args: &Value, agent_content: &str) -> Result<String> {
        let path_str = args.get("path").and_then(|p| p.as_str()).ok_or_else(|| anyhow::anyhow!("Missing 'path' argument"))?;
        let path_owned = shellexpand::tilde(path_str).to_string();
        let path = path_owned.as_str();

        // println!(">> [TOOL CALL: extract_and_write] Parsing thought process for target: {}", path);

        let blocks: Vec<&str> = agent_content.split("```").collect();
        let mut code_block = "";
        
        // Collect odd indices (actual code blocks between ``` markers)
        let odd_indices: Vec<usize> = (1..blocks.len()).step_by(2).collect();
        
        let file_ext = std::path::Path::new(path)
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("");

        // Pass 1: "Strict Match" — Prefer blocks that match the file extension
        if !file_ext.is_empty() {
            for &i in odd_indices.iter().rev() {
                let b = blocks[i].trim();
                let first_line = b.lines().next().unwrap_or("").to_lowercase();
                if first_line == file_ext 
                   || (file_ext == "sh" && first_line == "bash")
                   || (file_ext == "js" && first_line == "javascript")
                   || (file_ext == "rs" && first_line == "rust")
                   || (file_ext == "py" && first_line == "python") {
                    code_block = blocks[i];
                    break;
                }
            }
        }
        
        // Pass 2: "Heuristic Match" — skip noisy blocks (json) unless we specifically want them
        if code_block.is_empty() {
            let skip_langs = if file_ext == "json" { vec![] } else { vec!["json"] };
            
            for &i in odd_indices.iter().rev() {
                let b = blocks[i].trim();
                let first_line = b.lines().next().unwrap_or("").to_lowercase();
                
                // Skip JSON blocks if we aren't writing a JSON file
                if skip_langs.iter().any(|&s| first_line.starts_with(s)) {
                    continue;
                }

                // Skip generic shell blocks if we aren't writing a script
                let is_shell_block = ["sh", "bash", "zsh", "shell"].iter().any(|&s| first_line == s);
                let is_writing_script = ["sh", "bash", "zsh"].contains(&file_ext);
                if is_shell_block && !is_writing_script {
                    continue;
                }

                let is_tagged = !first_line.is_empty() && !first_line.contains(' ') && first_line.len() < 20;
                if is_tagged {
                    code_block = blocks[i];
                    break;
                }
            }
        }

        // Pass 3: Last Resort — take the first non-empty block that isn't JSON
        if code_block.is_empty() {
             for &i in odd_indices.iter().rev() {
                let b = blocks[i].trim();
                let first_line = b.lines().next().unwrap_or("");
                if !first_line.starts_with("json") && !b.is_empty() {
                    code_block = blocks[i];
                    break;
                }
             }
        }

        if !code_block.is_empty() {
            let clean_code = if let Some(first_newline) = code_block.find('\n') {
                let first_line = &code_block[0..first_newline];
                if !first_line.contains(' ') && !first_line.is_empty() {
                    &code_block[first_newline + 1..]
                } else {
                    code_block
                }
            } else {
                code_block
            };

            if clean_code.trim().is_empty() {
                anyhow::bail!("CRITICAL ERROR: The extracted code block was empty! You must write your actual code inside the triple backticks BEFORE calling this tool.");
            }

            if let Some(parent) = std::path::PathBuf::from(&path_owned).parent() {
                if !parent.as_os_str().is_empty() {
                    if let Err(e) = std::fs::create_dir_all(parent) {
                        anyhow::bail!("Failed to create directory structure for {}: {}. Is path writable?", parent.display(), e);
                    }
                }
            }

            let absolute_path = match std::path::Path::new(&path_owned).canonicalize() {
                Ok(p) => p,
                Err(_) => std::path::PathBuf::from(&path_owned),
            };

            match std::fs::write(&path_owned, clean_code.trim_matches('\n')) {
                Ok(_) => Ok(format!("Successfully extracted code block and wrote {} bytes to {}", clean_code.len(), absolute_path.display())),
                Err(e) => anyhow::bail!("OS ERROR: Failed to write to {}: {}. HINT: Check Folder Permissions / Full Disk Access on macOS.", absolute_path.display(), e),
            }
        } else {
            anyhow::bail!("Could not find a valid markdown code block (` ``` `) in your thought process to extract! You must write the code inside triple backticks explicitly before calling this tool.")
        }
    }
}

pub struct SystemInfoTool;

#[async_trait::async_trait]
impl AgentTool for SystemInfoTool {
    fn name(&self) -> &'static str { "system_info" }
    fn description(&self) -> &'static str { "Reads your Host PC's operating system, CPU architecture, current USER shell identity, HOME directory, and hardware limits (CPU/RAM)." }

    fn parameters(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {},
            "required": []
        })
    }

    async fn execute(&self, _args: &Value, _agent_content: &str) -> Result<String> {
        let mut sys = sysinfo::System::new_all();
        std::thread::sleep(sysinfo::MINIMUM_CPU_UPDATE_INTERVAL);
        sys.refresh_all();
        
        let cpu_count = sys.cpus().len();
        let avg_load: f32 = if cpu_count > 0 {
            sys.cpus().iter().map(|c| c.cpu_usage()).sum::<f32>() / cpu_count as f32
        } else {
            0.0
        };

        let user = std::env::var("USER").unwrap_or_else(|_| "Unknown".to_string());
        let home = dirs::home_dir().map(|p| p.to_string_lossy().to_string()).unwrap_or_else(|| "Unknown".to_string());
        let cwd = std::env::current_dir().map(|p| p.to_string_lossy().to_string()).unwrap_or_else(|_| "Unknown".to_string());

        let os_name = sysinfo::System::name().unwrap_or_else(|| "Unknown".to_owned());
        let os_ver = sysinfo::System::os_version().unwrap_or_else(|| "Unknown".to_owned());
        let host = sysinfo::System::host_name().unwrap_or_else(|| "Unknown".to_owned());

        let report = format!(
            "Tempest AI System Diagnostics:\n--------------------------\nUSER: {}\nHOME: {}\nCWD:  {}\n--------------------------\nOS: {} {}\nHostname: {}\nCPU Cores: {}\nAverage CPU Load: {:.1}%\nTotal RAM: {} MB\nUsed RAM: {} MB",
            user, home, cwd, os_name, os_ver, host, cpu_count, avg_load, sys.total_memory() / 1024 / 1024, sys.used_memory() / 1024 / 1024
        );

        Ok(report)
    }
}

pub struct SqliteQueryTool;

#[async_trait::async_trait]
impl AgentTool for SqliteQueryTool {
    fn name(&self) -> &'static str { "sqlite_query" }
    fn description(&self) -> &'static str { "Executes a raw SQL query against a specified SQLite database file securely. Returns a JSON string of the resulting rows. WARNING: You are parsing RAW SQL natively. Do NOT use CLI dot-commands like '.read' or '.schema'! Do NOT use 'CREATE DATABASE' (SQLite databases are created automatically when you query a new file via db_path)." }

    fn parameters(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "db_path": { "type": "string", "description": "Absolute path (or ~/) to the .sqlite or .db file." },
                "query": { "type": "string", "description": "The exact SQL query to execute (e.g., 'SELECT * FROM users LIMIT 5;')." }
            },
            "required": ["db_path", "query"]
        })
    }

    async fn execute(&self, args: &Value, _agent_content: &str) -> Result<String> {
        let db_path_str = args.get("db_path").and_then(|p| p.as_str()).ok_or_else(|| anyhow::anyhow!("Missing 'db_path' argument"))?;
        let db_path = shellexpand::tilde(db_path_str).to_string();
        
        let query = args.get("query").and_then(|q| q.as_str()).ok_or_else(|| anyhow::anyhow!("Missing 'query' argument"))?;

        // println!(">> [TOOL CALL: sqlite_query] Querying: {}", db_path);

        let conn = rusqlite::Connection::open(&db_path)?;
        conn.execute_batch("PRAGMA journal_mode=WAL;")?;
        conn.execute_batch("PRAGMA synchronous=NORMAL;")?;
        conn.execute_batch("PRAGMA foreign_keys=ON;")?;
        let mut stmt = conn.prepare(query)?;
        
        let column_names: Vec<String> = stmt.column_names().into_iter().map(|s| s.to_string()).collect();

        if column_names.is_empty() {
            let changed = stmt.execute([])?;
            return Ok(format!("Query executed successfully. {} rows affected.", changed));
        }

        let mut rows = stmt.query([])?;
        let mut results = Vec::new();

        while let Some(row) = rows.next()? {
            let mut row_map = serde_json::Map::new();
            for (i, name) in column_names.iter().enumerate() {
                let val_ref = row.get_ref(i)?;
                use rusqlite::types::ValueRef;
                let val = match val_ref {
                    ValueRef::Null => serde_json::Value::Null,
                    ValueRef::Integer(v) => serde_json::json!(v),
                    ValueRef::Real(v) => serde_json::json!(v),
                    ValueRef::Text(t) => serde_json::json!(String::from_utf8_lossy(t)),
                    ValueRef::Blob(b) => serde_json::json!(format!("<Blob {} bytes>", b.len())),
                };
                row_map.insert(name.clone(), val);
            }
            results.push(serde_json::Value::Object(row_map));
        }

        Ok(serde_json::to_string_pretty(&results)?)
    }
}

pub struct GitTool;

#[async_trait::async_trait]
impl AgentTool for GitTool {
    fn name(&self) -> &'static str { "git_action" }
    fn description(&self) -> &'static str { "Natively executes a secure 'git' command using the local OS bindings. Bypasses bash explicitly. Provide arguments as an array of strings (e.g., ['commit', '-m', 'Initial commit'])." }
    fn is_modifying(&self) -> bool { true }

    fn parameters(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "cwd": { "type": "string", "description": "The path to the repository directory." },
                "args": { "type": "array", "items": { "type": "string" }, "description": "Array of string arguments for git (e.g., ['push', 'origin', 'main'])." }
            },
            "required": ["cwd", "args"]
        })
    }

    async fn execute(&self, json_args: &Value, _agent_content: &str) -> Result<String> {
        let cwd_str = json_args.get("cwd").and_then(|c| c.as_str()).unwrap_or(".");
        let cwd = shellexpand::tilde(cwd_str).to_string();

        let raw_args = json_args.get("args").and_then(|a| a.as_array()).ok_or_else(|| anyhow::anyhow!("Missing 'args' array"))?;
        let mut string_args = Vec::new();
        for arg in raw_args {
            if let Some(s) = arg.as_str() {
                string_args.push(s.to_string());
            }
        }

        // println!(">> [TOOL CALL: git_action] git {}", string_args.join(" "));

        let output = std::process::Command::new("git")
            .current_dir(&cwd)
            .args(&string_args)
            .output()?;

        let mut result = String::from_utf8_lossy(&output.stdout).to_string();
        let err_result = String::from_utf8_lossy(&output.stderr).to_string();
        
        if !err_result.is_empty() {
            result.push_str("\n--- STDERR ---\n");
            result.push_str(&err_result);
        }

        if !output.status.success() {
            anyhow::bail!("Git command failed with status {}:\n{}", output.status, result);
        }

        Ok(result)
    }
}

pub struct WatchDirectoryTool;

#[async_trait::async_trait]
impl AgentTool for WatchDirectoryTool {
    fn name(&self) -> &'static str { "watch_directory" }
    fn description(&self) -> &'static str { "Starts a persistent background daemon that watches a directory for file modifications. When you make changes to files, it will instantly run the 'trigger_command' provided. Extremely useful for hot-reloading servers or auto-testing your code upon save!" }

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

    async fn execute(&self, args: &Value, _agent_content: &str) -> Result<String> {
        use notify::Watcher;
        
        let path_str = args.get("path").and_then(|p| p.as_str()).ok_or_else(|| anyhow::anyhow!("Missing 'path' argument"))?;
        let path = shellexpand::tilde(path_str).to_string();
        let cmd = args.get("trigger_command").and_then(|c| c.as_str()).ok_or_else(|| anyhow::anyhow!("Missing 'trigger_command' argument"))?.to_string();

        // println!(">> [TOOL CALL: watch_directory] Spawning daemon on: {}", path);

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
                                println!("\n>> [DAEMON: watch_directory] File changed! Triggering: {}", cmd);
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

// ========== NEW TOOLS: Extended Reach ==========

pub struct HttpRequestTool;

#[async_trait::async_trait]
impl AgentTool for HttpRequestTool {
    fn name(&self) -> &'static str { "http_request" }
    fn description(&self) -> &'static str { "Makes an arbitrary HTTP request (GET, POST, PUT, DELETE, PATCH) with optional headers and body. Use this to interact with REST APIs, webhooks, or any HTTP endpoint. Returns status code, headers, and response body." }
    fn parameters(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "method": { "type": "string", "description": "HTTP method: GET, POST, PUT, DELETE, PATCH" },
                "url": { "type": "string", "description": "The full URL to send the request to" },
                "headers": { "type": "object", "description": "Optional key-value pairs for HTTP headers (e.g., {\"Authorization\": \"Bearer TOKEN\"})" },
                "body": { "type": "string", "description": "Optional request body (typically JSON string for POST/PUT)" }
            },
            "required": ["method", "url"]
        })
    }

    async fn execute(&self, args: &Value, _agent_content: &str) -> Result<String> {
        let method = args.get("method").and_then(|m| m.as_str()).unwrap_or("GET").to_uppercase();
        let url = args.get("url").and_then(|u| u.as_str()).ok_or_else(|| anyhow::anyhow!("Missing 'url' argument"))?;
        // println!(">> [TOOL CALL: http_request] {} {}", method, url);

        let client = reqwest::Client::builder()
            .user_agent("TempestAI/0.1")
            .build()?;

        let mut request = match method.as_str() {
            "POST" => client.post(url),
            "PUT" => client.put(url),
            "DELETE" => client.delete(url),
            "PATCH" => client.patch(url),
            _ => client.get(url),
        };

        // Add custom headers
        if let Some(headers) = args.get("headers").and_then(|h| h.as_object()) {
            for (key, val) in headers {
                if let Some(v) = val.as_str() {
                    request = request.header(key.as_str(), v);
                }
            }
        }

        // Add body if provided
        if let Some(body) = args.get("body").and_then(|b| b.as_str()) {
            request = request.header("Content-Type", "application/json").body(body.to_string());
        }

        let response = request.send().await?;
        let status = response.status();
        let resp_headers: Vec<String> = response.headers().iter()
            .take(10)
            .map(|(k, v)| format!("{}: {}", k, v.to_str().unwrap_or("?")))
            .collect();

        let body = response.text().await?;
        let max_len = 15000;
        let mut truncated_body = body;
        if truncated_body.len() > max_len {
            let safe_len = truncated_body.char_indices().nth(max_len).map(|(i, _)| i).unwrap_or(truncated_body.len());
            truncated_body.truncate(safe_len);
            truncated_body.push_str("\n...[Response truncated]...");
        }

        Ok(format!("Status: {}\nHeaders:\n{}\n\nBody:\n{}", status, resp_headers.join("\n"), truncated_body))
    }
}

pub struct ClipboardTool;

#[async_trait::async_trait]
impl AgentTool for ClipboardTool {
    fn name(&self) -> &'static str { "clipboard" }
    fn description(&self) -> &'static str { "Read from or write to the system clipboard. Use 'read' to get clipboard contents, or 'write' to copy text to the clipboard so the user can paste it." }
    fn parameters(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "action": { "type": "string", "description": "'read' to get clipboard contents, 'write' to set them" },
                "content": { "type": "string", "description": "Text to copy to clipboard (required for 'write')" }
            },
            "required": ["action"]
        })
    }

    async fn execute(&self, args: &Value, _agent_content: &str) -> Result<String> {
        let action = args.get("action").and_then(|a| a.as_str()).unwrap_or("read");
        // println!(">> [TOOL CALL: clipboard] Action: {}", action);

        match action {
            "write" => {
                let content = args.get("content").and_then(|c| c.as_str())
                    .ok_or_else(|| anyhow::anyhow!("Missing 'content' for clipboard write"))?;
                let mut clipboard = arboard::Clipboard::new()
                    .map_err(|e| anyhow::anyhow!("Failed to access clipboard: {}", e))?;
                clipboard.set_text(content)
                    .map_err(|e| anyhow::anyhow!("Failed to write to clipboard: {}", e))?;
                Ok(format!("✅ Copied {} characters to clipboard.", content.len()))
            },
            "read" => {
                let mut clipboard = arboard::Clipboard::new()
                    .map_err(|e| anyhow::anyhow!("Failed to access clipboard: {}", e))?;
                let text = clipboard.get_text()
                    .map_err(|e| anyhow::anyhow!("Failed to read clipboard: {}", e))?;
                Ok(format!("Clipboard contents:\n{}", text))
            },
            _ => anyhow::bail!("Unknown clipboard action '{}'. Use 'read' or 'write'.", action),
        }
    }
}

pub struct NotifyTool;

#[async_trait::async_trait]
impl AgentTool for NotifyTool {
    fn name(&self) -> &'static str { "notify" }
    fn description(&self) -> &'static str { "Sends a native macOS desktop notification. Use this to alert the user when a long-running task completes or when something important happens." }
    fn parameters(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "title": { "type": "string", "description": "Notification title" },
                "message": { "type": "string", "description": "Notification message body" }
            },
            "required": ["title", "message"]
        })
    }

    async fn execute(&self, args: &Value, _agent_content: &str) -> Result<String> {
        let title = args.get("title").and_then(|t| t.as_str()).unwrap_or("Tempest AI");
        let message = args.get("message").and_then(|m| m.as_str()).unwrap_or("Task complete.");
        // println!(">> [TOOL CALL: notify] {} — {}", title, message);

        let script = format!(
            "display notification \"{}\" with title \"{}\" sound name \"Glass\"",
            message.replace('"', "\\\""),
            title.replace('"', "\\\"")
        );

        let output = Command::new("osascript")
            .arg("-e")
            .arg(&script)
            .output()?;

        if output.status.success() {
            Ok(format!("🔔 Notification sent: {} — {}", title, message))
        } else {
            let err = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!("Failed to send notification: {}", err)
        }
    }
}

// ========== NEW TOOLS: Precision & Dexterity ==========

pub struct FindReplaceTool;

#[async_trait::async_trait]
impl AgentTool for FindReplaceTool {
    fn name(&self) -> &'static str { "find_replace" }
    fn description(&self) -> &'static str { "Performs a regex or literal find-and-replace across one or more files. Can target a single file or recursively process a directory. Returns a summary of all replacements made. Use this for sweeping refactors like renaming functions, updating imports, or changing config values across an entire project." }
    fn parameters(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "path": { "type": "string", "description": "File or directory path to search in" },
                "find": { "type": "string", "description": "The text or regex pattern to find" },
                "replace": { "type": "string", "description": "The replacement string" },
                "is_regex": { "type": "boolean", "description": "If true, treat 'find' as a regex pattern. Default: false (literal match)" },
                "file_pattern": { "type": "string", "description": "Optional glob pattern to filter files when path is a directory (e.g., '*.rs', '*.toml')" }
            },
            "required": ["path", "find", "replace"]
        })
    }

    fn requires_confirmation(&self) -> bool { true }
    fn is_modifying(&self) -> bool { true }

    async fn execute(&self, args: &Value, _agent_content: &str) -> Result<String> {
        let path_str = args.get("path").and_then(|p| p.as_str()).ok_or_else(|| anyhow::anyhow!("Missing 'path'"))?;
        let path_owned = shellexpand::tilde(path_str).to_string();
        let find = args.get("find").and_then(|f| f.as_str()).ok_or_else(|| anyhow::anyhow!("Missing 'find'"))?;
        let replace = args.get("replace").and_then(|r| r.as_str()).ok_or_else(|| anyhow::anyhow!("Missing 'replace'"))?;
        let is_regex = args.get("is_regex").and_then(|r| r.as_bool()).unwrap_or(false);
        let file_pattern = args.get("file_pattern").and_then(|f| f.as_str());

        // println!(">> [TOOL CALL: find_replace] {} → {} in {}", find, replace, path_owned);

        let path = std::path::Path::new(&path_owned);
        let mut files_to_process: Vec<PathBuf> = vec![];

        if path.is_file() {
            files_to_process.push(path.to_path_buf());
        } else if path.is_dir() {
            fn collect_files(dir: &std::path::Path, pattern: Option<&str>, out: &mut Vec<PathBuf>) {
                if let Ok(entries) = fs::read_dir(dir) {
                    for entry in entries.flatten() {
                        let p = entry.path();
                        if p.is_dir() {
                            if !p.file_name().map(|n| n.to_str().unwrap_or("").starts_with('.')).unwrap_or(false) {
                                collect_files(&p, pattern, out);
                            }
                        } else if p.is_file() {
                            if let Some(pat) = pattern {
                                if let Some(name) = p.file_name().and_then(|n| n.to_str()) {
                                    let glob = pat.trim_start_matches('*');
                                    if name.ends_with(glob) {
                                        out.push(p);
                                    }
                                }
                            } else {
                                out.push(p);
                            }
                        }
                    }
                }
            }
            collect_files(path, file_pattern, &mut files_to_process);
        } else {
            anyhow::bail!("Path '{}' does not exist", path_owned);
        }

        let mut total_replacements = 0;
        let mut files_modified = 0;
        let mut summary = String::new();

        for file in &files_to_process {
            if let Ok(content) = fs::read_to_string(file) {
                let new_content = if is_regex {
                    let re = regex::Regex::new(find)
                        .map_err(|e| anyhow::anyhow!("Invalid regex: {}", e))?;
                    let count = re.find_iter(&content).count();
                    if count > 0 {
                        total_replacements += count;
                        files_modified += 1;
                        summary.push_str(&format!("  {} — {} replacements\n", file.display(), count));
                        re.replace_all(&content, replace).to_string()
                    } else {
                        continue;
                    }
                } else {
                    let count = content.matches(find).count();
                    if count > 0 {
                        total_replacements += count;
                        files_modified += 1;
                        summary.push_str(&format!("  {} — {} replacements\n", file.display(), count));
                        content.replace(find, replace)
                    } else {
                        continue;
                    }
                };
                fs::write(file, new_content)?;
            }
        }

        if total_replacements == 0 {
            Ok(format!("No matches found for '{}' in {}", find, path_owned))
        } else {
            Ok(format!("✅ {} replacements across {} files:\n{}", total_replacements, files_modified, summary))
        }
    }
}

pub struct TreeTool;

#[async_trait::async_trait]
impl AgentTool for TreeTool {
    fn name(&self) -> &'static str { "tree" }
    fn description(&self) -> &'static str { "Shows a recursive directory tree view. Gives you full project structure awareness instantly. Excludes hidden directories and common noise like node_modules, target, .git by default." }
    fn parameters(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "path": { "type": "string", "description": "Root directory to display tree for" },
                "max_depth": { "type": "integer", "description": "Maximum depth to recurse (default: 4)" }
            },
            "required": ["path"]
        })
    }

    async fn execute(&self, args: &Value, _agent_content: &str) -> Result<String> {
        let path_str = args.get("path").and_then(|p| p.as_str()).unwrap_or(".");
        let path_owned = shellexpand::tilde(path_str).to_string();
        let max_depth = args.get("max_depth").and_then(|d| d.as_u64()).unwrap_or(4) as usize;
        // println!(">> [TOOL CALL: tree] {} (depth: {})", path_owned, max_depth);

        let skip_dirs = ["node_modules", "target", ".git", "__pycache__", ".next", "dist", "build", ".DS_Store"];
        let mut output = String::new();
        let mut file_count = 0usize;
        let mut dir_count = 0usize;

        fn walk_tree(
            dir: &std::path::Path,
            prefix: &str,
            depth: usize,
            max_depth: usize,
            skip: &[&str],
            output: &mut String,
            file_count: &mut usize,
            dir_count: &mut usize,
        ) {
            if depth > max_depth { return; }
            let mut entries: Vec<_> = match fs::read_dir(dir) {
                Ok(rd) => rd.filter_map(|e| e.ok()).collect(),
                Err(_) => return,
            };
            entries.sort_by_key(|e| e.file_name());

            let total = entries.len();
            for (i, entry) in entries.iter().enumerate() {
                let name = entry.file_name().to_string_lossy().to_string();
                if skip.contains(&name.as_str()) || name.starts_with('.') {
                    continue;
                }

                let is_last = i == total - 1;
                let connector = if is_last { "└── " } else { "├── " };
                let child_prefix = if is_last { "    " } else { "│   " };

                let path = entry.path();
                if path.is_dir() {
                    *dir_count += 1;
                    output.push_str(&format!("{}{}{}/\n", prefix, connector, name));
                    walk_tree(&path, &format!("{}{}", prefix, child_prefix), depth + 1, max_depth, skip, output, file_count, dir_count);
                } else {
                    *file_count += 1;
                    let size = fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
                    let size_str = if size > 1_000_000 {
                        format!("{:.1}MB", size as f64 / 1_000_000.0)
                    } else if size > 1_000 {
                        format!("{:.1}KB", size as f64 / 1_000.0)
                    } else {
                        format!("{}B", size)
                    };
                    output.push_str(&format!("{}{}{} ({})\n", prefix, connector, name, size_str));
                }
            }
        }

        let root = std::path::Path::new(&path_owned);
        output.push_str(&format!("{}/\n", path_owned));
        walk_tree(root, "", 0, max_depth, &skip_dirs, &mut output, &mut file_count, &mut dir_count);
        output.push_str(&format!("\n{} directories, {} files", dir_count, file_count));

        Ok(output)
    }
}

pub struct NetworkCheckTool;

#[async_trait::async_trait]
impl AgentTool for NetworkCheckTool {
    fn name(&self) -> &'static str { "network_check" }
    fn description(&self) -> &'static str { "Performs safe, non-hanging network diagnostics. Supports 'ping' (with automatic -c 4 limit), 'dns' (resolves a hostname), and 'port' (checks if a TCP port accepts connections). Use this instead of run_command for network tests." }
    fn parameters(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "action": { "type": "string", "description": "'ping', 'dns', or 'port'" },
                "host": { "type": "string", "description": "Hostname or IP to test" },
                "port": { "type": "integer", "description": "Port number (required for 'port' action)" }
            },
            "required": ["action", "host"]
        })
    }

    async fn execute(&self, args: &Value, _agent_content: &str) -> Result<String> {
        let action = args.get("action").and_then(|a| a.as_str()).unwrap_or("ping");
        let host = args.get("host").and_then(|h| h.as_str()).ok_or_else(|| anyhow::anyhow!("Missing 'host'"))?;
        // println!(">> [TOOL CALL: network_check] {} {}", action, host);

        match action {
            "ping" => {
                let output = Command::new("ping")
                    .args(["-c", "4", "-W", "3", host])
                    .output()?;
                let stdout = String::from_utf8_lossy(&output.stdout);
                let stderr = String::from_utf8_lossy(&output.stderr);
                if output.status.success() {
                    Ok(format!("✅ Ping results:\n{}", stdout))
                } else {
                    Ok(format!("❌ Ping failed:\n{}{}", stdout, stderr))
                }
            },
            "dns" => {
                let output = Command::new("dig")
                    .args(["+short", "+time=3", "+tries=1", host])
                    .output()?;
                let result = String::from_utf8_lossy(&output.stdout).trim().to_string();
                if result.is_empty() {
                    Ok(format!("❌ DNS lookup failed for '{}'", host))
                } else {
                    Ok(format!("✅ DNS results for '{}':\n{}", host, result))
                }
            },
            "port" => {
                let port = args.get("port").and_then(|p| p.as_u64())
                    .ok_or_else(|| anyhow::anyhow!("Missing 'port' for port check"))? as u16;
                let addr = format!("{}:{}", host, port);
                match std::net::TcpStream::connect_timeout(
                    &addr.parse().unwrap_or_else(|_| std::net::SocketAddr::from(([127, 0, 0, 1], port))),
                    std::time::Duration::from_secs(3),
                ) {
                    Ok(_) => Ok(format!("✅ Port {} is OPEN on {}", port, host)),
                    Err(e) => Ok(format!("❌ Port {} is CLOSED on {} — {}", port, host, e)),
                }
            },
            _ => anyhow::bail!("Unknown network action '{}'. Use 'ping', 'dns', or 'port'.", action),
        }
    }
}

// ========== WAVE 6: Competitive Gap Tools ==========

pub struct DiffFilesTool;

#[async_trait::async_trait]
impl AgentTool for DiffFilesTool {
    fn name(&self) -> &'static str { "diff_files" }
    fn description(&self) -> &'static str { "Compare two files and show their differences in unified diff format." }
    fn parameters(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "file_a": { "type": "string", "description": "Path to the first file" },
                "file_b": { "type": "string", "description": "Path to the second file" }
            },
            "required": ["file_a", "file_b"]
        })
    }

    async fn execute(&self, args: &Value, _agent_content: &str) -> Result<String> {
        let file_a = args.get("file_a").and_then(|p| p.as_str()).ok_or_else(|| anyhow::anyhow!("Missing 'file_a'"))?;
        let file_b = args.get("file_b").and_then(|p| p.as_str()).ok_or_else(|| anyhow::anyhow!("Missing 'file_b'"))?;
        let a = shellexpand::tilde(file_a).to_string();
        let b = shellexpand::tilde(file_b).to_string();
        // println!(">> [TOOL CALL: diff_files] {} vs {}", a, b);

        let output = Command::new("diff")
            .args(["-u", &a, &b])
            .output()?;
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        
        if stdout.is_empty() && output.status.success() {
            Ok("Files are identical.".to_string())
        } else if !stderr.is_empty() {
            Ok(format!("Diff error: {}", stderr))
        } else {
            Ok(format!("Diff output:\n{}", stdout))
        }
    }
}

pub struct KillProcessTool;

#[async_trait::async_trait]
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

    async fn execute(&self, args: &Value, _agent_content: &str) -> Result<String> {
        let pid = args.get("pid").and_then(|p| p.as_str()).ok_or_else(|| anyhow::anyhow!("Missing 'pid'"))?;
        let signal = args.get("signal").and_then(|s| s.as_str()).unwrap_or("TERM");
        // println!(">> [TOOL CALL: kill_process] Sending {} to PID {}", signal, pid);

        let output = Command::new("kill")
            .args([&format!("-{}", signal), pid])
            .output()?;
        
        if output.status.success() {
            Ok(format!("✅ Sent {} signal to process {}", signal, pid))
        } else {
            let err = String::from_utf8_lossy(&output.stderr);
            Ok(format!("❌ Failed to kill process {}: {}", pid, err.trim()))
        }
    }
}

pub struct EnvVarTool;

#[async_trait::async_trait]
impl AgentTool for EnvVarTool {
    fn name(&self) -> &'static str { "env_var" }
    fn description(&self) -> &'static str { "Get or set environment variables. Use 'get' to read a variable, 'set' to set one for the current session, or 'list' to show all." }
    fn parameters(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "action": { "type": "string", "description": "'get', 'set', or 'list'" },
                "name": { "type": "string", "description": "Variable name (required for get/set)" },
                "value": { "type": "string", "description": "Variable value (required for set)" }
            },
            "required": ["action"]
        })
    }

    async fn execute(&self, args: &Value, _agent_content: &str) -> Result<String> {
        let action = args.get("action").and_then(|a| a.as_str()).unwrap_or("get");
        // println!(">> [TOOL CALL: env_var] Action: {}", action);

        match action {
            "get" => {
                let name = args.get("name").and_then(|n| n.as_str())
                    .ok_or_else(|| anyhow::anyhow!("Missing 'name' for env get"))?;
                match std::env::var(name) {
                    Ok(val) => Ok(format!("{}={}", name, val)),
                    Err(_) => Ok(format!("Variable '{}' is not set.", name)),
                }
            },
            "set" => {
                let name = args.get("name").and_then(|n| n.as_str())
                    .ok_or_else(|| anyhow::anyhow!("Missing 'name' for env set"))?;
                let value = args.get("value").and_then(|v| v.as_str())
                    .ok_or_else(|| anyhow::anyhow!("Missing 'value' for env set"))?;
                unsafe { std::env::set_var(name, value); }
                Ok(format!("✅ Set {}={}", name, value))
            },
            "list" => {
                let vars: Vec<String> = std::env::vars()
                    .take(50)
                    .map(|(k, v)| {
                        let truncated = if v.len() > 100 { format!("{}...", &v[..100]) } else { v };
                        format!("{}={}", k, truncated)
                    })
                    .collect();
                Ok(format!("Environment variables (first 50):\n{}", vars.join("\n")))
            },
            _ => anyhow::bail!("Unknown env_var action '{}'. Use 'get', 'set', or 'list'.", action),
        }
    }
}

pub struct ChmodTool;

#[async_trait::async_trait]
impl AgentTool for ChmodTool {
    fn name(&self) -> &'static str { "chmod" }
    fn description(&self) -> &'static str { "Change file or directory permissions using standard Unix mode strings (e.g., '755', '644', '+x')." }
    fn requires_confirmation(&self) -> bool { true }
    fn is_modifying(&self) -> bool { true }
    fn parameters(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "path": { "type": "string", "description": "Path to file or directory" },
                "mode": { "type": "string", "description": "Permission mode (e.g., '755', '644', '+x', 'u+rwx')" }
            },
            "required": ["path", "mode"]
        })
    }

    async fn execute(&self, args: &Value, _agent_content: &str) -> Result<String> {
        let path_str = args.get("path").and_then(|p| p.as_str()).ok_or_else(|| anyhow::anyhow!("Missing 'path'"))?;
        let path = shellexpand::tilde(path_str).to_string();
        let mode = args.get("mode").and_then(|m| m.as_str()).ok_or_else(|| anyhow::anyhow!("Missing 'mode'"))?;
        // println!(">> [TOOL CALL: chmod] {} {}", mode, path);

        let output = Command::new("chmod")
            .args([mode, &path])
            .output()?;
        
        if output.status.success() {
            Ok(format!("✅ Changed permissions of '{}' to '{}'", path, mode))
        } else {
            let err = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!("chmod failed: {}", err.trim())
        }
    }
}

pub struct AppendFileTool;

#[async_trait::async_trait]
impl AgentTool for AppendFileTool {
    fn name(&self) -> &'static str { "append_file" }
    fn description(&self) -> &'static str { "Append content to the end of an existing file without overwriting it. Creates the file if it doesn't exist." }
    fn requires_confirmation(&self) -> bool { true }
    fn is_modifying(&self) -> bool { true }
    fn parameters(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "path": { "type": "string", "description": "Path to the file to append to" },
                "content": { "type": "string", "description": "Content to append" }
            },
            "required": ["path", "content"]
        })
    }

    async fn execute(&self, args: &Value, _agent_content: &str) -> Result<String> {
        let path_str = args.get("path").and_then(|p| p.as_str()).ok_or_else(|| anyhow::anyhow!("Missing 'path'"))?;
        let path = shellexpand::tilde(path_str).to_string();
        let content = args.get("content").and_then(|c| c.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing 'content'"))?;
        // println!(">> [TOOL CALL: append_file] Appending to: {}", path);

        use std::io::Write;
        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)?;
        file.write_all(content.as_bytes())?;
        Ok(format!("✅ Appended {} bytes to {}", content.len(), path))
    }
}

pub struct DownloadFileTool;

#[async_trait::async_trait]
impl AgentTool for DownloadFileTool {
    fn name(&self) -> &'static str { "download_file" }
    fn description(&self) -> &'static str { "Download a file from a URL and save it to a local path. Useful for fetching remote resources, images, scripts, or data files." }
    fn requires_confirmation(&self) -> bool { true }
    fn is_modifying(&self) -> bool { true }
    fn parameters(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "url": { "type": "string", "description": "URL to download from" },
                "path": { "type": "string", "description": "Local path to save the downloaded file" }
            },
            "required": ["url", "path"]
        })
    }

    async fn execute(&self, args: &Value, _agent_content: &str) -> Result<String> {
        let url = args.get("url").and_then(|u| u.as_str()).ok_or_else(|| anyhow::anyhow!("Missing 'url'"))?;
        let path_str = args.get("path").and_then(|p| p.as_str()).ok_or_else(|| anyhow::anyhow!("Missing 'path'"))?;
        let path = shellexpand::tilde(path_str).to_string();
        // println!(">> [TOOL CALL: download_file] {} → {}", url, path);

        let client = reqwest::Client::builder()
            .user_agent("TempestAI/0.1")
            .build()?;
        let response = client.get(url).send().await?;
        let status = response.status();
        
        if !status.is_success() {
            anyhow::bail!("Download failed with status {}", status);
        }

        let bytes = response.bytes().await?;
        
        if let Some(parent) = std::path::Path::new(&path).parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent)?;
            }
        }
        std::fs::write(&path, &bytes)?;
        Ok(format!("✅ Downloaded {} bytes from {} → {}", bytes.len(), url, path))
    }
}

pub struct StoreMemoryTool {
    mem: Arc<Mutex<MemoryStore>>,
}

impl StoreMemoryTool {
    pub fn new(mem: Arc<Mutex<MemoryStore>>) -> Self { Self { mem } }
}

#[async_trait::async_trait]
impl AgentTool for StoreMemoryTool {
    fn name(&self) -> &'static str { "store_memory" }
    fn description(&self) -> &'static str { "Save a crucial fact, preference, API key, or architectural detail to your long-term encrypted memory database. You will retain this fact forever across reboots." }
    fn parameters(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "topic": { "type": "string", "description": "A short, unique keyword identifying this memory (e.g., 'user_github_email', 'project_build_commands')." },
                "content": { "type": "string", "description": "The detailed fact to remember." }
            },
            "required": ["topic", "content"]
        })
    }
    async fn execute(&self, args: &Value, _agent_content: &str) -> Result<String> {
        let topic = args.get("topic").and_then(|t| t.as_str()).ok_or_else(|| anyhow::anyhow!("Missing 'topic'"))?;
        let content = args.get("content").and_then(|c| c.as_str()).ok_or_else(|| anyhow::anyhow!("Missing 'content'"))?;
        // println!(">> [TOOL CALL: store_memory] Saving fact to brain under topic: {}", topic);
        self.mem.lock().unwrap().store(topic, content)?;
        Ok(format!("Memory '{}' stored securely in the encrypted brain.", topic))
    }
}

pub struct RecallMemoryTool {
    mem: Arc<Mutex<MemoryStore>>,
}

impl RecallMemoryTool {
    pub fn new(mem: Arc<Mutex<MemoryStore>>) -> Self { Self { mem } }
}

#[async_trait::async_trait]
impl AgentTool for RecallMemoryTool {
    fn name(&self) -> &'static str { "recall_memory" }
    fn description(&self) -> &'static str { "Search your encrypted long-term memory database for previously saved facts." }
    fn parameters(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "keyword": { "type": "string", "description": "The keyword to search for in your memory topics (use '%' for wildcard)." }
            },
            "required": ["keyword"]
        })
    }
    async fn execute(&self, args: &Value, _agent_content: &str) -> Result<String> {
        let keyword = args.get("keyword").and_then(|k| k.as_str()).ok_or_else(|| anyhow::anyhow!("Missing 'keyword'"))?;
        // println!(">> [TOOL CALL: recall_memory] Searching brain for: {}", keyword);
        let results = self.mem.lock().unwrap().recall(keyword)?;
        if results.is_empty() {
            Ok(format!("No memories found matching '{}'.", keyword))
        } else {
            let mut out = format!("Recalled {} memories:\n", results.len());
            for (t, c) in results {
                out.push_str(&format!("- [{}]: {}\n", t, c));
            }
            Ok(out)
        }
    }
}
pub struct SystemdManagerTool;

#[async_trait::async_trait]
impl AgentTool for SystemdManagerTool {
    fn name(&self) -> &'static str { "systemd_manager" }
    fn description(&self) -> &'static str { "Natively monitor and manage Systemd services on Linux. Use 'action': 'list' to see all units, or 'start'/'stop'/'restart'/'status' with a 'unit' name. REQUIRES LINUX HOST." }
    fn is_modifying(&self) -> bool { true }
    fn parameters(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "action": { "type": "string", "enum": ["list", "start", "stop", "restart", "status"], "description": "The systemctl action to perform." },
                "unit": { "type": "string", "description": "The name of the service unit (e.g. 'nginx.service')." }
            },
            "required": ["action"]
        })
    }
    async fn execute(&self, args: &Value, _agent_content: &str) -> Result<String> {
        #[cfg(target_os = "linux")]
        {
            let action = args.get("action").and_then(|a| a.as_str()).unwrap_or("list");
            let unit = args.get("unit").and_then(|u| u.as_str()).unwrap_or("");

            let mut cmd = std::process::Command::new("systemctl");
            match action {
                "list" => {
                    cmd.args(["list-units", "--type=service", "--all", "--no-pager"]);
                }
                "start" | "stop" | "restart" | "status" => {
                    if unit.is_empty() { return Ok("Error: Unit name required for this action.".to_string()); }
                    cmd.args([action, unit]);
                }
                _ => return Ok("Error: Unsupported action.".to_string()),
            }

            let output = cmd.output()?;
            let stdout = String::from_utf8_lossy(&output.stdout);
            let stderr = String::from_utf8_lossy(&output.stderr);

            if !stderr.is_empty() {
                Ok(format!("Systemd Output:\n{}\nErrors:\n{}", stdout, stderr))
            } else {
                Ok(format!("Systemd Output:\n{}", stdout))
            }
        }
        #[cfg(not(target_os = "linux"))]
        {
            let _ = args;
            Ok("Error: The systemd_manager tool is exclusive to Linux environments. Your host OS does not use systemd.".to_string())
        }
    }
}

// ═══════════════════════════════════════════════════════════════════
// 🧠 META-COGNITIVE TOOLS: Agent Self-Management
// ═══════════════════════════════════════════════════════════════════

pub struct TogglePlanningTool;

#[async_trait::async_trait]
impl AgentTool for TogglePlanningTool {
    fn name(&self) -> &'static str { "toggle_planning" }
    fn description(&self) -> &'static str { "Toggle between PLANNING mode (research only, no file writes) and EXECUTING mode (full tool access). Use 'on' to enter planning mode, 'off' to enter execution mode after the user approves your plan." }
    fn parameters(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "mode": { "type": "string", "enum": ["on", "off"], "description": "'on' = enter planning mode (block writes), 'off' = enter execution mode (allow writes)" }
            },
            "required": ["mode"]
        })
    }
    async fn execute(&self, args: &Value, _agent_content: &str) -> Result<String> {
        let mode = args.get("mode").and_then(|m| m.as_str()).unwrap_or("on");
        match mode {
            "on" => Ok("[PLANNING_MODE_ON] You are now in PLANNING mode. You may use read-only tools (read_file, list_dir, search_dir, search_web, system_info) to research. All file writes and commands are BLOCKED until you present a plan and switch to execution mode.".to_string()),
            "off" => Ok("[PLANNING_MODE_OFF] You are now in EXECUTION mode. All tools are available. Remember to VERIFY your work after every modification.".to_string()),
            _ => Ok("Error: mode must be 'on' or 'off'".to_string()),
        }
    }
}
