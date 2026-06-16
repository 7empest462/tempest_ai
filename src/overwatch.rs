//! Overwatch Context Rules Engine
//!
//! Implements Skelegent ContextOps for catching model hallucinations
//! before they reach the user. These rules apply backpressure to the agent loop
//! by injecting system corrections and forcing re-rolls when the model lies.
//!
//! Built on top of `skg_context_engine::ContextOp` and `Rule` with `Trigger::When`
//! predicates so they participate in the SKG context pipeline natively.
//!
//! ## Features
//! - **Fast-path detection**: Synchronous string-based rules for immediate interception
//! - **SKG integration**: Async context rules with priority ordering
//! - **Context scoring**: Tracks model suspicion level across multiple turns
//! - **Rate limiting**: Prevents infinite retry loops from overwatch triggers
//! - **Predicate caching**: Avoids redundant checks per turn
//! - **Error classification**: Integrates with error_classifier for recovery hints

use async_trait::async_trait;
use layer0::content::Content;
use layer0::context::{Message, Role};
use serde_json::{Value, json};
use skg_context_engine::{Context, ContextOp, EngineError, OutputSchema, Rule};
use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

// Note: error_classifier integration available for future recovery hint features

// ============================================================
// 📦 CONTEXT EXTENSIONS: PERSISTENT STATE & MEMORY
// ============================================================

/// Project-level facts and summaries persistent across conversation turns.
/// Stored in context extensions for long-term mission awareness.
#[derive(Debug, Clone, Default)]
pub struct ProjectMemory {
    /// Key facts discovered about the codebase (e.g., "uses Tokio for async")
    pub key_facts: Vec<String>,
    /// Summary of important files for quick recall
    pub file_summaries: HashMap<String, String>,
    /// Last successful plan to avoid repeating failed attempts
    pub last_successful_plan: Option<String>,
}

/// Sentinel state for tracking model behavior and guardrail triggers.
/// Enables adaptive guardrail enforcement across turns.
#[derive(Debug, Clone, Default)]
pub struct SentinelState {
    /// Total hallucination detections this session
    pub hallucination_count: u32,
    /// Current suspicion level derived from ContextScore
    pub suspicion_level: u32,
    /// Rules that have fired (for learning/adaptation)
    pub triggered_rules: Vec<String>,
    /// Timestamps of rule triggers for rate analysis
    pub rule_trigger_history: VecDeque<(String, u64)>,
}

impl SentinelState {
    /// Record a rule trigger and update history
    pub fn record_trigger(&mut self, rule_name: String) {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        self.triggered_rules.push(rule_name.clone());

        // Keep only last 20 triggers
        if self.rule_trigger_history.len() >= 20 {
            self.rule_trigger_history.pop_front();
        }
        self.rule_trigger_history.push_back((rule_name, now));
    }

    /// Get frequency of a rule trigger in last 60 seconds
    pub fn trigger_frequency(&self, rule_name: &str) -> u32 {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        self.rule_trigger_history
            .iter()
            .filter(|(name, timestamp)| name == rule_name && now.saturating_sub(*timestamp) < 60)
            .count() as u32
    }

    /// Check if a rule is spamming (triggered >3 times in 60 seconds)
    pub fn is_rule_spamming(&self, rule_name: &str) -> bool {
        self.trigger_frequency(rule_name) > 3
    }
}

// ============================================================
// ============================================================
/// Tracks the model's suspicion level and intercept rate across turns.
/// Used to prevent infinite retry loops and increase guardrail pressure.
#[derive(Debug, Clone)]
pub struct ContextScore {
    /// Current suspicion level (0-100). Increases per intercept, decays per successful turn.
    pub suspicion: u32,
    /// Timestamp of last intercept (unix seconds). Used for decay calculation.
    pub last_intercept_time: u64,
    /// Number of consecutive intercepts. Resets on success.
    pub consecutive_intercepts: u32,
    /// Rolling window of intercept events for rate limiting.
    pub recent_intercepts: VecDeque<u64>,
}

impl Default for ContextScore {
    fn default() -> Self {
        Self {
            suspicion: 0,
            last_intercept_time: 0,
            consecutive_intercepts: 0,
            recent_intercepts: VecDeque::with_capacity(10),
        }
    }
}

impl ContextScore {
    /// Update suspicion after an intercept. Returns true if rate limit exceeded.
    pub fn record_intercept(&mut self) -> bool {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        self.last_intercept_time = now;
        self.consecutive_intercepts += 1;
        self.suspicion = (self.suspicion + 15).min(100);

        // Keep only last 10 intercepts (60 second window)
        self.recent_intercepts
            .retain(|&t| now.saturating_sub(t) < 60);
        self.recent_intercepts.push_back(now);

        // Rate limit: more than 5 intercepts in 60 seconds = halt
        self.recent_intercepts.len() > 5
    }

    /// Mark a successful turn. Gradually decay suspicion.
    pub fn record_success(&mut self) {
        self.suspicion = self.suspicion.saturating_sub(5);
        self.consecutive_intercepts = 0;
    }

    /// Check if we're in a retry loop (too many consecutive intercepts).
    pub fn in_retry_loop(&self) -> bool {
        self.consecutive_intercepts > 3
    }

    /// Get a human-readable suspicion level.
    pub fn suspicion_label(&self) -> &'static str {
        match self.suspicion {
            0..=20 => "low",
            21..=50 => "moderate",
            51..=80 => "high",
            81..=100 => "critical",
            _ => "critical", // Catch-all for values beyond 100 (shouldn't happen due to capping)
        }
    }
}

/// Predicate cache to avoid redundant checks per turn.
#[derive(Debug, Default)]
pub struct PredicateCache {
    pub is_hallucinating_action: Option<bool>,
    pub is_hallucinating_file_io: Option<bool>,
    pub is_faking_tool_results: Option<bool>,
    pub has_json_violation: Option<bool>,
    pub has_self_contradiction: Option<bool>,
    pub has_scope_creep: Option<bool>,
    pub is_over_confident: Option<bool>,
}

impl PredicateCache {
    pub fn clear(&mut self) {
        *self = Self::default();
    }
}

// ============================================================
// �🛡️ HALLUCINATION GUARD
// ============================================================
// Catches the model using transition phrases ("I will", "here is")
// without actually emitting a tool-call block. This is the #1
// failure mode for small MLX models in multi-turn loops.

/// ContextOp that injects a harsh correction when hallucination is detected.
pub struct HallucinationGuardOp;

#[async_trait]
impl ContextOp for HallucinationGuardOp {
    type Output = ();

    async fn execute(&self, ctx: &mut Context) -> Result<(), EngineError> {
        ctx.messages.push(Message::new(
            Role::User, // System-level reprimand
            Content::text(
                "🛑 [OVERWATCH - HALLUCINATION GUARD]: You claimed to take an action \
                but did NOT output any tool call. You cannot perform actions through \
                natural language alone. You MUST output the correct JSON tool-call schema \
                to interact with the system. Re-issue your response with the proper tool call NOW.",
            ),
        ));
        Err(EngineError::Halted {
            reason: "Hallucination intercepted: action claim without tool call. Forcing LLM retry."
                .into(),
        })
    }
}

// ============================================================
// 📂 FILE I/O OVERWATCH
// ============================================================
// Catches the model claiming to have read or written files without
// actually calling file tools. This is the most common and most
// dangerous hallucination: the model invents file contents.

/// ContextOp that injects a correction when fake file I/O is detected.
pub struct FileIOOverwatchOp;

#[async_trait]
impl ContextOp for FileIOOverwatchOp {
    type Output = ();

    async fn execute(&self, ctx: &mut Context) -> Result<(), EngineError> {
        ctx.messages.push(Message::new(
            Role::User,
            Content::text(
                "🛑 [OVERWATCH - FILE I/O]: You hallucinated a file operation. \
                You CANNOT read or write files using natural language text. \
                You MUST output the correct tool-call schema (read_file, write_file, etc.) \
                to interact with the filesystem. Your previous response has been REJECTED. \
                Re-issue with proper tool calls.",
            ),
        ));
        Err(EngineError::Halted {
            reason: "File IO hallucination intercepted. Forcing retry.".into(),
        })
    }
}

// ============================================================
// 🎭 FAKE TOOL RESULT OVERWATCH
// ============================================================
// Catches the model impersonating system/tool output by emitting
// markers like "=== TOOL RESULT ===" that only the runtime should produce.

/// ContextOp that injects a correction when fake tool results are detected.
pub struct FakeToolResultOp;

#[async_trait]
impl ContextOp for FakeToolResultOp {
    type Output = ();

    async fn execute(&self, ctx: &mut Context) -> Result<(), EngineError> {
        ctx.messages.push(Message::new(
            Role::User,
            Content::text(
                "🛑 [OVERWATCH - IMPERSONATION]: You are fabricating tool/system output. \
                Only the Tempest runtime can produce tool results. You MUST call the actual tool \
                to get real results. Your fabricated output has been DISCARDED.",
            ),
        ));
        Err(EngineError::Halted {
            reason: "Fake tool result impersonation intercepted. Forcing retry.".into(),
        })
    }
}

// ============================================================
// 🔧 PREDICATE FUNCTIONS
// ============================================================

/// Strip <think>...</think> blocks from content so we don't flag
/// the model's internal reasoning as hallucination.
fn strip_thinking_blocks(content: &str) -> String {
    let mut result = content.to_string();
    while let Some(start) = result.find("<think>") {
        if let Some(end) = result[start..].find("</think>") {
            result.replace_range(start..start + end + 8, "");
        } else {
            result.truncate(start);
            break;
        }
    }
    result
}

/// Get the text content of the last assistant message, if any.
fn last_assistant_text(ctx: &Context) -> Option<String> {
    ctx.messages.iter().rev().find_map(|m| {
        if m.role == Role::Assistant {
            Some(m.text_content().to_string())
        } else {
            None
        }
    })
}

/// Check if a message contains valid tool-call syntax.
fn has_tool_call(content: &str) -> bool {
    content.contains("```json")
        || content.contains("[/MISSION]")
        || content.contains("\"tool\":")
        || content.contains("\"name\":")
}

/// Predicate: last assistant message claims action but has no tool call.
fn is_hallucinating_action(ctx: &Context) -> bool {
    let Some(raw) = last_assistant_text(ctx) else {
        return false;
    };
    let analysis = strip_thinking_blocks(&raw.to_lowercase());

    let transitions = [
        "i will now",
        "i'll now",
        "i'm going to",
        "let me now",
        "i will use",
        "i'll use",
        "i will run",
        "i'll run",
        "i will execute",
        "let me execute",
        "let me read",
        "let me write",
        "let me check",
        "i will read",
        "i will write",
        "i will check",
    ];

    let claims_action = transitions.iter().any(|&t| analysis.contains(t));
    claims_action && !has_tool_call(&raw)
}

/// Predicate: last assistant message claims file I/O without tool call.
fn is_hallucinating_file_io(ctx: &Context) -> bool {
    let Some(raw) = last_assistant_text(ctx) else {
        return false;
    };
    let analysis = strip_thinking_blocks(&raw.to_lowercase());

    let file_hallucinations = [
        "i have read the file",
        "i've read the file",
        "here are the contents",
        "the file contains",
        "the contents of",
        "i wrote the file",
        "i've written the file",
        "i saved the code to",
        "the file has been updated",
        "the file has been created",
        "i updated the file",
        "i've updated the file",
        "i created the file",
        "i've created the file",
        "here's what the file looks like",
        "the code in the file",
    ];

    let claims_file = file_hallucinations.iter().any(|&t| analysis.contains(t));
    claims_file && !has_tool_call(&raw)
}

/// Predicate: last assistant message contains fake tool result markers.
fn is_faking_tool_results(ctx: &Context) -> bool {
    let Some(raw) = last_assistant_text(ctx) else {
        return false;
    };
    let lower = raw.to_lowercase();

    let fake_markers = ["=== tool result ===", "=== tool error ==="];

    fake_markers
        .iter()
        .any(|&m| lower.contains(m) && !lower.contains(&format!("\"{}\"", m)))
}

// ============================================================
// 🔴 NEW RULES: JSON SCHEMA VIOLATIONS
// ============================================================

/// Predicate: JSON/Tool call has structural problems
/// (incomplete braces, trailing commas). Does NOT check for missing optional fields.
fn has_json_schema_violation(content: &str) -> bool {
    // Look for incomplete JSON structures in markdown blocks
    if content.contains("```json")
        && let Some(start) = content.find("```json")
    {
        let after = &content[start + 7..];
        if let Some(end) = after.find("```") {
            let json_block = &after[..end];

            // Check for incomplete JSON: opening brace but no closing, or vice versa
            let open_braces = json_block.matches('{').count();
            let close_braces = json_block.matches('}').count();

            if open_braces != close_braces {
                return true;
            }

            // Check for trailing commas (common error)
            if json_block.contains(",}") || json_block.contains(",]") {
                return true;
            }

            // Valid markdown JSON block — return false, don't check naked JSON
            return false;
        } else {
            // Opening marker but no closing — incomplete
            return true;
        }
    }

    // Check for naked JSON objects (outside markdown)
    if let Some(start) = content.find("{\"") {
        let json_part = &content[start..];

        // Mismatched braces
        let open = json_part.matches('{').count();
        let close = json_part.matches('}').count();
        if open != close {
            return true;
        }

        // Trailing comma in JSON
        if json_part.contains(",}") || json_part.contains(",]") {
            return true;
        }
    }

    false
}

// ============================================================
// 🔴 NEW RULES: SELF-CONTRADICTION DETECTION
// ============================================================

/// Check if model contradicts its own previous outputs
fn has_self_contradiction(ctx: &Context) -> bool {
    let Some(current) = last_assistant_text(ctx) else {
        return false;
    };
    let current_lower = current.to_lowercase();

    // Look for patterns like "I did X" but context shows we haven't called any tools
    let contradiction_patterns = [
        ("i have executed", "tool"),        // Claims execution without tool call
        ("i've completed", "tool"),         // Claims completion without result
        ("successfully created", "create"), // Claims creation without tool call
        ("the file now contains", "write"), // Claims write without write tool
        ("i found the error", "error classifier"), // Claims debugging without proper tool
    ];

    for (claim, required_evidence) in &contradiction_patterns {
        if current_lower.contains(claim) && !current_lower.contains(required_evidence) {
            // Check if recent history actually shows this action
            let has_evidence = ctx
                .messages
                .iter()
                .rev()
                .take(5) // Look at last 5 messages
                .any(|m| m.text_content().to_lowercase().contains(required_evidence));

            if !has_evidence {
                return true;
            }
        }
    }

    false
}

// ============================================================
// 🔴 NEW RULES: SCOPE CREEP DETECTION
// ============================================================

/// Detect if agent is trying to do things outside current scope/mission
fn has_scope_creep(ctx: &Context) -> bool {
    let Some(current) = last_assistant_text(ctx) else {
        return false;
    };
    let current_lower = current.to_lowercase();

    // Patterns indicating mission drift
    let scope_violations = [
        ("i will now rewrite", "refactor"), // Unsolicited refactoring
        ("let me optimize", "improve"),     // Scope creep: optimizing without being asked
        ("i'll deploy this", "deploy"),     // Attempting deployment outside scope
        ("i will run a full test suite", "test"), // Running tests not requested
        ("let me check the entire codebase", "scan"), // Scanning beyond scope
        ("i should update all", "batch modify"), // Batch changes without permission
    ];

    for (action, verb) in &scope_violations {
        if current_lower.contains(action) {
            // Only flag if there's no recent context supporting this action
            let has_permission = ctx
                .messages
                .iter()
                .rev()
                .take(3)
                .any(|m| m.text_content().to_lowercase().contains(verb));

            if !has_permission {
                return true;
            }
        }
    }

    false
}

// ============================================================
// 🔴 NEW RULES: OVER-CONFIDENCE DETECTION
// ============================================================

/// Detect false confidence on uncertain tasks
fn is_over_confident(content: &str) -> bool {
    let lower = content.to_lowercase();

    // Patterns of false certainty on inherently uncertain tasks
    let overconfident_patterns = [
        "100% certain",
        "absolutely guaranteed",
        "no doubt whatsoever",
        "definitely will not fail",
        "impossible for this to be wrong",
        "guaranteed to work",
        "100% accurate",
        "impossible to have any errors",
    ];

    if overconfident_patterns.iter().any(|p| lower.contains(p)) {
        // Over-confidence is only a violation if preceded by uncertainty markers
        // (e.g., dealing with network, parsing, concurrency)
        let uncertainty_context = [
            "network",
            "api",
            "parsing",
            "concurrent",
            "race",
            "timing",
            "external",
            "unstable",
            "unreliable",
        ];

        // If we're in uncertain context, the claim is illegitimate
        uncertainty_context.iter().any(|ctx| lower.contains(ctx))
    } else {
        false
    }
}

// ============================================================
// 🛑 HARD STOP / YIELD LOGIC
// ============================================================

/// Checks if the provided text contains a syntactically complete JSON tool call.
/// This is used to aggressively truncate model generation the millisecond it
/// finishes asking for a tool, preventing post-tool hallucinations.
pub fn is_complete_tool_json(text: &str) -> bool {
    let lower = text.to_lowercase();

    // Case 1: Markdown block ```json ... ```
    if let Some(start) = lower.find("```json") {
        let after = &text[start + 7..];
        if after.contains("```") {
            return true;
        }
    }

    // Case 2: Brace counting for any root object that looks like a tool call or validation
    // Find the first '{' and see if it balances out and contains "tool", "name", or "is_valid".
    if let Some(start) = text.find('{') {
        let json_part = &text[start..];
        let mut depth = 0;
        let mut in_string = false;
        let mut escape = false;
        let mut started = false;
        let mut end_idx = 0;

        for (i, c) in json_part.chars().enumerate() {
            if escape {
                escape = false;
                continue;
            }
            match c {
                '\\' => escape = true,
                '"' => in_string = !in_string,
                '{' if !in_string => {
                    depth += 1;
                    started = true;
                }
                '}' if !in_string => {
                    depth -= 1;
                    if started && depth == 0 {
                        end_idx = i;
                        break;
                    }
                }
                _ => {}
            }
        }

        if started && depth == 0 {
            let extracted = &json_part[..=end_idx];
            let lower_ext = extracted.to_lowercase();
            if lower_ext.contains("\"tool\"")
                || lower_ext.contains("\"name\"")
                || lower_ext.contains("\"is_valid\"")
            {
                return true;
            }
        }
    }

    false
}

/// Helper to repair unescaped nested double quotes in JSON values before parsing.
/// It scans for key-value starts like `"command": "` and escapes any unescaped double quotes
/// in the value before the matching closing quote (which is followed by a JSON delimiter).
pub fn repair_json_str(s: &str) -> String {
    let key_start_re = regex::Regex::new(r#""[a-zA-Z0-9_-]+"\s*:\s*""#).unwrap();
    let mut result = s.to_string();

    // Scan backwards so that modifying indices doesn't affect earlier matches
    let mut matches: Vec<(usize, usize)> = key_start_re
        .find_iter(s)
        .map(|m| (m.start(), m.end()))
        .collect();
    matches.reverse();

    for (_start_idx, end_idx) in matches {
        let remaining = &result[end_idx..];

        let mut closing_quote_idx = None;
        let chars: Vec<char> = remaining.chars().collect();
        let mut j = 0;
        while j < chars.len() {
            if chars[j] == '"' {
                // Check if followed by JSON delimiter: , or } or ]
                let mut is_delimiter = false;
                let mut k = j + 1;
                while k < chars.len() {
                    let c = chars[k];
                    if c.is_whitespace() {
                        k += 1;
                        continue;
                    }
                    if c == ',' || c == '}' || c == ']' {
                        is_delimiter = true;
                    }
                    break;
                }

                if is_delimiter {
                    closing_quote_idx = Some(j);
                }
            }

            // Stop scanning if we hit another key-value start
            if j + 3 < chars.len() && chars[j] == '"' {
                let mut k = j + 1;
                while k < chars.len()
                    && (chars[k].is_alphanumeric() || chars[k] == '_' || chars[k] == '-')
                {
                    k += 1;
                }
                if k < chars.len() && chars[k] == '"' {
                    let mut colon = k + 1;
                    while colon < chars.len() && chars[colon].is_whitespace() {
                        colon += 1;
                    }
                    if colon < chars.len() && chars[colon] == ':' {
                        let mut quote = colon + 1;
                        while quote < chars.len() && chars[quote].is_whitespace() {
                            quote += 1;
                        }
                        if quote < chars.len() && chars[quote] == '"' {
                            break;
                        }
                    }
                }
            }
            j += 1;
        }

        if let Some(close_idx) = closing_quote_idx {
            // Find character index boundary safely
            let char_boundary_idx: usize = remaining
                .char_indices()
                .map(|(idx, _)| idx)
                .nth(close_idx)
                .unwrap_or(close_idx);
            let raw_value = &remaining[..char_boundary_idx];
            let mut repaired_value = String::new();
            let chars_val: Vec<char> = raw_value.chars().collect();
            let mut idx = 0;
            while idx < chars_val.len() {
                let c = chars_val[idx];
                if c == '"' {
                    let is_escaped = idx > 0 && chars_val[idx - 1] == '\\';
                    if !is_escaped {
                        repaired_value.push('\\');
                    }
                }
                repaired_value.push(c);
                idx += 1;
            }

            let prefix = &result[..end_idx];
            let suffix = &result[end_idx + char_boundary_idx..];
            result = format!("{}{}{}", prefix, repaired_value, suffix);
        }
    }

    result
}

// ============================================================
// 🔌 RULE FACTORY & CONTEXT OPERATIONS
// ============================================================

/// ContextOp for JSON schema violations.
pub struct JsonSchemaViolationOp;

#[async_trait]
impl ContextOp for JsonSchemaViolationOp {
    type Output = ();

    async fn execute(&self, ctx: &mut Context) -> Result<(), EngineError> {
        ctx.messages.push(Message::new(
            Role::User,
            Content::text(
                "🛑 [OVERWATCH - JSON SCHEMA]: Your tool call JSON is malformed. \
                Check for: missing closing braces, incomplete \"tool\"/\"name\" fields, \
                or missing \"params\"/\"arguments\". Reformat and retry.",
            ),
        ));
        Err(EngineError::Halted {
            reason: "JSON schema violation detected. Forcing retry with proper format.".into(),
        })
    }
}

/// ContextOp for self-contradictions.
pub struct SelfContradictionOp;

#[async_trait]
impl ContextOp for SelfContradictionOp {
    type Output = ();

    async fn execute(&self, ctx: &mut Context) -> Result<(), EngineError> {
        ctx.messages.push(Message::new(
            Role::User,
            Content::text(
                "🛑 [OVERWATCH - SELF-CONTRADICTION]: You claimed to have completed an action \
                but your history shows you never called the required tool or received a result. \
                Review your actual tool calls before making claims about completion.",
            ),
        ));
        Err(EngineError::Halted {
            reason: "Self-contradiction intercepted. Forcing consistency check.".into(),
        })
    }
}

/// ContextOp for scope creep.
pub struct ScopeCreepOp;

#[async_trait]
impl ContextOp for ScopeCreepOp {
    type Output = ();

    async fn execute(&self, ctx: &mut Context) -> Result<(), EngineError> {
        ctx.messages.push(Message::new(
            Role::User,
            Content::text(
                "🛑 [OVERWATCH - SCOPE CREEP]: You are attempting actions outside the current \
                scope/mission. Stay focused on the user's explicit request. Do not optimize, \
                refactor, deploy, or modify unrelated code without permission.",
            ),
        ));
        Err(EngineError::Halted {
            reason: "Scope creep detected. Forcing mission realignment.".into(),
        })
    }
}

/// ContextOp for over-confidence on uncertain tasks.
pub struct OverConfidenceOp;

#[async_trait]
impl ContextOp for OverConfidenceOp {
    type Output = ();

    async fn execute(&self, ctx: &mut Context) -> Result<(), EngineError> {
        ctx.messages.push(Message::new(
            Role::User,
            Content::text(
                "🛑 [OVERWATCH - OVER-CONFIDENCE]: You are claiming absolute certainty on an \
                inherently uncertain task (networking, parsing, concurrency, external APIs). \
                Use appropriate uncertainty language: \"likely\", \"should\", \"may fail\", etc.",
            ),
        ));
        Err(EngineError::Halted {
            reason: "Over-confidence on uncertain task. Forcing humility.".into(),
        })
    }
}

// ============================================================
// � TRIGGER SYSTEM & RULE BUILDER
// ============================================================

/// Event triggers that determine when rules fire during agent execution.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Trigger {
    /// Rule fires after model inference completes (checking output)
    OnInferenceComplete,
    /// Rule fires when model attempts to call a tool
    OnToolCall,
    /// Rule fires at any point during context evaluation
    Always,
}

/// Builder pattern for constructing overwatch rules with triggers and events.
///
/// Example:
/// ```ignore
/// let rule = RuleBuilder::new("Hallucination Guard")
///     .trigger(Trigger::OnInferenceComplete)
///     .when(is_hallucinating_action)
///     .priority(100)
///     .build();
/// ```
#[derive(Debug, Clone)]
pub struct RuleBuilder {
    name: String,
    trigger: Trigger,
    priority: i32, // Changed from u32 to i32 to match Rule::when signature
}

impl RuleBuilder {
    /// Create a new rule builder with the given name.
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            trigger: Trigger::Always,
            priority: 50,
        }
    }

    /// Set the trigger for when this rule fires.
    pub fn trigger(mut self, trigger: Trigger) -> Self {
        self.trigger = trigger;
        self
    }

    /// Set the priority of this rule (higher priority = fires first).
    pub fn priority(mut self, priority: i32) -> Self {
        self.priority = priority;
        self
    }

    /// Get the configured trigger.
    pub fn get_trigger(&self) -> Trigger {
        self.trigger
    }

    /// Get the configured priority.
    pub fn get_priority(&self) -> i32 {
        self.priority
    }

    /// Get the rule name.
    pub fn get_name(&self) -> &str {
        &self.name
    }
}

/// Predicate-based rule builder for creating Rule instances.
/// This wraps the skg_context_engine Rule::when pattern with trigger awareness.
pub struct PredicateRuleBuilder {
    builder: RuleBuilder,
}

impl PredicateRuleBuilder {
    /// Start building a rule with trigger support.
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            builder: RuleBuilder::new(name),
        }
    }

    /// Set the trigger for this rule.
    pub fn trigger(mut self, trigger: Trigger) -> Self {
        self.builder = self.builder.trigger(trigger);
        self
    }

    /// Set the priority for this rule.
    pub fn priority(mut self, priority: i32) -> Self {
        self.builder = self.builder.priority(priority);
        self
    }

    /// Build a Rule::when rule with the configured trigger and priority.
    /// The trigger is stored as metadata in the rule name via convention.
    pub fn when<P, O>(self, predicate: P, op: O) -> Rule
    where
        P: Fn(&Context) -> bool + Send + Sync + 'static,
        O: ContextOp<Output = ()> + 'static,
    {
        // Format rule name to include trigger info for observability
        let trigger_str = match self.builder.trigger {
            Trigger::OnInferenceComplete => "[INFERENCE]",
            Trigger::OnToolCall => "[TOOL_CALL]",
            Trigger::Always => "[ALWAYS]",
        };

        let descriptive_name = format!("{} {}", self.builder.get_name(), trigger_str);

        Rule::when(
            &descriptive_name,
            self.builder.get_priority(),
            predicate,
            op,
        )
    }
}

// ============================================================
// 🎯 STRUCTURED OUTPUT VALIDATION (SCHEMA BUILDER)
// ============================================================

/// Builder for creating structured output schemas with validation.
/// Integrates with OverwatchEngine for tool call validation.
///
/// Example:
/// ```ignore
/// let schema = OutputSchemaBuilder::new()
///     .with_tool_calls()
///     .with_reasoning()
///     .with_safety_check()
///     .build();
/// ```
#[derive(Debug, Clone)]
pub struct OutputSchemaBuilder {
    require_tool_calls: bool,
    require_reasoning: bool,
    require_safety_check: bool,
    max_retries: u32,
}

impl OutputSchemaBuilder {
    /// Create a new output schema builder with defaults.
    pub fn new() -> Self {
        Self {
            require_tool_calls: false,
            require_reasoning: false,
            require_safety_check: false,
            max_retries: 3,
        }
    }

    /// Require the output to contain tool calls (for action validation).
    pub fn with_tool_calls(mut self) -> Self {
        self.require_tool_calls = true;
        self
    }

    /// Require the output to include reasoning steps (for transparency).
    pub fn with_reasoning(mut self) -> Self {
        self.require_reasoning = true;
        self
    }

    /// Require the output to include safety checks (for harm prevention).
    pub fn with_safety_check(mut self) -> Self {
        self.require_safety_check = true;
        self
    }

    /// Set maximum validation retries (default: 3).
    pub fn max_retries(mut self, count: u32) -> Self {
        self.max_retries = count;
        self
    }

    /// Build the OutputSchema with tool-call validation.
    pub fn build(self) -> OutputSchema {
        let mut properties = json!({
            "result": {
                "type": "string",
                "description": "The final result of the operation"
            }
        });

        // Add reasoning field if required
        if self.require_reasoning
            && let Value::Object(ref mut props) = properties
        {
            props.insert(
                "reasoning".to_string(),
                json!({
                    "type": "string",
                    "description": "Step-by-step reasoning for the decision"
                }),
            );
        }

        // Add safety check field if required
        if self.require_safety_check
            && let Value::Object(ref mut props) = properties
        {
            props.insert(
                "safety_check".to_string(),
                json!({
                    "type": "object",
                    "properties": {
                        "is_safe": { "type": "boolean" },
                        "concerns": {
                            "type": "array",
                            "items": { "type": "string" }
                        }
                    },
                    "required": ["is_safe"]
                }),
            );
        }

        // Build required fields list
        let mut required = vec!["result"];
        if self.require_reasoning {
            required.push("reasoning");
        }
        if self.require_safety_check {
            required.push("safety_check");
        }

        let schema = json!({
            "type": "object",
            "properties": properties,
            "required": required,
            "additionalProperties": false
        });

        // Validator closure: checks required fields are present and non-empty
        let require_tool_calls = self.require_tool_calls;
        let require_reasoning = self.require_reasoning;
        let require_safety_check = self.require_safety_check;

        OutputSchema::tool_call(schema, move |value| {
            // Check basic structure
            let obj = value
                .as_object()
                .ok_or_else(|| "Output must be a JSON object".to_string())?;

            // Validate result field
            let result = obj
                .get("result")
                .ok_or_else(|| "Missing 'result' field".to_string())?
                .as_str()
                .ok_or_else(|| "'result' must be a string".to_string())?;

            if result.is_empty() {
                return Err("'result' cannot be empty".to_string());
            }

            // Validate reasoning if required
            if require_reasoning {
                let reasoning = obj
                    .get("reasoning")
                    .ok_or_else(|| "Missing required 'reasoning' field".to_string())?
                    .as_str()
                    .ok_or_else(|| "'reasoning' must be a string".to_string())?;

                if reasoning.is_empty() {
                    return Err("'reasoning' cannot be empty".to_string());
                }
            }

            // Validate safety check if required
            if require_safety_check {
                let safety = obj
                    .get("safety_check")
                    .ok_or_else(|| "Missing required 'safety_check' field".to_string())?
                    .as_object()
                    .ok_or_else(|| "'safety_check' must be an object".to_string())?;

                let is_safe = safety
                    .get("is_safe")
                    .ok_or_else(|| "Missing 'is_safe' in safety_check".to_string())?
                    .as_bool()
                    .ok_or_else(|| "'is_safe' must be a boolean".to_string())?;

                if !is_safe
                    && let Some(concerns) = safety.get("concerns")
                    && let Some(arr) = concerns.as_array()
                    && arr.is_empty()
                {
                    return Err("Safety check failed: concerns array is empty".to_string());
                }
            }

            // Validate tool calls if required
            if require_tool_calls
                && !result.contains("tool_call")
                && !result.contains("calling_tool")
            {
                return Err("Output requires tool calls but none detected.".to_string());
            }

            Ok(value.clone())
        })
    }
}

impl Default for OutputSchemaBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// Tool call validation helper for OverwatchEngine.
#[derive(Debug, Clone)]
pub struct ToolCallValidator {
    pub max_name_length: usize,
    pub max_args_length: usize,
    pub blocked_patterns: Vec<String>,
}

impl ToolCallValidator {
    /// Create a new tool call validator with defaults.
    pub fn new() -> Self {
        Self {
            max_name_length: 256,
            max_args_length: 65536,
            blocked_patterns: vec![
                "rm -rf".to_string(),
                "format.*disk".to_string(),
                "chmod.*000".to_string(),
            ],
        }
    }

    /// Validate a tool call name and arguments.
    pub fn validate_tool_call(&self, tool_name: &str, args_json: &str) -> Result<(), String> {
        if tool_name.len() > self.max_name_length {
            return Err(format!(
                "Tool name exceeds max length of {} chars",
                self.max_name_length
            ));
        }

        if args_json.len() > self.max_args_length {
            return Err(format!(
                "Tool arguments exceed max size of {} bytes",
                self.max_args_length
            ));
        }

        for pattern in &self.blocked_patterns {
            if args_json.contains(pattern) {
                return Err(format!("Tool call contains blocked pattern: {}", pattern));
            }
        }

        serde_json::from_str::<Value>(args_json)
            .map_err(|e| format!("Tool arguments are not valid JSON: {}", e))?;

        Ok(())
    }
}

impl Default for ToolCallValidator {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================
// �🌪️ RULE FACTORY (SKG CONTEXT ENGINE)
// ============================================================

/// Build the set of Overwatch rules for registration into a Context.
///
/// These are `Rule::when` rules with `Trigger::When` predicates that
/// fire automatically during `Context::run()` calls. They have maximum
/// priority (100) so they fire before any other rules.
///
/// Rules are ordered by criticality:
/// 1. Hallucination (priority 100) — no tool call claimed
/// 2. File I/O (priority 100) — fake file operations
/// 3. Fake Results (priority 100) — impersonation
/// 4. JSON Schema (priority 99) — malformed tool calls
/// 5. Self-Contradiction (priority 98) — claims without evidence
/// 6. Scope Creep (priority 97) — out-of-scope actions
/// 7. Over-Confidence (priority 96) — false certainty on uncertain tasks
pub fn overwatch_rules() -> Vec<Rule> {
    vec![
        Rule::when(
            "Hallucination Guard [INFERENCE]",
            100,
            is_hallucinating_action,
            HallucinationGuardOp,
        ),
        Rule::when(
            "File I/O Overwatch [INFERENCE]",
            100,
            is_hallucinating_file_io,
            FileIOOverwatchOp,
        ),
        Rule::when(
            "Fake Result Guard [INFERENCE]",
            100,
            is_faking_tool_results,
            FakeToolResultOp,
        ),
        Rule::when(
            "JSON Schema Violation [INFERENCE]",
            99,
            is_hallucinating_file_io, // Reuse existing predicate adapter until full context available
            JsonSchemaViolationOp,
        ),
        Rule::when(
            "Self-Contradiction Check [INFERENCE]",
            98,
            has_self_contradiction,
            SelfContradictionOp,
        ),
        Rule::when(
            "Scope Creep Detection [INFERENCE]",
            97,
            has_scope_creep,
            ScopeCreepOp,
        ),
    ]
}

/// Build overwatch rules using the advanced RuleBuilder pattern with Trigger support.
///
/// This is an alternative to `overwatch_rules()` that uses the builder pattern
/// for more declarative rule definitions with explicit trigger specification.
///
/// Example of how this pattern enables future integration:
/// ```ignore
/// let hallucination_rule = PredicateRuleBuilder::new("Hallucination Guard")
///     .trigger(Trigger::OnInferenceComplete)
///     .priority(100)
///     .when(is_hallucinating_action, HallucinationGuardOp);
/// ```
///
/// The rules fire in this order:
/// 1. OnInferenceComplete triggers (highest priority first)
/// 2. OnToolCall triggers
/// 3. Always triggers
pub fn setup_overwatch_rules_with_triggers() -> Vec<Rule> {
    vec![
        PredicateRuleBuilder::new("Hallucination Guard")
            .trigger(Trigger::OnInferenceComplete)
            .priority(100)
            .when(is_hallucinating_action, HallucinationGuardOp),
        PredicateRuleBuilder::new("File I/O Overwatch")
            .trigger(Trigger::OnInferenceComplete)
            .priority(100)
            .when(is_hallucinating_file_io, FileIOOverwatchOp),
        PredicateRuleBuilder::new("Fake Result Guard")
            .trigger(Trigger::OnInferenceComplete)
            .priority(100)
            .when(is_faking_tool_results, FakeToolResultOp),
        PredicateRuleBuilder::new("JSON Schema Violation")
            .trigger(Trigger::OnInferenceComplete)
            .priority(99)
            .when(is_hallucinating_file_io, JsonSchemaViolationOp),
        PredicateRuleBuilder::new("Self-Contradiction Check")
            .trigger(Trigger::OnInferenceComplete)
            .priority(98)
            .when(has_self_contradiction, SelfContradictionOp),
        PredicateRuleBuilder::new("Scope Creep Detection")
            .trigger(Trigger::OnInferenceComplete)
            .priority(97)
            .when(has_scope_creep, ScopeCreepOp),
    ]
}

pub fn initialize_overwatch_extensions(ctx: &mut Context) {
    ctx.extensions.insert(ProjectMemory::default());
    ctx.extensions.insert(SentinelState::default());
    ctx.extensions.insert(ContextScore::default());
}

pub fn register_overwatch_rules(ctx: &mut Context) {
    initialize_overwatch_extensions(ctx);

    for rule in setup_overwatch_rules_with_triggers() {
        ctx.add_rule(rule);
    }
}

// ============================================================
// 🔌 NATIVE OVERWATCH ENGINE (Agent-side fast-path)
// ============================================================
// This is the synchronous fast-path used directly by the agent's
// StreamingContent handler. It doesn't require a full Context —
// it just inspects the raw assistant output string.

/// Result of running an overwatch rule.
#[derive(Debug)]
pub enum OverwatchVerdict {
    /// Model output is clean. Proceed normally.
    Pass,
    /// Hallucination detected.
    Intercept {
        /// System message injected into chat history to reprimand the model.
        correction: String,
        /// Short log line for the TUI sentinel panel.
        log: String,
        /// Name of the rule that fired.
        rule_name: String,
    },
}

/// Trait for fast-path overwatch rules (synchronous, string-based).
pub trait OverwatchRule: Send + Sync {
    /// Human-readable name of this rule.
    fn name(&self) -> &'static str;
    /// Inspect the last assistant message and return a verdict.
    fn evaluate(&self, content: &str) -> OverwatchVerdict;
}

// --- Fast-path rule implementations ---

struct HallucinationGuardRule;
impl OverwatchRule for HallucinationGuardRule {
    fn name(&self) -> &'static str {
        "Hallucination Guard"
    }
    fn evaluate(&self, content: &str) -> OverwatchVerdict {
        let analysis = strip_thinking_blocks(&content.to_lowercase());
        let transitions = [
            "i will now",
            "i'll now",
            "i'm going to",
            "let me now",
            "i will use",
            "i'll use",
            "i will run",
            "i'll run",
            "i will execute",
            "let me execute",
            "let me read",
            "let me write",
            "let me check",
            "i will read",
            "i will write",
            "i will check",
        ];
        if transitions.iter().any(|&t| analysis.contains(t)) && !has_tool_call(content) {
            OverwatchVerdict::Intercept {
                correction: "🛑 [OVERWATCH - HALLUCINATION GUARD]: You claimed to take an action but did NOT output any tool call. You MUST output the correct JSON tool-call schema to interact with the system. Re-issue your response with the proper tool call NOW.".to_string(),
                log: "Blocked hallucinated action claim (no tool call emitted)".to_string(),
                rule_name: "Hallucination Guard".to_string(),
            }
        } else {
            OverwatchVerdict::Pass
        }
    }
}

struct FileIOOverwatchFastRule;
impl OverwatchRule for FileIOOverwatchFastRule {
    fn name(&self) -> &'static str {
        "File I/O Overwatch"
    }
    fn evaluate(&self, content: &str) -> OverwatchVerdict {
        let analysis = strip_thinking_blocks(&content.to_lowercase());
        let file_hallucinations = [
            "i have read the file",
            "i've read the file",
            "here are the contents",
            "the file contains",
            "the contents of",
            "i wrote the file",
            "i've written the file",
            "i saved the code to",
            "the file has been updated",
            "the file has been created",
            "i updated the file",
            "i've updated the file",
            "i created the file",
            "i've created the file",
            "here's what the file looks like",
            "the code in the file",
        ];
        if file_hallucinations.iter().any(|&t| analysis.contains(t)) && !has_tool_call(content) {
            OverwatchVerdict::Intercept {
                correction: "🛑 [OVERWATCH - FILE I/O]: You hallucinated a file operation. You MUST output the correct tool-call schema to interact with the filesystem. Your previous response has been REJECTED.".to_string(),
                log: "Blocked hallucinated file I/O (no tool call emitted)".to_string(),
                rule_name: "File I/O Overwatch".to_string(),
            }
        } else {
            OverwatchVerdict::Pass
        }
    }
}

struct FakeToolResultFastRule;
impl OverwatchRule for FakeToolResultFastRule {
    fn name(&self) -> &'static str {
        "Fake Result Guard"
    }
    fn evaluate(&self, content: &str) -> OverwatchVerdict {
        let lower = content.to_lowercase();
        let fake_markers = ["=== tool result ===", "=== tool error ==="];
        if fake_markers
            .iter()
            .any(|&m| lower.contains(m) && !lower.contains(&format!("\"{}\"", m)))
        {
            OverwatchVerdict::Intercept {
                correction: "🛑 [OVERWATCH - IMPERSONATION]: You are fabricating tool/system output. Only the Tempest runtime can produce tool results. Your fabricated output has been DISCARDED.".to_string(),
                log: "Blocked fake tool result impersonation".to_string(),
                rule_name: "Fake Result Guard".to_string(),
            }
        } else {
            OverwatchVerdict::Pass
        }
    }
}

struct JsonSchemaViolationFastRule;
impl OverwatchRule for JsonSchemaViolationFastRule {
    fn name(&self) -> &'static str {
        "JSON Schema Violation"
    }
    fn evaluate(&self, content: &str) -> OverwatchVerdict {
        if has_json_schema_violation(content) {
            OverwatchVerdict::Intercept {
                correction: "🛑 [OVERWATCH - JSON SCHEMA]: Your tool call JSON is malformed. Check for: missing closing braces, incomplete \"tool\"/\"name\" fields, or missing \"params\"/\"arguments\". Reformat and retry.".to_string(),
                log: "Blocked malformed JSON schema in tool call".to_string(),
                rule_name: "JSON Schema Violation".to_string(),
            }
        } else {
            OverwatchVerdict::Pass
        }
    }
}

struct ScopeCreepFastRule;
impl OverwatchRule for ScopeCreepFastRule {
    fn name(&self) -> &'static str {
        "Scope Creep Detection"
    }
    fn evaluate(&self, content: &str) -> OverwatchVerdict {
        let lower = content.to_lowercase();
        let scope_violations = [
            "i will now rewrite",
            "let me optimize",
            "i'll deploy",
            "i will run a full test",
            "let me check the entire",
            "i should update all",
        ];

        if scope_violations.iter().any(|v| lower.contains(v)) {
            OverwatchVerdict::Intercept {
                correction: "🛑 [OVERWATCH - SCOPE CREEP]: You are attempting actions outside current scope. Stay focused on the user's explicit request.".to_string(),
                log: "Detected scope creep attempt".to_string(),
                rule_name: "Scope Creep Detection".to_string(),
            }
        } else {
            OverwatchVerdict::Pass
        }
    }
}

struct OverConfidenceFastRule;
impl OverwatchRule for OverConfidenceFastRule {
    fn name(&self) -> &'static str {
        "Over-Confidence Guard"
    }
    fn evaluate(&self, content: &str) -> OverwatchVerdict {
        if is_over_confident(content) {
            OverwatchVerdict::Intercept {
                correction: "🛑 [OVERWATCH - OVER-CONFIDENCE]: Avoid absolute certainty on inherently uncertain tasks. Use appropriate uncertainty language.".to_string(),
                log: "Blocked false certainty on uncertain task".to_string(),
                rule_name: "Over-Confidence Guard".to_string(),
            }
        } else {
            OverwatchVerdict::Pass
        }
    }
}

/// The fast-path overwatch engine used directly by the agent's StreamingContent handler.
pub struct OverwatchEngine {
    rules: Vec<Box<dyn OverwatchRule>>,
    /// Tracks context score and rate limiting for current session.
    context_score: Arc<Mutex<ContextScore>>,
    /// Caches predicate results to avoid redundant checks.
    predicate_cache: Arc<Mutex<PredicateCache>>,
    /// Long-term project memory for mission awareness across turns.
    project_memory: Arc<Mutex<ProjectMemory>>,
    /// Sentinel state tracking model behavior and rule triggers.
    sentinel_state: Arc<Mutex<SentinelState>>,
}

impl Default for OverwatchEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl OverwatchEngine {
    /// Create a new engine with the default rule set.
    pub fn new() -> Self {
        Self {
            rules: vec![
                Box::new(HallucinationGuardRule),
                Box::new(FileIOOverwatchFastRule),
                Box::new(FakeToolResultFastRule),
                Box::new(JsonSchemaViolationFastRule),
                Box::new(ScopeCreepFastRule),
                Box::new(OverConfidenceFastRule),
            ],
            context_score: Arc::new(Mutex::new(ContextScore::default())),
            predicate_cache: Arc::new(Mutex::new(PredicateCache::default())),
            project_memory: Arc::new(Mutex::new(ProjectMemory::default())),
            sentinel_state: Arc::new(Mutex::new(SentinelState::default())),
        }
    }

    /// Run all rules against the assistant output.
    /// Returns the first Intercept verdict found, or Pass if all rules pass.
    ///
    /// Also tracks context score and enforces rate limiting to prevent
    /// infinite retry loops. If rate limit is exceeded, returns a halt verdict.
    pub fn evaluate_pre_reaction(&self, content: &str) -> OverwatchVerdict {
        let mut score = self.context_score.lock().unwrap();

        // Check rate limit first (without recording a new false intercept)
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        score.recent_intercepts.retain(|&t| now.saturating_sub(t) < 60);
        if score.recent_intercepts.len() > 5 {
            return OverwatchVerdict::Intercept {
                correction: "🛑 [OVERWATCH - RATE LIMIT]: Too many consecutive intercepts. \
                    The model appears to be stuck in a retry loop. \
                    Halting to prevent infinite recursion. Please review the conversation context."
                    .to_string(),
                log: "Rate limit exceeded (>5 intercepts in 60s)".to_string(),
                rule_name: "Rate Limiter".to_string(),
            };
        }

        drop(score);

        // Run all rules against the assistant output
        for rule in &self.rules {
            match rule.evaluate(content) {
                OverwatchVerdict::Pass => continue,
                intercept => {
                    // Update context score on intercept
                    let mut score = self.context_score.lock().unwrap();
                    score.record_intercept();
                    return intercept;
                }
            }
        }

        OverwatchVerdict::Pass
    }

    /// Validate tool calls BEFORE execution. This closes the critical architectural gap
    /// where native tool calls (via Ollama/MLX structured tool calling) bypassed the
    /// Overwatch engine entirely by going PendingTools → ExecutingTools without passing
    /// through StreamingContent.
    ///
    /// Returns Pass if all tool calls look safe, or Intercept with details if any are suspicious.
    pub fn validate_tool_calls(
        &self,
        tool_calls: &[serde_json::Value],
        last_user_message: Option<&str>,
    ) -> OverwatchVerdict {
        let mut write_count = 0;
        let mut critical_file_writes: Vec<String> = Vec::new();

        // Critical project files that should never be casually overwritten
        let critical_files = [
            "Cargo.toml",
            "Cargo.lock",
            "Package.json",
            "package-lock.json",
            "Makefile",
            "CMakeLists.txt",
            "build.gradle",
            "pom.xml",
            ".gitignore",
            "Dockerfile",
        ];
        // Critical source entry points
        let critical_sources = [
            "main.rs", "lib.rs", "mod.rs", "index.ts", "index.js", "app.py",
        ];

        for call in tool_calls {
            let tool_name = call
                .get("tool")
                .or_else(|| call.get("name"))
                .or_else(|| call.get("function"))
                .and_then(|v| v.as_str())
                .unwrap_or("");

            let args = call
                .get("arguments")
                .or_else(|| call.get("params"))
                .or_else(|| call.get("args"));

            if tool_name == "write_file" {
                write_count += 1;

                if let Some(args) = args {
                    let path = args.get("path").and_then(|v| v.as_str()).unwrap_or("");
                    let content_len = args
                        .get("content")
                        .and_then(|v| v.as_str())
                        .map(|s| s.len())
                        .unwrap_or(0);

                    // Check if writing to a critical file
                    let filename = std::path::Path::new(path)
                        .file_name()
                        .and_then(|f| f.to_str())
                        .unwrap_or("");

                    let path_obj = std::path::Path::new(path);
                    let mut is_root = true;
                    let mut is_in_src = false;
                    if let Some(parent) = path_obj.parent() {
                        let parent_str = parent.to_string_lossy();
                        if !parent_str.is_empty() && parent_str != "." && parent_str != "./" {
                            is_root = false;
                            if parent_str == "src" || parent_str == "./src" || parent_str == "src/" {
                                is_in_src = true;
                            }
                        }
                    }

                    let is_critical = (is_root && critical_files.contains(&filename))
                        || ((is_root || is_in_src) && critical_sources.contains(&filename));

                    if is_critical && content_len < 200 {
                        critical_file_writes.push(format!("{} ({} bytes)", path, content_len));
                    }
                }
            }
        }

        // 🛡️ BATCH WRITE GUARD: Multiple write_file calls in one turn is suspicious
        if write_count >= 3 {
            return OverwatchVerdict::Intercept {
                correction: format!(
                    "🛑 [OVERWATCH - BATCH WRITE GUARD]: You emitted {} write_file calls in a single turn. \
                    This is dangerously aggressive. Break this into individual, verified steps.",
                    write_count
                ),
                log: format!(
                    "Blocked batch of {} write_file calls in single turn",
                    write_count
                ),
                rule_name: "Batch Write Guard".to_string(),
            };
        }

        // 🛡️ CRITICAL FILE GUARD: Tiny writes to critical project files
        if !critical_file_writes.is_empty() {
            // Check if user actually asked for something that would justify this
            let user_asked_for_init = last_user_message.is_some_and(|msg| {
                let lower = msg.to_lowercase();
                lower.contains("create")
                    || lower.contains("init")
                    || lower.contains("new project")
                    || lower.contains("hello world")
                    || lower.contains("scaffold")
                    || lower.contains("replace")
                    || lower.contains("overwrite")
                    || lower.contains("make")
                    || lower.contains("write")
                    || lower.contains("add")
                    || lower.contains("generate")
                    || lower.contains("setup")
                    || lower.contains("bench")
                    || lower.contains("compare")
                    || lower.contains("speed")
            });

            if !user_asked_for_init {
                return OverwatchVerdict::Intercept {
                    correction: format!(
                        "🛑 [OVERWATCH - CRITICAL FILE GUARD]: You are writing tiny content to critical project files: [{}]. \
                        This looks like a hallucinated project initialization. The user did NOT ask to create a new project. \
                        Review the user's actual request and respond appropriately.",
                        critical_file_writes.join(", ")
                    ),
                    log: format!(
                        "Blocked suspicious tiny write to critical files: {:?}",
                        critical_file_writes
                    ),
                    rule_name: "Critical File Guard".to_string(),
                };
            }
        }

        OverwatchVerdict::Pass
    }

    /// Validate algebraic effects BEFORE physical execution. This is the core sandboxing guard
    /// for Phase 3, allowing Overwatch to intercept destructive filesystem or shell operations.
    pub fn validate_effects(
        &self,
        effects: &[crate::effects::TempestEffect],
        last_user_message: Option<&str>,
    ) -> OverwatchVerdict {
        let mut write_count = 0;
        let mut critical_file_writes: Vec<String> = Vec::new();
        let mut dangerous_commands: Vec<String> = Vec::new();

        let critical_files = [
            "Cargo.toml",
            "Cargo.lock",
            "Package.json",
            "package-lock.json",
            "Makefile",
            "CMakeLists.txt",
            "build.gradle",
            "pom.xml",
            ".gitignore",
            "Dockerfile",
        ];
        let critical_sources = [
            "main.rs", "lib.rs", "mod.rs", "index.ts", "index.js", "app.py",
        ];

        for effect in effects {
            match effect {
                crate::effects::TempestEffect::WriteFile { path, content, .. } => {
                    write_count += 1;
                    let filename = std::path::Path::new(path)
                        .file_name()
                        .and_then(|f| f.to_str())
                        .unwrap_or("");

                    let path_obj = std::path::Path::new(path);
                    let mut is_root = true;
                    let mut is_in_src = false;
                    if let Some(parent) = path_obj.parent() {
                        let parent_str = parent.to_string_lossy();
                        if !parent_str.is_empty() && parent_str != "." && parent_str != "./" {
                            is_root = false;
                            if parent_str == "src" || parent_str == "./src" || parent_str == "src/" {
                                is_in_src = true;
                            }
                        }
                    }

                    let is_critical = (is_root && critical_files.contains(&filename))
                        || ((is_root || is_in_src) && critical_sources.contains(&filename));

                    if is_critical && content.len() < 200 {
                        critical_file_writes.push(format!("{} ({} bytes)", path, content.len()));
                    }
                }
                crate::effects::TempestEffect::RunCommand { command, .. } => {
                    let cmd_lower = command.to_lowercase();
                    // Block absolute delete commands on critical paths
                    if cmd_lower.contains("rm ")
                        && (cmd_lower.contains("src")
                            || cmd_lower.contains("/")
                            || cmd_lower.contains("*")
                            || cmd_lower.contains("cargo"))
                    {
                        dangerous_commands.push(command.clone());
                    }
                }
                _ => {}
            }
        }

        if !dangerous_commands.is_empty() {
            return OverwatchVerdict::Intercept {
                correction: format!(
                    "🛑 [OVERWATCH - ALGEBRAIC EFFECT GUARD]: Destructive command execution blocked: [{}]. \
                    You are not allowed to execute destructive delete/cleanup commands on core source files. \
                    Formulate a safe alternative command.",
                    dangerous_commands.join(", ")
                ),
                log: format!(
                    "Blocked dangerous command execution effect: {:?}",
                    dangerous_commands
                ),
                rule_name: "Algebraic Effect Guard - Dangerous Command".to_string(),
            };
        }

        if write_count >= 3 {
            return OverwatchVerdict::Intercept {
                correction: format!(
                    "🛑 [OVERWATCH - ALGEBRAIC EFFECT GUARD]: Suspicious batch of {} file writes blocked. \
                    Break your work down into individual, verified steps.",
                    write_count
                ),
                log: format!(
                    "Blocked batch of {} write effects in single turn",
                    write_count
                ),
                rule_name: "Algebraic Effect Guard - Batch Write".to_string(),
            };
        }

        if !critical_file_writes.is_empty() {
            let user_asked_for_init = last_user_message.is_some_and(|msg| {
                let lower = msg.to_lowercase();
                lower.contains("create")
                    || lower.contains("init")
                    || lower.contains("new project")
                    || lower.contains("hello world")
                    || lower.contains("scaffold")
                    || lower.contains("replace")
                    || lower.contains("overwrite")
                    || lower.contains("make")
                    || lower.contains("write")
                    || lower.contains("add")
                    || lower.contains("generate")
                    || lower.contains("setup")
                    || lower.contains("bench")
                    || lower.contains("compare")
                    || lower.contains("speed")
            });

            if !user_asked_for_init {
                return OverwatchVerdict::Intercept {
                    correction: format!(
                        "🛑 [OVERWATCH - ALGEBRAIC EFFECT GUARD]: Suspicious tiny write to critical project files blocked: [{}]. \
                        This looks like a hallucinated project overwrite. The user did NOT ask to overwrite project files. \
                        Review the user's actual instructions.",
                        critical_file_writes.join(", ")
                    ),
                    log: format!(
                        "Blocked suspicious tiny write effect to critical files: {:?}",
                        critical_file_writes
                    ),
                    rule_name: "Algebraic Effect Guard - Critical File".to_string(),
                };
            }
        }

        OverwatchVerdict::Pass
    }

    /// Mark a turn as successful (no intercepts). Decays suspicion.
    pub fn mark_success(&self) {
        if let Ok(mut score) = self.context_score.lock() {
            score.record_success();
        }
        if let Ok(mut cache) = self.predicate_cache.lock() {
            cache.clear();
        }
    }

    /// Get current context score for telemetry/UI display.
    pub fn get_score(&self) -> Option<ContextScore> {
        self.context_score.lock().ok().map(|s| s.clone())
    }

    /// Return the names of all registered rules (for TUI HUD).
    pub fn rule_names(&self) -> Vec<String> {
        self.rules.iter().map(|r| r.name().to_string()).collect()
    }

    /// Return current suspicion level and label (for telemetry).
    pub fn suspicion_status(&self) -> Option<(u32, String)> {
        self.context_score
            .lock()
            .ok()
            .map(|s| (s.suspicion, s.suspicion_label().to_string()))
    }

    /// Get project memory for mission awareness.
    pub fn project_memory(&self) -> Option<ProjectMemory> {
        self.project_memory.lock().ok().map(|m| m.clone())
    }

    /// Get sentinel state for monitoring model behavior.
    pub fn sentinel_state(&self) -> Option<SentinelState> {
        self.sentinel_state.lock().ok().map(|s| s.clone())
    }

    /// Update project memory (e.g., add facts, summaries).
    pub fn update_memory<F>(&self, f: F) -> Result<(), String>
    where
        F: FnOnce(&mut ProjectMemory),
    {
        self.project_memory
            .lock()
            .map_err(|_| "Failed to lock ProjectMemory".to_string())
            .map(|mut pm| f(&mut pm))
    }

    /// Record a rule trigger in sentinel state.
    pub fn record_rule_trigger(&self, rule_name: &str) {
        if let Ok(mut state) = self.sentinel_state.lock() {
            state.record_trigger(rule_name.to_string());
        }
    }
}

impl Clone for OverwatchEngine {
    fn clone(&self) -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::assert_matches;

    #[test]
    fn hallucination_guard_catches_empty_promises() {
        let engine = OverwatchEngine::new();
        let content = "I will now read the file and check its contents.";
        assert_matches!(
            engine.evaluate_pre_reaction(content),
            OverwatchVerdict::Intercept { .. }
        );
    }

    #[test]
    fn hallucination_guard_passes_with_tool_call() {
        let engine = OverwatchEngine::new();
        let content = "I will now read the file.\n```json\n{\"tool\": \"read_file\"}\n```";
        assert_matches!(
            engine.evaluate_pre_reaction(content),
            OverwatchVerdict::Pass
        );
    }

    #[test]
    fn file_io_catches_invented_contents() {
        let engine = OverwatchEngine::new();
        let content = "I have read the file. Here are the contents:\n\nfn main() { }";
        assert_matches!(
            engine.evaluate_pre_reaction(content),
            OverwatchVerdict::Intercept { .. }
        );
    }

    #[test]
    fn thinking_blocks_are_excluded() {
        let engine = OverwatchEngine::new();
        let content = "<think>I will now read the file.</think>The task is complete.";
        assert_matches!(
            engine.evaluate_pre_reaction(content),
            OverwatchVerdict::Pass
        );
    }

    #[test]
    fn fake_result_guard_catches_impersonation() {
        let engine = OverwatchEngine::new();
        let content = "=== TOOL RESULT ===\nFile written successfully to /tmp/foo.rs";
        assert_matches!(
            engine.evaluate_pre_reaction(content),
            OverwatchVerdict::Intercept { .. }
        );
    }

    #[test]
    fn clean_output_passes() {
        let engine = OverwatchEngine::new();
        assert_matches!(
            engine.evaluate_pre_reaction("Task complete."),
            OverwatchVerdict::Pass
        );
    }

    // --- NEW TESTS: JSON SCHEMA VIOLATIONS ---

    #[test]
    fn json_violation_catches_missing_closing_brace() {
        let content = "Here is my tool call:\n```json\n{\"tool\": \"read_file\", \"params\": {";
        assert!(has_json_schema_violation(content));
    }

    #[test]
    fn json_violation_catches_unclosed_markdown_block() {
        let content = "```json\n{\"tool\": \"read_file\"}"; // Missing closing ```
        assert!(has_json_schema_violation(content));
    }

    #[test]
    fn json_violation_catches_trailing_comma() {
        let content = "```json\n{\"tool\": \"read_file\", \"params\": {},}\n```";
        assert!(has_json_schema_violation(content));
    }

    #[test]
    fn valid_json_passes_schema_check() {
        let content =
            "```json\n{\"tool\": \"read_file\", \"params\": {\"path\": \"/tmp/file.rs\"}}\n```";
        assert!(!has_json_schema_violation(content));
    }

    // --- NEW TESTS: SCOPE CREEP ---

    #[test]
    fn scope_creep_detects_unsolicited_optimization() {
        let lower = "i will now optimize the entire codebase for performance";
        let scope_violations = [
            "i will now rewrite",
            "i will now optimize",
            "i'll deploy",
            "i will run a full test",
            "let me check the entire",
            "i should update all",
        ];
        assert!(scope_violations.iter().any(|v| lower.contains(v)));
    }

    #[test]
    fn scope_creep_detects_unauthorized_deployment() {
        let lower = "i'll deploy this to production now";
        let scope_violations = [
            "i will now rewrite",
            "let me optimize",
            "i'll deploy",
            "i will run a full test",
            "let me check the entire",
            "i should update all",
        ];
        assert!(scope_violations.iter().any(|v| lower.contains(v)));
    }

    // --- NEW TESTS: OVER-CONFIDENCE ---

    #[test]
    fn over_confidence_catches_certainty_on_network_tasks() {
        let content = "This network call is 100% guaranteed to work.";
        assert!(is_over_confident(content));
    }

    #[test]
    fn over_confidence_catches_impossible_claims_on_parsing() {
        let content = "This JSON parser is impossible for this to be wrong on parsing.";
        assert!(is_over_confident(content));
    }

    #[test]
    fn over_confidence_allows_reasonable_confidence() {
        let content = "This local file read should work correctly.";
        assert!(!is_over_confident(content));
    }

    // --- NEW TESTS: CONTEXT SCORING ---

    #[test]
    fn context_score_tracks_intercepts() {
        let mut score = ContextScore::default();
        assert_eq!(score.suspicion, 0);

        let rate_limited = score.record_intercept();
        assert!(!rate_limited); // First intercept shouldn't trigger rate limit
        assert!(score.suspicion > 0);
        assert_eq!(score.consecutive_intercepts, 1);
    }

    #[test]
    fn context_score_detects_retry_loop() {
        let mut score = ContextScore::default();
        for _ in 0..4 {
            score.record_intercept();
        }
        assert!(score.in_retry_loop());
    }

    #[test]
    fn context_score_decays_on_success() {
        let mut score = ContextScore::default();
        score.record_intercept();
        let initial_suspicion = score.suspicion;

        score.record_success();
        assert!(score.suspicion < initial_suspicion);
        assert_eq!(score.consecutive_intercepts, 0);
    }

    #[test]
    fn context_score_rate_limits_on_excessive_intercepts() {
        let mut score = ContextScore::default();

        // Trigger 6 rapid intercepts. Rate limit is: len > 5, so triggers on 6th (i=5)
        for i in 0..7 {
            let rate_limited = score.record_intercept();
            if i >= 5 {
                assert!(rate_limited, "Should trigger rate limit on intercept {}", i);
            }
        }
    }

    #[test]
    fn suspicion_labels_work() {
        let mut score = ContextScore::default();
        assert_eq!(score.suspicion_label(), "low");

        score.suspicion = 30;
        assert_eq!(score.suspicion_label(), "moderate");

        score.suspicion = 60;
        assert_eq!(score.suspicion_label(), "high");

        score.suspicion = 90;
        assert_eq!(score.suspicion_label(), "critical");
    }

    #[test]
    fn engine_tracks_suspicion_across_turns() {
        let engine = OverwatchEngine::new();

        let initial = engine.suspicion_status();
        assert!(initial.is_some());
        let (initial_suspicion, _) = initial.unwrap();
        assert_eq!(initial_suspicion, 0);

        // Trigger an intercept
        let _ = engine.evaluate_pre_reaction("I will now read the file");

        let after = engine.suspicion_status();
        assert!(after.is_some());
        let (after_suspicion, label) = after.unwrap();
        assert!(after_suspicion > initial_suspicion);
        assert!(!label.is_empty());
    }

    // --- SKG CONTEXT ENGINE INTEGRATION TESTS ---

    #[tokio::test]
    async fn skg_hallucination_rule_fires_on_predicate() {
        let rules = overwatch_rules();
        let mut ctx = Context::with_rules(rules);

        ctx.messages.push(Message::new(
            Role::Assistant,
            Content::text("I will now read the file and check its contents."),
        ));

        struct NoOp;
        #[async_trait]
        impl ContextOp for NoOp {
            type Output = ();
            async fn execute(&self, _ctx: &mut Context) -> Result<(), EngineError> {
                Ok(())
            }
        }

        let result = ctx.run(NoOp).await;
        assert!(result.is_err());
        assert!(ctx.messages.len() >= 2);
        let correction = ctx.messages.last().unwrap().text_content();
        assert!(correction.contains("OVERWATCH"));
    }

    #[tokio::test]
    async fn skg_clean_message_passes() {
        let rules = overwatch_rules();
        let mut ctx = Context::with_rules(rules);

        ctx.messages.push(Message::new(
            Role::Assistant,
            Content::text("Task complete. All files have been processed."),
        ));

        struct NoOp;
        #[async_trait]
        impl ContextOp for NoOp {
            type Output = ();
            async fn execute(&self, _ctx: &mut Context) -> Result<(), EngineError> {
                Ok(())
            }
        }

        let result = ctx.run(NoOp).await;
        assert!(result.is_ok());
    }

    // --- NEW TESTS: RULE BUILDER & TRIGGER SYSTEM ---

    #[test]
    fn rule_builder_creates_with_defaults() {
        let builder = RuleBuilder::new("Test Rule");
        assert_eq!(builder.get_name(), "Test Rule");
        assert_eq!(builder.get_trigger(), Trigger::Always);
        assert_eq!(builder.get_priority(), 50);
    }

    #[test]
    fn rule_builder_sets_trigger() {
        let builder = RuleBuilder::new("Test Rule").trigger(Trigger::OnInferenceComplete);
        assert_eq!(builder.get_trigger(), Trigger::OnInferenceComplete);
    }

    #[test]
    fn rule_builder_sets_priority() {
        let builder = RuleBuilder::new("Test Rule").priority(100);
        assert_eq!(builder.get_priority(), 100);
    }

    #[test]
    fn rule_builder_fluent_chain() {
        let builder = RuleBuilder::new("Test Rule")
            .trigger(Trigger::OnToolCall)
            .priority(75);
        assert_eq!(builder.get_name(), "Test Rule");
        assert_eq!(builder.get_trigger(), Trigger::OnToolCall);
        assert_eq!(builder.get_priority(), 75);
    }

    #[test]
    fn trigger_enum_variants() {
        assert_eq!(Trigger::OnInferenceComplete, Trigger::OnInferenceComplete);
        assert_eq!(Trigger::OnToolCall, Trigger::OnToolCall);
        assert_eq!(Trigger::Always, Trigger::Always);
        assert_ne!(Trigger::OnInferenceComplete, Trigger::OnToolCall);
    }

    #[test]
    fn predicate_rule_builder_creates_rule() {
        let _rule = PredicateRuleBuilder::new("Hallucination Guard")
            .trigger(Trigger::OnInferenceComplete)
            .priority(100)
            .when(
                |_ctx: &Context| false, // Predicate always returns false
                HallucinationGuardOp,
            );

        // Rule was successfully created (didn't panic)
    }

    #[test]
    fn predicate_rule_builder_includes_trigger_in_name() {
        let rule = PredicateRuleBuilder::new("Test")
            .trigger(Trigger::OnInferenceComplete)
            .when(|_: &Context| false, HallucinationGuardOp);

        // Verify the rule was created with trigger metadata in the name
        // Rule::when builds the rule with our descriptive_name that includes the trigger
        assert!(rule.name.contains("Test"));
        assert!(rule.name.contains("[INFERENCE]"));
    }

    #[test]
    fn predicate_rule_builder_tool_call_trigger_in_name() {
        let rule = PredicateRuleBuilder::new("Tool Validator")
            .trigger(Trigger::OnToolCall)
            .when(|_: &Context| false, HallucinationGuardOp);

        assert!(rule.name.contains("Tool Validator"));
        assert!(rule.name.contains("[TOOL_CALL]"));
    }

    #[test]
    fn setup_overwatch_rules_with_triggers_returns_rules() {
        let rules = setup_overwatch_rules_with_triggers();
        assert!(!rules.is_empty());

        // Verify trigger metadata is in rule names
        let has_inference_rules = rules.iter().any(|r| r.name.contains("[INFERENCE]"));
        assert!(has_inference_rules);
    }

    #[test]
    fn setup_overwatch_rules_with_triggers_has_high_priority() {
        let rules = setup_overwatch_rules_with_triggers();

        // All rules should have priorities >= 95 (based on criticality order)
        for rule in rules.iter() {
            assert!(
                rule.priority >= 95,
                "Rule {} has priority {}, expected >= 95",
                rule.name,
                rule.priority
            );
        }
    }

    #[test]
    fn original_overwatch_rules_still_work() {
        // Verify the original factory still produces valid rules
        let rules = overwatch_rules();
        assert!(!rules.is_empty());
        assert_eq!(rules.len(), setup_overwatch_rules_with_triggers().len());
    }

    // --- NEW TESTS: STRUCTURED OUTPUT VALIDATION ---

    #[test]
    fn output_schema_builder_creates_with_defaults() {
        let builder = OutputSchemaBuilder::new();
        assert!(!builder.require_tool_calls);
        assert!(!builder.require_reasoning);
        assert!(!builder.require_safety_check);
        assert_eq!(builder.max_retries, 3);
    }

    #[test]
    fn output_schema_builder_with_tool_calls() {
        let builder = OutputSchemaBuilder::new().with_tool_calls();
        assert!(builder.require_tool_calls);
    }

    #[test]
    fn output_schema_builder_with_reasoning() {
        let builder = OutputSchemaBuilder::new().with_reasoning();
        assert!(builder.require_reasoning);
    }

    #[test]
    fn output_schema_builder_with_safety_check() {
        let builder = OutputSchemaBuilder::new().with_safety_check();
        assert!(builder.require_safety_check);
    }

    #[test]
    fn output_schema_builder_fluent_chain() {
        let builder = OutputSchemaBuilder::new()
            .with_tool_calls()
            .with_reasoning()
            .with_safety_check()
            .max_retries(5);

        assert!(builder.require_tool_calls);
        assert!(builder.require_reasoning);
        assert!(builder.require_safety_check);
        assert_eq!(builder.max_retries, 5);
    }

    #[test]
    fn output_schema_builder_builds_schema() {
        let schema = OutputSchemaBuilder::new()
            .with_tool_calls()
            .with_reasoning()
            .build();

        // Verify schema was created
        assert!(!schema.schema.is_null());
    }

    #[test]
    fn output_schema_validates_valid_result() {
        let schema = OutputSchemaBuilder::new().build();

        let valid = json!({
            "result": "Task completed successfully"
        });

        let result = schema.validate(&valid);
        assert!(result.is_ok());
    }

    #[test]
    fn output_schema_rejects_missing_result() {
        let schema = OutputSchemaBuilder::new().build();

        let invalid = json!({
            "other_field": "value"
        });

        let result = schema.validate(&invalid);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("result"));
    }

    #[test]
    fn output_schema_rejects_empty_result() {
        let schema = OutputSchemaBuilder::new().build();

        let invalid = json!({
            "result": ""
        });

        let result = schema.validate(&invalid);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("empty"));
    }

    #[test]
    fn output_schema_validates_with_reasoning() {
        let schema = OutputSchemaBuilder::new().with_reasoning().build();

        let valid = json!({
            "result": "Task completed",
            "reasoning": "Step 1: analyzed, Step 2: decided"
        });

        let result = schema.validate(&valid);
        assert!(result.is_ok());
    }

    #[test]
    fn output_schema_requires_reasoning_when_specified() {
        let schema = OutputSchemaBuilder::new().with_reasoning().build();

        let invalid = json!({
            "result": "Task completed"
        });

        let result = schema.validate(&invalid);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("reasoning"));
    }

    #[test]
    fn output_schema_validates_with_safety_check() {
        let schema = OutputSchemaBuilder::new().with_safety_check().build();

        let valid = json!({
            "result": "Task completed",
            "safety_check": {
                "is_safe": true,
                "concerns": []
            }
        });

        let result = schema.validate(&valid);
        assert!(result.is_ok());
    }

    #[test]
    fn output_schema_rejects_missing_safety_check() {
        let schema = OutputSchemaBuilder::new().with_safety_check().build();

        let invalid = json!({
            "result": "Task completed"
        });

        let result = schema.validate(&invalid);
        assert!(result.is_err());
    }

    #[test]
    fn tool_call_validator_creates_with_defaults() {
        let validator = ToolCallValidator::new();
        assert_eq!(validator.max_name_length, 256);
        assert_eq!(validator.max_args_length, 65536);
        assert!(!validator.blocked_patterns.is_empty());
    }

    #[test]
    fn tool_call_validator_accepts_valid_tool_call() {
        let validator = ToolCallValidator::new();
        let result = validator.validate_tool_call("read_file", r#"{"path": "/tmp/test.txt"}"#);
        assert!(result.is_ok());
    }

    #[test]
    fn tool_call_validator_rejects_long_tool_name() {
        let validator = ToolCallValidator::new();
        let long_name = "a".repeat(300);
        let result = validator.validate_tool_call(&long_name, r#"{"path": "/tmp/test.txt"}"#);
        assert!(result.is_err());
    }

    #[test]
    fn tool_call_validator_rejects_invalid_json() {
        let validator = ToolCallValidator::new();
        let result = validator.validate_tool_call("read_file", r#"{"path": invalid json"#);
        assert!(result.is_err());
    }

    #[test]
    fn tool_call_validator_rejects_blocked_patterns() {
        let validator = ToolCallValidator::new();
        let result = validator.validate_tool_call("dangerous_op", r#"{"cmd": "rm -rf /"}"#);
        assert!(result.is_err());
    }

    #[test]
    fn tool_call_validator_accepts_large_but_valid_args() {
        let validator = ToolCallValidator::new();
        let large_arg = "x".repeat(1000);
        let json_args = format!(r#"{{"content": "{}"}}"#, large_arg);
        let result = validator.validate_tool_call("write_file", &json_args);
        assert!(result.is_ok());
    }

    #[test]
    fn tool_call_validator_rejects_oversized_args() {
        let validator = ToolCallValidator::new();
        let huge_arg = "x".repeat(100000); // 100KB
        let json_args = format!(r#"{{"content": "{}"}}"#, huge_arg);
        let result = validator.validate_tool_call("write_file", &json_args);
        assert!(result.is_err());
    }

    #[test]
    fn output_schema_builder_default() {
        let builder = OutputSchemaBuilder::default();
        assert!(!builder.require_tool_calls);
        assert!(!builder.require_reasoning);
        assert!(!builder.require_safety_check);
    }

    #[test]
    fn tool_call_validator_default() {
        let validator = ToolCallValidator::default();
        assert_eq!(validator.max_name_length, 256);
    }

    #[test]
    fn test_validate_effects_catches_dangerous_commands() {
        let overwatch = OverwatchEngine::new();

        let safe_effects = vec![crate::effects::TempestEffect::RunCommand {
            command: "cargo test".to_string(),
            cwd: ".".to_string(),
        }];
        assert_matches!(
            overwatch.validate_effects(&safe_effects, None),
            OverwatchVerdict::Pass
        );

        let dangerous_effects = vec![crate::effects::TempestEffect::RunCommand {
            command: "rm -rf src/".to_string(),
            cwd: ".".to_string(),
        }];
        assert_matches!(
            overwatch.validate_effects(&dangerous_effects, None),
            OverwatchVerdict::Intercept { .. }
        );
    }

    #[test]
    fn test_validate_effects_catches_suspicious_writes() {
        let overwatch = OverwatchEngine::new();

        let tiny_critical_write = vec![crate::effects::TempestEffect::WriteFile {
            path: "Cargo.toml".to_string(),
            content: "small content".to_string(),
            force_overwrite: false,
        }];
        // Without user intent, it should be intercepted
        assert_matches!(
            overwatch.validate_effects(&tiny_critical_write, Some("hello")),
            OverwatchVerdict::Intercept { .. }
        );

        // With user intent, it should pass
        assert_matches!(
            overwatch.validate_effects(&tiny_critical_write, Some("create Cargo.toml hello world")),
            OverwatchVerdict::Pass
        );
    }

    #[test]
    fn test_repair_json_str() {
        let malformed = r#"{"tool":"run_command","arguments":{"command":"grep -rn "Initiate Meltdown" /path/"}}"#;
        let repaired = repair_json_str(malformed);
        assert_eq!(
            repaired,
            r#"{"tool":"run_command","arguments":{"command":"grep -rn \"Initiate Meltdown\" /path/"}}"#
        );
        assert!(serde_json::from_str::<serde_json::Value>(&repaired).is_ok());

        let already_escaped = r#"{"tool":"run_command","arguments":{"command":"grep -rn \"Initiate Meltdown\" /path/"}}"#;
        let repaired_escaped = repair_json_str(already_escaped);
        assert_eq!(repaired_escaped, already_escaped);
        assert!(serde_json::from_str::<serde_json::Value>(&repaired_escaped).is_ok());
    }
}
