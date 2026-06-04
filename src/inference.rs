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
    PagedAttentionMetaBuilder, MemoryGpuConfig, PagedCacheType,
    EmbeddingRequest
};
use crate::tui::AgentEvent;
use rig::completion::CompletionModel;
use rig::embeddings::EmbeddingModel;
use tool_parser::ToolParser;
#[cfg(target_os = "macos")]
use sysinfo::System;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum AgentMode {
    Ollama,
    MLX,
    Bridge,
    LMStudio,
    Kalosm,
    Gemini,
}

#[derive(Clone)]
pub enum Backend {
    Ollama(Ollama),
    #[cfg(target_os = "macos")]
    MLX {
        model: std::sync::Arc<Model>,
        _ctx_limit: usize,
        embedder: Option<std::sync::Arc<Model>>,
        ollama_fallback: Option<Ollama>,
    },
    Bridge(crate::ai_bridge::TempestAiBridge),
    Kalosm {
        model: String,
        engine: std::sync::Arc<tokio::sync::Mutex<Option<kalosm::language::Llama>>>,
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

#[derive(serde::Deserialize, Extract, Debug)]
pub struct ToolCallPayload {
    #[serde(alias = "tool", alias = "function", alias = "action", alias = "function_name")]
    pub name: String,
    #[serde(alias = "params", alias = "args", alias = "parameters")]
    pub arguments: serde_json::Value,
}

impl Backend {
    pub fn mode(&self) -> AgentMode {
        match self {
            Backend::Ollama(_) => AgentMode::Ollama,
            #[cfg(target_os = "macos")]
            Backend::MLX { .. } => AgentMode::MLX,
            Backend::Bridge(_) => AgentMode::Bridge, // Note: LMStudio also maps to Bridge internally for now
            Backend::Kalosm { .. } => AgentMode::Kalosm,
            // Gemini uses the Bridge
        }
    }

    pub fn raw_history(&self) -> Option<std::sync::Arc<parking_lot::Mutex<Vec<serde_json::Value>>>> {
        match self {
            Backend::Bridge(bridge) => Some(bridge.raw_history.clone()),
            _ => None,
        }
    }


    #[allow(dead_code)]
    pub fn supports_tools(&self) -> bool {
        match self {
            Backend::Ollama(_) => true,
            #[cfg(target_os = "macos")]
            Backend::MLX { .. } => true,
            Backend::Bridge(_) => true, // We handle raw text tool parsing for Bridge
            Backend::Kalosm { .. } => false, // No native tool calling for basic kalosm right now
        }
    }

    pub async fn new(
        mode: AgentMode, 
        model: String, 
        quant: String, 
        event_tx: Arc<parking_lot::Mutex<Option<tokio::sync::mpsc::Sender<AgentEvent>>>>, 
        paged_attn: bool, 
        ctx_limit: usize,
        base_url: Option<String>,
        pa_memory_mb: Option<usize>
    ) -> Result<(Self, String)> {
        #[cfg(not(target_os = "macos"))]
        {
            let _ = quant;
            let _ = event_tx;
            let _ = paged_attn;
            let _ = ctx_limit;
            let _ = pa_memory_mb;
        }

        match mode {
            AgentMode::Ollama => {
                let client = if let Some(url_str) = base_url {
                    if let Ok(url) = url::Url::parse(&url_str) {
                        Ollama::from_url(url)
                    } else {
                        Ollama::default()
                    }
                } else {
                    Ollama::default()
                };
                Ok((Backend::Ollama(client), model))
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
                            .with_prefix_cache_n(Some(16))
                            .with_max_num_seqs(1);

                        if paged_attn {
                            println!("{} MLX: Initializing Paged Attention (Window: {} tokens)", "⚡".yellow(), ctx_limit);
                            
                            // Determine the memory limit
                            let limit_mb = if let Some(custom_limit) = pa_memory_mb {
                                custom_limit
                            } else {
                                let mut sys = System::new_all();
                                sys.refresh_memory();
                                let total_mb = sys.total_memory() / 1024 / 1024;
                                (total_mb as f32 * 0.90) as usize
                            };
                            
                            println!("{} PagedAttention Budget: {} MB (KV Cache Quantization: F8)", "⚡".yellow(), limit_mb);

                            let paged_attn_cfg = PagedAttentionMetaBuilder::default()
                                .with_block_size(32)
                                .with_gpu_memory(MemoryGpuConfig::MbAmount(limit_mb))
                                .with_paged_cache_type(PagedCacheType::F8E4M3)
                                .build()
                                .map_err(|e| miette!("Failed to configure Paged Attention: {}", e))?;
                            builder = builder.with_paged_attn(paged_attn_cfg);
                        }
                        builder.build().await.map_err(|e| miette!("Failed to load MLX GGUF model: {}", e))?
                    } else {
                        println!("{} MLX: Initializing Native Safetensors Backend...", "⚡".yellow());
                        let mut builder = TextModelBuilder::new(&repo)
                            .with_logging()
                            .with_prefix_cache_n(Some(16))
                            .with_max_num_seqs(1);
                        
                        if paged_attn {
                             // Determine the memory limit for Native SafeTensors
                             let limit_mb = if let Some(custom_limit) = pa_memory_mb {
                                 custom_limit
                             } else {
                                 let mut sys = System::new_all();
                                 sys.refresh_memory();
                                 let total_mb = sys.total_memory() / 1024 / 1024;
                                 (total_mb as f32 * 0.90) as usize
                             };
                             
                             println!("{} PagedAttention Budget: {} MB (KV Cache Quantization: F8)", "⚡".yellow(), limit_mb);

                             let paged_attn_cfg = PagedAttentionMetaBuilder::default()
                                .with_block_size(32)
                                .with_gpu_memory(MemoryGpuConfig::MbAmount(limit_mb))
                                .with_paged_cache_type(PagedCacheType::F8E4M3)
                                .build()
                                .map_err(|e| miette!("Failed to configure Paged Attention: {}", e))?;
                            builder = builder.with_paged_attn(paged_attn_cfg);
                        }
                        builder.build().await.map_err(|e| miette!("Failed to load MLX Native model: {}", e))?
                    };

                    // The MLX backend in Mistral.rs currently lacks support for BertModel 
                    // architectures (like all-minilm or nomic). We bypass the native embedder 
                    // and rely entirely on the Ollama fallback for semantic indexing.
                    let embed_model = None;
                    
                    Ok((Backend::MLX { 
                        model: std::sync::Arc::new(mlx_model), 
                        _ctx_limit: ctx_limit,
                        embedder: embed_model,
                        ollama_fallback: Some(Ollama::default())
                    }, model))
                }
                #[cfg(not(target_os = "macos"))]
                {
                    Ok((Backend::Ollama(Ollama::default()), model))
                }
            }
            AgentMode::Bridge => {
                let url = base_url.unwrap_or_else(|| "http://127.0.0.1:11434".to_string());
                let provider = crate::ai_bridge::ModelProvider::Ollama { 
                    base_url: url 
                };
                let models: Vec<String> = model
                    .split(',')
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty())
                    .collect();
                let bridge = crate::ai_bridge::TempestAiBridge::new(provider, models)?;
                Ok((Backend::Bridge(bridge), model))
            }
            AgentMode::LMStudio => {
                let url = base_url.unwrap_or_else(|| "http://127.0.0.1:1234/v1".to_string());
                let provider = crate::ai_bridge::ModelProvider::OpenAI { 
                    api_key: "lm-studio".to_string(),
                    base_url: Some(url)
                };
                let models: Vec<String> = model
                    .split(',')
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty())
                    .collect();
                let bridge = crate::ai_bridge::TempestAiBridge::new(provider, models)?;
                Ok((Backend::Bridge(bridge), model))
            }
            AgentMode::Kalosm => {
                let tx_opt = event_tx.lock().clone();
                if let Some(tx) = tx_opt.clone() {
                    let _ = tx.send(AgentEvent::SubagentStatus(Some("🚀 Initializing Kalosm Native Backend...".to_string()))).await;
                } else {
                    println!("🚀 Initializing Kalosm Native Backend...");
                }

                // Check for potential out-of-memory or swap thrashing scenario
                let mut sys = sysinfo::System::new();
                sys.refresh_memory();
                let total_mb = sys.total_memory() / 1024 / 1024;
                if total_mb < 24000 && (model.contains("Q8") || model.contains("q8") || model.contains("8_0")) {
                    let warning = "⚠️  [SYSTEM WARNING]: Loading Q8_0 (~8.6GB) model on a low-RAM system (<24GB Unified Memory) may trigger swap space thrashing and freeze the computer. Q4_K_M is recommended, or use the optimized MLX backend.";
                    if let Some(ref tx) = tx_opt {
                        let _ = tx.send(AgentEvent::SubagentStatus(Some(warning.to_string()))).await;
                        tokio::time::sleep(tokio::time::Duration::from_secs(4)).await;
                    } else {
                        println!("{}", warning);
                    }
                }
                
                // Kalosm Model Initialization
                // If the user specifies "kalosm_default", we pull Llama-3.
                // If the string contains "::", we treat it as a HuggingFace download: "repo/id::filename.gguf".
                // Otherwise, we check if it's a valid local file path.
                let engine = if model == "kalosm_default" || model.trim().is_empty() {
                    kalosm::language::Llama::builder()
                        .build()
                        .await
                        .map_err(|e| miette::miette!("Failed to load default Kalosm model: {}", e))?
                } else if model.contains("::") {
                    let parts: Vec<&str> = model.split("::").collect();
                    let source = kalosm::language::FileSource::HuggingFace {
                        model_id: parts[0].to_string(),
                        revision: "main".to_string(),
                        file: parts[1].to_string(),
                    };
                    kalosm::language::Llama::builder()
                        .with_source(kalosm::language::LlamaSource::new(source))
                        .build()
                        .await
                        .map_err(|e| miette::miette!("Failed to load HuggingFace Kalosm model: {}", e))?
                } else if std::path::Path::new(&model).exists() {
                    let source = kalosm::language::FileSource::Local(std::path::PathBuf::from(&model));
                    kalosm::language::Llama::builder()
                        .with_source(kalosm::language::LlamaSource::new(source))
                        .build()
                        .await
                        .map_err(|e| miette::miette!("Failed to load local Kalosm model: {}", e))?
                } else {
                    return Err(miette::miette!(
                        "Invalid Kalosm model format: '{}'. Use 'kalosm_default', a local file path, or 'RepoID/Name::filename.gguf' for HF downloads.", model
                    ));
                };
                
                Ok((Backend::Kalosm { 
                    model: model.clone(),
                    engine: std::sync::Arc::new(tokio::sync::Mutex::new(Some(engine))),
                }, model))
            }
            AgentMode::Gemini => {
                let api_key = std::env::var("GEMINI_API_KEY").unwrap_or_default();
                if api_key.is_empty() {
                    return Err(miette::miette!("GEMINI_API_KEY environment variable is missing or empty."));
                }
                
                let tx_opt = event_tx.lock().clone();
                if let Some(tx) = tx_opt {
                    let _ = tx.send(AgentEvent::SubagentStatus(Some("🚀 Connecting to Google Gemini API (OpenAI Compat)...".to_string()))).await;
                } else {
                    println!("{} Connecting to Google Gemini API...", "🚀");
                }

                let provider = crate::ai_bridge::ModelProvider::Gemini { api_key };
                let models: Vec<String> = model
                    .split(',')
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty())
                    .collect();
                let bridge = crate::ai_bridge::TempestAiBridge::new(provider, models)?;
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
        let history = if matches!(self, Backend::Bridge(_)) {
            history
        } else {
            normalize_history(&history)
        };

        // Pre-allocate with capacity to reduce reallocations during streaming
        // ~8192 tokens at ~4-5 chars per token ≈ 32-40KB; allocate 64KB to be safe
        let mut full_content = String::with_capacity(65536);
        let mut reasoning_content = String::with_capacity(65536);
        let mut native_tool_calls = Vec::new();
        
        // 🧠 REASONING CAP: Prevent unbounded thinking from causing loops
        // 20KB ≈ 5000 tokens; enough for detailed reasoning, prevents accumulation bloat
        const REASONING_CAP: usize = 20480;

        let options = ModelOptions::default()
            .num_ctx(sampling.context_size)
            .num_predict(8192)
            .temperature(sampling.temperature)
            .repeat_penalty(sampling.repeat_penalty)
            .top_k(40)
            .top_p(sampling.top_p);

        match self {
            Backend::Kalosm { engine, model: _ } => {
                use kalosm::language::*;
                use futures::stream::StreamExt;
                
                let mut engine_lock = engine.lock().await;
                if let Some(llama) = engine_lock.as_mut() {
                    // Build prompt from full history (don't window here - let model see context)
                    let mut prompt = String::new();
                    for msg in &history {
                        match msg.role {
                            MessageRole::System => {
                                prompt.push_str("<|im_start|>system\n");
                                prompt.push_str(&msg.content);
                                prompt.push_str("<|im_end|>\n");
                            }
                            MessageRole::User => {
                                prompt.push_str("<|im_start|>user\n");
                                prompt.push_str(&msg.content);
                                prompt.push_str("<|im_end|>\n");
                            }
                            MessageRole::Assistant => {
                                prompt.push_str("<|im_start|>assistant\n");
                                prompt.push_str(&msg.content);
                                prompt.push_str("<|im_end|>\n");
                            }
                            _ => {
                                prompt.push_str("<|im_start|>system\nTool Result:\n");
                                prompt.push_str(&msg.content);
                                prompt.push_str("<|im_end|>\n");
                            }
                        }
                    }
                    prompt.push_str("<|im_start|>assistant\n");
                    
                    // Use configured sampling parameters
                    let mut chat = llama.chat().with_system_prompt("You are Tempest AI.");
                    let mut stream = chat(&prompt);
                    let mut token_count = 0;
                    let kalosm_max_tokens: usize = sampling.context_size as usize / 4; // Allow up to 1/4 of context size
                    
                    while let Some(chunk) = stream.next().await {
                        if stop.load(std::sync::atomic::Ordering::Relaxed) {
                            break;
                        }
                        
                        token_count += 1;
                        if token_count > kalosm_max_tokens {
                            break;
                        }
                        
                        let chunk_str: String = chunk.to_string();
                        full_content.push_str(&chunk_str);
                        
                        let tx_opt = event_tx.lock().clone();
                        if let Some(tx) = tx_opt {
                            let _ = tx.try_send(crate::tui::AgentEvent::StreamToken(chunk_str));
                        }
                    }
                    
                    // Explicitly drop stream to release streaming resources
                    drop(stream);
                }
                
                return Ok(InferenceOutput {
                    content: full_content,
                    reasoning: reasoning_content,
                    native_tool_calls: vec![],
                });
            }
            Backend::Bridge(bridge) => {
                let model_lower = model.to_lowercase();
                let is_reasoning = model_lower.contains("r1") || model_lower.contains("deepseek") || model_lower.contains("deep-seek");
                
                let tx_opt = event_tx.lock().clone();
                if let Some(tx) = tx_opt {
                    let _ = tx.send(AgentEvent::Thinking(Some("Thinking...".to_string()))).await;
                }

                let mut stream = bridge.stream_chat(history.clone(), tool_registry.clone()).await?;
                
                let tx_opt = event_tx.lock().clone();
                if let Some(tx) = tx_opt {
                    let _ = tx.send(AgentEvent::SubagentStatus(Some("⚡ Bridge: Connection established, waiting for tokens...".to_string()))).await;
                }

                let mut is_thinking = is_reasoning;
                let in_thought_block = is_reasoning;
                let mut tag_residue = String::new();
                let mut first_token = true;
                let mut token_count = 0;
                let start_time = std::time::Instant::now();

                let mut tool_arguments_deltas: std::collections::HashMap<usize, String> = std::collections::HashMap::new();
                let mut max_tool_index = 0usize;  // Track highest index used so far

                while let Some(chunk_res) = stream.next().await {
                    if stop.load(std::sync::atomic::Ordering::Relaxed) { break; }
                    let chunk_val = chunk_res.map_err(|e| miette!("Bridge Stream Error: {}", e))?;

                    if let Some(choices) = chunk_val.get("choices").and_then(|c| c.as_array()) {
                        if let Some(choice) = choices.first() {
                            let delta = choice.get("delta").cloned().unwrap_or_default();
                            
                            if let Some(token) = delta.get("content").and_then(|c| c.as_str()) {
                                token_count += 1;
                                
                                // Broadcast TPS pulse
                                let elapsed = start_time.elapsed().as_secs_f32();
                                if elapsed > 0.1 {
                                    let tps = token_count as f32 / elapsed;
                                    let tx_opt = event_tx.lock().clone();
                                    if let Some(tx) = tx_opt {
                                        let _ = tx.try_send(AgentEvent::TelemetryMetrics { 
                                            cpu: None, gpu: None, ram: None, tps: Some(tps as u64) 
                                        });
                                    }
                                }

                                if first_token {
                                    first_token = false;
                                    let tx_opt = event_tx.lock().clone();
                                    if let Some(tx) = tx_opt {
                                        let _ = tx.send(AgentEvent::Thinking(None)).await;
                                        let _ = tx.send(AgentEvent::SubagentStatus(Some("🌊 Stream Active: Receiving tokens...".to_string()))).await;
                                    }
                                }
                                let mut text = tag_residue.clone();
                                text.push_str(token);
                                tag_residue.clear();

                                let mut current_pos = 0;
                                while current_pos < text.len() {
                                    if !is_thinking && !in_thought_block {
                                        if let Some(start_idx) = text[current_pos..].find("<think>") {
                                            let before = &text[current_pos..current_pos + start_idx];
                                            if !before.trim().is_empty() {
                                                full_content.push_str(before);
                                                if let Some(tx) = event_tx.lock().clone() {
                                                    let _ = tx.try_send(AgentEvent::StreamToken(before.to_string()));
                                                }
                                            }
                                            is_thinking = true;
                                            if let Some(tx) = event_tx.lock().clone() {
                                                let _ = tx.try_send(AgentEvent::ReasoningToken("".to_string()));
                                            }
                                            current_pos += start_idx + 7;
                                            continue;
                                        } else if let Some(last_lt) = text[current_pos..].rfind('<') {
                                            // Potential partial start tag
                                            let pot_tag = &text[current_pos + last_lt..];
                                            if "<think>".starts_with(pot_tag) {
                                                tag_residue = pot_tag.to_string();
                                                let before = &text[current_pos..current_pos + last_lt];
                                                if !before.is_empty() {
                                                    full_content.push_str(before);
                                                    if let Some(tx) = event_tx.lock().clone() {
                                                        let _ = tx.try_send(AgentEvent::StreamToken(before.to_string()));
                                                    }
                                                }
                                                break;
                                            }
                                        }
                                    }

                                    if is_thinking {
                                        if let Some(end_idx) = text[current_pos..].find("</think>") {
                                            let reasoning = &text[current_pos..current_pos + end_idx];
                                            reasoning_content.push_str(reasoning);
                                            if let Some(tx) = event_tx.lock().clone() {
                                                let _ = tx.try_send(AgentEvent::ReasoningToken(reasoning.to_string()));
                                            }
                                            is_thinking = false;
                                            current_pos += end_idx + 8;
                                            continue;
                                        } else if let Some(last_lt) = text[current_pos..].rfind('<') {
                                            // Potential partial end tag
                                            let pot_tag = &text[current_pos + last_lt..];
                                            if "</think>".starts_with(pot_tag) {
                                                tag_residue = pot_tag.to_string();
                                                let before = &text[current_pos..current_pos + last_lt];
                                                if !before.is_empty() {
                                                    reasoning_content.push_str(before);
                                                    if let Some(tx) = event_tx.lock().clone() {
                                                        let _ = tx.try_send(AgentEvent::ReasoningToken(before.to_string()));
                                                    }
                                                }
                                                break;
                                            }
                                        }
                                        
                                        let remaining = &text[current_pos..];
                                        reasoning_content.push_str(remaining);
                                        if let Some(tx) = event_tx.lock().clone() {
                                            let _ = tx.try_send(AgentEvent::ReasoningToken(remaining.to_string()));
                                        }
                                        break;
                                    } else {
                                        let remaining = &text[current_pos..];
                                        full_content.push_str(remaining);
                                        if let Some(tx) = event_tx.lock().clone() {
                                            let _ = tx.try_send(AgentEvent::StreamToken(remaining.to_string()));
                                        }
                                        break;
                                    }
                                }
                            }

                            if let Some(tool_calls) = delta.get("tool_calls").and_then(|t| t.as_array()) {
                                for tc in tool_calls {
                                    // ⚠️ CRITICAL: Use the explicit index field if provided, otherwise track by call number
                                    let index_from_provider = tc.get("index").and_then(|i| i.as_u64()).map(|i| i as usize);
                                    
                                    if let Some(func) = tc.get("function") {
                                        let mut actual_name = String::new();
                                        let mut override_idx: Option<usize> = None;
                                        
                                        if let Some(name_str) = func.get("name").and_then(|n| n.as_str()) {
                                            // Check if the name has index encoding: __idx_N__actual_name
                                            if name_str.starts_with("__idx_") {
                                                if let Some(end) = name_str[6..].find("__") {
                                                    let absolute_end = 6 + end;
                                                    if let Ok(num) = name_str[6..absolute_end].parse::<usize>() {
                                                        // Successfully parsed index from encoding
                                                        override_idx = Some(num);
                                                        // Extract the REAL tool name (after __idx_N__)
                                                        actual_name = name_str[absolute_end+2..].to_string();
                                                    }
                                                }
                                            } else {
                                                // No encoding, use name as-is
                                                actual_name = name_str.to_string();
                                            }
                                        }
                                        
                                        // 🔄 Determine index FIRST — argument-delta chunks have no name
                                        // but still carry arguments via the provider's index field.
                                        let resolved_idx = override_idx.or(index_from_provider);

                                        // Skip truly unprocessable chunks (no name AND no index)
                                        if actual_name.is_empty() && resolved_idx.is_none() {
                                            continue;
                                        }

                                        let extracted_idx = resolved_idx.unwrap_or_else(|| {
                                            // Assign next available index and increment tracker
                                            let idx = max_tool_index;
                                            max_tool_index = max_tool_index.saturating_add(1);
                                            idx
                                        });
                                        
                                        // Also track the highest index we've used (from encoded or provider)
                                        if let Some(idx) = resolved_idx {
                                            max_tool_index = max_tool_index.max(idx + 1);
                                        }
                                        
                                        // Grow array to accommodate this index
                                        while native_tool_calls.len() <= extracted_idx {
                                            native_tool_calls.push(ollama_rs::generation::tools::ToolCall {
                                                function: ollama_rs::generation::tools::ToolCallFunction {
                                                    name: String::new(),
                                                    arguments: serde_json::Value::Object(serde_json::Map::new()),
                                                },
                                            });
                                        }
                                        
                                        // Set tool name only on chunks that carry one (first chunk per call)
                                        if !actual_name.is_empty() && native_tool_calls[extracted_idx].function.name.is_empty() {
                                            native_tool_calls[extracted_idx].function.name = actual_name.clone();
                                            if let Some(tx) = event_tx.lock().clone() {
                                                let _ = tx.try_send(AgentEvent::StreamToken(format!("\n\n```json\n// Tool: {}\n", actual_name)));
                                            }
                                        }

                                        // 🔄 ALWAYS accumulate arguments, even on name-less delta chunks
                                        let args_delta_opt = if let Some(a) = func.get("arguments") {
                                            if let Some(s) = a.as_str() {
                                                Some(s.to_string())
                                            } else if a.is_object() {
                                                Some(a.to_string())
                                            } else {
                                                None
                                            }
                                        } else {
                                            None
                                        };

                                        if let Some(args_delta) = args_delta_opt {
                                            if !args_delta.is_empty() {
                                                let entry = tool_arguments_deltas.entry(extracted_idx).or_default();
                                                entry.push_str(&args_delta);
                                                if let Some(tx) = event_tx.lock().clone() {
                                                    let _ = tx.try_send(AgentEvent::StreamToken(args_delta));
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }

                    // 🛑 HARD STOP / YIELD: For Bridge backend, DO NOT break early on tool calls
                    // Tool calls may arrive sequentially, and breaking after the first one will
                    // cause subsequent tool calls to be missed. Let the stream complete naturally.
                    // The fallback extraction will catch any tool calls we might have missed.
                    // (Early break logic removed for Bridge backend - too aggressive)
                }

                // Finalize tool call arguments
                for (idx, args_str) in tool_arguments_deltas {
                    if args_str.is_empty() {
                        // Empty arguments - leave as empty object
                        continue;
                    }
                    
                    // Try to parse as complete JSON first
                    match serde_json::from_str::<serde_json::Value>(&args_str) {
                        Ok(args_val) => {
                            if idx < native_tool_calls.len() {
                                native_tool_calls[idx].function.arguments = args_val;
                            }
                        }
                        Err(_e) => {
                            // If JSON parsing fails, try to repair it
                            let repaired = crate::overwatch::repair_json_str(&args_str);
                            match serde_json::from_str::<serde_json::Value>(&repaired) {
                                Ok(args_val) => {
                                    if idx < native_tool_calls.len() {
                                        native_tool_calls[idx].function.arguments = args_val;
                                    }
                                }
                                Err(_) => {
                                    // Still failed - log as warning but don't crash
                                    if let Some(tx) = event_tx.lock().clone() {
                                        let _ = tx.try_send(AgentEvent::SystemUpdate(
                                            format!("⚠️ Could not parse tool arguments for index {}: {}", idx, args_str.chars().take(100).collect::<String>())
                                        ));
                                    }
                                }
                            }
                        }
                    }
                }

                // Filter out invalid tool calls (empty names indicate incomplete parsing)
                native_tool_calls.retain(|call| !call.function.name.is_empty());

                // 🔄 Save content BEFORE appending native tool calls, for supplementary extraction.
                // This captures text-based tool calls that the model output as content,
                // without re-extracting native API calls that get appended below.
                let content_for_extraction = format!("{}\n{}", reasoning_content, full_content);

                // Incorporate native tool calls into full_content for consistent tracking
                for call in &native_tool_calls {
                    let json_val = serde_json::json!({
                        "tool": call.function.name,
                        "arguments": call.function.arguments
                    });
                    if let Ok(json_str) = serde_json::to_string(&json_val) {
                        if !full_content.ends_with('\n') && !full_content.is_empty() {
                            full_content.push('\n');
                        }
                        full_content.push_str(&json_str);
                    }
                }

                // --- 🛡️ HARDENED TOOL EXTRACTION (llm-extract) ---
                // 🔄 ALWAYS run extraction, even when native API calls exist.
                // Gemini often sends the first call natively and outputs the rest as text.
                {
                    let combined_content = content_for_extraction;

                    let mut extract_success = false;
                    if let Ok(payloads) = llm_extract::extract_all::<ToolCallPayload>(&combined_content) {
                        if !payloads.is_empty() {
                            extract_success = true;
                            for payload in payloads {
                                let mut name = payload.name;
                                if name.starts_with("__idx_") {
                                    if let Some(end) = name[6..].find("__") {
                                        let absolute_end = 6 + end;
                                        name = name[absolute_end+2..].to_string();
                                    }
                                }
                                native_tool_calls.push(ollama_rs::generation::tools::ToolCall {
                                    function: ollama_rs::generation::tools::ToolCallFunction {
                                        name,
                                        arguments: payload.arguments,
                                    }
                                });
                            }
                        }
                    }
                    
                    if !extract_success {
                        // --- 🛡️ FALLBACK: tool-parser (v1.2.0) with DeepSeekParser ---
                        let parser = tool_parser::DeepSeekParser::new();
                        let mut parsed_success = false;
                        if let Ok((_text, calls)) = parser.parse_complete(&combined_content).await {
                            if !calls.is_empty() {
                                parsed_success = true;
                                for call in calls {
                                    let mut name = call.function.name;
                                    if name.starts_with("__idx_") {
                                        if let Some(end) = name[6..].find("__") {
                                            let absolute_end = 6 + end;
                                            name = name[absolute_end+2..].to_string();
                                        }
                                    }
                                    native_tool_calls.push(ollama_rs::generation::tools::ToolCall {
                                        function: ollama_rs::generation::tools::ToolCallFunction {
                                            name,
                                            arguments: serde_json::from_str(&call.function.arguments).unwrap_or(serde_json::json!({})),
                                        }
                                    });
                                }
                            }
                        }

                        if !parsed_success {
                            // --- 🛡️ EXTRA FALLBACK: Robust brace counting extraction ---
                            let chars: Vec<char> = combined_content.chars().collect();
                            let mut i = 0;
                            while i < chars.len() {
                                if chars[i] == '{' {
                                    let mut brace_count = 0;
                                    let mut in_str = false;
                                    let mut esc = false;
                                    let start_idx = i;
                                    let mut end_idx = None;

                                    for j in i..chars.len() {
                                        let c = chars[j];
                                        if esc {
                                            esc = false;
                                            continue;
                                        }
                                        match c {
                                            '\\' => esc = true,
                                            '"' => in_str = !in_str,
                                            '{' if !in_str => {
                                                brace_count += 1;
                                            }
                                            '}' if !in_str => {
                                                brace_count -= 1;
                                                if brace_count == 0 {
                                                    end_idx = Some(j);
                                                    break;
                                                }
                                            }
                                            _ => {}
                                        }
                                    }

                                    if let Some(end_idx) = end_idx {
                                        let json_str: String = chars[start_idx..=end_idx].iter().collect();
                                        let json_str_repaired = crate::overwatch::repair_json_str(&json_str);
                                        if let Ok(val) = serde_json::from_str::<serde_json::Value>(&json_str_repaired) {
                                            if let Some(obj) = val.as_object() {
                                                let mut name_opt = obj.get("tool").or(obj.get("name")).or(obj.get("function")).or(obj.get("function_name")).and_then(|v| v.as_str());
                                                let mut args_opt = obj.get("arguments").or(obj.get("args")).or(obj.get("parameters")).cloned();

                                                let extracted_name = if name_opt.is_none() {
                                                    let prefix: String = chars[..start_idx].iter().collect();
                                                    if let Some(tool_comment_idx) = prefix.rfind("// Tool:") {
                                                        let comment_line = &prefix[tool_comment_idx..];
                                                        let name = if let Some(newline_idx) = comment_line.find('\n') {
                                                            comment_line["// Tool:".len()..newline_idx].trim()
                                                        } else {
                                                            comment_line["// Tool:".len()..].trim()
                                                        };
                                                        if !name.is_empty() {
                                                            Some(name.to_string())
                                                        } else { None }
                                                    } else { None }
                                                } else { None };

                                                if let Some(ref name_str) = extracted_name {
                                                    name_opt = Some(name_str);
                                                    args_opt = Some(val.clone());
                                                }

                                                if let Some(name) = name_opt {
                                                    let mut final_name = name.to_string();
                                                    if final_name.starts_with("__idx_") {
                                                        if let Some(end) = final_name[6..].find("__") {
                                                            let absolute_end = 6 + end;
                                                            final_name = final_name[absolute_end+2..].to_string();
                                                        }
                                                    }
                                                    native_tool_calls.push(ollama_rs::generation::tools::ToolCall {
                                                        function: ollama_rs::generation::tools::ToolCallFunction {
                                                            name: final_name,
                                                            arguments: args_opt.unwrap_or(serde_json::json!({})),
                                                        }
                                                    });
                                                }
                                            }
                                        }
                                        i = end_idx;
                                    }
                                }
                                i += 1;
                            }
                        }
                    }
                }

                // 🛡️ DEDUP: Remove duplicate tool calls that appeared both natively and in text
                {
                    let mut seen = std::collections::HashSet::new();
                    native_tool_calls.retain(|call| {
                        let key = format!("{}:{}", call.function.name, call.function.arguments);
                        seen.insert(key)
                    });
                }

                // Enforce [ACTOR PROTOCOL]: Truncate content after first tool call if found
                if !native_tool_calls.is_empty() {
                    if let Some(pos) = full_content.find('{') {
                        full_content.truncate(pos);
                    }
                }
            }
            Backend::Ollama(ollama) => {
                let model_lower = model.to_lowercase();
                let is_reasoning = model_lower.contains("r1") || model_lower.contains("deepseek") || model_lower.contains("deep-seek");
                
                let tx_opt = event_tx.lock().clone();
                if let Some(tx) = tx_opt {
                    let _ = tx.send(AgentEvent::Thinking(Some("Thinking...".to_string()))).await;
                }

                let mut stream = if is_reasoning {
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
                    let mut s = s.ok_or_else(|| miette!("Ollama raw stream failed: {:?}", last_err))?;
                    
                    // Wrap the generation stream to match the chat stream interface
                    Box::pin(async_stream::stream! {
                        while let Some(res) = s.next().await {
                            match res {
                                Ok(chunks) => {
                                    if let Some(chunk) = chunks.first() {
                                        yield Ok(ollama_rs::generation::chat::ChatMessageResponse {
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
                                        });
                                    }
                                }
                                Err(e) => {
                                    yield Err(miette!("Ollama Gen Stream Error: {}", e));
                                }
                            }
                        }
                    }) as std::pin::Pin<Box<dyn futures::Stream<Item = Result<ollama_rs::generation::chat::ChatMessageResponse, miette::Report>> + Send>>
                } else {
                    let mut request = ChatMessageRequest::new(model, history).options(options);
                    if let Some(registry) = tool_registry {
                        // DeepSeek R1/V3 on Ollama does not support native tools and throws 400
                        if !is_reasoning {
                            request = request.tools(registry);
                        }
                    }

                    let mut s = None;
                    for _attempt in 1..=3 {
                        match ollama.send_chat_messages_stream(request.clone()).await {
                            Ok(res) => {
                                s = Some(res);
                                break;
                            }
                            Err(e) => {
                                let tx_opt = event_tx.lock().clone();
                                if let Some(tx) = tx_opt {
                                    let _ = tx.send(AgentEvent::SystemUpdate(format!("⚠️ Ollama Error: {}. Retrying (attempt {}/3)...", e, _attempt))).await;
                                }
                                tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
                            }
                        }
                    }
                    let s = s.ok_or_else(|| miette!("Ollama chat stream failed"))?;
                    Box::pin(s.map(|res| res.map_err(|_| miette!("Ollama Stream Disconnected")))) as std::pin::Pin<Box<dyn futures::Stream<Item = Result<ollama_rs::generation::chat::ChatMessageResponse, miette::Report>> + Send>>
                };

                let mut is_thinking = is_reasoning;
                let mut first_token = true;
                let mut last_segments: Vec<String> = Vec::new();
                let mut last_thinking_segments: Vec<String> = Vec::new(); // 🛡️ Thinking-block repetition sentinel
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
                            let _ = tx.try_send(AgentEvent::TelemetryMetrics { cpu: None, gpu: None, ram: None, tps: Some(tps) });
                        }
                    }

                    if stop.load(std::sync::atomic::Ordering::Relaxed) { break; }
                    let chunk = res?;
                    
                    let mut got_native_thinking = false;
                    let mut received_any_token = false;
                    // Handle native thinking field from Ollama (DeepSeek R1)
                    if let Some(thinking) = &chunk.message.thinking {
                        if !thinking.is_empty() {
                            got_native_thinking = true;
                            received_any_token = true;
                            // Cap reasoning to prevent unbounded accumulation
                            if reasoning_content.len() < REASONING_CAP {
                                let space_left = REASONING_CAP - reasoning_content.len();
                                reasoning_content.push_str(&thinking[..space_left.min(thinking.len())]);
                            }
                            if let Some(tx) = event_tx.lock().clone() {
                                let _ = tx.try_send(AgentEvent::ReasoningToken(thinking.to_string()));
                            }
                            
                            // 🛡️ THINKING REPETITION SENTINEL
                            // Detect loops in the thinking block (e.g., the model repeating 
                            // "OVERRIDE" blocks 7+ times, which caused the catastrophe).
                            // Uses 50-char normalized segments for comparison.
                            let thinking_normalized = thinking.trim().to_lowercase();
                            if thinking_normalized.len() >= 50 {
                                let segment = thinking_normalized[..50].to_string();
                                last_thinking_segments.push(segment.clone());
                                if last_thinking_segments.len() > 12 {
                                    last_thinking_segments.remove(0);
                                }
                                // If any segment appears 6+ times in the window, the model is stuck
                                let max_repeats = last_thinking_segments.iter()
                                    .filter(|s| *s == &segment)
                                    .count();
                                if max_repeats >= 6 {
                                    // Inject a sentinel break into content and stop
                                    full_content = "⚠️ [SENTINEL - THINKING LOOP]: Repetition detected in reasoning block. \
                                        The model's internal reasoning is stuck in a loop. Breaking stream to prevent runaway.".to_string();
                                    if let Some(tx) = event_tx.lock().clone() {
                                        let _ = tx.try_send(AgentEvent::SubagentStatus(
                                            Some("🛑 Thinking Repetition Sentinel triggered".to_string())
                                        ));
                                    }
                                    break;
                                }
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
                                    "thinking...", "thinking", "hmm", "let me think", "i need to think"
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
                                    ["THOUGHT:", "PLAN:", "REASONING:", "ANALYSIS:", "[MISSION]"].iter()
                                        .filter_map(|&marker| {
                                            if marker == "[MISSION]" {
                                                upper.find(marker).map(|idx| (idx, marker.len()))
                                            } else {
                                                upper.find(marker).map(|idx| (idx, marker.len()))
                                            }
                                        })
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

                        // --- 🛡️ DEFAULT CASE: Normal Content ---
                        if !is_thinking && !in_thought_block {
                            let remaining = &text[current_pos..];
                            if !remaining.is_empty() {
                                full_content.push_str(remaining);
                                if let Some(tx) = event_tx.lock().clone() {
                                    let _ = tx.try_send(AgentEvent::StreamToken(remaining.to_string()));
                                }
                                current_pos = text.len();
                                received_any_token = true;
                            }
                        }

                        if is_thinking {
                            if let Some(end_idx) = text[current_pos..].find("</think>") {
                                // Reasoning before </think>
                                let reasoning = &text[current_pos..current_pos + end_idx];
                                if !got_native_thinking {
                                    // Cap reasoning to prevent unbounded accumulation
                                    if reasoning_content.len() < REASONING_CAP {
                                        let space_left = REASONING_CAP - reasoning_content.len();
                                        reasoning_content.push_str(&reasoning[..space_left.min(reasoning.len())]);
                                    }
                                    if let Some(tx) = event_tx.lock().clone() {
                                        let _ = tx.try_send(AgentEvent::ReasoningToken(reasoning.to_string()));
                                    }
                                }
                                is_thinking = false;
                                current_pos += end_idx + 8; // Skip "</think>"
                            } else if !is_reasoning && text[current_pos..].find("```json").is_some() {
                                // FORGETFUL MODEL RECOVERY: Non-reasoning models (like Qwen) forced to use <think>
                                // often forget to output </think> and dive straight into the tool call.
                                let json_idx = text[current_pos..].find("```json").unwrap();
                                let reasoning = &text[current_pos..current_pos + json_idx];
                                if !got_native_thinking {
                                    reasoning_content.push_str(reasoning);
                                    if let Some(tx) = event_tx.lock().clone() {
                                        let _ = tx.try_send(AgentEvent::ReasoningToken(reasoning.to_string()));
                                    }
                                }
                                is_thinking = false;
                                if let Some(tx) = event_tx.lock().clone() {
                                    let _ = tx.try_send(AgentEvent::Thinking(None));
                                }
                                current_pos += json_idx; // Don't skip ```json, it belongs to full_content
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
                                if let Some(tx) = event_tx.lock().clone() {
                                    let _ = tx.try_send(AgentEvent::Thinking(None));
                                }
                                current_pos += json_idx; // Don't skip ```json, it belongs to full_content
                            } else if let Some(mission_idx) = text[current_pos..].to_uppercase().find("[/MISSION]") {
                                let reasoning = &text[current_pos..current_pos + mission_idx];
                                reasoning_content.push_str(reasoning);
                                if let Some(tx) = event_tx.lock().clone() {
                                    let _ = tx.try_send(AgentEvent::ReasoningToken(reasoning.to_string()));
                                    let _ = tx.try_send(AgentEvent::Thinking(None));
                                }
                                in_thought_block = false;
                                current_pos += mission_idx + 10;
                            } else if let Some(think_idx) = text[current_pos..].to_uppercase().find("</THINK>") {
                                let reasoning = &text[current_pos..current_pos + think_idx];
                                reasoning_content.push_str(reasoning);
                                if let Some(tx) = event_tx.lock().clone() {
                                    let _ = tx.try_send(AgentEvent::ReasoningToken(reasoning.to_string()));
                                    let _ = tx.try_send(AgentEvent::Thinking(None));
                                }
                                in_thought_block = false;
                                current_pos += think_idx + 8;
                            } else if let Some(done_idx) = text[current_pos..].find("DONE:") {
                                let reasoning = &text[current_pos..current_pos + done_idx];
                                reasoning_content.push_str(reasoning);
                                if let Some(tx) = event_tx.lock().clone() {
                                    let _ = tx.try_send(AgentEvent::ReasoningToken(reasoning.to_string()));
                                    let _ = tx.try_send(AgentEvent::Thinking(None));
                                }
                                in_thought_block = false;
                                current_pos += done_idx;
                            } else if let Some(newline_idx) = text[current_pos..].find("\n\n") {
                                // Heuristic: If we see a double newline and then a transition phrase, end thoughts
                                let after_nl = &text[current_pos + newline_idx + 2..];
                                let lower = after_nl.to_lowercase();
                                let transitions = [
                                    "i will", "i'll", "i'm going to", "now", "starting", "let's begin",
                                    "here is", "i have", "first,", "certainly!", "certainly,", "hello!", "hi!", "okay,"
                                ];
                                
                                if transitions.iter().any(|&t| lower.starts_with(t)) {
                                    let reasoning = &text[current_pos..current_pos + newline_idx];
                                    reasoning_content.push_str(reasoning);
                                    if let Some(tx) = event_tx.lock().clone() {
                                        let _ = tx.try_send(AgentEvent::ReasoningToken(reasoning.to_string()));
                                    }
                                    in_thought_block = false;
                                    if let Some(tx) = event_tx.lock().clone() {
                                        let _ = tx.try_send(AgentEvent::Thinking(None));
                                    }
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

                            // Incorporate native tool call into full_content for consistent tracking
                            let json_val = serde_json::json!({
                                "tool": call.function.name,
                                "arguments": call.function.arguments
                            });
                            if let Ok(json_str) = serde_json::to_string(&json_val) {
                                if !full_content.ends_with('\n') && !full_content.is_empty() {
                                    full_content.push('\n');
                                }
                                full_content.push_str(&json_str);

                                // Stream the token to the UI
                                if let Some(tx) = event_tx.lock().clone() {
                                    let _ = tx.try_send(AgentEvent::StreamToken(format!("\n{}", json_str)));
                                }
                            }

                            native_tool_calls.push(call);
                        }
                    }
                    // --- 🛡️ REPETITION SENTINEL (Single-Token & Block-Level) ---
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

                    // 🛡️ BLOCK-LEVEL REPETITION GUARD (Checks accumulated full_content for line-repetition)
                    {
                        let mut line_counts = std::collections::HashMap::new();
                        let mut loop_detected = false;
                        for line in full_content.lines() {
                            let line_trimmed = line.trim();
                            if line_trimmed.len() >= 15 {
                                let count = line_counts.entry(line_trimmed).or_insert(0);
                                *count += 1;
                                if *count >= 6 {
                                    loop_detected = true;
                                    break;
                                }
                            }
                        }
                        if loop_detected {
                            let warning = "\n\n⚠️ [REPETITION SENTINEL - BLOCK LOOP]: Breaking loop. Destructive block repetition detected in stream.";
                            full_content.push_str(warning);
                            let tx = event_tx.lock().clone();
                            if let Some(tx) = tx {
                                let _ = tx.send(AgentEvent::StreamToken(warning.to_string())).await;
                            }
                            break;
                        }
                    }

                    // 🛑 HARD STOP / YIELD: Truncate generation the millisecond the JSON tool is complete
                    if crate::overwatch::is_complete_tool_json(&full_content) {
                        let tx = event_tx.lock().clone();
                        if let Some(tx) = tx {
                            let _ = tx.try_send(AgentEvent::SystemUpdate("🛑 Hard Stop: Tool call detected, yielding execution.".to_string()));
                        }
                        break;
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
                let mut last_thinking_segments: Vec<String> = Vec::new(); // 🛡️ Thinking-block repetition sentinel
                let mut token_count = 0;
                let start_time = std::time::Instant::now();
                
                while let Some(response) = stream.next().await {
                    token_count += 1;
                    let elapsed = start_time.elapsed().as_secs_f64();
                    if elapsed > 0.1 {
                        let tps = (token_count as f64 / elapsed) as u64;
                        if let Some(tx) = event_tx.lock().clone() {
                            let _ = tx.try_send(AgentEvent::TelemetryMetrics { cpu: None, gpu: None, ram: None, tps: Some(tps) });
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

                                    // Incorporate native tool call into full_content for consistent tracking
                                    let json_val = serde_json::json!({
                                        "tool": mapped_call.function.name,
                                        "arguments": mapped_call.function.arguments
                                    });
                                    if let Ok(json_str) = serde_json::to_string(&json_val) {
                                        if !full_content.ends_with('\n') && !full_content.is_empty() {
                                            full_content.push('\n');
                                        }
                                        full_content.push_str(&json_str);

                                        // Stream the token to the UI
                                        if let Some(tx) = event_tx.lock().clone() {
                                            let _ = tx.try_send(AgentEvent::StreamToken(format!("\n{}", json_str)));
                                        }
                                    }

                                    native_tool_calls.push(mapped_call);
                                }
                            }

                            if let Some(reasoning) = &chunk.choices[0].delta.reasoning_content {
                                if !reasoning.is_empty() {
                                    // 🛡️ THINKING REPETITION SENTINEL (MLX)
                                    let thinking_normalized = reasoning.trim().to_lowercase();
                                    if thinking_normalized.len() >= 50 {
                                        let segment = thinking_normalized[..50].to_string();
                                        last_thinking_segments.push(segment.clone());
                                        if last_thinking_segments.len() > 12 {
                                            last_thinking_segments.remove(0);
                                        }
                                        let max_repeats = last_thinking_segments.iter()
                                            .filter(|s| *s == &segment)
                                            .count();
                                        if max_repeats >= 6 {
                                            full_content = "⚠️ [SENTINEL - THINKING LOOP]: Repetition detected in reasoning block. \
                                                Breaking stream to prevent runaway.".to_string();
                                            if let Some(tx) = event_tx.lock().clone() {
                                                let _ = tx.try_send(AgentEvent::SubagentStatus(
                                                    Some("🛑 Thinking Repetition Sentinel triggered".to_string())
                                                ));
                                            }
                                            break;
                                        }
                                    }

                                    // CAP REASONING: Prevent unbounded accumulation (20KB max ≈ 5000 tokens)
                                    let reasoning_cap = 20480;
                                    if reasoning_content.len() + reasoning.len() <= reasoning_cap {
                                        reasoning_content.push_str(reasoning);
                                    } else if reasoning_content.len() < reasoning_cap {
                                        // Fill to capacity, then stop
                                        let remaining = reasoning_cap - reasoning_content.len();
                                        reasoning_content.push_str(&reasoning[..remaining.min(reasoning.len())]);
                                    }
                                    
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
                                let mut text = std::mem::take(&mut tag_residue);
                                text.push_str(content);

                                let mut current_pos = 0;
                                
                                if first_token && !text.trim().is_empty() {
                                    let lower = text.trim().to_lowercase();
                                    let implicit_phrases = [
                                        "thinking...", "thinking", "hmm", "let me think", "i need to think"
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
                                            ["THOUGHT:", "PLAN:", "REASONING:", "ANALYSIS:", "[MISSION]"].iter()
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
                                        } else if let Some(json_idx) = text[current_pos..].find("{\"tool\":") {
                                            // FAILSAFE: Implicit end of thinking block via raw tool call JSON
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
                                            if let Some(tx) = event_tx.lock().clone() {
                                                let _ = tx.try_send(AgentEvent::Thinking(None));
                                            }
                                            current_pos += json_idx;
                                        } else if let Some(mission_idx) = text[current_pos..].to_uppercase().find("[/MISSION]") {
                                            let reasoning = &text[current_pos..current_pos + mission_idx];
                                            reasoning_content.push_str(reasoning);
                                            if let Some(tx) = event_tx.lock().clone() {
                                                let _ = tx.try_send(AgentEvent::ReasoningToken(reasoning.to_string()));
                                                let _ = tx.try_send(AgentEvent::Thinking(None));
                                            }
                                            in_thought_block = false;
                                            current_pos += mission_idx + 10;
                                        } else if let Some(done_idx) = text[current_pos..].find("DONE:") {
                                            let reasoning = &text[current_pos..current_pos + done_idx];
                                            reasoning_content.push_str(reasoning);
                                            if let Some(tx) = event_tx.lock().clone() {
                                                let _ = tx.try_send(AgentEvent::ReasoningToken(reasoning.to_string()));
                                                let _ = tx.try_send(AgentEvent::Thinking(None));
                                            }
                                            in_thought_block = false;
                                            current_pos += done_idx;
                                        } else if let Some(newline_idx) = text[current_pos..].find("\n\n") {
                                            // Heuristic: Transition from thought to message (MLX)
                                            let after_nl = &text[current_pos + newline_idx + 2..];
                                            let lower = after_nl.to_lowercase();
                                            let transitions = [
                                                "i will", "i'll", "i'm going to", "now", "starting", "let's begin",
                                                "here is", "i have", "first,", "certainly!", "certainly,", "hello!", "hi!", "okay,"
                                            ];
                                            
                                            if transitions.iter().any(|&t| lower.starts_with(t)) {
                                                let reasoning = &text[current_pos..current_pos + newline_idx];
                                                reasoning_content.push_str(reasoning);
                                                if let Some(tx) = event_tx.lock().clone() {
                                                    let _ = tx.try_send(AgentEvent::ReasoningToken(reasoning.to_string()));
                                                    let _ = tx.try_send(AgentEvent::SubagentStatus(Some(format!("⚡ MLX Engine: Reasoning... ({} chars)", reasoning_content.len()))));
                                                }
                                                in_thought_block = false;
                                                if let Some(tx) = event_tx.lock().clone() {
                                                    let _ = tx.try_send(AgentEvent::Thinking(None));
                                                }
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

                                    // 🛡️ BLOCK-LEVEL REPETITION GUARD (Checks accumulated full_content for line-repetition)
                                    {
                                        let mut line_counts = std::collections::HashMap::new();
                                        let mut loop_detected = false;
                                        for line in full_content.lines() {
                                            let line_trimmed = line.trim();
                                            if line_trimmed.len() >= 15 {
                                                let count = line_counts.entry(line_trimmed).or_insert(0);
                                                *count += 1;
                                                if *count >= 6 {
                                                    loop_detected = true;
                                                    break;
                                                }
                                            }
                                        }
                                        if loop_detected {
                                            let warning = "\n\n⚠️ [REPETITION SENTINEL - BLOCK LOOP]: Breaking loop. Destructive block repetition detected in stream.";
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
                            
                            // 🛑 HARD STOP / YIELD: Truncate generation the millisecond the JSON tool is complete
                            if crate::overwatch::is_complete_tool_json(&full_content) {
                                let tx = event_tx.lock().clone();
                                if let Some(tx) = tx {
                                    let _ = tx.try_send(AgentEvent::SystemUpdate("🛑 Hard Stop: Tool call detected, yielding execution.".to_string()));
                                }
                                break;
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

            let mut extract_success = false;
            if let Ok(payloads) = llm_extract::extract_all::<ToolCallPayload>(&combined_content) {
                if !payloads.is_empty() {
                    extract_success = true;
                    for payload in payloads {
                        let mut final_name = payload.name;
                        if final_name.starts_with("__idx_") {
                            if let Some(end) = final_name[6..].find("__") {
                                let absolute_end = 6 + end;
                                final_name = final_name[absolute_end+2..].to_string();
                            }
                        }
                        native_tool_calls.push(ollama_rs::generation::tools::ToolCall {
                            function: ollama_rs::generation::tools::ToolCallFunction {
                                name: final_name,
                                arguments: payload.arguments,
                            }
                        });
                    }
                }
            }
            
            if !extract_success {
                // --- 🛡️ FALLBACK: tool-parser (v1.2.0) with DeepSeekParser ---
                // If llm-extract fails, try specialized DeepSeekParser for multi-block recovery
                let parser = tool_parser::DeepSeekParser::new();
                let mut parsed_success = false;
                if let Ok((_text, calls)) = parser.parse_complete(&combined_content).await {
                    if !calls.is_empty() {
                        parsed_success = true;
                        for call in calls {
                            let mut final_name = call.function.name;
                            if final_name.starts_with("__idx_") {
                                if let Some(end) = final_name[6..].find("__") {
                                    let absolute_end = 6 + end;
                                    final_name = final_name[absolute_end+2..].to_string();
                                }
                            }
                            native_tool_calls.push(ollama_rs::generation::tools::ToolCall {
                                function: ollama_rs::generation::tools::ToolCallFunction {
                                    name: final_name,
                                    arguments: serde_json::from_str(&call.function.arguments).unwrap_or(serde_json::json!({})),
                                }
                            });
                        }
                    }
                }

                if !parsed_success {
                    // --- 🛡️ EXTRA FALLBACK: Robust brace counting extraction from test_json_stop.rs ---
                    let chars: Vec<char> = combined_content.chars().collect();
                    let mut i = 0;
                    while i < chars.len() {
                        if chars[i] == '{' {
                            let mut brace_count = 0;
                            let mut in_string = false;
                            let mut escape = false;
                            let start_idx = i;
                            let mut end_idx = i;

                            let mut j = i;
                            while j < chars.len() {
                                let c = chars[j];
                                if !escape && c == '"' {
                                    in_string = !in_string;
                                }
                                if !in_string && !escape {
                                    if c == '{' { brace_count += 1; }
                                    else if c == '}' { brace_count -= 1; }
                                }

                                if c == '\\' {
                                    escape = !escape;
                                } else {
                                    escape = false;
                                }

                                if brace_count == 0 {
                                    end_idx = j;
                                    break;
                                }
                                j += 1;
                            }

                            if brace_count == 0 {
                                let json_str: String = chars[start_idx..=end_idx].iter().collect();
                                let json_str_repaired = crate::overwatch::repair_json_str(&json_str);
                                if let Ok(val) = serde_json::from_str::<serde_json::Value>(&json_str_repaired) {
                                    if let Some(obj) = val.as_object() {
                                        let name_opt = obj.get("tool")
                                            .or_else(|| obj.get("name"))
                                            .or_else(|| obj.get("function"))
                                            .or_else(|| obj.get("function_name"))
                                            .or_else(|| obj.get("action"))
                                            .and_then(|v| v.as_str())
                                            .map(|s| s.to_string());

                                        let args_opt = obj.get("arguments")
                                            .or_else(|| obj.get("params"))
                                            .or_else(|| obj.get("args"))
                                            .cloned();

                                        if let Some(name) = name_opt {
                                            let mut final_name = name.clone();
                                            if final_name.starts_with("__idx_") {
                                                if let Some(end) = final_name[6..].find("__") {
                                                    let absolute_end = 6 + end;
                                                    final_name = final_name[absolute_end+2..].to_string();
                                                }
                                            }
                                            native_tool_calls.push(ollama_rs::generation::tools::ToolCall {
                                                function: ollama_rs::generation::tools::ToolCallFunction {
                                                    name: final_name,
                                                    arguments: args_opt.unwrap_or(serde_json::json!({})),
                                                }
                                            });
                                        }
                                    }
                                }
                                i = end_idx;
                            }
                        }
                        i += 1;
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
            Backend::Kalosm { .. } => {}
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
            Backend::MLX { embedder, ollama_fallback, .. } => {
                if let Some(embed) = embedder {
                    let request = EmbeddingRequest::builder().add_prompt(format!("passage: {}", text));
                    let res = embed.generate_embeddings(request).await
                        .map_err(|e| miette!("MLX embedding failed: {}", e))?;
                    Ok(res.first().cloned().unwrap_or_default())
                } else if let Some(ollama) = ollama_fallback {
                    let req = ollama_rs::generation::embeddings::request::GenerateEmbeddingsRequest::new(
                        "all-minilm".to_string(), 
                        text.to_string().into()
                    );
                    let res = ollama.generate_embeddings(req).await
                        .map_err(|e| miette!("Ollama embedding fallback failed: {}", e))?;
                    Ok(res.embeddings.first().cloned().unwrap_or_default())
                } else {
                    Err(miette!("MLX Embedder not loaded and no Ollama fallback available"))
                }
            }
            Backend::Bridge(bridge) => {
                bridge.generate_embeddings(text.to_string()).await
            }
            Backend::Kalosm { .. } => {
                Err(miette!("Embeddings not yet supported via Kalosm"))
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

fn normalize_history(history: &[ChatMessage]) -> Vec<ChatMessage> {
    if history.is_empty() {
        return Vec::new();
    }

    let mut normalized = Vec::with_capacity(history.len());
    
    // 1. Process roles and convert mid-history System/Tool messages to User
    for (idx, msg) in history.iter().enumerate() {
        let role = match &msg.role {
            MessageRole::System => {
                if idx == 0 {
                    MessageRole::System // Keep the initial system instructions
                } else {
                    MessageRole::User // Convert mid-history system observations to User
                }
            }
            MessageRole::Tool => MessageRole::User, // Convert tool results to User for universal compatibility
            other => other.clone(),
        };

        // Format tool calls back into the assistant message if present!
        let mut content = msg.content.clone();
        if role == MessageRole::Assistant && !msg.tool_calls.is_empty() {
            for call in &msg.tool_calls {
                let json_val = serde_json::json!({
                    "tool": call.function.name,
                    "arguments": call.function.arguments
                });
                if let Ok(json_str) = serde_json::to_string(&json_val) {
                    if !content.ends_with('\n') && !content.is_empty() {
                        content.push('\n');
                    }
                    content.push_str(&json_str);
                }
            }
        }

        normalized.push(ChatMessage {
            role,
            content,
            images: msg.images.clone(),
            tool_calls: msg.tool_calls.clone(),
            thinking: msg.thinking.clone(),
        });
    }

    // 2. Merge consecutive messages of the same role to enforce strict alternation
    let mut merged: Vec<ChatMessage> = Vec::with_capacity(normalized.len());
    for msg in normalized {
        if let Some(last) = merged.last_mut() {
            if last.role == msg.role {
                // Merge content
                if !last.content.is_empty() && !msg.content.is_empty() {
                    last.content.push_str("\n\n");
                }
                last.content.push_str(&msg.content);

                // Merge tool calls
                for tc in msg.tool_calls {
                    if !last.tool_calls.iter().any(|existing| existing.function.name == tc.function.name && existing.function.arguments == tc.function.arguments) {
                        last.tool_calls.push(tc);
                    }
                }

                // Merge thinking
                if let Some(think) = msg.thinking {
                    if let Some(ref mut last_think) = last.thinking {
                        last_think.push_str("\n");
                        last_think.push_str(&think);
                    } else {
                        last.thinking = Some(think);
                    }
                }
                continue;
            }
        }
        merged.push(msg);
    }

    merged
}
