//! pagi-personal-ui: Egui lightweight dashboard add-on.
//!
//! Focus: system tray integration and minimal resource usage.
//! All AGI reasoning via `pagi_core::Orchestrator::dispatch()`.

pub mod app;

pub use app::{build_orchestrator, default_tenant};

pub fn init() {}