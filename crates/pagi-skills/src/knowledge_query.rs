//! Knowledge Query skill: retrieves values from a KB slot by key.

use pagi_core::{AgentSkill, KnowledgeStore, TenantContext};
use std::sync::Arc;

const SKILL_NAME: &str = "KnowledgeQuery";

/// Retrieves values from the 8-slot knowledge base via slot_id and query_key.
pub struct KnowledgeQuery {
    store: Arc<KnowledgeStore>,
}

impl KnowledgeQuery {
    pub fn new(store: Arc<KnowledgeStore>) -> Self {
        Self { store }
    }
}

#[async_trait::async_trait]
impl AgentSkill for KnowledgeQuery {
    fn name(&self) -> &str {
        SKILL_NAME
    }

    async fn execute(
        &self,
        _ctx: &TenantContext,
        payload: Option<serde_json::Value>,
    ) -> Result<serde_json::Value, Box<dyn std::error::Error + Send + Sync>> {
        let payload = payload.ok_or("KnowledgeQuery requires payload: { slot_id: 1..8, query_key: string }")?;
        let slot_id = payload
            .get("slot_id")
            .and_then(|s| s.as_u64())
            .ok_or("slot_id required")? as u8;
        let query_key = payload
            .get("query_key")
            .and_then(|q| q.as_str())
            .ok_or("query_key required")?
            .to_string();
        if !(1..=8).contains(&slot_id) {
            return Err("slot_id must be 1–8".into());
        }
        let value = self
            .store
            .get(slot_id, &query_key)?
            .and_then(|v| String::from_utf8(v).ok());
        Ok(serde_json::json!({
            "status": "ok",
            "skill": SKILL_NAME,
            "slot_id": slot_id,
            "query_key": query_key,
            "value": value
        }))
    }
}
