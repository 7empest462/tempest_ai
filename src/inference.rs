use miette::{Result, miette};
#[cfg(target_os = "macos")]
use miette::IntoDiagnostic;
#[cfg(target_os = "macos")]
use colored::Colorize;
use ollama_rs::{
    generation::{
        chat::{request::ChatMessageRequest, ChatMessage, MessageRole},
        completion::request::GenerationRequest,
        parameters::{KeepAlive, TimeUnit},
    },
    models::ModelOptions,
    Ollama,
};
use futures::StreamExt;
use std::sync::Arc;
use llm_extract::Extract;

#[cfg(target_os = "macos")]
use mistralrs::{
    GgufModelBuilder, TextModelBuilder, Model, TextMessageRole, 
    Response as MistralResponse, RequestBuilder, SamplingParams,
    Tool, ToolChoice, Function, ToolType,
    PagedAttentionMetaBuilder, MemoryGpuConfig,
    EmbeddingModelBuilder, EmbeddingRequest
};
use crate::tui::AgentEvent;
use rig::completion::CompletionModel;
use rig::embeddings::EmbeddingModel;
use tool_parser::ToolParser;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum AgentMode {
    Ollama,
    MLX,
    Bridge,
}

#[derive(Clone)]
pub enum Backend {
    Ollama(Ollama),
    #[cfg(target_os = "macos")]
    MLX {
        model: std::sync::Arc<Model>,
        _ctx_limit: usize,
        embedder: Option<std::sync::Arc<Model>>,
    },
    Bridge(crate::ai_bridge::TempestAiBridge),
}

#[derive(Debug, Clone, Copy)]
pub struct SamplingConfig {
    pub temperature: f32,
    pub top_p: f32,
    pub repeat_penalty: f32,
    pub context_size: u64,
}

pub struct InferenceOutput {
    pub content: String,
    pub reasoning: String,
    pub native_tool_calls: Vec<ollama_rs::generation::tools::ToolCall>,
}

#[derive(serde::Deserialize, Extract, Debug)]
pub struct ToolCallPayload {
    pub name: String,
    pub arguments: serde_json::Value,
}

impl Backend {
    pub fn mode(&self) -> AgentMode {
        match self {
            Backend::Ollama(_) => AgentMode::Ollama,
            #[cfg(target_os = "macos")]
            Backend::MLX { .. } => AgentMode::MLX,
            Backend::Bridge(_) => AgentMode::Bridge,
        }
    }

    #[allow(dead_code)]
    pub fn supports_tools(&self) -> bool {
        match self {
            Backend::Ollama(_) => true,
            #[cfg(target_os = "macos")]
            Backend::MLX { .. } => true,
            Backend::Bridge(_) => false,
        }
    }

    pub async fn new(mode: AgentMode, model: String, quant: String, event_tx: Arc<parking_lot::Mutex<Option<tokio::sync::mpsc::Sender<AgentEvent>>>>, paged_attn: bool, ctx_limit: usize) -> Result<(Self, String)> {
        #[cfg(not(target_os = "macos"))]
        {
            let _ = quant;
            let _ = event_tx;
            let _ = paged_attn;
            let _ = ctx_limit;
        }

        match mode {
            AgentMode::Ollama => {
                Ok((Backend::Ollama(Ollama::default()), model))
            }
            AgentMode::MLX => {
                #[cfg(target_os = "macos")]
                {
                    let tx_opt = event_tx.lock().clone();
                    if let Some(tx) = tx_opt {
                        let _ = tx.send(AgentEvent::SubagentStatus(Some("🚀 Loading MLX Backend (Apple Silicon Neural Engine + GPU)...".to_string()))).await;
                        let _ = tx.send(AgentEvent::SubagentStatus(Some("📦 Note: Downloading/Verifying ~5GB model from Hugging Face if not cached...".to_string()))).await;
                    } else {
                        println!("{} Loading MLX Backend (Apple Silicon Neural Engine + GPU)...", "🚀".blue());
                        println!("{} Note: Downloading/Verifying model from Hugging Face if not cached...", "📦".blue());
                    }
                    
                    let tx_opt = event_tx.lock().clone();
                    if let Some(tx) = tx_opt {
                        let _ = tx.send(AgentEvent::SubagentStatus(Some(format!("⚡ [MLX]: Allocating Metal KV Cache ({}k context)...", ctx_limit / 1024)))).await;
                    } else {
                        if paged_attn {
                            println!("{} [MLX]: Allocating Paged Attention KV Cache (M4 Optimized)...", "⚡".yellow());
                        } else {
                            println!("{} [MLX]: Allocating Metal KV Cache ({}k context)...", "⚡".yellow(), ctx_limit / 1024);
                        }
                    }

                    let (repo, filename_prefix) = if model.contains("/") {
                        let parts: Vec<&str> = model.split('/').collect();
                        let mut prefix = parts.last().unwrap().to_string();
                        // Strip -GGUF suffix from filename if it exists in the repo name
                        if prefix.ends_with("-GGUF") {
                            prefix = prefix.strip_suffix("-GGUF").unwrap().to_string();
                        }
                        (model.clone(), prefix)
                    } else {
                        ("bartowski/Qwen2.5-Coder-7B-Instruct-GGUF".to_string(), "Qwen2.5-Coder-7B-Instruct".to_string())
                    };

                    let is_gguf = repo.to_lowercase().contains("gguf") || 
                                 std::path::Path::new(&repo).extension().map_or(false, |ext| ext == "gguf") ||
                                 std::path::Path::new(&repo).is_dir() && std::fs::read_dir(&repo).map_or(false, |mut dir| dir.any(|entry| entry.map_or(false, |e| e.file_name().to_string_lossy().ends_with(".gguf"))));

                    let mlx_model = if is_gguf {
                        let gguf_file = format!("{}-{}.gguf", filename_prefix, quant);
                        let mut builder = GgufModelBuilder::new(&repo, vec![gguf_file.clone()])
                            .with_logging()
                            .with_max_num_seqs(1);

                        if paged_attn {
                            println!("{} MLX: Initializing Paged Attention (Window: {} tokens)", "⚡".yellow(), ctx_limit);
                            let paged_attn_cfg = PagedAttentionMetaBuilder::default()
                                .with_block_size(16)
                                .with_gpu_memory(MemoryGpuConfig::ContextSize(ctx_limit))
                                .build()
                                .map_err(|e| miette!("Failed to configure Paged Attention: {}", e))?;
                            builder = builder.with_paged_attn(paged_attn_cfg);
                        }
                        builder.build().await.map_err(|e| miette!("Failed to load MLX GGUF model: {}", e))?
                    } else {
                        println!("{} MLX: Initializing Native Safetensors Backend...", "⚡".yellow());
                        let mut builder = TextModelBuilder::new(&repo)
                            .with_logging()
                            .with_max_num_seqs(1);
                        
                        if paged_attn {
                             let paged_attn_cfg = PagedAttentionMetaBuilder::default()
                                .with_block_size(16)
                                .with_gpu_memory(MemoryGpuConfig::ContextSize(ctx_limit))
                                .build()
                                .map_err(|e| miette!("Failed to configure Paged Attention: {}", e))?;
                            builder = builder.with_paged_attn(paged_attn_cfg);
                        }
                        builder.build().await.map_err(|e| miette!("Failed to load MLX Native model: {}", e))?
                    };

                    let embed_model = EmbeddingModelBuilder::new("sentence-transformers/all-MiniLM-L6-v2")
                        .build()
                        .await
                        .ok(); // Fallback to None if embedding model fails to load

                    Ok((Backend::MLX { 
                        model: std::sync::Arc::new(mlx_model), 
                        _ctx_limit: ctx_limit,
                        embedder: embed_model.map(std::sync::Arc::new)
                    }, model))
                }
                #[cfg(not(target_os = "macos"))]
                {
                    Ok((Backend::Ollama(Ollama::default()), model))
                }
            }
            AgentMode::Bridge => {
                let provider = crate::ai_bridge::ModelProvider::Ollama { 
                    base_url: "http://127.0.0.1:11434".to_string() 
                };
                let bridge = crate::ai_bridge::TempestAiBridge::new(provider, model.clone())?;
                Ok((Backend::Bridge(bridge), model))
            }
        }
    }

    pub async fn stream_chat(
        &self,
        model: String,
        history: Vec<ChatMessage>,
        sampling: SamplingConfig,
        event_tx: Arc<parking_lot::Mutex<Option<tokio::sync::mpsc::Sender<AgentEvent>>>>,
        stop: std::sync::Arc<std::sync::atomic::AtomicBool>,
        _system_prompt: String,
        on_tool_call: Option<tokio::sync::mpsc::UnboundedSender<ollama_rs::generation::tools::ToolCall>>,
        tool_registry: Option<Vec<ollama_rs::generation::tools::ToolInfo>>,
    ) -> Result<InferenceOutput> {
        let mut full_content = String::new();
        let mut reasoning_content = String::new();
        let mut native_tool_calls = Vec::new();

        let options = ModelOptions::default()
            .num_ctx(sampling.context_size)
            .num_predict(8192)
            .temperature(sampling.temperature)
            .repeat_penalty(sampling.repeat_penalty)
            .top_k(40)
            .top_p(sampling.top_p);

        match self {
            Backend::Bridge(bridge) => {
                use ai::chat_completions::ChatCompletionMessage;
                let ai_messages: Vec<ChatCompletionMessage> = history.iter().map(|m| {
                    match m.role {
                        MessageRole::System => ChatCompletionMessage::System(m.content.clone().into()),
                        MessageRole::User => ChatCompletionMessage::User(m.content.clone().into()),
                        MessageRole::Assistant => ChatCompletionMessage::Assistant(m.content.clone().into()),
                        _ => ChatCompletionMessage::User(m.content.clone().into()),
                    }
                }).collect();

                let mut stream = bridge.stream_chat(ai_messages).await?;
                while let Some(chunk_res) = stream.next().await {
                    if stop.load(std::sync::atomic::Ordering::Relaxed) { break; }
                    let chunk = chunk_res.map_err(|e| miette!("Bridge Stream Error: {}", e))?;
                    if let Some(choice) = chunk.choices.first() {
                        if let Some(ref token) = choice.delta.content {
                            full_content.push_str(token);
                            if let Some(tx) = event_tx.lock().clone() {
                                let _ = tx.try_send(AgentEvent::StreamToken(token.clone()));
                            }
                        }
                    }
                }
            }
            Backend::Ollama(ollama) => {
                let is_r1 = model.to_lowercase().contains("r1") || model.to_lowercase().contains("deepseek");
                let tx = event_tx.lock().clone();
                if let Some(tx) = tx {
                    let _ = tx.send(AgentEvent::Thinking(Some("Thinking...".to_string()))).await;
                }

                let mut stream = if is_r1 {
                    // --- 🧠 DEEPSEEK-R1 MANUAL TEMPLATE (OLLAMA) ---
                    let raw_prompt = build_deepseek_r1_prompt(&history);
                    let request = ollama_rs::generation::completion::request::GenerationRequest::new(model, raw_prompt)
                        .options(options);
                    
                    let mut s = None;
                    let mut last_err = None;
                    for _attempt in 1..=3 {
                        match ollama.generate_stream(request.clone()).await {
                            Ok(res) => { s = Some(res); break; }
                            Err(e) => {
                                last_err = Some(e);
                                tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
                            }
                        }
                    }
                    let s = s.ok_or_else(|| miette!("Ollama raw stream failed: {:?}", last_err))?;
                    
                    // Wrap the generation stream to match the chat stream interface
                    Box::pin(s.map(|res| {
                        let chunks = res.map_err(|_| ())?;
                        let chunk = chunks.first().ok_or(())?;
                        Ok(ollama_rs::generation::chat::ChatMessageResponse {
                            model: "".to_string(),
                            created_at: "".to_string(),
                            message: ChatMessage {
                                role: MessageRole::Assistant,
                                content: chunk.response.clone(),
                                images: None,
                                tool_calls: Vec::new(),
                                thinking: None,
                            },
                            done: chunk.done,
                            final_data: None,
                            logprobs: None,
                        })
                    })) as std::pin::Pin<Box<dyn futures::Stream<Item = Result<ollama_rs::generation::chat::ChatMessageResponse, ()>> + Send>>
                } else {
                    let mut request = ChatMessageRequest::new(model, history).options(options);
                    if let Some(registry) = tool_registry {
                        request = request.tools(registry);
                    }

                    let mut s = None;
                    for _ in 1..=3 {
                        if let Ok(res) = ollama.send_chat_messages_stream(request.clone()).await {
                            s = Some(res);
                            break;
                        }
                        tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
                    }
                    let s = s.ok_or_else(|| miette!("Ollama chat stream failed"))?;
                    Box::pin(s.map(|res| res.map_err(|_| ()))) as std::pin::Pin<Box<dyn futures::Stream<Item = Result<ollama_rs::generation::chat::ChatMessageResponse, ()>> + Send>>
                };

                let mut is_thinking = false;
                let mut first_token = true;
                let mut last_segments: Vec<String> = Vec::new();
                let mut tag_residue = String::new();
                let mut in_thought_block = false;

                let mut token_count = 0;
                let start_time = std::time::Instant::now();

                while let Some(res) = stream.next().await {
                    token_count += 1;
                    let elapsed = start_time.elapsed().as_secs_f64();
                    if elapsed > 0.1 {
                        let tps = (token_count as f64 / elapsed) as u64;
                        if let Some(tx) = event_tx.lock().clone() {
                            let _ = tx.try_send(AgentEvent::TelemetryMetrics { cpu: None, gpu: None, tps: Some(tps) });
                        }
                    }

                    if stop.load(std::sync::atomic::Ordering::Relaxed) { break; }
                    let chunk = res.map_err(|_| miette!("Ollama stream error"))?;
                    
                    let mut got_native_thinking = false;
                    let mut received_any_token = false;
                    // Handle native thinking field from Ollama (DeepSeek R1)
                    if let Some(thinking) = &chunk.message.thinking {
                        if !thinking.is_empty() {
                            got_native_thinking = true;
                            received_any_token = true;
                            reasoning_content.push_str(thinking);
                            if let Some(tx) = event_tx.lock().clone() {
                                let _ = tx.try_send(AgentEvent::ReasoningToken(thinking.to_string()));
                            }
                        }
                    }

                    let mut text = tag_residue.clone();
                    text.push_str(&chunk.message.content);
                    tag_residue.clear();
                    
                    // --- 🧠 STREAMING REASONING EXTRACTION (Cross-Chunk Robust) ---
                    let mut current_pos = 0;
                    while current_pos < text.len() {
                        if !is_thinking && !in_thought_block {
                            // Detect implicit thinking at the absolute start of response (skipping whitespace)
                            if first_token && !text.trim().is_empty() {
                                let trimmed_start = text.trim_start();
                                let lower = trimmed_start.to_lowercase();
                                let implicit_phrases = [
                                    "alright", "okay", "so,", "hmm", "thinking", "let me", "sure", 
                                    "certainly", "absolutely", "i'll", "let's", "i need to", "first,",
                                    "alright,", "okay,"
                                ];
                                if implicit_phrases.iter().any(|&p| lower.starts_with(p)) {
                                    in_thought_block = true;
                                    // Signal reasoning start immediately
                                    if let Some(tx) = event_tx.lock().clone() {
                                        let _ = tx.try_send(AgentEvent::ReasoningToken("".to_string()));
                                    }
                                }
                            }

                            if !in_thought_block {
                                if let Some(start_idx) = text[current_pos..].find("<think>") {
                                    // Content before <think>
                                    let before = &text[current_pos..current_pos + start_idx];
                                    if !before.is_empty() {
                                        full_content.push_str(before);
                                        if let Some(tx) = event_tx.lock().clone() {
                                            let _ = tx.try_send(AgentEvent::StreamToken(before.to_string()));
                                        }
                                    }
                                    is_thinking = true;
                                    current_pos += start_idx + 7; // Skip "<think>"
                                    continue;
                                } else if let Some(found_pos) = {
                                    let upper = text[current_pos..].to_uppercase();
                                    ["THOUGHT:", "PLAN:", "REASONING:", "ANALYSIS:"].iter()
                                        .filter_map(|&marker| upper.find(marker).map(|idx| (idx, marker.len())))
                                        .min_by_key(|&(idx, _)| idx)
                                } {
                                    let (found_idx, marker_len) = found_pos;
                                    let sub = &text[current_pos..];
                                    
                                    // Check for optional leading asterisks (e.g., **THOUGHT:)
                                    let mut start_idx = found_idx;
                                    while start_idx > 0 && sub.as_bytes()[start_idx - 1] == b'*' {
                                        start_idx -= 1;
                                    }

                                    let before = &sub[..start_idx];
                                    // Content before marker (unless it's just filler we already handled or want to ignore)
                                    if !before.trim().is_empty() {
                                        if let Some(tx) = event_tx.lock().clone() {
                                            let _ = tx.try_send(AgentEvent::StreamToken(before.to_string()));
                                        }
                                        full_content.push_str(before);
                                    }

                                    in_thought_block = true;
                                    
                                    // Signal reasoning start immediately
                                    if let Some(tx) = event_tx.lock().clone() {
                                        let _ = tx.try_send(AgentEvent::ReasoningToken("".to_string()));
                                    }

                                    // Move current_pos past marker and any trailing colon/asterisks/spaces
                                    let mut end_idx = found_idx + marker_len;
                                    while end_idx < sub.len() && (sub.as_bytes()[end_idx] == b':' || sub.as_bytes()[end_idx] == b'*' || sub.as_bytes()[end_idx] == b' ') {
                                        end_idx += 1;
                                    }
                                    current_pos += end_idx;
                                    continue;
                                }
                            }
                        }

                        if is_thinking {
                            if let Some(end_idx) = text[current_pos..].find("</think>") {
                                // Reasoning before </think>
                                let reasoning = &text[current_pos..current_pos + end_idx];
                                if !got_native_thinking {
                                    reasoning_content.push_str(reasoning);
                                    if let Some(tx) = event_tx.lock().clone() {
                                        let _ = tx.try_send(AgentEvent::ReasoningToken(reasoning.to_string()));
                                    }
                                }
                                is_thinking = false;
                                current_pos += end_idx + 8; // Skip "</think>"
                            } else if let Some(last_lt) = text[current_pos..].rfind('<') {
                                // Potential partial end tag
                                let pot_tag = &text[current_pos + last_lt..];
                                if "</think>".starts_with(pot_tag) {
                                    let before = &text[current_pos..current_pos + last_lt];
                                    if !before.is_empty() && !got_native_thinking {
                                        reasoning_content.push_str(before);
                                        if let Some(tx) = event_tx.lock().clone() {
                                            let _ = tx.try_send(AgentEvent::ReasoningToken(before.to_string()));
                                        }
                                    }
                                    tag_residue = pot_tag.to_string();
                                    break;
                                } else {
                                    let content = &text[current_pos..];
                                    if !got_native_thinking {
                                        reasoning_content.push_str(content);
                                        if let Some(tx) = event_tx.lock().clone() {
                                            let _ = tx.try_send(AgentEvent::ReasoningToken(content.to_string()));
                                        }
                                    }
                                    break;
                                }
                            } else {
                                let remaining = &text[current_pos..];
                                if !got_native_thinking {
                                    reasoning_content.push_str(remaining);
                                    if let Some(tx) = event_tx.lock().clone() {
                                        let _ = tx.try_send(AgentEvent::ReasoningToken(remaining.to_string()));
                                    }
                                }
                                break;
                            }
                        } else if in_thought_block {
                            // If we're in a THOUGHT: block, we look for the first JSON block or DONE: to end it
                            if let Some(json_idx) = text[current_pos..].find("```json") {
                                let reasoning = &text[current_pos..current_pos + json_idx];
                                reasoning_content.push_str(reasoning);
                                if let Some(tx) = event_tx.lock().clone() {
                                    let _ = tx.try_send(AgentEvent::ReasoningToken(reasoning.to_string()));
                                }
                                in_thought_block = false;
                                current_pos += json_idx; // Don't skip ```json, it belongs to full_content
                            } else if let Some(done_idx) = text[current_pos..].find("DONE:") {
                                let reasoning = &text[current_pos..current_pos + done_idx];
                                reasoning_content.push_str(reasoning);
                                if let Some(tx) = event_tx.lock().clone() {
                                    let _ = tx.try_send(AgentEvent::ReasoningToken(reasoning.to_string()));
                                }
                                in_thought_block = false;
                                current_pos += done_idx; // Don't skip DONE:
                            } else if let Some(newline_idx) = text[current_pos..].find("\n\n") {
                                // Heuristic: If we see a double newline and then a transition phrase, end thoughts
                                let after_nl = &text[current_pos + newline_idx + 2..];
                                let lower = after_nl.to_lowercase();
                                let transitions = [
                                    "i will", "i'll", "i'm going to", "now", "starting", "let's begin",
                                    "here is", "i have", "first,"
                                ];
                                
                                if transitions.iter().any(|&t| lower.starts_with(t)) {
                                    let reasoning = &text[current_pos..current_pos + newline_idx];
                                    reasoning_content.push_str(reasoning);
                                    if let Some(tx) = event_tx.lock().clone() {
                                        let _ = tx.try_send(AgentEvent::ReasoningToken(reasoning.to_string()));
                                    }
                                    in_thought_block = false;
                                    current_pos += newline_idx;
                                } else {
                                    let reasoning = &text[current_pos..current_pos + newline_idx + 2];
                                    reasoning_content.push_str(reasoning);
                                    if let Some(tx) = event_tx.lock().clone() {
                                        let _ = tx.try_send(AgentEvent::ReasoningToken(reasoning.to_string()));
                                    }
                                    current_pos += newline_idx + 2;
                                }
                            } else {
                                let remaining = &text[current_pos..];
                                reasoning_content.push_str(remaining);
                                if let Some(tx) = event_tx.lock().clone() {
                                    let _ = tx.try_send(AgentEvent::ReasoningToken(remaining.to_string()));
                                }
                                break;
                            }
                        }
                        if !text[current_pos..].is_empty() {
                             received_any_token = true;
                        }
                    }

                    if first_token && received_any_token {
                        first_token = false;
                        if let Some(tx) = event_tx.lock().clone() {
                            let _ = tx.try_send(AgentEvent::Thinking(None));
                            let _ = tx.try_send(AgentEvent::SubagentStatus(None));
                        }
                    }


                    if !chunk.message.tool_calls.is_empty() {
                        for call in chunk.message.tool_calls {
                            if let Some(ref tx) = on_tool_call {
                                let _ = tx.send(call.clone());
                            }
                            native_tool_calls.push(call);
                        }
                    }
                    // --- 🛡️ REPETITION SENTINEL ---
                    let trimmed = chunk.message.content.trim();
                    if !trimmed.is_empty() && trimmed.len() > 3 {
                        last_segments.push(trimmed.to_string());
                        if last_segments.len() > 15 { last_segments.remove(0); }
                        if last_segments.iter().filter(|&s| s == trimmed).count() >= 8 {
                            let warning = "\n\n⚠️ [REPETITION SENTINEL]: Breaking loop to prevent hallucination plateau.";
                            full_content.push_str(warning);
                            let tx = event_tx.lock().clone();
                            if let Some(tx) = tx {
                                let _ = tx.send(AgentEvent::StreamToken(warning.to_string())).await;
                            }
                            break; 
                        }
                    }
                }
            }
            #[cfg(target_os = "macos")]
            Backend::MLX { model: mistral_model, .. } => {
                let on_tool_call = on_tool_call;
                let tx_opt = event_tx.lock().clone();
                if let Some(tx) = tx_opt {
                    let _ = tx.try_send(AgentEvent::Thinking(Some("Thinking...".to_string())));
                    let est_tokens = crate::context_manager::estimate_tokens(&history);
                    let _ = tx.try_send(AgentEvent::ContextStatus { used: est_tokens, total: sampling.context_size });
                    let _ = tx.try_send(AgentEvent::SubagentStatus(Some(format!("⚡ MLX Engine: Dispatching request ({} history tokens)...", est_tokens))));
                }

                let mut request_builder = RequestBuilder::new();
                let mut system_content = Vec::new();
                let mut other_messages: Vec<ollama_rs::generation::chat::ChatMessage> = Vec::new();
                
                for msg in &history {
                    if msg.role == MessageRole::System {
                        system_content.push(msg.content.clone());
                    } else {
                        // --- 🛡️ STRICT ROLE ALTERNATION (COMPRESSOR) ---
                        // Ministral 8B panics if roles do not strictly alternate (e.g., User -> User).
                        // If the previous message has the same role, merge them.
                        if let Some(last) = other_messages.last_mut() {
                            if last.role == msg.role {
                                last.content.push_str("\n\n");
                                last.content.push_str(&msg.content);
                                continue;
                            }
                        }
                        other_messages.push(msg.clone());
                    }
                }
                
                let model_lower = model.to_lowercase();
                let is_reasoning_model = model_lower.contains("deepseek") || model_lower.contains("r1");

                let merged_system = system_content.join("\n\n");

                // === CRITICAL FIRST-TURN SAFETY GUARD (fixes silent start with Ministral 8B) ===
                if other_messages.is_empty() || other_messages.iter().all(|m| m.role != MessageRole::User) {
                    // Force at least one User message — this is the most common cause of "stream never starts"
                    let user_content = history.last()
                        .filter(|m| m.role == MessageRole::User)
                        .map(|m| m.content.clone())
                        .unwrap_or_else(|| "Hello, please begin.".to_string());

                    if !merged_system.is_empty() {
                        request_builder = request_builder.add_message(TextMessageRole::System, merged_system);
                    }
                    request_builder = request_builder.add_message(TextMessageRole::User, user_content);
                } else {
                    // Normal path
                    if !merged_system.is_empty() {
                        request_builder = request_builder.add_message(TextMessageRole::System, merged_system);
                    }

                    let mut total_tool_calls = 0;
                    let mut total_tool_results = 0;

                    for msg in other_messages {
                        match msg.role {
                            MessageRole::User => {
                                request_builder = request_builder.add_message(TextMessageRole::User, msg.content);
                            }
                            MessageRole::Assistant => {
                                if !msg.tool_calls.is_empty() {
                                    let mut mistral_calls = Vec::new();
                                    for (i, c) in msg.tool_calls.iter().enumerate() {
                                        let global_idx = total_tool_calls + i;
                                        mistral_calls.push(mistralrs::ToolCallResponse {
                                            index: i,
                                            id: format!("call_{}", global_idx),
                                            tp: mistralrs::ToolCallType::Function,
                                            function: mistralrs::CalledFunction {
                                                name: c.function.name.clone(),
                                                arguments: c.function.arguments.to_string(),
                                            },
                                        });
                                    }
                                    total_tool_calls += msg.tool_calls.len();
                                    
                                    request_builder = request_builder.add_message_with_tool_call(
                                        TextMessageRole::Assistant,
                                        msg.content,
                                        mistral_calls
                                    );
                                } else {
                                    request_builder = request_builder.add_message(TextMessageRole::Assistant, msg.content);
                                }
                            }
                            MessageRole::Tool => {
                                let call_id = format!("call_{}", total_tool_results);
                                request_builder = request_builder.add_tool_message(msg.content, call_id);
                                total_tool_results += 1;
                            }
                            _ => {
                                request_builder = request_builder.add_message(TextMessageRole::User, msg.content);
                            }
                        }
                    }
                }

                if is_reasoning_model {
                    request_builder = request_builder.enable_thinking(true);
                }

                // --- 🛠️ NATIVE TOOL CALLING (MLX) ---
                if let Some(registry) = tool_registry {
                    let mut mistral_tools = Vec::new();
                    for tool in registry {
                        // Convert Ollama ToolInfo to mistralrs Tool
                        let mistral_tool = Tool {
                            tp: ToolType::Function,
                            function: Function {
                                name: tool.function.name.clone(),
                                description: Some(tool.function.description.clone()),
                                parameters: {
                                    // Extract properties from Ollama Schema
                                    let schema_val = serde_json::to_value(&tool.function.parameters).unwrap_or(serde_json::json!({}));
                                    if let Some(props) = schema_val.get("properties").and_then(|p| p.as_object()) {
                                        let mut map = std::collections::HashMap::new();
                                        for (k, v) in props {
                                            map.insert(k.clone(), v.clone());
                                        }
                                        Some(map)
                                    } else {
                                        None
                                    }
                                },
                            },
                        };
                        mistral_tools.push(mistral_tool);
                    }
                        if !mistral_tools.is_empty() {
                            request_builder = request_builder.set_tools(mistral_tools);
                            request_builder = request_builder.set_tool_choice(ToolChoice::Auto);
                        }
                }

                // Apply backend-aware sampling parameters to MLX via direct SamplingParams configuration
                let mut sampling_params = SamplingParams::deterministic();
                sampling_params.temperature = Some(sampling.temperature.into());
                sampling_params.top_p = Some(sampling.top_p.into());
                sampling_params.top_k = Some(40);
                sampling_params.repetition_penalty = Some(sampling.repeat_penalty as f32);
                sampling_params.max_len = Some(8192);
                request_builder = request_builder.set_sampling(sampling_params.clone());

                let tx_opt = event_tx.lock().clone();
                if let Some(tx) = tx_opt.as_ref() {
                    let _ = tx.try_send(AgentEvent::SubagentStatus(Some("⚡ MLX Engine: Dispatching Request to Metal...".to_string())));
                }

                let mut stream = if is_reasoning_model {
                    // --- 🧠 DEEPSEEK-R1 MANUAL TEMPLATE (MLX) ---
                    let raw_prompt = build_deepseek_r1_prompt(&history);
                    let req = RequestBuilder::new()
                        .add_message(TextMessageRole::User, raw_prompt)
                        .set_sampling(sampling_params);
                    
                    mistral_model.stream_chat_request(req).await.into_diagnostic()?
                } else {
                    mistral_model.stream_chat_request(request_builder).await.into_diagnostic()?
                };
                
                if let Some(tx) = tx_opt.as_ref() {
                    let _ = tx.try_send(AgentEvent::SubagentStatus(Some("⚡ MLX Engine: Stream established, waiting for first token...".to_string())));
                }

                // --- FIRST-TURN METAL WARMUP FIX ---
                if history.len() >= 5 {
                    tokio::time::sleep(tokio::time::Duration::from_millis(150)).await;
                }

                let mut first_token = true;
                let mut is_thinking = is_reasoning_model; 
                let mut tag_residue = String::new();
                let mut in_thought_block = is_reasoning_model;

                if is_thinking {
                    if let Some(tx) = event_tx.lock().clone() {
                        let _ = tx.try_send(AgentEvent::ReasoningToken("".to_string()));
                        let _ = tx.try_send(AgentEvent::Thinking(None));
                    }
                }
                let mut last_segments: Vec<String> = Vec::new();
                let mut token_count = 0;
                let start_time = std::time::Instant::now();
                
                while let Some(response) = stream.next().await {
                    token_count += 1;
                    let elapsed = start_time.elapsed().as_secs_f64();
                    if elapsed > 0.1 {
                        let tps = (token_count as f64 / elapsed) as u64;
                        if let Some(tx) = event_tx.lock().clone() {
                            let _ = tx.try_send(AgentEvent::TelemetryMetrics { cpu: None, gpu: None, tps: Some(tps) });
                        }
                    }

                    if stop.load(std::sync::atomic::Ordering::Relaxed) { break; }

                    match response {
                        MistralResponse::Chunk(chunk) => {
                            // Forward tool calls if present (future-proofing)
                            if let Some(tool_calls) = &chunk.choices[0].delta.tool_calls {
                                // If we are in a thought block, a native tool call should end it.
                                in_thought_block = false;
                                is_thinking = false;
                                if let Some(tx) = event_tx.lock().clone() {
                                    let _ = tx.try_send(AgentEvent::Thinking(None));
                                }

                                for call in tool_calls {
                                    // Map mistralrs tool call to ollama_rs tool call
                                    let mapped_call = ollama_rs::generation::tools::ToolCall {
                                        function: ollama_rs::generation::tools::ToolCallFunction {
                                            name: call.function.name.clone(),
                                            arguments: serde_json::from_str(&call.function.arguments).unwrap_or(serde_json::json!({})),
                                        }
                                    };
                                    if let Some(ref tx) = on_tool_call {
                                        let _ = tx.send(mapped_call.clone());
                                    }
                                    native_tool_calls.push(mapped_call);
                                }
                            }

                            if let Some(reasoning) = &chunk.choices[0].delta.reasoning_content {
                                if !reasoning.is_empty() {
                                    reasoning_content.push_str(reasoning);
                                    if let Some(tx) = event_tx.lock().clone() {
                                        let _ = tx.try_send(AgentEvent::ReasoningToken(reasoning.to_string()));
                                    }
                                    if first_token {
                                        first_token = false;
                                        if let Some(tx) = event_tx.lock().clone() {
                                            let _ = tx.try_send(AgentEvent::Thinking(None));
                                            let _ = tx.try_send(AgentEvent::SubagentStatus(None));
                                        }
                                    }
                                }
                            }

                            if let Some(content) = &chunk.choices[0].delta.content {
                                let mut text = tag_residue.clone();
                                text.push_str(content);
                                tag_residue.clear();

                                let mut current_pos = 0;
                                
                                if first_token && !text.trim().is_empty() {
                                    let lower = text.trim().to_lowercase();
                                    let implicit_phrases = [
                                        "alright", "okay", "so,", "hmm", "thinking", "let me", "sure", 
                                        "certainly", "absolutely", "i'll", "let's", "i need to", "first,",
                                        "alright,", "okay,"
                                    ];
                                    if implicit_phrases.iter().any(|&p| lower.starts_with(p)) {
                                        in_thought_block = true;
                                        if let Some(tx) = event_tx.lock().clone() {
                                            let _ = tx.try_send(AgentEvent::ReasoningToken("".to_string()));
                                            let _ = tx.try_send(AgentEvent::SubagentStatus(Some("⚡ MLX Engine: Thinking...".to_string())));
                                        }
                                    }
                                    
                                    if text.len() >= 10 || !text.starts_with("<") {
                                        first_token = false;
                                        if !text.contains("<think>") {
                                            if let Some(tx) = event_tx.lock().clone() {
                                                let _ = tx.try_send(AgentEvent::Thinking(None));
                                                let _ = tx.try_send(AgentEvent::SubagentStatus(None));
                                            }
                                        }
                                    }
                                }
                                    
                                while current_pos < text.len() {
                                    if !is_thinking && !in_thought_block {
                                        if let Some(start_idx) = text[current_pos..].find("<think>") {
                                            let before = &text[current_pos..current_pos + start_idx];
                                            if !before.is_empty() {
                                                full_content.push_str(before);
                                                if let Some(tx) = event_tx.lock().clone() {
                                                    let _ = tx.try_send(AgentEvent::StreamToken(before.to_string()));
                                                }
                                            }
                                            is_thinking = true;
                                            current_pos += start_idx + 7;
                                        } else if let Some(found_pos) = {
                                            let upper = text[current_pos..].to_uppercase();
                                            ["THOUGHT:", "PLAN:", "REASONING:", "ANALYSIS:"].iter()
                                                .filter_map(|&marker| upper.find(marker).map(|idx| (idx, marker.len())))
                                                .min_by_key(|&(idx, _)| idx)
                                        } {
                                            let (found_idx, marker_len) = found_pos;
                                            let sub = &text[current_pos..];
                                            let mut is_implicit_thinking = false;
                                            
                                            let mut start_idx = found_idx;
                                            while start_idx > 0 && sub.as_bytes()[start_idx - 1] == b'*' {
                                                start_idx -= 1;
                                            }

                                            let before = &sub[..start_idx];
                                            if !before.is_empty() {
                                                // SMOTHER PREAMBLE & DETECT IMPLICIT THINKING (MLX)
                                                let trimmed = before.trim();
                                                let lower = trimmed.to_lowercase();
                                                
                                                let is_filler = ["sure", "okay", "i can", "here is", "i will", "certainly", "alright"].iter()
                                                    .any(|&filler| lower.starts_with(filler) || lower == filler);
                                                
                                                is_implicit_thinking = first_token && (
                                                    lower.starts_with("alright, so") || 
                                                    lower.starts_with("okay, so") || 
                                                    lower.starts_with("so, the user") ||
                                                    lower.starts_with("hmm")
                                                );

                                                if is_implicit_thinking {
                                                    in_thought_block = true;
                                                    if let Some(tx) = event_tx.lock().clone() {
                                                        let _ = tx.try_send(AgentEvent::ReasoningToken("".to_string()));
                                                    }
                                                } else if !is_filler || trimmed.len() > 40 {
                                                    if let Some(tx) = event_tx.lock().clone() {
                                                        let _ = tx.try_send(AgentEvent::StreamToken(before.to_string()));
                                                    }
                                                    full_content.push_str(before);
                                                }
                                            }

                                            if !is_implicit_thinking {
                                                in_thought_block = true;
                                                if let Some(tx) = event_tx.lock().clone() {
                                                    let _ = tx.try_send(AgentEvent::ReasoningToken("".to_string()));
                                                }
                                            }

                                            let mut end_idx = found_idx + marker_len;
                                            while end_idx < sub.len() && (sub.as_bytes()[end_idx] == b':' || sub.as_bytes()[end_idx] == b'*' || sub.as_bytes()[end_idx] == b' ') {
                                                end_idx += 1;
                                            }
                                            current_pos += end_idx;
                                        } else if let Some(found_idx) = {
                                            // Check for partial tags (<think>) or partial markers (THOUGHT:)
                                            let upper = text[current_pos..].to_uppercase();
                                            let markers = ["THOUGHT:", "PLAN:", "REASONING:", "ANALYSIS:"];
                                            let mut best_partial = None;
                                            
                                            // 1. Check for partial <think>
                                            if let Some(last_lt) = text[current_pos..].rfind('<') {
                                                let pot_tag = &text[current_pos + last_lt..];
                                                if "<think>".starts_with(pot_tag) {
                                                    best_partial = Some((last_lt, pot_tag.len()));
                                                }
                                            }

                                            // 2. Check for partial markers
                                            for marker in markers {
                                                for i in 1..marker.len() {
                                                    let partial = &marker[..i];
                                                    if upper.ends_with(partial) {
                                                        let pos = upper.len() - i;
                                                        if best_partial.map_or(true, |(p, _)| pos < p) {
                                                            best_partial = Some((pos, i));
                                                        }
                                                    }
                                                }
                                            }
                                            best_partial
                                        } {
                                            let (found_idx_rel, _len) = found_idx;
                                            let before = &text[current_pos..current_pos + found_idx_rel];
                                            if !before.is_empty() {
                                                full_content.push_str(before);
                                                if let Some(tx) = event_tx.lock().clone() {
                                                    let _ = tx.try_send(AgentEvent::StreamToken(before.to_string()));
                                                }
                                            }
                                            tag_residue = text[current_pos + found_idx_rel..].to_string();
                                            break;
                                        } else {
                                            let remaining = &text[current_pos..];
                                            full_content.push_str(remaining);
                                            if let Some(tx) = event_tx.lock().clone() {
                                                let _ = tx.try_send(AgentEvent::StreamToken(remaining.to_string()));
                                            }
                                            break;
                                        }
                                    } else if is_thinking {
                                        if let Some(end_idx) = text[current_pos..].find("</think>") {
                                            let reasoning = &text[current_pos..current_pos + end_idx];
                                            reasoning_content.push_str(reasoning);
                                            if let Some(tx) = event_tx.lock().clone() {
                                                let _ = tx.try_send(AgentEvent::ReasoningToken(reasoning.to_string()));
                                            }
                                            is_thinking = false;
                                            current_pos += end_idx + 8;
                                            
                                            if first_token {
                                                first_token = false;
                                                if let Some(tx) = event_tx.lock().clone() {
                                                    let _ = tx.try_send(AgentEvent::Thinking(None));
                                                    let _ = tx.try_send(AgentEvent::SubagentStatus(None));
                                                }
                                            }
                                        } else if let Some(json_idx) = text[current_pos..].find("```json") {
                                            // FAILSAFE: Implicit end of explicit thinking block
                                            let reasoning = &text[current_pos..current_pos + json_idx];
                                            reasoning_content.push_str(reasoning);
                                            if let Some(tx) = event_tx.lock().clone() {
                                                let _ = tx.try_send(AgentEvent::ReasoningToken(reasoning.to_string()));
                                            }
                                            is_thinking = false;
                                            current_pos += json_idx;
                                            
                                            if first_token {
                                                first_token = false;
                                                if let Some(tx) = event_tx.lock().clone() {
                                                    let _ = tx.try_send(AgentEvent::Thinking(None));
                                                    let _ = tx.try_send(AgentEvent::SubagentStatus(None));
                                                }
                                            }
                                        } else if let Some(last_lt) = text[current_pos..].rfind('<') {
                                            let pot_tag = &text[current_pos + last_lt..];
                                            if "</think>".starts_with(pot_tag) {
                                                let before = &text[current_pos..current_pos + last_lt];
                                                if !before.is_empty() {
                                                    reasoning_content.push_str(before);
                                                    if let Some(tx) = event_tx.lock().clone() {
                                                        let _ = tx.try_send(AgentEvent::ReasoningToken(before.to_string()));
                                                    }
                                                }
                                                tag_residue = pot_tag.to_string();
                                                break;
                                            } else {
                                                let remaining = &text[current_pos..];
                                                reasoning_content.push_str(remaining);
                                                if let Some(tx) = event_tx.lock().clone() {
                                                    let _ = tx.try_send(AgentEvent::ReasoningToken(remaining.to_string()));
                                                }
                                                break;
                                            }
                                        } else {
                                            let remaining = &text[current_pos..];
                                            reasoning_content.push_str(remaining);
                                            if let Some(tx) = event_tx.lock().clone() {
                                                let _ = tx.try_send(AgentEvent::ReasoningToken(remaining.to_string()));
                                            }
                                            break;
                                        }
                                    } else if in_thought_block {
                                        if let Some(json_idx) = text[current_pos..].find("```json") {
                                            let reasoning = &text[current_pos..current_pos + json_idx];
                                            reasoning_content.push_str(reasoning);
                                            if let Some(tx) = event_tx.lock().clone() {
                                                let _ = tx.try_send(AgentEvent::ReasoningToken(reasoning.to_string()));
                                            }
                                            in_thought_block = false;
                                            current_pos += json_idx;
                                        } else if let Some(done_idx) = text[current_pos..].find("DONE:") {
                                            let reasoning = &text[current_pos..current_pos + done_idx];
                                            reasoning_content.push_str(reasoning);
                                            if let Some(tx) = event_tx.lock().clone() {
                                                let _ = tx.try_send(AgentEvent::ReasoningToken(reasoning.to_string()));
                                            }
                                            in_thought_block = false;
                                            current_pos += done_idx;
                                        } else if let Some(newline_idx) = text[current_pos..].find("\n\n") {
                                            // Heuristic: Transition from thought to message (MLX)
                                            let after_nl = &text[current_pos + newline_idx + 2..];
                                            let lower = after_nl.to_lowercase();
                                            let transitions = [
                                                "i will", "i'll", "i'm going to", "now", "starting", "let's begin",
                                                "here is", "i have", "first,"
                                            ];
                                            
                                            if transitions.iter().any(|&t| lower.starts_with(t)) {
                                                let reasoning = &text[current_pos..current_pos + newline_idx];
                                                reasoning_content.push_str(reasoning);
                                                if let Some(tx) = event_tx.lock().clone() {
                                                    let _ = tx.try_send(AgentEvent::ReasoningToken(reasoning.to_string()));
                                                    let _ = tx.try_send(AgentEvent::SubagentStatus(Some(format!("⚡ MLX Engine: Reasoning... ({} chars)", reasoning_content.len()))));
                                                }
                                                in_thought_block = false;
                                                current_pos += newline_idx;
                                            } else {
                                                let reasoning = &text[current_pos..current_pos + newline_idx + 2];
                                                reasoning_content.push_str(reasoning);
                                                if let Some(tx) = event_tx.lock().clone() {
                                                    let _ = tx.try_send(AgentEvent::ReasoningToken(reasoning.to_string()));
                                                    let _ = tx.try_send(AgentEvent::SubagentStatus(Some(format!("⚡ MLX Engine: Reasoning... ({} chars)", reasoning_content.len()))));
                                                }
                                                current_pos += newline_idx + 2;
                                            }
                                        } else {
                                            let remaining = &text[current_pos..];
                                            let mut save_residue = 0;
                                            
                                            // Check for partial ```json
                                            for i in 1..=7 {
                                                if remaining.ends_with(&"```json"[..i]) { save_residue = i; }
                                            }
                                            // Check for partial DONE:
                                            for i in 1..=5 {
                                                if remaining.ends_with(&"DONE:"[..i]) { save_residue = i; }
                                            }

                                            if save_residue > 0 {
                                                let safe_len = remaining.len() - save_residue;
                                                let safe_part = &remaining[..safe_len];
                                                reasoning_content.push_str(safe_part);
                                                if let Some(tx) = event_tx.lock().clone() {
                                                    let _ = tx.try_send(AgentEvent::ReasoningToken(safe_part.to_string()));
                                                    let _ = tx.try_send(AgentEvent::SubagentStatus(Some(format!("⚡ MLX Engine: Reasoning... ({} chars)", reasoning_content.len()))));
                                                }
                                                tag_residue = remaining[safe_len..].to_string();
                                            } else {
                                                reasoning_content.push_str(remaining);
                                                if let Some(tx) = event_tx.lock().clone() {
                                                    let _ = tx.try_send(AgentEvent::ReasoningToken(remaining.to_string()));
                                                    let _ = tx.try_send(AgentEvent::SubagentStatus(Some(format!("⚡ MLX Engine: Reasoning... ({} chars)", reasoning_content.len()))));
                                                }
                                            }
                                            break;
                                        }
                                    }
                                }

                                // --- 🛡️ DYNAMIC SENTINELS (MLX) ---
                                let trimmed = content.trim();
                                if !trimmed.is_empty() {
                                    // 1. REPETITION SENTINEL
                                    if trimmed.len() > 3 {
                                        last_segments.push(trimmed.to_string());
                                        if last_segments.len() > 15 { last_segments.remove(0); }
                                        if last_segments.iter().filter(|&s| s == trimmed).count() >= 8 {
                                            let warning = "\n\n⚠️ [REPETITION SENTINEL]: Breaking loop to prevent hallucination plateau.";
                                            full_content.push_str(warning);
                                            if let Some(tx) = event_tx.lock().clone() {
                                                let _ = tx.try_send(AgentEvent::StreamToken(warning.to_string()));
                                            }
                                            break;
                                        }
                                    }

                                    // 2. PASSIVITY SENTINEL
                                    let lower_trimmed = trimmed.to_lowercase();
                                    if lower_trimmed.contains("would you like me to") || 
                                       lower_trimmed.contains("shall i") ||
                                       lower_trimmed.contains("do you want me to") {
                                        let warning = "\n\n⚠️ [PASSIVITY SENTINEL]: Take action. Do not ask permission for the next logical step.";
                                        full_content.push_str(warning);
                                        if let Some(tx) = event_tx.lock().clone() {
                                            let _ = tx.try_send(AgentEvent::StreamToken(warning.to_string()));
                                        }
                                        // We don't break, just nudge
                                    }

                                    // 3. CHINESE LEAKAGE SENTINEL
                                    // Detect any CJK (Chinese, Japanese, Korean) characters
                                    if trimmed.chars().any(|c| (c >= '\u{4e00}' && c <= '\u{9fff}') || (c >= '\u{3400}' && c <= '\u{4dbf}')) {
                                        let warning = "\n\n⚠️ [LANGUAGE SENTINEL]: Respond in English only.";
                                        full_content.push_str(warning);
                                        if let Some(tx) = event_tx.lock().clone() {
                                            let _ = tx.try_send(AgentEvent::StreamToken(warning.to_string()));
                                        }
                                    }
                                }
                            }
                        }
                        MistralResponse::ModelError(e, _) => return Err(miette!("MLX Model Error: {}", e)),
                        _ => {}
                    }
                }
            }
        }

        // --- 🛡️ HARDENED TOOL EXTRACTION (llm-extract) ---
        // If native_tool_calls is empty but full_content has JSON blocks, parse them using
        // self-repairing extraction logic. Handles markdown, malformed JSON, and field typos.
        if native_tool_calls.is_empty() {
            let combined_content = format!("{}\n{}", reasoning_content, full_content);

            // Extract multiple tool calls using self-repairing JSON logic
            if let Ok(payloads) = llm_extract::extract_all::<ToolCallPayload>(&combined_content) {
                for payload in payloads {
                    native_tool_calls.push(ollama_rs::generation::tools::ToolCall {
                        function: ollama_rs::generation::tools::ToolCallFunction {
                            name: payload.name,
                            arguments: payload.arguments,
                        }
                    });
                }
            } else {
                // --- 🛡️ FALLBACK: tool-parser (v1.2.0) with DeepSeekParser ---
                // If llm-extract fails, try specialized DeepSeekParser for multi-block recovery
                let parser = tool_parser::DeepSeekParser::new();
                if let Ok((_text, calls)) = parser.parse_complete(&combined_content).await {
                    for call in calls {
                        native_tool_calls.push(ollama_rs::generation::tools::ToolCall {
                            function: ollama_rs::generation::tools::ToolCallFunction {
                                name: call.function.name,
                                arguments: serde_json::from_str(&call.function.arguments).unwrap_or(serde_json::json!({})),
                            }
                        });
                    }
                }
            }

            // Enforce [ACTOR PROTOCOL]: Truncate content after first tool call if found
            if !native_tool_calls.is_empty() {
                if let Some(pos) = full_content.find('{') {
                    full_content.truncate(pos);
                }
            }
        }

        Ok(InferenceOutput {
            content: full_content,
            reasoning: reasoning_content,
            native_tool_calls,
        })
    }

    pub async fn shutdown(&self, model: String) {
        match self {
            Backend::Ollama(ollama) => {
                let options = ModelOptions::default()
                    .num_ctx(1)
                    .temperature(0.1);

                let request = GenerationRequest::new(model, "".to_string())
                    .options(options)
                    .keep_alive(KeepAlive::Until { time: 0, unit: TimeUnit::Seconds });

                let _ = tokio::time::timeout(
                    tokio::time::Duration::from_millis(200),
                    ollama.generate(request)
                ).await;
            }
            #[cfg(target_os = "macos")]
            Backend::MLX { .. } => {
                // MistralRs doesn't have a 'stop' command, but the OS will reclaim 
                // all model memory as soon as the main process returns.
                tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
            }
            Backend::Bridge(_) => {
                // Bridge clients handle their own cleanup
            }
        }
    }

    pub async fn generate_embeddings(&self, text: &str) -> Result<Vec<f32>> {
        match self {
            Backend::Ollama(ollama) => {
                let req = ollama_rs::generation::embeddings::request::GenerateEmbeddingsRequest::new(
                    "all-minilm".to_string(), 
                    text.to_string().into()
                );
                let res = ollama.generate_embeddings(req).await
                    .map_err(|e| miette!("Ollama embedding failed: {}", e))?;
                Ok(res.embeddings.first().cloned().unwrap_or_default())
            }
            #[cfg(target_os = "macos")]
            Backend::MLX { embedder, .. } => {
                if let Some(embed) = embedder {
                    let request = EmbeddingRequest::builder().add_prompt(format!("passage: {}", text));
                    let res = embed.generate_embeddings(request).await
                        .map_err(|e| miette!("MLX embedding failed: {}", e))?;
                    Ok(res.first().cloned().unwrap_or_default())
                } else {
                    Err(miette!("MLX Embedder not loaded"))
                }
            }
            Backend::Bridge(bridge) => {
                bridge.generate_embeddings(text.to_string()).await
            }
        }
    }
}

// --- 🌪️ RIG-CORE INTEGRATION (Orchestration Layer) ---

impl CompletionModel for Backend {
    type Response = String;
    type StreamingResponse = ();
    type Client = ();

    fn make(_client: &Self::Client, _model: impl Into<String>) -> Self {
        unimplemented!("Backend is usually created via Backend::new")
    }

    fn completion(
        &self,
        request: rig::completion::CompletionRequest,
    ) -> impl std::future::Future<Output = std::result::Result<rig::completion::CompletionResponse<Self::Response>, rig::completion::CompletionError>> + rig::wasm_compat::WasmCompatSend {
        let this = self.clone();
        async move {
            // Map rig::completion::CompletionRequest to our stream_chat parameters
            let prompt = request.chat_history.iter().last().map(|m| {
                match m {
                    rig::message::Message::System { content } => content.clone(),
                    rig::message::Message::User { content } => {
                        content.iter().find_map(|c| match c {
                            rig::message::UserContent::Text(t) => Some(t.text.clone()),
                            _ => None
                        }).unwrap_or_default()
                    }
                    rig::message::Message::Assistant { content, .. } => {
                        content.iter().find_map(|c| match c {
                            rig::message::AssistantContent::Text(t) => Some(t.text.clone()),
                            _ => None
                        }).unwrap_or_default()
                    }
                }
            }).unwrap_or_default();
            let history = vec![ChatMessage::new(MessageRole::User, prompt)];
            
            let sampling = SamplingConfig {
                temperature: request.temperature.unwrap_or(0.1) as f32,
                top_p: 0.9,
                repeat_penalty: 1.1,
                context_size: 8192,
            };
            
            let stop = Arc::new(std::sync::atomic::AtomicBool::new(false));
            let event_tx = Arc::new(parking_lot::Mutex::new(None));
            
            let output = this.stream_chat(
                request.model.unwrap_or_else(|| "default".to_string()),
                history,
                sampling,
                event_tx,
                stop,
                "".to_string(),
                None,
                None,
            ).await.map_err(|e| rig::completion::CompletionError::ProviderError(e.to_string()))?;

            Ok(rig::completion::CompletionResponse {
                choice: rig::OneOrMany::one(rig::message::AssistantContent::text(output.content)),
                usage: rig::completion::Usage::new(),
                raw_response: "".to_string(),
                message_id: None,
            })
        }
    }

    fn stream(
        &self,
        _request: rig::completion::CompletionRequest,
    ) -> impl std::future::Future<Output = std::result::Result<rig::streaming::StreamingCompletionResponse<Self::StreamingResponse>, rig::completion::CompletionError>> + rig::wasm_compat::WasmCompatSend {
        async move {
            Err(rig::completion::CompletionError::ProviderError("Streaming completion not yet implemented for rig-core wrapper".to_string()))
        }
    }
}

impl EmbeddingModel for Backend {
    const MAX_DOCUMENTS: usize = 100;
    type Client = ();

    fn make(_client: &Self::Client, _model: impl Into<String>, _dims: Option<usize>) -> Self {
        unimplemented!("Backend is usually created via Backend::new")
    }

    fn ndims(&self) -> usize {
        384 // MiniLM-L6-v2 dimensions
    }

    fn embed_texts(
        &self,
        texts: impl IntoIterator<Item = String> + rig::wasm_compat::WasmCompatSend,
    ) -> impl std::future::Future<Output = std::result::Result<Vec<rig::embeddings::Embedding>, rig::embeddings::EmbeddingError>> + rig::wasm_compat::WasmCompatSend {
        let texts_vec: Vec<String> = texts.into_iter().collect();
        let this = self.clone();
        async move {
            let mut results = Vec::new();
            for text in texts_vec {
                let emb = this.generate_embeddings(&text).await
                    .map_err(|e| rig::embeddings::EmbeddingError::ProviderError(e.to_string()))?;
                results.push(rig::embeddings::Embedding {
                    document: text,
                    vec: emb.into_iter().map(|f| f as f64).collect(),
                });
            }
            Ok(results)
        }
    }
}

fn build_deepseek_r1_prompt(history: &[ChatMessage]) -> String {
    let mut prompt = String::from("<｜begin of sentence｜>");
    let mut system_content = Vec::new();
    for msg in history {
        if msg.role == MessageRole::System {
            system_content.push(msg.content.clone());
        }
    }
    if !system_content.is_empty() {
        prompt.push_str(&system_content.join("\n\n"));
    }
    for msg in history {
        match msg.role {
            MessageRole::User => {
                prompt.push_str("<｜User｜>");
                prompt.push_str(&msg.content);
            }
            MessageRole::Assistant => {
                prompt.push_str("<｜Assistant｜>");
                if let Some(thinking) = &msg.thinking {
                    if !thinking.is_empty() {
                        prompt.push_str("<think>\n");
                        prompt.push_str(thinking);
                        prompt.push_str("\n</think>\n");
                    }
                }
                prompt.push_str(&msg.content);
            }
            MessageRole::Tool => {
                // For DeepSeek-R1 style, we inject tool results back as user-like context 
                // or specific block markers.
                prompt.push_str("\n\n[TOOL_RESULT]\n");
                prompt.push_str(&msg.content);
                prompt.push_str("\n");
            }
            MessageRole::System => {}
        }
    }
    
    // Force the model to start thinking for the NEW response
    // Re-forcing <think> to ensure the model maintains its reasoning identity.
    // The parser now correctly handles the transition to conversation after </think>.
    if !prompt.ends_with("<｜Assistant｜><think>\n") {
        if prompt.ends_with("<｜Assistant｜>") {
            prompt.push_str("<think>\n");
        } else {
            prompt.push_str("<｜Assistant｜><think>\n");
        }
    }
    prompt
}
