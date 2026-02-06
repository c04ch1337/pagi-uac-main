//! pagi_bridge: async channel between Control Panel UI and Master Orchestrator.
//!
//! Uses the protocol defined in pagi-core (`ControlPanelMessage`). The UI thread
//! uses `try_send` (non-blocking); the orchestrator receives and applies via
//! `pagi_apply_control_signal`.

use pagi_core::ControlPanelMessage;
use tokio::sync::mpsc;

/// Re-export so UI code can use the same type name; protocol lives in pagi-core.
pub use pagi_core::ControlPanelMessage as PagiControlPanelMessage;

/// Sender half of the control-panel → orchestrator channel.
pub type PagiBridgeSender = mpsc::Sender<ControlPanelMessage>;

/// Receiver half (hold on the orchestrator; pass to `Orchestrator::spawn_control_listener`).
pub type PagiBridgeReceiver = mpsc::Receiver<ControlPanelMessage>;

/// Creates a bounded channel for control panel messages.
/// Give the sender to `PagiControlPanel::with_bridge(sender)`.
/// Give the receiver to `Orchestrator::spawn_control_listener(arc_orchestrator, receiver)`.
pub fn pagi_control_panel_channel(
    capacity: usize,
) -> (PagiBridgeSender, PagiBridgeReceiver) {
    mpsc::channel(capacity)
}
