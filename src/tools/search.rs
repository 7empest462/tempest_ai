use serde_json::{json, Value};
use miette::{Result, IntoDiagnostic, miette};
use async_trait::async_trait;
use super::{AgentTool, ToolContext};
use crate::tools::execution::RunCommandTool;
use schemars::JsonSchema;
use serde::Deserialize;
use ollama_rs::generation::tools::{ToolInfo, ToolFunctionInfo, ToolType};

#[derive(Deserialize, JsonSchema)]
pub struct SemanticSearchArgs {
    /// The search query in natural language.
    pub query: String,
    /// Number of results (default 5)
    pub top_k: Option<usize>,
}

pub struct SemanticSearchTool;

#[async_trait]
impl AgentTool for SemanticSearchTool {
    fn name(&self) -> &'static str { "semantic_search" }
    fn description(&self) -> &'static str { "Searches the project's conceptual index. Best for finding 'how' things are done or locating logic by meaning rather than exact keywords." }
    fn tool_info(&self) -> ToolInfo {
        let mut settings = schemars::generate::SchemaSettings::draft07();
        settings.inline_subschemas = true;
        let payload = settings.into_generator().into_root_schema_for::<SemanticSearchArgs>();
        
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
        let typed_args: SemanticSearchArgs = serde_json::from_value(args.clone()).into_diagnostic()?;
        let query = typed_args.query;
        let top_k = typed_args.top_k.unwrap_or(5);

        let req = ollama_rs::generation::embeddings::request::GenerateEmbeddingsRequest::new(
            "nomic-embed-text".to_string(),
            query.clone().into()
        );

        match context.ollama.generate_embeddings(req).await {
            Ok(res) => {
                if let Some(embedding) = res.embeddings.first() {
                    let hits = context.vector_brain.lock().search(embedding, top_k);
                    let mut report = format!("Conceptual matches for '{}':\n\n", query);
                    for (entry, sim) in hits {
                        report.push_str(&format!("[{:.1}%] {}: {}\n---\n", sim * 100.0, entry.source, entry.text));
                    }
                    Ok(report)
                } else {
                    Ok("No embeddings generated for query.".to_string())
                }
            }
            Err(e) => Err(miette!("Embedding error: {}", e)),
        }
    }
}

#[derive(Deserialize, JsonSchema)]
pub struct GrepSearchArgs {
    /// Keywords or pattern to search for.
    pub query: String,
    /// Optional directory to restrict search (default '.').
    pub path: Option<String>,
}

pub struct GrepSearchTool;

#[async_trait]
impl AgentTool for GrepSearchTool {
    fn name(&self) -> &'static str { "grep_search" }
    fn description(&self) -> &'static str { "Performs a fast keyword search across the project. Use this for finding exact variable names, function calls, or error strings." }
    fn tool_info(&self) -> ToolInfo {
        let mut settings = schemars::generate::SchemaSettings::draft07();
        settings.inline_subschemas = true;
        let payload = settings.into_generator().into_root_schema_for::<GrepSearchArgs>();
        
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
        let typed_args: GrepSearchArgs = serde_json::from_value(args.clone()).into_diagnostic()?;
        let query = typed_args.query;
        let path = typed_args.path.unwrap_or_else(|| ".".to_string());
        
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
    fn tool_info(&self) -> ToolInfo {
        let mut settings = schemars::generate::SchemaSettings::draft07();
        settings.inline_subschemas = true;
        let payload = settings.into_generator().into_root_schema_for::<IndexFileSemanticallyArgs>();
        
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
        let typed_args: IndexFileSemanticallyArgs = serde_json::from_value(args.clone()).into_diagnostic()?;
        let path = shellexpand::tilde(&typed_args.path).to_string();

        let content = std::fs::read_to_string(&path).into_diagnostic()?;
        
        let req = ollama_rs::generation::embeddings::request::GenerateEmbeddingsRequest::new(
            "nomic-embed-text".to_string(),
            content.clone().into()
        );

        match context.ollama.generate_embeddings(req).await {
            Ok(res) => {
                if let Some(embedding) = res.embeddings.first() {
                    let mut brain = context.vector_brain.lock();
                    brain.add_entry(content.clone(), embedding.clone(), path.clone(), std::collections::HashMap::new());
                    let _ = brain.save_to_disk(context.brain_path);
                    Ok(format!("✅ Successfully indexed file: {} ({} bytes)", path, content.len()))
                } else {
                    Ok("No embeddings generated for file content.".to_string())
                }
            }
            Err(e) => Err(miette!("Embedding index error: {}", e)),
        }
    }
}

#[derive(Deserialize, JsonSchema)]
pub struct IndexFileSemanticallyArgs {
    /// The path to the file to index.
    pub path: String,
}
