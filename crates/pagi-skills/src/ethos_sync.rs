//! **EthosSync Skill** — Switches SAGE_BOT's philosophical personality lens.
//!
//! Allows the user to choose a philosophical school (e.g. "Stoic", "Growth-Mindset",
//! "Compassionate-Witness", "Taoist", "Existentialist") that governs how the AGI
//! reframes stressors, advises on conflicts, and modulates its advisory tone.
//!
//! The policy is stored in **KB_ETHOS** (Slot 6) under key `ethos/current` as an
//! [`EthosPolicy`] struct. Other skills (notably `ReflectShadow`) read this policy
//! to inject school-specific system instructions into the LLM prompt.
//!
//! ## Preset Schools
//!
//! | School                  | Core Approach                                      |
//! |-------------------------|----------------------------------------------------|
//! | `Stoic`                 | Dichotomy of Control — focus on your reaction       |
//! | `Growth-Mindset`        | Challenges as learning opportunities                |
//! | `Compassionate-Witness` | Non-judgmental observation; NVC principles           |
//! | `Taoist`                | Wu-wei — path of least resistance                   |
//! | `Existentialist`        | Radical freedom — create your own meaning            |
//!
//! Custom schools are supported by providing `core_maxims` directly.

use pagi_core::{AgentSkill, EthosPolicy, EventRecord, KnowledgeStore, TenantContext};
use serde::Deserialize;
use std::sync::Arc;

const SKILL_NAME: &str = "EthosSync";

/// Input payload for the EthosSync skill.
#[derive(Debug, Deserialize)]
struct EthosSyncArgs {
    /// Philosophical school to activate (e.g. "Stoic", "Growth-Mindset").
    /// If a preset is recognized, its default maxims are used unless `core_maxims` is provided.
    active_school: String,

    /// Optional override: specific maxims/principles for the LLM to apply.
    /// If empty and the school is a known preset, the preset maxims are used.
    #[serde(default)]
    core_maxims: Vec<String>,

    /// Optional tone weight override (0.0–1.0). Defaults to 0.8 if not provided.
    #[serde(default)]
    tone_weight: Option<f32>,
}

pub struct EthosSync {
    store: Arc<KnowledgeStore>,
}

impl EthosSync {
    pub fn new(store: Arc<KnowledgeStore>) -> Self {
        Self { store }
    }
}

#[async_trait::async_trait]
impl AgentSkill for EthosSync {
    fn name(&self) -> &str {
        SKILL_NAME
    }

    async fn execute(
        &self,
        ctx: &TenantContext,
        payload: Option<serde_json::Value>,
    ) -> Result<serde_json::Value, Box<dyn std::error::Error + Send + Sync>> {
        let payload = payload.ok_or("EthosSync requires payload: { active_school, [core_maxims], [tone_weight] }")?;
        let args: EthosSyncArgs = serde_json::from_value(payload)?;

        if args.active_school.trim().is_empty() {
            return Err("EthosSync requires non-empty active_school".into());
        }

        // Build the policy: try preset first, then allow custom.
        let mut policy = if args.core_maxims.is_empty() {
            // Try to load a preset; if not recognized, create a custom policy with no maxims.
            EthosPolicy::preset(&args.active_school).unwrap_or(EthosPolicy {
                active_school: args.active_school.clone(),
                core_maxims: Vec::new(),
                tone_weight: 0.8,
            })
        } else {
            EthosPolicy {
                active_school: args.active_school.clone(),
                core_maxims: args.core_maxims,
                tone_weight: 0.8,
            }
        };

        // Apply tone_weight override if provided.
        if let Some(tw) = args.tone_weight {
            policy.tone_weight = tw;
        }
        policy.clamp();

        // Persist to KB_ETHOS under `ethos/current`.
        self.store.set_ethos_philosophical_policy(&policy)?;

        let agent_id = ctx.resolved_agent_id();

        // Chronos: log the school switch.
        let event = EventRecord::now(
            "Chronos",
            format!(
                "Ethos philosophical lens switched to '{}' (tone_weight={:.1}, maxims={}).",
                policy.active_school,
                policy.tone_weight,
                policy.core_maxims.len(),
            ),
        )
        .with_skill(SKILL_NAME)
        .with_outcome("ethos_switch");
        let _ = self.store.append_chronos_event(agent_id, &event);

        let system_instruction = policy.to_system_instruction();

        Ok(serde_json::json!({
            "status": "ok",
            "skill": SKILL_NAME,
            "slot_id": 6,
            "ethos_policy": {
                "active_school": policy.active_school,
                "core_maxims": policy.core_maxims,
                "tone_weight": policy.tone_weight,
            },
            "system_instruction_preview": system_instruction,
            "chronos_logged": true,
            "message": format!(
                "Philosophical lens set to '{}'. SAGE_BOT will now use {} principles when reframing stressors and advising on conflicts.",
                policy.active_school,
                policy.active_school,
            ),
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pagi_core::KnowledgeStore;

    #[tokio::test]
    async fn ethos_sync_sets_stoic_preset() {
        let kb_dir = tempfile::tempdir().unwrap();
        let knowledge = Arc::new(KnowledgeStore::open_path(kb_dir.path()).unwrap());
        let skill = EthosSync::new(Arc::clone(&knowledge));

        let ctx = TenantContext {
            tenant_id: "test".to_string(),
            correlation_id: None,
            agent_id: Some("default".to_string()),
        };
        let payload = serde_json::json!({
            "active_school": "Stoic",
        });

        let result = skill.execute(&ctx, Some(payload)).await.unwrap();

        assert_eq!(result["status"], "ok");
        assert_eq!(result["skill"], SKILL_NAME);
        assert_eq!(result["ethos_policy"]["active_school"], "Stoic");
        let tw = result["ethos_policy"]["tone_weight"].as_f64().unwrap();
        assert!((tw - 0.8).abs() < 0.01, "tone_weight should be ~0.8, got {}", tw);
        assert!(result["chronos_logged"].as_bool().unwrap());

        // Verify persisted in store.
        let stored = knowledge.get_ethos_philosophical_policy().unwrap();
        assert_eq!(stored.active_school, "Stoic");
        assert!(!stored.core_maxims.is_empty());
        assert!(stored.core_maxims[0].contains("control"));

        // Verify system instruction contains Stoic reference.
        let instruction = stored.to_system_instruction();
        assert!(instruction.contains("Stoic"), "Instruction should mention Stoic: {}", instruction);
    }

    #[tokio::test]
    async fn ethos_sync_sets_growth_mindset() {
        let kb_dir = tempfile::tempdir().unwrap();
        let knowledge = Arc::new(KnowledgeStore::open_path(kb_dir.path()).unwrap());
        let skill = EthosSync::new(Arc::clone(&knowledge));

        let ctx = TenantContext {
            tenant_id: "test".to_string(),
            correlation_id: None,
            agent_id: Some("default".to_string()),
        };
        let payload = serde_json::json!({
            "active_school": "Growth-Mindset",
        });

        let result = skill.execute(&ctx, Some(payload)).await.unwrap();

        assert_eq!(result["ethos_policy"]["active_school"], "Growth-Mindset");
        let stored = knowledge.get_ethos_philosophical_policy().unwrap();
        assert_eq!(stored.active_school, "Growth-Mindset");
        assert!(stored.core_maxims.iter().any(|m| m.contains("growth")));
    }

    #[tokio::test]
    async fn ethos_sync_custom_school_with_maxims() {
        let kb_dir = tempfile::tempdir().unwrap();
        let knowledge = Arc::new(KnowledgeStore::open_path(kb_dir.path()).unwrap());
        let skill = EthosSync::new(Arc::clone(&knowledge));

        let ctx = TenantContext {
            tenant_id: "test".to_string(),
            correlation_id: None,
            agent_id: Some("default".to_string()),
        };
        let payload = serde_json::json!({
            "active_school": "Absurdist",
            "core_maxims": ["Embrace the absurd; find joy in the meaningless.", "Revolt, freedom, passion."],
            "tone_weight": 0.6,
        });

        let result = skill.execute(&ctx, Some(payload)).await.unwrap();

        assert_eq!(result["ethos_policy"]["active_school"], "Absurdist");
        let tw = result["ethos_policy"]["tone_weight"].as_f64().unwrap();
        assert!((tw - 0.6).abs() < 0.01, "tone_weight should be ~0.6, got {}", tw);
        let stored = knowledge.get_ethos_philosophical_policy().unwrap();
        assert_eq!(stored.active_school, "Absurdist");
        assert_eq!(stored.core_maxims.len(), 2);
        assert!((stored.tone_weight - 0.6).abs() < 0.01);
    }

    #[tokio::test]
    async fn ethos_sync_rejects_empty_school() {
        let kb_dir = tempfile::tempdir().unwrap();
        let knowledge = Arc::new(KnowledgeStore::open_path(kb_dir.path()).unwrap());
        let skill = EthosSync::new(Arc::clone(&knowledge));

        let ctx = TenantContext {
            tenant_id: "test".to_string(),
            correlation_id: None,
            agent_id: Some("default".to_string()),
        };
        let payload = serde_json::json!({
            "active_school": "",
        });

        let result = skill.execute(&ctx, Some(payload)).await;
        assert!(result.is_err());
    }
}
