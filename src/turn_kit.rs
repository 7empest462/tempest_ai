use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum TurnPhase {
    Planning,
    Executing,
    Verifying,
}

pub struct TempestTurnDecomposer {
    pub current_phase: TurnPhase,
}

impl TempestTurnDecomposer {
    pub fn new() -> Self {
        Self {
            current_phase: TurnPhase::Planning,
        }
    }

    pub fn transition_phase(&mut self, next: TurnPhase) -> &'static str {
        self.current_phase = next;
        match self.current_phase {
            TurnPhase::Planning => "Planning Phase",
            TurnPhase::Executing => "Execution Phase",
            TurnPhase::Verifying => "Verification Phase",
        }
    }
}
