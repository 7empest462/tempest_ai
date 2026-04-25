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

#[cfg(feature = "mlx")]
use mistralrs::{
    GgufModelBuilder, Model, TextMessageRole, 
    Response as MistralResponse, RequestBuilder, SamplingParams
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
    MLX(Model),
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
            Backend::MLX(_) => AgentMode::MLX,
        }
    }

    pub async fn new(mode: AgentMode, model: String, quant: String, event_tx: Arc<parking_lot::Mutex<Option<tokio::sync::mpsc::Sender<AgentEvent>>>>) -> (Self, String) {
        match mode {
            AgentMode::Ollama => {
                let tx_opt = event_tx.lock().clone();
                if let Some(tx) = tx_opt {
                    let _ = tx.send(AgentEvent::SubagentStatus(Some("🚀 Connecting to Ollama Backend...".to_string()))).await;
                }
                (Backend::Ollama(Ollama::default()), model)
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
                    
                    // Disabled PagedAttention temporarily to troubleshoot Metal hangs. 
                    // We will use standard attention which is more stable on some M4 configurations.
                    /*
                    let paged_attn_cfg = PagedAttentionMetaBuilder::default()
                        .with_gpu_memory(MemoryGpuConfig::ContextSize(16384))
                        .build()
                        .expect("Failed to build PagedAttention config");
                    */

                    let tx_opt = event_tx.lock().clone();
                    if let Some(tx) = tx_opt {
                        let _ = tx.send(AgentEvent::SubagentStatus(Some("⚡ [MLX]: Allocating Metal KV Cache (16k context)...".to_string()))).await;
                    } else {
                        println!("{} [MLX]: Allocating Metal KV Cache (16k context)...", "⚡".yellow());
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
                    let mlx_model = GgufModelBuilder::new(
                        &repo, 
                        vec![gguf_file.clone()]
                    )
                    // .with_paged_attn(paged_attn_cfg) // Disabled for stability
                    .with_max_num_seqs(1)
                    .build()
                    .await
                    .expect("Failed to load MLX model");

                    let tx_opt = event_tx.lock().clone();
                    if let Some(tx) = tx_opt {
                        let _ = tx.send(AgentEvent::SubagentStatus(Some("✅ MLX model loaded successfully on Apple Silicon".to_string()))).await;
                    } else {
                        println!("{} MLX model loaded successfully on Apple Silicon", "✅".green());
                    }

                    (Backend::MLX(mlx_model), model)
                }
                #[cfg(not(feature = "mlx"))]
                {
                    (Backend::Ollama(Ollama::default()), model)
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
        mut on_tool_call: Option<Box<dyn FnMut(ollama_rs::generation::tools::ToolCall) + Send>>,
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
                let request = ChatMessageRequest::new(model, history).options(options);
                
                let tx = event_tx.lock().clone();
                if let Some(tx) = tx {
                    let _ = tx.send(AgentEvent::Thinking(Some("Thinking...".to_string()))).await;
                }

                let mut stream = ollama.send_chat_messages_stream(request).await.into_diagnostic()?;
                let mut is_thinking = false;
                let mut first_token = true;
                let mut last_segments: Vec<String> = Vec::new();
                let mut tag_residue = String::new();

                while let Some(res) = stream.next().await {
                    if stop.load(std::sync::atomic::Ordering::Relaxed) { break; }
                    let chunk = res.map_err(|_| miette!("Ollama stream error"))?;
                    
                    let mut text = tag_residue.clone();
                    text.push_str(&chunk.message.content);
                    tag_residue.clear();
                    
                    // --- 🧠 STREAMING REASONING EXTRACTION (Cross-Chunk Robust) ---
                    let mut current_pos = 0;
                    while current_pos < text.len() {
                        if !is_thinking {
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
                            } else if let Some(last_lt) = text[current_pos..].rfind('<') {
                                // Potential partial tag at end of chunk
                                let pot_tag = &text[current_pos + last_lt..];
                                if "<think>".starts_with(pot_tag) {
                                    // It's a potential start tag - buffer it
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
                                    // Not a tag, just a normal bracket
                                    let content = &text[current_pos..];
                                    full_content.push_str(content);
                                    if let Some(tx) = event_tx.lock().clone() {
                                        let _ = tx.try_send(AgentEvent::StreamToken(content.to_string()));
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
                        } else {
                            if let Some(end_idx) = text[current_pos..].find("</think>") {
                                // Reasoning before </think>
                                let reasoning = &text[current_pos..current_pos + end_idx];
                                reasoning_content.push_str(reasoning);
                                if let Some(tx) = event_tx.lock().clone() {
                                    let _ = tx.try_send(AgentEvent::ReasoningToken(reasoning.to_string()));
                                }
                                is_thinking = false;
                                current_pos += end_idx + 8; // Skip "</think>"
                            } else if let Some(last_lt) = text[current_pos..].rfind('<') {
                                // Potential partial end tag
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
                                    let content = &text[current_pos..];
                                    reasoning_content.push_str(content);
                                    if let Some(tx) = event_tx.lock().clone() {
                                        let _ = tx.try_send(AgentEvent::ReasoningToken(content.to_string()));
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
                        }
                    }

                    if first_token && !full_content.is_empty() {
                        first_token = false;
                        if let Some(tx) = event_tx.lock().clone() {
                            let _ = tx.try_send(AgentEvent::Thinking(None));
                        }
                    }

                    if !chunk.message.tool_calls.is_empty() {
                        for call in chunk.message.tool_calls {
                            if let Some(ref mut cb) = on_tool_call {
                                cb(call.clone());
                            }
                            native_tool_calls.push(call);
                        }
                    }

                    // --- 🛡️ REPETITION SENTINEL ---
                    let trimmed = text.trim();
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
            Backend::MLX(mistralrs) => {
                let _on_tool_call = on_tool_call; // Currently unused in MLX
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
                
                let merged_system = system_content.join("\n\n");
                let mut system_injected = false;

                for msg in other_messages {
                    let role = match msg.role {
                        MessageRole::User => TextMessageRole::User,
                        MessageRole::Assistant => TextMessageRole::Assistant,
                        _ => TextMessageRole::User,
                    };
                    
                    let mut content = msg.content;
                    if role == TextMessageRole::User && !system_injected && !merged_system.is_empty() {
                        content = format!("### SYSTEM INSTRUCTIONS ###\n{}\n\n### USER REQUEST ###\n{}", merged_system, content);
                        system_injected = true;
                    }
                    
                    request_builder = request_builder.add_message(role, content);
                }
                
                if !system_injected && !merged_system.is_empty() {
                    request_builder = request_builder.add_message(TextMessageRole::User, format!("### SYSTEM INSTRUCTIONS ###\n{}", merged_system));
                }

                // Apply backend-aware sampling parameters to MLX via direct SamplingParams configuration
                let mut sampling_params = SamplingParams::deterministic();
                sampling_params.temperature = Some(sampling.temperature.into());
                sampling_params.top_p = Some(sampling.top_p.into());
                sampling_params.top_k = Some(40);
                sampling_params.repetition_penalty = Some(sampling.repeat_penalty as f32);
                sampling_params.max_len = Some(8192);
                request_builder = request_builder.set_sampling(sampling_params);

                let tx_opt = event_tx.lock().clone();
                if let Some(tx) = tx_opt.as_ref() {
                    let _ = tx.try_send(AgentEvent::SubagentStatus(Some("⚡ MLX Engine: Pre-filling KV cache...".to_string())));
                }

                let mut stream = mistralrs.stream_chat_request(request_builder).await.into_diagnostic()?;
                
                if let Some(tx) = tx_opt.as_ref() {
                    let _ = tx.try_send(AgentEvent::SubagentStatus(Some("⚡ MLX Engine: Stream established, waiting for first token...".to_string())));
                }

                let mut first_token = true;
                let mut is_thinking = false;
                let mut tag_residue = String::new();
                
                while let Some(response) = stream.next().await {
                    if stop.load(std::sync::atomic::Ordering::Relaxed) { break; }
                    match response {
                        MistralResponse::Chunk(chunk) => {
                            if let Some(content) = &chunk.choices[0].delta.content {
                                let mut text = tag_residue.clone();
                                text.push_str(content);
                                tag_residue.clear();

                                 if first_token {
                                     if text.len() >= 10 || !text.starts_with("<") {
                                         first_token = false;
                                         if !text.contains("<think>") {
                                             if let Some(tx) = event_tx.lock().clone() {
                                                 let _ = tx.try_send(AgentEvent::Thinking(None));
                                             }
                                         }
                                     }
                                 }

                                // --- 🧠 STREAMING REASONING EXTRACTION (MLX Cross-Chunk) ---
                                let mut current_pos = 0;
                                while current_pos < text.len() {
                                    if !is_thinking {
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
                                    } else {
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
            Backend::MLX(_) => {
            }
        }
    }
}
