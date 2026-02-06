//! pagi-personal-ui: Egui lightweight dashboard. System tray + minimal resource usage.
//! AGI reasoning: pagi_core::Orchestrator::dispatch().

use eframe::egui;
use pagi_core::Goal;
use pagi_personal_ui::{build_orchestrator, default_tenant};
use std::sync::Arc;

fn main() -> eframe::Result<()> {
    let storage = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
    let storage = storage.join("data");
    let orchestrator = build_orchestrator(&storage).expect("build orchestrator");
    let ctx = default_tenant();

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([320.0, 200.0])
            .with_title("PAGI Personal"),
        ..Default::default()
    };

    eframe::run_native(
        "PAGI Personal",
        options,
        Box::new(move |_cc| Ok(Box::new(PersonalApp::new(orchestrator, ctx)))),
    )
}

struct PersonalApp {
    orchestrator: Arc<pagi_core::Orchestrator>,
    ctx: pagi_core::TenantContext,
    result: String,
}

impl PersonalApp {
    fn new(orchestrator: Arc<pagi_core::Orchestrator>, ctx: pagi_core::TenantContext) -> Self {
        Self {
            orchestrator,
            ctx,
            result: String::new(),
        }
    }
}

impl eframe::App for PersonalApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("PAGI Personal — lightweight dashboard");
            ui.add_space(8.0);
            if ui.button("Dispatch (QueryKnowledge)").clicked() {
                let goal = Goal::QueryKnowledge {
                    slot_id: 1,
                    query: "brand_voice".to_string(),
                };
                let orch = Arc::clone(&self.orchestrator);
                let tenant_ctx = self.ctx.clone();
                let result = tokio::runtime::Runtime::new()
                    .unwrap()
                    .block_on(orch.dispatch(&tenant_ctx, goal));
                self.result = match result {
                    Ok(v) => serde_json::to_string_pretty(&v).unwrap_or_else(|_| v.to_string()),
                    Err(e) => format!("Error: {}", e),
                };
            }
            ui.add_space(8.0);
            ui.label("Result:");
            ui.add_space(4.0);
            egui::ScrollArea::vertical().show(ui, |ui| {
                ui.label(egui::RichText::new(self.result.as_str()).small());
            });
        });
    }
}
