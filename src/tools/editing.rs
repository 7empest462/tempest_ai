use serde_json::Value;
use anyhow::Result;
use async_trait::async_trait;
use super::{AgentTool, ToolContext};
use std::fs;
use similar::{TextDiff, ChangeTag};
use schemars::JsonSchema;
use serde::Deserialize;
use ollama_rs::generation::tools::{ToolInfo, ToolFunctionInfo, ToolType};

#[derive(Deserialize, JsonSchema)]
#[allow(dead_code)]
pub struct EditFileWithDiffArgs {
    /// Path to the file to edit.
    pub path: String,
    /// The FULL new content of the file.
    pub new_content: String,
}

#[allow(dead_code)]
pub struct EditFileWithDiffTool;

#[async_trait]
impl AgentTool for EditFileWithDiffTool {
    fn name(&self) -> &'static str { "edit_file_with_diff" }
    fn description(&self) -> &'static str { "Safely edits a file by applying a new version and showing a diff preview. Best for targeted code changes." }
    fn requires_confirmation(&self) -> bool { true }
    fn is_modifying(&self) -> bool { true }
    fn tool_info(&self) -> ToolInfo {
        let mut settings = schemars::generate::SchemaSettings::draft07();
        settings.inline_subschemas = true;
        let generator = settings.into_generator();
        let payload = generator.into_root_schema_for::<EditFileWithDiffArgs>();
        
        ToolInfo {
            tool_type: ToolType::Function,
            function: ToolFunctionInfo {
                name: self.name().to_string(),
                description: self.description().to_string(),
                parameters: payload.into(),
            }
        }
    }

    async fn execute(&self, args: &Value, context: ToolContext) -> Result<String> {
        let typed_args: EditFileWithDiffArgs = serde_json::from_value(args.clone())?;
        let path = shellexpand::tilde(&typed_args.path).to_string();
        let new_content = &typed_args.new_content;

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
