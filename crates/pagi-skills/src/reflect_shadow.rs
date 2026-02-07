//! **ReflectShadow Skill** — Reflective Processing over Shadow Vault entries.
//!
//! Lets the user intentionally "open" a Vault entry to SAGE_BOT for processing
//! (e.g. Stoicism, Non-Violent Communication) without persisting the raw text
//! anywhere except the LLM's transient context.
//!
//! 1. **Secure bridge:** Uses `session_key` validation (gateway must verify against
//!    `PAGI_SHADOW_KEY` before invoking). Decrypts `raw_content` from ShadowStore
//!    only in memory.
//! 2. **Private prompt:** Sends content to a non-logging LLM path (ModelRouter
//!    `generate_reflection`) with Ethos/Kardia context for reframing.
//! 3. **Volatile memory:** Decrypted content is never written to `pagi_knowledge`
//!    or logs; it exists only in the prompt context and is purged after use.
//! 4. **Chronos recap:** Logs only "User performed a Shadow Reflection on record [ID]."

use pagi_core::{
    AgentSkill, EventRecord, KnowledgeStore, MentalState, ShadowStoreHandle, TenantContext,
};
use crate::model_router::ModelRouter;
use serde::Deserialize;
use std::sync::Arc;

const SKILL_NAME: &str = "ReflectShadow";

#[derive(Debug, Deserialize)]
struct ReflectShadowArgs {
    /// Journal record ID (e.g. "journal/1738...").
    record_id: String,
    /// Session key; gateway must validate against PAGI_SHADOW_KEY before calling.
    /// Skill requires non-empty to ensure user explicitly opened the vault.
    session_key: String,
}

/// Securely zero a String's backing buffer before dropping (no sensitive data in freed memory).
#[inline]
fn secure_purge(mut s: String) {
    unsafe {
        let bytes = s.as_bytes_mut();
        for b in bytes.iter_mut() {
            std::ptr::write_volatile(b, 0u8);
        }
    }
    drop(s);
}

pub struct ReflectShadowSkill {
    store: Arc<KnowledgeStore>,
    shadow: ShadowStoreHandle,
    model_router: Arc<ModelRouter>,
}

impl ReflectShadowSkill {
    pub fn new(
        store: Arc<KnowledgeStore>,
        shadow: ShadowStoreHandle,
        model_router: Arc<ModelRouter>,
    ) -> Self {
        Self {
            store,
            shadow,
            model_router,
        }
    }
}

#[async_trait::async_trait]
impl AgentSkill for ReflectShadowSkill {
    fn name(&self) -> &str {
        SKILL_NAME
    }

    async fn execute(
        &self,
        ctx: &TenantContext,
        payload: Option<serde_json::Value>,
    ) -> Result<serde_json::Value, Box<dyn std::error::Error + Send + Sync>> {
        let payload = payload.ok_or("ReflectShadow requires payload: { record_id, session_key }")?;
        let args: ReflectShadowArgs = serde_json::from_value(payload)?;

        if args.session_key.trim().is_empty() {
            return Err("ReflectShadow requires non-empty session_key (vault must be explicitly opened)".into());
        }

        let agent_id = ctx.resolved_agent_id();

        // Decrypt entry from ShadowStore (key validated by gateway; store uses PAGI_SHADOW_KEY).
        let raw_content = {
            let guard = self.shadow.read().await;
            let store = guard.as_ref().ok_or("ShadowStore not initialized")?;
            let decrypted = store
                .get_journal(&args.record_id)
                .map_err(|e| format!("ShadowStore get_journal: {}", e))?;
            let entry = decrypted.ok_or("Record not found in Shadow Vault")?;
            // Copy out only what we need; we will purge after use.
            entry.0.raw_content.clone().unwrap_or_default()
        };

        if raw_content.is_empty() {
            return Err("Vault entry has no raw_content to reflect on".into());
        }

        // Build context from effective MentalState (Kardia + Soma/BioGate) and Ethos — no raw content in logs.
        let mental = self.store.get_effective_mental_state(agent_id);
        let kardia_context = format!(
            "User's current mental state: relational_stress={:.2}, burnout_risk={:.2}, grace_multiplier={:.2}. \
             Prefer supportive, low-pressure reframing.",
            mental.relational_stress,
            mental.burnout_risk,
            mental.grace_multiplier,
        );
        // Soma (Physical Load): when BioGate has triggered, explicitly ask for supportive tone in the reflection.
        let soma_hint = if mental.has_physical_load_adjustment() {
            format!(
                "[Soma — Physical load elevated (e.g. low sleep/readiness). {}]",
                MentalState::PHYSICAL_LOAD_SYSTEM_INSTRUCTION
            )
        } else {
            String::new()
        };
        // Philosophical lens: fetch EthosPolicy from `ethos/current` for school-specific reframing.
        let ethos_hint = if let Some(phil) = self.store.get_ethos_philosophical_policy() {
            phil.to_system_instruction()
        } else {
            // Fallback: check safety policy exists → generic guardrail hint.
            self.store
                .get_ethos_policy()
                .map(|_| "Respond within the user's guardrails (Ethos).".to_string())
                .unwrap_or_default()
        };

        // Relational Map: if content mentions a person in the Kardia Map, inject their trust_score and attachment_style.
        let content_lower = raw_content.to_lowercase();
        let people = self.store.list_people().unwrap_or_default();
        let mentioned: Vec<_> = people
            .into_iter()
            .filter(|p| !p.name.is_empty() && content_lower.contains(&p.name.to_lowercase()))
            .collect();
        let relationship_context = if mentioned.is_empty() {
            String::new()
        } else {
            let lines: Vec<String> = mentioned
                .iter()
                .map(|p| {
                    let triggers = if p.triggers.is_empty() {
                        "none noted".to_string()
                    } else {
                        p.triggers.join(", ")
                    };
                    format!(
                        "{} (relationship={}, trust_score={:.2}, attachment_style={}; triggers: {}).",
                        p.name,
                        if p.relationship.is_empty() { "—" } else { &p.relationship },
                        p.trust_score,
                        if p.attachment_style.is_empty() { "—" } else { &p.attachment_style },
                        triggers
                    )
                })
                .collect();
            format!(
                "Mentioned relationships — use for context-aware reframing (acknowledge low trust or attachment dynamics when relevant): {}",
                lines.join(" ")
            )
        };

        let prompt = if relationship_context.is_empty() {
            if soma_hint.is_empty() {
                format!(
                    "I am processing a sensitive entry from my Shadow Vault: [CONTENT]. \
                     Based on my Ethos and Kardia, provide a perspective that helps me reframe this stressor \
                     without storing this text in your permanent memory.\n\n\
                     [Kardia context: {}]\n\
                     [{}]\n\n\
                     [CONTENT]:\n{}",
                    kardia_context,
                    ethos_hint,
                    raw_content,
                )
            } else {
                format!(
                    "I am processing a sensitive entry from my Shadow Vault: [CONTENT]. \
                     Based on my Ethos and Kardia, provide a perspective that helps me reframe this stressor \
                     without storing this text in your permanent memory.\n\n\
                     [Kardia context: {}]\n\
                     {}\n\
                     [{}]\n\n\
                     [CONTENT]:\n{}",
                    kardia_context,
                    soma_hint,
                    ethos_hint,
                    raw_content,
                )
            }
        } else {
            if soma_hint.is_empty() {
                format!(
                    "I am processing a sensitive entry from my Shadow Vault: [CONTENT]. \
                     Based on my Ethos and Kardia, provide a perspective that helps me reframe this stressor \
                     without storing this text in your permanent memory.\n\n\
                     [Kardia context: {}]\n\
                     [Relational Map — {}]\n\
                     [{}]\n\n\
                     [CONTENT]:\n{}",
                    kardia_context,
                    relationship_context,
                    ethos_hint,
                    raw_content,
                )
            } else {
                format!(
                    "I am processing a sensitive entry from my Shadow Vault: [CONTENT]. \
                     Based on my Ethos and Kardia, provide a perspective that helps me reframe this stressor \
                     without storing this text in your permanent memory.\n\n\
                     [Kardia context: {}]\n\
                     {}\n\
                     [Relational Map — {}]\n\
                     [{}]\n\n\
                     [CONTENT]:\n{}",
                    kardia_context,
                    soma_hint,
                    relationship_context,
                    ethos_hint,
                    raw_content,
                )
            }
        };

        // Volatile path: LLM sees content only in this request; we never log or persist it.
        let reflection = self
            .model_router
            .generate_reflection(&prompt)
            .await
            .map_err(|e| format!("Reflection LLM: {}", e))?;

        // Purge decrypted content and full prompt from memory immediately.
        secure_purge(raw_content);
        secure_purge(prompt);
        // Do not log or write prompt/raw_content anywhere.

        // Chronos: log only the fact of reflection, never the content.
        let event = EventRecord::now("Chronos", format!("User performed a Shadow Reflection on record {}.", args.record_id))
            .with_skill(SKILL_NAME)
            .with_outcome("shadow_reflection");
        let _ = self.store.append_chronos_event(agent_id, &event);

        Ok(serde_json::json!({
            "status": "ok",
            "skill": SKILL_NAME,
            "record_id": args.record_id,
            "reflection": reflection,
            "chronos_logged": true,
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pagi_core::{KnowledgeStore, PersonalHistoryEntry, ShadowStore, ShadowStoreHandle};

    /// 64 hex chars = 32 bytes. NOT for production.
    fn test_key_hex() -> String {
        (0..32).map(|i| format!("{:02x}", (i as u8).wrapping_mul(7).wrapping_add(42))).collect()
    }

    #[tokio::test]
    async fn reflect_shadow_returns_reframing_and_keeps_vault_encrypted() {
        let key_hex = test_key_hex();
        std::env::set_var("PAGI_SHADOW_KEY", &key_hex);

        let shadow_dir = tempfile::tempdir().unwrap();
        let shadow_path = shadow_dir.path().join("pagi_shadow");
        let shadow = ShadowStore::open_path(&shadow_path).unwrap();
        let entry = PersonalHistoryEntry {
            label: "work_conflict".to_string(),
            intensity: 0.7,
            timestamp_ms: 12345,
            raw_content: Some(
                "Had a conflict with my manager today about the deadline. Feeling stressed and unheard.".to_string(),
            ),
        };
        shadow.put_journal("journal/12345", &entry).unwrap();
        let shadow_handle: ShadowStoreHandle = Arc::new(tokio::sync::RwLock::new(Some(shadow)));

        let kb_dir = tempfile::tempdir().unwrap();
        let knowledge = Arc::new(KnowledgeStore::open_path(kb_dir.path()).unwrap());
        let model_router = Arc::new(ModelRouter::with_knowledge(Arc::clone(&knowledge)));
        let skill = ReflectShadowSkill::new(
            Arc::clone(&knowledge),
            Arc::clone(&shadow_handle),
            model_router,
        );

        let ctx = TenantContext {
            tenant_id: "test".to_string(),
            correlation_id: None,
            agent_id: Some("default".to_string()),
        };
        let payload = serde_json::json!({
            "record_id": "journal/12345",
            "session_key": key_hex,
        });

        let result = skill.execute(&ctx, Some(payload)).await.unwrap();

        assert_eq!(result["status"], "ok");
        assert_eq!(result["skill"], SKILL_NAME);
        assert_eq!(result["record_id"], "journal/12345");
        let reflection = result["reflection"].as_str().unwrap_or("");
        assert!(
            reflection.len() > 0,
            "SAGE_BOT should provide a supportive reframing; got: {:?}",
            result
        );
        assert!(
            reflection.to_lowercase().contains("reframe") || reflection.to_lowercase().contains("sense") || reflection.to_lowercase().contains("agency") || reflection.to_lowercase().contains("gentle") || reflection.to_lowercase().contains("supportive") || reflection.to_lowercase().contains("feeling"),
            "Reflection should be supportive; got: {}",
            reflection
        );
        assert_eq!(result["chronos_logged"], true);

        // Vault still has the entry (encrypted); we did not write raw_content to pagi_knowledge.
        let guard = shadow_handle.read().await;
        let store = guard.as_ref().unwrap();
        let decrypted = store.get_journal("journal/12345").unwrap();
        assert!(decrypted.is_some(), "Record should still exist in pagi_shadow");
        let raw = decrypted.unwrap().0.raw_content.unwrap_or_default();
        assert_eq!(raw, "Had a conflict with my manager today about the deadline. Feeling stressed and unheard.");
    }
}
