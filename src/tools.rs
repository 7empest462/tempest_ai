use anyhow::Result;
use colored::Colorize;
use serde_json::Value;
use std::process::Command;
use std::sync::{Arc, Mutex, OnceLock};
use std::collections::HashMap;
use std::{fs, path::PathBuf};

type ProcessLogs = Arc<Mutex<String>>;

fn process_registry() -> &'static Mutex<HashMap<String, ProcessLogs>> {
    static REGISTRY: OnceLock<Mutex<HashMap<String, ProcessLogs>>> = OnceLock::new();
    REGISTRY.get_or_init(|| Mutex::new(HashMap::new()))
}

/// A trait representing an autonomous tool the agent can use.
pub trait AgentTool: Send + Sync {
    fn name(&self) -> &'static str;
    fn description(&self) -> &'static str;
    fn execute(&self, args: &Value, _agent_content: &str) -> Result<String>;
    
    /// Define the JSON schema for this tool's parameters.
    fn parameters(&self) -> Value;

    /// Whether this tool requires explicit human confirmation before executing.
    fn requires_confirmation(&self) -> bool {
        false
    }
}

pub struct RunCommandTool;

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

    fn parameters(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "command": {
                    "type": "string",
                    "description": "The command string to execute (e.g., 'ls -la' or 'cat Cargo.toml')."
                }
            },
            "required": ["command"]
        })
    }

    fn execute(&self, args: &Value, _agent_content: &str) -> Result<String> {
        let cmd = args.get("command")
            .and_then(|c| c.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing 'command' argument"))?;

        println!(">> [TOOL CALL: run_command] Executing: {}", cmd);
        
        let current_path = std::env::var("PATH").unwrap_or_default();
        let new_path = format!("/opt/homebrew/bin:/usr/local/bin:{}", current_path);
        
        let mut child = Command::new("sh")
            .env("PATH", new_path)
            .arg("-c")
            .arg(cmd)
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()?;

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

    fn execute(&self, args: &Value, _agent_content: &str) -> Result<String> {
        let path_str = args.get("path")
            .and_then(|p| p.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing 'path' argument"))?;
        let path_owned = shellexpand::tilde(path_str).to_string();
        let path = path_owned.as_str();

        println!(">> [TOOL CALL: read_file] Reading: {}", path);
        let content = fs::read_to_string(path)?;
        Ok(content)
    }
}

pub struct WriteFileTool;

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

    fn execute(&self, args: &Value, _agent_content: &str) -> Result<String> {
        let path_str = args.get("path")
            .and_then(|p| p.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing 'path' argument"))?;
        let path_owned = shellexpand::tilde(path_str).to_string();
        let path = path_owned.as_str();
            
        let content = args.get("content")
            .and_then(|c| c.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing 'content' argument"))?;

        if content.contains("...rest of") || content.contains("left unchanged") || content.contains("... rest of") || content.contains("... existing code ...") {
            return Err(anyhow::anyhow!("[System Guardrail] CRITICAL ERROR: You attempted to write placeholder text (e.g. '...rest of file left unchanged...'). You are a machine executing a literal file-write. Placeholders will physically delete the user's code. You MUST provide the FULL, EXACT code. Re-evaluate and call the tool properly."));
        }

        println!(">> [TOOL CALL: write_file] Writing to: {}", path);
        
        if let Some(parent) = PathBuf::from(path).parent() {
            fs::create_dir_all(parent)?;
        }
        
        fs::write(path, content)?;
        Ok(format!("Successfully wrote {} bytes to {}", content.len(), path))
    }
}

pub struct ListDirTool;

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

    fn execute(&self, args: &Value, _agent_content: &str) -> Result<String> {
        let path_str = args.get("path")
            .and_then(|p| p.as_str())
            .unwrap_or(".");
        let path_owned = shellexpand::tilde(path_str).to_string();
        let path = path_owned.as_str();
            
        println!(">> [TOOL CALL: list_dir] Listing: {}", path);
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

    fn execute(&self, args: &Value, _agent_content: &str) -> Result<String> {
        let query = args.get("query").and_then(|q| q.as_str()).unwrap_or("").to_string();
        println!(">> [TOOL CALL: search_web] Query: {}", query);
        
        tokio::task::block_in_place(move || {
            let url = "https://lite.duckduckgo.com/lite/";
            let client = reqwest::blocking::Client::builder()
                .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64)")
                .build()?;
                
            let res = client.post(url)
                .form(&[("q", &query)])
                .send()?
                .text()?;
                
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
        })
    }
}

pub struct ReadUrlTool;

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
    
    fn execute(&self, args: &Value, _agent_content: &str) -> Result<String> {
        let url = args.get("url").and_then(|u| u.as_str()).unwrap_or("").to_string();
        println!(">> [TOOL CALL: read_url] Fetching: {}", url);
        
        tokio::task::block_in_place(move || {
            let client = reqwest::blocking::Client::builder()
                .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36")
                .build()?;
                
            let res = client.get(&url).send()?;
            let html_bytes = res.bytes()?;
            
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
        })
    }
}

pub struct PatchFileTool;

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

    fn execute(&self, args: &Value, _agent_content: &str) -> Result<String> {
        let path_str = args.get("file_path").and_then(|p| p.as_str()).ok_or_else(|| anyhow::anyhow!("Missing 'file_path' argument"))?;
        let path_owned = shellexpand::tilde(path_str).to_string();
        let path = path_owned.as_str();
        let start_line = args.get("start_line").and_then(|v| v.as_u64()).ok_or_else(|| anyhow::anyhow!("Missing 'start_line' argument"))? as usize;
        let end_line = args.get("end_line").and_then(|v| v.as_u64()).ok_or_else(|| anyhow::anyhow!("Missing 'end_line' argument"))? as usize;
        let content = args.get("content").and_then(|c| c.as_str()).ok_or_else(|| anyhow::anyhow!("Missing 'content' argument"))?;

        if content.contains("...rest of") || content.contains("left unchanged") || content.contains("... rest of") || content.contains("... existing code ...") {
            return Err(anyhow::anyhow!("[System Guardrail] CRITICAL ERROR: You attempted to write placeholder text (e.g. '...rest of file left unchanged...'). You are a machine executing a literal file-write. Placeholders will physically delete the user's code. You MUST provide the FULL, EXACT code. Re-evaluate and call the tool properly."));
        }

        println!(">> [TOOL CALL: patch_file] Patching: {} from line {} to {}", path, start_line, end_line);

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

impl AgentTool for RunBackgroundTool {
    fn name(&self) -> &'static str { "run_background" }
    fn description(&self) -> &'static str { "Spawns a long-running bash/zsh command in the background (like starting a web server). Returns a process_id immediately. Use read_process_logs to check its output." }
    fn requires_confirmation(&self) -> bool { true }
    fn parameters(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "command": { "type": "string", "description": "The command string to execute in the background." }
            },
            "required": ["command"]
        })
    }
    fn execute(&self, args: &Value, _agent_content: &str) -> Result<String> {
        let cmd = args.get("command").and_then(|c| c.as_str()).ok_or_else(|| anyhow::anyhow!("Missing 'command' argument"))?;
        println!(">> [TOOL CALL: run_background] Spawning: {}", cmd);

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
    
    fn execute(&self, args: &Value, _agent_content: &str) -> Result<String> {
        let pid = args.get("process_id").and_then(|p| p.as_str()).ok_or_else(|| anyhow::anyhow!("Missing 'process_id' argument"))?;
        println!(">> [TOOL CALL: read_process_logs] PID: {}", pid);

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
    
    fn execute(&self, args: &Value, _agent_content: &str) -> Result<String> {
        let path = args.get("path").and_then(|p| p.as_str()).unwrap_or(".");
        let query = args.get("query").and_then(|q| q.as_str()).ok_or_else(|| anyhow::anyhow!("Missing 'query' argument"))?;
        
        println!(">> [TOOL CALL: search_dir] Searching for '{}' in {}", query, path);

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
    
    fn execute(&self, args: &Value, _agent_content: &str) -> Result<String> {
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

impl AgentTool for ExtractAndWriteTool {
    fn name(&self) -> &'static str { "extract_and_write" }
    fn description(&self) -> &'static str { "Extracts the latest markdown code block from your thought process and writes it to a file. Use this for complex files to avoid JSON escaping issues. MUST wrap your code in triple backticks BEFORE calling this tool." }

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

    fn execute(&self, args: &Value, agent_content: &str) -> Result<String> {
        let path_str = args.get("path").and_then(|p| p.as_str()).ok_or_else(|| anyhow::anyhow!("Missing 'path' argument"))?;
        let path_owned = shellexpand::tilde(path_str).to_string();
        let path = path_owned.as_str();

        println!(">> [TOOL CALL: extract_and_write] Parsing thought process for target: {}", path);

        let blocks: Vec<&str> = agent_content.split("```").collect();
        if blocks.len() >= 3 {
            let code_block = blocks[blocks.len() - 2];
            let clean_code = if let Some(first_newline) = code_block.find('\n') {
                let first_line = &code_block[0..first_newline];
                if !first_line.contains(' ') {
                    &code_block[first_newline + 1..]
                } else {
                    code_block
                }
            } else {
                code_block
            };

            if let Some(parent) = std::path::PathBuf::from(path).parent() {
                if !parent.as_os_str().is_empty() {
                    std::fs::create_dir_all(parent)?;
                }
            }
            std::fs::write(path, clean_code.trim_matches('\n'))?;
            Ok(format!("Successfully extracted code block and wrote {} bytes to {}", clean_code.len(), path))
        } else {
            anyhow::bail!("Could not find a valid markdown code block (` ``` `) in your thought process to extract! You must write the code inside triple backticks explicitly before calling this tool.")
        }
    }
}
