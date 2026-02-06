//! Master Brain: task delegation and reasoning.

mod blueprint;
mod control;
mod planner;

pub use blueprint::{BlueprintRegistry, Plan};
pub use control::ControlPanelMessage;

use crate::shared::{Goal, TenantContext};
use std::fmt;
use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
use std::sync::{Arc, RwLock};
use tokio::sync::mpsc;

#[derive(Debug)]
struct UnknownSkill(String);

impl fmt::Display for UnknownSkill {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "unknown skill: {}", self.0)
    }
}

impl std::error::Error for UnknownSkill {}

/// Trait implemented by all agent capabilities (skills).
#[async_trait::async_trait]
pub trait AgentSkill: Send + Sync {
    /// Unique skill name for routing.
    fn name(&self) -> &str;

    /// Executes the skill with the given context and optional payload.
    async fn execute(
        &self,
        ctx: &TenantContext,
        payload: Option<serde_json::Value>,
    ) -> Result<serde_json::Value, Box<dyn std::error::Error + Send + Sync>>;
}

/// Registry of agent skills that can be dispatched by name.
pub struct SkillRegistry {
    skills: Vec<Arc<dyn AgentSkill>>,
}

impl SkillRegistry {
    pub fn new() -> Self {
        Self {
            skills: Vec::new(),
        }
    }

    pub fn register(&mut self, skill: Arc<dyn AgentSkill>) {
        self.skills.push(skill);
    }

    pub fn get(&self, name: &str) -> Option<Arc<dyn AgentSkill>> {
        self.skills.iter().find(|s| s.name() == name).cloned()
    }

    /// Returns the names of all registered skills (for discovery and planning).
    pub fn skill_names(&self) -> Vec<String> {
        self.skills.iter().map(|s| s.name().to_string()).collect()
    }
}

impl Default for SkillRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// Receiver for control-panel messages (hold on orchestrator side).
pub type ControlPanelReceiver = mpsc::Receiver<ControlPanelMessage>;

/// Orchestrator dispatches goals to skills and coordinates execution.
/// Holds control state (active KBs, skills enabled, memory weights) updated by the control panel.
pub struct Orchestrator {
    registry: Arc<SkillRegistry>,
    blueprint: Arc<BlueprintRegistry>,
    /// Bitmask: bit i (0..7) = KB-(i+1) active. All 8 bits set = all active.
    active_kbs: AtomicU8,
    /// When false, dispatch returns "Skills Disabled" without calling skills.
    skills_enabled: AtomicBool,
    /// (short_term, long_term) weights for memory retrieval scoring.
    memory_weights: RwLock<(f32, f32)>,
}

impl Orchestrator {
    pub fn new(registry: Arc<SkillRegistry>) -> Self {
        Self {
            registry: Arc::clone(&registry),
            blueprint: Arc::new(BlueprintRegistry::default_blueprint()),
            active_kbs: AtomicU8::new(0xFF),
            skills_enabled: AtomicBool::new(true),
            memory_weights: RwLock::new((0.7, 0.3)),
        }
    }

    pub fn with_blueprint(registry: Arc<SkillRegistry>, blueprint: Arc<BlueprintRegistry>) -> Self {
        Self {
            registry,
            blueprint,
            active_kbs: AtomicU8::new(0xFF),
            skills_enabled: AtomicBool::new(true),
            memory_weights: RwLock::new((0.7, 0.3)),
        }
    }

    /// Applies a control-panel message to the orchestrator state (lock-free where possible).
    pub fn pagi_apply_control_signal(&self, msg: ControlPanelMessage) {
        use ControlPanelMessage::*;
        match msg {
            KbState { index, active } => {
                if index < 8 {
                    let mask = 1u8 << index;
                    self.active_kbs.fetch_update(Ordering::SeqCst, Ordering::SeqCst, |v| {
                        Some(if active { v | mask } else { v & !mask })
                    })
                    .ok();
                }
            }
            SkillsEnabled(enabled) => {
                self.skills_enabled.store(enabled, Ordering::SeqCst);
            }
            MemoryWeights { short_term, long_term } => {
                if let Ok(mut w) = self.memory_weights.write() {
                    *w = (short_term, long_term);
                }
            }
            FullState {
                kb_states,
                skills_enabled: se,
                short_term_memory_weight: st,
                long_term_memory_weight: lt,
            } => {
                let mut mask = 0u8;
                for (i, &on) in kb_states.iter().enumerate().take(8) {
                    if on {
                        mask |= 1u8 << i;
                    }
                }
                self.active_kbs.store(mask, Ordering::SeqCst);
                self.skills_enabled.store(se, Ordering::SeqCst);
                if let Ok(mut w) = self.memory_weights.write() {
                    *w = (st, lt);
                }
            }
        }
    }

    /// Returns whether the given KB slot (1..=8) is active.
    #[inline]
    pub fn pagi_kb_active(&self, slot_id: u8) -> bool {
        if !(1..=8).contains(&slot_id) {
            return false;
        }
        let index = (slot_id - 1) as usize;
        let mask = 1u8 << index;
        self.active_kbs.load(Ordering::Acquire) & mask != 0
    }

    /// Returns current memory weights (short_term, long_term).
    pub fn pagi_memory_weights(&self) -> (f32, f32) {
        self.memory_weights.read().map(|g| *g).unwrap_or((0.7, 0.3))
    }

    /// Returns whether the skills execution engine is enabled (control-panel state).
    #[inline]
    pub fn pagi_skills_enabled(&self) -> bool {
        self.skills_enabled.load(Ordering::Acquire)
    }

    /// Spawns a background tokio task that receives control messages and applies them to this orchestrator.
    /// Call with `Arc::clone(&orchestrator)` and the receiver half of the control-panel channel.
    pub fn spawn_control_listener(self: Arc<Self>, mut receiver: ControlPanelReceiver) {
        tokio::spawn(async move {
            while let Some(msg) = receiver.recv().await {
                self.pagi_apply_control_signal(msg);
            }
        });
    }

    /// Dispatches a goal; ExecuteSkill is routed to the registered skill and executed.
    /// Respects control-panel state: skills disabled and inactive KBs are gated.
    pub async fn dispatch(
        &self,
        ctx: &TenantContext,
        goal: Goal,
    ) -> Result<serde_json::Value, Box<dyn std::error::Error + Send + Sync>> {
        if !self.skills_enabled.load(Ordering::Acquire) {
            return Ok(serde_json::json!({
                "status": "skills_disabled",
                "message": "Skills execution is disabled by the control panel.",
                "goal": serde_json::to_string(&goal).unwrap_or_default()
            }));
        }

        match goal {
            Goal::ExecuteSkill { name, payload } => {
                let skill = self
                    .registry
                    .get(&name)
                    .ok_or_else(|| UnknownSkill(name.clone()))?;
                skill.execute(ctx, payload).await
            }
            Goal::QueryKnowledge { slot_id, query } => {
                if !self.pagi_kb_active(slot_id) {
                    return Ok(serde_json::json!({
                        "status": "kb_disabled",
                        "message": format!("KB-{} is disabled by the control panel.", slot_id),
                        "slot_id": slot_id,
                        "query": query
                    }));
                }
                let payload = serde_json::json!({ "slot_id": slot_id, "query_key": query });
                let skill = self
                    .registry
                    .get("KnowledgeQuery")
                    .ok_or_else(|| UnknownSkill("KnowledgeQuery".into()))?;
                skill.execute(ctx, Some(payload)).await
            }
            Goal::IngestData { payload } => {
                let skill = self
                    .registry
                    .get("LeadCapture")
                    .ok_or_else(|| UnknownSkill("LeadCapture".into()))?;
                skill.execute(ctx, payload).await
            }
            Goal::AssembleContext { context_id } => {
                let payload = serde_json::json!({ "lead_id": context_id });
                let skill = self
                    .registry
                    .get("DraftResponse")
                    .ok_or_else(|| UnknownSkill("DraftResponse".into()))?;
                skill.execute(ctx, Some(payload)).await
            }
            Goal::GenerateFinalResponse { context_id } => {
                let draft_skill = self
                    .registry
                    .get("DraftResponse")
                    .ok_or_else(|| UnknownSkill("DraftResponse".into()))?;
                let draft_payload = serde_json::json!({ "lead_id": context_id });
                let draft_result = draft_skill.execute(ctx, Some(draft_payload)).await?;
                let prompt = draft_result
                    .get("draft")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                let router_skill = self
                    .registry
                    .get("ModelRouter")
                    .ok_or_else(|| UnknownSkill("ModelRouter".into()))?;
                let router_payload = serde_json::json!({ "prompt": prompt });
                let router_result = router_skill.execute(ctx, Some(router_payload)).await?;
                let mut map = match router_result {
                    serde_json::Value::Object(m) => m,
                    _ => {
                        return Err(std::io::Error::new(
                            std::io::ErrorKind::InvalidData,
                            "ModelRouter did not return object",
                        )
                        .into())
                    }
                };
                map.insert("goal".to_string(), serde_json::json!("GenerateFinalResponse"));
                map.insert("context_id".to_string(), serde_json::json!(context_id));
                Ok(serde_json::Value::Object(map))
            }
            Goal::AutonomousGoal { intent, context } => {
                let plan = self.blueprint.plan_for_intent(&intent).ok_or_else(|| {
                    std::io::Error::new(
                        std::io::ErrorKind::InvalidInput,
                        format!("unknown intent: {}", intent),
                    )
                })?;
                let initial_context = context.clone().unwrap_or(serde_json::json!({}));
                let mut payload = initial_context.clone();
                let mut previous_result = serde_json::Value::Null;
                let mut previous_skill: Option<String> = None;
                let mut steps_trace: Vec<serde_json::Value> = Vec::new();

                for skill_name in &plan.steps {
                    let skill = self
                        .registry
                        .get(skill_name)
                        .ok_or_else(|| UnknownSkill(skill_name.clone()))?;
                    let step_input = chain_payload(previous_skill.as_deref(), skill_name, &previous_result, payload.clone());
                    previous_result = skill.execute(ctx, step_input.clone()).await?;
                    previous_skill = Some(skill_name.clone());
                    payload = previous_result.clone();

                    steps_trace.push(serde_json::json!({
                        "skill": skill_name,
                        "input": step_input,
                        "output": previous_result
                    }));
                }

                let final_result = previous_result.clone();
                let thought_log = serde_json::json!({
                    "intent": intent,
                    "context": initial_context,
                    "plan_steps": plan.steps,
                    "steps": steps_trace,
                    "final_result": final_result
                });

                if let Some(audit_skill) = self.registry.get("ResearchAudit") {
                    let audit_payload = serde_json::json!({ "trace": thought_log });
                    if let Ok(audit_result) = audit_skill.execute(ctx, Some(audit_payload)).await {
                        if let Some(trace_id) = audit_result.get("trace_id").and_then(|v| v.as_str()) {
                            let mut out = match final_result {
                                serde_json::Value::Object(m) => m,
                                _ => {
                                    let mut m = serde_json::Map::new();
                                    m.insert("result".to_string(), final_result);
                                    m
                                }
                            };
                            out.insert("goal".to_string(), serde_json::json!("AutonomousGoal"));
                            out.insert("intent".to_string(), serde_json::json!(intent));
                            out.insert("plan_steps".to_string(), serde_json::json!(plan.steps));
                            out.insert("trace_id".to_string(), serde_json::json!(trace_id));
                            return Ok(serde_json::Value::Object(out));
                        }
                    }
                }

                let mut out = match final_result {
                    serde_json::Value::Object(m) => m,
                    _ => return Ok(final_result),
                };
                out.insert("goal".to_string(), serde_json::json!("AutonomousGoal"));
                out.insert("intent".to_string(), serde_json::json!(intent));
                out.insert("plan_steps".to_string(), serde_json::json!(plan.steps));
                Ok(serde_json::Value::Object(out))
            }
            Goal::UpdateKnowledgeSlot {
                slot_id,
                source_url,
                source_html,
            } => {
                if !self.pagi_kb_active(slot_id) {
                    return Ok(serde_json::json!({
                        "status": "kb_disabled",
                        "message": format!("KB-{} is disabled by the control panel.", slot_id),
                        "slot_id": slot_id
                    }));
                }
                let mut payload = serde_json::json!({ "slot_id": slot_id });
                if let Some(url) = source_url {
                    payload["url"] = serde_json::Value::String(url);
                }
                if let Some(html) = source_html {
                    payload["html"] = serde_json::Value::String(html);
                }
                let skill = self
                    .registry
                    .get("CommunityScraper")
                    .ok_or_else(|| UnknownSkill("CommunityScraper".into()))?;
                skill.execute(ctx, Some(payload)).await
            }
            Goal::MemoryOp { path, value } => {
                Ok(serde_json::json!({ "path": path, "value": value, "status": "dispatched" }))
            }
            Goal::Custom(s) => Ok(serde_json::json!({ "custom": s, "status": "dispatched" })),
        }
    }
}

/// Derives the next skill's payload from the previous skill's result (output chaining).
fn chain_payload(
    previous_skill: Option<&str>,
    next_skill: &str,
    previous_result: &serde_json::Value,
    fallback: serde_json::Value,
) -> Option<serde_json::Value> {
    match (previous_skill, next_skill) {
        (Some("DraftResponse"), "SalesCloser") => {
            let draft = previous_result
                .get("draft")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            Some(serde_json::json!({ "draft": draft }))
        }
        (Some("SalesCloser"), "ModelRouter") => {
            let prompt = previous_result
                .get("draft")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            Some(serde_json::json!({ "prompt": prompt }))
        }
        (Some("DraftResponse"), "ModelRouter") => {
            let prompt = previous_result
                .get("draft")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            Some(serde_json::json!({ "prompt": prompt }))
        }
        (Some("CommunityScraper"), "ModelRouter") => {
            let prompt = previous_result
                .get("event")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            Some(serde_json::json!({ "prompt": prompt }))
        }
        _ if previous_result.is_null() => Some(fallback),
        _ => Some(fallback),
    }
}
