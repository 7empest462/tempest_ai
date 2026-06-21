use crate::inference::{Backend, SamplingConfig};
use miette::Result;
use ollama_rs::generation::chat::{ChatMessage, MessageRole};
use ollama_rs::models::ModelOptions;
use std::sync::Arc;

// SKG types
use skg_context_engine::{Context, Rule};

pub struct ContextLimit(pub usize);

/// Count tokens for a single string using the cached BPE tokenizer.
pub fn count_tokens(text: &str) -> usize {
    let enc = tiktoken::get_encoding("qwen2")
        .or(tiktoken::get_encoding("deepseek_v3"))
        .unwrap_or(tiktoken::get_encoding("cl100k_base").unwrap());
    enc.count(text)
}

/// Counts tokens using tiktoken's real BPE tokenizer (qwen2 encoding).
pub fn estimate_tokens(messages: &[ChatMessage]) -> usize {
    let mut total = 0;
    for msg in messages {
        total += count_tokens(&msg.content);
        total += 4;
    }
    total
}

/// Estimate tokens on layer0 messages
pub fn estimate_tokens_layer0(messages: &[layer0::context::Message]) -> usize {
    let mut total = 0;
    for msg in messages {
        total += count_tokens(&msg.text_content());
        total += 4;
    }
    total
}

/// Helper mapping from ChatMessage to layer0 Message
pub fn to_layer0_messages(messages: &[ChatMessage]) -> Vec<layer0::context::Message> {
    messages
        .iter()
        .map(|msg| {
            let role = match msg.role {
                MessageRole::System => layer0::context::Role::System,
                MessageRole::User => layer0::context::Role::User,
                MessageRole::Assistant => layer0::context::Role::Assistant,
                MessageRole::Tool => layer0::context::Role::User, // Map to User for generic rule processing
            };
            let content = if msg.role == MessageRole::Tool {
                format!("[TOOL RESULT]: {}", msg.content)
            } else {
                msg.content.clone()
            };
            layer0::context::Message::new(role, layer0::content::Content::text(content))
        })
        .collect()
}

/// Helper mapping from layer0 Message to ChatMessage
pub fn to_chat_messages(messages: &[layer0::context::Message]) -> Vec<ChatMessage> {
    messages
        .iter()
        .map(|msg| {
            let role = match msg.role {
                layer0::context::Role::System => MessageRole::System,
                layer0::context::Role::User => MessageRole::User,
                layer0::context::Role::Assistant => MessageRole::Assistant,
                _ => MessageRole::User,
            };
            let mut content = msg.text_content().to_string();
            let final_role = if content.starts_with("[TOOL RESULT]: ") {
                content = content["[TOOL RESULT]: ".len()..].to_string();
                MessageRole::Tool
            } else {
                role
            };
            ChatMessage {
                role: final_role,
                content,
                images: None,
                tool_calls: Vec::new(),
                thinking: None,
            }
        })
        .collect()
}

/// Returns true if the estimated token count exceeds the trigger threshold.
pub fn needs_compaction(messages: &[ChatMessage], ctx_limit: u64) -> bool {
    let estimated = estimate_tokens(messages);
    let focal_cap = 20000;
    let threshold = ((ctx_limit as f64 * 0.85) as usize).min(focal_cap);
    estimated > threshold
}

// ==========================================
// 🔌 SKG PRE-INFERENCE CONTEXT OPERATIONS
// ==========================================

pub struct RunwayReportOp;

#[async_trait::async_trait]
impl skg_context_engine::ContextOp for RunwayReportOp {
    type Output = ();

    async fn execute(
        &self,
        ctx: &mut skg_context_engine::Context,
    ) -> std::result::Result<(), skg_context_engine::EngineError> {
        let limit = ctx
            .extensions
            .get::<ContextLimit>()
            .map(|l| l.0)
            .unwrap_or(20000);
        let current_tokens = estimate_tokens_layer0(&ctx.messages);
        let report = generate_runway_report(current_tokens, limit as u64);

        if !ctx.messages.is_empty() && ctx.messages[0].role == layer0::context::Role::System {
            let mut content = ctx.messages[0].text_content().to_string();
            if let Some(pos) = content.find("\n\n[SESSION RUNWAY STATUS]") {
                content.truncate(pos);
            }
            content.push_str("\n\n");
            content.push_str(&report);
            ctx.messages[0].content = layer0::content::Content::text(content);
        }
        Ok(())
    }
}

pub struct VectorCompactionOp {
    pub backend: Backend,
    pub sub_model: String,
    pub vector_brain: Arc<parking_lot::Mutex<crate::vector_brain::VectorBrain>>,
    pub brain_path: std::path::PathBuf,
}

#[async_trait::async_trait]
impl skg_context_engine::ContextOp for VectorCompactionOp {
    type Output = ();

    async fn execute(
        &self,
        ctx: &mut skg_context_engine::Context,
    ) -> std::result::Result<(), skg_context_engine::EngineError> {
        let limit = ctx
            .extensions
            .get::<ContextLimit>()
            .map(|l| l.0)
            .unwrap_or(20000);
        let current_tokens = estimate_tokens_layer0(&ctx.messages);

        let focal_cap = 20000;
        let threshold = ((limit as f64 * 0.85) as usize).min(focal_cap);

        if current_tokens <= threshold {
            return Ok(());
        }

        let pressure_pct = (current_tokens as f64 / limit as f64 * 100.0).min(100.0);

        let (ratio, intensity) = if pressure_pct > 90.0 {
            (
                "10x",
                "CRITICAL: Maximum density required. Drop all conversational filler. Preserve ONLY raw technical facts, paths, and tool results.",
            )
        } else {
            (
                "5x",
                "Standard density. Distill history while retaining technical context and logic flow.",
            )
        };

        if ctx.messages.len() <= 6 {
            return Ok(());
        }

        // Zone Protection:
        // Index 0: System Prompt + Schema (PROTECTED)
        // Index 1 & 2: Original user prompts (PROTECTED)
        // Last 6 messages: Active working context (PROTECTED)
        let head_size = 3.min(ctx.messages.len() - 1);
        let tail_size = 6.min(ctx.messages.len() - head_size - 1);

        let compaction_target_range = head_size..(ctx.messages.len() - tail_size);
        let target_messages: Vec<layer0::context::Message> =
            ctx.messages[compaction_target_range.clone()].to_vec();

        // 🧠 SEMANTIC VECTOR INDEXING
        {
            let mut current_chunk = String::new();
            let mut chunks_to_embed = Vec::new();
            for msg in &target_messages {
                let role = match msg.role {
                    layer0::context::Role::User => "User",
                    layer0::context::Role::Assistant => "Assistant",
                    layer0::context::Role::System => "System",
                    _ => "Other",
                };
                current_chunk.push_str(&format!("{}: {}\n\n", role, msg.text_content()));

                if msg.role == layer0::context::Role::Assistant || current_chunk.len() > 1200 {
                    let chunk_text = current_chunk.trim().to_string();
                    if !chunk_text.is_empty() {
                        chunks_to_embed.push(chunk_text);
                    }
                    current_chunk.clear();
                }
            }

            let chunk_text = current_chunk.trim().to_string();
            if !chunk_text.is_empty() {
                chunks_to_embed.push(chunk_text);
            }

            for chunk in chunks_to_embed {
                if let Ok(embedding) = self.backend.generate_embeddings(&chunk).await {
                    let mut brain = self.vector_brain.lock();
                    brain.add_entry(
                        chunk,
                        embedding,
                        "context_compaction".to_string(),
                        std::collections::HashMap::new(),
                    );
                }
            }

            let brain = self.vector_brain.lock();
            let _ = brain.save_to_disk(&self.brain_path);
        }

        // Construct summarization prompt
        let mut summary_context = String::new();
        for msg in &target_messages {
            let role = match msg.role {
                layer0::context::Role::User => "User",
                layer0::context::Role::Assistant => "Assistant",
                layer0::context::Role::System => "System",
                _ => "Other",
            };
            summary_context.push_str(&format!("{}: {}\n\n", role, msg.text_content()));
        }

        let summary_prompt = format!(
            "### TASK: CONTEXT DENSITY COMPACTION (Pressure: {:.1}%)\n\
            You are a high-speed context compressor. Summarize the following session history.\n\n\
            ### DENSITY TARGET: {}\n\
            {}\n\n\
            ### RULES:\n\
            1. OUTPUT DENSITY: Achieve the target ratio above.\n\
            2. TECHNICAL PRESERVATION: Retain all tool calls, file paths, PIDs, and error codes.\n\
            3. NO LOGORRHEA: Do NOT use phrases like 'The user and assistant discussed...'.\n\
            4. NO VERBATIM: Do not repeat long blocks of text.\n\
            5. NO PREAMBLE: Start immediately with the summary.\n\
            \n### CONVERSATION STREAM:\n{}",
            pressure_pct, ratio, intensity, summary_context
        );

        let summary_text = match &self.backend {
            Backend::Ollama(ollama, _) => {
                let options = ModelOptions::default()
                    .temperature(0.01)
                    .top_p(0.9)
                    .repeat_penalty(1.1)
                    .num_ctx(4096);
                let mut coordinator = ollama_rs::coordinator::Coordinator::new(
                    ollama.clone(),
                    self.sub_model.to_string(),
                    vec![],
                )
                .options(options)
                .think(ollama_rs::generation::parameters::ThinkType::Low);

                let chat_fut =
                    coordinator.chat(vec![ChatMessage::new(MessageRole::User, summary_prompt)]);
                match tokio::time::timeout(tokio::time::Duration::from_secs(30), chat_fut).await {
                    Ok(Ok(response)) => {
                        let mut text = response.message.content;
                        if let Some(thinking) = response.message.thinking
                            && !thinking.is_empty()
                        {
                            text = format!("<think>\n{}\n</think>\n{}", thinking, text);
                        }
                        Some(text)
                    }
                    _ => None,
                }
            }
            _ => {
                let sampling = SamplingConfig {
                    temperature: 0.01,
                    top_p: 0.9,
                    repeat_penalty: 1.1,
                    context_size: 4096,
                };
                let stop = Arc::new(std::sync::atomic::AtomicBool::new(false));
                let event_tx = Arc::new(parking_lot::Mutex::new(None));
                let cloned_backend = self.backend.clone();
                let chat_fut = cloned_backend.stream_chat(crate::inference::ChatRequest {
                    model: self.sub_model.to_string(),
                    history: vec![ChatMessage::new(MessageRole::User, summary_prompt)],
                    sampling,
                    event_tx,
                    stop,
                    system_prompt: "".to_string(),
                    on_tool_call: None,
                    tool_registry: None,
                });
                match tokio::time::timeout(tokio::time::Duration::from_secs(30), chat_fut).await {
                    Ok(Ok(response)) => Some(response.content),
                    _ => None,
                }
            }
        };

        match summary_text {
            Some(summary_text) => {
                let summary_message = layer0::context::Message::new(
                    layer0::context::Role::System,
                    layer0::content::Content::text(format!(
                        "[CONTEXT SUMMARY - COMPACTED]:\n{}",
                        summary_text
                    )),
                );
                ctx.messages
                    .splice(compaction_target_range, std::iter::once(summary_message));
            }
            None => {
                // Determine target range budget:
                let total_used = estimate_tokens_layer0(&ctx.messages);
                let target_used = estimate_tokens_layer0(&target_messages);
                let reserved_tokens = total_used.saturating_sub(target_used);
                let remaining_budget = if threshold > reserved_tokens {
                    threshold - reserved_tokens
                } else {
                    1000 // absolute minimum safety buffer
                };

                let config = skg_context::SaliencePackingConfig {
                    token_budget: remaining_budget,
                    lambda: 0.7,
                    default_salience: 0.5,
                    reorder_for_recall: true, // "lost in the middle" reordering
                    chars_per_token: 4,
                };

                let mut compactor = skg_context::salience_packing_compactor(config);
                let compacted_target = compactor(&target_messages);

                ctx.messages
                    .splice(compaction_target_range, compacted_target);

                let panic_message = layer0::context::Message::new(
                    layer0::context::Role::System,
                    layer0::content::Content::text("⚠️ [CRITICAL OVERLOAD]: Summarization model error or timeout (30s). Old history mathematically compacted using salience packing to restore stability.".to_string()),
                );
                ctx.messages.insert(head_size, panic_message);
            }
        }

        Ok(())
    }
}

/// Compacts the middle of the history by summarizing it using the sub-agent model,
/// running the skg-context-engine pipeline natively under the hood.
pub async fn compact_history(
    backend: &Backend,
    sub_model: &str,
    messages: Vec<ChatMessage>,
    ctx_limit: u64,
    vector_brain: &Arc<parking_lot::Mutex<crate::vector_brain::VectorBrain>>,
    brain_path: &std::path::Path,
) -> Result<Vec<ChatMessage>> {
    let layer0_msgs = to_layer0_messages(&messages);
    let mut ctx = Context::new();
    ctx.messages = layer0_msgs;
    ctx.extensions.insert(ContextLimit(ctx_limit as usize));

    // Register runway monitor rule
    let runway_rule = Rule::when("Context Runway Monitor", 100, |_| true, RunwayReportOp);
    ctx.add_rule(runway_rule);

    // Run compaction operator
    let compaction_op = VectorCompactionOp {
        backend: backend.clone(),
        sub_model: sub_model.to_string(),
        vector_brain: Arc::clone(vector_brain),
        brain_path: brain_path.to_path_buf(),
    };

    match ctx.run(compaction_op).await {
        Ok(_) => Ok(to_chat_messages(&ctx.messages)),
        Err(e) => Err(miette::miette!(
            "SKG context compaction pipeline failed: {:?}",
            e
        )),
    }
}

/// Generates a status report for the agent to understand its remaining context "runway".
pub fn generate_runway_report(used: usize, total: u64) -> String {
    let runway_pct = if total > 0 {
        ((total as f64 - used as f64) / total as f64) * 100.0
    } else {
        0.0
    };

    let emoji = if runway_pct > 50.0 {
        "🟢"
    } else if runway_pct > 20.0 {
        "🟡"
    } else {
        "🔴"
    };

    let instruction = if runway_pct > 20.0 {
        "Continue with normal depth and research. Your mental runway is clear."
    } else if runway_pct > 10.0 {
        "CAUTION: Context window is becoming dense. Please begin consolidating your thoughts and provide more focused, concise answers."
    } else {
        "URGENT: Context window is nearly full. Please prioritize providing the final solution or summary for the user immediately before more turns are taken."
    };

    format!(
        "\n[SESSION RUNWAY STATUS] {}\n- Used: {} tokens\n- Limit: {} tokens\n- Remaining: {:.1}%\n\nDIRECTIVE: {}",
        emoji, used, total, runway_pct, instruction
    )
}
