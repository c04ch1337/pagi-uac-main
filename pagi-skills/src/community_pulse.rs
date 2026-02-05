//! Community Pulse skill: stores local neighborhood trends and events into KB-5 (Community).

use pagi_knowledge::KnowledgeStore;
use pagi_orchestrator::AgentSkill;
use pagi_shared::TenantContext;
use std::sync::Arc;

const SKILL_NAME: &str = "CommunityPulse";
const KB_SLOT_COMMUNITY: u8 = 5;
const CURRENT_PULSE_KEY: &str = "current_pulse";

/// Stores contextual pulse (location, trend, event) into KB-5 for use in drafts.
pub struct CommunityPulse {
    knowledge: Arc<KnowledgeStore>,
}

impl CommunityPulse {
    pub fn new(knowledge: Arc<KnowledgeStore>) -> Self {
        Self { knowledge }
    }
}

#[async_trait::async_trait]
impl AgentSkill for CommunityPulse {
    fn name(&self) -> &str {
        SKILL_NAME
    }

    async fn execute(
        &self,
        _ctx: &TenantContext,
        payload: Option<serde_json::Value>,
    ) -> Result<serde_json::Value, Box<dyn std::error::Error + Send + Sync>> {
        let payload = payload.ok_or("CommunityPulse requires payload: { location: string, trend: string, event: string }")?;
        let location = payload
            .get("location")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let trend = payload
            .get("trend")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let event = payload
            .get("event")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        let pulse = serde_json::json!({
            "location": location,
            "trend": trend,
            "event": event
        });
        let value = pulse.to_string();
        self.knowledge
            .insert(KB_SLOT_COMMUNITY, CURRENT_PULSE_KEY, value.as_bytes())?;

        Ok(serde_json::json!({
            "status": "ok",
            "skill": SKILL_NAME,
            "slot_id": KB_SLOT_COMMUNITY,
            "key": CURRENT_PULSE_KEY,
            "location": location,
            "trend": trend,
            "event": event
        }))
    }
}
