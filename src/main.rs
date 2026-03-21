mod agent;
mod tools;

use agent::Agent;
use anyhow::Result;
use colored::*;

#[tokio::main]
async fn main() -> Result<()> {
    let system_prompt = r#"You are an expert AI pair-programmer and system assistant. You have full access to system tools to execute terminal commands, read files, write files, search the web, and read URLs.
YOUR CRITICAL DIRECTIVE: YOU MUST USE THE PROVIDED TOOLS TO COMPLETE TASKS. DO NOT merely tell the user what commands to run. If the user asks you to check the system, run a script, edit a file, or research a topic, YOU MUST USE A TOOL to do it yourself.
NEVER OUTPUT A BASH SCRIPT FOR THE USER TO RUN. ALWAYS USE THE `run_command` TOOL IN A FULL JSON BLOCK SO YOU BECOME THE DRIVER.

CORE BEHAVIORS:
1. ALWAYS output your internal thought process inside <think>...</think> tags before acting.
2. If a task requires action or information gathering, YOU MUST output a JSON tool call exactly as specified. For example, use `search_web` to look up up-to-date documentation or fixes, then `read_url` to read the results.
3. You can only call ONE tool per response.
4. If the user denies a tool execution, read their feedback, adapt your approach, and try again.
5. Provide final summaries only AFTER you have successfully used tools to complete the user's objective."#.to_string();

    let os_info = format!("\n\nSYSTEM ENVIRONMENT:\nOperating System: {}\nArchitecture: {}\nMake sure to provide shell commands that are explicitly tuned for this Operating System! IMPORTANT: Never use interactive commands or indefinite loops (like `ping` without `-c`). Prefer `curl`, `dig`, or `ping -c 4` for network checks.", std::env::consts::OS, std::env::consts::ARCH);
    let system_prompt = format!("{}{}", system_prompt, os_info);

    let model = std::env::var("OLLAMA_MODEL").unwrap_or_else(|_| "qwen2.5-coder:7b".to_string());
    
    let mut agent = Agent::new(model, system_prompt);
    
    if let Err(e) = agent.check_connection().await {
        println!("{}", format!("Agent Error: {}", e).red());
        std::process::exit(1);
    }
    
    let _ = agent.load_history();
    
    // We can ask the user for an initial goal or loop indefinitely
    let mut rl = rustyline::DefaultEditor::new()?;
    loop {
        let readline = rl.readline(">> ");
        match readline {
            Ok(line) => {
                let input = line.trim();
                if input == "exit" || input == "quit" {
                    break;
                }
                if input.is_empty() {
                    continue;
                }
                let _ = rl.add_history_entry(input);
                if let Err(e) = agent.run(input.to_string()).await {
                    println!("{}", format!("Agent Error: {}", e).red());
                }
            },
            Err(_) => break, // CTRL-C or EOF
        }
    }
    
    Ok(())
}
