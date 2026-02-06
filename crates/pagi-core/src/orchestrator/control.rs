//! Control-panel protocol: messages from the UI to the Orchestrator.
//!
//! The control panel sends these over a tokio mpsc channel; the orchestrator
//! applies them via `pagi_apply_control_signal` (atomics / RwLock for low overhead).

use serde::{Deserialize, Serialize};

/// Message from the Control Panel to the Master Orchestrator.
/// Use with `tokio::sync::mpsc`; create channel as `mpsc::channel::<ControlPanelMessage>(cap)`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ControlPanelMessage {
    /// Single KB toggle (index 0..7 = KB-1..KB-8).
    KbState { index: usize, active: bool },
    /// Master skills execution switch.
    SkillsEnabled(bool),
    /// Memory layer weights for retrieval scoring.
    MemoryWeights {
        short_term: f32,
        long_term: f32,
    },
    /// Full snapshot; replace orchestrator control state.
    FullState {
        kb_states: [bool; 8],
        skills_enabled: bool,
        short_term_memory_weight: f32,
        long_term_memory_weight: f32,
    },
}
