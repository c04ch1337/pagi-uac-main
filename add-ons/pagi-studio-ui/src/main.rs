//! PAGI Studio UI — Developer's Cockpit. Standard Reference Frontend (Google AI Studio style).
//! Send triggers pagi_core::Orchestrator::dispatch; Control tab sends ControlPanelMessage to the same receiver.
//! Skill Tester: manual execution of any registered skill with raw JSON input (bypasses Orchestrator reasoning).

use eframe::egui;
use pagi_core::{pagi_kb_slot_label, ControlPanelMessage, Goal, TenantContext};
use pagi_studio_ui::{
    build_studio_stack, config::StudioConfig, StudioStack, MEMORY_PROMPT_PATH, MEMORY_RESPONSE_PATH,
};
use std::sync::mpsc;
use std::sync::Arc;
use std::time::Instant;

fn main() -> eframe::Result<()> {
    let storage = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
    let storage = storage.join("data");
    let (stack, ctx) = build_studio_stack(&storage).expect("build studio stack");

    let studio_config = StudioConfig::load();
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([studio_config.window_width, studio_config.window_height])
            .with_title("PAGI Studio — Developer's Cockpit"),
        ..Default::default()
    };

    eframe::run_native(
        "PAGI Studio",
        options,
        Box::new(move |_cc| {
            Ok(Box::new(StudioApp::new(stack, ctx, studio_config)))
        }),
    )
}

/// Result from pagi_execute_skill_async: Ok((json_output, elapsed_ms)) or Err((error_message, elapsed_ms)).
type PagiSkillTesterResult = Result<(String, u64), (String, u64)>;

struct StudioApp {
    stack: Arc<StudioStack>,
    ctx: TenantContext,
    config: StudioConfig,
    prompt: String,
    response: String,
    kb_status: Vec<(u8, usize)>,
    kb_status_dirty: bool,
    control_kb: [bool; 8],
    control_skills_enabled: bool,
    /// Skill Tester: selected index, raw JSON input, receiver for async result, last result, pending flag.
    pagi_skill_tester_selected: usize,
    pagi_skill_tester_input: String,
    pagi_skill_tester_rx: Option<mpsc::Receiver<PagiSkillTesterResult>>,
    pagi_skill_tester_last: Option<PagiSkillTesterResult>,
    pagi_skill_tester_pending: bool,
}

impl StudioApp {
    fn new(stack: StudioStack, ctx: TenantContext, config: StudioConfig) -> Self {
        let stack = Arc::new(stack);
        let prompt = load_from_memory(&stack, &ctx, MEMORY_PROMPT_PATH);
        let response = load_from_memory(&stack, &ctx, MEMORY_RESPONSE_PATH);
        let kb_status = (1..=8).map(|id| (id, 0)).collect();
        Self {
            control_kb: [true; 8],
            control_skills_enabled: true,
            pagi_skill_tester_selected: 0,
            pagi_skill_tester_input: String::new(),
            pagi_skill_tester_rx: None,
            pagi_skill_tester_last: None,
            pagi_skill_tester_pending: false,
            stack,
            ctx,
            config,
            prompt,
            response,
            kb_status,
            kb_status_dirty: true,
        }
    }

    /// Spawns a thread that runs the skill via orchestrator.dispatch(ExecuteSkill) and sends the result back.
    /// Sandboxes execution so a hanging skill (e.g. slow network) does not freeze the UI.
    fn pagi_fire_skill(&mut self) {
        if self.pagi_skill_tester_pending {
            return;
        }
        let skill_names = self.stack.skill_names.clone();
        if skill_names.is_empty() {
            return;
        }
        let index = self.pagi_skill_tester_selected.min(skill_names.len().saturating_sub(1));
        let skill_name = skill_names[index].clone();
        let payload = if self.pagi_skill_tester_input.trim().is_empty() {
            serde_json::Value::Null
        } else {
            match serde_json::from_str::<serde_json::Value>(self.pagi_skill_tester_input.trim()) {
                Ok(v) => v,
                Err(e) => {
                    self.pagi_skill_tester_last =
                        Some(Err((format!("Invalid JSON: {}", e), 0)));
                    return;
                }
            }
        };
        let orch = Arc::clone(&self.stack.orchestrator);
        let tenant_ctx = self.ctx.clone();
        let (tx, rx) = mpsc::channel();
        self.pagi_skill_tester_rx = Some(rx);
        self.pagi_skill_tester_pending = true;
        std::thread::spawn(move || {
            let start = Instant::now();
            let result = tokio::runtime::Runtime::new()
                .unwrap()
                .block_on(orch.dispatch(
                    &tenant_ctx,
                    Goal::ExecuteSkill {
                        name: skill_name,
                        payload: Some(payload),
                    },
                ));
            let elapsed_ms = start.elapsed().as_millis() as u64;
            let send = match result {
                Ok(v) => Ok((
                    serde_json::to_string_pretty(&v).unwrap_or_else(|_| v.to_string()),
                    elapsed_ms,
                )),
                Err(e) => Err((e.to_string(), elapsed_ms)),
            };
            let _ = tx.send(send);
        });
    }

    fn send_control(&self, msg: ControlPanelMessage) {
        let _ = self.stack.control_tx.try_send(msg);
    }

    fn refresh_kb_status(&mut self) {
        self.kb_status = (1..=8)
            .map(|slot_id| {
                let n = self
                    .stack
                    .knowledge
                    .scan_keys(slot_id)
                    .map(|k| k.len())
                    .unwrap_or(0);
                (slot_id, n)
            })
            .collect();
        self.kb_status_dirty = false;
    }
}

fn load_from_memory(stack: &StudioStack, ctx: &TenantContext, path: &str) -> String {
    stack
        .memory
        .get_path(ctx, path)
        .ok()
        .flatten()
        .and_then(|b| String::from_utf8(b).ok())
        .unwrap_or_default()
}

fn save_to_memory(stack: &StudioStack, ctx: &TenantContext, path: &str, value: &str) {
    let _ = stack.memory.save_path(ctx, path, value.as_bytes());
}

impl eframe::App for StudioApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if self.kb_status_dirty {
            self.refresh_kb_status();
        }

        // Poll Skill Tester result from worker thread
        if self.pagi_skill_tester_pending {
            if let Some(ref rx) = self.pagi_skill_tester_rx {
                if let Ok(r) = rx.try_recv() {
                    self.pagi_skill_tester_last = Some(r);
                    self.pagi_skill_tester_pending = false;
                    self.pagi_skill_tester_rx = None;
                }
            }
        }

        egui::TopBottomPanel::top("studio_header").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.heading("PAGI Studio — Developer's Cockpit");
                ui.label("(Standard Reference Frontend)");
            });
        });

        egui::SidePanel::left("kb_panel")
            .resizable(true)
            .default_width(280.0)
            .show(ctx, |ui| {
                ui.heading("Knowledge Bases (8 Sled trees)");
                ui.label("Status from pagi_core control state:");
                ui.separator();
                for (slot_id, count) in &self.kb_status {
                    let active = self.stack.orchestrator.pagi_kb_active(*slot_id);
                    let status = if active { "active" } else { "inactive" };
                    let name = pagi_kb_slot_label(*slot_id);
                    let label = if *count == 0 {
                        format!("{} — {} (0 keys)", name, status)
                    } else {
                        format!("{} — {} ({} keys)", name, status, count)
                    };
                    ui.label(label);
                }
                ui.add_space(8.0);
                if ui.button("Refresh").clicked() {
                    self.kb_status_dirty = true;
                }
            });

        egui::SidePanel::right("skills_panel")
            .resizable(true)
            .default_width(180.0)
            .show(ctx, |ui| {
                ui.heading("SkillRegistry (10 skills)");
                ui.separator();
                for (i, name) in self.stack.skill_names.iter().enumerate() {
                    ui.label(format!("{}. {}", i + 1, name));
                }
            });

        // Sync control bar from orchestrator (same source as ControlPanelMessage) so toggles match Control Panel add-on
        for i in 0..8 {
            self.control_kb[i] = self.stack.orchestrator.pagi_kb_active((i + 1) as u8);
        }
        self.control_skills_enabled = self.stack.orchestrator.pagi_skills_enabled();

        egui::TopBottomPanel::bottom("control_panel_bar")
            .min_height(0.0)
            .show(ctx, |ui| {
                ui.collapsing("Control Panel (same ControlPanelMessage state as add-on)", |ui| {
                    ui.horizontal_wrapped(|ui| {
                        for i in 0..8 {
                            if ui
                                .checkbox(&mut self.control_kb[i], format!("KB-{}", i + 1))
                                .changed()
                            {
                                self.send_control(ControlPanelMessage::KbState {
                                    index: i,
                                    active: self.control_kb[i],
                                });
                            }
                        }
                        ui.separator();
                        let label = if self.control_skills_enabled {
                            "Skills ON"
                        } else {
                            "Skills OFF"
                        };
                        if ui.checkbox(&mut self.control_skills_enabled, label).changed() {
                            self.send_control(ControlPanelMessage::SkillsEnabled(
                                self.control_skills_enabled,
                            ));
                        }
                    });
                });
            });

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.label("Prompt (maps to short-term memory):");
            });
            let prompt_id = egui::Id::new("studio_prompt");
            ui.add_sized(
                [ui.available_width(), 120.0],
                egui::TextEdit::multiline(&mut self.prompt).id(prompt_id),
            );
            ui.add_space(4.0);

            if ui.button("Send (Dispatch)").clicked() {
                save_to_memory(&self.stack, &self.ctx, MEMORY_PROMPT_PATH, &self.prompt);

                let slot_id = if (1..=8).contains(&self.config.default_slot_id) {
                    self.config.default_slot_id
                } else {
                    1
                };
                let query = self.prompt.trim();
                let query = if query.is_empty() {
                    "brand_voice".to_string()
                } else {
                    query.to_string()
                };
                let goal = Goal::QueryKnowledge { slot_id, query };

                let orch = Arc::clone(&self.stack.orchestrator);
                let tenant_ctx = self.ctx.clone();
                let result = tokio::runtime::Runtime::new()
                    .unwrap()
                    .block_on(orch.dispatch(&tenant_ctx, goal));
                self.response = match result {
                    Ok(v) => serde_json::to_string_pretty(&v).unwrap_or_else(|_| v.to_string()),
                    Err(e) => format!("Error: {}", e),
                };
                save_to_memory(&self.stack, &self.ctx, MEMORY_RESPONSE_PATH, &self.response);
                self.kb_status_dirty = true;
            }

            ui.add_space(8.0);
            ui.separator();
            ui.horizontal_wrapped(|ui| {
                ui.label("Response (from Orchestrator::dispatch):");
            });
            egui::ScrollArea::vertical().show(ui, |ui| {
                ui.label(egui::RichText::new(self.response.as_str()).small());
            });

            ui.add_space(12.0);
            ui.separator();
            ui.collapsing("PagiSkillTester — System Skill Console", |ui| {
                ui.label("Bypass Orchestrator: run any registered skill with raw JSON input.");
                ui.add_space(4.0);

                let skill_names = &self.stack.skill_names;
                if !skill_names.is_empty() {
                    let mut selected = self.pagi_skill_tester_selected.min(skill_names.len().saturating_sub(1));
                    egui::ComboBox::from_id_salt(egui::Id::new("pagi_skill_combo"))
                        .selected_text(skill_names[selected].as_str())
                        .show_ui(ui, |ui| {
                            for (i, name) in skill_names.iter().enumerate() {
                                ui.selectable_value(&mut selected, i, name.as_str());
                            }
                        });
                    self.pagi_skill_tester_selected = selected;
                }

                ui.label("Raw JSON Input (e.g. {\"url\": \"https://...\", \"html\": \"<html>...\"} for CommunityScraper):");
                ui.add_sized(
                    [ui.available_width(), 80.0],
                    egui::TextEdit::multiline(&mut self.pagi_skill_tester_input)
                        .font(egui::TextStyle::Monospace)
                        .hint_text("{} or leave empty for null"),
                );

                let fire_enabled = !self.pagi_skill_tester_pending;
                if ui.add_enabled(fire_enabled, egui::Button::new("Execute Skill (Fire)")).clicked() {
                    self.pagi_fire_skill();
                }
                if self.pagi_skill_tester_pending {
                    ui.spinner();
                    ui.label("Running…");
                }

                if let Some(ref r) = self.pagi_skill_tester_last {
                    ui.add_space(6.0);
                    ui.separator();
                    ui.label("Output Inspector:");
                    match r {
                        Ok((out, ms)) => {
                            ui.label(egui::RichText::new(format!("Success — {} ms", ms)).color(egui::Color32::DARK_GREEN));
                            egui::ScrollArea::vertical().max_height(200.0).show(ui, |ui| {
                                ui.label(egui::RichText::new(out.as_str()).small().monospace());
                            });
                        }
                        Err((err, ms)) => {
                            ui.label(egui::RichText::new(format!("Error — {} ms", ms)).color(egui::Color32::RED));
                            ui.label(egui::RichText::new(err.as_str()).small().color(egui::Color32::RED));
                        }
                    }
                }
            });
        });
    }
}
