//! Knowledge Insert skill: writes key-value pairs into a KB slot.

use pagi_knowledge::KnowledgeStore;
use pagi_orchestrator::AgentSkill;
use pagi_shared::TenantContext;
use std::sync::Arc;

const SKILL_NAME: &str = "KnowledgeInsert";

/// Writes values into the 8-slot knowledge base.
pub struct KnowledgeInsert {
    store: Arc<KnowledgeStore>,
}

impl KnowledgeInsert {
    pub fn new(store: Arc<KnowledgeStore>) -> Self {
        Self { store }
    }
}

#[async_trait::async_trait]
impl AgentSkill for KnowledgeInsert {
    fn name(&self) -> &str {
        SKILL_NAME
    }

    async fn execute(
        &self,
        _ctx: &TenantContext,
        payload: Option<serde_json::Value>,
    ) -> Result<serde_json::Value, Box<dyn std::error::Error + Send + Sync>> {
        let payload = payload.ok_or("KnowledgeInsert requires payload: { slot_id: 1..8, key: string, value: string }")?;
        let slot_id = payload
            .get("slot_id")
            .and_then(|s| s.as_u64())
            .ok_or("slot_id required")? as u8;
        let key = payload
            .get("key")
            .and_then(|k| k.as_str())
            .ok_or("key required")?
            .to_string();
        let value = payload
            .get("value")
            .and_then(|v| v.as_str())
            .ok_or("value required")?
            .to_string();
        if !(1..=8).contains(&slot_id) {
            return Err("slot_id must be 1–8".into());
        }
        self.store.insert(slot_id, &key, value.as_bytes())?;
        Ok(serde_json::json!({
            "status": "ok",
            "skill": SKILL_NAME,
            "slot_id": slot_id,
            "key": key
        }))
    }
}
