//! Check Alignment skill: consults **KB_ETHOS** to return pass/fail for an intended action.
//!
//! Allows the Agent or external systems to ask "Is running skill X with this payload
//! aligned with current safety protocols?" without executing the skill.

use pagi_core::{AgentSkill, AlignmentResult, KnowledgeStore, TenantContext};
use serde::Deserialize;
use std::sync::Arc;

const SKILL_NAME: &str = "check_alignment";

#[derive(Debug, Deserialize)]
struct CheckAlignmentArgs {
    /// Skill or action name to check.
    skill_name: String,
    /// Content to scan for sensitive keywords (e.g. payload content or summary).
    #[serde(default)]
    content: String,
}

/// Consults KB_ETHOS and returns whether the intended action is allowed.
pub struct CheckAlignment {
    store: Arc<KnowledgeStore>,
}

impl CheckAlignment {
    pub fn new(store: Arc<KnowledgeStore>) -> Self {
        Self { store }
    }
}

#[async_trait::async_trait]
impl AgentSkill for CheckAlignment {
    fn name(&self) -> &str {
        SKILL_NAME
    }

    async fn execute(
        &self,
        _ctx: &TenantContext,
        payload: Option<serde_json::Value>,
    ) -> Result<serde_json::Value, Box<dyn std::error::Error + Send + Sync>> {
        let payload = payload.ok_or("check_alignment requires payload: { skill_name, content? }")?;
        let args: CheckAlignmentArgs = serde_json::from_value(payload)?;
        let content = if args.content.is_empty() {
            "".to_string()
        } else {
            args.content
        };
        let policy = self.store.get_ethos_policy();
        let result = match policy {
            None => AlignmentResult::Pass,
            Some(p) => p.allows(&args.skill_name, &content),
        };
        let (pass, reason) = match &result {
            AlignmentResult::Pass => (true, serde_json::Value::Null),
            AlignmentResult::Fail { reason } => (false, serde_json::Value::String(reason.clone())),
        };
        Ok(serde_json::json!({
            "status": "ok",
            "skill": SKILL_NAME,
            "pass": pass,
            "reason": reason,
        }))
    }
}
