//! Draft Response skill: composite task that combines KB-1 (Brand Voice), KB-5 (Community Pulse), and lead data into a mock draft.

use pagi_core::{AgentSkill, KnowledgeStore, MemoryManager, TenantContext};
use std::sync::Arc;

const SKILL_NAME: &str = "DraftResponse";
const BRAND_VOICE_KEY: &str = "brand_voice";
const KB_SLOT_COMMUNITY: u8 = 5;
const CURRENT_PULSE_KEY: &str = "current_pulse";
const LEAD_HISTORY_PREFIX: &str = "lead_history";

/// Formats KB-5 current_pulse JSON into a readable Local Context string.
fn format_local_context(pulse_json: Option<&str>) -> String {
    let Some(json) = pulse_json else {
        return "(none)".to_string();
    };
    let Ok(pulse) = serde_json::from_str::<serde_json::Value>(json) else {
        return json.to_string();
    };
    let loc = pulse.get("location").and_then(|v| v.as_str()).unwrap_or("");
    let trend = pulse.get("trend").and_then(|v| v.as_str()).unwrap_or("");
    let event = pulse.get("event").and_then(|v| v.as_str()).unwrap_or("");
    let parts: Vec<&str> = [loc, trend, event].into_iter().filter(|s| !s.is_empty()).collect();
    if parts.is_empty() {
        "(none)".to_string()
    } else {
        parts.join(". ")
    }
}

/// Combines Brand Voice (KB-1), Community Pulse (KB-5), and a stored lead into a mock response draft.
pub struct DraftResponse {
    memory: Arc<MemoryManager>,
    knowledge: Arc<KnowledgeStore>,
}

impl DraftResponse {
    pub fn new(memory: Arc<MemoryManager>, knowledge: Arc<KnowledgeStore>) -> Self {
        Self { memory, knowledge }
    }
}

#[async_trait::async_trait]
impl AgentSkill for DraftResponse {
    fn name(&self) -> &str {
        SKILL_NAME
    }

    async fn execute(
        &self,
        ctx: &TenantContext,
        payload: Option<serde_json::Value>,
    ) -> Result<serde_json::Value, Box<dyn std::error::Error + Send + Sync>> {
        let lead_id = payload
            .as_ref()
            .and_then(|p| p.get("lead_id"))
            .and_then(|id| id.as_str())
            .ok_or("DraftResponse requires payload: { lead_id: string }")?
            .to_string();

        let path = format!("{}/{}/{}", LEAD_HISTORY_PREFIX, ctx.tenant_id, lead_id);
        let brand_voice = self
            .knowledge
            .get(1, BRAND_VOICE_KEY)?
            .and_then(|v| String::from_utf8(v).ok())
            .unwrap_or_else(|| "Friendly and professional".to_string());

        let current_pulse_raw = self
            .knowledge
            .get(KB_SLOT_COMMUNITY, CURRENT_PULSE_KEY)?
            .and_then(|v| String::from_utf8(v).ok());
        let local_context = format_local_context(current_pulse_raw.as_deref());

        let lead_data = self
            .memory
            .get_path(ctx, &path)?
            .and_then(|v| String::from_utf8(v).ok())
            .unwrap_or_else(|| "{}".to_string());

        let draft = format!(
            "[Mock Draft – precursor to LLM]\n\nBrand Voice: {}\n\nLocal Context: {}\n\nLead data: {}\n\n---\nDraft: Thank you for reaching out. We will respond shortly.",
            brand_voice, local_context, lead_data
        );

        Ok(serde_json::json!({
            "status": "ok",
            "skill": SKILL_NAME,
            "lead_id": lead_id,
            "draft": draft
        }))
    }
}
