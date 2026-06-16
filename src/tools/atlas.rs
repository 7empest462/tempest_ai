use super::{AgentTool, ToolContext};
use crate::tui::AgentEvent;
use crate::vector_brain::VectorBrain;
use async_trait::async_trait;
use colored::*;
use miette::{IntoDiagnostic, Result, miette};
use ollama_rs::generation::tools::{ToolFunctionInfo, ToolInfo, ToolType};
use parking_lot::Mutex;
use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::Value;
use std::fs;
use std::path::Path;
use std::sync::Arc;
use walkdir::WalkDir;

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
const MAX_FILES_TO_INDEX: usize = 300;

struct TreeWalkState<'a> {
    max_depth: usize,
    show_sizes: bool,
    output: &'a mut String,
    file_count: &'a mut usize,
    dir_count: &'a mut usize,
}

fn walk_project_tree(dir: &std::path::Path, prefix: &str, depth: usize, state: &mut TreeWalkState) {
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

#[derive(Deserialize, JsonSchema)]
pub struct TreeArgs {
    /// Root directory to display tree for
    pub path: String,
    /// Maximum depth to recurse (default: 4)
    pub max_depth: Option<u64>,
}

pub struct TreeTool;

#[async_trait]
impl AgentTool for TreeTool {
    fn name(&self) -> &'static str {
        "tree"
    }
    fn description(&self) -> &'static str {
        "Shows a recursive directory tree view. Excludes hidden directories and common noise (node_modules, target, .git) by default."
    }
    fn tool_info(&self) -> ToolInfo {
        let mut settings = schemars::generate::SchemaSettings::draft07();
        settings.inline_subschemas = true;
        let payload = settings.into_generator().into_root_schema_for::<TreeArgs>();

        ToolInfo {
            tool_type: ToolType::Function,
            function: ToolFunctionInfo {
                name: self.name().to_string(),
                description: self.description().to_string(),
                parameters: payload,
            },
        }
    }

    async fn execute(&self, args: &Value, _context: ToolContext) -> Result<String> {
        let typed_args: TreeArgs = serde_json::from_value(args.clone()).into_diagnostic()?;
        let path_owned = shellexpand::tilde(&typed_args.path).to_string();
        let max_depth = typed_args.max_depth.unwrap_or(4) as usize;

        let mut output = String::new();
        let mut file_count = 0usize;
        let mut dir_count = 0usize;

        let root = std::path::Path::new(&path_owned);
        output.push_str(&format!("{}/\n", path_owned));
        let mut state = TreeWalkState {
            max_depth,
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

        Ok(output)
    }
}

#[derive(Deserialize, JsonSchema)]
pub struct ProjectAtlasArgs {
    /// 'map' to generate, 'read' to view existing atlas
    pub action: String,
}

pub struct ProjectAtlasTool;

#[async_trait]
impl AgentTool for ProjectAtlasTool {
    fn name(&self) -> &'static str {
        "project_atlas"
    }
    fn description(&self) -> &'static str {
        "📍 SYSTEM MAP: Generates/reads the local '.tempest_atlas.md' file. This is a NATIVE tool; do not attempt to call external 'atlas' binaries or Python scripts (e.g., /usr/bin/atlas_cli.py)."
    }
    fn tool_info(&self) -> ToolInfo {
        let mut settings = schemars::generate::SchemaSettings::draft07();
        settings.inline_subschemas = true;
        let payload = settings
            .into_generator()
            .into_root_schema_for::<ProjectAtlasArgs>();

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
        let typed_args: ProjectAtlasArgs =
            serde_json::from_value(args.clone()).into_diagnostic()?;
        let action = typed_args.action;
        let atlas_path = ".tempest_atlas.md";

        match action.as_str() {
            "read" => {
                if let Ok(content) = fs::read_to_string(atlas_path) {
                    Ok(format!("📍 CURRENT PROJECT ATLAS:\n\n{}", content))
                } else {
                    Ok("❌ Atlas not found. Use 'map' to generate it first.".to_string())
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
                walk_project_tree(std::path::Path::new("."), "", 0, &mut state);
                output.push_str("```\n\n");
                output.push_str(&format!(
                    "---\nGenerated at: {}\n{} directories, {} files\n",
                    chrono::Local::now().format("%Y-%m-%d %H:%M:%S"),
                    dir_count,
                    file_count
                ));

                fs::write(atlas_path, &output).into_diagnostic()?;

                // 🧠 SEMANTIC SYNC
                let brain = context.vector_brain.clone();
                let backend = context.backend.clone();
                let brain_path = context.brain_path.clone();
                let tx = context.tx.clone();

                tokio::spawn(async move {
                    let b = backend.read().await;
                    if let Err(e) = run_semantic_indexing(&b, brain, &brain_path, true, tx).await {
                        eprintln!("{} Background indexing FAILED: {}", "❌".red().bold(), e);
                    }
                });

                Ok(format!(
                    "✅ Project Atlas generated and saved to '{}'. Conceptual re-indexing started in background.",
                    atlas_path
                ))
            }
            _ => Err(miette!("Unknown project_atlas action '{}'.", action)),
        }
    }
}

pub async fn run_semantic_indexing(
    backend: &crate::inference::Backend,
    brain_lock: Arc<Mutex<VectorBrain>>,
    brain_path: &Path,
    force: bool,
    tx: Option<tokio::sync::mpsc::Sender<AgentEvent>>,
) -> Result<()> {
    let update = |msg: String| {
        let tx_clone = tx.clone();
        tokio::spawn(async move {
            if let Some(ref sender) = tx_clone {
                let _ = sender.send(AgentEvent::SystemUpdate(msg)).await;
            } else {
                println!("{}", msg);
            }
        });
    };

    // 1. Check if we need to do anything
    {
        let mut brain = brain_lock.lock();
        if !brain.entries.is_empty() && !force {
            return Ok(());
        }
        if force {
            update(
                "🔄 Forced re-indexing triggered. Clearing old conceptual map..."
                    .yellow()
                    .bold()
                    .to_string(),
            );
            brain.entries.clear();
        }
    }

    update(
        "📍 Initializing Semantic Project Map..."
            .blue()
            .bold()
            .to_string(),
    );

    let extensions = [
        "rs", "toml", "md", "py", "js", "ts", "c", "cpp", "h", "sql", "sh",
    ];

    let mut files_to_index = Vec::new();
    for entry in WalkDir::new(".")
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
    {
        let path = entry.path();
        let name = entry.file_name().to_string_lossy();

        if name.starts_with('.')
            || path
                .components()
                .any(|c| SKIP_DIRS.contains(&c.as_os_str().to_str().unwrap_or("")))
        {
            continue;
        }

        if let Some(ext) = path.extension().and_then(|s| s.to_str())
            && extensions.contains(&ext)
        {
            files_to_index.push(path.to_path_buf());
        }
    }

    if files_to_index.is_empty() {
        return Ok(());
    }

    let mut total_files = files_to_index.len();
    if total_files > MAX_FILES_TO_INDEX {
        update(
            format!(
                "⚠️ Project is large ({} files). Capping initial index at {} files for safety.",
                total_files, MAX_FILES_TO_INDEX
            )
            .yellow()
            .to_string(),
        );
        files_to_index.truncate(MAX_FILES_TO_INDEX);
        total_files = MAX_FILES_TO_INDEX;
    }

    update(
        format!(
            "🔍 Processing conceptual embeddings for {} files...",
            total_files
        )
        .cyan()
        .to_string(),
    );

    for (idx, path) in files_to_index.into_iter().enumerate() {
        if let Ok(content) = std::fs::read_to_string(&path) {
            if content.trim().is_empty() {
                continue;
            }

            let chunk_size = 6000;
            let mut chunks = Vec::new();
            let mut current_chunk = String::new();
            for line in content.lines() {
                if current_chunk.len() + line.len() > chunk_size && !current_chunk.is_empty() {
                    chunks.push(current_chunk.clone());
                    current_chunk.clear();
                }
                current_chunk.push_str(line);
                current_chunk.push('\n');
            }
            if !current_chunk.is_empty() {
                chunks.push(current_chunk);
            }

            if idx % 10 == 0 && idx > 0 {
                update(
                    format!(
                        "⏳ Indexing progress: {}/{} files complete...",
                        idx, total_files
                    )
                    .dimmed()
                    .to_string(),
                );
            }

            for (i, chunk) in chunks.iter().enumerate() {
                match backend.generate_embeddings(chunk).await {
                    Ok(embedding) => {
                        let mut brain = brain_lock.lock();
                        brain.add_entry(
                            chunk.clone(),
                            embedding,
                            format!("{} (Chunk {})", path.to_string_lossy(), i + 1),
                            std::collections::HashMap::new(),
                        );
                    }
                    Err(e) => {
                        update(
                            format!(
                                "⚠️ Failed to index {} chunk {}: {}",
                                path.display(),
                                i + 1,
                                e
                            )
                            .yellow()
                            .to_string(),
                        );
                    }
                }
                // 🌪️ Prevent Metal memory spike by yielding to the OS
                tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
            }
        }
    }

    let entries_count = {
        let brain = brain_lock.lock();
        let _ = brain.save_to_disk(brain_path);
        brain.entries.len()
    };

    if entries_count > 0 {
        update(
            "✅ Project indexing complete. Conceptual search is now ENABLED."
                .green()
                .bold()
                .to_string(),
        );
    } else {
        update("⚠️ Project indexing finished, but no conceptual embeddings were created. Semantic search will be limited.".yellow().to_string());
    }

    Ok(())
}
