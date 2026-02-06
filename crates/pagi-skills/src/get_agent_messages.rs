//! Get Agent Messages skill: read the current agent's inbox from KB_SOMA.
//!
//! Returns the most recent messages sent to this agent by other agents (e.g. for
//! collaborative workflows or handoffs).

use pagi_core::{AgentSkill, KnowledgeStore, TenantContext};
use serde::Deserialize;
use std::sync::Arc;

const SKILL_NAME: &str = "get_agent_messages";

#[derive(Debug, Deserialize)]
struct GetAgentMessagesArgs {
    /// Maximum number of messages to return (default 10).
    #[serde(default = "default_limit")]
    limit: usize,
}

fn default_limit() -> usize {
    10
}

/// Returns recent messages for the current agent from KB_SOMA inbox.
pub struct GetAgentMessages {
    store: Arc<KnowledgeStore>,
}

impl GetAgentMessages {
    pub fn new(store: Arc<KnowledgeStore>) -> Self {
        Self { store }
    }
}

#[async_trait::async_trait]
impl AgentSkill for GetAgentMessages {
    fn name(&self) -> &str {
        SKILL_NAME
    }

    async fn execute(
        &self,
        ctx: &TenantContext,
        payload: Option<serde_json::Value>,
    ) -> Result<serde_json::Value, Box<dyn std::error::Error + Send + Sync>> {
        let args: GetAgentMessagesArgs = payload
            .and_then(|v| serde_json::from_value(v).ok())
            .unwrap_or(GetAgentMessagesArgs {
                limit: default_limit(),
            });
        let limit = args.limit.max(1).min(100);
        let agent_id = ctx.resolved_agent_id();
        let messages = self.store.get_agent_messages(agent_id, limit)?;
        let list: Vec<serde_json::Value> = messages
            .into_iter()
            .map(|m| {
                serde_json::json!({
                    "id": m.id,
                    "from_agent_id": m.from_agent_id,
                    "target_agent_id": m.target_agent_id,
                    "payload": m.payload,
                    "timestamp_ms": m.timestamp_ms,
                })
            })
            .collect();
        Ok(serde_json::json!({
            "status": "ok",
            "skill": SKILL_NAME,
            "agent_id": agent_id,
            "count": list.len(),
            "messages": list,
        }))
    }
}
