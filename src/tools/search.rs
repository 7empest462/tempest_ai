use super::{AgentTool, ToolContext};
use async_trait::async_trait;
use miette::{IntoDiagnostic, Result, miette};
use ollama_rs::generation::tools::{ToolFunctionInfo, ToolInfo, ToolType};
use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::Value;
use std::sync::Arc;

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
    fn name(&self) -> &'static str {
        "semantic_search"
    }
    fn description(&self) -> &'static str {
        "Searches the project's conceptual index. Best for finding 'how' things are done or locating logic by meaning rather than exact keywords."
    }
    fn tool_info(&self) -> ToolInfo {
        let mut settings = schemars::generate::SchemaSettings::draft07();
        settings.inline_subschemas = true;
        let payload = settings
            .into_generator()
            .into_root_schema_for::<SemanticSearchArgs>();

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
        let typed_args: SemanticSearchArgs =
            serde_json::from_value(args.clone()).into_diagnostic()?;
        let query = typed_args.query;
        let top_k = typed_args.top_k.unwrap_or(5);

        let req = ollama_rs::generation::embeddings::request::GenerateEmbeddingsRequest::new(
            context.embedding_model.clone(),
            query.clone().into(),
        );

        match context.ollama.generate_embeddings(req).await {
            Ok(res) => {
                if let Some(embedding) = res.embeddings.first() {
                    let hits = context.vector_brain.lock().search(embedding, top_k);
                    let mut report = format!("Conceptual matches for '{}':\n\n", query);
                    for (entry, sim) in hits {
                        report.push_str(&format!(
                            "[{:.1}%] {}: {}\n---\n",
                            sim * 100.0,
                            entry.source,
                            entry.text
                        ));
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
    fn name(&self) -> &'static str {
        "grep_search"
    }
    fn description(&self) -> &'static str {
        "Performs a fast keyword search across the project. Use this for finding exact variable names, function calls, or error strings."
    }
    fn tool_info(&self) -> ToolInfo {
        let mut settings = schemars::generate::SchemaSettings::draft07();
        settings.inline_subschemas = true;
        let payload = settings
            .into_generator()
            .into_root_schema_for::<GrepSearchArgs>();

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
        let typed_args: GrepSearchArgs = serde_json::from_value(args.clone()).into_diagnostic()?;
        let query = typed_args.query.clone();
        let mut path = typed_args.path.unwrap_or_else(|| ".".to_string());
        if path.is_empty() {
            path = ".".to_string();
        }

        // Run the native IO-bound search in a blocking thread to avoid starving the async executor
        let raw_results = tokio::task::spawn_blocking(move || -> Result<Vec<String>> {
            use grep::regex::RegexMatcher;
            use grep::searcher::{Searcher, sinks::UTF8};
            use ignore::WalkBuilder;

            let matcher = RegexMatcher::new(&query).into_diagnostic()?;
            let mut searcher = Searcher::new();
            let mut results = Vec::new();

            for result in WalkBuilder::new(&path).build() {
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
                        // Mimic rg -n format: file_path:line_num:line_content
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
        .into_diagnostic()??;

        let query = typed_args.query; // Shadow for async use

        use fuzzy_matcher::FuzzyMatcher;
        use fuzzy_matcher::skim::SkimMatcherV2;
        use rayon::prelude::*;

        let mut ranked: Vec<(i64, String)> = raw_results
            .into_par_iter()
            .map(|line| {
                // SkimMatcherV2 is lightweight to clone or recreate if needed
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

        Ok(report)
    }
}

pub struct IndexFileSemanticallyTool;

#[async_trait]
impl AgentTool for IndexFileSemanticallyTool {
    fn name(&self) -> &'static str {
        "index_file_semantically"
    }
    fn description(&self) -> &'static str {
        "Manually parses and indexes a local file into your conceptual search index. Use this to 'train' yourself on new codebase logic."
    }
    fn tool_info(&self) -> ToolInfo {
        let mut settings = schemars::generate::SchemaSettings::draft07();
        settings.inline_subschemas = true;
        let payload = settings
            .into_generator()
            .into_root_schema_for::<IndexFileSemanticallyArgs>();

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
        let typed_args: IndexFileSemanticallyArgs =
            serde_json::from_value(args.clone()).into_diagnostic()?;
        let path = shellexpand::tilde(&typed_args.path).to_string();

        let content = std::fs::read_to_string(&path).into_diagnostic()?;

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
            let mut brain = context.vector_brain.lock();
            brain.remove_entries_by_source_prefix(&path);
        }

        for (i, chunk) in chunks.iter().enumerate() {
            let req = ollama_rs::generation::embeddings::request::GenerateEmbeddingsRequest::new(
                context.embedding_model.clone(),
                chunk.clone().into(),
            );

            match context.ollama.generate_embeddings(req).await {
                Ok(res) => {
                    if let Some(embedding) = res.embeddings.first() {
                        let mut brain = context.vector_brain.lock();
                        brain.add_entry(
                            chunk.clone(),
                            embedding.clone(),
                            format!("{} (Chunk {})", path, i + 1),
                            std::collections::HashMap::new(),
                        );
                        success_count += 1;
                    }
                }
                Err(e) => {
                    return Err(miette!("Embedding index error on chunk {}: {}", i + 1, e));
                }
            }
        }

        let brain = context.vector_brain.lock();
        let _ = brain.save_to_disk(context.brain_path);

        Ok(format!(
            "✅ Successfully indexed file: {} ({} bytes across {} chunks)",
            path,
            content.len(),
            success_count
        ))
    }
}

#[derive(Deserialize, JsonSchema)]
pub struct IndexFileSemanticallyArgs {
    /// The path to the file to index.
    pub path: String,
}

#[derive(Deserialize, JsonSchema)]
pub struct IndexFileConceptuallyArgs {
    /// The path to the file to index.
    pub path: String,
    /// Model to use for conceptual summarization (optional, defaults to sub-model)
    pub model: Option<String>,
}

pub struct IndexFileConceptuallyTool;

#[async_trait]
impl AgentTool for IndexFileConceptuallyTool {
    fn name(&self) -> &'static str {
        "index_file_conceptually"
    }
    fn description(&self) -> &'static str {
        "Parses a local file, generates conceptual summaries for each chunk using a background sub-agent, and indexes both raw text and summaries semantically."
    }
    fn tool_info(&self) -> ToolInfo {
        let mut settings = schemars::generate::SchemaSettings::draft07();
        settings.inline_subschemas = true;
        let payload = settings
            .into_generator()
            .into_root_schema_for::<IndexFileConceptuallyArgs>();

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
        let typed_args: IndexFileConceptuallyArgs =
            serde_json::from_value(args.clone()).into_diagnostic()?;
        let path = shellexpand::tilde(&typed_args.path).to_string();

        let content = std::fs::read_to_string(&path).into_diagnostic()?;

        // A smaller chunk size (4000 chars) is better for high-quality conceptual indexing
        let chunk_size = 4000;
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

        {
            let mut brain = context.vector_brain.lock();
            brain.remove_entries_by_source_prefix(&path);
        }

        let sub_model = typed_args
            .model
            .unwrap_or_else(|| context.sub_agent_model.clone());
        let backend = context.backend.read().await.clone();

        let mut success_count = 0;

        for (i, chunk) in chunks.iter().enumerate() {
            let summarize_prompt = format!(
                "### TASK: CONCEPT EXTRACTION\n\
                 Analyze the following code/text chunk and extract a high-density, conceptual summary.\n\
                 Include: key functions/APIs exported, core logical flow, dependencies, and main purpose.\n\
                 Do not include any preamble or conversational filler. Start directly with the summary.\n\n\
                 ### CHUNK:\n{}",
                chunk
            );

            // Generate conceptual summary using ollama coordinator (or standard fallback if not ollama)
            let summary_text = match &backend {
                crate::inference::Backend::Ollama(ollama, _) => {
                    let options = ollama_rs::models::ModelOptions::default()
                        .temperature(0.1)
                        .num_ctx(4096);
                    let mut coordinator = ollama_rs::coordinator::Coordinator::new(
                        ollama.clone(),
                        sub_model.clone(),
                        vec![],
                    )
                    .options(options)
                    .think(ollama_rs::generation::parameters::ThinkType::Low);

                    let chat_fut =
                        coordinator.chat(vec![ollama_rs::generation::chat::ChatMessage::new(
                            ollama_rs::generation::chat::MessageRole::User,
                            summarize_prompt,
                        )]);
                    match tokio::time::timeout(tokio::time::Duration::from_secs(30), chat_fut).await
                    {
                        Ok(Ok(response)) => response.message.content,
                        _ => "Failed to generate conceptual summary (timeout/error).".to_string(),
                    }
                }
                _ => {
                    let sampling = crate::inference::SamplingConfig {
                        temperature: 0.1,
                        top_p: 0.9,
                        repeat_penalty: 1.1,
                        context_size: 4096,
                    };
                    let stop = Arc::new(std::sync::atomic::AtomicBool::new(false));
                    let event_tx = Arc::new(parking_lot::Mutex::new(None));
                    let cloned_backend = backend.clone();
                    let chat_fut = cloned_backend.stream_chat(crate::inference::ChatRequest {
                        model: sub_model.clone(),
                        history: vec![ollama_rs::generation::chat::ChatMessage::new(
                            ollama_rs::generation::chat::MessageRole::User,
                            summarize_prompt,
                        )],
                        sampling,
                        event_tx,
                        stop,
                        system_prompt: "".to_string(),
                        on_tool_call: None,
                        tool_registry: None,
                    });
                    match tokio::time::timeout(tokio::time::Duration::from_secs(30), chat_fut).await
                    {
                        Ok(Ok(response)) => response.content,
                        _ => "Failed to generate conceptual summary (timeout/error).".to_string(),
                    }
                }
            };

            // Combine summary with raw text to create a rich semantic block
            let conceptual_text = format!(
                "File: {}\n\nSummary:\n{}\n\nRaw Chunk:\n{}",
                path, summary_text, chunk
            );

            // Generate embeddings for the combined conceptual block
            let req = ollama_rs::generation::embeddings::request::GenerateEmbeddingsRequest::new(
                context.embedding_model.clone(),
                conceptual_text.clone().into(),
            );

            match context.ollama.generate_embeddings(req).await {
                Ok(res) => {
                    if let Some(embedding) = res.embeddings.first() {
                        let mut brain = context.vector_brain.lock();
                        brain.add_entry(
                            conceptual_text,
                            embedding.clone(),
                            format!("{} (Conceptual Chunk {})", path, i + 1),
                            std::collections::HashMap::new(),
                        );
                        success_count += 1;
                    }
                }
                Err(e) => {
                    return Err(miette!("Embedding index error on chunk {}: {}", i + 1, e));
                }
            }
        }

        let brain = context.vector_brain.lock();
        let _ = brain.save_to_disk(&context.brain_path);

        Ok(format!(
            "✅ Successfully indexed file conceptually: {} ({} bytes across {} chunks)",
            path,
            content.len(),
            success_count
        ))
    }
}
