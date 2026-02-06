//! pagi-companion-ui: High-fidelity Slint GUI add-on.
//!
//! Focus: character-engine traits and avatar rendering.
//! All AGI reasoning goes through `pagi_core::Orchestrator::dispatch()`.

pub mod app;

pub use app::{build_orchestrator, default_tenant};

/// Add-on initialization hook (e.g. for host-driven registration).
pub fn init() {
    // Companion UI is typically run as a standalone binary; extend for host integration.
}
