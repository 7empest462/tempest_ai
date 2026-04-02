use anyhow::Result;
use async_trait::async_trait;
use serde_json::Value;
use std::fs;
use super::{AgentTool, ToolContext};

pub struct TreeTool;

#[async_trait]
impl AgentTool for TreeTool {
    fn name(&self) -> &'static str { "tree" }
    fn description(&self) -> &'static str { "Shows a recursive directory tree view. Excludes hidden directories and common noise (node_modules, target, .git) by default." }
    fn parameters(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "path": { "type": "string", "description": "Root directory to display tree for" },
                "max_depth": { "type": "integer", "description": "Maximum depth to recurse (default: 4)" }
            },
            "required": ["path"]
        })
    }

    async fn execute(&self, args: &Value, _context: ToolContext) -> Result<String> {
        let path_str = args.get("path").and_then(|p| p.as_str()).unwrap_or(".");
        let path_owned = shellexpand::tilde(path_str).to_string();
        let max_depth = args.get("max_depth").and_then(|d| d.as_u64()).unwrap_or(4) as usize;

        let skip_dirs = ["node_modules", "target", ".git", "__pycache__", ".next", "dist", "build", ".DS_Store"];
        let mut output = String::new();
        let mut file_count = 0usize;
        let mut dir_count = 0usize;

        fn walk_tree(
            dir: &std::path::Path,
            prefix: &str,
            depth: usize,
            max_depth: usize,
            skip: &[&str],
            output: &mut String,
            file_count: &mut usize,
            dir_count: &mut usize,
        ) {
            if depth > max_depth { return; }
            let mut entries: Vec<_> = match fs::read_dir(dir) {
                Ok(rd) => rd.filter_map(|e| e.ok()).collect(),
                Err(_) => return,
            };
            entries.sort_by_key(|e| e.file_name());

            let total = entries.len();
            for (i, entry) in entries.iter().enumerate() {
                let name = entry.file_name().to_string_lossy().to_string();
                if skip.contains(&name.as_str()) || name.starts_with('.') {
                    continue;
                }

                let is_last = i == total - 1;
                let connector = if is_last { "└── " } else { "├── " };
                let child_prefix = if is_last { "    " } else { "│   " };

                let path = entry.path();
                if path.is_dir() {
                    *dir_count += 1;
                    output.push_str(&format!("{}{}{}/\n", prefix, connector, name));
                    walk_tree(&path, &format!("{}{}", prefix, child_prefix), depth + 1, max_depth, skip, output, file_count, dir_count);
                } else {
                    *file_count += 1;
                    let size = fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
                    let size_str = if size > 1_000_000 {
                        format!("{:.1}MB", size as f64 / 1_000_000.0)
                    } else if size > 1_000 {
                        format!("{:.1}KB", size as f64 / 1_000.0)
                    } else {
                        format!("{}B", size)
                    };
                    output.push_str(&format!("{}{}{} ({})\n", prefix, connector, name, size_str));
                }
            }
        }

        let root = std::path::Path::new(&path_owned);
        output.push_str(&format!("{}/\n", path_owned));
        walk_tree(root, "", 0, max_depth, &skip_dirs, &mut output, &mut file_count, &mut dir_count);
        output.push_str(&format!("\n{} directories, {} files", dir_count, file_count));

        Ok(output)
    }
}

pub struct ProjectAtlasTool;

#[async_trait]
impl AgentTool for ProjectAtlasTool {
    fn name(&self) -> &'static str { "project_atlas" }
    fn description(&self) -> &'static str { "Generates or updates a '.tempest_atlas.md' file in the project root to maintain spatial project awareness." }
    fn parameters(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "action": { "type": "string", "description": "'map' to generate, 'read' to view existing atlas" }
            },
            "required": ["action"]
        })
    }

    async fn execute(&self, args: &Value, _context: ToolContext) -> Result<String> {
        let action = args.get("action").and_then(|a| a.as_str()).unwrap_or("map");
        let atlas_path = ".tempest_atlas.md";

        match action {
            "read" => {
                if let Ok(content) = fs::read_to_string(atlas_path) {
                    Ok(format!("📍 CURRENT PROJECT ATLAS:\n\n{}", content))
                } else {
                    Ok("❌ Atlas not found. Use 'map' to generate it first.".to_string())
                }
            },
            "map" => {
                let skip_dirs = ["node_modules", "target", ".git", "__pycache__", ".next", "dist", "build", ".DS_Store"];
                let mut output = String::new();
                let mut file_count = 0usize;
                let mut dir_count = 0usize;

                output.push_str("# 📍 Project Atlas\n\n");
                output.push_str("> This file is an auto-generated map for the AI agent to maintain spatial awareness.\n\n");
                output.push_str("## 📂 Directory Structure\n\n```text\n");

                fn walk_atlas(
                    dir: &std::path::Path,
                    prefix: &str,
                    depth: usize,
                    max_depth: usize,
                    skip: &[&str],
                    output: &mut String,
                    file_count: &mut usize,
                    dir_count: &mut usize,
                ) {
                    if depth > max_depth { return; }
                    let mut entries: Vec<_> = match fs::read_dir(dir) {
                        Ok(rd) => rd.filter_map(|e| e.ok()).collect(),
                        Err(_) => return,
                    };
                    entries.sort_by_key(|e| e.file_name());

                    let total = entries.len();
                    for (i, entry) in entries.iter().enumerate() {
                        let name = entry.file_name().to_string_lossy().to_string();
                        if skip.contains(&name.as_str()) || name.starts_with('.') {
                            continue;
                        }

                        let is_last = i == total - 1;
                        let connector = if is_last { "└── " } else { "├── " };
                        let child_prefix = if is_last { "    " } else { "│   " };

                        let path = entry.path();
                        if path.is_dir() {
                            *dir_count += 1;
                            output.push_str(&format!("{}{}{}/\n", prefix, connector, name));
                            walk_atlas(&path, &format!("{}{}", prefix, child_prefix), depth + 1, max_depth, skip, output, file_count, dir_count);
                        } else {
                            *file_count += 1;
                            output.push_str(&format!("{}{}{}\n", prefix, connector, name));
                        }
                    }
                }

                walk_atlas(std::path::Path::new("."), "", 0, 4, &skip_dirs, &mut output, &mut file_count, &mut dir_count);
                output.push_str("```\n\n");
                output.push_str(&format!("---\nGenerated at: {}\n{} directories, {} files\n", 
                    chrono::Local::now().format("%Y-%m-%d %H:%M:%S"), dir_count, file_count));

                fs::write(atlas_path, &output)?;
                Ok(format!("✅ Project Atlas generated and saved to '{}'.", atlas_path))
            },
            _ => anyhow::bail!("Unknown project_atlas action '{}'.", action),
        }
    }
}
