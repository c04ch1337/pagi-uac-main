//! pagi-control-panel: bare-metal egui Control Panel for the PAGI Master Orchestrator.
//!
//! Single binary; communicates with the orchestrator via tokio mpsc (pagi_bridge).
//! All public items use the `pagi_` naming convention.

pub mod pagi_control_panel;

pub use pagi_control_panel::{
    pagi_control_panel_channel, PagiBridgeSender, PagiControlPanel, PagiControlPanelMessage,
};
