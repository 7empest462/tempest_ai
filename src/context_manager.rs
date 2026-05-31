use ollama_rs::generation::chat::ChatMessage;
use miette::Result;
use crate::inference::{Backend, SamplingConfig};
use std::sync::Arc;

/// Count tokens for a single string using the cached BPE tokenizer.
fn count_tokens(text: &str) -> usize {
    // tiktoken::get_encoding returns &'static CoreBpe, so no caching needed
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
        // Per-message overhead: role token + structural delimiters
        total += 4;
    }
    total
}

/// Returns true if the estimated token count exceeds the trigger threshold.
/// Incorporates a hard focal cap of 20,000 tokens for local models to prevent
/// "lost in the middle" reasoning degradation and maximize prompt processing speeds.
pub fn needs_compaction(messages: &[ChatMessage], ctx_limit: u64) -> bool {
    let estimated = estimate_tokens(messages);
    let focal_cap = 20000;
    let threshold = ((ctx_limit as f64 * 0.85) as usize).min(focal_cap);
    estimated > threshold
}

use ollama_rs::generation::chat::MessageRole;

/// Compacts the middle of the history by summarizing it using the sub-agent model,
/// and simultaneously indexing granular conversation turns into the semantic VectorBrain.
pub async fn compact_history(
    backend: &Backend,
    sub_model: &str,
    mut messages: Vec<ChatMessage>,
    _ctx_limit: u64,
    vector_brain: &Arc<parking_lot::Mutex<crate::vector_brain::VectorBrain>>,
    brain_path: &std::path::Path,
) -> Result<Vec<ChatMessage>> {
    let initial_count = estimate_tokens(&messages);
    let pressure_pct = (initial_count as f64 / _ctx_limit as f64 * 100.0).min(100.0);
    
    // Determine compression intensity based on pressure
    let (ratio, intensity) = if pressure_pct > 90.0 {
        ("10x", "CRITICAL: Maximum density required. Drop all conversational filler. Preserve ONLY raw technical facts, paths, and tool results.")
    } else {
        ("5x", "Standard density. Distill history while retaining technical context and logic flow.")
    };

    if messages.len() <= 6 {
        return Ok(messages); 
    }

    // Zone Protection:
    // Index 0: System Prompt + Schema (PROTECTED)
    // Last 6 messages: Active working context (PROTECTED)
    let tail_size = 6.min(messages.len() - 1);
    let head_size = 1;
    
    let compaction_target_range = head_size..(messages.len() - tail_size);
    let target_messages: Vec<ChatMessage> = messages[compaction_target_range.clone()].to_vec();

    // 🧠 SEMANTIC VECTOR INDEXING
    // Group targets into User-Assistant conversation chunks and embed them in the VectorBrain
    {
        let mut current_chunk = String::new();
        let mut chunks_to_embed = Vec::new();
        for msg in &target_messages {
            let role = match msg.role {
                MessageRole::User => "User",
                MessageRole::Assistant => "Assistant",
                MessageRole::System => "System",
                _ => "Other",
            };
            current_chunk.push_str(&format!("{}: {}\n\n", role, msg.content));
            
            // Slice when we reach the end of an Assistant turn or if the text gets large
            if msg.role == MessageRole::Assistant || current_chunk.len() > 1200 {
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
        
        // Generate embeddings and store them (acquiring and dropping the MutexGuard between await boundaries)
        for chunk in chunks_to_embed {
            if let Ok(embedding) = backend.generate_embeddings(&chunk).await {
                let mut brain = vector_brain.lock();
                brain.add_entry(
                    chunk,
                    embedding,
                    "context_compaction".to_string(),
                    std::collections::HashMap::new(),
                );
            }
        }
        
        // Save the updated VectorBrain to disk
        let brain = vector_brain.lock();
        let _ = brain.save_to_disk(brain_path);
    }
    
    // Construct the summarization request
    let mut summary_context = String::new();
    for msg in &target_messages {
        let role = match msg.role {
            MessageRole::User => "User",
            MessageRole::Assistant => "Assistant",
            MessageRole::System => "System",
            _ => "Other",
        };
        summary_context.push_str(&format!("{}: {}\n\n", role, msg.content));
    }

    let summary_prompt = format!(
        "### TASK: CONTEXT DENSITY COMPACTION (Pressure: {:.1}%)
    You are a high-speed context compressor. Summarize the following session history.
    
    ### DENSITY TARGET: {}
    {}
    
    ### RULES:
    1. OUTPUT DENSITY: Achieve the target ratio above.
    2. TECHNICAL PRESERVATION: Retain all tool calls, file paths, PIDs, and error codes.
    3. NO LOGORRHEA: Do NOT use phrases like 'The user and assistant discussed...'. 
    4. NO VERBATIM: Do not repeat long blocks of text.
    5. NO PREAMBLE: Start immediately with the summary.
    
    ### CONVERSATION STREAM:
    {}
    ",
        pressure_pct, ratio, intensity, summary_context
    );

    let sampling = SamplingConfig {
        temperature: 0.01,
        top_p: 0.9,
        repeat_penalty: 1.1,
        context_size: 4096,
    };

    let stop = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let event_tx = Arc::new(parking_lot::Mutex::new(None));

    // If we are at critical capacity, try to summarize, but fallback to Hard-Prune if it fails or is too slow
    let summary_result = tokio::time::timeout(
        tokio::time::Duration::from_secs(30),
        backend.stream_chat(
            sub_model.to_string(),
            vec![ChatMessage::new(MessageRole::User, summary_prompt)],
            sampling,
            event_tx,
            stop,
            "".to_string(),
            None,
            None,
        )
    ).await;

    match summary_result {
        Ok(Ok(response)) => {
            let summary_text = response.content;
            let summary_message = ChatMessage::new(
                MessageRole::System,
                format!("[CONTEXT SUMMARY - COMPACTED]:\n{}", summary_text)
            );
            messages.splice(compaction_target_range, std::iter::once(summary_message));
        },
        Ok(Err(e)) => {
            // Model error (e.g. model not found)
            // 🛡️ INTENT PRESERVATION: Extract the last user message before draining
            let last_user_msg = messages[compaction_target_range.clone()].iter().rev()
                .find(|m| m.role == MessageRole::User)
                .map(|m| m.content.clone());
            
            messages.drain(compaction_target_range);
            let panic_message = ChatMessage::new(
                MessageRole::System,
                format!("⚠️ [CRITICAL OVERLOAD]: Summarization model error ({}). Old history hard-pruned to restore stability.", e)
            );
            messages.insert(1, panic_message);
            
            // Re-inject the last user message so the model knows what it was doing
            if let Some(user_msg) = last_user_msg {
                let intent_msg = ChatMessage::new(
                    MessageRole::System,
                    format!("[USER INTENT PRESERVED]: The user's last request before compaction was:\n\n{}", user_msg)
                );
                messages.insert(2, intent_msg);
            }
        },
        Err(_) => {
            // Timeout fallback
            // 🛡️ INTENT PRESERVATION: Extract the last user message before draining
            let last_user_msg = messages[compaction_target_range.clone()].iter().rev()
                .find(|m| m.role == MessageRole::User)
                .map(|m| m.content.clone());
            
            messages.drain(compaction_target_range);
            let panic_message = ChatMessage::new(
                MessageRole::System,
                "⚠️ [CRITICAL OVERLOAD]: Summarization TIMEOUT (30s). Background model too slow. History hard-pruned.".to_string()
            );
            messages.insert(1, panic_message);
            
            // Re-inject the last user message so the model knows what it was doing
            if let Some(user_msg) = last_user_msg {
                let intent_msg = ChatMessage::new(
                    MessageRole::System,
                    format!("[USER INTENT PRESERVED]: The user's last request before compaction was:\n\n{}", user_msg)
                );
                messages.insert(2, intent_msg);
            }
        }
    }

    Ok(messages)
}

/// Generates a status report for the agent to understand its remaining context "runway".
pub fn generate_runway_report(used: usize, total: u64) -> String {
    let runway_pct = if total > 0 {
        ((total as f64 - used as f64) / total as f64) * 100.0
    } else {
        0.0
    };

    let emoji = if runway_pct > 50.0 { "🟢" } else if runway_pct > 20.0 { "🟡" } else { "🔴" };
    
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
