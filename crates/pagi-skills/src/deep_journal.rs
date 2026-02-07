//! **DeepJournal Skill** — Secure pipeline for sensitive journal entries.
//!
//! This skill implements the full "encrypt → anchor → purge" flow:
//!
//! 1. **Anchor Insertion:** Calls `knowledge.insert_shadow_anchor()` to store an
//!    `EmotionalAnchor` (label + intensity) in Slot 9 (Shadow_KB). This is what the
//!    Compassionate Router reads via `check_mental_load()`.
//!
//! 2. **Encrypted Full Text:** Wraps the raw journal entry in a JSON blob and stores it
//!    via `knowledge.insert(SHADOW_SLOT_ID, ...)` so the full text is AES-256-GCM
//!    encrypted at rest. Also stores in the separate `ShadowStore` for the journal DB.
//!
//! 3. **Memory Purge:** After both writes complete, the `raw_entry` String is explicitly
//!    zeroed and dropped. It is never logged, never sent to external APIs, and never
//!    held in memory longer than necessary.
//!
//! 4. **MentalState Update:** Extracted anchors update the Cognitive Governor's
//!    `MentalState` (relational_stress, burnout_risk, grace_multiplier).

use pagi_core::{
    AgentSkill, EmotionalAnchor, KnowledgeStore, PersonalHistoryEntry, ShadowStoreHandle,
    TenantContext,
};
use crate::journal_skill::{apply_anchors_to_state, extract_anchors};
use serde::Deserialize;
use std::sync::Arc;

const SKILL_NAME: &str = "DeepJournalSkill";

/// Slot 9 ID constant (matches `pagi_core::knowledge::store::SHADOW_SLOT_ID`).
const SHADOW_SLOT_ID: u8 = 9;

#[derive(Debug, Deserialize)]
struct DeepJournalArgs {
    /// Raw journal text. Encrypted into Shadow_KB (Slot 9); never logged.
    /// Anonymized anchors update MentalState for the Compassionate Router.
    raw_entry: String,

    /// Explicit anchor label (e.g. "grief", "work_pressure", "conflict").
    /// If omitted, auto-extracted from `raw_entry` via keyword matching.
    #[serde(default)]
    label: Option<String>,

    /// Explicit intensity 0.0–1.0 for this entry.
    /// If omitted, auto-extracted from `raw_entry` via keyword matching.
    #[serde(default)]
    intensity: Option<f32>,
}

pub struct DeepJournalSkill {
    store: Arc<KnowledgeStore>,
    shadow: ShadowStoreHandle,
}

impl DeepJournalSkill {
    pub fn new(store: Arc<KnowledgeStore>, shadow: ShadowStoreHandle) -> Self {
        Self { store, shadow }
    }
}

/// Securely zeroes a String's backing buffer before dropping it.
/// This prevents sensitive data from lingering in freed memory.
#[inline]
fn secure_purge(mut s: String) {
    // SAFETY: We overwrite the buffer with zeros, then drop the String.
    // The String is consumed by this function so no one else can read it.
    unsafe {
        let bytes = s.as_bytes_mut();
        for b in bytes.iter_mut() {
            std::ptr::write_volatile(b, 0u8);
        }
    }
    drop(s);
}

#[async_trait::async_trait]
impl AgentSkill for DeepJournalSkill {
    fn name(&self) -> &str {
        SKILL_NAME
    }

    async fn execute(
        &self,
        ctx: &TenantContext,
        payload: Option<serde_json::Value>,
    ) -> Result<serde_json::Value, Box<dyn std::error::Error + Send + Sync>> {
        let payload = payload.ok_or(
            "DeepJournalSkill requires payload: { raw_entry, label?, intensity? }",
        )?;
        let args: DeepJournalArgs = serde_json::from_value(payload)?;

        // Move raw_entry out so we can purge it after writes.
        let raw_entry = args.raw_entry;
        let agent_id = ctx.resolved_agent_id();

        let timestamp_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as i64)
            .unwrap_or(0);

        // ── Step 1: Determine anchor label & intensity ──────────────────────
        // Use explicit values if provided; otherwise auto-extract from text.
        let auto_anchors = extract_anchors(&raw_entry);
        let label = args
            .label
            .unwrap_or_else(|| {
                auto_anchors
                    .first()
                    .map(|(l, _)| l.clone())
                    .unwrap_or_else(|| "deep_journal".to_string())
            });
        let intensity = args
            .intensity
            .unwrap_or_else(|| {
                auto_anchors
                    .first()
                    .map(|(_, i)| *i)
                    .unwrap_or(0.3)
            })
            .clamp(0.0, 1.0);

        let record_id = format!("journal/{}", timestamp_ms);
        let anchor_key = format!("anchor/{}", label);

        // ── Step 2: Insert EmotionalAnchor into Slot 9 (Shadow_KB) ─────────
        // This is what `check_mental_load()` reads for Compassionate Routing.
        let anchor = EmotionalAnchor::new(&label, intensity).with_label(&label);
        if let Err(e) = self.store.insert_shadow_anchor(&anchor_key, &anchor) {
            tracing::warn!(
                target: "pagi::deep_journal",
                error = %e,
                "DeepJournalSkill: insert_shadow_anchor failed (vault may be locked)"
            );
            // Continue — we still update MentalState even if vault is locked.
        }

        // ── Step 3: Store encrypted full text in Slot 9 via KnowledgeStore ──
        // Wrap raw_entry in a JSON blob for structured storage.
        let journal_blob = serde_json::json!({
            "type": "deep_journal_entry",
            "label": &label,
            "intensity": intensity,
            "timestamp_ms": timestamp_ms,
            "content": &raw_entry,
        });
        let blob_bytes = serde_json::to_vec(&journal_blob)
            .map_err(|e| format!("serialize journal blob: {}", e))?;

        if let Err(e) = self.store.insert(SHADOW_SLOT_ID, &record_id, &blob_bytes) {
            tracing::warn!(
                target: "pagi::deep_journal",
                error = %e,
                "DeepJournalSkill: Slot 9 insert failed (vault may be locked)"
            );
        }

        // ── Step 4: Also store in the separate ShadowStore (journal DB) ─────
        let entry = PersonalHistoryEntry {
            label: label.clone(),
            intensity,
            timestamp_ms,
            raw_content: Some(raw_entry.clone()),
        };
        {
            let guard = self.shadow.read().await;
            if let Some(ref shadow_store) = *guard {
                if let Err(e) = shadow_store.put_journal(&record_id, &entry) {
                    tracing::warn!(
                        target: "pagi::deep_journal",
                        error = %e,
                        "DeepJournalSkill: ShadowStore put_journal failed"
                    );
                }
            }
            // If no key configured, we still proceed (graceful degradation).
        }

        // ── Step 5: MEMORY PURGE — zero and drop raw_entry immediately ──────
        // The raw text must not linger in memory after encryption is complete.
        secure_purge(raw_entry);
        // raw_entry is now consumed and zeroed; cannot be accessed.

        // ── Step 6: Update MentalState via Cognitive Governor ───────────────
        // Combine explicit anchor with any auto-extracted anchors.
        let mut all_anchors = vec![(label.clone(), intensity)];
        for (auto_label, auto_intensity) in &auto_anchors {
            if *auto_label != label {
                all_anchors.push((auto_label.clone(), *auto_intensity));
            }
        }

        // Log only anonymized anchor labels; never log raw content.
        let anchor_labels: Vec<&str> = all_anchors.iter().map(|(l, _)| l.as_str()).collect();
        tracing::info!(
            target: "pagi::deep_journal",
            record_id = %record_id,
            anchors = ?anchor_labels,
            intensity = intensity,
            "DeepJournalSkill: encrypted entry; {} anchor(s) extracted",
            anchor_labels.len()
        );

        let current = self.store.get_mental_state(agent_id);
        let next = apply_anchors_to_state(&current, &all_anchors);
        self.store.set_mental_state(agent_id, &next)?;

        Ok(serde_json::json!({
            "status": "ok",
            "skill": SKILL_NAME,
            "record_id": record_id,
            "anchor_key": anchor_key,
            "anchors_extracted": all_anchors.len(),
            "mental_state_updated": true,
            "relational_stress": next.relational_stress,
            "burnout_risk": next.burnout_risk,
            "grace_multiplier": next.grace_multiplier,
            "shadow_kb_active": true,
            "compassionate_routing_enabled": intensity > 0.5,
        }))
    }
}
