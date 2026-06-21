// ==========================================
// 📍 SKG ATLAS TOOLS — Native Skelegent Implementations
// ==========================================

use skg_tool::{ToolCallContext, ToolError};
use skg_tool_macro::skg_tool;
use std::fs;
use std::path::Path;

const SKIP_DIRS: &[&str] = &[
    "node_modules",
    "target",
    ".git",
    "__pycache__",
    ".next",
    "dist",
    "build",
    ".DS_Store",
    "venv",
    ".venv",
];

struct TreeWalkState<'a> {
    max_depth: usize,
    show_sizes: bool,
    output: &'a mut String,
    file_count: &'a mut usize,
    dir_count: &'a mut usize,
}

fn walk_project_tree(dir: &Path, prefix: &str, depth: usize, state: &mut TreeWalkState) {
    if depth > state.max_depth {
        return;
    }
    let mut entries: Vec<_> = match fs::read_dir(dir) {
        Ok(rd) => rd.filter_map(|e| e.ok()).collect(),
        Err(_) => return,
    };
    entries.sort_by_key(|e| e.file_name());

    let total = entries.len();
    for (i, entry) in entries.iter().enumerate() {
        let name = entry.file_name().to_string_lossy().to_string();
        if SKIP_DIRS.contains(&name.as_str()) || name.starts_with('.') {
            continue;
        }

        let is_last = i == total - 1;
        let connector = if is_last { "└── " } else { "├── " };
        let child_prefix = if is_last { "    " } else { "│   " };

        let path = entry.path();
        if path.is_dir() {
            *state.dir_count += 1;
            state
                .output
                .push_str(&format!("{}{}{}/\n", prefix, connector, name));
            walk_project_tree(
                &path,
                &format!("{}{}", prefix, child_prefix),
                depth + 1,
                state,
            );
        } else {
            *state.file_count += 1;
            if state.show_sizes {
                let size = fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
                let size_str = if size > 1_000_000 {
                    format!("{:.1}MB", size as f64 / 1_000_000.0)
                } else if size > 1_000 {
                    format!("{:.1}KB", size as f64 / 1_000.0)
                } else {
                    format!("{}B", size)
                };
                state
                    .output
                    .push_str(&format!("{}{}{} ({})\n", prefix, connector, name, size_str));
            } else {
                state
                    .output
                    .push_str(&format!("{}{}{}\n", prefix, connector, name));
            }
        }
    }
}

// ── tree ───────────────────────────────────────────────────────────────────────

#[skg_tool(
    name = "tree",
    description = "Shows a recursive directory tree view. Excludes hidden directories and common noise (node_modules, target, .git) by default."
)]
pub async fn tree(
    path: String,
    max_depth: Option<u64>,
    _ctx: &ToolCallContext,
) -> Result<serde_json::Value, ToolError> {
    let path_owned = shellexpand::tilde(&path).to_string();
    let max_depth_val = max_depth.unwrap_or(4) as usize;

    let mut output = String::new();
    let mut file_count = 0usize;
    let mut dir_count = 0usize;

    let root = Path::new(&path_owned);
    output.push_str(&format!("{}/\n", path_owned));
    let mut state = TreeWalkState {
        max_depth: max_depth_val,
        show_sizes: true,
        output: &mut output,
        file_count: &mut file_count,
        dir_count: &mut dir_count,
    };
    walk_project_tree(root, "", 0, &mut state);
    output.push_str(&format!(
        "\n{} directories, {} files",
        dir_count, file_count
    ));

    Ok(serde_json::Value::String(output))
}

// ── project_atlas ──────────────────────────────────────────────────────────────

#[skg_tool(
    name = "project_atlas",
    description = "📍 SYSTEM MAP: Generates/reads the local '.tempest_atlas.md' file. This is a NATIVE tool; do not attempt to call external 'atlas' binaries or Python scripts (e.g., /usr/bin/atlas_cli.py)."
)]
pub async fn project_atlas(
    action: String,
    ctx: &ToolCallContext,
) -> Result<serde_json::Value, ToolError> {
    let tool_ctx = ctx
        .deps::<std::sync::Arc<crate::tools::ToolContext>>()
        .ok_or_else(|| ToolError::ExecutionFailed("Missing ToolContext dependency".to_string()))?;

    let atlas_path = ".tempest_atlas.md";

    match action.as_str() {
        "read" => {
            if let Ok(content) = fs::read_to_string(atlas_path) {
                Ok(serde_json::Value::String(format!(
                    "📍 CURRENT PROJECT ATLAS:\n\n{}",
                    content
                )))
            } else {
                Ok(serde_json::Value::String(
                    "❌ Atlas not found. Use 'map' to generate it first.".to_string(),
                ))
            }
        }
        "map" => {
            let mut output = String::new();
            let mut file_count = 0usize;
            let mut dir_count = 0usize;

            output.push_str("# 📍 Project Atlas\n\n");
            output.push_str("> This file is an auto-generated map for the AI agent to maintain spatial awareness.\n\n");
            output.push_str("## 📂 Directory Structure\n\n```text\n");

            let mut state = TreeWalkState {
                max_depth: 4,
                show_sizes: false,
                output: &mut output,
                file_count: &mut file_count,
                dir_count: &mut dir_count,
            };
            walk_project_tree(Path::new("."), "", 0, &mut state);
            output.push_str("```\n\n");
            output.push_str(&format!(
                "---\nGenerated at: {}\n{} directories, {} files\n",
                chrono::Local::now().format("%Y-%m-%d %H:%M:%S"),
                dir_count,
                file_count
            ));

            fs::write(atlas_path, &output).map_err(|e| {
                ToolError::ExecutionFailed(format!("Failed to write atlas file: {}", e))
            })?;

            // Spawns indexing in the background
            let brain = tool_ctx.vector_brain.clone();
            let backend = tool_ctx.backend.clone();
            let brain_path = tool_ctx.brain_path.clone();
            let tx = tool_ctx.tx.clone();

            tokio::spawn(async move {
                let b = backend.read().await;
                if let Err(e) =
                    crate::tools::atlas::run_semantic_indexing(&b, brain, &brain_path, true, tx)
                        .await
                {
                    eprintln!("❌ Background indexing FAILED: {}", e);
                }
            });

            Ok(serde_json::Value::String(format!(
                "✅ Project Atlas generated and saved to '{}'. Conceptual re-indexing started in background.",
                atlas_path
            )))
        }
        _ => Err(ToolError::ExecutionFailed(format!(
            "Unknown project_atlas action '{}'.",
            action
        ))),
    }
}
