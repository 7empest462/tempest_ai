use serde::{Deserialize, Serialize};
use crate::context_manager;
use ollama_rs::generation::chat::ChatMessage;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SentinelAction {
    pub message: String,
    pub needs_compaction: bool,
    pub needs_privilege: bool,
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
    pub fn analyze_state(&self, messages: &[ChatMessage], ctx_limit: u64, repetition_stack: &[(String, String)]) -> Option<SentinelAction> {
        let mut action = SentinelAction {
            message: String::new(),
            needs_compaction: false,
            needs_privilege: false,
            active_sentinels: vec![
                "Context Runway".into(),
                "Privilege Escalator".into(),
                "Compiler Guard".into(),
                "Build Watcher".into(),
                "Thermal Guard".into(),
                "Code Guard".into(),
                "Hallucination Guard".into(),
            ],
        };
        
        // 0. Repetition Check
        if repetition_stack.len() >= 3 {
            let last = &repetition_stack[repetition_stack.len() - 1];
            let prev1 = &repetition_stack[repetition_stack.len() - 2];
            let prev2 = &repetition_stack[repetition_stack.len() - 3];
            
            if last == prev1 && last == prev2 {
                action.message.push_str(&format!("⚠️ [SENTINEL - REPETITION]: You have called '{}' with the same arguments 3 times in a row. YOU ARE LOOPING.\n", last.0));
                
            }
        }

        // 1. Context Runway Check
        if context_manager::needs_compaction(messages, ctx_limit) {
            action.message.push_str("⚠️ [SENTINEL - CONTEXT RUNWAY]: History is >85% full. Automatic compaction recommended.\n");
            action.needs_compaction = true;
            
        }

        // 2. Privilege Ladder Check & Compiler Guard & Build Watcher
        if let Some(last_msg) = messages.last() {
            let content = last_msg.content.to_lowercase();
            
            // Privilege Check
            if content.contains("permission denied") || content.contains("eacces") || content.contains("operation not permitted") {
                action.message.push_str("⚠️ [SENTINEL - PRIVILEGE ESCALATION]: Access error detected. Use 'request_privileges'.\n");
                action.needs_privilege = true;
                
            }

            // Hallucination Guard: Detect "Tool not found" loops or faked results
            if content.contains("tool not found in registry") || content.contains("hallucinated a capability") {
                action.message.push_str("⚠️ [SENTINEL - HALLUCINATION GUARD]: Tool hallucination detected. Model is inventing tools.\n");
                
            }
            if content.contains("tool result") || content.contains("tool error") || content.contains("output shown below") {
                action.message.push_str("⚠️ [SENTINEL - HALLUCINATION GUARD]: Faked tool output detected. Assistant is pretending to be the system.\n");
                
            }

            // Compiler Guard: Count errors in the last output
            let error_count = last_msg.content.matches("error:").count() + last_msg.content.matches("error[").count();
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

                if state.stagnation_counter >= 3 {
                    action.message.push_str(&format!("⚠️ [SENTINEL - COMPILER GUARD]: Build is STAGNANT ({} errors).\n", error_count));
                    
                    state.stagnation_counter = 0; 
                }
            } else if last_msg.content.contains("FINISHED") || last_msg.content.contains("COMPLETED") {
                let mut state = self.state.lock().unwrap();
                state.last_error_count = None;
                state.stagnation_counter = 0;
            }

            // Build Watcher: Detect if testing stale code
            if last_msg.content.contains("running data_analysis_test") || last_msg.content.contains("cargo test") || last_msg.content.contains("run_tests") {
                if let Ok(src_meta) = std::fs::metadata("src") {
                    let src_time = src_meta.modified().unwrap_or(std::time::SystemTime::UNIX_EPOCH);
                    let bins = ["target/debug/tempest_ai", "target/release/tempest_ai", "target/debug/test_bin"];
                    for bin in bins {
                        if let Ok(bin_meta) = std::fs::metadata(bin) {
                            if src_time > bin_meta.modified().unwrap_or(std::time::SystemTime::UNIX_EPOCH) {
                                action.message.push_str("⚠️ [SENTINEL - BUILD WATCHER]: You are running tests against STALE code.\n");
                                
                            }
                        }
                    }
                }
            }

            // Code Guard: Detect if the model is dumping raw code without planning
            if (last_msg.content.contains("```") || last_msg.content.contains("pub fn") || last_msg.content.contains("import ")) 
                && !last_msg.content.contains("THOUGHT:") && !last_msg.content.to_lowercase().contains("<think>") {
                action.message.push_str("⚠️ [SENTINEL - CODE GUARD]: Raw code dump detected without thinking phase.\n");
                
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
                        action.message.push_str(&format!("⚠️ [SENTINEL - THERMAL GUARD]: {} is HOT ({:.1}°C).\n", comp.label(), temp));
                        break;
                    }
                }
            }
        }

        // Always return the action to keep TUI HUD active
        Some(action)
    }
}
