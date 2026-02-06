//! Bare-metal Control Panel for the PAGI Master Orchestrator.
//!
//! Manages 8 Knowledge Base toggles, skills execution toggle, and memory-layer
//! weight sliders. Broadcasts state changes to the orchestrator via `pagi_bridge`
//! (tokio mpsc).

mod bridge;

pub use bridge::{pagi_control_panel_channel, PagiControlPanelMessage, PagiBridgeSender};

use egui::{Color32, RichText, Ui};

/// Active status of the 8 Knowledge Bases and skills; memory weight distribution.
#[derive(Debug, Clone)]
pub struct PagiControlPanel {
    /// KB-1 .. KB-8 active (included in retrieval/execution).
    pub kb_states: [bool; 8],
    /// Master switch for the skills execution engine.
    pub skills_enabled: bool,
    /// Weight for short-term memory layer (0..=1).
    pub short_term_memory_weight: f32,
    /// Weight for long-term memory layer (0..=1).
    pub long_term_memory_weight: f32,
    /// Optional sender to broadcast changes to the orchestrator (pagi_bridge).
    sender: Option<PagiBridgeSender>,
}

impl Default for PagiControlPanel {
    fn default() -> Self {
        Self {
            kb_states: [true; 8],
            skills_enabled: true,
            short_term_memory_weight: 0.7,
            long_term_memory_weight: 0.3,
            sender: None,
        }
    }
}

impl PagiControlPanel {
    /// Creates a panel with no bridge (standalone UI only).
    pub fn new() -> Self {
        Self::default()
    }

    /// Attaches a bridge sender so state changes are sent to the orchestrator.
    pub fn with_bridge(mut self, sender: PagiBridgeSender) -> Self {
        self.sender = Some(sender);
        self
    }

    /// Renders the control panel UI (egui immediate mode). Call each frame from your eframe app.
    pub fn pagi_ui(&mut self, ui: &mut Ui) {
        ui.heading(
            RichText::new("Pagi Master Orchestrator Control").color(Color32::from_rgb(100, 180, 255)),
        );
        ui.separator();

        // Multi-Layer Memory Controls
        ui.group(|ui| {
            ui.label("Memory Layer Distribution");
            if ui
                .add(
                    egui::Slider::new(&mut self.short_term_memory_weight, 0.0..=1.0)
                        .text("Short-Term"),
                )
                .changed()
            {
                self.pagi_try_send(PagiControlPanelMessage::MemoryWeights {
                    short_term: self.short_term_memory_weight,
                    long_term: self.long_term_memory_weight,
                });
            }
            if ui
                .add(
                    egui::Slider::new(&mut self.long_term_memory_weight, 0.0..=1.0)
                        .text("Long-Term"),
                )
                .changed()
            {
                self.pagi_try_send(PagiControlPanelMessage::MemoryWeights {
                    short_term: self.short_term_memory_weight,
                    long_term: self.long_term_memory_weight,
                });
            }
        });

        // 8 Knowledge Base Toggles
        ui.collapsing("Knowledge Bases (KB-1 to KB-8)", |ui| {
            for i in 0..8 {
                let label = format!("KB-{} Active", i + 1);
                if ui.checkbox(&mut self.kb_states[i], label).changed() {
                    self.pagi_try_send(PagiControlPanelMessage::KbState {
                        index: i,
                        active: self.kb_states[i],
                    });
                }
            }
        });

        // Skill-Type Solution Toggle
        ui.group(|ui| {
            ui.horizontal(|ui| {
                ui.label("Skills Execution Engine:");
                let label = if self.skills_enabled {
                    "ENABLED"
                } else {
                    "DISABLED"
                };
                if ui.checkbox(&mut self.skills_enabled, label).changed() {
                    self.pagi_try_send(PagiControlPanelMessage::SkillsEnabled(
                        self.skills_enabled,
                    ));
                }
            });
        });
    }

    /// Sends the current full state to the orchestrator (e.g. on connect or snapshot).
    /// No-op if no bridge is attached.
    pub fn pagi_update_orchestrator(&self) {
        if let Some(ref tx) = self.sender {
            let _ = tx.try_send(PagiControlPanelMessage::FullState {
                kb_states: self.kb_states,
                skills_enabled: self.skills_enabled,
                short_term_memory_weight: self.short_term_memory_weight,
                long_term_memory_weight: self.long_term_memory_weight,
            });
        }
    }

    fn pagi_try_send(&self, msg: PagiControlPanelMessage) {
        if let Some(ref tx) = self.sender {
            let _ = tx.try_send(msg);
        }
    }
}
