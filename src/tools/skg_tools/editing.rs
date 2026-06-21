// ==========================================
// 🖋️ SKG EDITING TOOLS — Native Skelegent Implementations
// ==========================================
// Replaces the legacy EditFileWithDiffTool.

use skg_tool::{ToolCallContext, ToolError};
use skg_tool_macro::skg_tool;

// ── edit_file_with_diff ────────────────────────────────────────────────────────

#[skg_tool(
    name = "edit_file_with_diff",
    description = "Safely edits a file by applying a new version and showing a diff preview. Best for targeted code changes."
)]
pub async fn edit_file_with_diff(
    path: String,
    new_content: String,
    ctx: &ToolCallContext,
) -> Result<serde_json::Value, ToolError> {
    let path_owned = shellexpand::tilde(&path).to_string();
    let path_buf = std::path::PathBuf::from(&path_owned);

    // Retrieve ToolContext dependency
    let tool_ctx = ctx
        .deps::<std::sync::Arc<crate::tools::ToolContext>>()
        .ok_or_else(|| ToolError::ExecutionFailed("Missing ToolContext dependency".to_string()))?;

    // Actually write the file
    let content_for_write = new_content.clone();
    tokio::task::spawn_blocking(move || -> Result<(), ToolError> {
        if let Some(parent) = path_buf.parent() {
            std::fs::create_dir_all(parent).map_err(|e| {
                ToolError::ExecutionFailed(format!("Failed to create parent directories: {}", e))
            })?;
        }
        std::fs::write(&path_buf, &content_for_write).map_err(|e| {
            ToolError::ExecutionFailed(format!(
                "Failed to write file {}: {}",
                path_buf.display(),
                e
            ))
        })?;
        Ok(())
    })
    .await
    .map_err(|e| ToolError::ExecutionFailed(format!("Task join error: {}", e)))??;

    // --- 🖋️ LIVE EDITOR SYNC ---
    if let Some(tx) = &tool_ctx.tx {
        let _ = tx.try_send(crate::tui::AgentEvent::EditorEdit {
            path: path_owned.clone(),
            content: new_content,
        });
    }

    Ok(serde_json::Value::String(format!(
        "Successfully applied changes to {}.",
        path_owned
    )))
}

// ── multi_edit ─────────────────────────────────────────────────────────────────

#[skg_tool(
    name = "multi_edit",
    description = "Applies multiple non-contiguous edits to a file. Each edit targets a specific block of text, optionally constrained to a line range."
)]
pub async fn multi_edit(
    path: String,
    edits: Vec<crate::tools::editing::EditChunk>,
    ctx: &ToolCallContext,
) -> Result<serde_json::Value, ToolError> {
    let path_owned = shellexpand::tilde(&path).to_string();
    let path_buf = std::path::PathBuf::from(&path_owned);

    // Retrieve ToolContext dependency
    let tool_ctx = ctx
        .deps::<std::sync::Arc<crate::tools::ToolContext>>()
        .ok_or_else(|| ToolError::ExecutionFailed("Missing ToolContext dependency".to_string()))?;

    // Read file, apply edits, write back
    let old_content = tokio::task::spawn_blocking({
        let path_clone = path_buf.clone();
        move || {
            std::fs::read_to_string(&path_clone).map_err(|e| {
                ToolError::ExecutionFailed(format!(
                    "Failed to read file {}: {}",
                    path_clone.display(),
                    e
                ))
            })
        }
    })
    .await
    .map_err(|e| ToolError::ExecutionFailed(format!("Task join error: {}", e)))??;

    let new_content = crate::tools::editing::apply_multi_edit(&old_content, &edits)
        .map_err(|e| ToolError::ExecutionFailed(format!("Multi-edit failed: {}", e)))?;

    let content_for_write = new_content.clone();
    let path_clone = path_buf.clone();
    tokio::task::spawn_blocking(move || -> Result<(), ToolError> {
        if let Some(parent) = path_clone.parent() {
            std::fs::create_dir_all(parent).map_err(|e| {
                ToolError::ExecutionFailed(format!("Failed to create parent directories: {}", e))
            })?;
        }
        std::fs::write(&path_clone, &content_for_write).map_err(|e| {
            ToolError::ExecutionFailed(format!(
                "Failed to write file {}: {}",
                path_clone.display(),
                e
            ))
        })?;
        Ok(())
    })
    .await
    .map_err(|e| ToolError::ExecutionFailed(format!("Task join error: {}", e)))??;

    // --- 🖋️ LIVE EDITOR SYNC ---
    if let Some(tx) = &tool_ctx.tx {
        let _ = tx.try_send(crate::tui::AgentEvent::EditorEdit {
            path: path_owned.clone(),
            content: new_content,
        });
    }

    Ok(serde_json::Value::String(format!(
        "Successfully applied multi-edit to {}.",
        path_owned
    )))
}
