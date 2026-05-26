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

use skg_context_engine::{Context, ContextOp, EngineError, Rule};
use layer0::content::Content;
use layer0::context::{Message, Role};
use async_trait::async_trait;
use std::collections::{VecDeque, HashMap};
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
        
        self.rule_trigger_history.iter()
            .filter(|(name, timestamp)| {
                name == rule_name && now.saturating_sub(*timestamp) < 60
            })
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
        self.recent_intercepts.retain(|&t| now.saturating_sub(t) < 60);
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
                to interact with the system. Re-issue your response with the proper tool call NOW."
            ),
        ));
        Err(EngineError::Halted {
            reason: "Hallucination intercepted: action claim without tool call. Forcing LLM retry.".into(),
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
                Re-issue with proper tool calls."
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
                to get real results. Your fabricated output has been DISCARDED."
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
    let Some(raw) = last_assistant_text(ctx) else { return false };
    let analysis = strip_thinking_blocks(&raw.to_lowercase());

    let transitions = [
        "i will now", "i'll now", "i'm going to", "let me now",
        "i will use", "i'll use", "i will run", "i'll run",
        "i will execute", "let me execute", "let me read",
        "let me write", "let me check", "i will read",
        "i will write", "i will check",
    ];

    let claims_action = transitions.iter().any(|&t| analysis.contains(t));
    claims_action && !has_tool_call(&raw)
}

/// Predicate: last assistant message claims file I/O without tool call.
fn is_hallucinating_file_io(ctx: &Context) -> bool {
    let Some(raw) = last_assistant_text(ctx) else { return false };
    let analysis = strip_thinking_blocks(&raw.to_lowercase());

    let file_hallucinations = [
        "i have read the file", "i've read the file",
        "here are the contents", "the file contains",
        "the contents of", "i wrote the file",
        "i've written the file", "i saved the code to",
        "the file has been updated", "the file has been created",
        "i updated the file", "i've updated the file",
        "i created the file", "i've created the file",
        "here's what the file looks like", "the code in the file",
    ];

    let claims_file = file_hallucinations.iter().any(|&t| analysis.contains(t));
    claims_file && !has_tool_call(&raw)
}

/// Predicate: last assistant message contains fake tool result markers.
fn is_faking_tool_results(ctx: &Context) -> bool {
    let Some(raw) = last_assistant_text(ctx) else { return false };
    let lower = raw.to_lowercase();

    let fake_markers = [
        "=== tool result ===",
        "=== tool error ===",
    ];

    fake_markers.iter().any(|&m| {
        lower.contains(m) && !lower.contains(&format!("\"{}\"", m))
    })
}

// ============================================================
// 🔴 NEW RULES: JSON SCHEMA VIOLATIONS
// ============================================================

/// Predicate: JSON/Tool call has structural problems
/// (incomplete braces, trailing commas). Does NOT check for missing optional fields.
fn has_json_schema_violation(content: &str) -> bool {
    // Look for incomplete JSON structures in markdown blocks
    if content.contains("```json") {
        if let Some(start) = content.find("```json") {
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
    let Some(current) = last_assistant_text(ctx) else { return false };
    let current_lower = current.to_lowercase();
    
    // Look for patterns like "I did X" but context shows we haven't called any tools
    let contradiction_patterns = [
        ("i have executed", "tool"),      // Claims execution without tool call
        ("i've completed", "tool"),       // Claims completion without result
        ("successfully created", "create"), // Claims creation without tool call
        ("the file now contains", "write"), // Claims write without write tool
        ("i found the error", "error classifier"), // Claims debugging without proper tool
    ];
    
    for (claim, required_evidence) in &contradiction_patterns {
        if current_lower.contains(claim) && !current_lower.contains(required_evidence) {
            // Check if recent history actually shows this action
            let has_evidence = ctx.messages.iter().rev()
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
    let Some(current) = last_assistant_text(ctx) else { return false };
    let current_lower = current.to_lowercase();
    
    // Patterns indicating mission drift
    let scope_violations = [
        ("i will now rewrite", "refactor"), // Unsolicited refactoring
        ("let me optimize", "improve"),      // Scope creep: optimizing without being asked
        ("i'll deploy this", "deploy"),      // Attempting deployment outside scope
        ("i will run a full test suite", "test"), // Running tests not requested
        ("let me check the entire codebase", "scan"), // Scanning beyond scope
        ("i should update all", "batch modify"), // Batch changes without permission
    ];
    
    for (action, verb) in &scope_violations {
        if current_lower.contains(action) {
            // Only flag if there's no recent context supporting this action
            let has_permission = ctx.messages.iter().rev()
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
            "network", "api", "parsing", "concurrent", "race", 
            "timing", "external", "unstable", "unreliable"
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
    
    // Case 2: Naked JSON object containing "tool"
    if let Some(start) = lower.find("{\"tool\":") {
        let json_part = &text[start..];
        let mut depth = 0;
        let mut in_string = false;
        let mut escape = false;
        let mut started = false;
        
        for c in json_part.chars() {
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
                        // Found the balanced end of the JSON object
                        return true;
                    }
                }
                _ => {}
            }
        }
    }
    
    false
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
                or missing \"params\"/\"arguments\". Reformat and retry."
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
                Review your actual tool calls before making claims about completion."
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
                refactor, deploy, or modify unrelated code without permission."
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
                Use appropriate uncertainty language: \"likely\", \"should\", \"may fail\", etc."
            ),
        ));
        Err(EngineError::Halted {
            reason: "Over-confidence on uncertain task. Forcing humility.".into(),
        })
    }
}

// ============================================================
// 🌪️ RULE FACTORY (SKG CONTEXT ENGINE)
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
            "Hallucination Guard",
            100,
            is_hallucinating_action,
            HallucinationGuardOp,
        ),
        Rule::when(
            "File I/O Overwatch",
            100,
            is_hallucinating_file_io,
            FileIOOverwatchOp,
        ),
        Rule::when(
            "Fake Result Guard",
            100,
            is_faking_tool_results,
            FakeToolResultOp,
        ),
        Rule::when(
            "JSON Schema Violation",
            99,
            is_hallucinating_file_io, // Reuse existing predicate adapter until full context available
            JsonSchemaViolationOp,
        ),
        Rule::when(
            "Self-Contradiction Check",
            98,
            has_self_contradiction,
            SelfContradictionOp,
        ),
        Rule::when(
            "Scope Creep Detection",
            97,
            has_scope_creep,
            ScopeCreepOp,
        ),
    ]
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
    fn name(&self) -> &'static str { "Hallucination Guard" }
    fn evaluate(&self, content: &str) -> OverwatchVerdict {
        let analysis = strip_thinking_blocks(&content.to_lowercase());
        let transitions = [
            "i will now", "i'll now", "i'm going to", "let me now",
            "i will use", "i'll use", "i will run", "i'll run",
            "i will execute", "let me execute", "let me read",
            "let me write", "let me check", "i will read",
            "i will write", "i will check",
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
    fn name(&self) -> &'static str { "File I/O Overwatch" }
    fn evaluate(&self, content: &str) -> OverwatchVerdict {
        let analysis = strip_thinking_blocks(&content.to_lowercase());
        let file_hallucinations = [
            "i have read the file", "i've read the file",
            "here are the contents", "the file contains",
            "the contents of", "i wrote the file",
            "i've written the file", "i saved the code to",
            "the file has been updated", "the file has been created",
            "i updated the file", "i've updated the file",
            "i created the file", "i've created the file",
            "here's what the file looks like", "the code in the file",
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
    fn name(&self) -> &'static str { "Fake Result Guard" }
    fn evaluate(&self, content: &str) -> OverwatchVerdict {
        let lower = content.to_lowercase();
        let fake_markers = ["=== tool result ===", "=== tool error ==="];
        if fake_markers.iter().any(|&m| lower.contains(m) && !lower.contains(&format!("\"{}\"", m))) {
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
    fn name(&self) -> &'static str { "JSON Schema Violation" }
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
    fn name(&self) -> &'static str { "Scope Creep Detection" }
    fn evaluate(&self, content: &str) -> OverwatchVerdict {
        let lower = content.to_lowercase();
        let scope_violations = [
            "i will now rewrite", "let me optimize", "i'll deploy",
            "i will run a full test", "let me check the entire", "i should update all"
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
    fn name(&self) -> &'static str { "Over-Confidence Guard" }
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
        
        // Check rate limit first
        if score.record_intercept() {
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
        self.context_score.lock()
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
        self.project_memory.lock()
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

    #[test]
    fn hallucination_guard_catches_empty_promises() {
        let engine = OverwatchEngine::new();
        let content = "I will now read the file and check its contents.";
        assert!(matches!(engine.evaluate_pre_reaction(content), OverwatchVerdict::Intercept { .. }));
    }

    #[test]
    fn hallucination_guard_passes_with_tool_call() {
        let engine = OverwatchEngine::new();
        let content = "I will now read the file.\n```json\n{\"tool\": \"read_file\"}\n```";
        assert!(matches!(engine.evaluate_pre_reaction(content), OverwatchVerdict::Pass));
    }

    #[test]
    fn file_io_catches_invented_contents() {
        let engine = OverwatchEngine::new();
        let content = "I have read the file. Here are the contents:\n\nfn main() { }";
        assert!(matches!(engine.evaluate_pre_reaction(content), OverwatchVerdict::Intercept { .. }));
    }

    #[test]
    fn thinking_blocks_are_excluded() {
        let engine = OverwatchEngine::new();
        let content = "<think>I will now read the file.</think>The task is complete.";
        assert!(matches!(engine.evaluate_pre_reaction(content), OverwatchVerdict::Pass));
    }

    #[test]
    fn fake_result_guard_catches_impersonation() {
        let engine = OverwatchEngine::new();
        let content = "=== TOOL RESULT ===\nFile written successfully to /tmp/foo.rs";
        assert!(matches!(engine.evaluate_pre_reaction(content), OverwatchVerdict::Intercept { .. }));
    }

    #[test]
    fn clean_output_passes() {
        let engine = OverwatchEngine::new();
        assert!(matches!(engine.evaluate_pre_reaction("Task complete."), OverwatchVerdict::Pass));
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
        let content = "```json\n{\"tool\": \"read_file\", \"params\": {\"path\": \"/tmp/file.rs\"}}\n```";
        assert!(!has_json_schema_violation(content));
    }

    // --- NEW TESTS: SCOPE CREEP ---

    #[test]
    fn scope_creep_detects_unsolicited_optimization() {
        let lower = "i will now optimize the entire codebase for performance";
        let scope_violations = [
            "i will now rewrite", "i will now optimize", "i'll deploy",
            "i will run a full test", "let me check the entire", "i should update all"
        ];
        assert!(scope_violations.iter().any(|v| lower.contains(v)));
    }

    #[test]
    fn scope_creep_detects_unauthorized_deployment() {
        let lower = "i'll deploy this to production now";
        let scope_violations = [
            "i will now rewrite", "let me optimize", "i'll deploy",
            "i will run a full test", "let me check the entire", "i should update all"
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
            async fn execute(&self, _ctx: &mut Context) -> Result<(), EngineError> { Ok(()) }
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
            async fn execute(&self, _ctx: &mut Context) -> Result<(), EngineError> { Ok(()) }
        }

        let result = ctx.run(NoOp).await;
        assert!(result.is_ok());
    }
}
