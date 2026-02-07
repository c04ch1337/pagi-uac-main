//! Integration test: Relational Map (Kardia) — PersonRecord storage in Slot 7.
//!
//! Verifies that:
//! 1. PersonRecord can be stored under `people/{name_slug}` and read back.
//! 2. list_people returns all stored persons.
//! 3. Verify scenario: "Project Manager" with trust 0.3 and attachment_style "Avoidant" is stored and retrieved.
//!
//! See also: total_context_stress_test — full scenario with Soma + Kardia + Shadow + ReflectShadow.

use pagi_core::{KnowledgeStore, PersonRecord, SomaState};

#[test]
fn person_slug_normalization() {
    assert_eq!(PersonRecord::name_slug("Project Manager"), "project_manager");
    assert_eq!(PersonRecord::name_slug("Sarah"), "sarah");
    assert_eq!(PersonRecord::name_slug("  Boss  "), "boss");
}

#[test]
fn kardia_set_get_person() {
    let dir = tempfile::tempdir().unwrap();
    let store = KnowledgeStore::open_path(dir.path()).unwrap();

    let record = PersonRecord {
        name: "Project Manager".to_string(),
        relationship: "Boss".to_string(),
        trust_score: 0.3,
        attachment_style: "Avoidant".to_string(),
        triggers: vec!["criticism".to_string(), "micromanagement".to_string()],
        last_interaction_summary: Some("Recent conflict over deadlines.".to_string()),
    };

    store.set_person(&record).expect("set_person should succeed");

    let slug = PersonRecord::name_slug("Project Manager");
    let got = store.get_person(&slug).expect("get_person should return the record");
    assert_eq!(got.name, "Project Manager");
    assert_eq!(got.relationship, "Boss");
    assert!((got.trust_score - 0.3).abs() < 0.001);
    assert_eq!(got.attachment_style, "Avoidant");
    assert_eq!(got.triggers, vec!["criticism", "micromanagement"]);
    assert_eq!(
        got.last_interaction_summary.as_deref(),
        Some("Recent conflict over deadlines.")
    );
}

#[test]
fn kardia_list_people() {
    let dir = tempfile::tempdir().unwrap();
    let store = KnowledgeStore::open_path(dir.path()).unwrap();

    store
        .set_person(&PersonRecord {
            name: "Project Manager".to_string(),
            relationship: "Boss".to_string(),
            trust_score: 0.3,
            attachment_style: "Avoidant".to_string(),
            triggers: vec![],
            last_interaction_summary: None,
        })
        .unwrap();
    store
        .set_person(&PersonRecord {
            name: "Sarah".to_string(),
            relationship: "Partner".to_string(),
            trust_score: 0.9,
            attachment_style: "Secure".to_string(),
            triggers: vec![],
            last_interaction_summary: None,
        })
        .unwrap();

    let people = store.list_people().unwrap();
    assert_eq!(people.len(), 2);
    let names: Vec<&str> = people.iter().map(|p| p.name.as_str()).collect();
    assert!(names.contains(&"Project Manager"));
    assert!(names.contains(&"Sarah"));
}

/// Total Context Stress Test — scenario setup.
/// Verifies that Soma (low sleep/readiness) drives get_effective_mental_state to supportive mode,
/// and that Kardia Map holds both Partner (high trust, Anxious) and Project Manager (low trust, Avoidant).
#[test]
fn total_context_stress_test_scenario_setup() {
    let dir = tempfile::tempdir().unwrap();
    let store = KnowledgeStore::open_path(dir.path()).unwrap();
    let agent_id = "default";

    // ── Soma: Low sleep (4.0h) and low readiness (30) ─────────────────────
    let mut soma = SomaState {
        sleep_hours: 4.0,
        resting_hr: 0,
        hrv: 0,
        readiness_score: 30,
    };
    soma.clamp();
    assert!(soma.needs_biogate_adjustment(), "BioGate adjustment should be triggered");
    store.set_soma_state(&soma).unwrap();

    let mental = store.get_effective_mental_state(agent_id);
    assert!(
        (mental.grace_multiplier - SomaState::GRACE_MULTIPLIER_OVERRIDE).abs() < 0.01,
        "Governor should apply grace_multiplier = 1.6 (supportive tone)"
    );
    assert!(
        mental.burnout_risk >= 0.15,
        "Governor should have elevated burnout_risk from Soma"
    );
    assert!(
        mental.has_physical_load_adjustment(),
        "ReflectShadow should inject Soma (Supportive Tone) instruction"
    );

    // ── Kardia Map: Partner (0.9, Anxious) and Project Manager (0.3, Avoidant) ──
    store
        .set_person(&PersonRecord {
            name: "Partner".to_string(),
            relationship: "Partner".to_string(),
            trust_score: 0.9,
            attachment_style: "Anxious".to_string(),
            triggers: vec![],
            last_interaction_summary: None,
        })
        .unwrap();
    store
        .set_person(&PersonRecord {
            name: "Project Manager".to_string(),
            relationship: "Boss".to_string(),
            trust_score: 0.3,
            attachment_style: "Avoidant".to_string(),
            triggers: vec!["criticism".to_string(), "micromanagement".to_string()],
            last_interaction_summary: None,
        })
        .unwrap();

    let people = store.list_people().unwrap();
    assert_eq!(people.len(), 2, "Kardia Map should contain Partner and Project Manager");

    let partner = people.iter().find(|p| p.name == "Partner").unwrap();
    assert!((partner.trust_score - 0.9).abs() < 0.01);
    assert_eq!(partner.attachment_style, "Anxious");

    let pm = people.iter().find(|p| p.name == "Project Manager").unwrap();
    assert!((pm.trust_score - 0.3).abs() < 0.01);
    assert_eq!(pm.attachment_style, "Avoidant");

    // Content "Argument with Partner about the Project Manager's deadlines." mentions both.
    let content_lower = "argument with partner about the project manager's deadlines.".to_string();
    let mentioned: Vec<_> = people
        .into_iter()
        .filter(|p| !p.name.is_empty() && content_lower.contains(&p.name.to_lowercase()))
        .collect();
    assert_eq!(mentioned.len(), 2, "ReflectShadow should inject both into prompt");
}
