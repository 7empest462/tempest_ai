use crate::tools::{AgentTool, RunCommandTool, ReadFileTool, WriteFileTool, PatchFileTool, RunBackgroundTool, ReadProcessLogsTool, ListDirTool, SearchWebTool, ReadUrlTool, SearchDirTool, AskUserTool, ExtractAndWriteTool, SystemInfoTool, SqliteQueryTool, GitTool, WatchDirectoryTool, HttpRequestTool, ClipboardTool, NotifyTool, FindReplaceTool, TreeTool, NetworkCheckTool};
use anyhow::Result;
use colored::*;
use ollama_rs::{
    generation::chat::{request::ChatMessageRequest, ChatMessage, MessageRole},
    Ollama,
};
use serde_json::Value;

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
        // We append the tools description to the system prompt so any model (even without native tool API support) 
        // can use the JSON output fallback.
        let mut full_system_prompt = self.system_prompt.clone();
        full_system_prompt.push_str("\n\nAVAILABLE TOOLS:\n");
        for tool in &self.tools {
            full_system_prompt.push_str(&format!("- {}: {}\n  Schema: {}\n", tool.name(), tool.description(), tool.parameters().to_string()));
        }
        full_system_prompt.push_str(
            "\nTO USE A TOOL, output exactly in this format inside a JSON block:\n```json\n{\n  \"tool\": \"tool_name\",\n  \"arguments\": {\"arg1\": \"value\"}\n}\n```\nAfter tool execution, you will receive the result and can use another tool or provide the final answer."
        );

        if !self.history.iter().any(|m| m.role == MessageRole::System) {
            self.history.insert(0, ChatMessage::new(MessageRole::System, full_system_prompt));
        }
        self.history.push(ChatMessage::new(MessageRole::User, initial_user_prompt));

        loop {
            // 🧠 Autonomously compress the context window if it gets too large
            let _ = self.auto_summarize_memory().await;

            // Build the request
            let request = ChatMessageRequest::new(
                self.model.clone(),
                self.history.clone(),
            );

            // Execute the model with a loading spinner
            let spinner = indicatif::ProgressBar::new_spinner();
            spinner.set_style(
                indicatif::ProgressStyle::default_spinner()
                    .tick_strings(&["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"])
                    .template("{spinner:.cyan} {msg}")
                    .unwrap()
            );
            spinner.set_message("Thinking...");
            spinner.enable_steady_tick(std::time::Duration::from_millis(80));
            
            let response = self.ollama.send_chat_messages(request).await?;
            spinner.finish_and_clear();
            
            let message = response.message;
            let content = message.content.clone();

            // Check if there are think tags and print them nicely (specific to DeepSeek-R1)
            let mut display_content = content.clone();
            if let (Some(start), Some(close)) = (display_content.find("<think>"), display_content.find("</think>")) {
                let end = close + "</think>".len();
                if let Some(thought) = display_content.get(start..end) {
                    println!("{}", thought.bright_black());
                }
                display_content = display_content.get(end..).unwrap_or("").trim().to_string();
            }

            if !display_content.is_empty() {
                // Render markdown output through termimad
                termimad::print_text(&display_content);
            }

            self.history.push(message);
            let _ = self.save_history();

            // Look for JSON blocks to execute tools (supports multiple per response)
            let mut executed_tools = false;

            match self.extract_tool_calls(&content) {
                Ok(tool_calls) if !tool_calls.is_empty() => {
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
                    println!("\n{} {}", "⚠️  Agent syntax error:".yellow(), guardrail_msg);
                    self.history.push(ChatMessage::new(MessageRole::User, format!("[System Guardrail] {}", guardrail_msg)));
                    let _ = self.save_history();
                    continue;
                }
                Ok(_) => {
                    if !executed_tools && content.contains("```") && !content.to_lowercase().contains("finished task") {
                        println!("\n{}", "⚠️  Agent provided code but forgot tools. Nudging...".yellow().bold());
                        let nudge = "[System Guardrail] You provided code but didn't use tools like `write_file` or `extract_and_write`. YOU MUST USE TOOLS. Rewrite your response using tool calls.".to_string();
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
        let max_history = 15;
        let num_to_summarize = 10;
        
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
                        if calls.is_empty() {
                            return Err(format!("Invalid JSON inside code block: {}", e));
                        }
                    }
                }
                search_from = abs_start + end_offset + 3;
            } else {
                break;
            }
        }
        
        if calls.is_empty() {
            // Anti-hallucination guardrail check
            if content.contains("```bash") || content.contains("```sh") {
                return Err("You provided a bash/sh code block. You MUST use the `run_command` tool within a strict ```json block to run commands yourself. DO NOT tell the user to run commands. Fix your response and use the run_command tool correctly.".to_string());
            }
        }
        
        Ok(calls)
    }
}
