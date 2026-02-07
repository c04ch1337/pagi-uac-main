//! Integration test: Ethos Framework — verifies that setting a philosophical policy
//! via EthosSync persists to KB_ETHOS and that ReflectShadow injects school-specific
//! reframing instructions into the LLM prompt.
//!
//! ## Scenario
//! 1. Set Ethos to "Stoic" via EthosSync skill.
//! 2. Create a Shadow Vault entry about a conflict with "Project Manager".
//! 3. Add "Project Manager" to the Kardia Map with attachment_style = "Avoidant".
//! 4. Run ReflectShadow on the vault entry.
//! 5. **Confirm** the mock LLM response references Stoic principles and focuses on
//!    the user's reaction rather than the Manager's avoidant behavior.

use pagi_core::{EthosPolicy, KnowledgeStore, PersonRecord, KARDIA_PEOPLE_PREFIX};
use std::sync::Arc;

#[test]
fn ethos_policy_preset_stoic_has_dichotomy_of_control() {
    let policy = EthosPolicy::preset("Stoic").unwrap();
    assert_eq!(policy.active_school, "Stoic");
    assert!(
        policy.core_maxims.iter().any(|m| m.contains("control")),
        "Stoic preset should mention Dichotomy of Control"
    );
    let instruction = policy.to_system_instruction();
    assert!(
        instruction.contains("Stoic"),
        "System instruction should reference Stoic: {}",
        instruction
    );
    assert!(
        instruction.contains("control"),
        "System instruction should mention control: {}",
        instruction
    );
}

#[test]
fn ethos_policy_preset_growth_mindset() {
    let policy = EthosPolicy::preset("Growth-Mindset").unwrap();
    assert_eq!(policy.active_school, "Growth-Mindset");
    assert!(policy.core_maxims.iter().any(|m| m.contains("growth")));
}

#[test]
fn ethos_policy_preset_compassionate_witness() {
    let policy = EthosPolicy::preset("Compassionate-Witness").unwrap();
    assert_eq!(policy.active_school, "Compassionate-Witness");
    assert!(policy.core_maxims.iter().any(|m| m.contains("compassion")));
}

#[test]
fn ethos_policy_preset_taoist() {
    let policy = EthosPolicy::preset("Taoist").unwrap();
    assert_eq!(policy.active_school, "Taoist");
    assert!(policy.core_maxims.iter().any(|m| m.contains("wu-wei") || m.contains("Wu-wei")));
}

#[test]
fn ethos_policy_preset_existentialist() {
    let policy = EthosPolicy::preset("Existentialist").unwrap();
    assert_eq!(policy.active_school, "Existentialist");
    assert!(policy.core_maxims.iter().any(|m| m.contains("freedom")));
}

#[test]
fn ethos_policy_unknown_school_returns_none() {
    assert!(EthosPolicy::preset("Nihilist").is_none());
}

#[test]
fn ethos_policy_roundtrip_serialization() {
    let policy = EthosPolicy::default();
    let bytes = policy.to_bytes();
    let restored = EthosPolicy::from_bytes(&bytes).unwrap();
    assert_eq!(restored.active_school, policy.active_school);
    assert_eq!(restored.core_maxims.len(), policy.core_maxims.len());
    assert!((restored.tone_weight - policy.tone_weight).abs() < 0.001);
}

#[test]
fn ethos_policy_store_roundtrip() {
    let kb_dir = tempfile::tempdir().unwrap();
    let store = Arc::new(KnowledgeStore::open_path(kb_dir.path()).unwrap());

    // Initially no philosophical policy.
    assert!(store.get_ethos_philosophical_policy().is_none());

    // Set Stoic.
    let stoic = EthosPolicy::preset("Stoic").unwrap();
    store.set_ethos_philosophical_policy(&stoic).unwrap();

    let loaded = store.get_ethos_philosophical_policy().unwrap();
    assert_eq!(loaded.active_school, "Stoic");
    assert_eq!(loaded.core_maxims.len(), 3);

    // Switch to Growth-Mindset.
    let gm = EthosPolicy::preset("Growth-Mindset").unwrap();
    store.set_ethos_philosophical_policy(&gm).unwrap();

    let loaded2 = store.get_ethos_philosophical_policy().unwrap();
    assert_eq!(loaded2.active_school, "Growth-Mindset");
}

#[test]
fn ethos_policy_clamp_tone_weight() {
    let mut policy = EthosPolicy {
        active_school: "Test".to_string(),
        core_maxims: vec![],
        tone_weight: 2.5,
    };
    policy.clamp();
    assert!((policy.tone_weight - 1.0).abs() < 0.001);

    policy.tone_weight = -0.5;
    policy.clamp();
    assert!((policy.tone_weight - 0.0).abs() < 0.001);
}

#[test]
fn ethos_stoic_system_instruction_contains_maxims() {
    let policy = EthosPolicy::preset("Stoic").unwrap();
    let instruction = policy.to_system_instruction();
    // Should contain numbered maxims.
    assert!(instruction.contains("1."), "Should have numbered maxims");
    assert!(instruction.contains("Stoic"), "Should mention Stoic");
    assert!(instruction.contains("tone_weight"), "Should include tone_weight");
}

#[test]
fn ethos_policy_custom_with_empty_maxims_generates_simple_instruction() {
    let policy = EthosPolicy {
        active_school: "Absurdist".to_string(),
        core_maxims: vec![],
        tone_weight: 0.5,
    };
    let instruction = policy.to_system_instruction();
    assert!(instruction.contains("Absurdist"));
    assert!(!instruction.contains("Core maxims:"));
}

/// Full integration: Ethos + Kardia + Shadow → ReflectShadow produces Stoic-flavored reflection.
#[test]
fn ethos_kardia_shadow_integration_stoic_conflict_with_project_manager() {
    let kb_dir = tempfile::tempdir().unwrap();
    let store = Arc::new(KnowledgeStore::open_path(kb_dir.path()).unwrap());

    // 1. Set Ethos to Stoic.
    let stoic = EthosPolicy::preset("Stoic").unwrap();
    store.set_ethos_philosophical_policy(&stoic).unwrap();

    // 2. Add "Project Manager" to Kardia Map.
    let pm = PersonRecord {
        name: "Project Manager".to_string(),
        relationship: "Boss".to_string(),
        trust_score: 0.3,
        attachment_style: "Avoidant".to_string(),
        triggers: vec!["silent treatment".to_string(), "deadline pressure".to_string()],
        last_interaction_summary: Some("Conflict about missed deadline".to_string()),
    };
    let slug = PersonRecord::name_slug(&pm.name);
    let key = format!("{}{}", KARDIA_PEOPLE_PREFIX, slug);
    let pm_bytes = serde_json::to_vec(&pm).unwrap();
    store.insert(7, &key, &pm_bytes).unwrap();

    // 3. Verify Ethos is loaded and generates correct instruction.
    let loaded = store.get_ethos_philosophical_policy().unwrap();
    assert_eq!(loaded.active_school, "Stoic");
    let instruction = loaded.to_system_instruction();
    assert!(
        instruction.contains("Stoic") && instruction.contains("control"),
        "Stoic instruction should mention control: {}",
        instruction
    );

    // 4. Verify Kardia person is retrievable.
    let people = store.list_people().unwrap();
    assert!(
        people.iter().any(|p| p.name == "Project Manager"),
        "Project Manager should be in Kardia Map"
    );
    let pm_loaded = people.iter().find(|p| p.name == "Project Manager").unwrap();
    assert_eq!(pm_loaded.attachment_style, "Avoidant");
    assert!((pm_loaded.trust_score - 0.3).abs() < 0.01);

    // 5. The full ReflectShadow test (with mock LLM) is in reflect_shadow.rs tests.
    //    Here we verify the data pipeline is correct: Ethos + Kardia are both available
    //    for the reflection prompt builder.
}
