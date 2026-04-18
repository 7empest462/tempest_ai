use ollama_rs::{
    generation::chat::{request::ChatMessageRequest, ChatMessage, MessageRole},
    models::ModelOptions,
    Ollama,
};
use miette::Result;

/// Estimates the number of tokens in a list of messages using a heuristic (char count / 3).
pub fn estimate_tokens(messages: &[ChatMessage]) -> usize {
    let mut total_chars = 0;
    for msg in messages {
        total_chars += msg.content.len();
        // Add overhead for role name and structure
        total_chars += 20; 
    }
    // DeepSeek and thinking models use more tokens for logic/symbols
    // 3 chars/token is a safer heuristic than 4
    total_chars / 3
}

/// Returns true if the estimated token count exceeds the threshold (75% of limit).
pub fn needs_compaction(messages: &[ChatMessage], ctx_limit: u64) -> bool {
    let estimated = estimate_tokens(messages);
    let threshold = (ctx_limit as f64 * 0.75) as usize;
    estimated > threshold
}

/// Compacts the middle of the history by summarizing it using the sub-agent model.
pub async fn compact_history(
    ollama: &Ollama,
    sub_model: &str,
    mut messages: Vec<ChatMessage>,
    _ctx_limit: u64,
) -> Result<Vec<ChatMessage>> {
    let _initial_count = estimate_tokens(&messages);

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
        "### TASK: CONTEXT DENSITY COMPACTION
You are a high-speed context compressor. Summarize the following session history.

### RULES:
1. OUTPUT DENSITY: Achieve a 5x compression ratio or higher.
2. TECHNICAL PRESERVATION: Retain all tool calls, file paths, PIDs, and error codes.
3. NO LOGORRHEA: Do NOT use phrases like 'The user and assistant discussed...'. 
4. NO VERBATIM: Do not repeat long blocks of text. Distill them into bulleted technical facts.
5. NO PREAMBLE: Start immediately with the summary.

### CONVERSATION STREAM:
{}
",
        summary_context
    );

    let options = ModelOptions::default()
        .num_ctx(4096) // Summarization doesn't need full context
        .temperature(0.01);

    let request = ChatMessageRequest::new(
        sub_model.to_string(),
        vec![ChatMessage::new(MessageRole::User, summary_prompt)],
    ).options(options);

    // If we are at critical capacity, try to summarize, but fallback to Hard-Prune if it fails or is too slow
    let summary_result = tokio::time::timeout(
        tokio::time::Duration::from_secs(15),
        ollama.send_chat_messages(request)
    ).await;

    match summary_result {
        Ok(Ok(response)) => {
            let summary_text = response.message.content;
            let summary_message = ChatMessage::new(
                MessageRole::System,
                format!("[CONTEXT SUMMARY - COMPACTED]:\n{}", summary_text)
            );
            messages.splice(compaction_target_range, std::iter::once(summary_message));
        },
        _ => {
            // HARD PRUNE FALLBACK: If summarizing fails or hangs, just drop the chunk to save the system
            messages.drain(compaction_target_range);
            let panic_message = ChatMessage::new(
                MessageRole::System,
                "⚠️ [CRITICAL OVERLOAD]: Summarization failed. Old history has been hard-pruned to restore system stability.".to_string()
            );
            messages.insert(1, panic_message);
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
