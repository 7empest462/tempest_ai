pub const NAME: &str = "task_complete";
pub const DESCRIPTION: &str = "Properly wrap up a significant task, verify results, and distill knowledge into the brain";
pub const INSTRUCTIONS: &str = r#"
## Steps
1. **Final Verification**: If you modified files or ran commands, perform one last check (e.g., cargo build, python3 test.py, ls -l, or cat) to ensure the system is in a stable state.
2. **Summarize for User**: Tell the user exactly what was achieved, what files were changed, and any important notes for them to be aware of.
3. **Distill Knowledge**: 
   - Identify the core "Topic" of the work (e.g., "sqlite_cleanup", "tui_modal_fix").
   - Call the distill_knowledge tool.
   - In the summary, include: 
     - What was done.
     - Key architectural decisions.
     - Any "gotchas" or bugs encountered.
     - Specific user preferences you observed (e.g., "The user prefers axum over actix").
4. **Transition to Planning**: Call toggle_planning with mode="on" to return to a safe, research-oriented state for the next request.

## Key Principles
- Never assume a task is done without verifying it first.
- Knowledge distillation is what makes you "smarter" every session. 
- Always ask the user if there is anything else they need before fully closing the task.
"#;
