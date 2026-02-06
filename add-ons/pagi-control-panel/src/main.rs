//! PAGI Control Panel — standalone egui window for KB/skills and memory weights.
//!
//! Run with: cargo run -p pagi-control-panel
//! Optionally wire the bridge receiver to your orchestrator for live updates.

use eframe::egui;
use pagi_control_panel::{pagi_control_panel_channel, PagiControlPanel};

fn main() -> eframe::Result<()> {
    let (tx, mut rx) = pagi_control_panel_channel(64);

    // Spawn a task to consume bridge messages (in a real setup this would feed the orchestrator).
    std::thread::spawn(move || {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("tokio runtime");
        rt.block_on(async move {
            while let Some(msg) = rx.recv().await {
                // Forward to orchestrator in integration; here we drain and log.
                eprintln!("[pagi_control_panel] bridge: {:?}", msg);
            }
        });
    });

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([420.0, 520.0])
            .with_title("PAGI Control Panel — Master Orchestrator"),
        ..Default::default()
    };

    eframe::run_native(
        "PAGI Control Panel",
        options,
        Box::new(move |_cc| {
            let panel = PagiControlPanel::new().with_bridge(tx.clone());
            Ok(Box::new(PagiControlPanelApp::new(panel)))
        }),
    )
}

struct PagiControlPanelApp {
    panel: PagiControlPanel,
}

impl PagiControlPanelApp {
    fn new(panel: PagiControlPanel) -> Self {
        Self { panel }
    }
}

impl eframe::App for PagiControlPanelApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            self.panel.pagi_ui(ui);
        });
    }
}
