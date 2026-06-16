use crate::context_manager;
use ollama_rs::generation::chat::ChatMessage;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SentinelAction {
    pub message: String,
    pub needs_compaction: bool,
    pub needs_privilege: bool,
    pub hardcore_kill: bool,
    pub active_sentinels: Vec<String>,
}

use sysinfo::Components;

use std::sync::Mutex;

#[derive(Debug)]
pub struct SentinelState {
    pub last_error_count: Option<usize>,
    pub stagnation_counter: usize,
    pub components: Components,
}

pub struct SentinelManager {
    pub state: Mutex<SentinelState>,
}

impl Clone for SentinelManager {
    fn clone(&self) -> Self {
        let state = self.state.lock().unwrap();
        Self {
            state: Mutex::new(SentinelState {
                last_error_count: state.last_error_count,
                stagnation_counter: state.stagnation_counter,
                components: Components::new(),
            }),
        }
    }
}

impl Default for SentinelManager {
    fn default() -> Self {
        Self::new()
    }
}

impl SentinelManager {
    pub fn new() -> Self {
        Self {
            state: Mutex::new(SentinelState {
                last_error_count: None,
                stagnation_counter: 0,
                components: Components::new_with_refreshed_list(),
            }),
        }
    }

    /// Analyzes the current state and returns an action.
    /// Now always returns Some to ensure the TUI HUD stays populated.
    pub fn analyze_state(
        &self,
        messages: &[ChatMessage],
        ctx_limit: u64,
        repetition_stack: &[(String, String, Option<String>)],
        is_hardcore: bool,
    ) -> Option<SentinelAction> {
        let mut action = SentinelAction {
            message: String::new(),
            needs_compaction: false,
            needs_privilege: false,
            hardcore_kill: false,
            active_sentinels: vec![
                "Context Runway".into(),
                "Privilege Escalator".into(),
                "Compiler Guard".into(),
                "Build Watcher".into(),
                "Thermal Guard".into(),
                "Code Guard".into(),
                "Hallucination Guard".into(),
                "File I/O Overwatch".into(),
                "Fake Result Guard".into(),
            ],
        };

        // 0. Repetition & Redundancy Check
        if !repetition_stack.is_empty() {
            let last = &repetition_stack[repetition_stack.len() - 1];

            // A: Strict Loop (3 in a row)
            if repetition_stack.len() >= 3 {
                let prev1 = &repetition_stack[repetition_stack.len() - 2];
                let prev2 = &repetition_stack[repetition_stack.len() - 3];
                if last == prev1 && last == prev2 {
                    action.message.push_str(&format!("⚠️ [SENTINEL - REPETITION]: You have called '{}' with the same arguments 3 times in a row. YOU ARE LOOPING.\n", last.0));
                    if is_hardcore {
                        action.hardcore_kill = true;
                    }
                }
            }

            // B: Redundant Success (Calling a successful tool again with same args)
            if repetition_stack.len() >= 2 {
                for prev in repetition_stack.iter().take(repetition_stack.len() - 1) {
                    if prev.0 == last.0 && prev.1 == last.1 && prev.2.is_some() {
                        // If it's a read operation, it's definitely redundant
                        if last.0 == "read_file" || last.0 == "search_files" || last.0 == "ls" {
                            action.message.push_str(&format!("⚠️ [SENTINEL - REDUNDANCY]: You already successfully called '{}' with these arguments in this session. CHECK YOUR CONTEXT.\n", last.0));
                            if is_hardcore {
                                action.hardcore_kill = true;
                            }
                            break;
                        }
                    }
                }
            }
        }

        // 1. Context Runway Check
        let threshold = if is_hardcore { 0.40 } else { 0.75 };
        if context_manager::needs_compaction(messages, (ctx_limit as f64 * threshold) as u64) {
            action.message.push_str(&format!("⚠️ [SENTINEL - CONTEXT RUNWAY]: History is >{}% full. Automatic compaction recommended.\n", (threshold * 100.0) as u32));
            action.needs_compaction = true;
        }

        // 2. Privilege Ladder Check & Compiler Guard & Build Watcher
        if let Some(last_msg) = messages.last() {
            let content = last_msg.content.to_lowercase();

            // Privilege Check
            if content.contains("permission denied")
                || content.contains("eacces")
                || content.contains("operation not permitted")
            {
                action.message.push_str("⚠️ [SENTINEL - PRIVILEGE ESCALATION]: Access error detected. Use 'request_privileges'.\n");
                action.needs_privilege = true;
            }

            // Hallucination Guard: Detect "Tool not found" loops or faked results
            // CRITICAL: Only check if the ASSISTANT is outputting these markers.
            // Do not flag legitimate System/Tool results.
            if last_msg.role == ollama_rs::generation::chat::MessageRole::Assistant {
                if content.contains("tool not found in registry")
                    || content.contains("hallucinated a capability")
                {
                    action.message.push_str("⚠️ [SENTINEL - HALLUCINATION GUARD]: Tool hallucination detected. Model is inventing tools.\n");
                    if is_hardcore {
                        action.hardcore_kill = true;
                    }
                }
                if content.contains("=== system observation ===")
                    || content.contains("=== system error ===")
                    || content.contains("=== tool result ===")
                {
                    action.message.push_str("⚠️ [SENTINEL - HALLUCINATION GUARD]: Faked tool output detected. Assistant is pretending to be the system.\n");
                    if is_hardcore {
                        action.hardcore_kill = true;
                    }
                }
            }

            // Compiler Guard: Count errors in the last output
            let error_count = last_msg.content.matches("error:").count()
                + last_msg.content.matches("error[").count();
            if error_count > 0 {
                let mut state = self.state.lock().unwrap();
                if let Some(last_count) = state.last_error_count {
                    if error_count >= last_count {
                        state.stagnation_counter += 1;
                    } else {
                        state.stagnation_counter = 0;
                    }
                }
                state.last_error_count = Some(error_count);

                let stagnation_threshold = if is_hardcore { 1 } else { 3 };
                if state.stagnation_counter >= stagnation_threshold {
                    action.message.push_str(&format!(
                        "⚠️ [SENTINEL - COMPILER GUARD]: Build is STAGNANT ({} errors).\n",
                        error_count
                    ));
                    if is_hardcore {
                        action.hardcore_kill = true;
                    }

                    state.stagnation_counter = 0;
                }
            } else if last_msg.content.contains("FINISHED")
                || last_msg.content.contains("COMPLETED")
            {
                let mut state = self.state.lock().unwrap();
                state.last_error_count = None;
                state.stagnation_counter = 0;
            }

            // Build Watcher: Detect if testing stale code
            if (last_msg.content.contains("running data_analysis_test")
                || last_msg.content.contains("cargo test")
                || last_msg.content.contains("run_tests"))
                && let Ok(src_meta) = std::fs::metadata("src")
            {
                let src_time = src_meta
                    .modified()
                    .unwrap_or(std::time::SystemTime::UNIX_EPOCH);
                let bins = [
                    "target/debug/tempest_ai",
                    "target/release/tempest_ai",
                    "target/debug/test_bin",
                ];
                for bin in bins {
                    if let Ok(bin_meta) = std::fs::metadata(bin)
                        && src_time
                            > bin_meta
                                .modified()
                                .unwrap_or(std::time::SystemTime::UNIX_EPOCH)
                    {
                        action.message.push_str("⚠️ [SENTINEL - BUILD WATCHER]: You are running tests against STALE code.\n");
                    }
                }
            }

            // Code Guard: Detect if the model is dumping raw code without planning
            if last_msg.role == ollama_rs::generation::chat::MessageRole::Assistant {
                let has_code = last_msg.content.contains("```")
                    || last_msg.content.contains("pub fn")
                    || last_msg.content.contains("import ");
                let has_explicit_thought = last_msg.content.contains("THOUGHT:")
                    || last_msg.content.to_lowercase().contains("<think>");
                let has_metadata_thought =
                    last_msg.thinking.as_ref().is_some_and(|t| !t.is_empty());

                if has_code && !has_explicit_thought && !has_metadata_thought {
                    action.message.push_str("⚠️ [SENTINEL - CODE GUARD]: Raw code dump detected without thinking phase.\n");
                    if is_hardcore {
                        action.hardcore_kill = true;
                    }
                }
            }
        }

        // 3. Thermal Guard (Optional/Non-blocking)
        if let Ok(mut state) = self.state.try_lock() {
            // Only refresh sensors every 10 turns to avoid blocking UI/Engine
            state.stagnation_counter += 1;
            if state.stagnation_counter > 0 && state.stagnation_counter % 10 == 0 {
                state.components.refresh(true);
                for comp in &state.components {
                    let temp = comp.temperature().unwrap_or(0.0);
                    if temp > 80.0 {
                        action.message.push_str(&format!(
                            "⚠️ [SENTINEL - THERMAL GUARD]: {} is HOT ({:.1}°C).\n",
                            comp.label(),
                            temp
                        ));
                        break;
                    }
                }
            }
        }

        // Always return the action to keep TUI HUD active
        Some(action)
    }
}
