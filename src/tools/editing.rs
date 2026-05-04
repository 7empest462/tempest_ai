use serde_json::Value;
use miette::{Result, IntoDiagnostic};
use async_trait::async_trait;
use super::{AgentTool, ToolContext};
use std::fs;
use similar::{TextDiff, ChangeTag};
use schemars::JsonSchema;
use serde::Deserialize;
use ollama_rs::generation::tools::{ToolInfo, ToolFunctionInfo, ToolType};

#[derive(Deserialize, JsonSchema)]
pub struct EditFileWithDiffArgs {
    /// Path to the file to edit.
    pub path: String,
    /// The FULL new content of the file.
    pub new_content: String,
}

pub struct EditFileWithDiffTool;

#[async_trait]
impl AgentTool for EditFileWithDiffTool {
    fn name(&self) -> &'static str { "edit_file_with_diff" }
    fn description(&self) -> &'static str { "Safely edits a file by applying a new version and showing a diff preview. Best for targeted code changes." }
    fn is_modifying(&self) -> bool { true }
    fn tool_info(&self) -> ToolInfo {
        let mut settings = schemars::generate::SchemaSettings::draft07();
        settings.inline_subschemas = true;
        let payload = settings.into_generator().into_root_schema_for::<EditFileWithDiffArgs>();
        
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
        let typed_args: EditFileWithDiffArgs = serde_json::from_value(args.clone()).into_diagnostic()?;
        let path_owned = shellexpand::tilde(&typed_args.path).to_string();
        let new_content = typed_args.new_content;
        let path = std::path::PathBuf::from(&path_owned);

        // Actually write it
        let content_for_write = new_content.clone();
        tokio::task::spawn_blocking(move || {
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent).into_diagnostic()?;
            }
            std::fs::write(&path, &content_for_write).into_diagnostic()?;
            Ok::<(), miette::Report>(())
        }).await.map_err(|e| miette::miette!("Task join error: {}", e))??;

        // --- 🖋️ LIVE EDITOR SYNC ---
        if let Some(tx) = context.tx {
            let _ = tx.try_send(crate::tui::AgentEvent::EditorEdit { 
                path: path_owned.clone(), 
                content: new_content.clone() 
            });
        }

        Ok(format!("Successfully applied changes to {}.", path_owned))
    }

    async fn get_approval_preview(&self, args: &Value) -> Option<String> {
        let typed_args: EditFileWithDiffArgs = serde_json::from_value(args.clone()).ok()?;
        let path = shellexpand::tilde(&typed_args.path).to_string();
        let new_content = &typed_args.new_content;

        let old_content = fs::read_to_string(&path).unwrap_or_default();
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
        
        Some(format!("🔄 Proposed changes for {}:\n\n{}", path, diff_output))
    }
}
