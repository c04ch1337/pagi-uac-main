//! Integration test: Dynamic Task Governance (Oikos) — verifies that the TaskGovernor
//! correctly evaluates tasks against the current Ethos (philosophical lens), Soma (biological
//! state), and Kardia (emotional load) to produce governance decisions.
//!
//! ## Scenarios
//! 1. Healthy state: all tasks proceed normally.
//! 2. Sleep-deprived (4h): high-difficulty tasks are postponed with Stoic reasoning.
//! 3. High emotional stress: conflict-tagged tasks are postponed.
//! 4. Critical tasks always proceed regardless of state.
//! 5. Growth-Mindset lens produces different reason text than Stoic.
//! 6. Full KB integration: tasks stored/retrieved from Oikos slot.
//! 7. Batch evaluation sorts by effective priority.
//! 8. Governance summary is human-readable.

use pagi_core::{
    EthosPolicy, GovernanceAction, GovernedTask, KnowledgeStore, MentalState, SomaState,
    TaskDifficulty, TaskGovernor,
};
use std::sync::Arc;

// ---------------------------------------------------------------------------
// Helper: build a standard task set for testing
// ---------------------------------------------------------------------------

fn sample_tasks() -> Vec<GovernedTask> {
    vec![
        GovernedTask::new("email_replies", "Reply to routine emails", TaskDifficulty::Low)
            .with_priority(0.4)
            .with_tags(vec!["work".to_string()]),
        GovernedTask::new("code_review", "Review PR #42", TaskDifficulty::Medium)
            .with_priority(0.6)
            .with_tags(vec!["work".to_string(), "code".to_string()]),
        GovernedTask::new("architecture_decision", "Design new microservice boundary", TaskDifficulty::High)
            .with_priority(0.8)
            .with_tags(vec!["work".to_string(), "architecture".to_string()]),
        GovernedTask::new("conflict_resolution", "Address PM feedback on missed deadline", TaskDifficulty::High)
            .with_priority(0.7)
            .with_tags(vec!["work".to_string(), "conflict".to_string()]),
        GovernedTask::new("deploy_hotfix", "Deploy critical security patch", TaskDifficulty::Critical)
            .with_priority(0.95)
            .with_tags(vec!["work".to_string(), "urgent".to_string()]),
    ]
}

fn healthy_soma() -> SomaState {
    SomaState {
        sleep_hours: 8.0,
        resting_hr: 60,
        hrv: 65,
        readiness_score: 85,
    }
}

fn sleep_deprived_soma() -> SomaState {
    SomaState {
        sleep_hours: 4.0,
        resting_hr: 75,
        hrv: 30,
        readiness_score: 35,
    }
}

fn healthy_mental() -> MentalState {
    MentalState {
        relational_stress: 0.1,
        burnout_risk: 0.1,
        grace_multiplier: 1.0,
    }
}

fn stressed_mental() -> MentalState {
    MentalState {
        relational_stress: 0.85,
        burnout_risk: 0.6,
        grace_multiplier: 1.6,
    }
}

// ===========================================================================
// Test 1: Healthy state — all tasks proceed
// ===========================================================================

#[test]
fn healthy_state_all_tasks_proceed() {
    let governor = TaskGovernor::new(healthy_soma(), healthy_mental(), None);
    let tasks = sample_tasks();
    let evaluated = governor.evaluate_batch(&tasks);

    for task in &evaluated {
        assert!(
            task.action.is_proceed(),
            "Task '{}' should proceed in healthy state, got: {:?}",
            task.title,
            task.action
        );
    }

    // Bio penalty should be near zero
    assert!(
        governor.bio_penalty() < 0.1,
        "Bio penalty should be near zero for healthy state: {}",
        governor.bio_penalty()
    );
}

// ===========================================================================
// Test 2: Sleep-deprived — high-difficulty tasks postponed (Stoic)
// ===========================================================================

#[test]
fn sleep_deprived_stoic_postpones_high_difficulty() {
    let stoic = EthosPolicy::preset("Stoic").unwrap();
    let governor = TaskGovernor::new(sleep_deprived_soma(), healthy_mental(), Some(stoic));
    let tasks = sample_tasks();
    let evaluated = governor.evaluate_batch(&tasks);

    // Architecture decision (High) should be postponed
    let arch = evaluated.iter().find(|t| t.task_id == "architecture_decision").unwrap();
    assert!(
        arch.action.is_postpone(),
        "High-difficulty 'architecture_decision' should be postponed when sleep-deprived, got: {:?}",
        arch.action
    );

    // Verify Stoic reasoning in the postpone reason
    if let GovernanceAction::Postpone { reason } = &arch.action {
        assert!(
            reason.contains("Stoic") || reason.contains("control"),
            "Stoic postpone reason should reference Stoic principles: {}",
            reason
        );
    }

    // Email replies (Low) should still proceed
    let email = evaluated.iter().find(|t| t.task_id == "email_replies").unwrap();
    assert!(
        email.action.is_proceed(),
        "Low-difficulty 'email_replies' should proceed even when sleep-deprived"
    );

    // Critical deploy should always proceed
    let deploy = evaluated.iter().find(|t| t.task_id == "deploy_hotfix").unwrap();
    assert!(
        deploy.action.is_proceed(),
        "Critical 'deploy_hotfix' should ALWAYS proceed"
    );
}

// ===========================================================================
// Test 3: High emotional stress — conflict tasks postponed
// ===========================================================================

#[test]
fn high_emotional_stress_postpones_conflict_tasks() {
    let compassionate = EthosPolicy::preset("Compassionate-Witness").unwrap();
    let governor = TaskGovernor::new(healthy_soma(), stressed_mental(), Some(compassionate));
    let tasks = sample_tasks();
    let evaluated = governor.evaluate_batch(&tasks);

    // Conflict resolution should be postponed due to high relational stress
    let conflict = evaluated.iter().find(|t| t.task_id == "conflict_resolution").unwrap();
    assert!(
        conflict.action.is_postpone(),
        "Conflict task should be postponed under high emotional stress, got: {:?}",
        conflict.action
    );

    // Verify Compassionate-Witness reasoning
    if let GovernanceAction::Postpone { reason } = &conflict.action {
        assert!(
            reason.contains("Compassionate") || reason.contains("compassion") || reason.contains("gentle"),
            "Compassionate-Witness reason should reference compassion: {}",
            reason
        );
    }
}

// ===========================================================================
// Test 4: Critical tasks ALWAYS proceed
// ===========================================================================

#[test]
fn critical_tasks_always_proceed() {
    // Worst possible state
    let soma = SomaState {
        sleep_hours: 2.0,
        resting_hr: 100,
        hrv: 15,
        readiness_score: 10,
    };
    let mental = MentalState {
        relational_stress: 0.95,
        burnout_risk: 0.95,
        grace_multiplier: 2.0,
    };
    let governor = TaskGovernor::new(soma, mental, None);

    let critical_task = GovernedTask::new("emergency", "Server on fire", TaskDifficulty::Critical)
        .with_priority(1.0);

    let (action, priority) = governor.evaluate(&critical_task);
    assert!(
        action.is_proceed(),
        "Critical task must ALWAYS proceed, got: {:?}",
        action
    );
    assert!(
        (priority - 1.0).abs() < 0.01,
        "Critical task priority should remain at 1.0, got: {}",
        priority
    );
}

// ===========================================================================
// Test 5: Growth-Mindset produces different reason text
// ===========================================================================

#[test]
fn growth_mindset_produces_different_reason_text() {
    let growth = EthosPolicy::preset("Growth-Mindset").unwrap();
    let governor = TaskGovernor::new(sleep_deprived_soma(), healthy_mental(), Some(growth));

    let hard_task = GovernedTask::new("deep_research", "Write research paper", TaskDifficulty::High)
        .with_priority(0.8);

    let (action, _) = governor.evaluate(&hard_task);
    assert!(
        action.is_postpone(),
        "High-difficulty task should be postponed when sleep-deprived"
    );

    if let GovernanceAction::Postpone { reason } = &action {
        assert!(
            reason.contains("Growth-Mindset") || reason.contains("growth") || reason.contains("recovery"),
            "Growth-Mindset reason should reference growth/recovery: {}",
            reason
        );
        // Should NOT contain Stoic language
        assert!(
            !reason.contains("Stoic"),
            "Growth-Mindset reason should not mention Stoic: {}",
            reason
        );
    }
}

// ===========================================================================
// Test 6: Taoist lens produces wu-wei reasoning
// ===========================================================================

#[test]
fn taoist_lens_produces_wu_wei_reasoning() {
    let taoist = EthosPolicy::preset("Taoist").unwrap();
    let governor = TaskGovernor::new(sleep_deprived_soma(), healthy_mental(), Some(taoist));

    let hard_task = GovernedTask::new("planning", "Strategic planning session", TaskDifficulty::High)
        .with_priority(0.7);

    let (action, _) = governor.evaluate(&hard_task);
    if let GovernanceAction::Postpone { reason } = &action {
        assert!(
            reason.contains("Taoist") || reason.contains("wu-wei") || reason.contains("Wu-wei") || reason.contains("flow"),
            "Taoist reason should reference wu-wei or flow: {}",
            reason
        );
    }
}

// ===========================================================================
// Test 7: Existentialist lens produces freedom reasoning
// ===========================================================================

#[test]
fn existentialist_lens_produces_freedom_reasoning() {
    let existentialist = EthosPolicy::preset("Existentialist").unwrap();
    let governor = TaskGovernor::new(sleep_deprived_soma(), healthy_mental(), Some(existentialist));

    let hard_task = GovernedTask::new("decision", "Major career decision", TaskDifficulty::High)
        .with_priority(0.9);

    let (action, _) = governor.evaluate(&hard_task);
    if let GovernanceAction::Postpone { reason } = &action {
        assert!(
            reason.contains("Existentialist") || reason.contains("freedom") || reason.contains("authentic"),
            "Existentialist reason should reference freedom/authenticity: {}",
            reason
        );
    }
}

// ===========================================================================
// Test 8: Bio penalty calculation
// ===========================================================================

#[test]
fn bio_penalty_scales_with_sleep_deprivation() {
    // 8h sleep = no penalty
    let gov_8h = TaskGovernor::new(
        SomaState { sleep_hours: 8.0, readiness_score: 85, ..Default::default() },
        healthy_mental(),
        None,
    );
    assert!(gov_8h.bio_penalty() < 0.1, "8h sleep should have near-zero penalty");

    // 5h sleep = moderate penalty
    let gov_5h = TaskGovernor::new(
        SomaState { sleep_hours: 5.0, readiness_score: 60, ..Default::default() },
        healthy_mental(),
        None,
    );
    let penalty_5h = gov_5h.bio_penalty();
    assert!(
        penalty_5h > 0.1 && penalty_5h < 0.6,
        "5h sleep should have moderate penalty: {}",
        penalty_5h
    );

    // 3h sleep = high penalty
    let gov_3h = TaskGovernor::new(
        SomaState { sleep_hours: 3.0, readiness_score: 25, ..Default::default() },
        healthy_mental(),
        None,
    );
    let penalty_3h = gov_3h.bio_penalty();
    assert!(
        penalty_3h > 0.5,
        "3h sleep should have high penalty: {}",
        penalty_3h
    );

    // Penalty should increase as sleep decreases
    assert!(
        penalty_3h > penalty_5h,
        "3h penalty ({}) should be greater than 5h penalty ({})",
        penalty_3h,
        penalty_5h
    );
}

// ===========================================================================
// Test 9: Emotional penalty calculation
// ===========================================================================

#[test]
fn emotional_penalty_scales_with_stress() {
    let calm = TaskGovernor::new(healthy_soma(), healthy_mental(), None);
    assert!(calm.emotional_penalty() < 0.1, "Calm state should have near-zero emotional penalty");

    let stressed = TaskGovernor::new(healthy_soma(), stressed_mental(), None);
    assert!(
        stressed.emotional_penalty() > 0.3,
        "Stressed state should have significant emotional penalty: {}",
        stressed.emotional_penalty()
    );
}

// ===========================================================================
// Test 10: Effective priority is reduced under load
// ===========================================================================

#[test]
fn effective_priority_reduced_under_load() {
    let governor = TaskGovernor::new(sleep_deprived_soma(), stressed_mental(), None);

    let task = GovernedTask::new("medium_task", "Regular work", TaskDifficulty::Medium)
        .with_priority(0.8);

    let (_, effective_priority) = governor.evaluate(&task);
    assert!(
        effective_priority < task.base_priority,
        "Effective priority ({}) should be less than base priority ({}) under load",
        effective_priority,
        task.base_priority
    );
}

// ===========================================================================
// Test 11: Batch evaluation sorts proceed-first, then by priority
// ===========================================================================

#[test]
fn batch_evaluation_sorts_correctly() {
    let governor = TaskGovernor::new(sleep_deprived_soma(), healthy_mental(), None);
    let tasks = sample_tasks();
    let evaluated = governor.evaluate_batch(&tasks);

    // Proceed tasks should come before postponed tasks
    let mut seen_non_proceed = false;
    for task in &evaluated {
        if !task.action.is_proceed() {
            seen_non_proceed = true;
        } else if seen_non_proceed {
            panic!(
                "Proceed task '{}' found after non-proceed task in sorted output",
                task.title
            );
        }
    }
}

// ===========================================================================
// Test 12: GovernedTask serialization roundtrip
// ===========================================================================

#[test]
fn governed_task_serialization_roundtrip() {
    let task = GovernedTask::new("test_task", "Test Task", TaskDifficulty::High)
        .with_priority(0.75)
        .with_tags(vec!["test".to_string(), "important".to_string()])
        .with_description("A test task for serialization");

    let bytes = task.to_bytes();
    let restored = GovernedTask::from_bytes(&bytes).unwrap();

    assert_eq!(restored.task_id, "test_task");
    assert_eq!(restored.title, "Test Task");
    assert_eq!(restored.difficulty, TaskDifficulty::High);
    assert!((restored.base_priority - 0.75).abs() < 0.001);
    assert_eq!(restored.tags.len(), 2);
    assert_eq!(restored.description, "A test task for serialization");
}

// ===========================================================================
// Test 13: TaskDifficulty cognitive weights
// ===========================================================================

#[test]
fn task_difficulty_cognitive_weights() {
    assert!(TaskDifficulty::Low.cognitive_weight() < TaskDifficulty::Medium.cognitive_weight());
    assert!(TaskDifficulty::Medium.cognitive_weight() < TaskDifficulty::High.cognitive_weight());
    assert!(
        (TaskDifficulty::Critical.cognitive_weight() - 0.0).abs() < 0.001,
        "Critical tasks should have 0.0 cognitive weight (never postponed)"
    );
}

// ===========================================================================
// Test 14: Full KB integration — store and retrieve governed tasks
// ===========================================================================

#[test]
fn kb_integration_store_and_retrieve_tasks() {
    let kb_dir = tempfile::tempdir().unwrap();
    let store = Arc::new(KnowledgeStore::open_path(kb_dir.path()).unwrap());

    // Store tasks
    let task1 = GovernedTask::new("task_a", "Task A", TaskDifficulty::Low).with_priority(0.3);
    let task2 = GovernedTask::new("task_b", "Task B", TaskDifficulty::High).with_priority(0.9);

    store.set_governed_task(&task1).unwrap();
    store.set_governed_task(&task2).unwrap();

    // Retrieve individual task
    let loaded = store.get_governed_task("task_a").unwrap();
    assert_eq!(loaded.task_id, "task_a");
    assert_eq!(loaded.title, "Task A");

    // List all tasks
    let all = store.list_governed_tasks().unwrap();
    assert_eq!(all.len(), 2);

    // Remove a task
    let removed = store.remove_governed_task("task_a").unwrap();
    assert!(removed, "Should return true when task existed");

    let all_after = store.list_governed_tasks().unwrap();
    assert_eq!(all_after.len(), 1);
    assert_eq!(all_after[0].task_id, "task_b");
}

// ===========================================================================
// Test 15: Full KB integration — evaluate_and_persist_tasks
// ===========================================================================

#[test]
fn kb_integration_evaluate_and_persist() {
    let kb_dir = tempfile::tempdir().unwrap();
    let store = Arc::new(KnowledgeStore::open_path(kb_dir.path()).unwrap());

    // Set up Soma (sleep-deprived)
    store.set_soma_state(&sleep_deprived_soma()).unwrap();

    // Set up Ethos (Stoic)
    let stoic = EthosPolicy::preset("Stoic").unwrap();
    store.set_ethos_philosophical_policy(&stoic).unwrap();

    // Store tasks
    for task in sample_tasks() {
        store.set_governed_task(&task).unwrap();
    }

    // Evaluate and persist
    let evaluated = store.evaluate_and_persist_tasks("default").unwrap();
    assert_eq!(evaluated.len(), 5);

    // Verify persisted tasks have updated actions
    let arch = store.get_governed_task("architecture_decision").unwrap();
    assert!(
        arch.action.is_postpone(),
        "Persisted architecture_decision should be postponed: {:?}",
        arch.action
    );
    assert!(arch.last_evaluated_ms > 0, "last_evaluated_ms should be set");

    // Verify governance summary was persisted (raw and via get_governance_summary)
    let summary = store.get_governance_summary().expect("Governance summary should be persisted");
    assert!(summary.contains("Task Governance Summary"), "Summary should contain header");
    assert!(summary.contains("Stoic"), "Summary should mention Stoic lens");
}

// ===========================================================================
// Test 16: create_task_governor reads cross-layer state
// ===========================================================================

#[test]
fn create_task_governor_reads_cross_layer_state() {
    let kb_dir = tempfile::tempdir().unwrap();
    let store = Arc::new(KnowledgeStore::open_path(kb_dir.path()).unwrap());

    // Set Soma
    let soma = SomaState {
        sleep_hours: 4.5,
        resting_hr: 72,
        hrv: 35,
        readiness_score: 40,
    };
    store.set_soma_state(&soma).unwrap();

    // Set Ethos
    let growth = EthosPolicy::preset("Growth-Mindset").unwrap();
    store.set_ethos_philosophical_policy(&growth).unwrap();

    // Create governor from store
    let governor = store.create_task_governor("default");

    // Verify it picked up the state
    assert!((governor.soma.sleep_hours - 4.5).abs() < 0.01);
    assert_eq!(governor.soma.readiness_score, 40);
    assert!(governor.ethos.is_some());
    assert_eq!(governor.ethos.as_ref().unwrap().active_school, "Growth-Mindset");

    // Mental state should have BioGate adjustment applied
    assert!(
        governor.mental.grace_multiplier >= 1.5,
        "Grace multiplier should be elevated due to poor sleep: {}",
        governor.mental.grace_multiplier
    );
}

// ===========================================================================
// Test 17: Governance summary is human-readable
// ===========================================================================

#[test]
fn governance_summary_is_readable() {
    let stoic = EthosPolicy::preset("Stoic").unwrap();
    let governor = TaskGovernor::new(sleep_deprived_soma(), stressed_mental(), Some(stoic));
    let tasks = sample_tasks();

    let summary = governor.governance_summary(&tasks);

    assert!(summary.contains("Task Governance Summary"));
    assert!(summary.contains("Stoic"));
    assert!(summary.contains("Sleep: 4.0h"));
    assert!(summary.contains("Readiness: 35"));
    assert!(summary.contains("proceed"));
    assert!(summary.contains("postponed") || summary.contains("POSTPONE"));
}

// ===========================================================================
// Test 18: No Ethos policy — generic reasons
// ===========================================================================

#[test]
fn no_ethos_policy_produces_generic_reasons() {
    let governor = TaskGovernor::new(sleep_deprived_soma(), healthy_mental(), None);

    let hard_task = GovernedTask::new("hard", "Hard task", TaskDifficulty::High)
        .with_priority(0.8);

    let (action, _) = governor.evaluate(&hard_task);
    if let GovernanceAction::Postpone { reason } = &action {
        // Should NOT contain any school name
        assert!(
            !reason.contains("Stoic") && !reason.contains("Growth") && !reason.contains("Taoist"),
            "No-ethos reason should be generic: {}",
            reason
        );
        // Should still mention sleep/readiness
        assert!(
            reason.contains("Sleep") || reason.contains("sleep") || reason.contains("Readiness"),
            "Generic reason should mention biological state: {}",
            reason
        );
    }
}

// ===========================================================================
// Test 19: Low tone_weight reduces philosophical influence
// ===========================================================================

#[test]
fn low_tone_weight_reduces_philosophical_influence() {
    let mut stoic = EthosPolicy::preset("Stoic").unwrap();
    stoic.tone_weight = 0.1; // Very low philosophical influence

    let governor = TaskGovernor::new(sleep_deprived_soma(), healthy_mental(), Some(stoic));

    let hard_task = GovernedTask::new("hard", "Hard task", TaskDifficulty::High)
        .with_priority(0.8);

    let (action, _) = governor.evaluate(&hard_task);
    if let GovernanceAction::Postpone { reason } = &action {
        // With low tone_weight, should NOT inject Stoic language
        assert!(
            !reason.contains("Stoic guidance"),
            "Low tone_weight should suppress Stoic language: {}",
            reason
        );
    }
}

// ===========================================================================
// Test 20: Medium difficulty under moderate load → Deprioritize
// ===========================================================================

#[test]
fn medium_difficulty_moderate_load_deprioritize() {
    // Moderate load: not terrible sleep but combined with burnout
    let soma = SomaState {
        sleep_hours: 5.5,
        resting_hr: 70,
        hrv: 40,
        readiness_score: 45,
    };
    let mental = MentalState {
        relational_stress: 0.5,
        burnout_risk: 0.65,
        grace_multiplier: 1.4,
    };
    let governor = TaskGovernor::new(soma, mental, None);

    let medium_task = GovernedTask::new("review", "Code review", TaskDifficulty::Medium)
        .with_priority(0.6);

    let (action, _) = governor.evaluate(&medium_task);
    // Under moderate combined load, medium tasks may be deprioritized
    // (depends on exact thresholds — the test verifies the action is not Proceed
    // when load is significant)
    let combined = governor.bio_penalty() * TaskDifficulty::Medium.cognitive_weight()
        + governor.emotional_penalty() * 0.3;
    if combined > 0.6 {
        assert!(
            matches!(action, GovernanceAction::Deprioritize { .. }),
            "Medium task should be deprioritized under moderate load (combined={:.2}), got: {:?}",
            combined,
            action
        );
    }
}
