//! PAGI Studio UI — HTTP server that serves the Google Studio style web UI and wires it to the orchestrator.
//! Run: cargo run -p pagi-studio-ui --bin pagi-studio-ui-server
//! Then open http://127.0.0.1:3001 (or the printed URL). Frontend/UI port range: 3001-3099. Send triggers Orchestrator::dispatch; Control API sends ControlPanelMessage.

use axum::{
    extract::State,
    routing::{get, post},
    Json, Router,
};
use pagi_core::{ControlPanelMessage, Goal, TenantContext};
use pagi_studio_ui::build_studio_stack;
use std::path::PathBuf;
use std::sync::Arc;
use tower_http::services::{ServeDir, ServeFile};

const PORT: u16 = 3001;

#[derive(Clone)]
struct AppState {
    stack: Arc<pagi_studio_ui::StudioStack>,
    ctx: TenantContext,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let storage = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let storage = storage.join("data");
    let (stack, ctx) = match build_studio_stack(&storage) {
        Ok(ok) => ok,
        Err(e) => {
            let msg = e.to_string();
            if msg.contains("lock") || msg.contains("locked") || msg.contains("access the file") {
                eprintln!("PAGI Studio UI server: cannot open database (already in use).");
                eprintln!("  Storage path: {}", storage.display());
                eprintln!();
                eprintln!("  If pagi-gateway is running, it holds the lock on data/pagi_vault and data/pagi_knowledge.");
                eprintln!("  Either:");
                eprintln!("    1. Stop the gateway, then run this server again (standalone mode), or");
                eprintln!("    2. Use the Vite dev server for the UI instead: npm run dev in add-ons/pagi-studio-ui/assets/studio-interface");
                eprintln!("       and keep the gateway on 8001 for the API.");
                std::process::exit(101);
            }
            return Err(e.into());
        }
    };
    let stack = Arc::new(stack);
    let state = AppState {
        stack: Arc::clone(&stack),
        ctx: ctx.clone(),
    };

    let static_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("assets")
        .join("studio-interface");
    // When run from workspace root, compile-time path may not exist; use cwd-relative path.
    let serve_path = if static_dir.exists() {
        let dist_dir = static_dir.join("dist");
        if dist_dir.exists() { dist_dir } else { static_dir }
    } else {
        let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        let from_cwd = cwd.join("add-ons").join("pagi-studio-ui").join("assets").join("studio-interface");
        let dist_from_cwd = from_cwd.join("dist");
        if dist_from_cwd.exists() {
            dist_from_cwd
        } else if from_cwd.exists() {
            from_cwd
        } else {
            static_dir
        }
    };
    let serve_dir = ServeDir::new(serve_path.clone()).append_index_html_on_directories(true);
    let index_css = serve_path.join("index.css");
    let index_tsx = serve_path.join("index.tsx");
    let app = Router::new()
        .route("/api/v1/execute", post(api_execute))
        .route("/api/v1/chat", post(api_chat))
        .route("/api/v1/control", post(api_control))
        .route("/api/v1/status", get(api_status))
        .route_service("/index.css", ServeFile::new(index_css))
        .route_service("/index.tsx", ServeFile::new(index_tsx))
        .with_state(state)
        .fallback_service(serve_dir);

    let addr = std::net::SocketAddr::from(([127, 0, 0, 1], PORT));
    let url = format!("http://{}", addr);
    println!("PAGI Studio UI server: {}", url);
    println!("Open in browser for Google Studio style interface. API: /api/v1/execute, /api/v1/chat, /api/v1/control");
    if let Ok(()) = webbrowser::open(&url) {}
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;
    Ok(())
}

async fn api_status(State(state): State<AppState>) -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "app": "pagi-studio-ui",
        "orchestrator": "connected",
        "skills": state.stack.skill_names.len()
    }))
}

async fn api_execute(
    State(state): State<AppState>,
    Json(goal): Json<Goal>,
) -> Json<serde_json::Value> {
    let result = state
        .stack
        .orchestrator
        .dispatch(&state.ctx, goal)
        .await;
    match result {
        Ok(v) => Json(v),
        Err(e) => Json(serde_json::json!({
            "status": "error",
            "message": e.to_string()
        })),
    }
}

#[derive(serde::Deserialize)]
struct ChatRequest {
    prompt: String,
}

async fn api_chat(
    State(state): State<AppState>,
    Json(req): Json<ChatRequest>,
) -> Json<serde_json::Value> {
    let query = req.prompt.trim();
    let query = if query.is_empty() {
        "brand_voice".to_string()
    } else {
        query.to_string()
    };
    let goal = Goal::QueryKnowledge {
        slot_id: 1,
        query: query.clone(),
    };
    let result = state.stack.orchestrator.dispatch(&state.ctx, goal).await;
    match result {
        Ok(v) => {
            let response = v
                .get("value")
                .and_then(|x| x.as_str())
                .unwrap_or_else(|| v.get("status").and_then(|x| x.as_str()).unwrap_or(""))
                .to_string();
            Json(serde_json::json!({
                "response": response,
                "thoughts": []
            }))
        }
        Err(e) => Json(serde_json::json!({
            "response": format!("Error: {}", e),
            "thoughts": []
        })),
    }
}

async fn api_control(
    State(state): State<AppState>,
    Json(msg): Json<ControlPanelMessage>,
) -> Json<serde_json::Value> {
    let _ = state.stack.control_tx.try_send(msg);
    Json(serde_json::json!({ "status": "ok" }))
}
