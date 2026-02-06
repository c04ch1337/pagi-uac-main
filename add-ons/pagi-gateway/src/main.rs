//! Axum-based API Gateway: entry point for UAC. Config-driven via CoreConfig.

use axum::{
    body::Body,
    extract::{Path, State},
    extract::Json,
    response::{sse::{Event, Sse}, IntoResponse, Response},
    routing::{get, post},
    Router,
};
use axum::http::{Method, StatusCode};
use futures_util::stream::StreamExt;
use std::time::Duration;
use tokio::sync::broadcast;
use tower_http::cors::{AllowOrigin, CorsLayer};
use tracing::field::Visit;
use tracing_subscriber::layer::Context;
use pagi_core::{
    initialize_core_identity, initialize_core_skills, initialize_ethos_policy, AlignmentResult, BlueprintRegistry, CoreConfig, EventRecord, Goal, KbRecord, KbType,
    KnowledgeStore, MemoryManager, Orchestrator, SkillRegistry, TenantContext,
};
use pagi_skills::{
    AnalyzeSentiment, CheckAlignment, CommunityPulse, CommunityScraper, DraftResponse, GetAgentMessages, KnowledgeInsert, KnowledgePruner,
    KnowledgeQuery, LeadCapture, MessageAgent, ModelRouter, RecallPastActions, ResearchAudit, SalesCloser,
    FsWorkspaceAnalyzer, ResearchEmbedInsert, ResearchSemanticSearch, WriteSandboxFile,
};
use std::path::Path as StdPath;
use std::sync::Arc;
use tower_http::services::{ServeDir, ServeFile};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
use std::collections::HashSet;

/// Captures the "message" field from a tracing event.
struct MessageCollector<'a>(&'a mut String);

impl Visit for MessageCollector<'_> {
    fn record_str(&mut self, field: &tracing::field::Field, value: &str) {
        if field.name() == "message" {
            *self.0 = value.to_string();
        }
    }
    fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn std::fmt::Debug) {
        if field.name() == "message" {
            *self.0 = format!("{:?}", value);
        }
    }
}

/// Sends each tracing event as a line to a broadcast channel for SSE log streaming.
#[derive(Clone)]
struct LogBroadcastLayer {
    tx: broadcast::Sender<String>,
}

impl LogBroadcastLayer {
    fn new(tx: broadcast::Sender<String>) -> Self {
        Self { tx }
    }
}

impl<S> tracing_subscriber::Layer<S> for LogBroadcastLayer
where
    S: tracing::Subscriber,
{
    fn on_event(&self, event: &tracing::Event<'_>, _ctx: Context<'_, S>) {
        let mut message = String::new();
        event.record(&mut MessageCollector(&mut message));
        let line = format!(
            "{} [{}] {}",
            event.metadata().level(),
            event.metadata().target(),
            message
        );
        let _ = self.tx.send(line);
    }
}

/// Pre-flight check: verify all 8 KBs are accessible and port is available.
fn run_verify() -> Result<(), String> {
    let config = CoreConfig::load().map_err(|e| format!("Config load failed: {}", e))?;
    let storage = StdPath::new(&config.storage_path);
    let vault_path = storage.join("pagi_vault");
    let kb_path = storage.join("pagi_knowledge");

    // 1. Check MemoryManager (pagi_vault Sled)
    print!("Checking pagi_vault... ");
    let vault = MemoryManager::open_path(&vault_path).map_err(|e| format!("pagi_vault LOCKED or inaccessible: {}", e))?;
    drop(vault);
    println!("OK");

    // 2. Check KnowledgeStore (pagi_knowledge Sled with 8 trees)
    print!("Checking pagi_knowledge (8 KBs)... ");
    let kb = KnowledgeStore::open_path(&kb_path).map_err(|e| format!("pagi_knowledge LOCKED or inaccessible: {}", e))?;
    for slot in 1..=8 {
        kb.get(slot, "__verify_probe__").map_err(|e| format!("KB slot {} failed: {}", slot, e))?;
    }
    drop(kb);
    println!("OK (all 8 slots accessible)");

    // 3. Check port availability
    let port = config.port;
    print!("Checking port {}... ", port);
    let addr = std::net::SocketAddr::from(([127, 0, 0, 1], port));
    match std::net::TcpListener::bind(addr) {
        Ok(listener) => {
            drop(listener);
            println!("OK (available)");
        }
        Err(e) => {
            return Err(format!("Port {} BLOCKED: {}", port, e));
        }
    }

    println!("\n✅ SUCCESS: All systems GO. Ready to start gateway.");
    Ok(())
}

#[tokio::main]
async fn main() {
    // Load .env file if present (before any env::var calls)
    if let Err(e) = dotenvy::dotenv() {
        eprintln!("[pagi-gateway] .env not loaded: {} (using system environment)", e);
    }

    // Handle --verify flag for pre-flight check
    let args: Vec<String> = std::env::args().collect();
    if args.iter().any(|a| a == "--verify") {
        match run_verify() {
            Ok(()) => std::process::exit(0),
            Err(e) => {
                eprintln!("❌ PRE-FLIGHT FAILED: {}", e);
                std::process::exit(1);
            }
        }
    }

    let (log_tx, _) = broadcast::channel(1000);
    let log_layer = LogBroadcastLayer::new(log_tx.clone());

    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::new(
            std::env::var("RUST_LOG").unwrap_or_else(|_| "info".into()),
        ))
        .with(tracing_subscriber::fmt::layer())
        .with(log_layer)
        .init();

    let config = Arc::new(CoreConfig::load().expect("load CoreConfig"));
    let storage = StdPath::new(&config.storage_path);
    let memory_path = storage.join("pagi_vault");
    let knowledge_path = storage.join("pagi_knowledge");

    let memory = Arc::new(
        MemoryManager::open_path(&memory_path).expect("open pagi_vault"),
    );
    let knowledge = Arc::new(
        KnowledgeStore::open_path(&knowledge_path).expect("open pagi_knowledge"),
    );
    knowledge.pagi_init_kb_metadata().ok(); // ensure 8 trees have metadata
    
    // Bootstrap core identity if KB-1 is empty (Mission Genesis)
    match initialize_core_identity(&knowledge) {
        Ok(true) => tracing::info!("Mission Genesis: Core identity bootstrapped successfully"),
        Ok(false) => tracing::debug!("Core identity already exists in KB-1"),
        Err(e) => tracing::warn!("Failed to bootstrap core identity: {}", e),
    }

    // Bootstrap Skill Registry (KB-5) with baseline skill manifests
    match initialize_core_skills(&knowledge) {
        Ok(true) => tracing::info!("Skill Registry: Core skills bootstrapped successfully (KB-5/Techne)"),
        Ok(false) => tracing::debug!("Skill Registry already contains baseline skills (KB-5/Techne)"),
        Err(e) => tracing::warn!("Failed to bootstrap Skill Registry (KB-5/Techne): {}", e),
    }

    match initialize_ethos_policy(&knowledge) {
        Ok(true) => tracing::info!("Ethos: Default safety policy installed (KB_ETHOS)"),
        Ok(false) => tracing::debug!("Ethos: Default policy already present (KB_ETHOS)"),
        Err(e) => tracing::warn!("Failed to bootstrap Ethos policy: {}", e),
    }

    // Cognitive Architecture boot: Pneuma (Vision) active; Oikos (Context) crate count
    let _pneuma_ok = pagi_core::verify_identity(&knowledge).complete;
    let oikos_slot = pagi_core::KbType::Oikos.slot_id();
    let crate_count: usize = knowledge
        .get_record(oikos_slot, "workspace_scan/latest")
        .ok()
        .flatten()
        .and_then(|rec| {
            serde_json::from_str::<serde_json::Value>(&rec.content).ok()
                .and_then(|v| v.get("crate_count").and_then(|c| c.as_u64()).map(|n| n as usize))
        })
        .unwrap_or(0);
    if crate_count == 0 {
        // Run an initial workspace scan so Oikos has context
        let cwd = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
        let scan = pagi_skills::analyze_workspace(&cwd);
        let n = scan.get("crate_count").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
        let content = serde_json::to_string(&scan).unwrap_or_else(|_| "{}".to_string());
        let record = pagi_core::KbRecord::with_metadata(
            content,
            serde_json::json!({ "type": "workspace_scan", "skill": "fs_workspace_analyzer", "crate_count": n, "tags": ["oikos", "workspace"] }),
        );
        let _ = knowledge.insert_record(oikos_slot, "workspace_scan/latest", &record);
        tracing::info!(
            "[Cognitive Architecture] Pneuma (Vision) active. Oikos (Context) mapped to {} crates.",
            n
        );
    } else {
        tracing::info!(
            "[Cognitive Architecture] Pneuma (Vision) active. Oikos (Context) mapped to {} crates.",
            crate_count
        );
    }

    let mut registry = SkillRegistry::new();
    registry.register(Arc::new(LeadCapture::new(Arc::clone(&memory))));
    registry.register(Arc::new(KnowledgeQuery::new(Arc::clone(&knowledge))));
    registry.register(Arc::new(KnowledgeInsert::new(Arc::clone(&knowledge))));
    registry.register(Arc::new(CommunityPulse::new(Arc::clone(&knowledge))));
    registry.register(Arc::new(DraftResponse::new(
        Arc::clone(&memory),
        Arc::clone(&knowledge),
    )));
    registry.register(Arc::new(ModelRouter::with_knowledge(Arc::clone(&knowledge))));
    registry.register(Arc::new(FsWorkspaceAnalyzer::new_with_store(Arc::clone(&knowledge))));
    registry.register(Arc::new(WriteSandboxFile::new()));
    registry.register(Arc::new(ResearchEmbedInsert::new(Arc::clone(&knowledge))));
    registry.register(Arc::new(ResearchSemanticSearch::new(Arc::clone(&knowledge))));
    registry.register(Arc::new(ResearchAudit::new(Arc::clone(&knowledge))));
    registry.register(Arc::new(RecallPastActions::new(Arc::clone(&knowledge))));
    registry.register(Arc::new(CheckAlignment::new(Arc::clone(&knowledge))));
    registry.register(Arc::new(AnalyzeSentiment::new(Arc::clone(&knowledge))));
    registry.register(Arc::new(CommunityScraper::new(Arc::clone(&knowledge))));
    registry.register(Arc::new(SalesCloser::new(Arc::clone(&knowledge))));
    registry.register(Arc::new(KnowledgePruner::new(Arc::clone(&knowledge))));
    registry.register(Arc::new(MessageAgent::new(Arc::clone(&knowledge))));
    registry.register(Arc::new(GetAgentMessages::new(Arc::clone(&knowledge))));
    let blueprint_path = std::env::var("PAGI_BLUEPRINT_PATH")
        .unwrap_or_else(|_| "config/blueprint.json".to_string());
    let blueprint = Arc::new(BlueprintRegistry::load_json_path(&blueprint_path));
    let orchestrator = Arc::new(Orchestrator::with_blueprint(
        Arc::new(registry),
        Arc::clone(&blueprint),
    ));

    // Create a dedicated ModelRouter instance for streaming support
    let model_router = Arc::new(ModelRouter::with_knowledge(Arc::clone(&knowledge)));

    // Heartbeat (Autonomous Orchestrator): in-process background task so we can share
    // the same Sled-backed KnowledgeStore without cross-process lock contention.
    // Tick rate is configurable via env `PAGI_TICK_RATE_SECS`.
    let tick_rate = std::env::var("PAGI_TICK_RATE_SECS")
        .ok()
        .and_then(|s| s.parse::<u64>().ok())
        .unwrap_or(5)
        .max(1);
    tokio::spawn(heartbeat_loop(
        Arc::clone(&knowledge),
        Arc::clone(&model_router),
        std::time::Duration::from_secs(tick_rate),
    ));
    
    let app = build_app(AppState {
        config: Arc::clone(&config),
        orchestrator,
        knowledge,
        log_tx,
        model_router,
    });

    let port = config.port;
    let app_name = config.app_name.clone();
    let addr = std::net::SocketAddr::from(([127, 0, 0, 1], port));
    tracing::info!("{} listening on {}", app_name, addr);
    axum::serve(
        tokio::net::TcpListener::bind(addr).await.unwrap(),
        app,
    )
    .await
    .unwrap();
}

async fn heartbeat_loop(
    knowledge: Arc<KnowledgeStore>,
    model_router: Arc<ModelRouter>,
    tick: std::time::Duration,
) {
    tracing::info!(
        target: "pagi::daemon",
        tick_rate_secs = tick.as_secs(),
        "Heartbeat loop started"
    );
    let mut interval = tokio::time::interval(tick);
    loop {
        interval.tick().await;
        if let Err(e) = heartbeat_tick(Arc::clone(&knowledge), Arc::clone(&model_router)).await {
            tracing::warn!(target: "pagi::daemon", error = %e, "Heartbeat tick failed");
        }
    }
}

async fn heartbeat_tick(
    knowledge: Arc<KnowledgeStore>,
    model_router: Arc<ModelRouter>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // Discover active agents by scanning KB_SOMA inbox keys: inbox/{agent_id}/...
    let soma_slot = KbType::Soma.slot_id();
    let keys = knowledge.scan_keys(soma_slot)?;
    let mut agents: HashSet<String> = HashSet::new();
    for k in keys {
        if let Some(rest) = k.strip_prefix("inbox/") {
            if let Some((agent_id, _tail)) = rest.split_once('/') {
                if !agent_id.trim().is_empty() {
                    agents.insert(agent_id.to_string());
                }
            }
        }
    }

    for agent_id in agents {
        // AUTO-POLL: check inbox.
        // We fetch a small batch so we can skip already-processed messages without getting stuck.
        let inbox = knowledge.get_agent_messages_with_keys(&agent_id, 25)?;
        if let Some((inbox_key, msg)) = inbox
            .into_iter()
            .find(|(_k, m)| !m.is_processed)
        {
            // Stop infinite ping-pong: never auto-reply to an auto-reply.
            // Still ACK it so it doesn't remain "unprocessed" forever.
            let msg_type = msg
                .payload
                .as_object()
                .and_then(|o| o.get("type"))
                .and_then(|v| v.as_str())
                .unwrap_or("");
            if msg_type == "agent_auto_reply" {
                let mut updated = msg.clone();
                updated.is_processed = true;
                knowledge.insert(soma_slot, &inbox_key, &updated.to_bytes())?;
                continue;
            }

            // Trigger response generation for the agent.
            let prompt = format!(
                "You are agent_id={}. You have a new inbox message from {}. Message payload: {}\n\nRespond appropriately.",
                agent_id,
                msg.from_agent_id,
                msg.payload
            );

            let generated = model_router
                .generate_text_raw(&prompt)
                .await
                .unwrap_or_else(|e| format!("[heartbeat] generation failed: {}", e));

            // Deliver response back to sender as an inter-agent message.
            knowledge.push_agent_message(
                &agent_id,
                &msg.from_agent_id,
                &serde_json::json!({
                    "type": "agent_auto_reply",
                    "in_reply_to": msg.id,
                    "text": generated,
                }),
            )?;

            // ACK: mark the original inbox message as processed (preserve KB_SOMA history).
            let mut updated = msg.clone();
            updated.is_processed = true;
            knowledge.insert(soma_slot, &inbox_key, &updated.to_bytes())?;

            // Reflection: write a Chronos event for the agent.
            let reflection = EventRecord::now(
                "Chronos",
                format!("Auto-replied to message {} from {}", msg.id, msg.from_agent_id),
            )
            .with_skill("heartbeat")
            .with_outcome("auto_reply_sent");
            let _ = knowledge.append_chronos_event(&agent_id, &reflection);
        } else {
            // If no inbox message exists, check Pneuma for background tasks.
            // Minimal v1: if a key `pneuma/{agent_id}/background_task` exists, run it through the router.
            let pneuma_slot = KbType::Pneuma.slot_id();
            let bg_key = format!("pneuma/{}/background_task", agent_id);
            if let Ok(Some(bytes)) = knowledge.get(pneuma_slot, &bg_key) {
                if let Ok(task) = String::from_utf8(bytes) {
                    if !task.trim().is_empty() {
                        let prompt = format!(
                            "You are agent_id={}. Background task: {}\n\nProvide a short status update.",
                            agent_id,
                            task
                        );
                        let generated = model_router
                            .generate_text_raw(&prompt)
                            .await
                            .unwrap_or_else(|e| format!("[heartbeat] background generation failed: {}", e));
                        let reflection = EventRecord::now(
                            "Chronos",
                            format!("Background task ticked: {}", generated),
                        )
                        .with_skill("heartbeat")
                        .with_outcome("background_task_ticked");
                        let _ = knowledge.append_chronos_event(&agent_id, &reflection);
                    }
                }
            }
        }
    }

    Ok(())
}

fn frontend_root_dir() -> std::path::PathBuf {
    // Prefer a working-directory relative path for local development (run from workspace root).
    // Fall back to workspace-root-relative path from add-ons/pagi-gateway: manifest -> .. -> .. -> pagi-frontend.
    let cwd = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
    let from_cwd = cwd.join("pagi-frontend");
    if from_cwd.exists() {
        return from_cwd;
    }

    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("pagi-frontend")
}

fn build_app(state: AppState) -> Router {
    let frontend_enabled = state.config.frontend_enabled;

    // CORS: allow Backend/API (8001-8099) and Frontend/UI (3001-3099) port ranges.
    // NOTE: SSE streaming often triggers additional browser-managed headers
    // (e.g., Accept, Cache-Control, Pragma). If we only allow CONTENT_TYPE,
    // fetch() may fail before the request reaches the handler.
    let cors = CorsLayer::new()
        .allow_origin(AllowOrigin::predicate(|origin: &axum::http::HeaderValue, _| {
            let s = origin.to_str().unwrap_or("");
            let port = s
                .split(':')
                .last()
                .and_then(|p| p.parse::<u16>().ok())
                .unwrap_or(0);
            (3001..=3099).contains(&port) || (8001..=8099).contains(&port)
        }))
        .allow_methods([Method::GET, Method::POST, Method::OPTIONS, Method::PUT, Method::DELETE])
        .allow_headers(tower_http::cors::Any)
        .expose_headers(tower_http::cors::Any);

    let mut app = Router::new()
        .route("/v1/status", get(status))
        .route("/v1/execute", post(execute))
        .route("/v1/research/trace/:trace_id", get(get_research_trace))
        .route("/api/v1/health", get(health))
        .route("/api/v1/logs", get(logs_stream))
        .route("/api/v1/chat", post(chat))
        .route("/api/v1/kardia/:user_id", get(get_kardia_relation))
        .route("/api/v1/kb-status", get(kb_status))
        .with_state(state);

    if frontend_enabled {
        let frontend_dir = frontend_root_dir();
        let index_file = frontend_dir.join("index.html");
        let assets_dir = frontend_dir.join("assets");

        // Map `/` -> `pagi-frontend/index.html`
        app = app.route_service("/", ServeFile::new(index_file));

        // Map `/assets/*` -> `pagi-frontend/assets/*` (CSS, images, etc.)
        if assets_dir.exists() {
            app = app.nest_service("/assets", ServeDir::new(assets_dir));
        }

        // Map `/ui/*` -> `pagi-frontend/*` (app.js, assets, and any other files)
        app = app.nest_service("/ui", ServeDir::new(frontend_dir));
    }

    app.layer(cors)
}

#[derive(Clone)]
pub(crate) struct AppState {
    pub(crate) config: Arc<CoreConfig>,
    pub(crate) orchestrator: Arc<Orchestrator>,
    pub(crate) knowledge: Arc<KnowledgeStore>,
    pub(crate) log_tx: broadcast::Sender<String>,
    pub(crate) model_router: Arc<ModelRouter>,
}

/// GET /api/v1/health – liveness check for UI and scripts.
async fn health() -> axum::Json<serde_json::Value> {
    axum::Json(serde_json::json!({ "status": "ok" }))
}

/// GET /api/v1/kb-status – returns status of all 8 Knowledge Bases (L2 Memory).
async fn kb_status(State(state): State<AppState>) -> axum::Json<serde_json::Value> {
    let kb_statuses = state.knowledge.get_all_status();
    let all_connected = kb_statuses.iter().all(|s| s.connected);
    let total_entries: usize = kb_statuses.iter().map(|s| s.entry_count).sum();
    
    axum::Json(serde_json::json!({
        "status": if all_connected { "ok" } else { "degraded" },
        "all_connected": all_connected,
        "total_entries": total_entries,
        "knowledge_bases": kb_statuses
    }))
}

/// GET /api/v1/logs – Server-Sent Events stream of gateway logs (tracing output).
async fn logs_stream(
    State(state): State<AppState>,
) -> Sse<impl futures_util::Stream<Item = Result<Event, std::convert::Infallible>> + Send + 'static> {
    use async_stream::stream;
    let mut rx = state.log_tx.subscribe();
    let stream = stream! {
        loop {
            tokio::select! {
                r = rx.recv() => match r {
                    Ok(line) => yield Ok(Event::default().data(line)),
                    Err(broadcast::error::RecvError::Lagged(n)) => {
                        yield Ok(Event::default().data(format!("... {} log lines dropped", n)));
                    }
                    Err(broadcast::error::RecvError::Closed) => break,
                },
                _ = tokio::time::sleep(Duration::from_secs(15)) => {
                    yield Ok(Event::default().comment("keepalive"));
                }
            }
        }
    };
    Sse::new(stream).keep_alive(
        axum::response::sse::KeepAlive::new()
            .interval(Duration::from_secs(15))
            .text("keepalive"),
    )
}

/// GET /v1/status – app identity and slot labels from config.
async fn status(State(state): State<AppState>) -> axum::Json<serde_json::Value> {
    let labels: std::collections::HashMap<u8, String> = state.config.slot_labels_map();
    let labels_json: std::collections::HashMap<String, String> = labels
        .into_iter()
        .map(|(k, v)| (k.to_string(), v))
        .collect();
    axum::Json(serde_json::json!({
        "app_name": state.config.app_name,
        "port": state.config.port,
        "llm_mode": state.config.llm_mode,
        "slot_labels": labels_json,
    }))
}

#[derive(serde::Deserialize)]
struct ExecuteRequest {
    tenant_id: String,
    correlation_id: Option<String>,
    /// Agent instance ID for multi-agent mode. Chronos and Kardia are keyed by this. Default: "default".
    #[serde(default)]
    agent_id: Option<String>,
    goal: Goal,
}

/// Chat request from the Studio UI frontend
#[derive(serde::Deserialize)]
struct ChatRequest {
    prompt: String,
    #[serde(default)]
    stream: bool,
    #[serde(default)]
    user_alias: Option<String>,
    /// Agent instance ID for multi-agent mode (Kardia owner). Default: "default".
    #[serde(default)]
    agent_id: Option<String>,
    #[serde(default)]
    model: Option<String>,
    #[serde(default)]
    temperature: Option<f32>,
    #[serde(default)]
    max_tokens: Option<u32>,
    #[serde(default)]
    persona: Option<String>,
}

async fn execute(
    State(state): State<AppState>,
    Json(req): Json<ExecuteRequest>,
) -> axum::Json<serde_json::Value> {
    tracing::info!("Skill execution started");
    let agent_id = req.agent_id.as_deref().filter(|s| !s.is_empty()).unwrap_or(pagi_core::DEFAULT_AGENT_ID);
    let is_kb_query = matches!(req.goal, Goal::QueryKnowledge { .. });
    let ctx = TenantContext {
        tenant_id: req.tenant_id,
        correlation_id: req.correlation_id,
        agent_id: Some(agent_id.to_string()),
    };

    // ETHOS pre-execution check: consult KB_ETHOS before ExecuteSkill
    if let Goal::ExecuteSkill { ref name, ref payload } = req.goal {
        let content_to_scan = payload
            .as_ref()
            .map(|p| {
                p.get("content")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string()
            })
            .unwrap_or_else(|| payload.as_ref().map(|p| p.to_string()).unwrap_or_default());
        if let Some(policy) = state.knowledge.get_ethos_policy() {
            match policy.allows(name, &content_to_scan) {
                AlignmentResult::Fail { reason } => {
                    let violation = EventRecord::now("Ethos", format!("Policy Violation: {}", reason))
                        .with_skill(name.clone())
                        .with_outcome("blocked");
                    let _ = state.knowledge.append_chronos_event(agent_id, &violation);
                    tracing::warn!(
                        target: "pagi::ethos",
                        skill = %name,
                        reason = %reason,
                        "Ethos: execution blocked"
                    );
                    return axum::Json(serde_json::json!({
                        "status": "policy_violation",
                        "error": reason,
                        "skill": name,
                    }));
                }
                AlignmentResult::Pass => {}
            }
        }
    }

    match state.orchestrator.dispatch(&ctx, req.goal.clone()).await {
        Ok(result) => {
            if is_kb_query {
                tracing::info!("KB search success");
            }
            // Episodic memory: log successful execution to KB_CHRONOS (the Historian)
            if let Some(event) = chronos_event_from_goal_and_result(&req.goal, &result) {
                if state.knowledge.append_chronos_event(agent_id, &event).is_err() {
                    tracing::warn!(target: "pagi::chronos", "Failed to append Chronos event");
                }
            }
            axum::Json(result)
        }
        Err(e) => axum::Json(serde_json::json!({
            "error": e.to_string(),
            "status": "error"
        })),
    }
}

/// Builds an episodic EventRecord for KB_CHRONOS from the executed goal and its result.
fn chronos_event_from_goal_and_result(goal: &Goal, result: &serde_json::Value) -> Option<EventRecord> {
    let (source_kb, reflection, skill_name, outcome) = match goal {
        Goal::ExecuteSkill { name, .. } => {
            let outcome = result
                .get("status")
                .and_then(|v| v.as_str())
                .or_else(|| result.get("skill").and_then(|v| v.as_str()))
                .map(|s| s.to_string());
            (
                "Soma",
                format!("Executed skill: {}", name),
                Some(name.clone()),
                outcome,
            )
        }
        Goal::QueryKnowledge { slot_id, query } => (
            "Chronos",
            format!("Queried KB-{} for key: {}", slot_id, query),
            None,
            result.get("value").map(|v| if v.is_null() { "missing" } else { "retrieved" }.to_string()),
        ),
        Goal::UpdateKnowledgeSlot { slot_id, .. } => (
            "Soma",
            format!("Updated knowledge slot {}", slot_id),
            Some("CommunityScraper".to_string()),
            result.get("event").and_then(|v| v.as_str()).map(|s| s.to_string()),
        ),
        Goal::MemoryOp { path, .. } => (
            "Chronos",
            format!("Memory operation on path: {}", path),
            None,
            result.get("status").and_then(|v| v.as_str()).map(|s| s.to_string()),
        ),
        Goal::AutonomousGoal { intent, .. } => (
            "Pneuma",
            format!("Autonomous goal: {}", intent),
            None,
            result.get("status").and_then(|v| v.as_str()).map(|s| s.to_string()),
        ),
        Goal::GenerateFinalResponse { context_id } => (
            "Soma",
            format!("Generated final response for context: {}", context_id),
            Some("ModelRouter".to_string()),
            result.get("generated").and_then(|v| v.as_str()).map(|s| s.chars().take(80).chain(std::iter::once('…')).collect::<String>()),
        ),
        _ => return None,
    };
    let mut event = EventRecord::now(source_kb, reflection);
    if let Some(s) = skill_name {
        event = event.with_skill(s);
    }
    if let Some(o) = outcome {
        event = event.with_outcome(o);
    }
    Some(event)
}

/// Chat endpoint for the Studio UI - routes prompt through ModelRouter skill
/// Supports both streaming (SSE) and non-streaming (JSON) modes.
async fn chat(
    State(state): State<AppState>,
    Json(req): Json<ChatRequest>,
) -> Response {
    tracing::info!("Chat request received: {} chars, stream: {}", req.prompt.len(), req.stream);
    
    if req.stream {
        // Streaming mode - return SSE stream
        chat_streaming(state, req).await
    } else {
        // Non-streaming mode - return JSON
        chat_json(state, req).await.into_response()
    }
}

/// Non-streaming chat handler - returns JSON response
async fn chat_json(
    state: AppState,
    req: ChatRequest,
) -> axum::Json<serde_json::Value> {
    let user_id = req.user_alias.as_deref().unwrap_or("studio-user");
    let agent_id = req.agent_id.as_deref().filter(|s| !s.is_empty()).unwrap_or(pagi_core::DEFAULT_AGENT_ID);
    let ctx = TenantContext {
        tenant_id: user_id.to_string(),
        correlation_id: Some(uuid::Uuid::new_v4().to_string()),
        agent_id: Some(agent_id.to_string()),
    };

    // Adaptive response: inject KB_KARDIA relation context so the LLM can adjust tone
    let kardia_context = state
        .knowledge
        .get_kardia_relation(agent_id, user_id)
        .map(|r| r.prompt_context())
        .unwrap_or_default();
    let prompt_with_context = if kardia_context.is_empty() {
        req.prompt.clone()
    } else {
        format!("{}{}", kardia_context, req.prompt)
    };

    // Use ExecuteSkill goal to run ModelRouter with the prompt
    let goal = Goal::ExecuteSkill {
        name: "ModelRouter".to_string(),
        payload: Some(serde_json::json!({
            "prompt": prompt_with_context,
            "model": req.model,
            "temperature": req.temperature,
            "max_tokens": req.max_tokens,
            "persona": req.persona,
        })),
    };
    
    match state.orchestrator.dispatch(&ctx, goal).await {
        Ok(result) => {
            let generated = result.get("generated")
                .and_then(|v| v.as_str())
                .unwrap_or("No response generated")
                .to_string();
            
            // Save to KB-4 (Memory) for conversation history
            save_to_memory(&state.knowledge, &req.prompt, &generated);
            
            tracing::info!("Chat response generated successfully");
            axum::Json(serde_json::json!({
                "status": "ok",
                "response": generated,
                "thought": format!("Processed prompt ({} chars) via {} mode", 
                    req.prompt.len(),
                    result.get("mode").and_then(|v| v.as_str()).unwrap_or("unknown")
                ),
                "model": req.model.unwrap_or_else(|| "default".to_string()),
                "raw_result": result
            }))
        }
        Err(e) => {
            tracing::error!("Chat error: {}", e);
            axum::Json(serde_json::json!({
                "status": "error",
                "error": e.to_string(),
                "response": format!("Error: {}", e)
            }))
        }
    }
}

/// Streaming chat handler - returns SSE stream of tokens
async fn chat_streaming(
    state: AppState,
    req: ChatRequest,
) -> Response {
    use async_stream::stream;
    
    let user_id = req.user_alias.as_deref().unwrap_or("studio-user");
    let agent_id = req.agent_id.as_deref().filter(|s| !s.is_empty()).unwrap_or(pagi_core::DEFAULT_AGENT_ID);
    let kardia_context = state
        .knowledge
        .get_kardia_relation(agent_id, user_id)
        .map(|r| r.prompt_context())
        .unwrap_or_default();
    let prompt = if kardia_context.is_empty() {
        req.prompt.clone()
    } else {
        format!("{}{}", kardia_context, req.prompt)
    };

    let model = req.model.clone();
    let temperature = req.temperature;
    let max_tokens = req.max_tokens;
    let knowledge = Arc::clone(&state.knowledge);
    
    tracing::info!(
        target: "pagi::chat",
        agent_id = %agent_id,
        "[Chat] Starting streaming session for prompt ({} chars)",
        prompt.len()
    );
    
    // Check if we're in mock mode
    let is_live = std::env::var("PAGI_LLM_MODE").as_deref() == Ok("live");
    
    let stream = stream! {
        let mut accumulated_response = String::new();
        
        if is_live {
            // Live streaming from OpenRouter
            match state.model_router.stream_generate(
                &prompt,
                model.as_deref(),
                temperature,
                max_tokens,
            ).await {
                Ok(mut rx) => {
                    while let Some(chunk) = rx.recv().await {
                        accumulated_response.push_str(&chunk);
                        yield chunk;
                    }
                }
                Err(e) => {
                    tracing::error!(
                        target: "pagi::chat",
                        "[Chat] Stream generation error: {}",
                        e
                    );
                    yield format!("[Error: {}]", e);
                }
            }
        } else {
            // Mock streaming - word by word with delays
            let mut rx = state.model_router.mock_stream_generate(&prompt);
            while let Some(chunk) = rx.recv().await {
                accumulated_response.push_str(&chunk);
                yield chunk;
            }
        }
        
        // Save completed response to KB-4 (Memory) - use original user prompt for history
        let user_prompt = req.prompt.clone();
        if !accumulated_response.is_empty() {
            save_to_memory(&knowledge, &user_prompt, &accumulated_response);
            tracing::info!(
                target: "pagi::chat",
                "[Chat] Streaming complete. Saved {} chars to KB-4 (Memory)",
                accumulated_response.len()
            );
        }
    };
    
    // Convert to a body stream that sends raw text chunks
    let body_stream = stream.map(|chunk| Ok::<_, std::convert::Infallible>(chunk));
    let body = Body::from_stream(body_stream);
    
    Response::builder()
        .status(StatusCode::OK)
        .header("Content-Type", "text/plain; charset=utf-8")
        .header("Cache-Control", "no-cache")
        .header("Connection", "keep-alive")
        .body(body)
        .unwrap()
}

/// Saves a conversation exchange to KB-4 (Memory) for context recall
fn save_to_memory(knowledge: &Arc<KnowledgeStore>, prompt: &str, response: &str) {
    let memory_slot = KbType::Chronos.slot_id();
    let conversation_id = uuid::Uuid::new_v4().to_string();
    
    let record = KbRecord::with_metadata(
        format!("User: {}\n\nAssistant: {}", prompt, response),
        serde_json::json!({
            "type": "conversation",
            "prompt_len": prompt.len(),
            "response_len": response.len(),
            "timestamp": std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_millis() as i64)
                .unwrap_or(0),
        }),
    );
    
    if let Err(e) = knowledge.insert_record(memory_slot, &conversation_id, &record) {
        tracing::warn!(
            target: "pagi::chat",
            "[Chat] Failed to save conversation to KB-4: {}",
            e
        );
    }
}

const KB_SLOT_INTERNAL_RESEARCH: u8 = 8;

/// Query params for GET /api/v1/kardia/:user_id
#[derive(serde::Deserialize)]
struct KardiaQuery {
    #[serde(default)]
    agent_id: Option<String>,
}

/// Returns the current relation/sentiment record for a user from KB_KARDIA (for UI and verification).
async fn get_kardia_relation(
    State(state): State<AppState>,
    Path(user_id): Path<String>,
    axum::extract::Query(q): axum::extract::Query<KardiaQuery>,
) -> Result<axum::Json<serde_json::Value>, axum::http::StatusCode> {
    let owner_agent_id = q.agent_id.as_deref().filter(|s| !s.is_empty()).unwrap_or(pagi_core::DEFAULT_AGENT_ID);
    let record = state
        .knowledge
        .get_kardia_relation(owner_agent_id, &user_id)
        .ok_or(axum::http::StatusCode::NOT_FOUND)?;
    Ok(axum::Json(serde_json::json!({
        "user_id": record.user_id,
        "trust_score": record.trust_score,
        "communication_style": record.communication_style,
        "last_sentiment": record.last_sentiment,
        "last_updated_ms": record.last_updated_ms,
    })))
}

async fn get_research_trace(
    State(state): State<AppState>,
    Path(trace_id): Path<String>,
) -> Result<axum::Json<serde_json::Value>, axum::http::StatusCode> {
    let value = state
        .knowledge
        .get(KB_SLOT_INTERNAL_RESEARCH, &trace_id)
        .map_err(|_| axum::http::StatusCode::INTERNAL_SERVER_ERROR)?
        .and_then(|b| String::from_utf8(b).ok());
    let value = value.ok_or(axum::http::StatusCode::NOT_FOUND)?;
    let trace: serde_json::Value =
        serde_json::from_str(&value).map_err(|_| axum::http::StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(axum::Json(trace))
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use tower::ServiceExt;

    fn test_log_tx() -> broadcast::Sender<String> {
        let (tx, _) = broadcast::channel(1);
        tx
    }

    fn test_model_router() -> Arc<ModelRouter> {
        Arc::new(ModelRouter::new())
    }

    fn test_config() -> CoreConfig {
        CoreConfig {
            app_name: "Test Gateway".to_string(),
            port: 8001,
            storage_path: "./data".to_string(),
            llm_mode: "mock".to_string(),
            frontend_enabled: false,
            slot_labels: std::collections::HashMap::new(),
        }
    }

    #[tokio::test]
    async fn test_status_returns_app_identity_and_slot_labels() {
        let config = CoreConfig {
            app_name: "Test Identity".to_string(),
            port: 4000,
            storage_path: "./data".to_string(),
            llm_mode: "mock".to_string(),
            frontend_enabled: false,
            slot_labels: [
                ("1".to_string(), "Legal Compliance".to_string()),
                ("2".to_string(), "Marketing Tone".to_string()),
            ]
            .into_iter()
            .collect(),
        };
        let knowledge = Arc::new(
            KnowledgeStore::open_path("./data/pagi_knowledge_status_test").unwrap(),
        );
        let mut registry = SkillRegistry::new();
        registry.register(Arc::new(KnowledgeQuery::new(Arc::clone(&knowledge))));
        let orchestrator = Arc::new(Orchestrator::new(Arc::new(registry)));
        let app = Router::new()
            .route("/v1/status", get(status))
            .with_state(AppState {
                config: Arc::new(config),
                orchestrator,
                knowledge,
                log_tx: test_log_tx(),
                model_router: test_model_router(),
            });
        let req = Request::builder()
            .method("GET")
            .uri("/v1/status")
            .body(Body::empty())
            .unwrap();
        let res = app.oneshot(req).await.unwrap();
        assert_eq!(res.status(), StatusCode::OK);
        let bytes = axum::body::to_bytes(res.into_body(), usize::MAX).await.unwrap();
        let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(json["app_name"], "Test Identity");
        assert_eq!(json["port"], 4000);
        assert_eq!(json["llm_mode"], "mock");
        assert_eq!(json["slot_labels"]["1"], "Legal Compliance");
        assert_eq!(json["slot_labels"]["2"], "Marketing Tone");
    }

    #[tokio::test]
    async fn test_execute_lead_capture() {
        let memory = Arc::new(MemoryManager::new().unwrap());
        let knowledge = Arc::new(
            KnowledgeStore::open_path("./data/pagi_knowledge_lead_test").unwrap(),
        );
        let mut registry = SkillRegistry::new();
        registry.register(Arc::new(LeadCapture::new(Arc::clone(&memory))));
        let orchestrator = Arc::new(Orchestrator::new(Arc::new(registry)));
        let app = Router::new()
            .route("/v1/execute", post(execute))
            .with_state(AppState {
                config: Arc::new(test_config()),
                orchestrator,
                knowledge,
                log_tx: test_log_tx(),
                model_router: test_model_router(),
            });

        let body = serde_json::json!({
            "tenant_id": "test-tenant",
            "goal": {
                "IngestData": {
                    "payload": { "email": "lead@example.com", "message": "Customer inquiry" }
                }
            }
        });
        let req = Request::builder()
            .method("POST")
            .uri("/v1/execute")
            .header("content-type", "application/json")
            .body(Body::from(serde_json::to_string(&body).unwrap()))
            .unwrap();
        let res = app.oneshot(req).await.unwrap();
        assert_eq!(res.status(), StatusCode::OK);
        let bytes = axum::body::to_bytes(res.into_body(), usize::MAX)
            .await
            .unwrap();
        let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(json["status"], "saved");
        assert_eq!(json["skill"], "LeadCapture");
        assert!(json.get("lead_id").is_some());
    }

    #[tokio::test]
    async fn test_frontend_index_served_when_enabled() {
        let knowledge = Arc::new(
            KnowledgeStore::open_path("./data/pagi_frontend_index_test").unwrap(),
        );
        let orchestrator = Arc::new(Orchestrator::new(Arc::new(SkillRegistry::new())));

        let config = CoreConfig {
            app_name: "Test UI".to_string(),
            port: 0,
            storage_path: "./data".to_string(),
            llm_mode: "mock".to_string(),
            frontend_enabled: true,
            slot_labels: std::collections::HashMap::new(),
        };

        let app = build_app(AppState {
            config: Arc::new(config),
            orchestrator,
            knowledge: Arc::clone(&knowledge),
            log_tx: test_log_tx(),
            model_router: Arc::new(ModelRouter::with_knowledge(Arc::clone(&knowledge))),
        });

        let req = Request::builder()
            .method("GET")
            .uri("/")
            .body(Body::empty())
            .unwrap();

        let res = app.oneshot(req).await.unwrap();
        assert_eq!(res.status(), StatusCode::OK);

        let bytes = axum::body::to_bytes(res.into_body(), usize::MAX).await.unwrap();
        let body = String::from_utf8_lossy(&bytes);
        assert!(body.contains("PAGI Gateway UI"), "Drop-In UI title should be present");
        assert!(
            body.contains("Drop your AI Studio") || body.contains("pagi-frontend"),
            "Drop-In UI hint should be reachable when enabled; got body len {}",
            body.len()
        );
    }

    #[tokio::test]
    async fn test_kb1_brand_voice_retrieve() {
        let knowledge = Arc::new(
            KnowledgeStore::open_path("./data/pagi_knowledge_test")
                .unwrap(),
        );
        knowledge
            .insert(1, "brand_voice", b"Friendly and professional")
            .unwrap();

        let mut registry = SkillRegistry::new();
        registry.register(Arc::new(KnowledgeQuery::new(Arc::clone(&knowledge))));
        let orchestrator = Arc::new(Orchestrator::new(Arc::new(registry)));
        let app = Router::new()
            .route("/v1/execute", post(execute))
            .with_state(AppState {
            config: Arc::new(test_config()),
            orchestrator,
            knowledge,
            log_tx: test_log_tx(),
            model_router: test_model_router(),
        });

        let body = serde_json::json!({
            "tenant_id": "test-tenant",
            "goal": {
                "QueryKnowledge": { "slot_id": 1, "query": "brand_voice" }
            }
        });
        let req = Request::builder()
            .method("POST")
            .uri("/v1/execute")
            .header("content-type", "application/json")
            .body(Body::from(serde_json::to_string(&body).unwrap()))
            .unwrap();
        let res = app.oneshot(req).await.unwrap();
        assert_eq!(res.status(), StatusCode::OK);
        let bytes = axum::body::to_bytes(res.into_body(), usize::MAX)
            .await
            .unwrap();
        let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(json["status"], "ok");
        assert_eq!(json["skill"], "KnowledgeQuery");
        assert_eq!(json["slot_id"], 1);
        assert_eq!(json["query_key"], "brand_voice");
        assert_eq!(json["value"], "Friendly and professional");
    }

    #[tokio::test]
    async fn test_chronos_episodic_memory_and_recall_past_actions() {
        let knowledge = Arc::new(
            KnowledgeStore::open_path("./data/pagi_chronos_recall_test").unwrap(),
        );
        knowledge.insert(1, "test_key", b"test_value").unwrap();
        let mut registry = SkillRegistry::new();
        registry.register(Arc::new(KnowledgeQuery::new(Arc::clone(&knowledge))));
        registry.register(Arc::new(RecallPastActions::new(Arc::clone(&knowledge))));
        let orchestrator = Arc::new(Orchestrator::new(Arc::new(registry)));
        let app = Router::new()
            .route("/v1/execute", post(execute))
            .with_state(AppState {
                config: Arc::new(test_config()),
                orchestrator,
                knowledge: Arc::clone(&knowledge),
                log_tx: test_log_tx(),
                model_router: test_model_router(),
            });

        let query_body = serde_json::json!({
            "tenant_id": "test-tenant",
            "goal": { "QueryKnowledge": { "slot_id": 1, "query": "test_key" } }
        });
        let query_req = Request::builder()
            .method("POST")
            .uri("/v1/execute")
            .header("content-type", "application/json")
            .body(Body::from(serde_json::to_string(&query_body).unwrap()))
            .unwrap();
        let query_res = app.clone().oneshot(query_req).await.unwrap();
        assert_eq!(query_res.status(), StatusCode::OK);

        let recall_body = serde_json::json!({
            "tenant_id": "test-tenant",
            "goal": {
                "ExecuteSkill": {
                    "name": "recall_past_actions",
                    "payload": { "limit": 5 }
                }
            }
        });
        let recall_req = Request::builder()
            .method("POST")
            .uri("/v1/execute")
            .header("content-type", "application/json")
            .body(Body::from(serde_json::to_string(&recall_body).unwrap()))
            .unwrap();
        let recall_res = app.oneshot(recall_req).await.unwrap();
        assert_eq!(recall_res.status(), StatusCode::OK);
        let recall_bytes = axum::body::to_bytes(recall_res.into_body(), usize::MAX).await.unwrap();
        let recall_json: serde_json::Value = serde_json::from_slice(&recall_bytes).unwrap();
        assert_eq!(recall_json["status"], "ok");
        assert_eq!(recall_json["skill"], "recall_past_actions");
        let events = recall_json["events"].as_array().expect("events array");
        assert!(!events.is_empty(), "Chronos should have at least one event after QueryKnowledge");
        let has_query_event = events
            .iter()
            .any(|e| e["reflection"].as_str().unwrap_or("").contains("Queried"));
        assert!(
            has_query_event,
            "Chronos should contain the QueryKnowledge event; got events: {:?}",
            events
        );
    }

    #[tokio::test]
    async fn test_ethos_blocks_write_sandbox_with_mock_secret_and_logs_violation() {
        let knowledge = Arc::new(
            KnowledgeStore::open_path("./data/pagi_ethos_violation_test").unwrap(),
        );
        knowledge.set_ethos_policy(&PolicyRecord::default()).unwrap();
        let mut registry = SkillRegistry::new();
        registry.register(Arc::new(WriteSandboxFile::new()));
        registry.register(Arc::new(RecallPastActions::new(Arc::clone(&knowledge))));
        let orchestrator = Arc::new(Orchestrator::new(Arc::new(registry)));
        let app = Router::new()
            .route("/v1/execute", post(execute))
            .with_state(AppState {
                config: Arc::new(test_config()),
                orchestrator,
                knowledge: Arc::clone(&knowledge),
                log_tx: test_log_tx(),
                model_router: test_model_router(),
            });

        let write_body = serde_json::json!({
            "tenant_id": "test-tenant",
            "goal": {
                "ExecuteSkill": {
                    "name": "write_sandbox_file",
                    "payload": {
                        "path": "ethos_test.txt",
                        "content": "Do not store: api_key=sk-12345 and password=secret123"
                    }
                }
            }
        });
        let write_req = Request::builder()
            .method("POST")
            .uri("/v1/execute")
            .header("content-type", "application/json")
            .body(Body::from(serde_json::to_string(&write_body).unwrap()))
            .unwrap();
        let write_res = app.clone().oneshot(write_req).await.unwrap();
        assert_eq!(write_res.status(), StatusCode::OK);
        let write_bytes = axum::body::to_bytes(write_res.into_body(), usize::MAX).await.unwrap();
        let write_json: serde_json::Value = serde_json::from_slice(&write_bytes).unwrap();
        assert_eq!(
            write_json["status"],
            "policy_violation",
            "Ethos should block write when content contains sensitive keywords; got: {:?}",
            write_json
        );
        assert!(write_json["error"].as_str().unwrap_or("").contains("sensitive") || write_json["error"].as_str().unwrap_or("").contains("keyword"));

        let recall_body = serde_json::json!({
            "tenant_id": "test-tenant",
            "goal": {
                "ExecuteSkill": {
                    "name": "recall_past_actions",
                    "payload": { "limit": 5 }
                }
            }
        });
        let recall_req = Request::builder()
            .method("POST")
            .uri("/v1/execute")
            .header("content-type", "application/json")
            .body(Body::from(serde_json::to_string(&recall_body).unwrap()))
            .unwrap();
        let recall_res = app.oneshot(recall_req).await.unwrap();
        assert_eq!(recall_res.status(), StatusCode::OK);
        let recall_bytes = axum::body::to_bytes(recall_res.into_body(), usize::MAX).await.unwrap();
        let recall_json: serde_json::Value = serde_json::from_slice(&recall_bytes).unwrap();
        let events = recall_json["events"].as_array().expect("events array");
        let has_violation = events
            .iter()
            .any(|e| e["reflection"].as_str().unwrap_or("").contains("Policy Violation"));
        assert!(
            has_violation,
            "Chronos should contain a Policy Violation event; got events: {:?}",
            events
        );
    }

    #[tokio::test]
    async fn test_kardia_sentiment_stored_and_chat_injects_context() {
        let knowledge = Arc::new(
            KnowledgeStore::open_path("./data/pagi_kardia_verify_test").unwrap(),
        );
        let mut registry = SkillRegistry::new();
        registry.register(Arc::new(AnalyzeSentiment::new(Arc::clone(&knowledge))));
        registry.register(Arc::new(ModelRouter::with_knowledge(Arc::clone(&knowledge))));
        let orchestrator = Arc::new(Orchestrator::new(Arc::new(registry)));
        let app = Router::new()
            .route("/v1/execute", post(execute))
            .route("/api/v1/kardia/:user_id", get(get_kardia_relation))
            .route("/api/v1/chat", post(chat))
            .with_state(AppState {
                config: Arc::new(test_config()),
                orchestrator,
                knowledge: Arc::clone(&knowledge),
                log_tx: test_log_tx(),
                model_router: Arc::new(ModelRouter::with_knowledge(Arc::clone(&knowledge))),
            });

        let sentiment_body = serde_json::json!({
            "tenant_id": "kardia-verify-user",
            "goal": {
                "ExecuteSkill": {
                    "name": "analyze_sentiment",
                    "payload": {
                        "user_id": "kardia-verify-user",
                        "messages": ["I am so angry", "This is terrible", "Nothing works"]
                    }
                }
            }
        });
        let sentiment_req = Request::builder()
            .method("POST")
            .uri("/v1/execute")
            .header("content-type", "application/json")
            .body(Body::from(serde_json::to_string(&sentiment_body).unwrap()))
            .unwrap();
        let sentiment_res = app.clone().oneshot(sentiment_req).await.unwrap();
        assert_eq!(sentiment_res.status(), StatusCode::OK);
        let sentiment_json: serde_json::Value = serde_json::from_slice(
            &axum::body::to_bytes(sentiment_res.into_body(), usize::MAX).await.unwrap(),
        )
        .unwrap();
        assert_eq!(sentiment_json["status"], "ok");
        assert_eq!(sentiment_json["last_sentiment"], "angry");

        let kardia_req = Request::builder()
            .method("GET")
            .uri("/api/v1/kardia/kardia-verify-user")
            .body(Body::empty())
            .unwrap();
        let kardia_res = app.clone().oneshot(kardia_req).await.unwrap();
        assert_eq!(kardia_res.status(), StatusCode::OK);
        let kardia_json: serde_json::Value = serde_json::from_slice(
            &axum::body::to_bytes(kardia_res.into_body(), usize::MAX).await.unwrap(),
        )
        .unwrap();
        assert_eq!(kardia_json["last_sentiment"], "angry");
        assert_eq!(kardia_json["user_id"], "kardia-verify-user");

        let chat_body = serde_json::json!({
            "prompt": "How would you describe our current working relationship?",
            "stream": false,
            "user_alias": "kardia-verify-user"
        });
        let chat_req = Request::builder()
            .method("POST")
            .uri("/api/v1/chat")
            .header("content-type", "application/json")
            .body(Body::from(serde_json::to_string(&chat_body).unwrap()))
            .unwrap();
        let chat_res = app.oneshot(chat_req).await.unwrap();
        assert_eq!(chat_res.status(), StatusCode::OK);
        let chat_json: serde_json::Value = serde_json::from_slice(
            &axum::body::to_bytes(chat_res.into_body(), usize::MAX).await.unwrap(),
        )
        .unwrap();
        assert_eq!(chat_json["status"], "ok");
        assert!(chat_json.get("response").and_then(|v| v.as_str()).unwrap_or("").len() > 0);
    }

    #[tokio::test]
    async fn test_kb2_insert_and_retrieve_welcome_template() {
        let knowledge = Arc::new(
            KnowledgeStore::open_path("./data/pagi_kb2_test").unwrap(),
        );
        let mut registry = SkillRegistry::new();
        registry.register(Arc::new(KnowledgeInsert::new(Arc::clone(&knowledge))));
        registry.register(Arc::new(KnowledgeQuery::new(Arc::clone(&knowledge))));
        let orchestrator = Arc::new(Orchestrator::new(Arc::new(registry)));
        let app = Router::new()
            .route("/v1/execute", post(execute))
            .with_state(AppState {
            config: Arc::new(test_config()),
            orchestrator,
            knowledge,
            log_tx: test_log_tx(),
            model_router: test_model_router(),
        });

        let insert_body = serde_json::json!({
            "tenant_id": "test-tenant",
            "goal": {
                "ExecuteSkill": {
                    "name": "KnowledgeInsert",
                    "payload": {
                        "slot_id": 2,
                        "key": "welcome_email_template",
                        "value": "Welcome! We're glad you reached out. A team member will follow up within 24 hours."
                    }
                }
            }
        });
        let insert_req = Request::builder()
            .method("POST")
            .uri("/v1/execute")
            .header("content-type", "application/json")
            .body(Body::from(serde_json::to_string(&insert_body).unwrap()))
            .unwrap();
        let insert_res = app.clone().oneshot(insert_req).await.unwrap();
        assert_eq!(insert_res.status(), StatusCode::OK);
        let insert_bytes = axum::body::to_bytes(insert_res.into_body(), usize::MAX)
            .await
            .unwrap();
        let insert_json: serde_json::Value = serde_json::from_slice(&insert_bytes).unwrap();
        assert_eq!(insert_json["status"], "ok");
        assert_eq!(insert_json["skill"], "KnowledgeInsert");
        assert_eq!(insert_json["slot_id"], 2);
        assert_eq!(insert_json["key"], "welcome_email_template");

        let query_body = serde_json::json!({
            "tenant_id": "test-tenant",
            "goal": {
                "QueryKnowledge": { "slot_id": 2, "query": "welcome_email_template" }
            }
        });
        let query_req = Request::builder()
            .method("POST")
            .uri("/v1/execute")
            .header("content-type", "application/json")
            .body(Body::from(serde_json::to_string(&query_body).unwrap()))
            .unwrap();
        let query_res = app.oneshot(query_req).await.unwrap();
        assert_eq!(query_res.status(), StatusCode::OK);
        let query_bytes = axum::body::to_bytes(query_res.into_body(), usize::MAX)
            .await
            .unwrap();
        let query_json: serde_json::Value = serde_json::from_slice(&query_bytes).unwrap();
        assert_eq!(query_json["status"], "ok");
        assert_eq!(query_json["skill"], "KnowledgeQuery");
        assert_eq!(query_json["value"], "Welcome! We're glad you reached out. A team member will follow up within 24 hours.");
    }

    #[tokio::test]
    async fn test_draft_response_includes_brand_voice_and_local_event() {
        let memory = Arc::new(MemoryManager::open_path("./data/pagi_vault_draft_test").unwrap());
        let knowledge = Arc::new(
            KnowledgeStore::open_path("./data/pagi_knowledge_draft_test").unwrap(),
        );

        // Set Brand Voice in KB-1
        knowledge.insert(1, "brand_voice", b"Warm, neighborly, and helpful").unwrap();

        // Set Local Event in KB-5 via CommunityPulse
        let mut registry = SkillRegistry::new();
        registry.register(Arc::new(LeadCapture::new(Arc::clone(&memory))));
        registry.register(Arc::new(KnowledgeInsert::new(Arc::clone(&knowledge))));
        registry.register(Arc::new(CommunityPulse::new(Arc::clone(&knowledge))));
        registry.register(Arc::new(DraftResponse::new(
            Arc::clone(&memory),
            Arc::clone(&knowledge),
        )));
        let orchestrator = Arc::new(Orchestrator::new(Arc::new(registry)));
        let app = Router::new()
            .route("/v1/execute", post(execute))
            .with_state(AppState {
            config: Arc::new(test_config()),
            orchestrator,
            knowledge,
            log_tx: test_log_tx(),
            model_router: test_model_router(),
        });

        // 1. Capture a lead to get lead_id (IngestData)
        let lead_body = serde_json::json!({
            "tenant_id": "test-tenant",
            "goal": {
                "IngestData": {
                    "payload": { "email": "customer@example.com", "message": "Interested in services" }
                }
            }
        });
        let lead_req = Request::builder()
            .method("POST")
            .uri("/v1/execute")
            .header("content-type", "application/json")
            .body(Body::from(serde_json::to_string(&lead_body).unwrap()))
            .unwrap();
        let lead_res = app.clone().oneshot(lead_req).await.unwrap();
        assert_eq!(lead_res.status(), StatusCode::OK);
        let lead_bytes = axum::body::to_bytes(lead_res.into_body(), usize::MAX).await.unwrap();
        let lead_json: serde_json::Value = serde_json::from_slice(&lead_bytes).unwrap();
        let lead_id = lead_json["lead_id"].as_str().unwrap().to_string();

        // 2. Set Community Pulse (e.g. Strawberry Festival) in KB-5
        let pulse_body = serde_json::json!({
            "tenant_id": "test-tenant",
            "goal": {
                "ExecuteSkill": {
                    "name": "CommunityPulse",
                    "payload": {
                        "location": "Stockdale",
                        "trend": "rainy week",
                        "event": "Strawberry Festival this weekend"
                    }
                }
            }
        });
        let pulse_req = Request::builder()
            .method("POST")
            .uri("/v1/execute")
            .header("content-type", "application/json")
            .body(Body::from(serde_json::to_string(&pulse_body).unwrap()))
            .unwrap();
        let pulse_res = app.clone().oneshot(pulse_req).await.unwrap();
        assert_eq!(pulse_res.status(), StatusCode::OK);

        // 3. Execute AssembleContext (draft for this context_id)
        let draft_body = serde_json::json!({
            "tenant_id": "test-tenant",
            "goal": {
                "AssembleContext": { "context_id": lead_id }
            }
        });
        let draft_req = Request::builder()
            .method("POST")
            .uri("/v1/execute")
            .header("content-type", "application/json")
            .body(Body::from(serde_json::to_string(&draft_body).unwrap()))
            .unwrap();
        let draft_res = app.oneshot(draft_req).await.unwrap();
        assert_eq!(draft_res.status(), StatusCode::OK);
        let draft_bytes = axum::body::to_bytes(draft_res.into_body(), usize::MAX).await.unwrap();
        let draft_json: serde_json::Value = serde_json::from_slice(&draft_bytes).unwrap();
        assert_eq!(draft_json["status"], "ok");
        assert_eq!(draft_json["skill"], "DraftResponse");

        let draft_text = draft_json["draft"].as_str().unwrap();
        assert!(draft_text.contains("Warm, neighborly, and helpful"), "draft should include Brand Voice from KB-1");
        assert!(draft_text.contains("Strawberry Festival this weekend"), "draft should include Local Event from KB-5");
        assert!(draft_text.contains("Local Context:"), "draft should include Local Context section");
    }

    #[tokio::test]
    async fn test_generate_final_response_chain_returns_generated_string() {
        let memory = Arc::new(
            MemoryManager::open_path("./data/pagi_vault_generate_test").unwrap(),
        );
        let knowledge = Arc::new(
            KnowledgeStore::open_path("./data/pagi_knowledge_generate_test").unwrap(),
        );
        knowledge.insert(1, "brand_voice", b"Warm and professional").unwrap();

        let mut registry = SkillRegistry::new();
        registry.register(Arc::new(LeadCapture::new(Arc::clone(&memory))));
        registry.register(Arc::new(DraftResponse::new(
            Arc::clone(&memory),
            Arc::clone(&knowledge),
        )));
        registry.register(Arc::new(ModelRouter::new()));
        let orchestrator = Arc::new(Orchestrator::new(Arc::new(registry)));
        let app = Router::new()
            .route("/v1/execute", post(execute))
            .with_state(AppState {
            config: Arc::new(test_config()),
            orchestrator,
            knowledge,
            log_tx: test_log_tx(),
            model_router: test_model_router(),
        });

        // 1. Capture a lead (IngestData)
        let lead_body = serde_json::json!({
            "tenant_id": "test-tenant",
            "goal": {
                "IngestData": {
                    "payload": { "email": "guest@example.com", "message": "Hello" }
                }
            }
        });
        let lead_req = Request::builder()
            .method("POST")
            .uri("/v1/execute")
            .header("content-type", "application/json")
            .body(Body::from(serde_json::to_string(&lead_body).unwrap()))
            .unwrap();
        let lead_res = app.clone().oneshot(lead_req).await.unwrap();
        assert_eq!(lead_res.status(), StatusCode::OK);
        let lead_bytes = axum::body::to_bytes(lead_res.into_body(), usize::MAX).await.unwrap();
        let lead_json: serde_json::Value = serde_json::from_slice(&lead_bytes).unwrap();
        let lead_id = lead_json["lead_id"].as_str().unwrap().to_string();

        // 2. Generate final response (AssembleContext -> ModelRouter chain)
        let gen_body = serde_json::json!({
            "tenant_id": "test-tenant",
            "goal": {
                "GenerateFinalResponse": { "context_id": lead_id }
            }
        });
        let gen_req = Request::builder()
            .method("POST")
            .uri("/v1/execute")
            .header("content-type", "application/json")
            .body(Body::from(serde_json::to_string(&gen_body).unwrap()))
            .unwrap();
        let gen_res = app.oneshot(gen_req).await.unwrap();
        assert_eq!(gen_res.status(), StatusCode::OK);
        let gen_bytes = axum::body::to_bytes(gen_res.into_body(), usize::MAX).await.unwrap();
        let gen_json: serde_json::Value = serde_json::from_slice(&gen_bytes).unwrap();

        assert_eq!(gen_json["status"], "ok");
        assert_eq!(gen_json["goal"], "GenerateFinalResponse");
        assert_eq!(gen_json["context_id"], lead_id);
        let generated = gen_json["generated"].as_str().expect("response must contain 'generated' string");
        assert!(!generated.is_empty(), "generated text must not be empty");
        assert!(
            generated.contains("Generated") || generated.contains("personalized") || generated.contains("Thank you"),
            "generated should be LLM-style output, not just the raw mock draft template"
        );
    }

    #[tokio::test]
    async fn test_autonomous_goal_respond_to_lead_triggers_generation_chain() {
        let memory = Arc::new(
            MemoryManager::open_path("./data/pagi_vault_autonomous_test").unwrap(),
        );
        let knowledge = Arc::new(
            KnowledgeStore::open_path("./data/pagi_knowledge_autonomous_test").unwrap(),
        );
        knowledge.insert(1, "brand_voice", b"Friendly and local").unwrap();

        let mut registry = SkillRegistry::new();
        registry.register(Arc::new(LeadCapture::new(Arc::clone(&memory))));
        registry.register(Arc::new(DraftResponse::new(
            Arc::clone(&memory),
            Arc::clone(&knowledge),
        )));
        registry.register(Arc::new(SalesCloser::new(Arc::clone(&knowledge))));
        registry.register(Arc::new(ModelRouter::new()));
        registry.register(Arc::new(ResearchAudit::new(Arc::clone(&knowledge))));
        let orchestrator = Arc::new(Orchestrator::new(Arc::new(registry)));
        let app = Router::new()
            .route("/v1/execute", post(execute))
            .route("/v1/research/trace/:trace_id", get(get_research_trace))
            .with_state(AppState {
            config: Arc::new(test_config()),
            orchestrator,
            knowledge,
            log_tx: test_log_tx(),
            model_router: test_model_router(),
        });

        // 1. Capture a lead (IngestData)
        let lead_body = serde_json::json!({
            "tenant_id": "test-tenant",
            "goal": {
                "IngestData": {
                    "payload": { "email": "neighbor@town.com", "message": "Interested in events" }
                }
            }
        });
        let lead_req = Request::builder()
            .method("POST")
            .uri("/v1/execute")
            .header("content-type", "application/json")
            .body(Body::from(serde_json::to_string(&lead_body).unwrap()))
            .unwrap();
        let lead_res = app.clone().oneshot(lead_req).await.unwrap();
        assert_eq!(lead_res.status(), StatusCode::OK);
        let lead_bytes = axum::body::to_bytes(lead_res.into_body(), usize::MAX).await.unwrap();
        let lead_json: serde_json::Value = serde_json::from_slice(&lead_bytes).unwrap();
        let lead_id = lead_json["lead_id"].as_str().unwrap().to_string();

        // 2. AutonomousGoal "respond to lead" with context.lead_id
        let autonomous_body = serde_json::json!({
            "tenant_id": "test-tenant",
            "goal": {
                "AutonomousGoal": {
                    "intent": "respond to lead",
                    "context": { "lead_id": lead_id }
                }
            }
        });
        let autonomous_req = Request::builder()
            .method("POST")
            .uri("/v1/execute")
            .header("content-type", "application/json")
            .body(Body::from(serde_json::to_string(&autonomous_body).unwrap()))
            .unwrap();
        let autonomous_res = app.clone().oneshot(autonomous_req).await.unwrap();
        assert_eq!(autonomous_res.status(), StatusCode::OK);
        let autonomous_bytes = axum::body::to_bytes(autonomous_res.into_body(), usize::MAX).await.unwrap();
        let auto_json: serde_json::Value = serde_json::from_slice(&autonomous_bytes).unwrap();

        assert_eq!(auto_json["goal"], "AutonomousGoal");
        assert_eq!(auto_json["intent"], "respond to lead");
        assert_eq!(
            auto_json["plan_steps"],
            serde_json::json!(["DraftResponse", "SalesCloser", "ModelRouter"])
        );
        let generated = auto_json["generated"].as_str().expect("response must contain 'generated' from chain");
        assert!(!generated.is_empty());
        assert!(
            generated.contains("Generated") || generated.contains("personalized") || generated.contains("Thank you"),
            "autonomous chain should produce LLM-style generated text"
        );
        let trace_id = auto_json["trace_id"].as_str().expect("ResearchAudit should return trace_id");
        assert!(!trace_id.is_empty());

        // 3. Retrieve Thought Log from research endpoint
        let trace_req = Request::builder()
            .method("GET")
            .uri(format!("/v1/research/trace/{}", trace_id))
            .body(Body::empty())
            .unwrap();
        let trace_res = app.oneshot(trace_req).await.unwrap();
        assert_eq!(trace_res.status(), StatusCode::OK);
        let trace_bytes = axum::body::to_bytes(trace_res.into_body(), usize::MAX).await.unwrap();
        let trace_json: serde_json::Value = serde_json::from_slice(&trace_bytes).unwrap();
        assert_eq!(trace_json["trace_id"], trace_id);
        let trace_inner = &trace_json["trace"];
        assert_eq!(trace_inner["intent"], "respond to lead");
        assert_eq!(
            trace_inner["plan_steps"],
            serde_json::json!(["DraftResponse", "SalesCloser", "ModelRouter"])
        );
        let steps = trace_inner["steps"].as_array().expect("trace should have steps array");
        assert_eq!(steps.len(), 3, "respond to lead has three steps");
        assert_eq!(steps[0]["skill"], "DraftResponse");
        assert_eq!(steps[1]["skill"], "SalesCloser");
        assert_eq!(steps[2]["skill"], "ModelRouter");
        assert!(trace_inner.get("final_result").is_some(), "trace should have final_result");
    }

    #[tokio::test]
    async fn test_community_scraper_extracts_event_and_saves_to_kb5() {
        let knowledge = Arc::new(
            KnowledgeStore::open_path("./data/pagi_knowledge_scraper_test").unwrap(),
        );
        let mut registry = SkillRegistry::new();
        registry.register(Arc::new(CommunityScraper::new(Arc::clone(&knowledge))));
        registry.register(Arc::new(KnowledgeQuery::new(Arc::clone(&knowledge))));
        let orchestrator = Arc::new(Orchestrator::new(Arc::new(registry)));
        let app = Router::new()
            .route("/v1/execute", post(execute))
            .with_state(AppState {
            config: Arc::new(test_config()),
            orchestrator,
            knowledge,
            log_tx: test_log_tx(),
            model_router: test_model_router(),
        });

        let mock_html = r#"<!DOCTYPE html>
<html><body>
<h1>Stockdale Fair 2025</h1>
<h2>Local events this weekend</h2>
<article><h2>Farmers Market Sunday</h2></article>
</body></html>"#;

        let scrape_body = serde_json::json!({
            "tenant_id": "test-tenant",
            "goal": {
                "ExecuteSkill": {
                    "name": "CommunityScraper",
                    "payload": {
                        "url": "https://example.com/local-news",
                        "html": mock_html
                    }
                }
            }
        });
        let scrape_req = Request::builder()
            .method("POST")
            .uri("/v1/execute")
            .header("content-type", "application/json")
            .body(Body::from(serde_json::to_string(&scrape_body).unwrap()))
            .unwrap();
        let scrape_res = app.clone().oneshot(scrape_req).await.unwrap();
        assert_eq!(scrape_res.status(), StatusCode::OK);
        let scrape_bytes = axum::body::to_bytes(scrape_res.into_body(), usize::MAX).await.unwrap();
        let scrape_json: serde_json::Value = serde_json::from_slice(&scrape_bytes).unwrap();
        assert_eq!(scrape_json["status"], "ok");
        assert_eq!(scrape_json["skill"], "CommunityScraper");
        assert_eq!(scrape_json["slot_id"], 5);
        assert!(scrape_json["event"].as_str().unwrap().contains("Stockdale Fair 2025"));
        assert!(scrape_json["event"].as_str().unwrap().contains("Local events this weekend"));
        assert!(scrape_json["event"].as_str().unwrap().contains("Farmers Market Sunday"));

        let query_body = serde_json::json!({
            "tenant_id": "test-tenant",
            "goal": {
                "QueryKnowledge": { "slot_id": 5, "query": "current_pulse" }
            }
        });
        let query_req = Request::builder()
            .method("POST")
            .uri("/v1/execute")
            .header("content-type", "application/json")
            .body(Body::from(serde_json::to_string(&query_body).unwrap()))
            .unwrap();
        let query_res = app.oneshot(query_req).await.unwrap();
        assert_eq!(query_res.status(), StatusCode::OK);
        let query_bytes = axum::body::to_bytes(query_res.into_body(), usize::MAX).await.unwrap();
        let query_json: serde_json::Value = serde_json::from_slice(&query_bytes).unwrap();
        assert_eq!(query_json["status"], "ok");
        assert_eq!(query_json["slot_id"], 5);
        assert_eq!(query_json["query_key"], "current_pulse");
        let value = query_json["value"].as_str().expect("current_pulse value");
        let pulse: serde_json::Value = serde_json::from_str(value).unwrap();
        assert_eq!(pulse["location"], "Stockdale");
        assert_eq!(pulse["trend"], "Scraped");
        assert!(pulse["event"].as_str().unwrap().contains("Stockdale Fair 2025"));
    }

    #[tokio::test]
    async fn test_refresh_local_context_dispatches_community_scraper() {
        let knowledge = Arc::new(
            KnowledgeStore::open_path("./data/pagi_knowledge_refresh_test").unwrap(),
        );
        let mut registry = SkillRegistry::new();
        registry.register(Arc::new(CommunityScraper::new(Arc::clone(&knowledge))));
        registry.register(Arc::new(KnowledgeQuery::new(Arc::clone(&knowledge))));
        let orchestrator = Arc::new(Orchestrator::new(Arc::new(registry)));
        let app = Router::new()
            .route("/v1/execute", post(execute))
            .with_state(AppState {
            config: Arc::new(test_config()),
            orchestrator,
            knowledge,
            log_tx: test_log_tx(),
            model_router: test_model_router(),
        });

        let mock_html = r#"<html><body><h1>Fall Festival Next Week</h1></body></html>"#;
        let body = serde_json::json!({
            "tenant_id": "test-tenant",
            "goal": {
                "UpdateKnowledgeSlot": {
                    "slot_id": 5,
                    "source_url": "https://example.com/news",
                    "source_html": mock_html
                }
            }
        });
        let req = Request::builder()
            .method("POST")
            .uri("/v1/execute")
            .header("content-type", "application/json")
            .body(Body::from(serde_json::to_string(&body).unwrap()))
            .unwrap();
        let res = app.oneshot(req).await.unwrap();
        assert_eq!(res.status(), StatusCode::OK);
        let bytes = axum::body::to_bytes(res.into_body(), usize::MAX).await.unwrap();
        let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(json["status"], "ok");
        assert_eq!(json["skill"], "CommunityScraper");
        assert!(json["event"].as_str().unwrap().contains("Fall Festival Next Week"));
    }

    #[tokio::test]
    async fn test_sales_closer_cta_in_final_response() {
        let memory = Arc::new(
            MemoryManager::open_path("./data/pagi_vault_sales_test").unwrap(),
        );
        let knowledge = Arc::new(
            KnowledgeStore::open_path("./data/pagi_knowledge_sales_test").unwrap(),
        );
        knowledge.insert(1, "brand_voice", b"Warm and professional").unwrap();
        knowledge
            .insert(2, "closing_strategy", b"Book a free consultation today")
            .unwrap();

        let mut registry = SkillRegistry::new();
        registry.register(Arc::new(LeadCapture::new(Arc::clone(&memory))));
        registry.register(Arc::new(DraftResponse::new(
            Arc::clone(&memory),
            Arc::clone(&knowledge),
        )));
        registry.register(Arc::new(SalesCloser::new(Arc::clone(&knowledge))));
        registry.register(Arc::new(ModelRouter::new()));
        let orchestrator = Arc::new(Orchestrator::new(Arc::new(registry)));
        let app = Router::new()
            .route("/v1/execute", post(execute))
            .with_state(AppState {
            config: Arc::new(test_config()),
            orchestrator,
            knowledge,
            log_tx: test_log_tx(),
            model_router: test_model_router(),
        });

        let lead_body = serde_json::json!({
            "tenant_id": "test-tenant",
            "goal": {
                "IngestData": {
                    "payload": { "email": "lead@example.com", "message": "Interested in services" }
                }
            }
        });
        let lead_req = Request::builder()
            .method("POST")
            .uri("/v1/execute")
            .header("content-type", "application/json")
            .body(Body::from(serde_json::to_string(&lead_body).unwrap()))
            .unwrap();
        let lead_res = app.clone().oneshot(lead_req).await.unwrap();
        assert_eq!(lead_res.status(), StatusCode::OK);
        let lead_bytes = axum::body::to_bytes(lead_res.into_body(), usize::MAX).await.unwrap();
        let lead_json: serde_json::Value = serde_json::from_slice(&lead_bytes).unwrap();
        let lead_id = lead_json["lead_id"].as_str().unwrap().to_string();

        let auto_body = serde_json::json!({
            "tenant_id": "test-tenant",
            "goal": {
                "AutonomousGoal": {
                    "intent": "respond to lead",
                    "context": { "lead_id": lead_id }
                }
            }
        });
        let auto_req = Request::builder()
            .method("POST")
            .uri("/v1/execute")
            .header("content-type", "application/json")
            .body(Body::from(serde_json::to_string(&auto_body).unwrap()))
            .unwrap();
        let auto_res = app.oneshot(auto_req).await.unwrap();
        assert_eq!(auto_res.status(), StatusCode::OK);
        let auto_bytes = axum::body::to_bytes(auto_res.into_body(), usize::MAX).await.unwrap();
        let auto_json: serde_json::Value = serde_json::from_slice(&auto_bytes).unwrap();
        let generated = auto_json["generated"].as_str().expect("generated");
        assert!(
            generated.to_lowercase().contains("free consultation"),
            "final generated response should include the KB-2 sales push (free consultation); got: {}",
            generated
        );
    }

    #[tokio::test]
    async fn test_blueprint_alternate_intent_summarize_news() {
        let knowledge = Arc::new(
            KnowledgeStore::open_path("./data/pagi_knowledge_blueprint_test").unwrap(),
        );
        let mut registry = SkillRegistry::new();
        registry.register(Arc::new(CommunityScraper::new(Arc::clone(&knowledge))));
        registry.register(Arc::new(ModelRouter::new()));

        let mut intents = std::collections::HashMap::new();
        intents.insert(
            "summarize news".to_string(),
            vec!["CommunityScraper".to_string(), "ModelRouter".to_string()],
        );
        let blueprint = Arc::new(BlueprintRegistry::from_intents(intents));
        let orchestrator = Arc::new(Orchestrator::with_blueprint(
            Arc::new(registry),
            Arc::clone(&blueprint),
        ));
        let app = Router::new()
            .route("/v1/execute", post(execute))
            .with_state(AppState {
            config: Arc::new(test_config()),
            orchestrator,
            knowledge,
            log_tx: test_log_tx(),
            model_router: test_model_router(),
        });

        let body = serde_json::json!({
            "tenant_id": "test-tenant",
            "goal": {
                "AutonomousGoal": {
                    "intent": "summarize news",
                    "context": {
                        "slot_id": 5,
                        "html": "<html><body><h1>Local Election Results</h1><h2>Budget approved</h2></body></html>"
                    }
                }
            }
        });
        let req = Request::builder()
            .method("POST")
            .uri("/v1/execute")
            .header("content-type", "application/json")
            .body(Body::from(serde_json::to_string(&body).unwrap()))
            .unwrap();
        let res = app.oneshot(req).await.unwrap();
        assert_eq!(res.status(), StatusCode::OK);
        let bytes = axum::body::to_bytes(res.into_body(), usize::MAX).await.unwrap();
        let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(json["goal"], "AutonomousGoal");
        assert_eq!(json["intent"], "summarize news");
        assert_eq!(
            json["plan_steps"],
            serde_json::json!(["CommunityScraper", "ModelRouter"])
        );
        let generated = json["generated"].as_str().expect("generated");
        assert!(
            generated.contains("Election") || generated.contains("Budget") || generated.contains("personalized"),
            "generated should reflect scraped content or mock; got: {}",
            generated
        );
    }

    #[tokio::test]
    async fn test_knowledge_pruner_removes_old_kb5_and_kb8_entries() {
        let knowledge = Arc::new(
            KnowledgeStore::open_path("./data/pagi_knowledge_pruner_test").unwrap(),
        );
        let old_ts = 1_u64;
        let old_pulse = serde_json::json!({
            "location": "Test",
            "trend": "old",
            "event": "Stale event",
            "updated_at": old_ts
        });
        let old_trace = serde_json::json!({
            "trace_id": "old-trace-id",
            "created_at": old_ts,
            "trace": { "intent": "test" }
        });
        knowledge
            .insert(5, "stale_pulse", old_pulse.to_string().as_bytes())
            .unwrap();
        knowledge
            .insert(8, "old-trace-id", old_trace.to_string().as_bytes())
            .unwrap();

        let mut registry = SkillRegistry::new();
        registry.register(Arc::new(KnowledgePruner::new(Arc::clone(&knowledge))));
        let orchestrator = Arc::new(Orchestrator::new(Arc::new(registry)));
        let app = Router::new()
            .route("/v1/execute", post(execute))
            .with_state(AppState {
                config: Arc::new(test_config()),
                orchestrator,
                knowledge: Arc::clone(&knowledge),
                log_tx: test_log_tx(),
                model_router: test_model_router(),
            });

        let prune_body = serde_json::json!({
            "tenant_id": "test-tenant",
            "goal": {
                "ExecuteSkill": {
                    "name": "KnowledgePruner",
                    "payload": { "kb5_max_age_days": 1, "kb8_max_age_days": 1 }
                }
            }
        });
        let req = Request::builder()
            .method("POST")
            .uri("/v1/execute")
            .header("content-type", "application/json")
            .body(Body::from(serde_json::to_string(&prune_body).unwrap()))
            .unwrap();
        let res = app.oneshot(req).await.unwrap();
        assert_eq!(res.status(), StatusCode::OK);
        let bytes = axum::body::to_bytes(res.into_body(), usize::MAX).await.unwrap();
        let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(json["status"], "ok");
        assert_eq!(json["skill"], "KnowledgePruner");
        assert_eq!(json["kb5_pruned"], 1);
        assert_eq!(json["kb8_pruned"], 1);
        assert!(json["kb5_removed_keys"].as_array().unwrap().contains(&serde_json::json!("stale_pulse")));
        assert!(json["kb8_removed_keys"]
            .as_array()
            .unwrap()
            .iter()
            .any(|v| v.as_str() == Some("old-trace-id")));

        assert!(knowledge.get(5, "stale_pulse").unwrap().is_none());
        assert!(knowledge.get(8, "old-trace-id").unwrap().is_none());
    }
}
