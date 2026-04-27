use miette::{Result, IntoDiagnostic, miette};
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
use regex::Regex;

#[cfg(feature = "mlx")]
use mistralrs::{
    GgufModelBuilder, Model, TextMessageRole, 
    Response as MistralResponse, RequestBuilder, SamplingParams,
    Tool, ToolChoice, Function, ToolType,
    PagedAttentionMetaBuilder, MemoryGpuConfig,
    EmbeddingModelBuilder, EmbeddingRequest
};
use crate::tui::AgentEvent;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum AgentMode {
    Ollama,
    MLX,
}

pub enum Backend {
    Ollama(Ollama),
    #[cfg(feature = "mlx")]
    MLX {
        model: Model,
        ctx_limit: usize,
        embedder: Option<Model>,
    },
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

impl Backend {
    pub fn mode(&self) -> AgentMode {
        match self {
            Backend::Ollama(_) => AgentMode::Ollama,
            #[cfg(feature = "mlx")]
            Backend::MLX { .. } => AgentMode::MLX,
        }
    }

    pub async fn new(mode: AgentMode, model: String, quant: String, event_tx: Arc<parking_lot::Mutex<Option<tokio::sync::mpsc::Sender<AgentEvent>>>>, paged_attn: bool) -> Result<(Self, String)> {
        match mode {
            AgentMode::Ollama => {
                Ok((Backend::Ollama(Ollama::default()), model))
            }
            AgentMode::MLX => {
                #[cfg(feature = "mlx")]
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
                        let _ = tx.send(AgentEvent::SubagentStatus(Some("⚡ [MLX]: Allocating Metal KV Cache (16k context)...".to_string()))).await;
                    } else {
                        println!("{} [MLX]: Allocating Paged Attention KV Cache (M4 Optimized)...", "⚡".yellow());
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

                    let gguf_file = format!("{}-{}.gguf", filename_prefix, quant);
                    
                    let mut builder = GgufModelBuilder::new(
                        &repo, 
                        vec![gguf_file.clone()]
                    )
                    .with_logging()
                    .with_max_num_seqs(1);
                    let ctx_limit = if paged_attn { 8192 } else { 16384 };

                    if paged_attn {
                        let paged_attn_cfg = PagedAttentionMetaBuilder::default()
                            .with_block_size(16)                    // Smaller blocks = much less fragmentation on Metal
                            .with_gpu_memory(MemoryGpuConfig::ContextSize(ctx_limit))
                            .build()
                            .map_err(|e| miette!("Failed to configure Paged Attention: {}", e))?;
                        builder = builder.with_paged_attn(paged_attn_cfg);
                    }

                    let mlx_model = builder
                        .build()
                        .await
                        .map_err(|e| miette!("Failed to load MLX model: {}", e))?;

                    let embed_model = EmbeddingModelBuilder::new("sentence-transformers/all-MiniLM-L6-v2")
                        .build()
                        .await
                        .ok(); // Fallback to None if embedding model fails to load

                    Ok((Backend::MLX { 
                        model: mlx_model, 
                        ctx_limit,
                        embedder: embed_model 
                    }, model))
                }
                #[cfg(not(feature = "mlx"))]
                {
                    Ok((Backend::Ollama(Ollama::default()), model))
                }
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
            Backend::Ollama(ollama) => {
                let mut request = ChatMessageRequest::new(model, history).options(options);
                if let Some(registry) = tool_registry {
                    request = request.tools(registry);
                }

                let tx = event_tx.lock().clone();
                if let Some(tx) = tx {
                    let _ = tx.send(AgentEvent::Thinking(Some("Thinking...".to_string()))).await;
                }
                
                let mut stream = ollama.send_chat_messages_stream(request).await.into_diagnostic()?;
                let mut is_thinking = false;
                let mut first_token = true;
                let mut last_segments: Vec<String> = Vec::new();
                let mut tag_residue = String::new();
                let mut in_thought_block = false;

                while let Some(res) = stream.next().await {
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
            #[cfg(feature = "mlx")]
            Backend::MLX { model: mistralrs, ctx_limit, .. } => {
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
                let mut other_messages = Vec::new();
                
                for msg in history {
                    if msg.role == MessageRole::System {
                        system_content.push(msg.content);
                    } else {
                        other_messages.push(msg);
                    }
                }
                
                let model_lower = model.to_lowercase();
                let is_reasoning_model = model_lower.contains("deepseek") || model_lower.contains("r1");

                let merged_system = system_content.join("\n\n");
                if !merged_system.is_empty() {
                    request_builder = request_builder.add_message(TextMessageRole::System, merged_system);
                }

                for msg in other_messages {
                    match msg.role {
                        MessageRole::User => {
                            request_builder = request_builder.add_message(TextMessageRole::User, msg.content);
                        }
                        MessageRole::Assistant => {
                            if !msg.tool_calls.is_empty() {
                                // Convert Ollama ToolCalls to Mistral ToolCallResponses
                                let mut mistral_calls = Vec::new();
                                for (i, c) in msg.tool_calls.iter().enumerate() {
                                    mistral_calls.push(mistralrs::ToolCallResponse {
                                        index: i,
                                        id: format!("call_{}", i), // Synthetic ID for ollama-rs compatibility
                                        tp: mistralrs::ToolCallType::Function,
                                        function: mistralrs::CalledFunction {
                                            name: c.function.name.clone(),
                                            arguments: c.function.arguments.to_string(),
                                        },
                                    });
                                }
                                
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
                            // Link tool results back to synthetic IDs (assuming sequential order for now)
                            let call_id = "call_0".to_string(); 
                            request_builder = request_builder.add_tool_message(msg.content, call_id);
                        }
                        _ => {
                            request_builder = request_builder.add_message(TextMessageRole::User, msg.content);
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
                sampling_params.max_len = Some(*ctx_limit);
                request_builder = request_builder.set_sampling(sampling_params);

                let tx_opt = event_tx.lock().clone();
                if let Some(tx) = tx_opt.as_ref() {
                    let _ = tx.try_send(AgentEvent::SubagentStatus(Some("⚡ MLX Engine: Dispatching Request to Metal...".to_string())));
                }

                let mut stream = mistralrs.stream_chat_request(request_builder).await.into_diagnostic()?;
                
                if let Some(tx) = tx_opt.as_ref() {
                    let _ = tx.try_send(AgentEvent::SubagentStatus(Some("⚡ MLX Engine: Stream established, waiting for first token...".to_string())));
                }

                let mut first_token = true;
                let mut is_thinking = is_reasoning_model; 
                let mut tag_residue = String::new();
                let mut in_thought_block = false;

                if is_thinking {
                    if let Some(tx) = event_tx.lock().clone() {
                        let _ = tx.try_send(AgentEvent::ReasoningToken("".to_string()));
                    }
                }
                let mut last_segments: Vec<String> = Vec::new();
                let mut should_break = false;
                
                while let Some(response) = stream.next().await {
                    if stop.load(std::sync::atomic::Ordering::Relaxed) || should_break { break; }
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

                                // --- 🧠 STREAMING REASONING EXTRACTION (MLX Cross-Chunk) ---
                                let mut current_pos = 0;
                                
                                // Detect implicit thinking at the absolute start of response (MLX Standalone)
                                // We do this BEFORE consuming first_token so we can catch "Alright..." etc.
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
                                        if let Some(tx) = event_tx.lock().clone() {
                                            let _ = tx.try_send(AgentEvent::ReasoningToken("".to_string()));
                                        }
                                    }
                                }

                                if first_token {
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
                                        } else if let Some(last_lt) = text[current_pos..].rfind('<') {
                                            let pot_tag = &text[current_pos + last_lt..];
                                            if "<think>".starts_with(pot_tag) {
                                                let before = &text[current_pos..current_pos + last_lt];
                                                if !before.is_empty() {
                                                    full_content.push_str(before);
                                                    if let Some(tx) = event_tx.lock().clone() {
                                                        let _ = tx.try_send(AgentEvent::StreamToken(before.to_string()));
                                                    }
                                                }
                                                tag_residue = pot_tag.to_string();
                                                break;
                                            } else {
                                                let remaining = &text[current_pos..];
                                                full_content.push_str(remaining);
                                                if let Some(tx) = event_tx.lock().clone() {
                                                    let _ = tx.try_send(AgentEvent::StreamToken(remaining.to_string()));
                                                }
                                                break;
                                            }
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
                                            let remaining = text[current_pos..].to_string();
                                            reasoning_content.push_str(&remaining);
                                            if let Some(tx) = event_tx.lock().clone() {
                                                let _ = tx.try_send(AgentEvent::ReasoningToken(remaining));
                                                let _ = tx.try_send(AgentEvent::SubagentStatus(Some(format!("⚡ MLX Engine: Reasoning... ({} chars)", reasoning_content.len()))));
                                            }
                                            break;
                                        }
                                    } else {
                                        let action = text[current_pos..].to_string();
                                        full_content.push_str(&action);
                                        if let Some(tx) = event_tx.lock().clone() {
                                            let _ = tx.try_send(AgentEvent::StreamToken(action));
                                            let _ = tx.try_send(AgentEvent::SubagentStatus(Some(format!("⚡ MLX Engine: Generating Action... ({} chars)", full_content.len()))));
                                        }
                                        break;
                                    }
                                }
                                
                                // --- 🛡️ REPETITION SENTINEL (MLX) ---
                                let trimmed = content.trim();
                                if !trimmed.is_empty() && trimmed.len() > 3 {
                                    last_segments.push(trimmed.to_string());
                                    if last_segments.len() > 15 { last_segments.remove(0); }
                                    if last_segments.iter().filter(|&s| s == trimmed).count() >= 8 {
                                        let warning = "\n\n⚠️ [REPETITION SENTINEL]: Breaking loop to prevent hallucination plateau.";
                                        full_content.push_str(warning);
                                        if let Some(tx) = event_tx.lock().clone() {
                                            let _ = tx.try_send(AgentEvent::StreamToken(warning.to_string()));
                                        }
                                        should_break = true;
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

        // --- 🛠️ FALLBACK JSON TOOL PARSER (MLX) ---
        // If native_tool_calls is empty but full_content has JSON blocks, parse them manually.
        // This is critical for models like DeepSeek-R1 that may hallucinate the native format
        // but correctly output the JSON in the text content.
        if native_tool_calls.is_empty() && !full_content.is_empty() {
            // Match ```json { "name": "...", "arguments": { ... } } ```
            let re_json = Regex::new(r"(?s)```json\s*(\{.*?\})\s*```").unwrap();
            for cap in re_json.captures_iter(&full_content) {
                if let Some(json_str) = cap.get(1) {
                    if let Ok(val) = serde_json::from_str::<serde_json::Value>(json_str.as_str()) {
                        // Extract name from root or nested function object
                        let tool_name = val.get("name")
                            .or_else(|| val.get("tool"))
                            .or_else(|| val.get("action"))
                            .or_else(|| val.get("function").and_then(|f| f.get("name")))
                            .and_then(|v| v.as_str());

                        if let Some(n) = tool_name {
                            // Fuzzy Tool Name Repair
                            let fixed_name = match n.to_lowercase().as_str() {
                                "extract_and_write" | "write" | "save" => "write_file".to_string(),
                                "read" | "cat" | "view" => "read_file".to_string(),
                                "shell" | "exec" | "terminal" => "run_command".to_string(),
                                _ => n.to_string(),
                            };

                            let arguments = val.get("arguments")
                                .or_else(|| val.get("args"))
                                .or_else(|| val.get("function").and_then(|f| f.get("arguments")))
                                .cloned()
                                .unwrap_or(serde_json::json!({}));

                            native_tool_calls.push(ollama_rs::generation::tools::ToolCall {
                                function: ollama_rs::generation::tools::ToolCallFunction {
                                    name: fixed_name,
                                    arguments,
                                }
                            });
                        }
                    }
                                                }
            }
            
            // Second pass: Catch naked JSON blocks if no code blocks were found
            if native_tool_calls.is_empty() {
                // UNIVERSAL NESTED TOOL CATCHER: Handles flat OR nested "function" structures.
                // Catch: { "name": "..." } OR { "function": { "name": "..." } }
                let re_native = Regex::new(r#"(?s)(?:```json\s*)?\{\s*(?:"function"\s*:\s*\{\s*)?"?\s*(?:name|tool|action|function)"?\s*:\s*"([^"]+)"\s*,\s*"?arguments"?\s*:\s*(\{.*?\})\s*\}?\s*\}(?:\s*```)?"#).unwrap();
                for cap in re_native.captures_iter(&full_content) {
                    let name = cap.get(1).map(|m| m.as_str().to_string());
                    let args_str = cap.get(2).map(|m| m.as_str());
                    
                    if let (Some(n), Some(a)) = (name, args_str) {
                        // Fuzzy Tool Name Repair (catch common LLM hallucinations)
                        let fixed_name = match n.to_lowercase().as_str() {
                            "extract_and_write" | "write" | "save" => "write_file".to_string(),
                            "read" | "cat" | "view" => "read_file".to_string(),
                            "shell" | "exec" | "terminal" => "run_command".to_string(),
                            _ => n,
                        };

                        if let Ok(args) = serde_json::from_str::<serde_json::Value>(a) {
                            native_tool_calls.push(ollama_rs::generation::tools::ToolCall {
                                function: ollama_rs::generation::tools::ToolCallFunction {
                                    name: fixed_name,
                                    arguments: args,
                                },
                            });
                        }
                    }
                }
            }

            // Third pass: Catch SEARCH/REPLACE diff blocks (hallucinated Aider-style tools)
            if native_tool_calls.is_empty() {
                let re_diff = Regex::new(r"(?s)<<<<<<<\s*SEARCH\n(.*?)\n=======\n(.*?)\n>>>>>>>\s*REPLACE").unwrap();
                for cap in re_diff.captures_iter(&full_content) {
                    let search = cap.get(1).map(|m| m.as_str().to_string()).unwrap_or_default();
                    let replace = cap.get(2).map(|m| m.as_str().to_string()).unwrap_or_default();
                    
                    // Heuristic: Try to find a filename mentioned in the 150 characters before the block
                    let block_start = cap.get(0).unwrap().start();
                    let preceding = &full_content[block_start.saturating_sub(150)..block_start];
                    let re_file = Regex::new(r"([a-zA-Z0-9_\-\./]+\.[a-z]+)").unwrap();
                    let path = re_file.find_iter(preceding)
                        .last()
                        .map(|m| m.as_str().to_string())
                        .unwrap_or_else(|| "unknown_file.txt".to_string());

                    native_tool_calls.push(ollama_rs::generation::tools::ToolCall {
                        function: ollama_rs::generation::tools::ToolCallFunction {
                            name: "edit_file_with_diff".to_string(),
                            arguments: serde_json::json!({
                                "path": path,
                                "diff": format!("<<<<<<< SEARCH\n{}\n=======\n{}\n>>>>>>> REPLACE", search, replace)
                            }),
                        }
                    });
                }
            }

            // Fourth pass: Catch raw Markdown code blocks (the "Hail Mary" catcher)
            if native_tool_calls.is_empty() {
                let re_markdown = Regex::new(r"(?s)```[a-zA-Z]*\n(.*?)\n```").unwrap();
                for cap in re_markdown.captures_iter(&full_content) {
                    let code = cap.get(1).map(|m| m.as_str().to_string()).unwrap_or_default();
                    if code.trim().len() < 10 { continue; }

                    // Heuristic: Search for a filename in the last 200 characters before the block
                    let block_start = cap.get(0).unwrap().start();
                    let context = if block_start > 200 {
                        &full_content[block_start-200..block_start]
                    } else {
                        &full_content[..block_start]
                    };
                    
                    let re_file = Regex::new(r"([a-zA-Z0-9_\-\./]+\.[a-z]+)").unwrap();
                    if let Some(path_match) = re_file.find_iter(context).last() {
                        let path = path_match.as_str().to_string();
                        // Ignore common false positives like "json" or "python" as paths
                        if path != "json" && path != "python" && path != "rust" && path != "bash" {
                            native_tool_calls.push(ollama_rs::generation::tools::ToolCall {
                                function: ollama_rs::generation::tools::ToolCallFunction {
                                    name: "write_file".to_string(),
                                    arguments: serde_json::json!({
                                        "path": path,
                                        "content": code
                                    }),
                                }
                            });
                        }
                    }
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
            #[cfg(feature = "mlx")]
            Backend::MLX { .. } => {
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
            #[cfg(feature = "mlx")]
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
        }
    }
}
