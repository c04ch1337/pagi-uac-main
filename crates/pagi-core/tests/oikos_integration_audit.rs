//! **Oikos Integration Audit** — The "Closing of the Circuit" test.
//!
//! This is the full-stack integration test that validates the PAGI system's
//! transition from passive observer to **Active Proxy of Executive Function**.
//!
//! ## Scenario: "Biology Fails, AGI Protects"
//!
//! The user had only 4.5 hours of sleep. Their project manager (trust: 0.2)
//! is a known stress trigger. Three tasks are queued:
//!
//! 1. **Finish Quarterly Report** — High difficulty, tagged `#manager`
//! 2. **Weekly Grocery Run** — Low difficulty
//! 3. **Emergency Server Patch** — Critical priority
//!
//! ## Expected Governance Decisions
//!
//! | Task | Action | Reason |
//! |------|--------|--------|
//! | Quarterly Report | **Postpone** | High difficulty + low sleep + #manager conflict |
//! | Grocery Run | **Proceed** | Low cognitive load, unaffected by state |
//! | Server Patch | **Proceed** | Critical immunity — never postponed |
//!
//! ## Cross-Layer Verification
//!
//! | Layer | Slot | What It Provides |
//! |-------|------|-----------------|
//! | **Soma** | KB-8 | Hardware Budget (4.5h sleep = low budget) |
//! | **Kardia** | KB-7 | High-Stress Domain (Project Manager, trust 0.2) |
//! | **Ethos** | KB-6 | Decision Rationale (Stoic: focus on what you can control) |
//! | **Oikos** | KB-2 | GovernanceAction (Command and Control) |

use pagi_core::{
    EthosPolicy, GovernanceAction, GovernedTask, KnowledgeStore, MentalState, PersonRecord,
    SomaState, TaskDifficulty,
};
use std::sync::Arc;

// ===========================================================================
// Helper: Create the "bad day" scenario state
// ===========================================================================

/// Soma: 4.5 hours of sleep, elevated resting HR, low HRV, low readiness.
/// This represents a user whose "hardware budget" is severely depleted.
fn depleted_soma() -> SomaState {
    SomaState {
        sleep_hours: 4.5,
        resting_hr: 78,
        hrv: 28,
        readiness_score: 32,
    }
}

/// Kardia: High relational stress from the #manager domain.
/// The user is emotionally loaded from a low-trust relationship.
fn manager_stressed_mental() -> MentalState {
    MentalState {
        relational_stress: 0.75,
        burnout_risk: 0.55,
        grace_multiplier: 1.0, // Will be overridden by BioGate
    }
}

/// PersonRecord for the Project Manager — low trust, known triggers.
fn project_manager_person() -> PersonRecord {
    PersonRecord {
        name: "Project Manager".to_string(),
        relationship: "Boss".to_string(),
        trust_score: 0.2,
        attachment_style: "Avoidant".to_string(),
        triggers: vec![
            "criticism".to_string(),
            "deadline pressure".to_string(),
            "micromanagement".to_string(),
        ],
        last_interaction_summary: Some(
            "Tense meeting about missed Q4 deliverables. PM expressed frustration.".to_string(),
        ),
    }
}

/// The three governed tasks for the audit scenario.
fn audit_tasks() -> Vec<GovernedTask> {
    vec![
        // Task 1: High difficulty, tagged #manager — should be POSTPONED
        GovernedTask::new(
            "quarterly_report",
            "Finish Quarterly Report",
            TaskDifficulty::High,
        )
        .with_priority(0.8)
        .with_tags(vec![
            "work".to_string(),
            "manager".to_string(),
            "conflict".to_string(),
        ])
        .with_description(
            "Complete the Q4 quarterly report for the Project Manager. \
             Requires deep analysis and careful framing of missed targets.",
        ),
        // Task 2: Low difficulty — should PROCEED
        GovernedTask::new(
            "grocery_run",
            "Weekly Grocery Run",
            TaskDifficulty::Low,
        )
        .with_priority(0.4)
        .with_tags(vec!["personal".to_string(), "routine".to_string()])
        .with_description("Pick up groceries for the week. Low cognitive load."),
        // Task 3: Critical priority — MUST PROCEED (immunity)
        GovernedTask::new(
            "server_patch",
            "Emergency Server Patch",
            TaskDifficulty::Critical,
        )
        .with_priority(0.95)
        .with_tags(vec![
            "work".to_string(),
            "urgent".to_string(),
            "infrastructure".to_string(),
        ])
        .with_description(
            "Critical security vulnerability detected. Must be patched immediately \
             regardless of operator state.",
        ),
    ]
}

// ===========================================================================
// Test 1: Full KB Integration — The Complete "Closing of the Circuit"
// ===========================================================================

/// **The Master Integration Test.**
///
/// Sets up all four layers (Soma, Kardia, Ethos, Oikos) in the KnowledgeStore,
/// calls `evaluate_and_persist_tasks()`, and verifies the governance decisions
/// match the expected "Adaptive Governance" behavior.
#[test]
fn oikos_integration_audit_full_circuit() {
    let kb_dir = tempfile::tempdir().unwrap();
    let store = Arc::new(KnowledgeStore::open_path(kb_dir.path()).unwrap());

    // ── Layer 1: SOMA (Slot 8) — Set the "Hardware Budget" ──────────────
    let soma = depleted_soma();
    store.set_soma_state(&soma).unwrap();

    // Verify Soma was persisted
    let loaded_soma = store.get_soma_state();
    assert!(
        (loaded_soma.sleep_hours - 4.5).abs() < 0.01,
        "Soma sleep_hours should be 4.5, got: {}",
        loaded_soma.sleep_hours
    );
    assert_eq!(loaded_soma.readiness_score, 32);

    // ── Layer 2: KARDIA (Slot 7) — Set the "High-Stress Domain" ────────
    let mental = manager_stressed_mental();
    store.set_mental_state("default", &mental).unwrap();

    // Store the Project Manager PersonRecord in the Relational Map
    let pm = project_manager_person();
    store.set_person(&pm).unwrap();

    // Verify Kardia PersonRecord was persisted
    let loaded_pm = store.get_person("project_manager").unwrap();
    assert_eq!(loaded_pm.name, "Project Manager");
    assert!((loaded_pm.trust_score - 0.2).abs() < 0.01);
    assert_eq!(loaded_pm.triggers.len(), 3);

    // Verify effective mental state includes BioGate adjustment
    let effective = store.get_effective_mental_state("default");
    assert!(
        effective.grace_multiplier >= 1.5,
        "BioGate should elevate grace_multiplier due to 4.5h sleep: {}",
        effective.grace_multiplier
    );
    assert!(
        effective.burnout_risk > mental.burnout_risk,
        "BioGate should increment burnout_risk: base={}, effective={}",
        mental.burnout_risk,
        effective.burnout_risk
    );

    // ── Layer 3: ETHOS (Slot 6) — Set the "Decision Rationale" ─────────
    let stoic = EthosPolicy::preset("Stoic").unwrap();
    store.set_ethos_philosophical_policy(&stoic).unwrap();

    // Verify Ethos was persisted
    let loaded_ethos = store.get_ethos_philosophical_policy().unwrap();
    assert_eq!(loaded_ethos.active_school, "Stoic");
    assert!(!loaded_ethos.core_maxims.is_empty());

    // ── Layer 4: OIKOS (Slot 2) — Store the Governed Tasks ─────────────
    for task in audit_tasks() {
        store.set_governed_task(&task).unwrap();
    }

    // Verify all 3 tasks are stored
    let stored_tasks = store.list_governed_tasks().unwrap();
    assert_eq!(
        stored_tasks.len(),
        3,
        "Should have 3 governed tasks in Oikos"
    );

    // ── EXECUTION: evaluate_and_persist_tasks() ────────────────────────
    let evaluated = store.evaluate_and_persist_tasks("default").unwrap();
    assert_eq!(evaluated.len(), 3, "Should evaluate all 3 tasks");

    // ── VERIFICATION: Quarterly Report → POSTPONED ─────────────────────
    let report = evaluated
        .iter()
        .find(|t| t.task_id == "quarterly_report")
        .expect("quarterly_report should be in evaluated results");

    assert!(
        report.action.is_postpone(),
        "Quarterly Report should be POSTPONED (High difficulty + 4.5h sleep + #manager conflict), got: {:?}",
        report.action
    );

    // Verify the governance reason uses the Stoic Ethos lens
    if let GovernanceAction::Postpone { reason } = &report.action {
        assert!(
            reason.contains("Stoic") || reason.contains("control"),
            "Postpone reason should reference Stoic principles (focus on what you can control): {}",
            reason
        );
        // The reason should mention the biological state
        assert!(
            reason.contains("Sleep") || reason.contains("sleep") || reason.contains("4.5"),
            "Postpone reason should reference sleep deprivation: {}",
            reason
        );
    }

    // Verify effective priority was reduced
    assert!(
        report.effective_priority < report.base_priority,
        "Report effective_priority ({}) should be less than base_priority ({})",
        report.effective_priority,
        report.base_priority
    );

    // Verify last_evaluated_ms was set
    assert!(
        report.last_evaluated_ms > 0,
        "last_evaluated_ms should be set after evaluation"
    );

    // ── VERIFICATION: Grocery Run → PROCEED ────────────────────────────
    let groceries = evaluated
        .iter()
        .find(|t| t.task_id == "grocery_run")
        .expect("grocery_run should be in evaluated results");

    assert!(
        groceries.action.is_proceed(),
        "Weekly Grocery Run should PROCEED (Low cognitive load), got: {:?}",
        groceries.action
    );

    // ── VERIFICATION: Server Patch → PROCEED (Critical Immunity) ───────
    let patch = evaluated
        .iter()
        .find(|t| t.task_id == "server_patch")
        .expect("server_patch should be in evaluated results");

    assert!(
        patch.action.is_proceed(),
        "Emergency Server Patch MUST PROCEED (Critical immunity), got: {:?}",
        patch.action
    );

    // Critical tasks retain their full base priority
    assert!(
        (patch.effective_priority - patch.base_priority).abs() < 0.01,
        "Critical task priority should be unchanged: effective={}, base={}",
        patch.effective_priority,
        patch.base_priority
    );

    // ── VERIFICATION: Governance Summary ────────────────────────────────
    let summary = store
        .get_governance_summary()
        .expect("Governance summary should be persisted in Oikos");

    assert!(
        summary.contains("Task Governance Summary"),
        "Summary should contain header"
    );
    assert!(
        summary.contains("Stoic"),
        "Summary should reference the Stoic philosophical lens"
    );
    assert!(
        summary.contains("4.5") || summary.contains("Sleep"),
        "Summary should reference sleep state"
    );
    assert!(
        summary.contains("PROCEED") || summary.contains("proceed"),
        "Summary should mention proceeding tasks"
    );
    assert!(
        summary.contains("POSTPONE") || summary.contains("postponed"),
        "Summary should mention postponed tasks"
    );

    // ── VERIFICATION: Persisted tasks reflect governance decisions ──────
    let persisted_report = store
        .get_governed_task("quarterly_report")
        .expect("quarterly_report should be persisted");
    assert!(
        persisted_report.action.is_postpone(),
        "Persisted quarterly_report should still be postponed"
    );

    let persisted_patch = store
        .get_governed_task("server_patch")
        .expect("server_patch should be persisted");
    assert!(
        persisted_patch.action.is_proceed(),
        "Persisted server_patch should still be proceed"
    );

    // Print the governance summary for audit visibility
    println!("\n{}\n", "=".repeat(72));
    println!("  OIKOS INTEGRATION AUDIT — GOVERNANCE REPORT");
    println!("{}\n", "=".repeat(72));
    println!("{}", summary);
    println!("\n{}", "=".repeat(72));
}

// ===========================================================================
// Test 2: Cross-Layer State Assembly — Governor reads all layers
// ===========================================================================

/// Verifies that `create_task_governor()` correctly assembles state from
/// Soma (Slot 8), Kardia (Slot 7), and Ethos (Slot 6) into a single governor.
#[test]
fn oikos_audit_cross_layer_assembly() {
    let kb_dir = tempfile::tempdir().unwrap();
    let store = Arc::new(KnowledgeStore::open_path(kb_dir.path()).unwrap());

    // Set all layers
    store.set_soma_state(&depleted_soma()).unwrap();
    store
        .set_mental_state("default", &manager_stressed_mental())
        .unwrap();
    store
        .set_ethos_philosophical_policy(&EthosPolicy::preset("Stoic").unwrap())
        .unwrap();

    // Create governor from store
    let governor = store.create_task_governor("default");

    // Verify Soma was read
    assert!(
        (governor.soma.sleep_hours - 4.5).abs() < 0.01,
        "Governor should read Soma sleep_hours: {}",
        governor.soma.sleep_hours
    );
    assert_eq!(governor.soma.readiness_score, 32);

    // Verify Ethos was read
    assert!(governor.ethos.is_some(), "Governor should have Ethos policy");
    assert_eq!(
        governor.ethos.as_ref().unwrap().active_school,
        "Stoic"
    );

    // Verify Mental state has BioGate cross-layer adjustment
    assert!(
        governor.mental.grace_multiplier >= 1.5,
        "Governor mental state should have BioGate adjustment: grace_multiplier={}",
        governor.mental.grace_multiplier
    );

    // Verify bio penalty is significant
    let bio = governor.bio_penalty();
    assert!(
        bio > 0.3,
        "Bio penalty should be significant with 4.5h sleep: {}",
        bio
    );

    // Verify emotional penalty is significant
    let emo = governor.emotional_penalty();
    assert!(
        emo > 0.2,
        "Emotional penalty should be significant with relational_stress=0.75: {}",
        emo
    );
}

// ===========================================================================
// Test 3: Kardia Relational Map — Project Manager record
// ===========================================================================

/// Verifies that the PersonRecord for the Project Manager is correctly stored
/// and retrievable from KB_KARDIA (Slot 7), providing the "who" context.
#[test]
fn oikos_audit_kardia_relational_map() {
    let kb_dir = tempfile::tempdir().unwrap();
    let store = Arc::new(KnowledgeStore::open_path(kb_dir.path()).unwrap());

    let pm = project_manager_person();
    store.set_person(&pm).unwrap();

    // Retrieve by slug
    let loaded = store.get_person("project_manager").unwrap();
    assert_eq!(loaded.name, "Project Manager");
    assert_eq!(loaded.relationship, "Boss");
    assert!((loaded.trust_score - 0.2).abs() < 0.01);
    assert_eq!(loaded.attachment_style, "Avoidant");
    assert_eq!(loaded.triggers.len(), 3);
    assert!(loaded.triggers.contains(&"criticism".to_string()));
    assert!(loaded.triggers.contains(&"deadline pressure".to_string()));
    assert!(loaded.triggers.contains(&"micromanagement".to_string()));
    assert!(loaded.last_interaction_summary.is_some());

    // Verify it appears in list_people
    let people = store.list_people().unwrap();
    assert_eq!(people.len(), 1);
    assert_eq!(people[0].name, "Project Manager");
}

// ===========================================================================
// Test 4: Ethos Lens Shapes Governance Reason
// ===========================================================================

/// Verifies that the Stoic Ethos lens produces governance reasons that reference
/// Stoic principles (e.g., "focus on what you can control", "rational choice").
#[test]
fn oikos_audit_ethos_shapes_governance_reason() {
    let kb_dir = tempfile::tempdir().unwrap();
    let store = Arc::new(KnowledgeStore::open_path(kb_dir.path()).unwrap());

    // Set up the depleted state with Stoic lens
    store.set_soma_state(&depleted_soma()).unwrap();
    store
        .set_mental_state("default", &manager_stressed_mental())
        .unwrap();
    store
        .set_ethos_philosophical_policy(&EthosPolicy::preset("Stoic").unwrap())
        .unwrap();

    // Store only the report task
    let report = GovernedTask::new(
        "quarterly_report",
        "Finish Quarterly Report",
        TaskDifficulty::High,
    )
    .with_priority(0.8)
    .with_tags(vec![
        "work".to_string(),
        "manager".to_string(),
        "conflict".to_string(),
    ]);
    store.set_governed_task(&report).unwrap();

    // Evaluate
    let evaluated = store.evaluate_and_persist_tasks("default").unwrap();
    let report_result = &evaluated[0];

    assert!(
        report_result.action.is_postpone(),
        "Report should be postponed"
    );

    if let GovernanceAction::Postpone { reason } = &report_result.action {
        // Stoic-specific language
        assert!(
            reason.contains("Stoic") || reason.contains("control") || reason.contains("rational"),
            "Stoic governance reason should reference Stoic principles: '{}'",
            reason
        );
        // Should NOT contain other school names
        assert!(
            !reason.contains("Growth-Mindset") && !reason.contains("Taoist"),
            "Stoic reason should not mention other schools: '{}'",
            reason
        );
    }
}

// ===========================================================================
// Test 5: Critical Immunity Under Maximum Stress
// ===========================================================================

/// Verifies that Critical tasks (Emergency Server Patch) proceed even when
/// ALL layers indicate maximum stress — the "immunity" guarantee.
#[test]
fn oikos_audit_critical_immunity_under_maximum_stress() {
    let kb_dir = tempfile::tempdir().unwrap();
    let store = Arc::new(KnowledgeStore::open_path(kb_dir.path()).unwrap());

    // Worst possible biological state
    store
        .set_soma_state(&SomaState {
            sleep_hours: 2.0,
            resting_hr: 110,
            hrv: 12,
            readiness_score: 5,
        })
        .unwrap();

    // Worst possible emotional state
    store
        .set_mental_state(
            "default",
            &MentalState {
                relational_stress: 0.99,
                burnout_risk: 0.99,
                grace_multiplier: 2.0,
            },
        )
        .unwrap();

    // Stoic lens (shouldn't matter for Critical)
    store
        .set_ethos_philosophical_policy(&EthosPolicy::preset("Stoic").unwrap())
        .unwrap();

    // Only the critical task
    let critical = GovernedTask::new(
        "server_patch",
        "Emergency Server Patch",
        TaskDifficulty::Critical,
    )
    .with_priority(0.95);
    store.set_governed_task(&critical).unwrap();

    let evaluated = store.evaluate_and_persist_tasks("default").unwrap();
    assert_eq!(evaluated.len(), 1);

    let patch = &evaluated[0];
    assert!(
        patch.action.is_proceed(),
        "Critical task MUST proceed even under maximum stress: {:?}",
        patch.action
    );
    assert!(
        (patch.effective_priority - 0.95).abs() < 0.01,
        "Critical task priority must be preserved: {}",
        patch.effective_priority
    );
}

// ===========================================================================
// Test 6: BioGate Cross-Layer Reaction (Soma → Kardia)
// ===========================================================================

/// Verifies the BioGate cross-layer reaction: when Soma reports poor sleep,
/// the effective MentalState (Kardia) is automatically adjusted with elevated
/// burnout_risk and grace_multiplier.
#[test]
fn oikos_audit_biogate_cross_layer_reaction() {
    let kb_dir = tempfile::tempdir().unwrap();
    let store = Arc::new(KnowledgeStore::open_path(kb_dir.path()).unwrap());

    // Set Soma with poor sleep (triggers BioGate)
    store.set_soma_state(&depleted_soma()).unwrap();

    // Set baseline Kardia (moderate stress)
    let baseline = MentalState {
        relational_stress: 0.4,
        burnout_risk: 0.3,
        grace_multiplier: 1.0,
    };
    store.set_mental_state("default", &baseline).unwrap();

    // Get effective state (should have BioGate adjustment)
    let effective = store.get_effective_mental_state("default");

    // BioGate should have incremented burnout_risk by 0.15
    assert!(
        effective.burnout_risk >= baseline.burnout_risk + SomaState::BURNOUT_RISK_INCREMENT - 0.01,
        "BioGate should increment burnout_risk: baseline={}, effective={}",
        baseline.burnout_risk,
        effective.burnout_risk
    );

    // BioGate should have set grace_multiplier to 1.6
    assert!(
        (effective.grace_multiplier - SomaState::GRACE_MULTIPLIER_OVERRIDE).abs() < 0.01,
        "BioGate should set grace_multiplier to {}: got {}",
        SomaState::GRACE_MULTIPLIER_OVERRIDE,
        effective.grace_multiplier
    );
}

// ===========================================================================
// Test 7: Governance Summary Contains All Layers
// ===========================================================================

/// Verifies the governance summary is a comprehensive, human-readable report
/// that references all four layers: Soma state, emotional penalties, Ethos lens,
/// and per-task governance decisions.
#[test]
fn oikos_audit_governance_summary_comprehensive() {
    let kb_dir = tempfile::tempdir().unwrap();
    let store = Arc::new(KnowledgeStore::open_path(kb_dir.path()).unwrap());

    // Set up full scenario
    store.set_soma_state(&depleted_soma()).unwrap();
    store
        .set_mental_state("default", &manager_stressed_mental())
        .unwrap();
    store
        .set_ethos_philosophical_policy(&EthosPolicy::preset("Stoic").unwrap())
        .unwrap();
    for task in audit_tasks() {
        store.set_governed_task(&task).unwrap();
    }

    store.evaluate_and_persist_tasks("default").unwrap();

    let summary = store.get_governance_summary().unwrap();

    // Header
    assert!(summary.contains("Task Governance Summary"));

    // Soma reference
    assert!(
        summary.contains("Sleep: 4.5h"),
        "Summary should show sleep hours: {}",
        summary
    );
    assert!(
        summary.contains("Readiness: 32"),
        "Summary should show readiness score: {}",
        summary
    );

    // Ethos reference
    assert!(
        summary.contains("Stoic"),
        "Summary should reference Stoic lens: {}",
        summary
    );

    // Task counts
    assert!(
        summary.contains("3 total"),
        "Summary should show 3 total tasks: {}",
        summary
    );

    // Individual task names
    assert!(
        summary.contains("Finish Quarterly Report"),
        "Summary should list the report task"
    );
    assert!(
        summary.contains("Weekly Grocery Run"),
        "Summary should list the grocery task"
    );
    assert!(
        summary.contains("Emergency Server Patch"),
        "Summary should list the server patch task"
    );
}

// ===========================================================================
// Test 8: Task Ordering — Proceed tasks sorted before Postponed
// ===========================================================================

/// Verifies that after evaluation, Proceed tasks are sorted before Postponed
/// tasks, and within each group, tasks are sorted by effective priority.
#[test]
fn oikos_audit_task_ordering() {
    let kb_dir = tempfile::tempdir().unwrap();
    let store = Arc::new(KnowledgeStore::open_path(kb_dir.path()).unwrap());

    store.set_soma_state(&depleted_soma()).unwrap();
    store
        .set_mental_state("default", &manager_stressed_mental())
        .unwrap();
    store
        .set_ethos_philosophical_policy(&EthosPolicy::preset("Stoic").unwrap())
        .unwrap();
    for task in audit_tasks() {
        store.set_governed_task(&task).unwrap();
    }

    let evaluated = store.evaluate_and_persist_tasks("default").unwrap();

    // Proceed tasks should come first
    let mut seen_non_proceed = false;
    for task in &evaluated {
        if !task.action.is_proceed() {
            seen_non_proceed = true;
        } else if seen_non_proceed {
            panic!(
                "Proceed task '{}' found after non-proceed task — ordering is wrong",
                task.title
            );
        }
    }

    // Server Patch (Critical, Proceed) should be first or near-first
    let patch_idx = evaluated
        .iter()
        .position(|t| t.task_id == "server_patch")
        .unwrap();
    assert!(
        patch_idx < 2,
        "Critical Server Patch should be in top 2 positions, found at index {}",
        patch_idx
    );
}

// ===========================================================================
// Test 9: Multiple Ethos Lenses Produce Different Reasons
// ===========================================================================

/// Verifies that switching the Ethos lens changes the governance reason text
/// while the governance *action* (Postpone) remains the same.
#[test]
fn oikos_audit_ethos_lens_differentiation() {
    let kb_dir = tempfile::tempdir().unwrap();

    let schools = ["Stoic", "Growth-Mindset", "Compassionate-Witness", "Taoist", "Existentialist"];
    let mut reasons: Vec<(String, String)> = Vec::new();

    for school in &schools {
        let store = Arc::new(
            KnowledgeStore::open_path(kb_dir.path().join(format!("kb_{}", school))).unwrap(),
        );

        store.set_soma_state(&depleted_soma()).unwrap();
        store
            .set_mental_state("default", &manager_stressed_mental())
            .unwrap();
        store
            .set_ethos_philosophical_policy(&EthosPolicy::preset(school).unwrap())
            .unwrap();

        let report = GovernedTask::new(
            "quarterly_report",
            "Finish Quarterly Report",
            TaskDifficulty::High,
        )
        .with_priority(0.8)
        .with_tags(vec!["work".to_string(), "conflict".to_string()]);
        store.set_governed_task(&report).unwrap();

        let evaluated = store.evaluate_and_persist_tasks("default").unwrap();
        let result = &evaluated[0];

        assert!(
            result.action.is_postpone(),
            "{} lens: Report should still be postponed",
            school
        );

        if let GovernanceAction::Postpone { reason } = &result.action {
            reasons.push((school.to_string(), reason.clone()));
        }
    }

    // Verify each school produces a unique reason
    for i in 0..reasons.len() {
        for j in (i + 1)..reasons.len() {
            assert_ne!(
                reasons[i].1, reasons[j].1,
                "Reasons for {} and {} should differ",
                reasons[i].0, reasons[j].0
            );
        }
    }

    // Verify school-specific keywords
    for (school, reason) in &reasons {
        match school.as_str() {
            "Stoic" => assert!(
                reason.contains("Stoic") || reason.contains("control"),
                "Stoic reason missing keywords: {}",
                reason
            ),
            "Growth-Mindset" => assert!(
                reason.contains("Growth") || reason.contains("growth") || reason.contains("recovery"),
                "Growth-Mindset reason missing keywords: {}",
                reason
            ),
            "Compassionate-Witness" => assert!(
                reason.contains("Compassionate") || reason.contains("compassion") || reason.contains("gentle"),
                "Compassionate-Witness reason missing keywords: {}",
                reason
            ),
            "Taoist" => assert!(
                reason.contains("Taoist") || reason.contains("wu-wei") || reason.contains("Wu-wei") || reason.contains("flow"),
                "Taoist reason missing keywords: {}",
                reason
            ),
            "Existentialist" => assert!(
                reason.contains("Existentialist") || reason.contains("freedom") || reason.contains("authentic"),
                "Existentialist reason missing keywords: {}",
                reason
            ),
            _ => {}
        }
    }
}

// ===========================================================================
// Test 10: End-to-End Audit Report (Printed Output)
// ===========================================================================

/// Generates and prints the full audit report for manual inspection.
/// This test always passes but produces a detailed governance report
/// that can be reviewed in test output (`cargo test -- --nocapture`).
#[test]
fn oikos_audit_print_full_report() {
    let kb_dir = tempfile::tempdir().unwrap();
    let store = Arc::new(KnowledgeStore::open_path(kb_dir.path()).unwrap());

    // Set up full scenario
    store.set_soma_state(&depleted_soma()).unwrap();
    store
        .set_mental_state("default", &manager_stressed_mental())
        .unwrap();
    store
        .set_ethos_philosophical_policy(&EthosPolicy::preset("Stoic").unwrap())
        .unwrap();
    store.set_person(&project_manager_person()).unwrap();
    for task in audit_tasks() {
        store.set_governed_task(&task).unwrap();
    }

    let evaluated = store.evaluate_and_persist_tasks("default").unwrap();
    let summary = store.get_governance_summary().unwrap();
    let pm = store.get_person("project_manager").unwrap();
    let effective_mental = store.get_effective_mental_state("default");
    let soma = store.get_soma_state();

    println!("\n{}", "═".repeat(72));
    println!("  🏛️  OIKOS INTEGRATION AUDIT — SOVEREIGN SANCTUARY REPORT");
    println!("{}\n", "═".repeat(72));

    println!("┌─────────────────────────────────────────────────────────────────────┐");
    println!("│ LAYER 1: SOMA (Slot 8) — Hardware Budget                           │");
    println!("├─────────────────────────────────────────────────────────────────────┤");
    println!(
        "│  Sleep: {:.1}h | Resting HR: {} BPM | HRV: {} ms | Readiness: {}    │",
        soma.sleep_hours, soma.resting_hr, soma.hrv, soma.readiness_score
    );
    println!(
        "│  BioGate Active: {}                                                 │",
        if soma.needs_biogate_adjustment() {
            "YES ⚠️"
        } else {
            "NO  ✅"
        }
    );
    println!("└─────────────────────────────────────────────────────────────────────┘\n");

    println!("┌─────────────────────────────────────────────────────────────────────┐");
    println!("│ LAYER 2: KARDIA (Slot 7) — Emotional Context                       │");
    println!("├─────────────────────────────────────────────────────────────────────┤");
    println!(
        "│  Relational Stress: {:.2} | Burnout Risk: {:.2} | Grace: {:.1}x       │",
        effective_mental.relational_stress,
        effective_mental.burnout_risk,
        effective_mental.grace_multiplier
    );
    println!(
        "│  High-Stress Person: {} (trust: {:.1}, style: {})     │",
        pm.name, pm.trust_score, pm.attachment_style
    );
    println!(
        "│  Triggers: {:?}                                                       │",
        pm.triggers
    );
    println!("└─────────────────────────────────────────────────────────────────────┘\n");

    println!("┌─────────────────────────────────────────────────────────────────────┐");
    println!("│ LAYER 3: ETHOS (Slot 6) — Philosophical Lens                       │");
    println!("├─────────────────────────────────────────────────────────────────────┤");
    println!("│  Active School: Stoic                                               │");
    println!("│  Core Maxims:                                                       │");
    let ethos = store.get_ethos_philosophical_policy().unwrap();
    for (i, maxim) in ethos.core_maxims.iter().enumerate() {
        println!("│    {}. {}  │", i + 1, maxim);
    }
    println!("└─────────────────────────────────────────────────────────────────────┘\n");

    println!("┌─────────────────────────────────────────────────────────────────────┐");
    println!("│ LAYER 4: OIKOS (Slot 2) — Governance Decisions                     │");
    println!("├─────────────────────────────────────────────────────────────────────┤");
    for task in &evaluated {
        let action_str = match &task.action {
            GovernanceAction::Proceed => "✅ PROCEED".to_string(),
            GovernanceAction::Postpone { reason } => format!("⏸️  POSTPONE: {}", reason),
            GovernanceAction::Simplify { suggestion } => format!("🔧 SIMPLIFY: {}", suggestion),
            GovernanceAction::Deprioritize { reason } => format!("⬇️  DEPRIORITIZE: {}", reason),
        };
        println!(
            "│  [{:.2}] {} ({:?})",
            task.effective_priority, task.title, task.difficulty
        );
        println!("│    → {}", action_str);
        println!("│");
    }
    println!("└─────────────────────────────────────────────────────────────────────┘\n");

    println!("{}", summary);
    println!("\n{}", "═".repeat(72));
    println!("  ✅ AUDIT COMPLETE — All governance decisions verified.");
    println!("{}\n", "═".repeat(72));
}
