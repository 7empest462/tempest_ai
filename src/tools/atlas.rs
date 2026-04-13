use miette::{Result, IntoDiagnostic, miette};
use async_trait::async_trait;
use serde_json::Value;
use std::fs;
use super::{AgentTool, ToolContext};
use schemars::JsonSchema;
use serde::Deserialize;
use ollama_rs::generation::tools::{ToolInfo, ToolFunctionInfo, ToolType};
use std::path::Path;
use walkdir::WalkDir;
use colored::*;
use std::sync::Arc;
use parking_lot::Mutex;
use ollama_rs::Ollama;
use crate::vector_brain::VectorBrain;

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
    fn name(&self) -> &'static str { "tree" }
    fn description(&self) -> &'static str { "Shows a recursive directory tree view. Excludes hidden directories and common noise (node_modules, target, .git) by default." }
    fn tool_info(&self) -> ToolInfo {
        let mut settings = schemars::generate::SchemaSettings::draft07();
        settings.inline_subschemas = true;
        let payload = settings.into_generator().into_root_schema_for::<TreeArgs>();
        
        ToolInfo {
            tool_type: ToolType::Function,
            function: ToolFunctionInfo {
                name: self.name().to_string(),
                description: self.description().to_string(),
                parameters: payload.into(),
            }
        }
    }

    async fn execute(&self, args: &Value, _context: ToolContext) -> Result<String> {
        let typed_args: TreeArgs = serde_json::from_value(args.clone()).into_diagnostic()?;
        let path_owned = shellexpand::tilde(&typed_args.path).to_string();
        let max_depth = typed_args.max_depth.unwrap_or(4) as usize;

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

#[derive(Deserialize, JsonSchema)]
pub struct ProjectAtlasArgs {
    /// 'map' to generate, 'read' to view existing atlas
    pub action: String,
}

pub struct ProjectAtlasTool;

#[async_trait]
impl AgentTool for ProjectAtlasTool {
    fn name(&self) -> &'static str { "project_atlas" }
    fn description(&self) -> &'static str { "📍 SYSTEM MAP: Generates/reads the local '.tempest_atlas.md' file. This is a NATIVE tool; do not attempt to call external 'atlas' binaries or Python scripts (e.g., /usr/bin/atlas_cli.py)." }
    fn tool_info(&self) -> ToolInfo {
        let mut settings = schemars::generate::SchemaSettings::draft07();
        settings.inline_subschemas = true;
        let payload = settings.into_generator().into_root_schema_for::<ProjectAtlasArgs>();
        
        ToolInfo {
            tool_type: ToolType::Function,
            function: ToolFunctionInfo {
                name: self.name().to_string(),
                description: self.description().to_string(),
                parameters: payload.into(),
            }
        }
    }

    async fn execute(&self, args: &Value, _context: ToolContext) -> Result<String> {
        let typed_args: ProjectAtlasArgs = serde_json::from_value(args.clone()).into_diagnostic()?;
        let action = typed_args.action;
        let atlas_path = ".tempest_atlas.md";

        match action.as_str() {
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

                fs::write(atlas_path, &output).into_diagnostic()?;
                
                // 🧠 SEMANTIC SYNC
                println!("{} Syncing conceptual map with project atlas...", "🧠".magenta().bold());
                let brain = _context.vector_brain.clone();
                let ollama = _context.ollama.clone();
                let brain_path = _context.brain_path.clone();
                
                tokio::spawn(async move {
                    let _ = run_semantic_indexing(&ollama, brain, &brain_path, true).await;
                });

                Ok(format!("✅ Project Atlas generated and saved to '{}'. Conceptual re-indexing started in background.", atlas_path))
            },
            _ => Err(miette!("Unknown project_atlas action '{}'.", action)),
        }
    }
}

pub async fn run_semantic_indexing(ollama: &Ollama, brain_lock: Arc<Mutex<VectorBrain>>, brain_path: &Path, force: bool) -> Result<()> {
    // 1. Check if we need to do anything
    {
        let mut brain = brain_lock.lock();
        if !brain.entries.is_empty() && !force {
            return Ok(());
        }
        if force {
            println!("{} Forced re-indexing triggered. Clearing old conceptual map...", "🔄".yellow().bold());
            brain.entries.clear();
        }
    }

    println!("{} Initializing Semantic Project Map...", "📍".blue().bold());
    
    let skip_dirs = ["node_modules", "target", ".git", "__pycache__", ".next", "dist", "build", ".DS_Store"];
    let extensions = ["rs", "toml", "md", "py", "js", "ts", "c", "cpp", "h", "sql", "sh"];
    
    let mut files_to_index = Vec::new();
    for entry in WalkDir::new(".")
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file()) {
            let path = entry.path();
            let name = entry.file_name().to_string_lossy();
            
            if name.starts_with('.') || path.components().any(|c| skip_dirs.contains(&c.as_os_str().to_str().unwrap_or(""))) {
                continue;
            }
            
            if let Some(ext) = path.extension().and_then(|s| s.to_str()) {
                if extensions.contains(&ext) {
                    files_to_index.push(path.to_path_buf());
                }
            }
    }

    if files_to_index.is_empty() {
        return Ok(());
    }

    println!("{} Found {} files for indexing. Processing conceptual embeddings...", "🔍".cyan(), files_to_index.len());

    for path in files_to_index {
        if let Ok(content) = std::fs::read_to_string(&path) {
            if content.trim().is_empty() { continue; }
            
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

            for (i, chunk) in chunks.iter().enumerate() {
                let req = ollama_rs::generation::embeddings::request::GenerateEmbeddingsRequest::new(
                    "nomic-embed-text".to_string(),
                    chunk.clone().into()
                );

                match ollama.generate_embeddings(req).await {
                    Ok(res) => {
                        if let Some(embedding) = res.embeddings.first() {
                            let mut brain = brain_lock.lock();
                            brain.add_entry(
                                chunk.clone(), 
                                embedding.clone(), 
                                format!("{} (Chunk {})", path.to_string_lossy(), i + 1), 
                                std::collections::HashMap::new()
                            );
                        }
                    }
                    Err(e) => {
                        eprintln!("{} Failed to index {} chunk {}: {}", "⚠️".yellow(), path.display(), i + 1, e);
                    }
                }
            }
        }
    }

    // Final save
    {
        let brain = brain_lock.lock();
        let _ = brain.save_to_disk(brain_path);
    }
    
    println!("{} Project indexing complete. Conceptual search is now ENABLED.", "✅".green().bold());
    
    Ok(())
}
