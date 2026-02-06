//! Multi-agent “Dev-Audit” simulation.
//!
//! This integration test demonstrates the multi-agent substrate:
//! - shared workspace (file system / OIKOS)
//! - per-agent CHRONOS (episodic memory streams)
//! - agent-to-agent messaging via KB_SOMA inbox primitives (wrapped by the
//!   `message_agent` / `get_agent_messages` skills in `pagi-skills`)
//! - ETHOS alignment scan against the stored policy.
//!
//! NOTE: This test lives in `pagi-core`, so it exercises the underlying
//! `KnowledgeStore::{push_agent_message,get_agent_messages}` primitives directly
//! (avoids a circular dev-dependency on `pagi-skills`).

use pagi_core::{AlignmentResult, EventRecord, KnowledgeStore, PolicyRecord};
use tempfile::tempdir;

#[test]
fn multi_agent_dev_audit_ping_pong() {
    // Agent identities (strings in the current implementation).
    let dev_agent_id = "DEV_BOT";
    let sage_agent_id = "SAGE_BOT";

    // Isolated DB + workspace.
    let dir = tempdir().expect("tempdir");
    let store = KnowledgeStore::open_path(dir.path()).expect("open knowledge store");

    // Shared workspace root for the simulation.
    let workspace_root = dir.path().join("workspace");
    std::fs::create_dir_all(workspace_root.join("research_sandbox"))
        .expect("create research_sandbox");

    // Make relative paths behave like the real gateway run-from-root behavior.
    let prev_cwd = std::env::current_dir().expect("current_dir");
    std::env::set_current_dir(&workspace_root).expect("set_current_dir");

    // Install default ETHOS policy (blocks sensitive keywords like api_key).
    store
        .set_ethos_policy(&PolicyRecord::default())
        .expect("set_ethos_policy");

    // --- 1) DEV writes vulnerable code into the shared workspace.
    let rel_file = "vulnerable_code.rs";
    let file_path = workspace_root
        .join("research_sandbox")
        .join(rel_file);
    let vulnerable = r#"// intentionally vulnerable
pub fn demo() -> &'static str {
    let api_key = \"sk-THIS_SHOULD_NOT_BE_HARDCODED\";
    api_key
}
"#;
    std::fs::write(&file_path, vulnerable).expect("write vulnerable code");

    // DEV marks its own state as pending review.
    let dev_pending = EventRecord::now(
        "Soma",
        format!("Task Pending: awaiting audit of research_sandbox/{rel_file}"),
    )
    .with_skill("message_agent")
    .with_outcome("pending");
    store
        .append_chronos_event(dev_agent_id, &dev_pending)
        .expect("append dev chronos");

    // --- 2) DEV -> SAGE handoff message (KB_SOMA inbox).
    let payload_to_sage = serde_json::json!({
        "msg": "Review this code",
        "file": rel_file,
    });
    let _message_id = store
        .push_agent_message(dev_agent_id, sage_agent_id, &payload_to_sage)
        .expect("push_agent_message dev->sage");

    // --- 3) SAGE polls inbox and reads the shared file.
    let inbox = store
        .get_agent_messages(sage_agent_id, 10)
        .expect("get_agent_messages sage")
        .into_iter()
        .collect::<Vec<_>>();
    assert!(
        inbox.iter().any(|m| m.from_agent_id == dev_agent_id),
        "SAGE should receive at least one message from DEV"
    );
    let first = inbox
        .iter()
        .find(|m| m.from_agent_id == dev_agent_id)
        .expect("message from dev exists");
    let requested_file = first
        .payload
        .get("file")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    assert_eq!(requested_file, rel_file);

    let content = std::fs::read_to_string(&file_path).expect("read code for audit");

    // --- 4) SAGE performs ETHOS alignment scan (equivalent to `check_alignment`).
    let policy = store.get_ethos_policy().expect("ethos policy present");
    let scan = policy.allows("write_sandbox_file", &content);
    let violation_reason = match scan {
        AlignmentResult::Pass => None,
        AlignmentResult::Fail { reason } => Some(reason),
    };
    assert!(
        violation_reason.is_some(),
        "Expected policy violation due to hardcoded api_key"
    );
    let violation_reason = violation_reason.unwrap();

    // SAGE logs the violation to its own CHRONOS.
    let sage_event = EventRecord::now(
        "Ethos",
        format!("Policy Violation: {violation_reason}"),
    )
    .with_skill("check_alignment")
    .with_outcome("blocked");
    store
        .append_chronos_event(sage_agent_id, &sage_event)
        .expect("append sage chronos");

    // SAGE -> DEV reply message.
    let payload_to_dev = serde_json::json!({
        "status": "policy_violation",
        "reason": violation_reason,
        "file": rel_file,
        "suggestion": "Remove hardcoded secrets; load from env or secret manager and avoid committing credentials.",
    });
    store
        .push_agent_message(sage_agent_id, dev_agent_id, &payload_to_dev)
        .expect("push_agent_message sage->dev");

    // --- 5) VERIFY: per-agent Chronos streams contain expected markers.
    let sage_events = store
        .get_recent_chronos_events(sage_agent_id, 10)
        .expect("get_recent_chronos_events sage");
    assert!(
        sage_events
            .iter()
            .any(|e| e.reflection.contains("Policy Violation")),
        "SAGE chronos should contain a Policy Violation event"
    );

    let dev_events = store
        .get_recent_chronos_events(dev_agent_id, 10)
        .expect("get_recent_chronos_events dev");
    assert!(
        dev_events.iter().any(|e| e.reflection.contains("Task Pending")),
        "DEV chronos should contain a Task Pending marker"
    );

    // VERIFY: DEV receives the violation message back.
    let dev_inbox = store
        .get_agent_messages(dev_agent_id, 10)
        .expect("get_agent_messages dev");
    assert!(
        dev_inbox.iter().any(|m| {
            m.from_agent_id == sage_agent_id
                && m.payload
                    .get("status")
                    .and_then(|v| v.as_str())
                    == Some("policy_violation")
        }),
        "DEV should receive a policy_violation message from SAGE"
    );

    // Restore cwd for test isolation.
    std::env::set_current_dir(prev_cwd).expect("restore current_dir");
}

