use crate::tools::{AgentTool, RunCommandTool, ReadFileTool, WriteFileTool, PatchFileTool, RunBackgroundTool, ReadProcessLogsTool, ListDirTool, SearchWebTool, ReadUrlTool, SearchDirTool, AskUserTool, ExtractAndWriteTool, SystemInfoTool, SqliteQueryTool, GitTool, WatchDirectoryTool, HttpRequestTool, ClipboardTool, NotifyTool, FindReplaceTool, TreeTool, NetworkCheckTool, DiffFilesTool, KillProcessTool, EnvVarTool, ChmodTool, AppendFileTool, DownloadFileTool};
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

pub struct Agent {
    ollama: Ollama,
    model: String,
    history: Vec<ChatMessage>,
    tools: Vec<Box<dyn AgentTool>>,
    system_prompt: String,
    recent_tool_calls: std::collections::VecDeque<String>,
    history_path: String,
    #[allow(dead_code)]
    pub session_id: String,
}

impl Agent {
    pub fn new(model: String, system_prompt: String, history_path: String) -> Self {
        Agent {
            ollama: Ollama::default(),
            model,
            history: vec![],
            tools: vec![
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
            ],
            system_prompt,
            recent_tool_calls: std::collections::VecDeque::new(),
            history_path,
            session_id: uuid::Uuid::new_v4().to_string(),
        }
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
        println!("{}", "🚀 Tempest AI Agent Initialized".green().bold());
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
            let _ = self.auto_summarize_memory().await;

            // Build the request with moderate options (8k context)
            let options = GenerationOptions::default()
                .num_ctx(8192)
                .num_predict(4096);

            let request = ChatMessageRequest::new(
                self.model.clone(),
                self.history.clone(),
            ).options(options);

            println!("{}", "📡 Connected to Ollama. Streaming response...".dimmed());
            let mut stream = self.ollama.send_chat_messages_stream(request).await?;
            let mut full_content = String::new();
            let mut in_thinking = false;

            print!("\n"); 
            while let Some(res) = stream.next().await {
                let chunk = res.map_err(|e| anyhow::anyhow!("Ollama stream error: {:?}", e))?;
                let text = chunk.message.content;
                full_content.push_str(&text);

                // Live Highlight Thinking tags
                if text.contains("<think>") {
                    in_thinking = true;
                    print!("{}", "<think>".bright_black());
                }
                
                let clean_text = text.replace("<think>", "").replace("</think>", "");
                if in_thinking {
                    print!("{}", clean_text.bright_black());
                } else if !clean_text.is_empty() {
                    print!("{}", clean_text);
                } else {
                    // Pulse to show we are receiving data even if text is empty
                    print!("{}", ".".dimmed());
                }

                if text.contains("</think>") {
                    in_thinking = false;
                    println!("{}", "</think>".bright_black());
                }
                let _ = std::io::stdout().flush();
            }
            println!();

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
                                println!("\n{} {}", "🛠️  Attempting to run:".magenta().bold(), tool_name);
                                
                                let mut allowed = true;
                                let mut feedback = String::new();
                                if tool.requires_confirmation() {
                                    println!("{} \n{}", "⚠️  Agent wants to execute:".yellow().bold(), serde_json::to_string_pretty(args).unwrap_or_default().cyan());
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
                                }

                                if !allowed {
                                    tool_result_str = if feedback.is_empty() { "Error: User denied permission.".to_string() } else { format!("Error: User feedback: '{}'", feedback) };
                                } else {
                                    match tool.execute(args, &content).await {
                                        Ok(res) => {
                                            println!("{}", "✅ Tool execution successful".green());
                                            tool_result_str = res;
                                        }
                                        Err(e) => {
                                            let err_str = format!("{}", e);
                                            println!("{} {}", "❌ Tool execution failed:".red(), err_str);
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
                    // Only nudge if the model outputs a non-shell code block without tool calls.
                    // Shell blocks (sh, bash, shell, zsh) are allowed as suggestions.
                    let has_non_shell_code = content.contains("```") 
                        && !content.contains("```sh") 
                        && !content.contains("```bash") 
                        && !content.contains("```shell")
                        && !content.contains("```zsh");
                    if !executed_tools && has_non_shell_code && !content.to_lowercase().contains("finished task") {
                        guardrail_retries += 1;
                        if guardrail_retries >= MAX_GUARDRAIL_RETRIES {
                            println!("\n{}", "🛑 Max nudge retries reached. Stopping.".red().bold());
                            break;
                        }
                        println!("\n{}", "⚠️  Agent provided code but forgot tools. Nudging...".yellow().bold());
                        let nudge = "[System Guardrail] You provided code but didn't use tools like `extract_and_write`. YOU MUST USE TOOLS. Rewrite your response using tool calls.".to_string();
                        self.history.push(ChatMessage::new(MessageRole::User, nudge));
                        let _ = self.save_history();
                        continue;
                    }
                }
            }

            if !executed_tools {
                println!("\n{}", "✅ Agent finished task.".green().bold());
                break;
            }
        }

        Ok(())
    }

    async fn auto_summarize_memory(&mut self) -> Result<()> {
        let max_history = 40;
        let num_to_summarize = 20;
        
        let chat_messages = self.history.len().saturating_sub(1);
        
        if chat_messages > max_history {
            println!("\n{} {}", "🧠 Compressing old memories to preserve context window...".cyan().bold(), "");
            
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
                println!("{}", "✅ Memory compression complete.".green());
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
                match serde_json::from_str::<Value>(block) {
                    Ok(val) => {
                        if val.get("tool").is_some() && val.get("arguments").is_some() {
                            calls.push(val);
                        }
                    }
                    Err(e) => {
                        let err_msg = format!("{}", e);
                        
                        // 🚑 EMERGENCY RECOVERY: Rescue tool name + path from malformed JSON
                        let re_tool = regex::Regex::new(r#""tool"\s*:\s*"([^"]+)""#).unwrap();
                        let re_path = regex::Regex::new(r#""path"\s*:\s*"([^"]+)""#).unwrap();
                        
                        if let (Some(t_cap), Some(p_cap)) = (re_tool.captures(block), re_path.captures(block)) {
                            let tool_name = t_cap.get(1).unwrap().as_str();
                            let path = p_cap.get(1).unwrap().as_str();
                            
                            // Auto-rescue: redirect ANY file-writing tool with escape errors to extract_and_write
                            if tool_name == "extract_and_write" || tool_name == "write_file" {
                                let rescued_tool = "extract_and_write";
                                if tool_name == "write_file" {
                                    println!("{}", "🚑 Auto-Rescue: Redirecting broken 'write_file' → 'extract_and_write'".yellow());
                                } else {
                                    println!("{}", "🚑 Emergency Recovery: Rescued 'extract_and_write' from malformed JSON.".yellow());
                                }
                                calls.push(serde_json::json!({
                                    "tool": rescued_tool,
                                    "arguments": { "path": path }
                                }));
                                search_from = abs_start + end_offset + 3;
                                continue;
                            }
                        }

                        let hint = if err_msg.contains("invalid escape") {
                            " [HINT: Use 'extract_and_write' with ONLY a 'path' argument. DO NOT include 'content'. The code block above will be extracted automatically.]"
                        } else {
                            ""
                        };
                        return Err(format!("Invalid JSON inside code block: {}{}", e, hint));
                    }
                }
                search_from = abs_start + end_offset + 3;
            } else {
                break;
            }
        }
        
        // Only flag non-shell/non-json code blocks without tool calls as needing a tool.
        let has_non_shell_code = content.contains("```") 
            && !content.contains("```sh") 
            && !content.contains("```bash") 
            && !content.contains("```shell")
            && !content.contains("```zsh")
            && !content.contains("```json");
        if calls.is_empty() && has_non_shell_code && !content.contains("tool") {
             return Err("You provided a code block but did not call a tool. Add a ```json block with: { \"tool\": \"extract_and_write\", \"arguments\": { \"path\": \"filename\" } }".to_string());
        }
        
        Ok(calls)
    }
}
