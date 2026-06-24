// Copyright (c) 2026 Robert Simens. All Rights Reserved.
// Licensed under the Tempest AI Source-Available License.
// See LICENSE in the project root for full license information.

//! # Tool RAG — Dynamic Tool Schema Selection via Vector Similarity
//!
//! Instead of sending a static whitelist of tool schemas to the LLM on every turn,
//! this module embeds all tool descriptions into a vector index at startup.
//! On each user prompt, it queries the index for the top-K most relevant tools
//! and returns only those schemas — dramatically reducing token overhead,
//! hallucination risk, and GPU prefill latency.
//!
//! ## Architecture
//! - Uses `nomic-embed-text` (768-dim) for high-fidelity short-text retrieval
//! - Stores embeddings in a simple in-memory vector index (no external DB)
//! - Always includes a small set of "always-on" tools regardless of similarity
//! - The full toolbox remains discoverable via `query_schema`

use miette::{Result, miette};
use ollama_rs::generation::tools::{ToolFunctionInfo, ToolInfo, ToolType};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;

/// The embedding model used for tool description retrieval.
/// `nomic-embed-text` (768 dims) is chosen over `all-minilm` (384 dims) because
/// Tool RAG requires discriminating between short, semantically similar descriptions
/// (e.g., "read file" vs "search files" vs "list directory"), which benefits from
/// the higher-dimensional separation.
pub const TOOL_RAG_EMBEDDING_MODEL: &str = "nomic-embed-text";

/// Tools that are ALWAYS included in the schema regardless of semantic relevance.
/// These are fundamental operations the model must always have access to.
pub const ALWAYS_ON_TOOLS: &[&str] = &[
    "read_file",
    "write_file",
    "run_command",
    "ask_user",
    "query_schema",
];

/// Default number of dynamically selected tools per prompt.
const DEFAULT_TOP_K: usize = 8;

/// A serializable tool descriptor used for embedding and retrieval.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolDescriptor {
    pub name: String,
    pub description: String,
    /// Optional category tag for logging/filtering
    pub category: String,
}

/// An entry in the tool vector index: the descriptor + its embedding vector.
#[derive(Debug, Clone)]
struct ToolVectorEntry {
    descriptor: ToolDescriptor,
    embedding: Vec<f32>,
    /// The full ToolInfo schema to inject into the prompt when selected
    tool_info: ToolInfo,
}

/// The main Tool RAG index. Holds embedded tool descriptions and provides
/// similarity-based retrieval.
pub struct ToolVectorIndex {
    entries: Vec<ToolVectorEntry>,
    /// Map from tool name → full ToolInfo for always-on tools (fast lookup)
    always_on: HashMap<String, ToolInfo>,
    /// All tool infos for full discovery (used by query_schema)
    all_tool_infos: Vec<ToolInfo>,
}

fn tool_dyn_to_info(tool: &dyn skg_tool::ToolDyn) -> ToolInfo {
    let parameters: schemars::Schema =
        serde_json::from_value(tool.input_schema()).unwrap_or_else(|_| {
            let mut settings = schemars::generate::SchemaSettings::draft07();
            settings.inline_subschemas = true;
            settings.into_generator().into_root_schema_for::<()>()
        });
    ToolInfo {
        tool_type: ToolType::Function,
        function: ToolFunctionInfo {
            name: tool.name().to_string(),
            description: tool.description().to_string(),
            parameters,
        },
    }
}

impl ToolVectorIndex {
    pub fn normalize_vector(v: &mut [f32]) {
        let norm: f32 = v.iter().map(|&x| x * x).sum::<f32>().sqrt();
        if norm > 0.0 {
            for x in v.iter_mut() {
                *x /= norm;
            }
        }
    }

    /// Build the index from a set of tools by generating embeddings for each tool's
    /// `"{name}: {description}"` string.
    ///
    /// This runs once at startup and takes ~100-500ms depending on the number of tools.
    pub async fn build(
        tools: &[Arc<dyn skg_tool::ToolDyn>],
        backend: &crate::inference::Backend,
        event_tx: Arc<
            parking_lot::Mutex<Option<tokio::sync::mpsc::Sender<crate::tui::AgentEvent>>>,
        >,
    ) -> Result<Self> {
        let mut entries = Vec::with_capacity(tools.len());
        let mut always_on = HashMap::new();
        let all_tool_infos: Vec<ToolInfo> =
            tools.iter().map(|t| tool_dyn_to_info(t.as_ref())).collect();

        // Notify the TUI that we're building the tool index
        if let Some(tx) = event_tx.lock().clone() {
            let _ = tx.try_send(crate::tui::AgentEvent::SystemUpdate(format!(
                "🎯 [TOOL RAG]: Indexing {} tool descriptions with {}...",
                tools.len(),
                TOOL_RAG_EMBEDDING_MODEL
            )));
        }

        // Separate always-on tools first
        for tool in tools {
            if ALWAYS_ON_TOOLS.contains(&tool.name()) {
                always_on.insert(tool.name().to_string(), tool_dyn_to_info(tool.as_ref()));
            }
        }

        // Gather all descriptions for batch embedding
        let mut embed_texts = Vec::with_capacity(tools.len());
        for tool in tools {
            embed_texts.push(format!(
                "search_document: {}: {}",
                tool.name(),
                tool.description()
            ));
        }

        // Query batch embeddings
        let batch_res = backend.generate_batch_embeddings(&embed_texts).await;

        let mut embeddings = match batch_res {
            Ok(embeds) if embeds.len() == tools.len() => embeds,
            _ => {
                // Sequential fallback in case of errors
                let mut fallback_embeds = Vec::with_capacity(tools.len());
                for embed_text in &embed_texts {
                    match generate_tool_embedding(backend, embed_text).await {
                        Ok(emb) => fallback_embeds.push(emb),
                        Err(_) => fallback_embeds.push(Vec::new()),
                    }
                }
                fallback_embeds
            }
        };

        let mut embed_failures = 0usize;
        for (i, tool) in tools.iter().enumerate() {
            let mut embedding = std::mem::take(&mut embeddings[i]);
            let category = categorize_tool(tool.name());
            let tool_info = tool_dyn_to_info(tool.as_ref());

            if embedding.is_empty() {
                embed_failures += 1;
            } else {
                Self::normalize_vector(&mut embedding);
            }

            entries.push(ToolVectorEntry {
                descriptor: ToolDescriptor {
                    name: tool.name().to_string(),
                    description: tool.description().to_string(),
                    category,
                },
                embedding,
                tool_info,
            });
        }

        if let Some(tx) = event_tx.lock().clone() {
            let status = if embed_failures == 0 {
                format!(
                    "✅ [TOOL RAG]: Indexed {} tools successfully",
                    entries.len()
                )
            } else {
                format!(
                    "⚠️ [TOOL RAG]: Indexed {} tools ({} embedding failures — those tools remain available via always-on or query_schema)",
                    entries.len(),
                    embed_failures
                )
            };
            let _ = tx.try_send(crate::tui::AgentEvent::SystemUpdate(status));
        }

        Ok(Self {
            entries,
            always_on,
            all_tool_infos,
        })
    }

    pub fn build_fallback(tools: &[Arc<dyn skg_tool::ToolDyn>]) -> Self {
        let all_tool_infos: Vec<ToolInfo> =
            tools.iter().map(|t| tool_dyn_to_info(t.as_ref())).collect();
        let mut always_on = HashMap::new();
        for tool in tools {
            if ALWAYS_ON_TOOLS.contains(&tool.name()) {
                always_on.insert(tool.name().to_string(), tool_dyn_to_info(tool.as_ref()));
            }
        }
        Self {
            entries: Vec::new(),
            always_on,
            all_tool_infos,
        }
    }

    /// Query the index for the top-K tools most relevant to the given user prompt.
    ///
    /// Returns a deduplicated list combining:
    /// 1. Always-on tools (guaranteed presence)
    /// 2. Top-K tools by cosine similarity to the prompt embedding
    pub async fn resolve(
        &self,
        prompt: &str,
        backend: &crate::inference::Backend,
        top_k: Option<usize>,
    ) -> Result<(Vec<ToolInfo>, Vec<(String, f32)>)> {
        let k = top_k.unwrap_or(DEFAULT_TOP_K);

        // Generate the query embedding using nomic-embed-text
        let query_text = format!("search_query: {}", prompt);
        let query_vec = generate_tool_embedding(backend, &query_text).await?;

        if query_vec.is_empty() || self.entries.is_empty() {
            let mut result: Vec<ToolInfo> = Vec::new();
            for info in self.always_on.values() {
                result.push(info.clone());
            }
            return Ok((result, Vec::new()));
        }

        // Calculate the norm of the query vector once
        let query_norm: f32 = query_vec.iter().map(|&x| x * x).sum::<f32>().sqrt();
        if query_norm == 0.0 {
            let mut result: Vec<ToolInfo> = Vec::new();
            for info in self.always_on.values() {
                result.push(info.clone());
            }
            return Ok((result, Vec::new()));
        }

        // Score all entries by cosine similarity
        let mut scored: Vec<(&ToolVectorEntry, f32)> = self
            .entries
            .iter()
            .filter(|e| !e.embedding.is_empty())
            .map(|entry| {
                if entry.embedding.len() != query_vec.len() {
                    return (entry, 0.0);
                }
                let dot_product: f32 = entry
                    .embedding
                    .iter()
                    .zip(query_vec.iter())
                    .map(|(a, b)| a * b)
                    .sum();
                // Since entry.embedding is pre-normalized, its norm is 1.0.
                let sim = dot_product / query_norm;
                (entry, sim)
            })
            .collect();

        // Sort by similarity descending
        scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        // Build the final tool list
        let mut result: Vec<ToolInfo> = Vec::new();
        let mut selected_names: Vec<String> = Vec::new();
        let mut selection_log: Vec<(String, f32)> = Vec::new();

        // 1. Always-on tools first
        for (name, info) in &self.always_on {
            result.push(info.clone());
            selected_names.push(name.clone());
        }

        // 2. Top-K from similarity search (skip if already in always-on)
        for (entry, sim) in scored.iter().take(k) {
            if !selected_names.contains(&entry.descriptor.name) {
                result.push(entry.tool_info.clone());
                selected_names.push(entry.descriptor.name.clone());
                selection_log.push((entry.descriptor.name.clone(), *sim));
            }
        }

        Ok((result, selection_log))
    }

    /// Returns the full list of all tool infos for discovery via `query_schema`.
    pub fn all_tools(&self) -> &[ToolInfo] {
        &self.all_tool_infos
    }

    /// Returns the list of always-on tools.
    pub fn always_on_tools(&self) -> Vec<ToolInfo> {
        self.always_on.values().cloned().collect()
    }

    /// Returns the number of indexed tools.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    #[allow(dead_code)]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

/// Generate an embedding for a tool description using the dedicated tool RAG model.
///
/// We use `nomic-embed-text` here rather than the default `all-minilm` because
/// tool descriptions are short, semantically dense texts that benefit from
/// nomic's higher dimensionality (768 vs 384).
async fn generate_tool_embedding(
    backend: &crate::inference::Backend,
    text: &str,
) -> Result<Vec<f32>> {
    match backend {
        crate::inference::Backend::Ollama(ollama, embedding_model) => {
            let req = ollama_rs::generation::embeddings::request::GenerateEmbeddingsRequest::new(
                embedding_model.clone(),
                text.to_string().into(),
            );
            let res = ollama
                .generate_embeddings(req)
                .await
                .map_err(|e| miette!("Tool RAG embedding failed ({}): {}", embedding_model, e))?;
            Ok(res.embeddings.first().cloned().unwrap_or_default())
        }
        #[cfg(target_os = "macos")]
        crate::inference::Backend::MLX {
            ollama_fallback,
            embedding_model,
            ..
        } => {
            // MLX embedder uses all-minilm natively, so for Tool RAG we prefer
            // the Ollama fallback which can route to the configured embedding model
            if let Some(ollama) = ollama_fallback {
                let req =
                    ollama_rs::generation::embeddings::request::GenerateEmbeddingsRequest::new(
                        embedding_model.clone(),
                        text.to_string().into(),
                    );
                let res = ollama
                    .generate_embeddings(req)
                    .await
                    .map_err(|e| miette!("Tool RAG embedding fallback failed: {}", e))?;
                Ok(res.embeddings.first().cloned().unwrap_or_default())
            } else {
                Err(miette!(
                    "Tool RAG requires Ollama for {} embeddings (no fallback available)",
                    embedding_model
                ))
            }
        }
        crate::inference::Backend::Bridge(bridge) => {
            bridge.generate_embeddings(text.to_string()).await
        }
        crate::inference::Backend::Kalosm { .. } => Err(miette!(
            "Tool RAG embedding failed (Kalosm not supported yet)"
        )),
    }
}

/// Cosine similarity between two vectors.
pub fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != b.len() || a.is_empty() {
        return 0.0;
    }
    let dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
    let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm_a == 0.0 || norm_b == 0.0 {
        0.0
    } else {
        dot / (norm_a * norm_b)
    }
}

/// Categorize a tool by its name prefix for telemetry grouping.
pub fn categorize_tool(name: &str) -> String {
    if name.starts_with("git_") || name == "git_action" {
        return "git".to_string();
    }
    if name.starts_with("skg_") {
        return "skelegent".to_string();
    }
    if name.contains("file")
        || name == "list_dir"
        || name == "create_directory"
        || name == "delete_file"
        || name == "rename_file"
        || name == "append_file"
        || name == "patch_file"
        || name == "find_replace"
        || name == "diff_files"
    {
        return "filesystem".to_string();
    }
    if name.contains("web")
        || name == "read_url"
        || name == "http_request"
        || name == "download_file"
        || name == "stock_scraper"
    {
        return "web".to_string();
    }
    if name.contains("search") || name == "grep_search" || name == "semantic_search" {
        return "search".to_string();
    }
    if name.contains("memory")
        || name.contains("brain")
        || name.contains("knowledge")
        || name.contains("skill")
    {
        return "memory".to_string();
    }
    if name.contains("command")
        || name.contains("test")
        || name.contains("build")
        || name.contains("background")
        || name.contains("process")
    {
        return "execution".to_string();
    }
    if name.contains("telemetry")
        || name.contains("system")
        || name.contains("network")
        || name.contains("service")
        || name.contains("socket")
    {
        return "system".to_string();
    }
    if name.contains("cargo") || name.contains("rust") || name.contains("ast") {
        return "rust".to_string();
    }
    "general".to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cosine_similarity_identical() {
        let v = vec![1.0, 2.0, 3.0];
        let sim = cosine_similarity(&v, &v);
        assert!((sim - 1.0).abs() < 1e-6);
    }

    #[test]
    fn test_cosine_similarity_orthogonal() {
        let a = vec![1.0, 0.0];
        let b = vec![0.0, 1.0];
        let sim = cosine_similarity(&a, &b);
        assert!(sim.abs() < 1e-6);
    }

    #[test]
    fn test_cosine_similarity_empty() {
        let sim = cosine_similarity(&[], &[]);
        assert_eq!(sim, 0.0);
    }

    #[test]
    fn test_cosine_similarity_mismatched() {
        let sim = cosine_similarity(&[1.0, 2.0], &[1.0]);
        assert_eq!(sim, 0.0);
    }

    #[test]
    fn test_categorize_tool() {
        assert_eq!(categorize_tool("git_status"), "git");
        assert_eq!(categorize_tool("read_file"), "filesystem");
        assert_eq!(categorize_tool("search_web"), "web");
        assert_eq!(categorize_tool("run_command"), "execution");
        assert_eq!(categorize_tool("cargo_add"), "rust");
        assert_eq!(categorize_tool("ask_user"), "general");
    }

    #[test]
    fn test_always_on_tools_list() {
        // Ensure the always-on set contains the critical tools
        assert!(ALWAYS_ON_TOOLS.contains(&"read_file"));
        assert!(ALWAYS_ON_TOOLS.contains(&"write_file"));
        assert!(ALWAYS_ON_TOOLS.contains(&"run_command"));
        assert!(ALWAYS_ON_TOOLS.contains(&"ask_user"));
        assert!(ALWAYS_ON_TOOLS.contains(&"query_schema"));
    }
}
