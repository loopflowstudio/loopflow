//! The mind's state machine.
//!
//! This machine is about the MIND only: workers grinding in the background are
//! not a mind state (wave-level display is derived from `(mind_state,
//! workers_in_flight)`). A failed *turn* is `TurnFinished { status: Failed }`
//! and the mind returns to `Idle`; [`MindState::Failed`] is reserved for the
//! mind itself (thread dead, retries exhausted).
//!
//! `Failed` is produced by the mind's scheduler (consecutive-failure cap or a
//! terminal harness error — see [`super::mind`]); `Failed → Idle` fires when
//! a user message revives the mind. `Interrupting` is produced by a user
//! interrupt op (`Turning → Interrupting`, cooperative cancel) and is
//! deadline-bounded: the mind's janitor force-finalizes the turn if the
//! harness never delivers its terminal event (see `mind::INTERRUPT_DEADLINE`).
//!
//! Transitions go through [`can_transition`]; an illegal move is a bug —
//! logged and refused by the caller (see `WaveRuntime::transition`), never
//! silently applied. Every legal transition appends a `MindState` event to the
//! wave journal.

use serde::{Deserialize, Serialize};

/// Current state of the wave's mind. Serialized into journal `MindState`
/// events; internal persistence, not a wire DTO.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum MindState {
    /// Thread alive, no turn in flight.
    Idle,
    /// One turn generating / tool-calling.
    Turning { turn_id: String },
    /// Cancel fired; cooperative → grace → kill.
    Interrupting { turn_id: String },
    /// The mind itself is dead (retries exhausted); algedonic.
    Failed { reason: String },
}

impl MindState {
    /// Short name for `/health` and logs.
    pub fn name(&self) -> &'static str {
        match self {
            Self::Idle => "idle",
            Self::Turning { .. } => "turning",
            Self::Interrupting { .. } => "interrupting",
            Self::Failed { .. } => "failed",
        }
    }
}

/// The transition table. `true` means the move is legal.
///
/// - `Idle → Turning`: a turn opens.
/// - `Turning → Idle`: the turn finalized (completed, failed, or interrupted —
///   turn failure is a `TurnFinished` status, not a mind failure).
/// - `Turning → Interrupting`: cancel fired for the *same* turn.
/// - `Interrupting → Idle`: the interrupted turn finalized.
/// - any live state `→ Failed`: the mind died.
/// - `Failed → Idle`: recovery (restart janitor / thread respawn).
pub fn can_transition(from: &MindState, to: &MindState) -> bool {
    use MindState::*;
    match (from, to) {
        (Idle, Turning { .. }) => true,
        (Turning { .. }, Idle) => true,
        (Turning { turn_id: a }, Interrupting { turn_id: b }) => a == b,
        (Interrupting { .. }, Idle) => true,
        (Failed { .. }, Idle) => true,
        (Failed { .. }, Failed { .. }) => false,
        (_, Failed { .. }) => true,
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn turning(id: &str) -> MindState {
        MindState::Turning {
            turn_id: id.to_string(),
        }
    }

    fn interrupting(id: &str) -> MindState {
        MindState::Interrupting {
            turn_id: id.to_string(),
        }
    }

    fn failed(reason: &str) -> MindState {
        MindState::Failed {
            reason: reason.to_string(),
        }
    }

    #[test]
    fn turn_lifecycle_is_legal() {
        assert!(can_transition(&MindState::Idle, &turning("turn-1")));
        assert!(can_transition(&turning("turn-1"), &MindState::Idle));
    }

    #[test]
    fn interrupt_targets_the_open_turn_only() {
        assert!(can_transition(&turning("turn-3"), &interrupting("turn-3")));
        assert!(!can_transition(&turning("turn-3"), &interrupting("turn-9")));
        assert!(can_transition(&interrupting("turn-3"), &MindState::Idle));
        // Nothing to interrupt when idle.
        assert!(!can_transition(&MindState::Idle, &interrupting("turn-3")));
    }

    #[test]
    fn any_live_state_can_fail_but_failed_is_sticky() {
        assert!(can_transition(&MindState::Idle, &failed("spawn error")));
        assert!(can_transition(&turning("turn-1"), &failed("thread died")));
        assert!(can_transition(
            &interrupting("turn-1"),
            &failed("kill hung")
        ));
        assert!(!can_transition(&failed("a"), &failed("b")));
    }

    #[test]
    fn failed_recovers_only_to_idle() {
        assert!(can_transition(&failed("dead"), &MindState::Idle));
        assert!(!can_transition(&failed("dead"), &turning("turn-1")));
        assert!(!can_transition(&failed("dead"), &interrupting("turn-1")));
    }

    #[test]
    fn no_self_loops_or_skips() {
        assert!(!can_transition(&MindState::Idle, &MindState::Idle));
        assert!(!can_transition(&turning("turn-1"), &turning("turn-2")));
        assert!(!can_transition(&interrupting("turn-1"), &turning("turn-1")));
    }

    #[test]
    fn state_names_for_health() {
        assert_eq!(MindState::Idle.name(), "idle");
        assert_eq!(turning("turn-1").name(), "turning");
        assert_eq!(interrupting("turn-1").name(), "interrupting");
        assert_eq!(failed("x").name(), "failed");
    }

    #[test]
    fn mind_state_round_trips_through_json() {
        for state in [
            MindState::Idle,
            turning("turn-4"),
            interrupting("turn-4"),
            failed("thread died"),
        ] {
            let value = serde_json::to_value(&state).expect("serialize");
            let decoded: MindState = serde_json::from_value(value).expect("deserialize");
            assert_eq!(decoded, state);
        }
    }
}
