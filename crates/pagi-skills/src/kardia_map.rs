//! **KardiaMap Skill** — Relational Map (Kardia) upsert.
//!
//! Lets SAGE_BOT maintain a map of people in the user's life: relationship, trust score,
//! attachment style, and triggers. Stored in **Slot 7 (Kardia)** under `people/{name_slug}`.
//! ReflectShadow uses this to inject relationship context when reflecting on journal entries
//! that mention a mapped person.

use pagi_core::{AgentSkill, KnowledgeStore, PersonRecord, TenantContext};
use serde::Deserialize;
use std::sync::Arc;

const SKILL_NAME: &str = "KardiaMap";

#[derive(Debug, Deserialize)]
struct KardiaMapArgs {
    /// Person's display name (e.g. "Project Manager", "Sarah").
    name: String,
    /// Relationship role (e.g. "Boss", "Partner", "Mother").
    #[serde(default)]
    relationship: String,
    /// Trust level 0.0–1.0. Optional; preserved on upsert if omitted.
    #[serde(default)]
    trust_score: Option<f32>,
    /// Attachment style (e.g. "Avoidant", "Anxious", "Secure"). Optional.
    #[serde(default)]
    attachment_style: Option<String>,
    /// Known triggers (e.g. "criticism", "silent treatment"). Optional.
    #[serde(default)]
    triggers: Option<Vec<String>>,
    /// Summary of a recent interaction; stored as last_interaction_summary.
    #[serde(default)]
    interaction_summary: Option<String>,
}

pub struct KardiaMap {
    store: Arc<KnowledgeStore>,
}

impl KardiaMap {
    pub fn new(store: Arc<KnowledgeStore>) -> Self {
        Self { store }
    }
}

#[async_trait::async_trait]
impl AgentSkill for KardiaMap {
    fn name(&self) -> &str {
        SKILL_NAME
    }

    async fn execute(
        &self,
        _ctx: &TenantContext,
        payload: Option<serde_json::Value>,
    ) -> Result<serde_json::Value, Box<dyn std::error::Error + Send + Sync>> {
        let payload = payload.ok_or("KardiaMap requires payload: { name, relationship?, trust_score?, attachment_style?, triggers?, interaction_summary? }")?;
        let args: KardiaMapArgs = serde_json::from_value(payload)?;

        if args.name.trim().is_empty() {
            return Err("KardiaMap requires non-empty name".into());
        }

        let slug = PersonRecord::name_slug(&args.name);
        let existing = self.store.get_person(&slug);

        let mut record = existing.unwrap_or_else(|| PersonRecord {
            name: args.name.trim().to_string(),
            relationship: args.relationship.trim().to_string(),
            trust_score: 0.5,
            attachment_style: String::new(),
            triggers: Vec::new(),
            last_interaction_summary: None,
        });

        // Overwrite name/relationship if provided (for new or update)
        if !args.relationship.is_empty() {
            record.relationship = args.relationship.trim().to_string();
        }
        if let Some(score) = args.trust_score {
            record.trust_score = score.clamp(0.0, 1.0);
        }
        if let Some(style) = args.attachment_style {
            if !style.is_empty() {
                record.attachment_style = style.trim().to_string();
            }
        }
        if let Some(triggers) = args.triggers {
            record.triggers = triggers.into_iter().map(|t| t.trim().to_string()).filter(|t| !t.is_empty()).collect();
        }
        if let Some(summary) = args.interaction_summary {
            if !summary.trim().is_empty() {
                record.last_interaction_summary = Some(summary.trim().to_string());
            }
        }

        record.clamp();
        self.store.set_person(&record)?;

        Ok(serde_json::json!({
            "status": "ok",
            "skill": SKILL_NAME,
            "slot_id": 7,
            "name": record.name,
            "name_slug": slug,
            "relationship": record.relationship,
            "trust_score": record.trust_score,
            "attachment_style": record.attachment_style,
            "triggers": record.triggers,
            "message": format!("Upserted '{}' into Relational Map (Kardia).", record.name)
        }))
    }
}
