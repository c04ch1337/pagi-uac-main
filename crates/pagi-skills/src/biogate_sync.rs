//! BioGate (Soma) sync skill: ingests health/biometric data and writes to **Slot 8 (Soma)**.
//!
//! Accepts data from Apple Health, Oura, Whoop, or other exports/APIs. Updates both the
//! legacy `BiometricState` and the new `SomaState` in KB_SOMA; the Cognitive Governor uses
//! them via `get_effective_mental_state` to modulate tone.
//!
//! ## SomaState Cross-Layer Reaction
//!
//! When `readiness_score < 50` **OR** `sleep_hours < 6.0`:
//! - `burnout_risk` is incremented by **+0.15**
//! - `grace_multiplier` is set to **1.6** (forcing supportive, less demanding tone)

use pagi_core::{AgentSkill, BiometricState, KnowledgeStore, SomaState, TenantContext};
use serde::Deserialize;
use std::sync::Arc;

const SKILL_NAME: &str = "BioGateSync";

/// Input payload for the BioGateSync skill.
///
/// Supports both the new `SomaState` fields (`sleep_hours`, `resting_hr`, `hrv`,
/// `readiness_score`) and the legacy `BiometricState` fields (`sleep_score`,
/// `heart_rate_variability`, `activity_level`). Both are written to Slot 8.
#[derive(Debug, Deserialize)]
struct BioGateSyncArgs {
    // --- SomaState fields (BioGate v2) ---

    /// Hours of sleep in the last cycle (e.g. 4.5, 7.0, 8.5).
    #[serde(default)]
    sleep_hours: f32,
    /// Resting heart rate in BPM (e.g. 55, 72).
    #[serde(default)]
    resting_hr: u32,
    /// Heart rate variability in ms (e.g. RMSSD: 35, 60, 90).
    #[serde(default)]
    hrv: u32,
    /// Overall readiness score (0–100). Values < 50 trigger BioGate cross-layer reaction.
    #[serde(default = "default_readiness")]
    readiness_score: u32,

    // --- Legacy BiometricState fields ---

    /// Sleep quality score 0–100. Values < 60 trigger supportive tone adjustment.
    #[serde(default)]
    sleep_score: f32,
    /// Heart rate variability (e.g. normalized 0–1 or RMSSD).
    #[serde(default)]
    heart_rate_variability: f32,
    /// Activity level (0–100 or 0–1).
    #[serde(default)]
    activity_level: f32,
}

fn default_readiness() -> u32 {
    100
}

pub struct BioGateSync {
    store: Arc<KnowledgeStore>,
}

impl BioGateSync {
    pub fn new(store: Arc<KnowledgeStore>) -> Self {
        Self { store }
    }
}

#[async_trait::async_trait]
impl AgentSkill for BioGateSync {
    fn name(&self) -> &str {
        SKILL_NAME
    }

    async fn execute(
        &self,
        _ctx: &TenantContext,
        payload: Option<serde_json::Value>,
    ) -> Result<serde_json::Value, Box<dyn std::error::Error + Send + Sync>> {
        let payload = payload.unwrap_or(serde_json::Value::Null);
        let args: BioGateSyncArgs = serde_json::from_value(payload).unwrap_or(BioGateSyncArgs {
            sleep_hours: 0.0,
            resting_hr: 0,
            hrv: 0,
            readiness_score: 100,
            sleep_score: 0.0,
            heart_rate_variability: 0.0,
            activity_level: 0.0,
        });

        // --- Write SomaState (BioGate v2) to Slot 8 ---
        let mut soma = SomaState {
            sleep_hours: args.sleep_hours,
            resting_hr: args.resting_hr,
            hrv: args.hrv,
            readiness_score: args.readiness_score,
        };
        soma.clamp();
        self.store.set_soma_state(&soma)?;

        // --- Write legacy BiometricState to Slot 8 (backward compat) ---
        // If legacy fields are provided, write them; otherwise derive from SomaState.
        let sleep_score = if args.sleep_score > 0.0 {
            args.sleep_score
        } else {
            // Derive: map sleep_hours to a 0–100 score (8h = 100, 0h = 0)
            (soma.sleep_hours / 8.0 * 100.0).clamp(0.0, 100.0)
        };
        let mut bio = BiometricState {
            sleep_score,
            heart_rate_variability: args.heart_rate_variability,
            activity_level: args.activity_level,
        };
        bio.clamp();
        self.store.set_biometric_state(&bio)?;

        let biogate_triggered = soma.needs_biogate_adjustment();
        let legacy_triggered = bio.poor_sleep();

        Ok(serde_json::json!({
            "status": "ok",
            "skill": SKILL_NAME,
            "slot_id": 8,
            "soma_state": {
                "sleep_hours": soma.sleep_hours,
                "resting_hr": soma.resting_hr,
                "hrv": soma.hrv,
                "readiness_score": soma.readiness_score,
                "biogate_triggered": biogate_triggered,
            },
            "biometric_state": {
                "sleep_score": bio.sleep_score,
                "heart_rate_variability": bio.heart_rate_variability,
                "activity_level": bio.activity_level,
                "legacy_triggered": legacy_triggered,
            },
            "cross_layer_active": biogate_triggered || legacy_triggered,
            "message": if biogate_triggered {
                format!(
                    "SomaState stored. BioGate cross-layer reaction ACTIVE: burnout_risk += {}, grace_multiplier = {} (readiness={}, sleep={}h).",
                    SomaState::BURNOUT_RISK_INCREMENT,
                    SomaState::GRACE_MULTIPLIER_OVERRIDE,
                    soma.readiness_score,
                    soma.sleep_hours,
                )
            } else if legacy_triggered {
                "Biometric state stored. Cognitive Governor will apply supportive tone (grace_multiplier=1.5, elevated burnout_risk).".to_string()
            } else {
                "Health metrics stored in Slot 8 (Soma). No cross-layer adjustment needed.".to_string()
            }
        }))
    }
}
