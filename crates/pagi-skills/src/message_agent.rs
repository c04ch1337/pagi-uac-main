//! Message Agent skill: send a JSON payload to another agent's inbox (KB_SOMA).
//!
//! Enables inter-agent communication for multi-agent workflows. The sender is
//! the current agent (from TenantContext); the target is specified in the payload.

use pagi_core::{AgentSkill, KnowledgeStore, TenantContext};
use serde::Deserialize;
use std::sync::Arc;

const SKILL_NAME: &str = "message_agent";

#[derive(Debug, Deserialize)]
struct MessageAgentArgs {
    /// Target agent id (e.g. "auditor", "developer").
    target_agent_id: String,
    /// JSON payload to deliver (object, string, or array).
    message: serde_json::Value,
}

/// Sends a message to another agent's inbox in KB_SOMA.
pub struct MessageAgent {
    store: Arc<KnowledgeStore>,
}

impl MessageAgent {
    pub fn new(store: Arc<KnowledgeStore>) -> Self {
        Self { store }
    }
}

#[async_trait::async_trait]
impl AgentSkill for MessageAgent {
    fn name(&self) -> &str {
        SKILL_NAME
    }

    async fn execute(
        &self,
        ctx: &TenantContext,
        payload: Option<serde_json::Value>,
    ) -> Result<serde_json::Value, Box<dyn std::error::Error + Send + Sync>> {
        let args: MessageAgentArgs = payload
            .and_then(|v| serde_json::from_value(v).ok())
            .ok_or("message_agent requires { target_agent_id, message }")?;
        let target = args.target_agent_id.trim();
        if target.is_empty() {
            return Err("target_agent_id is required".into());
        }
        let from_id = ctx.resolved_agent_id();
        let message_id = self
            .store
            .push_agent_message(from_id, target, &args.message)?;
        Ok(serde_json::json!({
            "status": "ok",
            "skill": SKILL_NAME,
            "message_id": message_id,
            "from_agent_id": from_id,
            "target_agent_id": target,
        }))
    }
}
