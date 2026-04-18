use ollama_rs::{
    generation::chat::{request::ChatMessageRequest, ChatMessage, MessageRole},
    models::ModelOptions,
    Ollama,
};
use miette::{Result, IntoDiagnostic};

/// Estimates the number of tokens in a list of messages using a heuristic (char count / 4).
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
    ctx_limit: u64,
) -> Result<Vec<ChatMessage>> {
    if messages.len() <= 10 {
        return Ok(messages); // Too few messages to meaningfully compact
    }

    // Zone Protection:
    // Index 0: System Prompt + Schema (PROTECTED)
    // Last 6 messages: Active working context (PROTECTED)
    let tail_size = 6;
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
        "CRITICAL: You are a summarization engine for an autonomous agent. 
Summarize the following conversation history into a concise, high-density briefing.

INSTRUCTIONS:
1. Preserve all tool names called and their outcomes.
2. Preserve all specific file paths, PIDs, or technical IDs mentioned.
3. Preserve the core problem the user is trying to solve.
4. BE CONCISE. Use bullet points for technical facts.
5. Output ONLY the summary. Do not use preamble like 'Here is the summary'.

CONVERSATION TO SUMMARIZE:
---
{}
---",
        summary_context
    );

    let options = ModelOptions::default()
        .num_ctx(ctx_limit) 
        .temperature(0.1);

    let request = ChatMessageRequest::new(
        sub_model.to_string(),
        vec![ChatMessage::new(MessageRole::User, summary_prompt)],
    ).options(options);

    let response = ollama.send_chat_messages(request).await.into_diagnostic()?;
    let summary_text = response.message.content;

    // Replace the range with a single summary message
    let summary_message = ChatMessage::new(
        MessageRole::System,
        format!("[CONTEXT SUMMARY - COMPACTED]:\n{}", summary_text)
    );

    messages.splice(compaction_target_range, std::iter::once(summary_message));

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
