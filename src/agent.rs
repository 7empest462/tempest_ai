use crate::tools::{AgentTool, RunCommandTool, ReadFileTool, WriteFileTool, PatchFileTool, RunBackgroundTool, ReadProcessLogsTool, ListDirTool, SearchWebTool, ReadUrlTool, SearchDirTool, AskUserTool, ExtractAndWriteTool, SystemInfoTool, SqliteQueryTool, GitTool, WatchDirectoryTool};
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
}

impl Agent {
    pub fn new(model: String, system_prompt: String) -> Self {
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
            ],
            system_prompt,
            recent_tool_calls: std::collections::VecDeque::new(),
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
        let history_path = std::path::Path::new("history.json");
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
        let history_path = std::path::Path::new("history.json");
        let data = serde_json::to_string_pretty(&self.history)?;
        std::fs::write(history_path, data)?;
        Ok(())
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

            // Execute the model
            println!("\n{}", "🤔 Thinking...".yellow());
            let response = self.ollama.send_chat_messages(request).await?;
            let message = response.message;
            let content = message.content.clone();

            // Check if there are think tags and print them nicely (specific to DeepSeek-R1)
            let mut display_content = content.clone();
            if display_content.contains("<think>") && display_content.contains("</think>") {
                let start = display_content.find("<think>").unwrap();
                let end = display_content.find("</think>").unwrap() + 9;
                let thought = &display_content[start..end];
                println!("{}", thought.bright_black());
                display_content = display_content[end..].trim().to_string();
            }

            if !display_content.is_empty() {
                println!("{}", display_content.cyan());
            }

            self.history.push(message);
            let _ = self.save_history();

            // Look for JSON block to execute tools
            let mut executed_tools = false;

            match self.extract_tool_call(&content) {
                Ok(Some(tool_req)) => {
                    if let Some(tool_name) = tool_req.get("tool").and_then(|v| v.as_str()) {
                        let args = tool_req.get("arguments").unwrap_or(&Value::Null);

                        let current_call_hash = format!("{}|{}", tool_name, serde_json::to_string(args).unwrap_or_default());
                        if self.recent_tool_calls.contains(&current_call_hash) {
                            println!("\n{}", "❌ Loop Detected. Intercepting duplicate tool sequence...".red());
                            let guard_msg = "[System Guardrail] LOOP DETECTED. You just executed the exact same tool and arguments as a recent failed tool call, which means your execution sequence is stuck in a hallucination loop. You MUST pivot to an entirely new strategy or ask the user for help. Do NOT repeat yourself.".to_string();
                            let tool_result_msg = format!("TOOL RESULT for {}:\n{}", tool_name, guard_msg);
                            self.history.push(ChatMessage::new(MessageRole::User, tool_result_msg));
                            let _ = self.save_history();
                            self.recent_tool_calls.clear();
                            continue;
                        }
                        self.recent_tool_calls.push_back(current_call_hash);
                        if self.recent_tool_calls.len() > 5 {
                            self.recent_tool_calls.pop_front();
                        }

                        // Execute tool
                        let tool_result_str;
                        if let Some(tool) = self.tools.iter().find(|t| t.name() == tool_name) {
                            println!("\n{} {}", "🛠️  Attempting to run:".magenta().bold(), tool_name);
                            
                            let mut allowed = true;
                            let mut feedback = String::new();
                            if tool.requires_confirmation() {
                                println!("{} \n{}", "⚠️  Agent wants to execute the following tool parameters:".yellow().bold(), serde_json::to_string_pretty(args).unwrap_or_default().cyan());
                                print!("Allow execution? [Y/n] (Or type instructions to correct): ");
                                let _ = std::io::Write::flush(&mut std::io::stdout());
                                
                                let mut input = String::new();
                                if std::io::stdin().read_line(&mut input).is_ok() {
                                    let ans = input.trim().to_lowercase();
                                    if ans != "y" && ans != "yes" && ans != "" {
                                        allowed = false;
                                        if ans != "n" && ans != "no" {
                                            feedback = input.trim().to_string();
                                        }
                                    }
                                }
                            }

                            if !allowed {
                                println!("{}", "❌ Tool execution denied by user.".red());
                                if feedback.is_empty() {
                                    tool_result_str = "Error: User denied permission to execute this tool. Re-evaluate your approach or ask the user for clarification.".to_string();
                                } else {
                                    tool_result_str = format!("Error: User denied permission and provided this feedback: '{}'. Adjust your execution plan accordingly.", feedback);
                                }
                            } else {
                                match tool.execute(args, &content) {
                                    Ok(res) => {
                                        println!("{}", "✅ Tool execution successful".green());
                                        tool_result_str = res;
                                    }
                                    Err(e) => {
                                        println!("{} {}", "❌ Tool execution failed:".red(), e);
                                        tool_result_str = format!("Error executing tool: {}", e);
                                    }
                                }
                            }
                        } else {
                            tool_result_str = format!("Error: Tool '{}' not found", tool_name);
                        }

                        // Feed result back to the model
                        let tool_result_msg = format!("TOOL RESULT for {}:\n{}", tool_name, tool_result_str);
                        self.history.push(ChatMessage::new(MessageRole::User, tool_result_msg));
                        let _ = self.save_history();
                        executed_tools = true;
                    }
                }
                Err(guardrail_msg) => {
                    // System Guardrail: Force self-correction
                    println!("\n{} {}", "⚠️  Agent syntax error, forcing self-correction:".yellow(), guardrail_msg);
                    self.history.push(ChatMessage::new(MessageRole::User, format!("[System Guardrail] {}", guardrail_msg)));
                    let _ = self.save_history();
                    continue;
                }
                Ok(None) => {}
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

    fn extract_tool_call(&self, content: &str) -> Result<Option<Value>, String> {
        if let Some(start) = content.find("```json") {
            let after_start = &content[start + 7..];
            if let Some(end_offset) = after_start.find("```") {
                let block = after_start[..end_offset].trim();
                return match serde_json::from_str::<Value>(block) {
                    Ok(val) => {
                        if val.get("tool").is_some() && val.get("arguments").is_some() {
                            Ok(Some(val))
                        } else {
                            Err("Missing 'tool' or 'arguments' field in JSON. Please format as {\"tool\": \"...\", \"arguments\": {...}}".to_string())
                        }
                    }
                    Err(e) => Err(format!("Invalid JSON inside code block: {}", e)),
                };
            }
        }
        
        // Anti-hallucination guardrail check
        if content.contains("```bash") || content.contains("```sh") {
            return Err("You provided a bash/sh code block. You MUST use the `run_command` tool within a strict ```json block to run commands yourself. DO NOT tell the user to run commands. Fix your response and use the run_command tool correctly.".to_string());
        }
        
        Ok(None)
    }
}
