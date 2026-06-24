use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum TurnPhase {
    Planning,
    Executing,
    Verifying,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct VerificationHook {
    pub name: String,
    pub command: String,
}

pub struct TempestTurnDecomposer {
    pub current_phase: TurnPhase,
    pub hooks: Vec<VerificationHook>,
    pub phase_start: std::time::Instant,
    pub planning_duration_ms: u64,
    pub executing_duration_ms: u64,
    pub verifying_duration_ms: u64,
    pub kv_cache_hit_pct: Option<f32>,
}

impl Default for TempestTurnDecomposer {
    fn default() -> Self {
        Self::new()
    }
}

impl TempestTurnDecomposer {
    pub fn new() -> Self {
        Self {
            current_phase: TurnPhase::Planning,
            hooks: Vec::new(),
            phase_start: std::time::Instant::now(),
            planning_duration_ms: 0,
            executing_duration_ms: 0,
            verifying_duration_ms: 0,
            kv_cache_hit_pct: None,
        }
    }

    /// Registers a verification hook to run at the end of tool execution during the Verifying phase.
    pub fn register_hook(&mut self, hook: VerificationHook) {
        self.hooks.push(hook);
    }

    /// Checks if a transition from `from` to `to` is legally valid in the turn lifecycle state machine.
    pub fn is_valid_transition(&self, from: &TurnPhase, to: &TurnPhase) -> bool {
        match (from, to) {
            // You can always start or loop in Planning
            (TurnPhase::Planning, TurnPhase::Planning) => true,
            (TurnPhase::Planning, TurnPhase::Executing) => true,
            (TurnPhase::Planning, TurnPhase::Verifying) => true,

            // Executing can transition to Verifying (normal), Planning (re-planning/failure), or Executing (multi-step loops)
            (TurnPhase::Executing, TurnPhase::Verifying) => true,
            (TurnPhase::Executing, TurnPhase::Planning) => true,
            (TurnPhase::Executing, TurnPhase::Executing) => true,

            // Verifying can transition to Planning (verification failed/re-plan), Verifying (loop), or Executing (recovery/new turn)
            (TurnPhase::Verifying, TurnPhase::Planning) => true,
            (TurnPhase::Verifying, TurnPhase::Verifying) => true,
            (TurnPhase::Verifying, TurnPhase::Executing) => true,
        }
    }

    pub fn transition_phase(&mut self, next: TurnPhase) -> &'static str {
        if !self.is_valid_transition(&self.current_phase, &next) {
            eprintln!(
                "🌪️ [SKELEGENT TURN-KIT] [GUARDRAIL WARNING]: Invalid phase transition from {:?} to {:?}",
                self.current_phase, next
            );
        }

        // Accumulate elapsed duration for the phase being exited
        let elapsed = self.phase_start.elapsed().as_millis() as u64;
        match self.current_phase {
            TurnPhase::Planning => self.planning_duration_ms += elapsed,
            TurnPhase::Executing => self.executing_duration_ms += elapsed,
            TurnPhase::Verifying => self.verifying_duration_ms += elapsed,
        }
        self.phase_start = std::time::Instant::now();

        self.current_phase = next;
        match self.current_phase {
            TurnPhase::Planning => "Planning Phase",
            TurnPhase::Executing => "Execution Phase",
            TurnPhase::Verifying => "Verification Phase",
        }
    }

    pub fn finalize(&mut self) {
        let elapsed = self.phase_start.elapsed().as_millis() as u64;
        match self.current_phase {
            TurnPhase::Planning => self.planning_duration_ms += elapsed,
            TurnPhase::Executing => self.executing_duration_ms += elapsed,
            TurnPhase::Verifying => self.verifying_duration_ms += elapsed,
        }
        self.phase_start = std::time::Instant::now();
    }

    pub fn format_telemetry(&self) -> String {
        let mut parts = vec![format!(
            "⏱️ Phase Durations: Planning: {}ms | Execution: {}ms | Verification: {}ms",
            self.planning_duration_ms, self.executing_duration_ms, self.verifying_duration_ms
        )];
        if let Some(hit_pct) = self.kv_cache_hit_pct {
            parts.push(format!("KV Cache Hit: {:.1}%", hit_pct));
        }
        parts.join(" | ")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_transitions() {
        let mut decomposer = TempestTurnDecomposer::new();
        assert_eq!(decomposer.current_phase, TurnPhase::Planning);

        // Planning -> Executing
        decomposer.transition_phase(TurnPhase::Executing);
        assert_eq!(decomposer.current_phase, TurnPhase::Executing);

        // Executing -> Verifying
        decomposer.transition_phase(TurnPhase::Verifying);
        assert_eq!(decomposer.current_phase, TurnPhase::Verifying);

        // Verifying -> Planning
        decomposer.transition_phase(TurnPhase::Planning);
        assert_eq!(decomposer.current_phase, TurnPhase::Planning);
    }

    #[test]
    fn test_loop_and_recovery_transitions() {
        let decomposer = TempestTurnDecomposer::new();

        // Executing -> Executing (Valid loop)
        assert!(decomposer.is_valid_transition(&TurnPhase::Executing, &TurnPhase::Executing));

        // Verifying -> Executing (Valid recovery or new execution turn)
        assert!(decomposer.is_valid_transition(&TurnPhase::Verifying, &TurnPhase::Executing));
    }

    #[test]
    fn test_register_hooks() {
        let mut decomposer = TempestTurnDecomposer::new();
        assert!(decomposer.hooks.is_empty());

        let hook = VerificationHook {
            name: "Cargo Test".to_string(),
            command: "cargo test".to_string(),
        };
        decomposer.register_hook(hook.clone());
        assert_eq!(decomposer.hooks.len(), 1);
        assert_eq!(decomposer.hooks[0], hook);
    }

    #[test]
    fn test_phase_duration_tracking() {
        let mut decomposer = TempestTurnDecomposer::new();
        // Artificially set phase_start to simulate elapsed time in Planning
        decomposer.phase_start = std::time::Instant::now() - std::time::Duration::from_millis(50);

        decomposer.transition_phase(TurnPhase::Executing);
        assert!(decomposer.planning_duration_ms >= 50);
        assert_eq!(decomposer.executing_duration_ms, 0);

        // Simulate elapsed time in Execution
        decomposer.phase_start = std::time::Instant::now() - std::time::Duration::from_millis(30);
        decomposer.transition_phase(TurnPhase::Verifying);
        assert!(decomposer.executing_duration_ms >= 30);
        assert_eq!(decomposer.verifying_duration_ms, 0);

        // Simulate elapsed time in Verification
        decomposer.phase_start = std::time::Instant::now() - std::time::Duration::from_millis(20);
        decomposer.finalize();
        assert!(decomposer.verifying_duration_ms >= 20);

        let report = decomposer.format_telemetry();
        assert!(report.contains("Planning:"));
        assert!(report.contains("Execution:"));
        assert!(report.contains("Verification:"));
    }
}
