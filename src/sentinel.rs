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
    pub ctx_limit: u64,
    pub state: Mutex<SentinelState>,
}

impl Clone for SentinelManager {
    fn clone(&self) -> Self {
        let state = self.state.lock().unwrap();
        Self {
            ctx_limit: self.ctx_limit,
            state: Mutex::new(SentinelState {
                last_error_count: state.last_error_count,
                stagnation_counter: state.stagnation_counter,
                components: Components::new(),
            }),
        }
    }
}

impl SentinelManager {
    pub fn new(ctx_limit: u64) -> Self {
        Self { 
            ctx_limit,
            state: Mutex::new(SentinelState {
                last_error_count: None,
                stagnation_counter: 0,
                components: Components::new_with_refreshed_list(),
            }),
        }
    }

    /// Analyzes the current state and returns an optional action if a sentinel triggers.
    pub fn analyze_state(&self, messages: &[ChatMessage]) -> Option<SentinelAction> {
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
            ],
        };
        let mut triggered = false;

        // 1. Context Runway Check
        if context_manager::needs_compaction(messages, self.ctx_limit) {
            action.message.push_str("⚠️ [SENTINEL - CONTEXT RUNWAY]: History is >85% full. Automatic compaction recommended.\n");
            action.needs_compaction = true;
            triggered = true;
        }

        // 2. Privilege Ladder Check & Compiler Guard & Build Watcher
        if let Some(last_msg) = messages.last() {
            let content = last_msg.content.to_lowercase();
            
            // Privilege Check
            if content.contains("permission denied") || content.contains("eacces") || content.contains("operation not permitted") {
                action.message.push_str("⚠️ [SENTINEL - PRIVILEGE ESCALATION]: Access error detected. Use 'request_privileges' or ensure you have sudo authority.\n");
                action.needs_privilege = true;
                triggered = true;
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
                    action.message.push_str(&format!("⚠️ [SENTINEL - COMPILER GUARD]: Build is STAGNANT ({} errors). STOP and RE-EVALUATE.\n", error_count));
                    triggered = true;
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
                                triggered = true;
                            }
                        }
                    }
                }
            }
        }

        // 3. Thermal Guard
        {
            let mut state = self.state.lock().unwrap();
            state.components.refresh(true);
            for comp in &state.components {
                let temp = comp.temperature().unwrap_or(0.0);
                if temp > 80.0 {
                    action.message.push_str(&format!("⚠️ [SENTINEL - THERMAL GUARD]: {} is running HOT ({:.1}°C). Throttling background work.\n", comp.label(), temp));
                    triggered = true;
                    break;
                }
            }
        }

        if triggered {
            Some(action)
        } else {
            // Even if not triggered, return the active sentinels list
            Some(action)
        }
    }
}
