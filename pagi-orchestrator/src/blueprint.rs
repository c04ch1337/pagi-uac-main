//! Blueprint: intent → skill chain. Loaded from JSON/TOML for use-case-agnostic orchestration.

use serde::Deserialize;
use std::collections::HashMap;
use std::path::Path;

/// A plan is an ordered sequence of skill names to execute.
#[derive(Debug, Clone)]
pub struct Plan {
    pub steps: Vec<String>,
}

/// JSON shape for blueprint file: { "intents": { "intent name": ["SkillA", "SkillB"], ... } }
#[derive(Debug, Deserialize)]
pub struct BlueprintFile {
    pub intents: HashMap<String, Vec<String>>,
}

/// Registry that maps intent names to plans. Load from file or use default.
#[derive(Debug, Clone)]
pub struct BlueprintRegistry {
    intents: HashMap<String, Vec<String>>,
}

impl BlueprintRegistry {
    /// Empty registry (no intents).
    pub fn empty() -> Self {
        Self {
            intents: HashMap::new(),
        }
    }

    /// Default blueprint with common intents (e.g. "respond to lead").
    pub fn default_blueprint() -> Self {
        let mut intents = HashMap::new();
        intents.insert(
            "respond to lead".to_string(),
            vec![
                "DraftResponse".to_string(),
                "SalesCloser".to_string(),
                "ModelRouter".to_string(),
            ],
        );
        Self { intents }
    }

    /// Load from a JSON file. Returns default on error or missing file.
    pub fn load_json_path<P: AsRef<Path>>(path: P) -> Self {
        let path = path.as_ref();
        let s = match std::fs::read_to_string(path) {
            Ok(s) => s,
            Err(_) => return Self::default_blueprint(),
        };
        let file: BlueprintFile = match serde_json::from_str(&s) {
            Ok(f) => f,
            Err(_) => return Self::default_blueprint(),
        };
        let intents = file
            .intents
            .into_iter()
            .map(|(k, v)| (k.trim().to_lowercase(), v))
            .collect();
        Self { intents }
    }

    /// Build from in-memory intents (e.g. for tests).
    pub fn from_intents(intents: HashMap<String, Vec<String>>) -> Self {
        let intents = intents
            .into_iter()
            .map(|(k, v)| (k.trim().to_lowercase(), v))
            .collect();
        Self { intents }
    }

    /// Returns a plan for the given intent, or None if unknown.
    pub fn plan_for_intent(&self, intent: &str) -> Option<Plan> {
        let key = intent.trim().to_lowercase();
        self.intents.get(&key).cloned().map(|steps| Plan { steps })
    }

    /// List registered intent names.
    pub fn intent_names(&self) -> Vec<String> {
        self.intents.keys().cloned().collect()
    }
}

impl Default for BlueprintRegistry {
    fn default() -> Self {
        Self::default_blueprint()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_has_respond_to_lead() {
        let reg = BlueprintRegistry::default_blueprint();
        let plan = reg.plan_for_intent("respond to lead").unwrap();
        assert_eq!(plan.steps, ["DraftResponse", "SalesCloser", "ModelRouter"]);
    }

    #[test]
    fn from_intents_custom_plan() {
        let mut intents = HashMap::new();
        intents.insert(
            "summarize news".to_string(),
            vec!["GenericWebFetcher".to_string(), "Summarize".to_string()],
        );
        let reg = BlueprintRegistry::from_intents(intents);
        let plan = reg.plan_for_intent("summarize news").unwrap();
        assert_eq!(plan.steps, ["GenericWebFetcher", "Summarize"]);
        assert!(reg.plan_for_intent("respond to lead").is_none());
    }
}
