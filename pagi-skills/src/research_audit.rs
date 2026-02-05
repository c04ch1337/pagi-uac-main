//! Research Audit skill: saves execution traces (Thought Logs) to KB-8 (Internal Research).

use pagi_knowledge::KnowledgeStore;
use pagi_orchestrator::AgentSkill;
use pagi_shared::TenantContext;
use std::sync::Arc;

const SKILL_NAME: &str = "ResearchAudit";
const KB_SLOT_INTERNAL_RESEARCH: u8 = 8;

/// Saves a full execution trace to KB-8 for research and internal testing observability.
pub struct ResearchAudit {
    store: Arc<KnowledgeStore>,
}

impl ResearchAudit {
    pub fn new(store: Arc<KnowledgeStore>) -> Self {
        Self { store }
    }
}

#[async_trait::async_trait]
impl AgentSkill for ResearchAudit {
    fn name(&self) -> &str {
        SKILL_NAME
    }

    async fn execute(
        &self,
        _ctx: &TenantContext,
        payload: Option<serde_json::Value>,
    ) -> Result<serde_json::Value, Box<dyn std::error::Error + Send + Sync>> {
        let payload = payload.ok_or("ResearchAudit requires payload: { trace: object }")?;
        let trace = payload.get("trace").ok_or("trace required")?;
        let trace_id = uuid::Uuid::new_v4().to_string();
        let value = serde_json::json!({
            "trace_id": trace_id,
            "trace": trace
        });
        let value_str = serde_json::to_string(&value)?;
        self.store
            .insert(KB_SLOT_INTERNAL_RESEARCH, &trace_id, value_str.as_bytes())?;
        Ok(serde_json::json!({
            "status": "ok",
            "skill": SKILL_NAME,
            "trace_id": trace_id
        }))
    }
}
