//! Master Brain: task delegation and reasoning.

mod blueprint;
mod planner;

pub use blueprint::{BlueprintRegistry, Plan};

use pagi_shared::{Goal, TenantContext};
use std::fmt;
use std::sync::Arc;

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

/// Orchestrator dispatches goals to skills and coordinates execution.
pub struct Orchestrator {
    registry: Arc<SkillRegistry>,
    blueprint: Arc<BlueprintRegistry>,
}

impl Orchestrator {
    pub fn new(registry: Arc<SkillRegistry>) -> Self {
        Self {
            registry: Arc::clone(&registry),
            blueprint: Arc::new(BlueprintRegistry::default_blueprint()),
        }
    }

    pub fn with_blueprint(registry: Arc<SkillRegistry>, blueprint: Arc<BlueprintRegistry>) -> Self {
        Self { registry, blueprint }
    }

    /// Dispatches a goal; ExecuteSkill is routed to the registered skill and executed.
    pub async fn dispatch(
        &self,
        ctx: &TenantContext,
        goal: Goal,
    ) -> Result<serde_json::Value, Box<dyn std::error::Error + Send + Sync>> {
        match goal {
            Goal::ExecuteSkill { name, payload } => {
                let skill = self
                    .registry
                    .get(&name)
                    .ok_or_else(|| UnknownSkill(name.clone()))?;
                skill.execute(ctx, payload).await
            }
            Goal::QueryKnowledge { slot_id, query } => {
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
