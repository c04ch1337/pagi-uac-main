//! Sales Closer skill: enriches a draft with a call-to-action from KB-2 (Sales) closing strategy.

use pagi_knowledge::KnowledgeStore;
use pagi_orchestrator::AgentSkill;
use pagi_shared::TenantContext;
use std::sync::Arc;

const SKILL_NAME: &str = "SalesCloser";
const KB_SLOT_SALES: u8 = 2;
const CLOSING_STRATEGY_KEY: &str = "closing_strategy";

/// Combines the current draft with a CTA from KB-2 (closing_strategy) for conversion-focused responses.
pub struct SalesCloser {
    knowledge: Arc<KnowledgeStore>,
}

impl SalesCloser {
    pub fn new(knowledge: Arc<KnowledgeStore>) -> Self {
        Self { knowledge }
    }
}

#[async_trait::async_trait]
impl AgentSkill for SalesCloser {
    fn name(&self) -> &str {
        SKILL_NAME
    }

    async fn execute(
        &self,
        _ctx: &TenantContext,
        payload: Option<serde_json::Value>,
    ) -> Result<serde_json::Value, Box<dyn std::error::Error + Send + Sync>> {
        let payload = payload.ok_or("SalesCloser requires payload: { draft: string }")?;
        let draft = payload
            .get("draft")
            .and_then(|v| v.as_str())
            .ok_or("draft required")?
            .to_string();

        let cta = self
            .knowledge
            .get(KB_SLOT_SALES, CLOSING_STRATEGY_KEY)
            .ok()
            .flatten()
            .and_then(|b| String::from_utf8(b).ok())
            .unwrap_or_else(|| "We'd love to schedule a quick call at your convenience.".to_string());

        let enriched = format!(
            "{}\n\nCall to action: {}",
            draft.trim_end(),
            cta.trim()
        );

        Ok(serde_json::json!({
            "status": "ok",
            "skill": SKILL_NAME,
            "draft": enriched,
            "closing_strategy_used": cta
        }))
    }
}
