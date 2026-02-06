//! Knowledge Pruner skill: removes outdated entries from KB-5 (Community) and KB-8 (Internal Research)
//! to keep storage lean. Uses `updated_at` (KB-5) and `created_at` (KB-8) for retention.

use pagi_core::{AgentSkill, KnowledgeStore, TenantContext};
use std::sync::Arc;

const SKILL_NAME: &str = "KnowledgePruner";
const KB_SLOT_COMMUNITY: u8 = 5;
const KB_SLOT_INTERNAL_RESEARCH: u8 = 8;
const SECS_PER_DAY: u64 = 86400;
const DEFAULT_KB5_MAX_AGE_DAYS: u64 = 30;
const DEFAULT_KB8_MAX_AGE_DAYS: u64 = 14;

/// Prunes old pulse data (KB-5) and thought logs (KB-8) by age. Invoke via ExecuteSkill or on a schedule.
pub struct KnowledgePruner {
    knowledge: Arc<KnowledgeStore>,
}

impl KnowledgePruner {
    pub fn new(knowledge: Arc<KnowledgeStore>) -> Self {
        Self { knowledge }
    }
}

#[async_trait::async_trait]
impl AgentSkill for KnowledgePruner {
    fn name(&self) -> &str {
        SKILL_NAME
    }

    async fn execute(
        &self,
        _ctx: &TenantContext,
        payload: Option<serde_json::Value>,
    ) -> Result<serde_json::Value, Box<dyn std::error::Error + Send + Sync>> {
        let now_secs = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();

        let p = payload.as_ref();
        let kb5_max_age_days = p
            .and_then(|v| v.get("kb5_max_age_days"))
            .and_then(|v| v.as_u64())
            .unwrap_or(DEFAULT_KB5_MAX_AGE_DAYS);
        let kb8_max_age_days = p
            .and_then(|v| v.get("kb8_max_age_days"))
            .and_then(|v| v.as_u64())
            .unwrap_or(DEFAULT_KB8_MAX_AGE_DAYS);

        let kb5_cutoff = now_secs.saturating_sub(kb5_max_age_days * SECS_PER_DAY);
        let kb8_cutoff = now_secs.saturating_sub(kb8_max_age_days * SECS_PER_DAY);

        let mut kb5_removed: Vec<String> = Vec::new();
        let mut kb8_removed: Vec<String> = Vec::new();

        if kb5_max_age_days > 0 {
            let keys = self.knowledge.scan_keys(KB_SLOT_COMMUNITY)?;
            for key in keys {
                if let Some(val) = self.knowledge.get(KB_SLOT_COMMUNITY, &key)? {
                    let ts = String::from_utf8_lossy(&val);
                    if let Ok(obj) = serde_json::from_str::<serde_json::Value>(&ts) {
                        let updated_at = obj
                            .get("updated_at")
                            .or_else(|| obj.get("created_at"))
                            .and_then(|v| v.as_u64());
                        if let Some(t) = updated_at {
                            if t < kb5_cutoff {
                                self.knowledge.remove(KB_SLOT_COMMUNITY, &key)?;
                                kb5_removed.push(key);
                            }
                        }
                    }
                }
            }
        }

        if kb8_max_age_days > 0 {
            let keys = self.knowledge.scan_keys(KB_SLOT_INTERNAL_RESEARCH)?;
            for key in keys {
                if let Some(val) = self.knowledge.get(KB_SLOT_INTERNAL_RESEARCH, &key)? {
                    let ts = String::from_utf8_lossy(&val);
                    if let Ok(obj) = serde_json::from_str::<serde_json::Value>(&ts) {
                        let created_at = obj.get("created_at").and_then(|v| v.as_u64());
                        if let Some(t) = created_at {
                            if t < kb8_cutoff {
                                self.knowledge.remove(KB_SLOT_INTERNAL_RESEARCH, &key)?;
                                kb8_removed.push(key);
                            }
                        }
                    }
                }
            }
        }

        Ok(serde_json::json!({
            "status": "ok",
            "skill": SKILL_NAME,
            "kb5_max_age_days": kb5_max_age_days,
            "kb8_max_age_days": kb8_max_age_days,
            "kb5_pruned": kb5_removed.len(),
            "kb8_pruned": kb8_removed.len(),
            "kb5_removed_keys": kb5_removed,
            "kb8_removed_keys": kb8_removed,
        }))
    }
}
