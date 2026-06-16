// ==========================================
// 🔍 SKG SEARCH TOOLS — Native Skelegent Implementations
// ==========================================
// Replaces the legacy GrepSearchTool, SemanticSearchTool, and IndexFileSemanticallyTool.

use skg_tool::{ToolCallContext, ToolError};
use skg_tool_macro::skg_tool;

// ── grep_search ────────────────────────────────────────────────────────────────

#[skg_tool(
    name = "grep_search",
    description = "Performs a fast keyword search across the project. Use this for finding exact variable names, function calls, or error strings."
)]
pub async fn grep_search(
    query: String,
    path: Option<String>,
    _ctx: &ToolCallContext,
) -> Result<serde_json::Value, ToolError> {
    let mut search_path = path.unwrap_or_else(|| ".".to_string());
    if search_path.is_empty() {
        search_path = ".".to_string();
    }

    let query_clone = query.clone();

    let raw_results = tokio::task::spawn_blocking(move || -> Result<Vec<String>, ToolError> {
        use grep::regex::RegexMatcher;
        use grep::searcher::{Searcher, sinks::UTF8};
        use ignore::WalkBuilder;

        let matcher = RegexMatcher::new(&query_clone)
            .map_err(|e| ToolError::ExecutionFailed(format!("Invalid regex: {}", e)))?;
        let mut searcher = Searcher::new();
        let mut results = Vec::new();

        for result in WalkBuilder::new(&search_path).build() {
            let entry = match result {
                Ok(e) => e,
                Err(_) => continue,
            };

            if !entry.file_type().is_some_and(|ft| ft.is_file()) {
                continue;
            }

            let path_str = entry.path().to_string_lossy().to_string();

            let _ = searcher.search_path(
                &matcher,
                entry.path(),
                UTF8(|lnum, line| {
                    let mut content = line.trim_end().to_string();
                    if content.len() > 200 {
                        content.truncate(200);
                        content.push_str("...");
                    }
                    results.push(format!("{}:{}:{}", path_str, lnum, content));
                    Ok(true)
                }),
            );
        }
        Ok(results)
    })
    .await
    .map_err(|e| ToolError::ExecutionFailed(format!("Task join error: {}", e)))??;

    use fuzzy_matcher::FuzzyMatcher;
    use fuzzy_matcher::skim::SkimMatcherV2;
    use rayon::prelude::*;

    let mut ranked: Vec<(i64, String)> = raw_results
        .into_par_iter()
        .map(|line| {
            let matcher = SkimMatcherV2::default();
            if let Some(score) = matcher.fuzzy_match(&line, &query) {
                (score, line)
            } else {
                (0, line)
            }
        })
        .collect();

    ranked.par_sort_by(|a, b| b.0.cmp(&a.0));

    let report = ranked
        .into_iter()
        .take(100)
        .map(|(_, line)| line)
        .collect::<Vec<_>>()
        .join("\n");

    Ok(serde_json::Value::String(report))
}

// ── semantic_search ────────────────────────────────────────────────────────────

#[skg_tool(
    name = "semantic_search",
    description = "Searches the project's conceptual index. Best for finding 'how' things are done or locating logic by meaning rather than exact keywords."
)]
pub async fn semantic_search(
    query: String,
    top_k: Option<usize>,
    ctx: &ToolCallContext,
) -> Result<serde_json::Value, ToolError> {
    let top_k = top_k.unwrap_or(5);

    // Access Ollama client and VectorBrain from ToolContext dependency
    let tool_ctx = ctx
        .deps::<std::sync::Arc<crate::tools::ToolContext>>()
        .ok_or_else(|| ToolError::ExecutionFailed("Missing ToolContext dependency".to_string()))?;

    let ollama = tool_ctx.ollama.clone();
    let vector_brain = tool_ctx.vector_brain.clone();

    let req = ollama_rs::generation::embeddings::request::GenerateEmbeddingsRequest::new(
        "nomic-embed-text".to_string(),
        query.clone().into(),
    );

    match ollama.generate_embeddings(req).await {
        Ok(res) => {
            if let Some(embedding) = res.embeddings.first() {
                let hits = vector_brain.lock().search(embedding, top_k);
                let mut report = format!("Conceptual matches for '{}':\n\n", query);
                for (entry, sim) in hits {
                    report.push_str(&format!(
                        "[{:.1}%] {}: {}\n---\n",
                        sim * 100.0,
                        entry.source,
                        entry.text
                    ));
                }
                Ok(serde_json::Value::String(report))
            } else {
                Ok(serde_json::Value::String(
                    "No embeddings generated for query.".to_string(),
                ))
            }
        }
        Err(e) => Err(ToolError::ExecutionFailed(format!(
            "Embedding error: {}",
            e
        ))),
    }
}

// ── index_file_semantically ────────────────────────────────────────────────────

#[skg_tool(
    name = "index_file_semantically",
    description = "Manually parses and indexes a local file into your conceptual search index. Use this to 'train' yourself on new codebase logic."
)]
pub async fn index_file_semantically(
    path: String,
    ctx: &ToolCallContext,
) -> Result<serde_json::Value, ToolError> {
    let file_path = shellexpand::tilde(&path).to_string();

    let content = std::fs::read_to_string(&file_path)
        .map_err(|e| ToolError::ExecutionFailed(format!("Cannot read file: {}", e)))?;

    // Access Ollama, VectorBrain, and brain_path from ToolContext dependency
    let tool_ctx = ctx
        .deps::<std::sync::Arc<crate::tools::ToolContext>>()
        .ok_or_else(|| ToolError::ExecutionFailed("Missing ToolContext dependency".to_string()))?;

    let ollama = tool_ctx.ollama.clone();
    let vector_brain = tool_ctx.vector_brain.clone();
    let brain_path = tool_ctx.brain_path.clone();

    // Chunk the file
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

    let mut success_count = 0;
    {
        let mut brain = vector_brain.lock();
        brain.remove_entries_by_source_prefix(&file_path);
    }

    for (i, chunk) in chunks.iter().enumerate() {
        let req = ollama_rs::generation::embeddings::request::GenerateEmbeddingsRequest::new(
            "nomic-embed-text".to_string(),
            chunk.clone().into(),
        );

        match ollama.generate_embeddings(req).await {
            Ok(res) => {
                if let Some(embedding) = res.embeddings.first() {
                    let mut brain = vector_brain.lock();
                    brain.add_entry(
                        chunk.clone(),
                        embedding.clone(),
                        format!("{} (Chunk {})", file_path, i + 1),
                        std::collections::HashMap::new(),
                    );
                    success_count += 1;
                }
            }
            Err(e) => {
                return Err(ToolError::ExecutionFailed(format!(
                    "Embedding index error on chunk {}: {}",
                    i + 1,
                    e
                )));
            }
        }
    }

    let brain = vector_brain.lock();
    let _ = brain.save_to_disk(brain_path);

    Ok(serde_json::json!({
        "status": "success",
        "message": format!("✅ Successfully indexed file: {} ({} bytes across {} chunks)", file_path, content.len(), success_count)
    }))
}
