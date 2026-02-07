//! Journal skill: extracts "Emotional Anchors" from raw text and updates MentalState (Cognitive Governor).
//! Raw journal text is never logged or sent to external APIs; only anonymized labels and score deltas are used.

use pagi_core::{AgentSkill, KnowledgeStore, MentalState, TenantContext};
use serde::Deserialize;
use std::sync::Arc;

const SKILL_NAME: &str = "JournalSkill";

#[derive(Debug, Deserialize)]
struct JournalSkillArgs {
    /// Raw journal text. Never logged or sent to any API; only used locally for pattern extraction.
    raw_text: String,
    /// Reserved: store encrypted in ShadowStore (future use when gateway has ShadowStore).
    #[serde(default)]
    #[allow(dead_code)]
    store_encrypted: bool,
}

/// Anonymized emotional anchor labels (never include names or raw content).
/// Exported for use by DeepJournalSkill (encrypt-then-extract flow).
pub fn extract_anchors(text: &str) -> Vec<(String, f32)> {
    let lower = text.to_lowercase();
    let mut anchors = Vec::new();
    // High stress / conflict
    if lower.contains("conflict") || lower.contains("argument") || lower.contains("fight") {
        anchors.push(("conflict".to_string(), 0.4));
    }
    if lower.contains("overwhelmed") || lower.contains("can't cope") || lower.contains("drowning") {
        anchors.push(("overwhelm".to_string(), 0.5));
    }
    if lower.contains("exhausted") || lower.contains("burnout") || lower.contains("burned out") {
        anchors.push(("exhaustion".to_string(), 0.5));
    }
    if lower.contains("deadline") && (lower.contains("stress") || lower.contains("panic") || lower.contains("too much")) {
        anchors.push(("deadline_pressure".to_string(), 0.35));
    }
    if lower.contains("grief") || lower.contains("loss") || lower.contains("mourning") {
        anchors.push(("grief".to_string(), 0.6));
    }
    if lower.contains("anxious") || lower.contains("anxiety") || lower.contains("worried") {
        anchors.push(("anxiety".to_string(), 0.35));
    }
    if lower.contains("angry") || lower.contains("furious") || lower.contains("resent") {
        anchors.push(("anger".to_string(), 0.4));
    }
    if lower.contains("sad") || lower.contains("depressed") || lower.contains("hopeless") {
        anchors.push(("low_mood".to_string(), 0.4));
    }
    // Slightly positive / relief (reduce stress)
    if lower.contains("relief") || lower.contains("resolved") || lower.contains("better now") {
        anchors.push(("relief".to_string(), -0.15));
    }
    anchors
}

/// Compute MentalState deltas from anchors (no raw text involved).
/// Exported for use by DeepJournalSkill.
pub fn apply_anchors_to_state(current: &MentalState, anchors: &[(String, f32)]) -> MentalState {
    let mut stress = current.relational_stress;
    let mut burnout = current.burnout_risk;
    let mut grace = current.grace_multiplier;
    for (_label, intensity) in anchors {
        if *intensity > 0.0 {
            stress = (stress + intensity * 0.15).min(1.0);
            burnout = (burnout + intensity * 0.1).min(1.0);
            grace = (grace - 0.05).max(0.2);
        } else {
            stress = (stress + intensity).max(0.0);
            grace = (grace + 0.05).min(2.0);
        }
    }
    let mut next = MentalState {
        relational_stress: stress,
        burnout_risk: burnout,
        grace_multiplier: grace,
    };
    next.clamp();
    next
}

pub struct JournalSkill {
    store: Arc<KnowledgeStore>,
}

impl JournalSkill {
    pub fn new(store: Arc<KnowledgeStore>) -> Self {
        Self { store }
    }
}

#[async_trait::async_trait]
impl AgentSkill for JournalSkill {
    fn name(&self) -> &str {
        SKILL_NAME
    }

    async fn execute(
        &self,
        ctx: &TenantContext,
        payload: Option<serde_json::Value>,
    ) -> Result<serde_json::Value, Box<dyn std::error::Error + Send + Sync>> {
        let payload = payload.ok_or("JournalSkill requires payload: { raw_text }")?;
        let args: JournalSkillArgs = serde_json::from_value(payload)?;
        let raw_text = args.raw_text;
        let agent_id = ctx.resolved_agent_id();

        let anchors = extract_anchors(&raw_text);
        let anchor_labels: Vec<&str> = anchors.iter().map(|(l, _)| l.as_str()).collect();
        // Log only anonymized labels; never log raw_text.
        if !anchor_labels.is_empty() {
            tracing::info!(
                target: "pagi::journal_skill",
                "JournalSkill: extracted {} anonymized anchor(s)",
                anchor_labels.len()
            );
        }

        let current = self.store.get_mental_state(agent_id);
        let next = apply_anchors_to_state(&current, &anchors);
        self.store.set_mental_state(agent_id, &next)?;

        Ok(serde_json::json!({
            "status": "ok",
            "skill": SKILL_NAME,
            "anchors_extracted": anchors.len(),
            "mental_state_updated": true,
            "relational_stress": next.relational_stress,
            "burnout_risk": next.burnout_risk,
            "grace_multiplier": next.grace_multiplier,
        }))
    }
}
