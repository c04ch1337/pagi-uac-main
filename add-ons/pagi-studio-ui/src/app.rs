//! Orchestrator, memory, and knowledge wiring for Studio UI (bare-metal).
//! Integrates control-panel channel so the UI can send ControlPanelMessage to the orchestrator.

use pagi_core::{
    BlueprintRegistry, ControlPanelMessage, KnowledgeStore, MemoryManager, Orchestrator,
    SkillRegistry, TenantContext,
};
use pagi_skills::{
    CommunityPulse, CommunityScraper, DraftResponse, KnowledgeInsert, KnowledgePruner,
    KnowledgeQuery, LeadCapture, ModelRouter, ResearchAudit, SalesCloser,
};
use std::path::Path;
use std::sync::Arc;
use tokio::sync::mpsc;

/// Studio stack: orchestrator + memory + knowledge + skill list + control sender.
pub struct StudioStack {
    pub orchestrator: Arc<Orchestrator>,
    pub memory: Arc<MemoryManager>,
    pub knowledge: Arc<KnowledgeStore>,
    pub skill_names: Vec<String>,
    /// Send control messages (KB toggles, skills enabled, memory weights) to the orchestrator.
    pub control_tx: mpsc::Sender<ControlPanelMessage>,
}

pub fn build_studio_stack(
    storage_dir: &Path,
) -> Result<(StudioStack, TenantContext), Box<dyn std::error::Error>> {
    let memory_path = storage_dir.join("pagi_vault");
    let knowledge_path = storage_dir.join("pagi_knowledge");

    let memory = Arc::new(MemoryManager::open_path(&memory_path)?);
    let knowledge = Arc::new(KnowledgeStore::open_path(&knowledge_path)?);
    knowledge.pagi_init_kb_metadata().ok(); // ensure 8 trees have metadata

    let mut registry = SkillRegistry::new();
    registry.register(Arc::new(LeadCapture::new(Arc::clone(&memory))));
    registry.register(Arc::new(KnowledgeQuery::new(Arc::clone(&knowledge))));
    registry.register(Arc::new(KnowledgeInsert::new(Arc::clone(&knowledge))));
    registry.register(Arc::new(CommunityPulse::new(Arc::clone(&knowledge))));
    registry.register(Arc::new(DraftResponse::new(
        Arc::clone(&memory),
        Arc::clone(&knowledge),
    )));
    registry.register(Arc::new(ModelRouter::new()));
    registry.register(Arc::new(ResearchAudit::new(Arc::clone(&knowledge))));
    registry.register(Arc::new(CommunityScraper::new(Arc::clone(&knowledge))));
    registry.register(Arc::new(SalesCloser::new(Arc::clone(&knowledge))));
    registry.register(Arc::new(KnowledgePruner::new(Arc::clone(&knowledge))));

    let blueprint_path = std::env::var("PAGI_BLUEPRINT_PATH")
        .unwrap_or_else(|_| "config/blueprint.json".to_string());
    let blueprint = Arc::new(BlueprintRegistry::load_json_path(&blueprint_path));

    let registry = Arc::new(registry);
    let skill_names = registry.skill_names();
    let orchestrator = Arc::new(Orchestrator::with_blueprint(registry, blueprint));

    let (control_tx, control_rx) = mpsc::channel(64);
    orchestrator.clone().spawn_control_listener(control_rx);

    let ctx = TenantContext {
        tenant_id: "pagi-studio-ui".to_string(),
        correlation_id: None,
    };

    Ok((
        StudioStack {
            orchestrator,
            memory,
            knowledge,
            skill_names,
            control_tx,
        },
        ctx,
    ))
}

/// Paths in short-term memory for Studio prompt/response (AGI state).
pub const MEMORY_PROMPT_PATH: &str = "studio/last_prompt";
pub const MEMORY_RESPONSE_PATH: &str = "studio/last_response";
