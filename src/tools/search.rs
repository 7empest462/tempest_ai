use serde_json::{json, Value};
use anyhow::Result;
use async_trait::async_trait;
use super::{AgentTool, ToolContext};
use crate::tools::execution::RunCommandTool;

pub struct SemanticSearchTool;

#[async_trait]
impl AgentTool for SemanticSearchTool {
    fn name(&self) -> &'static str { "semantic_search" }
    fn description(&self) -> &'static str { "Searches the project's conceptual index. Best for finding 'how' things are done or locating logic by meaning rather than exact keywords." }
    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "query": { "type": "string", "description": "The search query in natural language." },
                "top_k": { "type": "integer", "description": "Number of results (default 5)" }
            },
            "required": ["query"]
        })
    }

    async fn execute(&self, args: &Value, context: ToolContext) -> Result<String> {
        let query = args.get("query").and_then(|q| q.as_str()).unwrap().to_string();
        let top_k = args.get("top_k").and_then(|k| k.as_u64()).unwrap_or(5) as usize;

        let req = ollama_rs::generation::embeddings::request::GenerateEmbeddingsRequest::new(
            "nomic-embed-text".to_string(),
            query.clone().into()
        );

        match context.ollama.generate_embeddings(req).await {
            Ok(res) => {
                if let Some(embedding) = res.embeddings.first() {
                    let hits = context.vector_brain.lock().expect("VectorBrain Poisoned").search(embedding, top_k);
                    let mut report = format!("Conceptual matches for '{}':\n\n", query);
                    for (entry, sim) in hits {
                        report.push_str(&format!("[{:.1}%] {}: {}\n---\n", sim * 100.0, entry.source, entry.text));
                    }
                    Ok(report)
                } else {
                    Ok("No embeddings generated for query.".to_string())
                }
            }
            Err(e) => anyhow::bail!("Embedding error: {}", e),
        }
    }
}

pub struct GrepSearchTool;

#[async_trait]
impl AgentTool for GrepSearchTool {
    fn name(&self) -> &'static str { "grep_search" }
    fn description(&self) -> &'static str { "Performs a fast keyword search across the project. Use this for finding exact variable names, function calls, or error strings." }
    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "query": { "type": "string", "description": "Keywords or pattern to search for." },
                "path": { "type": "string", "description": "Optional directory to restrict search (default '.')." }
            },
            "required": ["query"]
        })
    }

    async fn execute(&self, args: &Value, context: ToolContext) -> Result<String> {
        let query = args.get("query").and_then(|q| q.as_str()).unwrap();
        let path = args.get("path").and_then(|p| p.as_str()).unwrap_or(".");
        
        let cmd = format!("rg --version >/dev/null 2>&1 && rg -n --no-heading --max-columns=200 \"{}\" {} || grep -rn \"{}\" {}", 
            query, path, query, path);
            
        let exec_args = json!({ "command": cmd });
        RunCommandTool.execute(&exec_args, context).await
    }
}
pub struct IndexFileSemanticallyTool;

#[async_trait]
impl AgentTool for IndexFileSemanticallyTool {
    fn name(&self) -> &'static str { "index_file_semantically" }
    fn description(&self) -> &'static str { "Manually parses and indexes a local file into your conceptual search index. Use this to 'train' yourself on new codebase logic." }
    fn is_modifying(&self) -> bool { true }
    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": { "type": "string", "description": "The path to the file to index." }
            },
            "required": ["path"]
        })
    }

    async fn execute(&self, args: &Value, context: ToolContext) -> Result<String> {
        let path_str = args.get("path").and_then(|p| p.as_str()).ok_or_else(|| anyhow::anyhow!("Missing 'path'"))?;
        let path = shellexpand::tilde(path_str).to_string();

        let content = std::fs::read_to_string(&path)?;
        
        let req = ollama_rs::generation::embeddings::request::GenerateEmbeddingsRequest::new(
            "nomic-embed-text".to_string(),
            content.clone().into()
        );

        match context.ollama.generate_embeddings(req).await {
            Ok(res) => {
                if let Some(embedding) = res.embeddings.first() {
                    let mut brain = context.vector_brain.lock().map_err(|_| anyhow::anyhow!("Brain Poisoned"))?;
                    brain.add_entry(content.clone(), embedding.clone(), path.clone(), std::collections::HashMap::new());
                    let _ = brain.save_to_disk(context.brain_path);
                    Ok(format!("✅ Successfully indexed file: {} ({} bytes)", path, content.len()))
                } else {
                    Ok("No embeddings generated for file content.".to_string())
                }
            }
            Err(e) => anyhow::bail!("Embedding index error: {}", e),
        }
    }
}
