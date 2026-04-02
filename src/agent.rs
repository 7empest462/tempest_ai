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
use crate::tools::ToolContext;

pub struct Agent {
    ollama: Ollama,
    model: String,
    history: Arc<Mutex<Vec<ChatMessage>>>,
    tools: Vec<Arc<dyn crate::tools::AgentTool>>,
    system_prompt: String,
    recent_tool_calls: Arc<Mutex<std::collections::VecDeque<String>>>,
    history_path: String,
    brain_path: std::path::PathBuf,
    pub planning_mode: Arc<Mutex<bool>>,
    pub task_context: Arc<Mutex<String>>,
    pub vector_brain: Arc<Mutex<crate::vector_brain::VectorBrain>>,
    #[allow(dead_code)]
    pub sub_agent_model: String,
    #[allow(dead_code)]
    syntax_set: SyntaxSet,
    #[allow(dead_code)]
    theme_set: ThemeSet,
    pub telemetry: Arc<Mutex<String>>,
}

use std::sync::{Arc, Mutex};
use std::path::Path;
use crate::memory::MemoryStore;

impl Agent {
    pub fn new(model: String, system_prompt: String, history_path: String, memory_store: Arc<Mutex<MemoryStore>>, sub_agent_model: String) -> Self {
        let tools: Vec<Arc<dyn crate::tools::AgentTool>> = vec![
            Arc::new(crate::tools::file::ReadFileTool),
            Arc::new(crate::tools::file::WriteFileTool),
            Arc::new(crate::tools::file::ListDirTool),
            Arc::new(crate::tools::file::SearchFilesTool),
            Arc::new(crate::tools::file::AppendFileTool),
            Arc::new(crate::tools::file::PatchFileTool),
            Arc::new(crate::tools::file::FindReplaceTool),
            Arc::new(crate::tools::execution::RunCommandTool),
            Arc::new(crate::tools::execution::RunTestsTool),
            Arc::new(crate::tools::execution::BuildProjectTool),
            Arc::new(crate::tools::git::GitStatusTool),
            Arc::new(crate::tools::git::GitDiffTool),
            Arc::new(crate::tools::git::GitCommitTool),
            Arc::new(crate::tools::search::SemanticSearchTool),
            Arc::new(crate::tools::search::GrepSearchTool),
            Arc::new(crate::tools::editing::EditFileWithDiffTool),
            Arc::new(crate::tools::memory::StoreMemoryTool::new(memory_store.clone())),
            Arc::new(crate::tools::memory::RecallMemoryTool::new(memory_store.clone())),
            Arc::new(crate::tools::agent_ops::AskUserTool),
            Arc::new(crate::tools::agent_ops::SpawnSubAgentTool),
            Arc::new(crate::tools::agent_ops::TogglePlanningTool),
            Arc::new(crate::tools::agent_ops::UpdateTaskContextTool),
            Arc::new(crate::tools::agent_ops::ExtractAndWriteTool),
            Arc::new(crate::tools::system::SystemInfoTool),
            Arc::new(crate::tools::system::LinuxProcessAnalyzerTool),
            Arc::new(crate::tools::system::GpuDiagnosticsTool),
            Arc::new(crate::tools::system::TelemetryChartTool),
            Arc::new(crate::tools::system::AdvancedSystemOracleTool),
            Arc::new(crate::tools::system::KernelDiagnosticTool),
            Arc::new(crate::tools::system::NetworkSnifferTool),
            Arc::new(crate::tools::system::SystemdManagerTool),
            // RESTORED WEB TOOLS
            Arc::new(crate::tools::web::SearchWebTool),
            Arc::new(crate::tools::web::ReadUrlTool),
            Arc::new(crate::tools::web::HttpRequestTool),
            Arc::new(crate::tools::web::DownloadFileTool),
            // RESTORED PROCESS TOOLS
            Arc::new(crate::tools::process::RunBackgroundTool),
            Arc::new(crate::tools::process::ReadProcessLogsTool),
            Arc::new(crate::tools::process::KillProcessTool),
            Arc::new(crate::tools::process::WatchDirectoryTool),
            // RESTORED UTILITY TOOLS
            Arc::new(crate::tools::utilities::ClipboardTool),
            Arc::new(crate::tools::utilities::NotifyTool),
            Arc::new(crate::tools::utilities::EnvVarTool),
            Arc::new(crate::tools::utilities::ChmodTool),
            // RESTORED KNOWLEDGE TOOLS
            Arc::new(crate::tools::knowledge::DistillKnowledgeTool),
            Arc::new(crate::tools::knowledge::RecallBrainTool),
            Arc::new(crate::tools::knowledge::ListSkillsTool),
            Arc::new(crate::tools::knowledge::SkillRecallTool),
            // RESTORED DATABASE & NETWORK TOOLS
            Arc::new(crate::tools::database::SqliteQueryTool),
            Arc::new(crate::tools::network::NetworkCheckTool),
            // RESTORED ATLAS TOOLS
            Arc::new(crate::tools::atlas::TreeTool),
            Arc::new(crate::tools::atlas::ProjectAtlasTool),
            // FINAL COMPLEMENTARY TOOLS
            Arc::new(crate::tools::git::GitActionTool),
            Arc::new(crate::tools::file::DiffFilesTool),
            Arc::new(crate::tools::search::IndexFileSemanticallyTool),
        ];

        let history_path_obj = Path::new(&history_path);
        let brain_path = history_path_obj.parent().unwrap_or(Path::new(".")).join("brain_vectors.json");

        let agent = Agent {
            ollama: Ollama::default(),
            model: model.clone(),
            history: Arc::new(Mutex::new(vec![])),
            tools,
            system_prompt: system_prompt.clone(),
            recent_tool_calls: Arc::new(Mutex::new(std::collections::VecDeque::new())),
            history_path: history_path.clone(),
            planning_mode: Arc::new(Mutex::new(true)),
            task_context: Arc::new(Mutex::new("Not started yet.".to_string())),
            vector_brain: Arc::new(Mutex::new(crate::vector_brain::VectorBrain::load_from_disk(&brain_path)
                .unwrap_or_else(|_| crate::vector_brain::VectorBrain::new()))),
            sub_agent_model,
            syntax_set: SyntaxSet::load_defaults_newlines(),
            theme_set: ThemeSet::load_defaults(),
            telemetry: Arc::new(Mutex::new(String::new())),
            brain_path,
        };

        // Standard prompt setup (TUI will override this if needed)
        let _ = agent.save_history();
        agent
    }

    pub async fn initialize_atlas(&self) -> Result<()> {
        if let Some(_t) = self.tools.iter().find(|t| t.name() == "project_atlas") {
            let _tx_clone = self.create_event_sender_noop(); // Need a helper for no-op sender if initializing atlas without TUI
            // For now, let's just skip atlas init if we can't easily get the context here.
            // Or use a dummy context.
        }
        Ok(())
    }

    fn create_event_sender_noop(&self) -> tokio::sync::mpsc::Sender<crate::tui::AgentEvent> {
        let (tx, _) = tokio::sync::mpsc::channel(1);
        tx
    }
    
    pub fn create_tool_context(&self, tx: tokio::sync::mpsc::Sender<crate::tui::AgentEvent>, tool_rx: Arc<tokio::sync::Mutex<tokio::sync::mpsc::Receiver<crate::tui::ToolResponse>>>) -> ToolContext {
        ToolContext {
            ollama: self.ollama.clone(),
            model: self.model.clone(),
            sub_agent_model: self.sub_agent_model.clone(),
            history: self.history.clone(),
            planning_mode: self.planning_mode.clone(),
            task_context: self.task_context.clone(),
            vector_brain: self.vector_brain.clone(),
            telemetry: self.telemetry.clone(),
            tx,
            tool_rx,
            recent_tool_calls: self.recent_tool_calls.clone(),
            brain_path: self.brain_path.clone(),
        }
    }

    fn calculate_optimal_ctx(&self) -> u64 {
        let model = self.model.to_lowercase();
        // 16GB RAM constraints (approx 17.1B bytes)
        // 14B+ models: 2048-4096 ctx
        // 7B-9B models: 8192 ctx
        // <4B models: 16384 ctx
        if model.contains("20b") || model.contains("27b") || model.contains("30b") || model.contains("deepseek-r1:32b") {
            2048
        } else if model.contains("14b") || model.contains("13b") || model.contains("16b") || model.contains("12b") {
            4096
        } else if model.contains("7b") || model.contains("8b") || model.contains("9b") {
            8192
        } else {
             16384 // Small models (phi, gemma 2b, qwen 3b)
        }
    }

    pub fn get_tool_descriptions(&self) -> String {
        let mut desc = String::new();
        for tool in &self.tools {
            let name = tool.name();
            let description = tool.description();
            let params = tool.parameters();
            
            desc.push_str(&format!("- {}: {}. JSON Schema: {}\n", name, description, params));
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

    pub fn load_history(&self) -> Result<()> {
        let history_path = std::path::Path::new(&self.history_path);
        if history_path.exists() {
            let data = std::fs::read_to_string(history_path)?;
            if let Ok(history) = serde_json::from_str::<Vec<ChatMessage>>(&data) {
                let mut h_lock = self.history.lock().unwrap();
                for msg in history {
                    if msg.role != MessageRole::System {
                        h_lock.push(msg);
                    }
                }
                if !h_lock.is_empty() {
                    println!("{} Loaded {} previous messages from history.", "📚".blue(), h_lock.len());
                }
            }
        }
        Ok(())
    }

    pub fn save_history(&self) -> Result<()> {
        let history_path = std::path::Path::new(&self.history_path);
        let h_lock = self.history.lock().unwrap();
        let data = serde_json::to_string_pretty(&*h_lock)?;
        std::fs::write(history_path, data)?;
        Ok(())
    }

    pub fn clear_history(&self) {
        self.history.lock().unwrap().clear();
        let _ = std::fs::remove_file(&self.history_path);
    }

    pub async fn run(&self, initial_user_prompt: String) -> Result<()> {
        println!("{}", "=".repeat(60).blue());
        let build_time = env!("BUILD_TIME");
        println!("{} {} (Build: {})", "🚀".green(), "Tempest AI Agent Initialized".bold(), build_time.cyan());
        println!("{} {}", "Model:".blue(), self.model);
        println!("{}", "=".repeat(60).blue());

        // Initialize history with system prompt
        let full_system_prompt = self.system_prompt.clone();

        {
            let mut h_lock = self.history.lock().unwrap();
            if let Some(pos) = h_lock.iter().position(|m| m.role == MessageRole::System) {
                if h_lock[pos].content != full_system_prompt {
                    h_lock[pos] = ChatMessage::new(MessageRole::System, full_system_prompt);
                }
            } else {
                h_lock.insert(0, ChatMessage::new(MessageRole::System, full_system_prompt));
            }
            h_lock.push(ChatMessage::new(MessageRole::User, initial_user_prompt));
        }
        let _ = self.save_history(); // Guarantee file creation immediately

        let max_iterations = 30;
        let mut iteration_count = 0;

        loop {
            iteration_count += 1;
            if iteration_count > max_iterations {
                println!("\n{}", "🛑 Execution limit reached (30 turns). Stopping to prevent infinite loop.".red());
                break;
            }
            // 🧠 Autonomously clear empty/useless messages from history before sending
            {
                let mut h_lock = self.history.lock().unwrap();
                h_lock.retain(|m| !m.content.trim().is_empty() || !m.tool_calls.is_empty());
            }

            // 🧹 Auto-strip old [System Guardrail] messages to prevent history poisoning.
            // Keep only guardrail messages from the last 4 history entries.
            {
                let mut h_lock = self.history.lock().unwrap();
                let guardrail_cutoff = h_lock.len().saturating_sub(4);
                
                // Hydrate system prompt with latest tool descriptions and task context
                if !h_lock.is_empty() && h_lock[0].role == MessageRole::System {
                    let task_ctx = self.task_context.lock().unwrap();
                    let tool_desc = self.get_tool_descriptions();
                    h_lock[0] = ChatMessage::new(
                        MessageRole::System, 
                        self.system_prompt
                            .replace("{tool_descriptions}", &tool_desc)
                            .replace("{task_context}", &*task_ctx)
                    );
                }

                for i in 0..guardrail_cutoff {
                    if h_lock[i].content.contains("[System Guardrail]") {
                        h_lock[i] = ChatMessage::new(MessageRole::User, "[trimmed]".to_string());
                    }
                }
                h_lock.retain(|m| m.content != "[trimmed]");
            }

            // 🧠 Compress old history when it gets too long (instead of hard-dropping)
            let _ = self.auto_summarize_memory(false).await;

            // Build the request with moderate options (8k context)
            let options = GenerationOptions::default()
                .num_ctx(8192)
                .num_predict(4096);

            let history_snapshot = {
                let h_lock = self.history.lock().unwrap();
                h_lock.clone()
            };

            let request = ChatMessageRequest::new(
                self.model.clone(),
                history_snapshot,
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
                self.history.lock().unwrap().push(message);
                let _ = self.save_history();
            }
            let content = full_content;

            // Look for JSON blocks to execute tools (supports multiple per response)
            let mut executed_tools = false;

            match self.extract_tool_calls(&content) {
                Ok(tool_calls) if !tool_calls.is_empty() => {
                    for tool_req in &tool_calls {
                        if let Some(tool_name) = tool_req.get("tool").and_then(|v| v.as_str()) {
                            let args = tool_req.get("arguments").unwrap_or(&Value::Null);

                            let current_call_hash = format!("{}|{}", tool_name, serde_json::to_string(args).unwrap_or_default());
                            {
                                let mut calls_lock = self.recent_tool_calls.lock().unwrap();
                                if calls_lock.contains(&current_call_hash) {
                                    println!("\n{}", "❌ Loop Detected. Intercepting duplicate tool sequence...".red());
                                    let guard_msg = "[System Guardrail] LOOP DETECTED. You just executed the exact same tool and arguments as a recent failed tool call. Pivot to a new strategy.".to_string();
                                    self.history.lock().unwrap().push(ChatMessage::new(MessageRole::User, format!("TOOL RESULT for {}:\n{}", tool_name, guard_msg)));
                                    let _ = self.save_history();
                                    calls_lock.clear();
                                    continue;
                                }
                                calls_lock.push_back(current_call_hash);
                                if calls_lock.len() > 5 { calls_lock.pop_front(); }
                            }

                            let tool_result_str;
                            if let Some(tool) = self.tools.iter().find(|t| t.name() == tool_name) {
                                // 🧠 PLANNING MODE GUARD (CLI)
                                let is_planning = *self.planning_mode.lock().unwrap();
                                if is_planning && tool.is_modifying() {
                                    tool_result_str = format!("[System Guardrail] PLANNING MODE ACTIVE: Tool '{}' modifies system state and is BLOCKED.\
                                        \n[INSTRUCTION]: You MUST present a clear implementation plan to the user for approval first.\
                                        \nDo NOT attempt to use this tool again until the user has approved your plan and you have used `toggle_planning` to enter EXECUTION mode.", tool_name);
                                    println!("\n{} {}", "🧠 Guardrail:".yellow().bold(), format!("Blocked '{}' (Planning Mode)", tool_name).yellow());
                                } else {
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

                                        // CLI Context creation (simpler, no TUI tx/rx for now)
                                        let (tx, _) = tokio::sync::mpsc::channel(1);
                                        let (_, tool_rx_internal) = tokio::sync::mpsc::channel(1);
                                        let tool_rx = Arc::new(tokio::sync::Mutex::new(tool_rx_internal));
                                        let context = self.create_tool_context(tx, tool_rx);

                                        match tool.execute(args, context).await {
                                            Ok(res) => {
                                                tool_spinner.finish_and_clear();
                                                println!("{} {} {}", "✔".green().bold(), "Tool execution successful:".green(), tool_name.cyan());
                                                tool_result_str = res;
                                            }
                                            Err(e) => {
                                                tool_spinner.finish_and_clear();
                                                let err_str = format!("{}", e);
                                                println!("{} {} {}", "❌".red().bold(), "Tool execution failed:".red(), err_str);
                                                tool_result_str = format!("Error: {}", e);
                                            }
                                        }
                                    }
                                }
                                self.history.lock().unwrap().push(ChatMessage::new(MessageRole::User, format!("TOOL RESULT for {}:\n{}", tool_name, tool_result_str)));
                                let _ = self.save_history();
                                executed_tools = true;
                            } else {
                                let err_msg = format!("Error: Tool '{}' not found.", tool_name);
                                self.history.lock().unwrap().push(ChatMessage::new(MessageRole::User, err_msg));
                                executed_tools = true;
                            }
                        }
                    }
                }
                Err(guardrail_msg) => {
                    println!("\n{} {}", "⚠️  Agent syntax error:".yellow(), guardrail_msg);
                    self.history.lock().unwrap().push(ChatMessage::new(MessageRole::User, format!("[System Guardrail] {}", guardrail_msg)));
                    let _ = self.save_history();
                    continue;
                }
                Ok(_) => {}
            }

            if !executed_tools {
                println!("\n{}", "✅ Agent finished task.".green().bold());
                break;
            }
        }

        Ok(())
    }

    async fn auto_summarize_memory(&self, silent: bool) -> Result<()> {
        let max_history = 40;
        let num_to_summarize = 20;
        
        let history_len = {
            let h_lock = self.history.lock().unwrap();
            h_lock.len()
        };
        
        let chat_messages = history_len.saturating_sub(1);
        
        if chat_messages > max_history {
            if !silent {
                println!("\n{} {}", "🧠 Compressing old memories to preserve context window...".cyan().bold(), "");
            }
            
            let mut summary_text = String::new();
            {
                let h_lock = self.history.lock().unwrap();
                for msg in h_lock.iter().skip(1).take(num_to_summarize) {
                    let role_str = match msg.role {
                        MessageRole::User => "User",
                        MessageRole::Assistant => "Agent",
                        MessageRole::System => "Archive",
                        MessageRole::Tool => "Tool",
                    };
                    summary_text.push_str(&format!("{}: {}\n", role_str, msg.content));
                }
            }
            
            let summary_prompt = format!(
                "Summarize the conversation concisely. Focus on core objectives and current progress. Do not output anything other than the summary itself.\n\n{}", 
                summary_text
            );
            
            let request = ChatMessageRequest::new(
                self.model.clone(),
                vec![ChatMessage::new(MessageRole::User, summary_prompt)],
            );
            
            if let Ok(response) = self.ollama.send_chat_messages(request).await {
                let summary = response.message.content;
                
                let mut h_lock = self.history.lock().unwrap();
                let mut new_history = vec![h_lock[0].clone()];
                new_history.push(ChatMessage::new(MessageRole::System, format!("[Archived Memory]: {}", summary)));
                new_history.extend_from_slice(&h_lock[(1 + num_to_summarize)..]);
                
                *h_lock = new_history;
                drop(h_lock);
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
        
        // 🚀 ROBUST TOOL EXTRACTION: Use a case-insensitive regex for finding ```json blocks.
        // This handles missing newlines, different casing (, e.g., ```JSON), and extra spaces.
        // The regex captures any text between triple backticks starting with 'json'.
        let block_regex = regex::RegexBuilder::new(r"```\s*json\s*([\s\S]*?)\s*```")
            .case_insensitive(true)
            .build()
            .unwrap();

        for caps in block_regex.captures_iter(content) {
            if let Some(m) = caps.get(1) {
                let block = m.as_str().trim();
                
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
                                continue;
                            }
                        }
                        
                        // 🚨 HARD BREAK: If we actually SAW a JSON block but couldn't parse or rescue it, we MUST error. 
                        // Returning an empty call list at this point would let the model continue thinking it succeeded.
                        return Err(format!("[System Guardrail] CRITICAL: Invalid JSON in code block. I saw a ```json block but was unable to parse it correctly. If you are trying to save code, use: ```json\n{{ \"tool\": \"extract_and_write\", \"arguments\": {{ \"path\": \"filename\" }} }}\n```"));
                    }
                }
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


    pub async fn run_tui_mode(&self, mut user_rx: tokio::sync::mpsc::Receiver<String>, tx: tokio::sync::mpsc::Sender<crate::tui::AgentEvent>, tool_rx: tokio::sync::mpsc::Receiver<crate::tui::ToolResponse>, stop_flag: std::sync::Arc<std::sync::atomic::AtomicBool>) -> Result<()> {
        let full_system_prompt = self.system_prompt.clone();
        let tool_rx_mutex = Arc::new(tokio::sync::Mutex::new(tool_rx));
        
        while let Some(msg) = user_rx.recv().await {
            stop_flag.store(false, std::sync::atomic::Ordering::SeqCst);
            {
                let mut h_lock = self.history.lock().unwrap();
                if let Some(pos) = h_lock.iter().position(|m| m.role == MessageRole::System) {
                    if h_lock[pos].content != full_system_prompt {
                        h_lock[pos] = ChatMessage::new(MessageRole::System, full_system_prompt.clone());
                    }
                } else {
                    h_lock.insert(0, ChatMessage::new(MessageRole::System, full_system_prompt.clone()));
                }
                h_lock.push(ChatMessage::new(MessageRole::User, msg));
            }
            let _ = self.save_history();
            
            let mut iteration_count = 0;
            const MAX_ITERATIONS: usize = 30;

            loop {
                iteration_count += 1;
                if iteration_count > MAX_ITERATIONS { break; }

                {
                    let mut h_lock = self.history.lock().unwrap();
                    h_lock.retain(|m| !m.content.trim().is_empty() || !m.tool_calls.is_empty());
                    let guardrail_cutoff = h_lock.len().saturating_sub(4);
                    for i in 0..guardrail_cutoff {
                        if h_lock[i].content.contains("[System Guardrail]") {
                            h_lock[i] = ChatMessage::new(MessageRole::User, "[trimmed]".to_string());
                        }
                    }
                    h_lock.retain(|m| m.content != "[trimmed]");
                }
                
                let _ = self.auto_summarize_memory(true).await;
                
                // Update System Prompt with latest Task Context (Reflective Memory)
                {
                    let mut h_lock = self.history.lock().unwrap();
                    let t_lock = self.task_context.lock().unwrap();
                    if !h_lock.is_empty() && h_lock[0].role == MessageRole::System {
                        h_lock[0] = ChatMessage::new(
                            MessageRole::System, 
                            self.system_prompt.replace("{task_context}", &*t_lock)
                        );
                    }
                }
                
                let history_snapshot = {
                    let h_lock = self.history.lock().unwrap();
                    h_lock.clone()
                };

                let options = GenerationOptions::default()
                    .num_ctx(self.calculate_optimal_ctx())
                    .num_predict(4096);

                let request = ChatMessageRequest::new(
                    self.model.clone(),
                    history_snapshot,
                ).options(options);

                let stream_res = self.ollama.send_chat_messages_stream(request).await;
                let mut stream = match stream_res {
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
                    self.history.lock().unwrap().push(ChatMessage::new(MessageRole::Assistant, full_content.clone()));
                    let _ = self.save_history();
                }
                let _ = tx.send(crate::tui::AgentEvent::Done).await;

                // 🚀 TOOL EXECUTION 
                match self.extract_tool_calls(&full_content) {
                    Ok(tool_calls) if !tool_calls.is_empty() => {
                        let context = self.create_tool_context(tx.clone(), tool_rx_mutex.clone());
                        
                        for tool_req in tool_calls {
                            if let Some(tool_name) = tool_req.get("tool").and_then(|v| v.as_str()) {
                                if let Some(tool) = self.tools.iter().find(|t| t.name() == tool_name) {
                                    let args = tool_req.get("arguments").unwrap_or(&serde_json::Value::Null);
                                    
                                    // 🧠 PLANNING MODE GUARD
                                    let is_planning = *self.planning_mode.lock().unwrap();
                                    if is_planning && tool.is_modifying() {
                                        let guard_msg = format!("[System Guardrail] PLANNING MODE ACTIVE: Tool '{}' modifies system state and is BLOCKED. \
                                            Present your implementation plan to the user for approval first. \
                                            Once approved, use `toggle_planning` to unlock editing.", tool_name);
                                        self.history.lock().unwrap().push(ChatMessage::new(MessageRole::System, guard_msg));
                                        continue;
                                    }

                                    let mut allowed = true;
                                    if tool.requires_confirmation() {
                                        let _ = tx.send(crate::tui::AgentEvent::RequestConfirmation(tool_name.to_string(), serde_json::to_string_pretty(args).unwrap_or_else(|_| "{}".to_string()))).await;
                                        let mut rx_lock = tool_rx_mutex.lock().await;
                                        allowed = match rx_lock.recv().await {
                                            Some(crate::tui::ToolResponse::Confirm(b)) => b,
                                            _ => false,
                                        };
                                    }

                                    if allowed {
                                        let _ = tx.send(crate::tui::AgentEvent::ToolStart(tool_name.to_string())).await;
                                        match tool.execute(args, context.clone()).await {
                                            Ok(res) => {
                                                self.history.lock().unwrap().push(ChatMessage::new(MessageRole::User, format!("TOOL RESULT for {}:\n{}", tool_name, res)));
                                            }
                                            Err(e) => {
                                                self.history.lock().unwrap().push(ChatMessage::new(MessageRole::User, format!("TOOL ERROR for {}:\n{}", tool_name, e)));
                                            }
                                        }
                                        let _ = tx.send(crate::tui::AgentEvent::ToolFinish).await;
                                    } else {
                                        self.history.lock().unwrap().push(ChatMessage::new(MessageRole::User, format!("Error: User denied permission for {}.", tool_name)));
                                    }
                                } else {
                                    self.history.lock().unwrap().push(ChatMessage::new(MessageRole::User, format!("Error: No such tool '{}'", tool_name)));
                                }
                            }
                        }
                    }
                    _ => break, // No more tools to call
                }
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::memory::MemoryStore;

    fn setup_agent() -> Agent {
        let memory_store = Arc::new(Mutex::new(MemoryStore::new("test-passphrase".to_string()).unwrap()));
        Agent::new(
            "test-model".to_string(),
            "test-system-prompt".to_string(),
            "/tmp/test-history.json".to_string(),
            memory_store,
            "test-sub-model".to_string()
        )
    }

    #[test]
    fn test_extract_standard_json() {
        let agent = setup_agent();
        let content = r#"Here is the file:
```json
{
  "tool": "ls",
  "arguments": { "path": "." }
}
```"#;
        let calls = agent.extract_tool_calls(content).unwrap();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0]["tool"], "ls");
        assert_eq!(calls[0]["arguments"]["path"], ".");
    }

    #[test]
    fn test_extract_case_insensitive() {
        let agent = setup_agent();
        let content = r#"```JSON
{ "tool": "whoami" }
```"#;
        let calls = agent.extract_tool_calls(content).unwrap();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0]["tool"], "whoami");
    }

    #[test]
    fn test_extract_missing_arguments_key() {
        let agent = setup_agent();
        let content = r#"```json
{
  "tool": "read_file",
  "path": "src/main.rs"
}
```"#;
        let calls = agent.extract_tool_calls(content).unwrap();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0]["tool"], "read_file");
        assert_eq!(calls[0]["arguments"]["path"], "src/main.rs");
    }

    #[test]
    fn test_tool_sanity_suite() {
        let agent = setup_agent();
        let tool_names = agent.get_tool_names();
        
        println!("🔍 Running sanity test on {} tools...", tool_names.len());
        assert!(!tool_names.is_empty(), "Agent should have at least some tools registered!");

        let mut seen_names = std::collections::HashSet::new();

        for tool in &agent.tools {
            let name = tool.name();
            let desc = tool.description();
            let params = tool.parameters();

            assert!(!name.is_empty(), "Tool should have a name!");
            assert!(!desc.is_empty(), "Tool '{}' should have a description!", name);
            assert!(params.is_object(), "Tool '{}' parameters must be a JSON object!", name);
            
            assert!(seen_names.insert(name.to_string()), "Tool name '{}' is duplicated!", name);
            
            // Basic parameter schema checks
            if let Some(p_type) = params.get("type") {
                 assert_eq!(p_type, "object", "Tool '{}' parameters type should be 'object'", name);
            }
        }
        println!("✅ All {} tools passed sanity checks.", tool_names.len());
    }
}
