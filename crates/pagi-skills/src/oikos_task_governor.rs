//! **OikosTaskGovernor Skill** — Task prioritization from Soma, Kardia, and Ethos.
//!
//! Evaluates the current task set (stored in **KB_OIKOS**, Slot 2) against the user's
//! biological state (Soma), emotional/relational load (Kardia), and philosophical lens (Ethos).
//! Returns a governance summary and a short **recommendation** narrative (e.g. "You are at 4.0h
//! sleep and facing an Avoidant Manager. Based on your Stoic Ethos, I recommend moving
//! 'Deadline Negotiation' to tomorrow and focusing on 'Deep Work' today.").
//!
//! Optional payload: `tasks` — array of `{ task_id, title, difficulty, description?, base_priority?, tags? }`
//! to upsert before evaluation. If omitted, only existing Oikos tasks are evaluated.

use pagi_core::{
    AgentSkill, GovernanceAction, GovernedTask, KnowledgeStore, TenantContext, TaskDifficulty,
};
use serde::Deserialize;
use std::sync::Arc;

const SKILL_NAME: &str = "OikosTaskGovernor";

#[derive(Debug, Deserialize)]
struct TaskInput {
    task_id: String,
    title: String,
    #[serde(default)]
    difficulty: Option<String>,
    #[serde(default)]
    description: String,
    #[serde(default)]
    base_priority: Option<f32>,
    #[serde(default)]
    tags: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct OikosTaskGovernorArgs {
    /// Optional: tasks to upsert before evaluation. If empty or missing, only existing tasks are evaluated.
    #[serde(default)]
    tasks: Vec<TaskInput>,
}

fn parse_difficulty(s: &str) -> TaskDifficulty {
    match s.to_lowercase().as_str() {
        "low" => TaskDifficulty::Low,
        "medium" => TaskDifficulty::Medium,
        "high" => TaskDifficulty::High,
        "critical" => TaskDifficulty::Critical,
        _ => TaskDifficulty::Medium,
    }
}

pub struct OikosTaskGovernor {
    store: Arc<KnowledgeStore>,
}

impl OikosTaskGovernor {
    pub fn new(store: Arc<KnowledgeStore>) -> Self {
        Self { store }
    }

    /// Builds a short recommendation narrative from governor state and evaluated tasks.
    fn build_recommendation(
        soma_sleep: f32,
        soma_readiness: u32,
        ethos_school: Option<&str>,
        people_context: &[String],
        evaluated: &[GovernedTask],
    ) -> String {
        let mut parts = Vec::new();

        parts.push(format!(
            "You are at {:.1}h sleep and readiness {}.",
            soma_sleep,
            soma_readiness
        ));

        if !people_context.is_empty() {
            parts.push(format!("Facing: {}.", people_context.join("; ")));
        }

        if let Some(school) = ethos_school {
            parts.push(format!("Based on your {} Ethos,", school));
        }

        let proceed: Vec<&str> = evaluated
            .iter()
            .filter(|t| t.action.is_proceed())
            .map(|t| t.title.as_str())
            .take(5)
            .collect();
        let postpone: Vec<&str> = evaluated
            .iter()
            .filter(|t| t.action.is_postpone())
            .map(|t| t.title.as_str())
            .take(5)
            .collect();

        if !proceed.is_empty() {
            parts.push(format!(
                "I recommend focusing today on: {}.",
                proceed.join(", ")
            ));
        }
        if !postpone.is_empty() {
            parts.push(format!(
                "Consider moving to another time: {}.",
                postpone.join(", ")
            ));
        }

        if parts.is_empty() {
            "No tasks in Oikos. Add tasks via payload or another skill, then re-run governance.".to_string()
        } else {
            parts.join(" ")
        }
    }
}

#[async_trait::async_trait]
impl AgentSkill for OikosTaskGovernor {
    fn name(&self) -> &str {
        SKILL_NAME
    }

    async fn execute(
        &self,
        ctx: &TenantContext,
        payload: Option<serde_json::Value>,
    ) -> Result<serde_json::Value, Box<dyn std::error::Error + Send + Sync>> {
        let agent_id = ctx.resolved_agent_id();

        let args: OikosTaskGovernorArgs = payload
            .and_then(|p| serde_json::from_value(p).ok())
            .unwrap_or(OikosTaskGovernorArgs { tasks: vec![] });

        // Optional: upsert tasks from payload
        for t in &args.tasks {
            if t.task_id.is_empty() || t.title.is_empty() {
                continue;
            }
            let difficulty = t
                .difficulty
                .as_deref()
                .map(parse_difficulty)
                .unwrap_or(TaskDifficulty::Medium);
            let mut task = GovernedTask::new(&t.task_id, &t.title, difficulty)
                .with_description(&t.description)
                .with_tags(t.tags.clone());
            if let Some(p) = t.base_priority {
                task = task.with_priority(p);
            }
            self.store.set_governed_task(&task)?;
        }

        // Evaluate all tasks with current Soma + Kardia + Ethos and persist
        let evaluated = self.store.evaluate_and_persist_tasks(agent_id)?;

        let summary = self
            .store
            .get_governance_summary()
            .unwrap_or_else(|| "No summary yet.".to_string());

        let governor = self.store.create_task_governor(agent_id);
        let ethos_school = governor.ethos.as_ref().map(|e| e.active_school.as_str());

        // Optional Kardia context: low-trust or avoidant people for "facing X" in recommendation
        let people = self.store.list_people().unwrap_or_default();
        let people_context: Vec<String> = people
            .iter()
            .filter(|p| p.trust_score < 0.5 || p.attachment_style.to_lowercase().contains("avoidant"))
            .map(|p| {
                if p.attachment_style.is_empty() {
                    format!("{} (low trust)", p.name)
                } else {
                    format!("{} ({})", p.name, p.attachment_style)
                }
            })
            .take(3)
            .collect();

        let recommendation = Self::build_recommendation(
            governor.soma.sleep_hours,
            governor.soma.readiness_score,
            ethos_school,
            &people_context,
            &evaluated,
        );

        let tasks_json: Vec<serde_json::Value> = evaluated
            .iter()
            .map(|t| {
                let action_str = match &t.action {
                    GovernanceAction::Proceed => "proceed".to_string(),
                    GovernanceAction::Postpone { reason } => format!("postpone: {}", reason),
                    GovernanceAction::Simplify { suggestion } => format!("simplify: {}", suggestion),
                    GovernanceAction::Deprioritize { reason } => format!("deprioritize: {}", reason),
                };
                serde_json::json!({
                    "task_id": t.task_id,
                    "title": t.title,
                    "difficulty": format!("{:?}", t.difficulty),
                    "effective_priority": t.effective_priority,
                    "action": action_str,
                })
            })
            .collect();

        Ok(serde_json::json!({
            "status": "ok",
            "skill": SKILL_NAME,
            "slot_id": 2,
            "summary": summary,
            "recommendation": recommendation,
            "tasks_evaluated": evaluated.len(),
            "tasks": tasks_json,
            "bio_penalty": governor.bio_penalty(),
            "emotional_penalty": governor.emotional_penalty(),
        }))
    }
}
