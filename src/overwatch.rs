//! Overwatch Context Rules Engine
//! 
//! Implements Skelegent ContextOps for catching model hallucinations
//! before they reach the user. These rules apply backpressure to the agent loop
//! by injecting system corrections and forcing re-rolls when the model lies.
//!
//! Built on top of `skg_context_engine::ContextOp` and `Rule` with `Trigger::When`
//! predicates so they participate in the SKG context pipeline natively.

use skg_context_engine::{Context, ContextOp, EngineError, Rule};
use layer0::content::Content;
use layer0::context::{Message, Role};
use async_trait::async_trait;

// ============================================================
// 🛡️ HALLUCINATION GUARD
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
// 🌪️ RULE FACTORY
// ============================================================

/// Build the set of Overwatch rules for registration into a Context.
///
/// These are `Rule::when` rules with `Trigger::When` predicates that
/// fire automatically during `Context::run()` calls. They have maximum
/// priority (100) so they fire before any other rules.
pub fn overwatch_rules() -> Vec<Rule> {
    vec![
        Rule::when(
            "Hallucination Guard",
            100, // Max priority — fire first
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

/// The fast-path overwatch engine used directly by the agent's StreamingContent handler.
pub struct OverwatchEngine {
    rules: Vec<Box<dyn OverwatchRule>>,
}

impl OverwatchEngine {
    /// Create a new engine with the default rule set.
    pub fn new() -> Self {
        Self {
            rules: vec![
                Box::new(HallucinationGuardRule),
                Box::new(FileIOOverwatchFastRule),
                Box::new(FakeToolResultFastRule),
            ],
        }
    }

    /// Run all rules against the assistant output.
    /// Returns the first Intercept verdict found, or Pass if all rules pass.
    pub fn evaluate_pre_reaction(&self, content: &str) -> OverwatchVerdict {
        for rule in &self.rules {
            match rule.evaluate(content) {
                OverwatchVerdict::Pass => continue,
                intercept => return intercept,
            }
        }
        OverwatchVerdict::Pass
    }

    /// Return the names of all registered rules (for TUI HUD).
    pub fn rule_names(&self) -> Vec<String> {
        self.rules.iter().map(|r| r.name().to_string()).collect()
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

    // --- SKG Context Engine integration tests ---

    #[tokio::test]
    async fn skg_hallucination_rule_fires_on_predicate() {
        let rules = overwatch_rules();
        let mut ctx = Context::with_rules(rules);

        // Simulate an assistant message that claims action without a tool call
        ctx.messages.push(Message::new(
            Role::Assistant,
            Content::text("I will now read the file and check its contents."),
        ));

        // The When predicate should match and the HallucinationGuardOp should halt
        struct NoOp;
        #[async_trait]
        impl ContextOp for NoOp {
            type Output = ();
            async fn execute(&self, _ctx: &mut Context) -> Result<(), EngineError> { Ok(()) }
        }

        let result = ctx.run(NoOp).await;
        assert!(result.is_err());
        // The correction message should have been injected
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
