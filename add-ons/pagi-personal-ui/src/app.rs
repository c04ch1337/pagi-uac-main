//! Orchestrator wiring for Personal UI (bare-metal, current_dir-relative).

use pagi_core::{
    BlueprintRegistry, KnowledgeStore, MemoryManager, Orchestrator, SkillRegistry, TenantContext,
};
use pagi_skills::{
    CommunityPulse, CommunityScraper, DraftResponse, KnowledgeInsert, KnowledgePruner,
    KnowledgeQuery, LeadCapture, ModelRouter, ResearchAudit, SalesCloser,
};
use std::path::Path;
use std::sync::Arc;

pub fn build_orchestrator(storage_dir: &Path) -> Result<Arc<Orchestrator>, Box<dyn std::error::Error>> {
    let memory_path = storage_dir.join("pagi_vault");
    let knowledge_path = storage_dir.join("pagi_knowledge");

    let memory = Arc::new(MemoryManager::open_path(&memory_path)?);
    let knowledge = Arc::new(KnowledgeStore::open_path(&knowledge_path)?);

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

    Ok(Arc::new(Orchestrator::with_blueprint(
        Arc::new(registry),
        blueprint,
    )))
}

pub fn default_tenant() -> TenantContext {
    TenantContext {
        tenant_id: "pagi-personal-ui".to_string(),
        correlation_id: None,
        agent_id: None,
    }
}
