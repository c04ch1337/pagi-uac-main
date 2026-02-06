//! Analyze Sentiment skill: updates **KB_KARDIA** with relationship state from recent user messages.
//!
//! Takes the last N user messages, infers sentiment and communication style,
//! and stores/updates a RelationRecord so the agent can adapt its voice (Pneuma) to the user (Kardia).

use pagi_core::{AgentSkill, KnowledgeStore, RelationRecord, TenantContext};
use serde::Deserialize;
use std::sync::Arc;

const SKILL_NAME: &str = "analyze_sentiment";

#[derive(Debug, Deserialize)]
struct AnalyzeSentimentArgs {
    /// User or tenant identifier.
    user_id: String,
    /// Last N user messages (newest last). Used to infer sentiment and style.
    messages: Vec<String>,
}

/// Infers sentiment from message text (keyword-based; can be replaced with LLM in live mode).
fn infer_sentiment(messages: &[String]) -> String {
    let combined = messages.join(" ").to_lowercase();
    if combined.contains("angry") || combined.contains("furious") || combined.contains("terrible") {
        return "angry".to_string();
    }
    if combined.contains("frustrated") || combined.contains("annoyed") || combined.contains("disappointed") {
        return "frustrated".to_string();
    }
    if combined.contains("urgent") || combined.contains("asap") || combined.contains("immediately") {
        return "urgent".to_string();
    }
    if combined.contains("thanks") || combined.contains("great") || combined.contains("helpful") {
        return "positive".to_string();
    }
    if combined.contains("please") && combined.len() > 20 {
        return "polite".to_string();
    }
    "neutral".to_string()
}

/// Infers communication style from message text.
fn infer_communication_style(messages: &[String]) -> String {
    let combined = messages.join(" ").to_lowercase();
    if combined.contains("!") && combined.matches('!').count() >= 2 {
        return "emphatic".to_string();
    }
    if combined.contains("asap") || combined.contains("urgent") || combined.contains("immediately") {
        return "urgent".to_string();
    }
    if combined.len() > 200 && combined.contains("?") {
        return "detailed".to_string();
    }
    if combined.contains("hey") || combined.contains("hi ") || combined.contains("thanks") {
        return "casual".to_string();
    }
    "formal".to_string()
}

/// Updates KB_KARDIA with relationship state derived from recent user messages.
pub struct AnalyzeSentiment {
    store: Arc<KnowledgeStore>,
}

impl AnalyzeSentiment {
    pub fn new(store: Arc<KnowledgeStore>) -> Self {
        Self { store }
    }
}

#[async_trait::async_trait]
impl AgentSkill for AnalyzeSentiment {
    fn name(&self) -> &str {
        SKILL_NAME
    }

    async fn execute(
        &self,
        ctx: &TenantContext,
        payload: Option<serde_json::Value>,
    ) -> Result<serde_json::Value, Box<dyn std::error::Error + Send + Sync>> {
        let payload = payload.ok_or("analyze_sentiment requires payload: { user_id, messages }")?;
        let args: AnalyzeSentimentArgs = serde_json::from_value(payload)?;
        let messages: Vec<String> = args.messages.into_iter().take(10).collect();
        let sentiment = infer_sentiment(&messages);
        let style = infer_communication_style(&messages);
        let owner_agent_id = ctx.resolved_agent_id();

        let mut record = self
            .store
            .get_kardia_relation(owner_agent_id, &args.user_id)
            .unwrap_or_else(|| RelationRecord::new(&args.user_id));
        record = record.with_sentiment(&sentiment).with_communication_style(&style);
        self.store.set_kardia_relation(owner_agent_id, &record)?;

        Ok(serde_json::json!({
            "status": "ok",
            "skill": SKILL_NAME,
            "user_id": args.user_id,
            "last_sentiment": sentiment,
            "communication_style": style,
            "trust_score": record.trust_score,
        }))
    }
}
