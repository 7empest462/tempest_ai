use super::{AgentTool, ToolContext};
use async_trait::async_trait;
use miette::{IntoDiagnostic, Result};
use ollama_rs::generation::tools::{ToolFunctionInfo, ToolInfo, ToolType};
use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::Value;
use similar::{ChangeTag, TextDiff};
use std::fs;

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
    fn name(&self) -> &'static str {
        "edit_file_with_diff"
    }
    fn description(&self) -> &'static str {
        "Safely edits a file by applying a new version and showing a diff preview. Best for targeted code changes."
    }
    fn is_modifying(&self) -> bool {
        true
    }
    fn tool_info(&self) -> ToolInfo {
        let mut settings = schemars::generate::SchemaSettings::draft07();
        settings.inline_subschemas = true;
        let payload = settings
            .into_generator()
            .into_root_schema_for::<EditFileWithDiffArgs>();

        ToolInfo {
            tool_type: ToolType::Function,
            function: ToolFunctionInfo {
                name: self.name().to_string(),
                description: self.description().to_string(),
                parameters: payload,
            },
        }
    }

    async fn execute(&self, args: &Value, context: ToolContext) -> Result<String> {
        let typed_args: EditFileWithDiffArgs =
            serde_json::from_value(args.clone()).into_diagnostic()?;
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
        })
        .await
        .map_err(|e| miette::miette!("Task join error: {}", e))??;

        // --- 🖋️ LIVE EDITOR SYNC ---
        if let Some(tx) = context.tx {
            let _ = tx.try_send(crate::tui::AgentEvent::EditorEdit {
                path: path_owned.clone(),
                content: new_content.clone(),
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

        Some(format!(
            "🔄 Proposed changes for {}:\n\n{}",
            path, diff_output
        ))
    }
}

#[derive(Deserialize, JsonSchema, Clone, Debug)]
pub struct EditChunk {
    /// Optional 1-indexed starting line to restrict the search area.
    pub start_line: Option<usize>,
    /// Optional 1-indexed ending line (inclusive) to restrict the search area.
    pub end_line: Option<usize>,
    /// Exact target text to find.
    pub target: String,
    /// Replacement text.
    pub replacement: String,
}

#[derive(Deserialize, JsonSchema, Debug)]
pub struct MultiEditArgs {
    /// Path to the file to edit.
    pub path: String,
    /// List of non-contiguous edits to apply.
    pub edits: Vec<EditChunk>,
}

pub struct MultiEditTool;

#[async_trait]
impl AgentTool for MultiEditTool {
    fn name(&self) -> &'static str {
        "multi_edit"
    }
    fn description(&self) -> &'static str {
        "Applies multiple non-contiguous edits to a file. Each edit targets a specific block of text, optionally constrained to a line range."
    }
    fn is_modifying(&self) -> bool {
        true
    }
    fn tool_info(&self) -> ToolInfo {
        let mut settings = schemars::generate::SchemaSettings::draft07();
        settings.inline_subschemas = true;
        let payload = settings
            .into_generator()
            .into_root_schema_for::<MultiEditArgs>();

        ToolInfo {
            tool_type: ToolType::Function,
            function: ToolFunctionInfo {
                name: self.name().to_string(),
                description: self.description().to_string(),
                parameters: payload,
            },
        }
    }

    async fn execute(&self, args: &Value, context: ToolContext) -> Result<String> {
        let typed_args: MultiEditArgs = serde_json::from_value(args.clone()).into_diagnostic()?;
        let path_owned = shellexpand::tilde(&typed_args.path).to_string();
        let path = std::path::PathBuf::from(&path_owned);

        let old_content = fs::read_to_string(&path).into_diagnostic()?;
        let new_content = apply_multi_edit(&old_content, &typed_args.edits)
            .map_err(|e| miette::miette!("Multi-edit failed: {}", e))?;

        let content_for_write = new_content.clone();
        tokio::task::spawn_blocking(move || {
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent).into_diagnostic()?;
            }
            std::fs::write(&path, &content_for_write).into_diagnostic()?;
            Ok::<(), miette::Report>(())
        })
        .await
        .map_err(|e| miette::miette!("Task join error: {}", e))??;

        // --- 🖋️ LIVE EDITOR SYNC ---
        if let Some(tx) = context.tx {
            let _ = tx.try_send(crate::tui::AgentEvent::EditorEdit {
                path: path_owned.clone(),
                content: new_content,
            });
        }

        Ok(format!(
            "Successfully applied multi-edit to {}.",
            path_owned
        ))
    }

    async fn get_approval_preview(&self, args: &Value) -> Option<String> {
        let typed_args: MultiEditArgs = serde_json::from_value(args.clone()).ok()?;
        let path = shellexpand::tilde(&typed_args.path).to_string();
        let old_content = fs::read_to_string(&path).unwrap_or_default();
        match apply_multi_edit(&old_content, &typed_args.edits) {
            Ok(new_content) => {
                let diff = TextDiff::from_lines(old_content.as_str(), &new_content);
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
                Some(format!(
                    "🔄 Proposed multi-edit changes for {}:\n\n{}",
                    path, diff_output
                ))
            }
            Err(e) => Some(format!(
                "⚠️ Proposed multi-edit changes for {} (Error applying edits: {})",
                path, e
            )),
        }
    }
}

struct ResolvedEdit {
    chunk_index: usize,
    start: usize,
    end: usize,
    replacement: String,
}

pub fn apply_multi_edit(content: &str, edits: &[EditChunk]) -> Result<String, String> {
    if edits.is_empty() {
        return Ok(content.to_string());
    }

    let mut resolved_edits = Vec::new();

    for (idx, chunk) in edits.iter().enumerate() {
        if chunk.target.is_empty() {
            return Err(format!("Edit chunk {} has an empty target", idx + 1));
        }

        // 1. Convert line range to byte range
        let (start_byte, end_byte) =
            line_range_to_byte_range(content, chunk.start_line, chunk.end_line)?;

        // 2. Search for the target text
        let search_area = &content[start_byte..end_byte];
        let matches: Vec<_> = search_area.match_indices(&chunk.target).collect();

        if matches.is_empty() {
            return Err(format!(
                "Target not found for edit chunk {} within line range {:?}:{:?}",
                idx + 1,
                chunk.start_line,
                chunk.end_line
            ));
        }

        if matches.len() > 1 {
            return Err(format!(
                "Multiple occurrences of target found for edit chunk {} within line range {:?}:{:?}. Please narrow down using line range or unique target context.",
                idx + 1,
                chunk.start_line,
                chunk.end_line
            ));
        }

        let local_match_idx = matches[0].0;
        let start = start_byte + local_match_idx;
        let end = start + chunk.target.len();

        resolved_edits.push(ResolvedEdit {
            chunk_index: idx,
            start,
            end,
            replacement: chunk.replacement.clone(),
        });
    }

    // 3. Detect overlaps
    resolved_edits.sort_by_key(|e| e.start);

    for i in 0..resolved_edits.len().saturating_sub(1) {
        if resolved_edits[i].end > resolved_edits[i + 1].start {
            return Err(format!(
                "Overlapping edits detected between edit chunk {} and edit chunk {} (byte ranges {}..{} and {}..{})",
                resolved_edits[i].chunk_index + 1,
                resolved_edits[i + 1].chunk_index + 1,
                resolved_edits[i].start,
                resolved_edits[i].end,
                resolved_edits[i + 1].start,
                resolved_edits[i + 1].end
            ));
        }
    }

    // 4. Apply replacements back-to-front
    let mut new_content = content.to_string();
    for edit in resolved_edits.iter().rev() {
        new_content.replace_range(edit.start..edit.end, &edit.replacement);
    }

    Ok(new_content)
}

fn line_range_to_byte_range(
    content: &str,
    start_line: Option<usize>,
    end_line: Option<usize>,
) -> Result<(usize, usize), String> {
    let mut line_starts = vec![0];
    for (i, c) in content.char_indices() {
        if c == '\n' {
            line_starts.push(i + 1);
        }
    }
    let total_lines = line_starts.len();

    let start_byte = match start_line {
        Some(0) | None => 0,
        Some(l) => {
            if l > total_lines {
                return Err(format!(
                    "start_line {} is out of bounds (file has {} lines)",
                    l, total_lines
                ));
            }
            line_starts[l - 1]
        }
    };

    let end_byte = match end_line {
        None => content.len(),
        Some(l) => {
            if l > total_lines {
                return Err(format!(
                    "end_line {} is out of bounds (file has {} lines)",
                    l, total_lines
                ));
            }
            if l == total_lines {
                content.len()
            } else {
                line_starts[l]
            }
        }
    };

    if start_byte > end_byte {
        return Err(format!(
            "start_line ({:?}) resolved to byte offset {} which is greater than end_line ({:?}) resolved to byte offset {}",
            start_line, start_byte, end_line, end_byte
        ));
    }

    Ok((start_byte, end_byte))
}
