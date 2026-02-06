//! pagi-studio-ui: Developer's Cockpit — Standard Reference Frontend.
//!
//! Google AI Studio style. Primary add-on; all AGI via `pagi_core::Orchestrator::dispatch()`.
//! Prompt/Response mapped to short-term memory; 8 KB status panel.

pub mod app;
pub mod config;

pub use app::{build_studio_stack, StudioStack, MEMORY_PROMPT_PATH, MEMORY_RESPONSE_PATH};
pub use config::StudioConfig;

pub fn init() {}