//! pagi-companion-ui: High-fidelity Slint GUI.
//! Focus: character-engine traits and avatar rendering. All AGI reasoning via pagi_core::Orchestrator::dispatch().

use pagi_companion_ui::{build_orchestrator, default_tenant};
use pagi_core::Goal;
use slint::ComponentHandle;
use std::sync::Arc;

slint::slint! {
    import { VerticalBox, HorizontalBox, Button, ScrollView } from "std-widgets.slint";

    export component CompanionWindow inherits Window {
        in property <string> result-text <=> result_area.text;
        callback request-reason();

        preferred-width: 420px;
        preferred-height: 360px;

        HorizontalBox {
            // Avatar / character-engine area (placeholder for high-fidelity rendering)
            VerticalBox {
                alignment: start;
                max-width: 120px;
                Rectangle {
                    height: 100px;
                    width: 100px;
                    background: #2d5a27;
                    border-radius: 8px;
                }
                Text {
                    text: "Avatar";
                    color: #a0a0a0;
                }
            }
            VerticalBox {
                alignment: start;
                spacing: 8px;
                Text {
                    text: "Companion UI";
                    font-size: 18px;
                    color: #e0e0e0;
                }
                Button {
                    text: "Reason (QueryKnowledge)";
                    clicked => { request-reason(); }
                }
                ScrollView {
                    result_area := Text {
                        text: "Result will appear here after dispatch.";
                        wrap: word-wrap;
                    }
                }
            }
        }
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let storage = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
    let storage = storage.join("data");
    let orchestrator = build_orchestrator(&storage)?;
    let ctx = default_tenant();

    let ui = CompanionWindow::new()?;
    let ui_handle = ui.as_weak();
    ui.on_request_reason(move || {
        let orch = Arc::clone(&orchestrator);
        let ctx = ctx.clone();
        let goal = Goal::QueryKnowledge {
            slot_id: 1,
            query: "brand_voice".to_string(),
        };
        let handle = ui_handle.clone();
        std::thread::spawn(move || {
            let rt = tokio::runtime::Runtime::new().unwrap();
            let result = rt.block_on(orch.dispatch(&ctx, goal));
            let text = match result {
                Ok(v) => serde_json::to_string_pretty(&v).unwrap_or_else(|_| v.to_string()),
                Err(e) => format!("Error: {}", e),
            };
            let _ = slint::invoke_from_event_loop(move || {
                if let Some(u) = handle.upgrade() {
                    u.set_result_text(text.as_str().into());
                }
            });
        });
    });

    ui.run()?;
    Ok(())
}
