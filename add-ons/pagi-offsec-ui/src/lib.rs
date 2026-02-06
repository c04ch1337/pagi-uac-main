//! pagi-offsec-ui: Ratatui TUI add-on.
//!
//! Focus: raw data streams, network logs, keyboard-driven navigation.
//! All AGI reasoning via `pagi_core::Orchestrator::dispatch()`.

pub mod app;

pub use app::{build_orchestrator, default_tenant};

pub fn init() {}