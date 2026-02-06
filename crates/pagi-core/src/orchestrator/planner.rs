//! Dynamic skill planning: delegates to BlueprintRegistry (default or loaded from config).

use super::blueprint::{BlueprintRegistry, Plan};

/// Returns a plan for the given intent using the default blueprint, or None if unknown.
#[allow(dead_code)]
pub fn plan_for_intent(intent: &str) -> Option<Plan> {
    BlueprintRegistry::default_blueprint().plan_for_intent(intent)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plan_respond_to_lead() {
        let plan = plan_for_intent("respond to lead").unwrap();
        assert_eq!(
            plan.steps,
            ["DraftResponse", "SalesCloser", "ModelRouter"]
        );
    }

    #[test]
    fn plan_unknown_intent() {
        assert!(plan_for_intent("unknown intent").is_none());
    }
}
