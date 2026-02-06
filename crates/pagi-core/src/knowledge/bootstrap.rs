//! Bootstrap routines for initializing Knowledge Bases with core data.
//!
//! This module ensures the Orchestrator has essential identity and configuration
//! data from first boot, establishing the "Mission Genesis" for the system.

use super::store::{KbRecord, KbType, KnowledgeStore, PolicyRecord, SkillRecord, ETHOS_DEFAULT_POLICY_KEY};
use std::sync::Arc;

/// Core identity record keys for KB-1 (Identity).
pub const IDENTITY_MISSION_KEY: &str = "core_mission";
pub const IDENTITY_PRIORITIES_KEY: &str = "core_priorities";
pub const IDENTITY_PERSONA_KEY: &str = "core_persona";
pub const IDENTITY_GOALS_KEY: &str = "research_goals_2026";

/// Initializes the core identity in KB-1 if it doesn't already exist.
///
/// This function checks if KB-1 (Identity) has the essential mission data.
/// If empty or missing core keys, it bootstraps the Agent's identity with:
/// - Mission statement
/// - Research priorities
/// - Persona characteristics
/// - 2026 research goals
///
/// # Arguments
/// * `store` - Arc reference to the KnowledgeStore
///
/// # Returns
/// * `Ok(bool)` - true if bootstrap was performed, false if identity already existed
/// * `Err` - if a database error occurred
pub fn initialize_core_identity(store: &Arc<KnowledgeStore>) -> Result<bool, sled::Error> {
    let identity_slot = KbType::Pneuma.slot_id();
    
    // Check if core mission already exists
    if let Some(_) = store.get(identity_slot, IDENTITY_MISSION_KEY)? {
        tracing::info!(
            target: "pagi::bootstrap",
            "KB-1 [Pneuma/Vision] already contains core mission data. Skipping bootstrap."
        );
        return Ok(false);
    }
    
    tracing::info!(
        target: "pagi::bootstrap",
        "KB-1 [Pneuma/Vision] is empty. Initializing Mission Genesis..."
    );
    
    // === MISSION STATEMENT ===
    let mission = KbRecord::with_metadata(
        "Autonomous AGI Research & Bare-Metal Orchestration. \
         This system is designed as a research-grade Master Orchestrator for 2026, \
         focused on exploring the boundaries of autonomous reasoning, multi-layer memory systems, \
         and real-time knowledge synthesis without containerization overhead.",
        serde_json::json!({
            "type": "mission_statement",
            "version": "1.0.0",
            "category": "identity",
            "immutable": false,
            "tags": ["mission", "identity", "core"]
        }),
    );
    store.insert_record(identity_slot, IDENTITY_MISSION_KEY, &mission)?;
    
    // === RESEARCH PRIORITIES ===
    let priorities = KbRecord::with_metadata(
        "1. Rust-based efficiency: Zero-copy operations, minimal allocations, bare-metal performance.\n\
         2. Multi-layer memory integrity: L1 (hot cache) + L2 (8 Knowledge Bases) + L3 (long-term vault).\n\
         3. 8-KB specialized recall: Each Knowledge Base serves a distinct cognitive function.\n\
         4. Live LLM integration: Real-time inference with token usage tracking.\n\
         5. Skill modularity: Pluggable capabilities without core system modifications.\n\
         6. Research transparency: Full audit trails and thought logging for reproducibility.",
        serde_json::json!({
            "type": "priorities",
            "version": "1.0.0",
            "category": "identity",
            "priority_count": 6,
            "tags": ["priorities", "research", "architecture"]
        }),
    );
    store.insert_record(identity_slot, IDENTITY_PRIORITIES_KEY, &priorities)?;
    
    // === PERSONA CHARACTERISTICS ===
    let persona = KbRecord::with_metadata(
        "Grounded, high-performance, and technically precise. \
         This Orchestrator communicates with clarity and directness, \
         prioritizing accuracy over pleasantries. It acknowledges uncertainty explicitly, \
         provides evidence-based reasoning, and maintains a research-focused mindset. \
         When engaging with complex problems, it breaks them into systematic components \
         and traces its reasoning transparently.",
        serde_json::json!({
            "type": "persona",
            "version": "1.0.0",
            "category": "identity",
            "traits": [
                "grounded",
                "high-performance",
                "technically-precise",
                "research-focused",
                "transparent-reasoning"
            ],
            "tags": ["persona", "voice", "communication"]
        }),
    );
    store.insert_record(identity_slot, IDENTITY_PERSONA_KEY, &persona)?;
    
    // === 2026 RESEARCH GOALS ===
    let goals = KbRecord::with_metadata(
        "Research Goals for 2026:\n\
         • Achieve persistent memory across sessions with semantic recall\n\
         • Implement autonomous skill discovery and execution\n\
         • Develop multi-agent coordination protocols\n\
         • Build robust security auditing for AI actions\n\
         • Create self-improving knowledge curation routines\n\
         • Establish benchmarks for bare-metal AGI performance\n\
         • Document reproducible research methodologies",
        serde_json::json!({
            "type": "goals",
            "version": "1.0.0",
            "year": 2026,
            "category": "identity",
            "goal_count": 7,
            "status": "active",
            "tags": ["goals", "2026", "research", "roadmap"]
        }),
    );
    store.insert_record(identity_slot, IDENTITY_GOALS_KEY, &goals)?;
    
    tracing::info!(
        target: "pagi::bootstrap",
        "✓ [Pneuma/Vision] Bootstrapped core mission goals into KB-1 (4 records)"
    );
    
    Ok(true)
}

/// Skill Registry bootstrap: inserts baseline skill manifests into KB-5.
///
/// Safe to call multiple times; will skip if the key already exists.
pub fn initialize_core_skills(store: &Arc<KnowledgeStore>) -> Result<bool, sled::Error> {
    let skills_slot = KbType::Techne.slot_id();

    let mut inserted_any = false;

    // --- fs_workspace_analyzer ---
    let key = "skills/fs_workspace_analyzer";
    if store.get(skills_slot, key)?.is_none() {
        let record = SkillRecord {
            slug: "fs_workspace_analyzer".to_string(),
            description: "Provides a high-level tree view of the local Rust workspace, identifying crates and key config files.".to_string(),
            schema: serde_json::json!({
                "path": "string (optional; defaults to current dir)",
                "depth": "number (optional)"
            }),
        };
        store.insert(
            skills_slot,
            key,
            serde_json::to_vec(&record).unwrap_or_default().as_slice(),
        )?;
        inserted_any = true;
    }

    // --- write_sandbox_file ---
    let key = "skills/write_sandbox_file";
    if store.get(skills_slot, key)?.is_none() {
        let record = SkillRecord {
            slug: "write_sandbox_file".to_string(),
            description: "Writes a file strictly within research_sandbox/. Rejects absolute paths and any traversal attempts.".to_string(),
            schema: serde_json::json!({
                "path": "string (required; within research_sandbox/)",
                "content": "string (required)",
                "append": "boolean (optional; default false)"
            }),
        };
        store.insert(
            skills_slot,
            key,
            serde_json::to_vec(&record).unwrap_or_default().as_slice(),
        )?;
        inserted_any = true;
    }

    // --- recall_past_actions ---
    let key = "skills/recall_past_actions";
    if store.get(skills_slot, key)?.is_none() {
        let record = SkillRecord {
            slug: "recall_past_actions".to_string(),
            description: "Queries KB_CHRONOS for the last N things the Agent did. Use to answer 'What did you do recently?' or 'What did you do five minutes ago?'".to_string(),
            schema: serde_json::json!({
                "limit": "number (optional; default 5, max 50)"
            }),
        };
        store.insert(
            skills_slot,
            key,
            serde_json::to_vec(&record).unwrap_or_default().as_slice(),
        )?;
        inserted_any = true;
    }

    // --- check_alignment ---
    let key = "skills/check_alignment";
    if store.get(skills_slot, key)?.is_none() {
        let record = SkillRecord {
            slug: "check_alignment".to_string(),
            description: "Consults KB_ETHOS to return pass/fail for an intended action (skill_name + content). Use before executing sensitive actions.".to_string(),
            schema: serde_json::json!({
                "skill_name": "string (required)",
                "content": "string (optional; payload content to scan for sensitive keywords)"
            }),
        };
        store.insert(
            skills_slot,
            key,
            serde_json::to_vec(&record).unwrap_or_default().as_slice(),
        )?;
        inserted_any = true;
    }

    // --- analyze_sentiment ---
    let key = "skills/analyze_sentiment";
    if store.get(skills_slot, key)?.is_none() {
        let record = SkillRecord {
            slug: "analyze_sentiment".to_string(),
            description: "Updates KB_KARDIA with relationship state from recent user messages. Provide user_id and last 3 messages; infers sentiment and communication style.".to_string(),
            schema: serde_json::json!({
                "user_id": "string (required)",
                "messages": "array of strings (last N user messages)"
            }),
        };
        store.insert(
            skills_slot,
            key,
            serde_json::to_vec(&record).unwrap_or_default().as_slice(),
        )?;
        inserted_any = true;
    }

    Ok(inserted_any)
}

/// Initializes the default safety policy in **KB_ETHOS** if not already present.
///
/// Default policy: do not write to the sandbox if the data contains raw API keys or secrets
/// (sensitive_keywords: api_key, secret, password, token, credentials; approval_required: true).
pub fn initialize_ethos_policy(store: &Arc<KnowledgeStore>) -> Result<bool, sled::Error> {
    let ethos_slot = KbType::Ethos.slot_id();
    if store.get(ethos_slot, ETHOS_DEFAULT_POLICY_KEY)?.is_some() {
        tracing::info!(
            target: "pagi::bootstrap",
            "KB_ETHOS already has default policy. Skipping."
        );
        return Ok(false);
    }
    let policy = PolicyRecord::default();
    store.set_ethos_policy(&policy)?;
    tracing::info!(
        target: "pagi::bootstrap",
        "✓ [Ethos] Default safety policy installed (sensitive_keywords + approval_required)"
    );
    Ok(true)
}

/// Verifies that core identity data exists and is accessible.
///
/// Returns a summary of the identity state for diagnostics.
pub fn verify_identity(store: &Arc<KnowledgeStore>) -> IdentityStatus {
    let identity_slot = KbType::Pneuma.slot_id();
    
    let mission_exists = store.get(identity_slot, IDENTITY_MISSION_KEY).ok().flatten().is_some();
    let priorities_exists = store.get(identity_slot, IDENTITY_PRIORITIES_KEY).ok().flatten().is_some();
    let persona_exists = store.get(identity_slot, IDENTITY_PERSONA_KEY).ok().flatten().is_some();
    let goals_exists = store.get(identity_slot, IDENTITY_GOALS_KEY).ok().flatten().is_some();
    
    let complete = mission_exists && priorities_exists && persona_exists && goals_exists;
    let record_count = [mission_exists, priorities_exists, persona_exists, goals_exists]
        .iter()
        .filter(|&&x| x)
        .count();
    
    IdentityStatus {
        complete,
        record_count,
        has_mission: mission_exists,
        has_priorities: priorities_exists,
        has_persona: persona_exists,
        has_goals: goals_exists,
    }
}

/// Status of the core identity in KB-1.
#[derive(Debug, Clone)]
pub struct IdentityStatus {
    /// Whether all 4 core identity records exist.
    pub complete: bool,
    /// Number of identity records found (0-4).
    pub record_count: usize,
    /// Whether the mission statement exists.
    pub has_mission: bool,
    /// Whether the priorities exist.
    pub has_priorities: bool,
    /// Whether the persona exists.
    pub has_persona: bool,
    /// Whether the goals exist.
    pub has_goals: bool,
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;
    
    #[test]
    fn test_bootstrap_creates_identity_records() {
        let dir = tempdir().unwrap();
        let store = Arc::new(KnowledgeStore::open_path(dir.path()).unwrap());
        
        // First call should bootstrap
        let result = initialize_core_identity(&store).unwrap();
        assert!(result, "Should return true on first bootstrap");
        
        // Verify records exist
        let status = verify_identity(&store);
        assert!(status.complete);
        assert_eq!(status.record_count, 4);
        
        // Second call should skip (already exists)
        let result2 = initialize_core_identity(&store).unwrap();
        assert!(!result2, "Should return false when identity already exists");
    }
}
