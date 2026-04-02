use serde_json::{json, Value};
use anyhow::Result;
use async_trait::async_trait;
use super::{AgentTool, ToolContext};
use std::fs;
use similar::{TextDiff, ChangeTag};

pub struct EditFileWithDiffTool;

#[async_trait]
impl AgentTool for EditFileWithDiffTool {
    fn name(&self) -> &'static str { "edit_file_with_diff" }
    fn description(&self) -> &'static str { "Safely edits a file by applying a new version and showing a diff preview. Best for targeted code changes." }
    fn requires_confirmation(&self) -> bool { true }
    fn is_modifying(&self) -> bool { true }
    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": { "type": "string", "description": "Path to the file to edit." },
                "new_content": { "type": "string", "description": "The FULL new content of the file." }
            },
            "required": ["path", "new_content"]
        })
    }

    async fn execute(&self, args: &Value, context: ToolContext) -> Result<String> {
        let path_str = args.get("path").and_then(|p| p.as_str()).ok_or_else(|| anyhow::anyhow!("Missing path"))?;
        let new_content = args.get("new_content").and_then(|c| c.as_str()).ok_or_else(|| anyhow::anyhow!("Missing content"))?;
        let path = shellexpand::tilde(path_str).to_string();

        let old_content = fs::read_to_string(&path).unwrap_or_default();
        
        // Generate Diff for the console/TUI
        let diff = TextDiff::from_lines(old_content.as_str(), new_content);
        let mut diff_output = String::new();
        
        for change in diff.iter_all_changes() {
            let sign = match change.tag() {
                ChangeTag::Delete => "- ",
                ChangeTag::Insert => "+ ",
                ChangeTag::Equal => "  ",
            };
            diff_output.push_str(sign);
            diff_output.push_str(change.value());
        }

        // Send diff to TUI before actually writing
        let _ = context.tx.send(crate::tui::AgentEvent::SystemUpdate(format!("🔄 Proposed changes for {}:\n\n{}", path, diff_output))).await;
        
        // Actually write it
        fs::write(&path, new_content)?;
        Ok(format!("Successfully applied changes to {}.", path))
    }
}
