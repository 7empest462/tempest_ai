use miette::{Result, IntoDiagnostic, miette};
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
use std::sync::atomic::AtomicBool;
#[cfg(feature = "mlx")]
use mistralrs::{
    GgufModelBuilder, Model, TextMessages, TextMessageRole, 
    Response as MistralResponse
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

pub struct InferenceOutput {
    pub content: String,
    pub reasoning: String,
    pub native_tool_calls: Vec<ollama_rs::generation::tools::ToolCall>,
}

impl Backend {
    pub async fn new(mode: AgentMode, model: String, quant: String, event_tx: Arc<parking_lot::Mutex<Option<tokio::sync::mpsc::Sender<AgentEvent>>>>) -> (Self, String) {
        match mode {
            AgentMode::Ollama => {
                let tx_opt = event_tx.lock().clone();
                if let Some(tx) = tx_opt {
                    let _ = tx.send(AgentEvent::SubagentStatus(Some("🚀 Loading MLX Backend (optimized for M4 Neural Engine + GPU)...".to_string()))).await;
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
                    }

                    let gguf_file = format!("Qwen2.5-Coder-7B-Instruct-{}.gguf", quant);
                    let mlx_model = GgufModelBuilder::new(
                        "bartowski/Qwen2.5-Coder-7B-Instruct-GGUF", 
                        vec![gguf_file.clone()] // Uses the selected quantization
                    )
                    // .with_paged_attn(paged_attn_cfg) // Disabled for stability
                    .with_max_num_seqs(1)
                    .build()
                    .await
                    .expect("Failed to load MLX model");

                    let tx_opt = event_tx.lock().clone();
                    if let Some(tx) = tx_opt {
                        let _ = tx.send(AgentEvent::SubagentStatus(Some("✅ MLX model loaded successfully on Apple Silicon".to_string()))).await;
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
        options: ModelOptions,
        event_tx: Arc<parking_lot::Mutex<Option<tokio::sync::mpsc::Sender<AgentEvent>>>>,
        stop: Arc<AtomicBool>,
        _system_prompt: String,
    ) -> Result<InferenceOutput> {
        let mut full_content = String::new();
        let mut reasoning_content = String::new();
        let mut native_tool_calls = Vec::new();

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

                while let Some(res) = stream.next().await {
                    if stop.load(std::sync::atomic::Ordering::Relaxed) { break; }
                    let chunk = res.map_err(|_| miette!("Ollama stream error"))?;
                    
                    let text = chunk.message.content.clone();
                    
                    // --- 🧠 REASONING EXTRACTION ---
                    if text.contains("<think>") {
                        is_thinking = true;
                        continue;
                    }
                    if text.contains("</think>") {
                        is_thinking = false;
                        continue;
                    }

                    if is_thinking {
                        reasoning_content.push_str(&text);
                        if let Some(tx) = event_tx.lock().clone() {
                            let _ = tx.try_send(AgentEvent::ReasoningToken(text.clone()));
                        }
                    } else {
                        if first_token {
                            first_token = false;
                            if let Some(tx) = event_tx.lock().clone() {
                                let _ = tx.try_send(AgentEvent::Thinking(None));
                            }
                        }
                        full_content.push_str(&text);
                        if let Some(tx) = event_tx.lock().clone() {
                            let _ = tx.try_send(AgentEvent::StreamToken(text.clone()));
                        }
                    }

                    if !chunk.message.tool_calls.is_empty() {
                        for call in chunk.message.tool_calls {
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
                let tx_opt = event_tx.lock().clone();
                if let Some(tx) = tx_opt {
                    let _ = tx.send(AgentEvent::Thinking(Some("Thinking...".to_string()))).await;
                    let est_tokens = crate::context_manager::estimate_tokens(&history);
                    let _ = tx.try_send(AgentEvent::ContextStatus { used: est_tokens, total: 16384 });
                    let _ = tx.try_send(AgentEvent::SubagentStatus(Some(format!("⚡ MLX Engine: Dispatching request ({} history tokens)...", est_tokens))));
                }

                let mut messages = TextMessages::new();
                for msg in history {
                    let role = match msg.role {
                        MessageRole::System => TextMessageRole::System,
                        MessageRole::User => TextMessageRole::User,
                        MessageRole::Assistant => TextMessageRole::Assistant,
                        _ => TextMessageRole::User,
                    };
                    messages = messages.add_message(role, msg.content);
                }

                let tx_opt = event_tx.lock().clone();
                if let Some(tx) = tx_opt.as_ref() {
                    let _ = tx.try_send(AgentEvent::SubagentStatus(Some("⚡ MLX Engine: Pre-filling KV cache...".to_string())));
                }

                let mut stream = mistralrs.stream_chat_request(messages).await.into_diagnostic()?;
                
                if let Some(tx) = tx_opt.as_ref() {
                    let _ = tx.try_send(AgentEvent::SubagentStatus(Some("⚡ MLX Engine: Stream established, waiting for first token...".to_string())));
                }

                let mut first_token = true;
                while let Some(response) = stream.next().await {
                    if stop.load(std::sync::atomic::Ordering::Relaxed) { break; }
                    match response {
                        MistralResponse::Chunk(chunk) => {
                            if let Some(text) = &chunk.choices[0].delta.content {
                                if first_token {
                                    first_token = false;
                                    if let Some(tx) = event_tx.lock().clone() {
                                        let _ = tx.try_send(AgentEvent::Thinking(None));
                                        let _ = tx.try_send(AgentEvent::SubagentStatus(Some("🚀 MLX Engine: Token generation started.".to_string())));
                                    }
                                }
                                full_content.push_str(text);
                                if let Some(tx) = event_tx.lock().clone() {
                                    let _ = tx.try_send(AgentEvent::StreamToken(text.clone()));
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
