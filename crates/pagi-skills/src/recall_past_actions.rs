//! Recall Past Actions skill: queries **KB_CHRONOS** for the last N things the Agent did.
//!
//! Enables the Agent to answer "What did you do five minutes ago?" by consulting
//! episodic memory rather than guessing.

use pagi_core::{AgentSkill, KnowledgeStore, TenantContext};
use serde::Deserialize;
use std::sync::Arc;

const SKILL_NAME: &str = "recall_past_actions";

#[derive(Debug, Deserialize)]
struct RecallArgs {
    /// Maximum number of recent events to return (default 5).
    #[serde(default = "default_limit")]
    limit: usize,
}

fn default_limit() -> usize {
    5
}

/// Returns the most recent episodic events from KB_CHRONOS (newest first).
pub struct RecallPastActions {
    store: Arc<KnowledgeStore>,
}

impl RecallPastActions {
    pub fn new(store: Arc<KnowledgeStore>) -> Self {
        Self { store }
    }
}

#[async_trait::async_trait]
impl AgentSkill for RecallPastActions {
    fn name(&self) -> &str {
        SKILL_NAME
    }

    async fn execute(
        &self,
        ctx: &TenantContext,
        payload: Option<serde_json::Value>,
    ) -> Result<serde_json::Value, Box<dyn std::error::Error + Send + Sync>> {
        let args: RecallArgs = payload
            .and_then(|v| serde_json::from_value(v).ok())
            .unwrap_or(RecallArgs { limit: default_limit() });
        let limit = args.limit.max(1).min(50);
        let agent_id = ctx.resolved_agent_id();
        let events = self.store.get_recent_chronos_events(agent_id, limit)?;
        let list: Vec<serde_json::Value> = events
            .into_iter()
            .map(|e| {
                serde_json::json!({
                    "timestamp_ms": e.timestamp_ms,
                    "source_kb": e.source_kb,
                    "skill_name": e.skill_name,
                    "reflection": e.reflection,
                    "outcome": e.outcome,
                })
            })
            .collect();
        Ok(serde_json::json!({
            "status": "ok",
            "skill": SKILL_NAME,
            "count": list.len(),
            "events": list,
        }))
    }
}
