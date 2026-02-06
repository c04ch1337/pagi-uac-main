//! Pagi Heartbeat Service (Autonomous Orchestrator)
//!
//! A long-running daemon that periodically checks agent inboxes (KB_SOMA)
//! and triggers background work without requiring synchronous polling.

use pagi_core::{CoreConfig, EventRecord, KnowledgeStore};
use pagi_skills::ModelRouter;
use std::{collections::HashSet, path::Path as StdPath, sync::Arc, time::Duration};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

/// Default tick rate to avoid slamming LLM APIs.
const DEFAULT_TICK_RATE_SECS: u64 = 5;

#[tokio::main]
async fn main() {
    // Load .env file if present (before any env::var calls)
    if let Err(e) = dotenvy::dotenv() {
        eprintln!("[pagi-daemon] .env not loaded: {} (using system environment)", e);
    }

    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::new(
            std::env::var("RUST_LOG").unwrap_or_else(|_| "info".into()),
        ))
        .with(tracing_subscriber::fmt::layer())
        .init();

    let config = Arc::new(CoreConfig::load().expect("load CoreConfig"));
    let tick_rate = std::env::var("PAGI_TICK_RATE_SECS")
        .ok()
        .and_then(|s| s.parse::<u64>().ok())
        .unwrap_or(DEFAULT_TICK_RATE_SECS)
        .max(1);

    let storage = StdPath::new(&config.storage_path);
    // NOTE: sled is single-writer; gateway and daemon must not open the same DB path concurrently.
    // Run the daemon against a separate copy/path, configurable via env.
    let knowledge_path = std::env::var("PAGI_DAEMON_KNOWLEDGE_PATH")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|_| storage.join("pagi_knowledge_daemon"));

    let knowledge = Arc::new(KnowledgeStore::open_path(&knowledge_path).expect("open daemon pagi_knowledge"));
    knowledge.pagi_init_kb_metadata().ok();

    // Router used to generate agent responses.
    let model_router = Arc::new(ModelRouter::with_knowledge(Arc::clone(&knowledge)));

    tracing::info!(
        tick_rate_secs = tick_rate,
        storage_path = %config.storage_path,
        "Pagi daemon started"
    );

    let mut interval = tokio::time::interval(Duration::from_secs(tick_rate));

    loop {
        tokio::select! {
            _ = interval.tick() => {
                if let Err(e) = tick(Arc::clone(&knowledge), Arc::clone(&model_router)).await {
                    tracing::warn!(error = %e, "daemon tick failed");
                }
            }
            _ = tokio::signal::ctrl_c() => {
                tracing::info!("CTRL-C received; shutting down daemon");
                break;
            }
        }
    }
}

async fn tick(
    knowledge: Arc<KnowledgeStore>,
    model_router: Arc<ModelRouter>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // Discover active agents by scanning KB_SOMA inbox keys: inbox/{agent_id}/...
    let soma_slot = pagi_core::KbType::Soma.slot_id();
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
        // AUTO-POLL: check inbox
        let messages = knowledge.get_agent_messages(&agent_id, 1)?;
        if let Some(msg) = messages.first() {
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
                .unwrap_or_else(|e| format!("[daemon] generation failed: {}", e));

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

            // Reflection: write a Chronos event for the agent.
            let reflection = EventRecord::now(
                "Chronos",
                format!("Auto-replied to message {} from {}", msg.id, msg.from_agent_id),
            )
            .with_skill("pagi-daemon")
            .with_outcome("auto_reply_sent");
            let _ = knowledge.append_chronos_event(&agent_id, &reflection);
        } else {
            // If no inbox message exists, check Pneuma for background tasks.
            // Minimal v1: if a key `pneuma/{agent_id}/background_task` exists, run it through the router.
            let pneuma_slot = pagi_core::KbType::Pneuma.slot_id();
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
                            .unwrap_or_else(|e| format!("[daemon] background generation failed: {}", e));
                        let reflection = EventRecord::now(
                            "Chronos",
                            format!("Background task ticked: {}", generated),
                        )
                        .with_skill("pagi-daemon")
                        .with_outcome("background_task_ticked");
                        let _ = knowledge.append_chronos_event(&agent_id, &reflection);
                    }
                }
            }
        }
    }

    Ok(())
}

