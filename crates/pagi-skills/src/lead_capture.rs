//! Lead Capture skill: persists customer inquiry payloads under the tenant's Lead History path.

use pagi_core::{AgentSkill, MemoryManager, TenantContext};
use std::sync::Arc;
use uuid::Uuid;

const SKILL_NAME: &str = "LeadCapture";
const LEAD_HISTORY_PREFIX: &str = "lead_history";

/// Saves customer inquiry payloads to the tenant's Lead History in pagi-memory.
pub struct LeadCapture {
    memory: Arc<MemoryManager>,
}

impl LeadCapture {
    pub fn new(memory: Arc<MemoryManager>) -> Self {
        Self { memory }
    }
}

#[async_trait::async_trait]
impl AgentSkill for LeadCapture {
    fn name(&self) -> &str {
        SKILL_NAME
    }

    async fn execute(
        &self,
        ctx: &TenantContext,
        payload: Option<serde_json::Value>,
    ) -> Result<serde_json::Value, Box<dyn std::error::Error + Send + Sync>> {
        let payload = payload.ok_or("LeadCapture requires a JSON payload (customer inquiry)")?;
        let lead_id = Uuid::new_v4().to_string();
        let path = format!("{}/{}/{}", LEAD_HISTORY_PREFIX, ctx.tenant_id, lead_id);
        let bytes = serde_json::to_vec(&payload)?;
        self.memory.save_path(ctx, &path, &bytes)?;
        Ok(serde_json::json!({
            "status": "saved",
            "skill": SKILL_NAME,
            "lead_id": lead_id,
            "path": path
        }))
    }
}
