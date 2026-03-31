use crate::tools::{AgentTool, RunCommandTool, ReadFileTool, WriteFileTool, PatchFileTool, RunBackgroundTool, ReadProcessLogsTool, ListDirTool, SearchWebTool, ReadUrlTool, SearchDirTool, AskUserTool, ExtractAndWriteTool, SystemInfoTool, SqliteQueryTool, GitTool, WatchDirectoryTool, HttpRequestTool, ClipboardTool, NotifyTool, FindReplaceTool, TreeTool, NetworkCheckTool, DiffFilesTool, KillProcessTool, EnvVarTool, ChmodTool, AppendFileTool, DownloadFileTool, TogglePlanningTool};
use anyhow::Result;
use colored::*;
use ollama_rs::{
    generation::{
        chat::{request::ChatMessageRequest, ChatMessage, MessageRole},
        options::GenerationOptions,
    },
    Ollama,
};
use serde_json::Value;
use futures::StreamExt;
use std::io::Write;
use syntect::parsing::SyntaxSet;
use syntect::highlighting::ThemeSet;

pub struct Agent {
    ollama: Ollama,
    model: String,
    history: Vec<ChatMessage>,
    tools: Vec<Box<dyn AgentTool>>,
    system_prompt: String,
    recent_tool_calls: std::collections::VecDeque<String>,
    history_path: String,
    pub planning_mode: bool,
    #[allow(dead_code)]
    pub session_id: String,
    #[allow(dead_code)]
    syntax_set: SyntaxSet,
    #[allow(dead_code)]
    theme_set: ThemeSet,
}

use std::sync::{Arc, Mutex};
use crate::memory::MemoryStore;

impl Agent {
    pub fn new(model: String, system_prompt: String, history_path: String, memory_store: Arc<Mutex<MemoryStore>>) -> Self {
        let mut agent = Agent {
            ollama: Ollama::default(),
            model,
            history: vec![],
            tools: vec![
                Box::new(crate::tools::StoreMemoryTool::new(memory_store.clone())),
                Box::new(crate::tools::RecallMemoryTool::new(memory_store.clone())),
                Box::new(crate::hardware::LinuxProcessAnalyzerTool),
                Box::new(crate::hardware::GpuDiagnosticsTool),
                Box::new(crate::hardware::TelemetryChartTool),
                Box::new(crate::telemetry::AdvancedSystemOracleTool),
                Box::new(crate::telemetry::KernelDiagnosticTool),
                Box::new(crate::telemetry::NetworkSnifferTool),
                Box::new(crate::tools::SystemdManagerTool),
                Box::new(RunCommandTool),
                Box::new(ReadFileTool),
                Box::new(WriteFileTool),
                Box::new(PatchFileTool),
                Box::new(RunBackgroundTool),
                Box::new(ReadProcessLogsTool),
                Box::new(ListDirTool),
                Box::new(SearchWebTool),
                Box::new(ReadUrlTool),
                Box::new(SearchDirTool),
                Box::new(AskUserTool),
                Box::new(ExtractAndWriteTool),
                Box::new(SystemInfoTool),
                Box::new(SqliteQueryTool),
                Box::new(GitTool),
                Box::new(WatchDirectoryTool),
                Box::new(HttpRequestTool),
                Box::new(ClipboardTool),
                Box::new(NotifyTool),
                Box::new(FindReplaceTool),
                Box::new(TreeTool),
                Box::new(NetworkCheckTool),
                Box::new(DiffFilesTool),
                Box::new(KillProcessTool),
                Box::new(EnvVarTool),
                Box::new(ChmodTool),
                Box::new(AppendFileTool),
                Box::new(DownloadFileTool),
                Box::new(TogglePlanningTool),
            ],
            system_prompt: String::new(),
            recent_tool_calls: std::collections::VecDeque::new(),
            history_path,
            planning_mode: true,
            session_id: uuid::Uuid::new_v4().to_string(),
            syntax_set: SyntaxSet::load_defaults_newlines(),
            theme_set: ThemeSet::load_defaults(),
        };

        // Dynamically inject tool descriptions into the system prompt
        let tool_desc = agent.get_tool_descriptions();
        let mut prompt = system_prompt.replace("{tool_descriptions}", &tool_desc);

        if let Ok(topics) = memory_store.lock().unwrap().list_topics() {
            if !topics.is_empty() {
                let topics_str = topics.join(", ");
                prompt.push_str(&format!("\n\n[SYSTEM MEMORY]: You have the following topics stored in your encrypted long-term memory: [{}]. Use the `recall_memory` tool to retrieve their full contents if they seem relevant.", topics_str));
            }
        }

        agent.system_prompt = prompt;
        
        // Add system message to history
        agent.history.push(ChatMessage::new(MessageRole::System, agent.system_prompt.clone()));
        
        agent
    }

    pub fn get_tool_descriptions(&self) -> String {
        let mut desc = String::new();
        for tool in &self.tools {
            let name = tool.name();
            let description = tool.description();
            let params = tool.parameters();
            
            // Format parameters as a concise JSON example or description
            let params_desc = if let Some(props) = params.get("properties").and_then(|p| p.as_object()) {
                let mut p_parts = Vec::new();
                for (k, v) in props {
                    let p_type = v.get("type").and_then(|t| t.as_str()).unwrap_or("string");
                    p_parts.push(format!("\"{}\": {}", k, p_type));
                }
                format!("{{ {} }}", p_parts.join(", "))
            } else {
                "{}".to_string()
            };

            desc.push_str(&format!("- {}: {}. {}\n", name, description, params_desc));
        }
        desc
    }

    #[allow(dead_code)]
    pub fn get_tool_names(&self) -> Vec<String> {
        self.tools.iter().map(|t| t.name().to_string()).collect()
    }

    pub async fn check_connection(&self) -> Result<()> {
        println!("{} Checking connection to Ollama...", "📡".blue());
        match self.ollama.list_local_models().await {
            Ok(_) => Ok(()),
            Err(e) => anyhow::bail!("Could not connect to Ollama. Please ensure 'ollama serve' or the Ollama app is running.\nError details: {}", e),
        }
    }

    pub fn load_history(&mut self) -> Result<()> {
        let history_path = std::path::Path::new(&self.history_path);
        if history_path.exists() {
            let data = std::fs::read_to_string(history_path)?;
            if let Ok(history) = serde_json::from_str::<Vec<ChatMessage>>(&data) {
                for msg in history {
                    if msg.role != MessageRole::System {
                        self.history.push(msg);
                    }
                }
                if !self.history.is_empty() {
                    println!("{} Loaded {} previous messages from history.", "📚".blue(), self.history.len());
                }
            }
        }
        Ok(())
    }

    pub fn save_history(&self) -> Result<()> {
        let history_path = std::path::Path::new(&self.history_path);
        let data = serde_json::to_string_pretty(&self.history)?;
        std::fs::write(history_path, data)?;
        Ok(())
    }

    pub fn clear_history(&mut self) {
        self.history.clear();
        let _ = std::fs::remove_file(&self.history_path);
    }

    pub async fn run(&mut self, initial_user_prompt: String) -> Result<()> {
        println!("{}", "=".repeat(60).blue());
        let build_time = env!("BUILD_TIME");
        println!("{} {} (Build: {})", "🚀".green(), "Tempest AI Agent Initialized".bold(), build_time.cyan());
        println!("{} {}", "Model:".blue(), self.model);
        println!("{}", "=".repeat(60).blue());

        // Initialize history with system prompt
        let full_system_prompt = self.system_prompt.clone();

        // If history has a system prompt but it's different from the current one, update it.
        // Otherwise, if no system prompt exists, insert it.
        if let Some(pos) = self.history.iter().position(|m| m.role == MessageRole::System) {
            if self.history[pos].content != full_system_prompt {
                self.history[pos] = ChatMessage::new(MessageRole::System, full_system_prompt);
            }
        } else {
            self.history.insert(0, ChatMessage::new(MessageRole::System, full_system_prompt));
        }
        self.history.push(ChatMessage::new(MessageRole::User, initial_user_prompt));
        let _ = self.save_history(); // Guarantee file creation immediately

        let max_iterations = 30;
        let mut iteration_count = 0;
        let mut guardrail_retries = 0;
        const MAX_GUARDRAIL_RETRIES: usize = 3;

        loop {
            iteration_count += 1;
            if iteration_count > max_iterations {
                println!("\n{}", "🛑 Execution limit reached (10 turns). Stopping to prevent infinite loop.".red());
                break;
            }
            // 🧠 Autonomously clear empty/useless messages from history before sending
            self.history.retain(|m| !m.content.trim().is_empty() || !m.tool_calls.is_empty());

            // 🧹 Auto-strip old [System Guardrail] messages to prevent history poisoning.
            // Keep only guardrail messages from the last 4 history entries.
            let guardrail_cutoff = self.history.len().saturating_sub(4);
            for i in 0..guardrail_cutoff {
                if self.history[i].content.contains("[System Guardrail]") {
                    self.history[i] = ChatMessage::new(MessageRole::User, "[trimmed]".to_string());
                }
            }
            self.history.retain(|m| m.content != "[trimmed]");

            // 🧠 Compress old history when it gets too long (instead of hard-dropping)
            let _ = self.auto_summarize_memory(false).await;

            // Build the request with moderate options (8k context)
            let options = GenerationOptions::default()
                .num_ctx(8192)
                .num_predict(4096);

            let request = ChatMessageRequest::new(
                self.model.clone(),
                self.history.clone(),
            ).options(options);

            let spinner = indicatif::ProgressBar::new_spinner();
            spinner.enable_steady_tick(std::time::Duration::from_millis(80));
            spinner.set_style(
                indicatif::ProgressStyle::default_spinner()
                    .tick_strings(&["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏", "✔"])
                    .template("{spinner:.cyan} {msg}")
                    .unwrap()
            );
            spinner.set_message("📡 Connected to Ollama. Thinking...".dimmed().to_string());

            let mut stream = self.ollama.send_chat_messages_stream(request).await?;
            let mut full_content = String::new();
            let mut in_thinking = false;
            let mut first_token = true;

            let theme = &self.theme_set.themes["base16-ocean.dark"];
            let mut highlighter: Option<syntect::easy::HighlightLines> = None;
            let mut line_buffer = String::new();
            let mut in_code_block = false;

            while let Some(res) = stream.next().await {
                if first_token {
                    spinner.finish_and_clear();
                    first_token = false;
                    print!("\n");
                }
                let chunk = res.map_err(|e| anyhow::anyhow!("Ollama stream error: {:?}", e))?;
                let text = chunk.message.content;
                full_content.push_str(&text);
                line_buffer.push_str(&text);

                // Process all full lines in the buffer
                while let Some(idx) = line_buffer.find('\n') {
                    let line = line_buffer[..=idx].to_string(); // keep newline
                    line_buffer = line_buffer[idx + 1..].to_string();

                    if line.starts_with("```") {
                        if in_code_block {
                            in_code_block = false;
                            highlighter = None;
                            print!("{}", line); 
                        } else {
                            in_code_block = true;
                            let lang = line.trim_start_matches("```").trim();
                            if let Some(syntax) = self.syntax_set.find_syntax_by_extension(lang).or_else(|| self.syntax_set.find_syntax_by_token(lang)) {
                                highlighter = Some(syntect::easy::HighlightLines::new(syntax, theme));
                            } else {
                                highlighter = Some(syntect::easy::HighlightLines::new(self.syntax_set.find_syntax_plain_text(), theme));
                            }
                            print!("{}", line);
                        }
                    } else if in_code_block {
                        if let Some(ref mut h) = highlighter {
                            let ranges: Vec<(syntect::highlighting::Style, &str)> = h.highlight_line(&line, &self.syntax_set).unwrap_or_default();
                            let escaped = syntect::util::as_24_bit_terminal_escaped(&ranges[..], true);
                            print!("{}", escaped);
                        } else {
                            print!("{}", line);
                        }
                    } else {
                        if line.contains("<think>") { in_thinking = true; }
                        
                        let clean_line = line.replace("<think>", &"<think>".bright_black().to_string())
                                             .replace("</think>", &"</think>".bright_black().to_string());
                        
                        if in_thinking {
                            print!("{}", clean_line.bright_black());
                        } else {
                            print!("{}", clean_line);
                        }
                        
                        if line.contains("</think>") { in_thinking = false; }
                    }
                    let _ = std::io::stdout().flush();
                }
            }

            // Print any remaining text in the buffer
            if !line_buffer.is_empty() {
                if in_thinking {
                    print!("{}", line_buffer.bright_black());
                } else {
                    print!("{}", line_buffer);
                }
                let _ = std::io::stdout().flush();
            }
            println!("\x1b[0m"); // Reset terminal colors


            // Only save if we actually got something
            if !full_content.trim().is_empty() {
                let message = ChatMessage::new(MessageRole::Assistant, full_content.clone());
                self.history.push(message);
                let _ = self.save_history();
            }
            let content = full_content;

            // Look for JSON blocks to execute tools (supports multiple per response)
            let mut executed_tools = false;

            match self.extract_tool_calls(&content) {
                Ok(tool_calls) if !tool_calls.is_empty() => {
                    guardrail_retries = 0; // Reset on successful parse
                    for tool_req in &tool_calls {
                        if let Some(tool_name) = tool_req.get("tool").and_then(|v| v.as_str()) {
                            let args = tool_req.get("arguments").unwrap_or(&Value::Null);

                            let current_call_hash = format!("{}|{}", tool_name, serde_json::to_string(args).unwrap_or_default());
                            if self.recent_tool_calls.contains(&current_call_hash) {
                                println!("\n{}", "❌ Loop Detected. Intercepting duplicate tool sequence...".red());
                                let guard_msg = "[System Guardrail] LOOP DETECTED. You just executed the exact same tool and arguments as a recent failed tool call. Pivot to a new strategy.".to_string();
                                self.history.push(ChatMessage::new(MessageRole::User, format!("TOOL RESULT for {}:\n{}", tool_name, guard_msg)));
                                let _ = self.save_history();
                                self.recent_tool_calls.clear();
                                continue;
                            }
                            self.recent_tool_calls.push_back(current_call_hash);
                            if self.recent_tool_calls.len() > 5 { self.recent_tool_calls.pop_front(); }

                            let tool_result_str;
                            if let Some(tool) = self.tools.iter().find(|t| t.name() == tool_name) {
                                let mut allowed = true;
                                let mut feedback = String::new();
                                if tool.requires_confirmation() {
                                    println!("\n{} \n{}", "⚠️  Agent wants to execute:".yellow().bold(), serde_json::to_string_pretty(args).unwrap_or_default().cyan());
                                    print!("Allow? [Y/n]: ");
                                    let _ = std::io::Write::flush(&mut std::io::stdout());
                                    let mut input = String::new();
                                    if std::io::stdin().read_line(&mut input).is_ok() {
                                        let ans = input.trim().to_lowercase();
                                        if ans != "y" && ans != "yes" && ans != "" {
                                            allowed = false;
                                            if ans != "n" && ans != "no" { feedback = input.trim().to_string(); }
                                        }
                                    }
                                } else {
                                    println!(); // Add a newline before auto-executing tools
                                }

                                if !allowed {
                                    tool_result_str = if feedback.is_empty() { "Error: User denied permission.".to_string() } else { format!("Error: User feedback: '{}'", feedback) };
                                } else {
                                    let tool_spinner = indicatif::ProgressBar::new_spinner();
                                    tool_spinner.enable_steady_tick(std::time::Duration::from_millis(80));
                                    tool_spinner.set_style(
                                        indicatif::ProgressStyle::default_spinner()
                                            .tick_strings(&["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏", "✔", "❌"])
                                            .template("{spinner:.magenta} {msg}")
                                            .unwrap()
                                    );
                                    tool_spinner.set_message(format!("{} {}", "Executing tool:".magenta().bold(), tool_name.cyan()));

                                    match tool.execute(args, &content).await {
                                        Ok(res) => {
                                            tool_spinner.finish_and_clear();
                                            println!("{} {} {}", "✔".green().bold(), "Tool execution successful:".green(), tool_name.cyan());
                                            tool_result_str = res;
                                        }
                                        Err(e) => {
                                            tool_spinner.finish_and_clear();
                                            let err_str = format!("{}", e);
                                            println!("{} {} {}", "❌".red().bold(), "Tool execution failed:".red(), err_str);
                                            let retry_hint = if err_str.contains("403") { " [HINT: Try a different URL.]" } 
                                                else if err_str.contains("404") { " [HINT: Page not found. Try searching again.]" }
                                                else if err_str.contains("timeout") { " [HINT: Server slow. Use network_check.]" }
                                                else { "" };
                                            tool_result_str = format!("Error: {}{}", e, retry_hint);
                                        }

                                    }
                                }
                            } else {
                                tool_result_str = format!("Error: Tool '{}' not found.", tool_name);
                            }

                            self.history.push(ChatMessage::new(MessageRole::User, format!("TOOL RESULT for {}:\n{}", tool_name, tool_result_str)));
                            let _ = self.save_history();
                            executed_tools = true;
                        }
                    }
                }
                Err(guardrail_msg) => {
                    guardrail_retries += 1;
                    if guardrail_retries >= MAX_GUARDRAIL_RETRIES {
                        println!("\n{}", "🛑 Max guardrail retries reached. Stopping this task.".red().bold());
                        println!("Error: {}", guardrail_msg);
                        break;
                    }
                    println!("\n{} {} ({}/{})", "⚠️  Agent syntax error:".yellow(), guardrail_msg, guardrail_retries, MAX_GUARDRAIL_RETRIES);
                    self.history.push(ChatMessage::new(MessageRole::User, format!("[System Guardrail] {}", guardrail_msg)));
                    let _ = self.save_history();
                    continue;
                }
                Ok(_) => {
                    guardrail_retries = 0; // Reset on clean response
                    // We removed the legacy string-matching nudge here because `extract_tool_calls`
                    // now has a much smarter logic to identify actual unsaved code vs informational blocks.
                }
            }

            if !executed_tools {
                println!("\n{}", "✅ Agent finished task.".green().bold());
                break;
            }
        }

        Ok(())
    }

    async fn auto_summarize_memory(&mut self, silent: bool) -> Result<()> {
        let max_history = 40;
        let num_to_summarize = 20;
        
        let chat_messages = self.history.len().saturating_sub(1);
        
        if chat_messages > max_history {
            if !silent {
                println!("\n{} {}", "🧠 Compressing old memories to preserve context window...".cyan().bold(), "");
            }
            
            let mut summary_text = String::new();
            for msg in self.history.iter().skip(1).take(num_to_summarize) {
                let role_str = match msg.role {
                    MessageRole::User => "User",
                    MessageRole::Assistant => "Agent",
                    MessageRole::System => "System Archive",
                    MessageRole::Tool => "Tool Feedback",
                };
                summary_text.push_str(&format!("{}: {}\n", role_str, msg.content));
            }
            
            let summary_prompt = format!(
                "Summarize the following conversation in a concise analytical paragraph. Focus on core objectives, any facts discovered, and the current state of progress. Do not output anything other than the summary itself.\n\n{}", 
                summary_text
            );
            
            let request = ChatMessageRequest::new(
                self.model.clone(),
                vec![ChatMessage::new(MessageRole::User, summary_prompt)],
            );
            
            if let Ok(response) = self.ollama.send_chat_messages(request).await {
                let summary = response.message.content;
                
                let mut new_history = vec![self.history[0].clone()];
                new_history.push(ChatMessage::new(MessageRole::System, format!("[Archived Memory of older turns]: {}", summary)));
                new_history.extend_from_slice(&self.history[(1 + num_to_summarize)..]);
                
                self.history = new_history;
                let _ = self.save_history();
                if !silent {
                    println!("{}", "✅ Memory compression complete.".green());
                }
            }
        }
        Ok(())
    }

    fn extract_tool_calls(&self, content: &str) -> Result<Vec<Value>, String> {
        let mut calls = Vec::new();
        let mut search_from = 0;
        
        while let Some(start) = content[search_from..].find("```json") {
            let abs_start = search_from + start + 7;
            if let Some(end_offset) = content[abs_start..].find("```") {
                let block = content[abs_start..abs_start + end_offset].trim();
                
                // 🚑 PRE-PARSE RESCUE: If the block contains shell redirection, it's almost certainly a mangled write_file.
                if block.contains("<<EOF") || block.contains("cat >") || block.contains("$(") {
                    let re_path = regex::Regex::new(r#""path"\s*:\s*"(./)?([^"]+)""#).unwrap();
                    if let Some(p_cap) = re_path.captures(block) {
                        let path = p_cap.get(2).unwrap().as_str();
                         // println!("{}", format!("🚑 Pre-Parse Rescue: Detected shell-injection intent for '{}'. Forcing extract_and_write.", path).yellow());
                         calls.push(serde_json::json!({
                            "tool": "extract_and_write",
                            "arguments": { "path": path }
                        }));
                        search_from = abs_start + end_offset + 3;
                        continue;
                    }
                }

                match serde_json::from_str::<Value>(block) {
                    Ok(mut val) => {
                        if val.get("tool").is_some() {
                            if val.get("arguments").is_none() {
                                if let Some(obj) = val.as_object_mut() {
                                    let mut args_map = serde_json::Map::new();
                                    let keys: Vec<String> = obj.keys().cloned().collect();
                                    for k in keys {
                                        if k != "tool" {
                                            if let Some(v) = obj.remove(&k) {
                                                args_map.insert(k, v);
                                            }
                                        }
                                    }
                                    obj.insert("arguments".to_string(), serde_json::Value::Object(args_map));
                                }
                            }
                            
                            let tool_name = val.get("tool").and_then(|t| t.as_str()).unwrap_or("");
                            let args = val.get("arguments").and_then(|a| a.as_object());

                            // 🚨 SHELL INJECTION GUARDRAIL: Catch if AI puts shell scripts inside write_file
                            if tool_name == "write_file" {
                                if let Some(content_val) = args.and_then(|a| a.get("content")).and_then(|c| c.as_str()) {
                                    if content_val.contains("<<EOF") || content_val.contains("cat >") || content_val.contains("$(") {
                                        let path = args.and_then(|a| a.get("path")).and_then(|p| p.as_str()).unwrap_or("file");
                                        // println!("{}", "🚑 Auto-Rescue: Redirecting shell-injection 'write_file' → 'extract_and_write'".yellow());
                                        calls.push(serde_json::json!({
                                            "tool": "extract_and_write",
                                            "arguments": { "path": path }
                                        }));
                                        search_from = abs_start + end_offset + 3;
                                        continue;
                                    }
                                }
                            }
                            calls.push(val);
                        }
                    }
                    Err(_) => {
                        // 🚑 EMERGENCY RECOVERY: Rescue from malformed JSON
                        // This regex is robust to missing braces or line breaks
                        let re_tool = regex::Regex::new(r#""tool"\s*:\s*"([^"]+)""#).unwrap();
                        let re_path = regex::Regex::new(r#""path"\s*:\s*"([^"]+)""#).unwrap();
                        
                        if let (Some(t_cap), Some(p_cap)) = (re_tool.captures(block), re_path.captures(block)) {
                            let tool_name = t_cap.get(1).unwrap().as_str();
                            let path = p_cap.get(1).unwrap().as_str();
                            
                            if !tool_name.is_empty() && !path.is_empty() {
                                // println!("{}", format!("🚑 Emergency Recovery: Rescued '{}' for '{}' from malformed JSON.", tool_name, path).yellow());
                                let target_tool = if tool_name == "write_file" { "extract_and_write" } else { tool_name };
                                calls.push(serde_json::json!({
                                    "tool": target_tool,
                                    "arguments": { "path": path }
                                }));
                                search_from = abs_start + end_offset + 3;
                                continue;
                            }
                        }
                        
                        // 🚨 HARD BREAK: If we actually SAW a JSON block but couldn't parse or rescue it, we MUST error. 
                        // Returning an empty call list at this point would let the model continue thinking it succeeded.
                        return Err(format!("[System Guardrail] CRITICAL: Invalid JSON in code block. I saw a ```json block but was unable to parse it correctly. If you are trying to save code, use: ```json\n{{ \"tool\": \"extract_and_write\", \"arguments\": {{ \"path\": \"filename\" }} }}\n```"));
                    }
                }
                search_from = abs_start + end_offset + 3;
            } else {
                break;
            }
        }
        
        // Final fallback: If no JSON tools found but model wrote a code block AND mentioned "save"/"extract"/"write"
        if calls.is_empty() {
            let has_code = content.contains("```");
            let is_json_intent = content.contains("```json"); // Special check for malformed intents
            let wants_to_save = content.to_lowercase().contains("save") || content.to_lowercase().contains("extract") || content.to_lowercase().contains("write");
            
            if (has_code || is_json_intent) && wants_to_save {
                let re_path = regex::Regex::new(r#"(?:path|to|file|as)\s*['":\s]+([^"'\s,]+)"#).unwrap();
                if let Some(cap) = re_path.captures(content) {
                    let path = cap.get(1).unwrap().as_str().trim_matches('.');
                    if !path.is_empty() && path.contains('.') {
                         // println!("{}", format!("🚑 Heuristic Recovery: Detected intent to save '{}'. Triggering extract_and_write.", path).yellow());
                         calls.push(serde_json::json!({
                            "tool": "extract_and_write",
                            "arguments": { "path": path }
                        }));
                        return Ok(calls);
                    }
                }
                
                // If it looks like a save intent but we can't find a path, don't just finish silently.
                if is_json_intent {
                    return Err("[System Guardrail] I detected a ```json block but could not parse any valid tool arguments. Please specify the tool and arguments clearly.".to_string());
                }
            }
        }

        // Only flag blocks as needing a tool if they look like actual code scripts (multi-line)
        if calls.is_empty() && !content.contains("tool") {
            let blocks: Vec<&str> = content.split("```").collect();
            for i in (1..blocks.len()).step_by(2) {
                let b_orig = blocks[i];
                let first_newline_idx = b_orig.find('\n').unwrap_or(b_orig.len());
                let lang_tag = b_orig[..first_newline_idx].trim().to_lowercase();
                
                let b_trimmed = b_orig.trim();
                let lines = b_trimmed.lines().count();
                
                let ignore_tags = ["json", "", "txt", "text", "log", "output", "console", "markdown", "md", "sh", "bash", "zsh"];
                
                if lines > 3 && !ignore_tags.contains(&lang_tag.as_str()) {
                     return Err("You provided a code block but did not call a tool. To save it, add: ```json\n{ \"tool\": \"extract_and_write\", \"arguments\": { \"path\": \"filename\" } }\n```".to_string());
                }
                
                if lines > 5 && ["sh", "bash", "zsh"].contains(&lang_tag.as_str()) {
                     return Err("You provided a script but did not call a tool to save it. To save it, add: ```json\n{ \"tool\": \"extract_and_write\", \"arguments\": { \"path\": \"script.sh\" } }\n```".to_string());
                }
            }
        }
        
        Ok(calls)
    }

    pub async fn run_tui_mode(&mut self, mut user_rx: tokio::sync::mpsc::Receiver<String>, tx: tokio::sync::mpsc::Sender<crate::tui::AgentEvent>, mut tool_rx: tokio::sync::mpsc::Receiver<bool>, stop_flag: std::sync::Arc<std::sync::atomic::AtomicBool>) -> Result<()> {
        let full_system_prompt = self.system_prompt.clone();
        
        while let Some(msg) = user_rx.recv().await {
            // Note: This logic was adapted to match the requested signature change
            stop_flag.store(false, std::sync::atomic::Ordering::SeqCst);
            if let Some(pos) = self.history.iter().position(|m| m.role == MessageRole::System) {
                if self.history[pos].content != full_system_prompt {
                    self.history[pos] = ChatMessage::new(MessageRole::System, full_system_prompt.clone());
                }
            } else {
                self.history.insert(0, ChatMessage::new(MessageRole::System, full_system_prompt.clone()));
            }

            self.history.push(ChatMessage::new(MessageRole::User, msg));
            let _ = self.save_history();
            
            let mut guardrail_retries = 0;
            const MAX_GUARDRAIL_RETRIES: usize = 3;

            loop {
                self.history.retain(|m| !m.content.trim().is_empty() || !m.tool_calls.is_empty());
                let guardrail_cutoff = self.history.len().saturating_sub(4);
                for i in 0..guardrail_cutoff {
                    if self.history[i].content.contains("[System Guardrail]") {
                        self.history[i] = ChatMessage::new(MessageRole::User, "[trimmed]".to_string());
                    }
                }
                self.history.retain(|m| m.content != "[trimmed]");
                let _ = self.auto_summarize_memory(true).await;

                let options = GenerationOptions::default()
                    .num_ctx(8192)
                    .num_predict(4096)
                    .repeat_penalty(1.1)
                    .temperature(0.7);
                let request = ChatMessageRequest::new(self.model.clone(), self.history.clone()).options(options);

                let mut stream = match self.ollama.send_chat_messages_stream(request).await {
                    Ok(s) => s,
                    Err(e) => {
                        let _ = tx.send(crate::tui::AgentEvent::SystemUpdate(format!("Ollama Error: {}", e))).await;
                        break;
                    }
                };
                
                let mut full_content = String::new();

                while let Some(res) = stream.next().await {
                    if stop_flag.load(std::sync::atomic::Ordering::SeqCst) {
                        full_content.push_str("\n\n[USER INTERRUPTED GENERATION]");
                        let _ = tx.send(crate::tui::AgentEvent::StreamToken("\n\n🛑 [GENERATION STOPPED]".to_string())).await;
                        break;
                    }
                    if let Ok(chunk) = res {
                        let text = chunk.message.content;
                        full_content.push_str(&text);
                        let _ = tx.send(crate::tui::AgentEvent::StreamToken(text)).await;
                    }
                }

                if !full_content.trim().is_empty() {
                    self.history.push(ChatMessage::new(MessageRole::Assistant, full_content.clone()));
                    let _ = self.save_history();
                }

                // Flush the stream token buffer to the permanent UI chat log BEFORE tool execution
                let _ = tx.send(crate::tui::AgentEvent::Done).await;

                let mut executed_tools = false;
                match self.extract_tool_calls(&full_content) {
                    Ok(tool_calls) if !tool_calls.is_empty() => {
                        guardrail_retries = 0;
                        for tool_req in &tool_calls {
                            if let Some(tool_name) = tool_req.get("tool").and_then(|v| v.as_str()) {
                                let args = tool_req.get("arguments").unwrap_or(&serde_json::Value::Null);
                                
                                let current_call_hash = format!("{}|{}", tool_name, serde_json::to_string(args).unwrap_or_default());
                                if self.recent_tool_calls.contains(&current_call_hash) {
                                    let diag = format!("🛑 [System Guardrail] LOOP DETECTED: You have called tool '{}' with identical arguments twice in a row. ABORTING PROJECT TO PREVENT SPAM.", tool_name);
                                    let _ = tx.send(crate::tui::AgentEvent::SystemUpdate(diag.clone())).await;
                                    self.history.push(ChatMessage::new(MessageRole::User, diag));
                                    self.recent_tool_calls.clear();
                                    executed_tools = false; // Force stop
                                    break; 
                                }
                                self.recent_tool_calls.push_back(current_call_hash);
                                if self.recent_tool_calls.len() > 5 { self.recent_tool_calls.pop_front(); }

                                let tool_result_str;
                                if let Some(tool) = self.tools.iter().find(|t| t.name() == tool_name) {
                                    // 🧠 PLANNING MODE GUARD: Block modifying tools in planning mode
                                    if self.planning_mode && tool.is_modifying() {
                                        tool_result_str = format!("[System Guardrail] PLANNING MODE ACTIVE: Tool '{}' modifies system state and is BLOCKED during planning. Present your implementation plan to the user first. Once they approve, use the `toggle_planning` tool to switch to Execution mode.", tool_name);
                                        let _ = tx.send(crate::tui::AgentEvent::SystemUpdate(format!("🧠 Blocked '{}' — Agent is in PLANNING mode", tool_name))).await;
                                    } else {
                                        let mut allowed = true;
                                        if tool.requires_confirmation() {
                                            // Drain the channel first to remove any stale inputs from previous calls
                                            while let Ok(_) = tool_rx.try_recv() {}
                                            
                                            let _ = tx.send(crate::tui::AgentEvent::RequestConfirmation(tool_name.to_string(), serde_json::to_string_pretty(args).unwrap_or_default())).await;
                                            allowed = tool_rx.recv().await.unwrap_or(false);
                                        }

                                        if allowed {
                                            let _ = tx.send(crate::tui::AgentEvent::ToolStart(tool_name.to_string())).await;
                                            match tool.execute(args, &full_content).await {
                                                Ok(res) => tool_result_str = res,
                                                Err(e) => tool_result_str = format!("Error: {}", e),
                                            }
                                            let _ = tx.send(crate::tui::AgentEvent::ToolFinish).await;

                                            // ✅ VERIFICATION LOOP: After any modifying tool, inject a verification prompt
                                            if tool.is_modifying() && !tool_result_str.starts_with("Error") {
                                                let verify_msg = format!(
                                                    "TOOL RESULT for {}:\n{}\n\n[System Verification Required] You just executed a modifying action ('{}').\
                                                    \nBEFORE continuing or declaring success, you MUST verify your work.\
                                                    \n- If you wrote a file: use `read_file` or `run_command` with `cat` to confirm the contents.\
                                                    \n- If you ran a command: check the output for errors.\
                                                    \n- If you edited code: use `run_command` to run the compiler/linter.\
                                                    \nDo NOT skip this step.",
                                                    tool_name, tool_result_str, tool_name
                                                );
                                                self.history.push(ChatMessage::new(MessageRole::User, verify_msg));
                                                let _ = self.save_history();
                                                executed_tools = true;
                                                continue; // Skip the normal push below
                                            }
                                        } else {
                                            tool_result_str = "Error: User denied permission via TUI Modal.".to_string();
                                        }
                                    }
                                } else {
                                    tool_result_str = format!("Error: No such tool '{}'", tool_name);
                                }
                                
                                self.history.push(ChatMessage::new(MessageRole::User, format!("TOOL RESULT for {}:\n{}", tool_name, tool_result_str)));
                                
                                // 🧠 SENTINEL DETECTION: Check if the tool result contains a planning mode toggle
                                if tool_result_str.contains("[PLANNING_MODE_ON]") {
                                    self.planning_mode = true;
                                    let _ = tx.send(crate::tui::AgentEvent::SystemUpdate("🧠 Agent entered PLANNING mode".to_string())).await;
                                } else if tool_result_str.contains("[PLANNING_MODE_OFF]") {
                                    self.planning_mode = false;
                                    let _ = tx.send(crate::tui::AgentEvent::SystemUpdate("⚡ Agent entered EXECUTION mode".to_string())).await;
                                }
                                
                                let _ = self.save_history();
                                executed_tools = true;
                            }
                        }
                    }
                    Ok(_) => {}
                    Err(e) => {
                        if guardrail_retries < MAX_GUARDRAIL_RETRIES {
                            guardrail_retries += 1;
                            self.history.push(ChatMessage::new(MessageRole::User, format!("[System Guardrail] JSON parsing failed: {}. REPAIR IT AND TRY AGAIN.", e)));
                            executed_tools = true; 
                        }
                    }
                }

                if !executed_tools {
                    break;
                }
                
                // Final safety: Prune history of redundant guardrail messages to prevent context drift
                let mut prune_indices = Vec::new();
                let mut guardrail_streak = 0;
                for (i, msg) in self.history.iter().enumerate().rev() {
                    if msg.content.contains("[System Guardrail]") {
                        guardrail_streak += 1;
                        if guardrail_streak > 2 {
                            prune_indices.push(i);
                        }
                    } else {
                        guardrail_streak = 0;
                    }
                }
                for i in prune_indices {
                    if i < self.history.len() {
                        self.history.remove(i);
                    }
                }
            } // end loop
        } // end while user_rx
        Ok(())
    }
}
