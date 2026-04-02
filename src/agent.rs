use crate::tools::{AgentTool, RunCommandTool, ReadFileTool, WriteFileTool, PatchFileTool, RunBackgroundTool, ReadProcessLogsTool, ListDirTool, SearchWebTool, ReadUrlTool, SearchDirTool, AskUserTool, ExtractAndWriteTool, SystemInfoTool, SqliteQueryTool, GitTool, WatchDirectoryTool, HttpRequestTool, ClipboardTool, NotifyTool, FindReplaceTool, TreeTool, NetworkCheckTool, DiffFilesTool, KillProcessTool, EnvVarTool, ChmodTool, AppendFileTool, DownloadFileTool, TogglePlanningTool, ListSkillsTool, SkillRecallTool, DistillKnowledgeTool, RecallBrainTool};
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
    tools: Vec<Arc<dyn AgentTool>>,
    system_prompt: String,
    recent_tool_calls: std::collections::VecDeque<String>,
    history_path: String,
    pub planning_mode: bool,
    pub task_context: String,
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
        let mut agent = Agent {
            ollama: Ollama::default(),
            model,
            history: vec![],
            tools: vec![
                Arc::new(crate::tools::StoreMemoryTool::new(memory_store.clone())),
                Arc::new(crate::tools::RecallMemoryTool::new(memory_store.clone())),
                Arc::new(crate::hardware::LinuxProcessAnalyzerTool),
                Arc::new(crate::hardware::GpuDiagnosticsTool),
                Arc::new(crate::hardware::TelemetryChartTool),
                Arc::new(crate::telemetry::AdvancedSystemOracleTool),
                Arc::new(crate::telemetry::KernelDiagnosticTool),
                Arc::new(crate::telemetry::NetworkSnifferTool),
                Arc::new(crate::tools::SystemdManagerTool),
                Arc::new(RunCommandTool),
                Arc::new(ReadFileTool),
                Arc::new(WriteFileTool),
                Arc::new(PatchFileTool),
                Arc::new(RunBackgroundTool),
                Arc::new(ReadProcessLogsTool),
                Arc::new(ListDirTool),
                Arc::new(SearchWebTool),
                Arc::new(ReadUrlTool),
                Arc::new(SearchDirTool),
                Arc::new(AskUserTool),
                Arc::new(ExtractAndWriteTool),
                Arc::new(SystemInfoTool),
                Arc::new(SqliteQueryTool),
                Arc::new(GitTool),
                Arc::new(WatchDirectoryTool),
                Arc::new(HttpRequestTool),
                Arc::new(ClipboardTool),
                Arc::new(NotifyTool),
                Arc::new(FindReplaceTool),
                Arc::new(TreeTool),
                Arc::new(NetworkCheckTool),
                Arc::new(DiffFilesTool),
                Arc::new(KillProcessTool),
                Arc::new(EnvVarTool),
                Arc::new(ChmodTool),
                Arc::new(AppendFileTool),
                Arc::new(DownloadFileTool),
                Arc::new(TogglePlanningTool),
                Arc::new(ListSkillsTool),
                Arc::new(SkillRecallTool),
                Arc::new(DistillKnowledgeTool),
                Arc::new(RecallBrainTool),
                Arc::new(crate::tools::SpawnSubAgentTool::new(memory_store.clone(), sub_agent_model.clone())),
                Arc::new(crate::tools::UpdateTaskContextTool),
                Arc::new(crate::tools::IndexFileSemanticallyTool),
                Arc::new(crate::tools::SemanticSearchTool),
            ],
            system_prompt: String::new(),
            recent_tool_calls: std::collections::VecDeque::new(),
            history_path: history_path.clone(),
            planning_mode: true,
            task_context: "Not started yet.".to_string(),
            vector_brain: Arc::new(Mutex::new(crate::vector_brain::VectorBrain::load_from_disk(
                Path::new(&history_path).parent().unwrap_or(Path::new(".")).join("brain_vectors.json")
            ).unwrap_or_else(|_| crate::vector_brain::VectorBrain::new()))),
            sub_agent_model,
            syntax_set: SyntaxSet::load_defaults_newlines(),
            theme_set: ThemeSet::load_defaults(),
            telemetry: Arc::new(Mutex::new("No telemetry data yet.".to_string())),
        };

        // Dynamically inject tool descriptions into the system prompt
        let tool_desc = agent.get_tool_descriptions();
        let mut prompt = system_prompt.replace("{tool_descriptions}", &tool_desc);
        
        // Inject Reflective Memory (Sketchpad)
        prompt.push_str("\n\n[CURRENT_MISSION_CONTEXT]: {task_context}\n(Use the `update_task_context` tool to pin important findings, sub-tasks, or progress updates here to ensure continuity across long tasks.)");

        if let Ok(topics) = memory_store.lock().unwrap().list_topics() {
            if !topics.is_empty() {
                let topics_str = topics.join(", ");
                prompt.push_str(&format!("\n\n[SYSTEM MEMORY]: You have the following topics stored in your encrypted long-term memory: [{}]. Use the `recall_memory` tool to retrieve their full contents if they seem relevant.", topics_str));
            }
        }

        // Inject available skills
        let skills = crate::skills::load_skills();
        if !skills.is_empty() {
            let skill_list: Vec<String> = skills.iter().map(|s| format!("{} ({})", s.name, s.description)).collect();
            prompt.push_str(&format!("\n\n[SKILLS]: You have {} reusable skills available: [{}]. Use `recall_skill` to load the full instructions for any skill before starting a related task.", skills.len(), skill_list.join(", ")));
        }

        // Inject brain knowledge items
        let brain_items = crate::skills::load_brain_items();
        if !brain_items.is_empty() {
            let brain_topics: Vec<String> = brain_items.iter().map(|i| i.0.clone()).collect();
            prompt.push_str(&format!("\n\n[BRAIN]: You have distilled knowledge from previous sessions on: [{}]. Use `recall_brain` to retrieve the full summary before starting a related task. After completing a significant task, use `distill_knowledge` to save what you learned.", brain_topics.join(", ")));
        }

        agent.system_prompt = prompt;
        
        // Add system message to history
        agent.history.push(ChatMessage::new(MessageRole::System, agent.system_prompt.clone()));
        
        agent
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
                println!("\n{}", "🛑 Execution limit reached (30 turns). Stopping to prevent infinite loop.".red());
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
                                // 🧠 PLANNING MODE GUARD (CLI)
                                if self.planning_mode && tool.is_modifying() {
                                    guardrail_retries += 1;
                                    if guardrail_retries > 2 {
                                        tool_result_str = "[System Guardrail] [SAFETY PIVOT]: You have hit the Planning Mode block multiple times. \
                                            STOP calling modifying tools. You must immediately provide a text-only implementation plan for the user to approve. \
                                            Once they approve, you must use `toggle_planning` to unlock these tools.".to_string();
                                    } else {
                                        tool_result_str = format!("[System Guardrail] PLANNING MODE ACTIVE: Tool '{}' modifies system state and is BLOCKED.\
                                            \n[INSTRUCTION]: You MUST present a clear implementation plan to the user for approval first.\
                                            \nDo NOT attempt to use this tool again until the user has approved your plan and you have used `toggle_planning` to enter EXECUTION mode.", tool_name);
                                    }
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
                                } // end planning mode else
                            } else {
                                tool_result_str = format!("Error: Tool '{}' not found.", tool_name);
                            }

                            self.history.push(ChatMessage::new(MessageRole::User, format!("TOOL RESULT for {}:\n{}", tool_name, tool_result_str)));
                            
                            // 🧠 SENTINEL DETECTION (CLI)
                            if tool_result_str.contains("[PLANNING_MODE_ON]") {
                                self.planning_mode = true;
                                println!("{}", "🧠 Agent entered PLANNING mode".cyan().bold());
                            } else if tool_result_str.contains("[PLANNING_MODE_OFF]") {
                                self.planning_mode = false;
                                println!("{}", "⚡ Agent entered EXECUTION mode".green().bold());
                            }
                            
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

    /// Handles tools that require direct agent state access (planning guard, sub-agents, 
    /// semantic brain, reflective memory, TUI modals). Returns `Some(result)` if handled,
    /// `None` if the tool should go through the normal execution path.
    async fn handle_agent_tool(
        &mut self,
        tool_name: &str,
        args: &serde_json::Value,
        tx: &tokio::sync::mpsc::Sender<crate::tui::AgentEvent>,
        tool_rx: &mut tokio::sync::mpsc::Receiver<crate::tui::ToolResponse>,
        guardrail_retries: &mut usize,
        is_modifying: bool,
    ) -> Option<String> {
        // 🧠 PLANNING MODE GUARD
        if self.planning_mode && is_modifying {
            *guardrail_retries += 1;
            let result = if *guardrail_retries > 2 {
                "[System Guardrail] [SAFETY PIVOT]: You have hit the Planning Mode block multiple times. \
                    STOP calling modifying tools. You must immediately provide a text-only implementation plan for the user to approve. \
                    Once they approve, you must use `toggle_planning` to unlock these tools.".to_string()
            } else {
                format!("[System Guardrail] PLANNING MODE ACTIVE: Tool '{}' modifies system state and is BLOCKED.\
                    \n[INSTRUCTION]: You MUST present a clear implementation plan to the user for approval first.\
                    \nDo NOT attempt to use this tool again until the user has approved your plan and you have used `toggle_planning` to enter EXECUTION mode.", tool_name)
            };
            let _ = tx.send(crate::tui::AgentEvent::SystemUpdate(format!("🧠 Guardrail: Blocked '{}' (Planning Mode)", tool_name))).await;
            return Some(result);
        }

        match tool_name {
            "ask_user" => {
                let question = args.get("question").and_then(|q| q.as_str()).unwrap_or("(No question provided)").to_string();
                while let Ok(_) = tool_rx.try_recv() {}
                let _ = tx.send(crate::tui::AgentEvent::RequestInput(tool_name.to_string(), question)).await;
                match tool_rx.recv().await {
                    Some(crate::tui::ToolResponse::Text(user_answer)) => Some(format!("User responded: {}", user_answer)),
                    _ => Some("User cancelled the input request.".to_string()),
                }
            }
            "spawn_sub_agent" => {
                let task = args.get("task").and_then(|t| t.as_str()).unwrap_or("(No task)").to_string();
                let model_name = args.get("model").and_then(|m| m.as_str()).unwrap_or(&self.sub_agent_model).to_string();
                let sub_agent_history = vec![
                    ChatMessage::new(MessageRole::System, "You are a specialized Sub-Agent. Perform the mission and provide a CONCISE summary.".to_string()),
                    ChatMessage::new(MessageRole::User, task.clone()),
                ];
                let request = ChatMessageRequest::new(model_name, sub_agent_history);
                match self.ollama.send_chat_messages(request).await {
                    Ok(res) => Some(format!("[MISSION REPORT]: {}\n\n[INSTRUCTION]: MISSION ACCOMPLISHED. Read the provided research above and summarize it for the user. Do NOT call this tool again for the same task.", res.message.content)),
                    Err(e) => Some(format!("Sub-Agent Error: {}", e)),
                }
            }
            "update_task_context" => {
                let context = args.get("context").and_then(|c| c.as_str()).unwrap_or("").to_string();
                self.task_context = context;
                Some("Reflective memory (Sketchpad) updated successfully.".to_string())
            }
            "semantic_search" => {
                let query = args.get("query").and_then(|q| q.as_str()).unwrap_or("").to_string();
                let top_k = args.get("top_k").and_then(|k| k.as_u64()).unwrap_or(5) as usize;
                let req = ollama_rs::generation::embeddings::request::GenerateEmbeddingsRequest::new(
                    "nomic-embed-text".to_string(),
                    query.clone().into()
                );
                match self.ollama.generate_embeddings(req).await {
                    Ok(res) => {
                        if let Some(embedding) = res.embeddings.first() {
                            let hits = self.vector_brain.lock().expect("VectorBrain Mutex Poisoned during search").search(embedding, top_k);
                            let mut report = format!("Conceptual results for: '{}'\n\n", query);
                            for (entry, sim) in hits {
                                report.push_str(&format!("[Match: {:.2}%] Source: {}\n{}\n---\n", sim * 100.0, entry.source, entry.text));
                            }
                            Some(report)
                        } else {
                            Some("Error: No embeddings generated for query.".to_string())
                        }
                    }
                    Err(e) => Some(format!("Embedding Error: {}. (Ensure 'nomic-embed-text' is pulled in Ollama.)", e)),
                }
            }
            "index_file_semantically" => {
                let path = args.get("path").and_then(|p| p.as_str()).unwrap_or("").to_string();
                let _ = tx.send(crate::tui::AgentEvent::SystemUpdate(format!("📂 Concepts: Analyzing {}...", path))).await;
                match std::fs::read_to_string(&path) {
                    Ok(content) => {
                        // 🧠 SMART CHUNKING: Split by double-newlines, then subdivide if still too large.
                        let mut chunks = Vec::new();
                        for paragraph in content.split("\n\n") {
                            if paragraph.trim().is_empty() { continue; }
                            // Subdivide large paragraphs (respecting line boundaries)
                            if paragraph.len() > 800 {
                                let mut current_chunk = String::new();
                                for line in paragraph.lines() {
                                    if current_chunk.len() + line.len() > 800 && !current_chunk.is_empty() {
                                        chunks.push(current_chunk);
                                        current_chunk = String::new();
                                    }
                                    current_chunk.push_str(line);
                                    current_chunk.push('\n');
                                }
                                if !current_chunk.is_empty() { chunks.push(current_chunk); }
                            } else {
                                chunks.push(paragraph.to_string());
                            }
                        }
                        
                        // 🧠 BATCHED EMBEDDING GENERATION
                        let mut all_embeddings = Vec::new();
                        let batch_size = 25;
                        let total_chunks = chunks.len();
                        
                        for (i, chunk_batch) in chunks.chunks(batch_size).enumerate() {
                            let start = i * batch_size;
                            let end = (start + chunk_batch.len()).min(total_chunks);
                            let _ = tx.send(crate::tui::AgentEvent::SystemUpdate(format!("📂 Concepts: Embedding chunks {}/{}...", end, total_chunks))).await;
                            
                            let req = ollama_rs::generation::embeddings::request::GenerateEmbeddingsRequest::new(
                                "nomic-embed-text".to_string(),
                                chunk_batch.to_vec().into()
                            );
                            
                            match self.ollama.generate_embeddings(req).await {
                                Ok(res) => {
                                    all_embeddings.extend(res.embeddings);
                                }
                                Err(e) => {
                                    return Some(format!("Embedding Error at batch {}: {}. (Ensure 'nomic-embed-text' is pulled in Ollama.)", i + 1, e));
                                }
                            }
                        }

                        let mut brain = self.vector_brain.lock().expect("VectorBrain Mutex Poisoned during indexing");
                        brain.entries.retain(|e| e.source != path);
                        for (i, emb) in all_embeddings.iter().enumerate() {
                            brain.add_entry(chunks[i].clone(), emb.clone(), path.clone(), std::collections::HashMap::new());
                        }
                        let brain_path = Path::new(&self.history_path).parent().unwrap_or(Path::new(".")).join("brain_vectors.json");
                        let _ = brain.save_to_disk(brain_path);
                        Some(format!("Successfully indexed {} ({} conceptual chunks). Memory updated.", path, all_embeddings.len()))
                    }
                    Err(e) => Some(format!("Read Error: {}", e)),
                }
            }
            _ => None, // Not an agent-handled tool — fall through to normal execution
        }
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

    pub async fn perform_autonomous_verification(&self) -> String {
        let mut report = String::new();
        
        // 🦀 RUST: Check for Cargo.toml
        if std::path::Path::new("Cargo.toml").exists() {
            let output = tokio::process::Command::new("cargo")
                .args(["check", "--message-format=short"])
                .output()
                .await;
            
            if let Ok(out) = output {
                if !out.status.success() {
                    let stderr = String::from_utf8_lossy(&out.stderr);
                    report.push_str(&format!("\n[AUTONOMOUS_CLIPPY] 🦀 Rust Compiler Errors detected:\n{}\n", stderr));
                }
            }
        }

        // 🐍 PYTHON: Check for .py files
        let has_py = std::fs::read_dir(".").ok()
            .map(|entries| entries.flatten().any(|e| e.path().extension().map_or(false, |ext| ext == "py")))
            .unwrap_or(false);
        if has_py {
             let output = tokio::process::Command::new("flake8")
                .arg(".")
                .output()
                .await;
            if let Ok(out) = output {
                if !out.status.success() {
                    report.push_str(&format!("\n[AUTONOMOUS_CLIPPY] 🐍 Python Linter (flake8) Errors:\n{}\n", String::from_utf8_lossy(&out.stdout)));
                }
            }
        }

        // 📦 NODE.JS: Check for package.json
        if std::path::Path::new("package.json").exists() {
            let output = tokio::process::Command::new("npm")
                .args(["run", "lint"])
                .output()
                .await;
            if let Ok(out) = output {
                if !out.status.success() {
                     report.push_str("\n[AUTONOMOUS_CLIPPY] 📦 Node.js Linter Errors detected in `npm run lint`.\n");
                }
            }
        }

        report
    }

    pub async fn run_tui_mode(&mut self, mut user_rx: tokio::sync::mpsc::Receiver<String>, tx: tokio::sync::mpsc::Sender<crate::tui::AgentEvent>, mut tool_rx: tokio::sync::mpsc::Receiver<crate::tui::ToolResponse>, stop_flag: std::sync::Arc<std::sync::atomic::AtomicBool>) -> Result<()> {
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
                
                // Update System Prompt with latest Task Context (Reflective Memory)
                if !self.history.is_empty() && self.history[0].role == MessageRole::System {
                    self.history[0] = ChatMessage::new(
                        MessageRole::System, 
                        self.system_prompt.replace("{task_context}", &self.task_context)
                    );
                }

                // 📡 INJECT SYSTEM SENTIENCE (Hardware Telemetry)
                let telemetry = {
                    let lock = self.telemetry.lock().unwrap();
                    lock.clone()
                };
                let telemetry_msg = format!("[SYSTEM_TELEMETRY] Current Host State:\n{}\n\nUse this information to pace your actions.", telemetry);
                self.history.push(ChatMessage::new(MessageRole::System, telemetry_msg));

                let ctx_size = self.calculate_optimal_ctx();
                let options = GenerationOptions::default()
                    .num_ctx(ctx_size)
                    .num_predict(4096)
                    .repeat_penalty(1.1)
                    .temperature(0.7);
                let request = ChatMessageRequest::new(self.model.clone(), self.history.clone()).options(options);

                // Remove the telemetry message IMMEDIATELY after building the request to avoid history bloat
                self.history.pop();
                
                let thinking_status = if self.calculate_optimal_ctx() <= 4096 {
                    format!("Loading Large Model ({})", self.model)
                } else {
                    "Thinking".to_string()
                };
                
                let _ = tx.send(crate::tui::AgentEvent::Thinking(Some(thinking_status))).await;

                let mut stream = match tokio::time::timeout(std::time::Duration::from_secs(300), self.ollama.send_chat_messages_stream(request)).await {
                    Ok(Ok(s)) => s,
                    Ok(Err(e)) => {
                        let _ = tx.send(crate::tui::AgentEvent::Thinking(None)).await;
                        let _ = tx.send(crate::tui::AgentEvent::SystemUpdate(format!("Ollama Error: {}", e))).await;
                        break;
                    }
                    Err(_) => {
                        let _ = tx.send(crate::tui::AgentEvent::Thinking(None)).await;
                        let _ = tx.send(crate::tui::AgentEvent::SystemUpdate("Ollama Error: Connection Timed Out (300s). Model may be too large for current RAM.".to_string())).await;
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
                        let _ = tx.send(crate::tui::AgentEvent::Thinking(Some("Generating".to_string()))).await;
                        let text = chunk.message.content;
                        full_content.push_str(&text);
                        let _ = tx.send(crate::tui::AgentEvent::StreamToken(text)).await;
                    }
                }
                let _ = tx.send(crate::tui::AgentEvent::Thinking(None)).await;

                if !full_content.trim().is_empty() {
                    self.history.push(ChatMessage::new(MessageRole::Assistant, full_content.clone()));
                    
                    // 🧠 ASSISTANT TEXT SENTINEL DETECTION (Sync Internal State)
                    if full_content.contains("[PLANNING_MODE_OFF]") {
                        self.planning_mode = false;
                        let _ = tx.send(crate::tui::AgentEvent::SystemUpdate("⚡ Agent synced to EXECUTION mode".to_string())).await;
                    } else if full_content.contains("[PLANNING_MODE_ON]") {
                        self.planning_mode = true;
                        let _ = tx.send(crate::tui::AgentEvent::SystemUpdate("🧠 Agent synced to PLANNING mode".to_string())).await;
                    }

                    let _ = self.save_history();
                }

                // Flush the stream token buffer to the permanent UI chat log BEFORE tool execution
                let _ = tx.send(crate::tui::AgentEvent::Done).await;

                let mut executed_tools = false;
                match self.extract_tool_calls(&full_content) {
                    Ok(tool_calls) if !tool_calls.is_empty() => {
                        guardrail_retries = 0;
                        
                        // 🚀 CLASSIFY TOOLS: Sequential (Modifying) vs Concurrent (Read-Only)
                        let mut sequential_batch = Vec::new();
                        let mut concurrent_batch = Vec::new();

                        for (idx, tool_req) in tool_calls.iter().enumerate() {
                            if let Some(tool_name) = tool_req.get("tool").and_then(|v| v.as_str()) {
                                let is_known = self.tools.iter().any(|t| t.name() == tool_name);
                                if !is_known {
                                    sequential_batch.push((idx, tool_req.clone()));
                                    continue;
                                }

                                let tool = self.tools.iter().find(|t| t.name() == tool_name).unwrap();
                                // Rules for sequential execution:
                                // 1. Modifying tools (write, patch, etc.)
                                // 2. Tools requiring user confirmation (Y/N)
                                // 3. Stateful agent-handled tools (ask_user, toggle_planning, etc.)
                                let is_modifying = tool.is_modifying();
                                let req_confirm = tool.requires_confirmation();
                                let is_agent_handled = ["toggle_planning", "ask_user", "distill_knowledge", "recall_brain", "update_task_context", "index_file_semantically", "spawn_sub_agent", "recall_memory", "store_memory"].contains(&tool_name);

                                if is_modifying || req_confirm || is_agent_handled {
                                    sequential_batch.push((idx, tool_req.clone()));
                                } else {
                                    concurrent_batch.push((idx, tool_req.clone()));
                                }
                            }
                        }

                        let mut all_results: Vec<(usize, String)> = Vec::new();

                        if !concurrent_batch.is_empty() {
                            let tool_names: Vec<String> = concurrent_batch.iter()
                                .map(|(_, req)| req.get("tool").and_then(|v| v.as_str()).unwrap_or("unknown").to_string())
                                .collect();
                            
                            let _ = tx.send(crate::tui::AgentEvent::ToolStart(format!("{} tools in parallel", tool_names.len()))).await;
                            
                            let mut join_set = tokio::task::JoinSet::new();
                            for (idx, tool_req) in concurrent_batch {
                                let tool_name = tool_req.get("tool").and_then(|v| v.as_str()).unwrap().to_string();
                                let args = tool_req.get("arguments").unwrap_or(&serde_json::Value::Null).clone();
                                let full_content_clone = full_content.clone();
                                
                                // Find the tool again inside the task (it must be Send + Sync)
                                // Since 'tools' is in 'self', we need a way to call execute.
                                // We'll find it by name.
                                let tool_ptr = self.tools.iter().find(|t| t.name() == tool_name).cloned().unwrap();

                                join_set.spawn(async move {
                                    match tool_ptr.execute(&args, &full_content_clone).await {
                                        Ok(res) => (idx, tool_name, res),
                                        Err(e) => (idx, tool_name, format!("Error: {}", e)),
                                    }
                                });
                            }

                            while let Some(res) = join_set.join_next().await {
                                if let Ok((idx, name, result)) = res {
                                    all_results.push((idx, format!("TOOL RESULT for {}:\n{}", name, result)));
                                }
                            }
                            let _ = tx.send(crate::tui::AgentEvent::ToolFinish).await;
                        }

                        for (idx, tool_req) in sequential_batch {
                            if let Some(tool_name) = tool_req.get("tool").and_then(|v| v.as_str()) {
                                let args = tool_req.get("arguments").unwrap_or(&serde_json::Value::Null);
                                
                                let current_call_hash = format!("{}|{}", tool_name, serde_json::to_string(args).unwrap_or_default());
                                if self.recent_tool_calls.contains(&current_call_hash) {
                                    let diag = format!("🛑 [System Guardrail] LOOP DETECTED: You just called '{}' with these exact arguments. \
                                                       DO NOT call it again. You already have the result in your history. \
                                                       Proceed to summarize the findings for the user immediately.", tool_name);
                                    let _ = tx.send(crate::tui::AgentEvent::SystemUpdate(diag.clone())).await;
                                    self.history.push(ChatMessage::new(MessageRole::System, diag));
                                    continue; 
                                }
                                self.recent_tool_calls.push_back(current_call_hash);
                                if self.recent_tool_calls.len() > 5 { self.recent_tool_calls.pop_front(); }

                                let tool_result_str;
                                let mut skip_push = false;

                                let tool_opt = self.tools.iter().find(|t| t.name() == tool_name).cloned();
                                
                                if tool_opt.is_none() {
                                    tool_result_str = format!("Error: No such tool '{}'", tool_name);
                                } else {
                                    let is_modifying = tool_opt.as_ref().unwrap().is_modifying();
                                    
                                    if let Some(result) = self.handle_agent_tool(tool_name, args, &tx, &mut tool_rx, &mut guardrail_retries, is_modifying).await {
                                        tool_result_str = result;
                                    } else {
                                        let tool = tool_opt.unwrap();
                                        let mut allowed = true;
                                        if tool.requires_confirmation() {
                                            while let Ok(_) = tool_rx.try_recv() {}
                                            let _ = tx.send(crate::tui::AgentEvent::RequestConfirmation(tool_name.to_string(), serde_json::to_string_pretty(args).unwrap_or_default())).await;
                                            allowed = match tool_rx.recv().await {
                                                Some(crate::tui::ToolResponse::Confirm(b)) => b,
                                                _ => false,
                                            };
                                        }

                                        if allowed {
                                            let _ = tx.send(crate::tui::AgentEvent::ToolStart(tool_name.to_string())).await;
                                            match tool.execute(args, &full_content).await {
                                                Ok(res) => tool_result_str = res,
                                                Err(e) => tool_result_str = format!("Error: {}", e),
                                            }
                                            let _ = tx.send(crate::tui::AgentEvent::ToolFinish).await;

                                            if tool.is_modifying() && !tool_result_str.starts_with("Error") {
                                                let auto_errors = self.perform_autonomous_verification().await;
                                                let verify_msg = format!(
                                                    "TOOL RESULT for {}:\n{}\n{}\n\n[System Verification Required] You just executed a modifying action ('{}').\
                                                    \nBEFORE continuing or declaring success, you MUST verify your work.\
                                                     Do NOT skip this step.",
                                                    tool_name, tool_result_str, auto_errors, tool_name
                                                );
                                                all_results.push((idx, verify_msg));
                                                skip_push = true;
                                            }
                                        } else {
                                            tool_result_str = "Error: User denied permission via TUI Modal.".to_string();
                                        }
                                    }
                                }

                                if !skip_push {
                                    all_results.push((idx, format!("TOOL RESULT for {}:\n{}", tool_name, tool_result_str)));
                                }
                                
                                if tool_result_str.contains("[PLANNING_MODE_ON]") {
                                    self.planning_mode = true;
                                    let _ = tx.send(crate::tui::AgentEvent::SystemUpdate("🧠 Agent entered PLANNING mode".to_string())).await;
                                } else if tool_result_str.contains("[PLANNING_MODE_OFF]") {
                                    self.planning_mode = false;
                                    let _ = tx.send(crate::tui::AgentEvent::SystemUpdate("⚡ Agent entered EXECUTION mode".to_string())).await;
                                }
                            }
                        }

                        // 3. MERGE RESULTS AND UPDATE HISTORY (Maintain Original Order)
                        all_results.sort_by_key(|(idx, _)| *idx);
                        for (_, res_text) in all_results {
                            self.history.push(ChatMessage::new(MessageRole::System, res_text));
                        }

                        let _ = self.save_history();
                        executed_tools = true;
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
                let mut guardrail_streak = 0;
                let mut to_keep = vec![true; self.history.len()];
                
                for (i, msg) in self.history.iter().enumerate().rev() {
                    if msg.content.contains("[System Guardrail]") {
                        guardrail_streak += 1;
                        if guardrail_streak > 2 {
                            to_keep[i] = false;
                        }
                    } else {
                        guardrail_streak = 0;
                    }
                }
                
                let mut new_history = Vec::with_capacity(self.history.len());
                for (msg, keep) in self.history.drain(..).zip(to_keep) {
                    if keep {
                        new_history.push(msg);
                    }
                }
                self.history = new_history;
            } // end loop
        } // end while user_rx
        Ok(())
    }
}
