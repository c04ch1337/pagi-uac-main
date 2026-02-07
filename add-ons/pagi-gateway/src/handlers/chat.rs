//! Chat handler: reconnects the Sovereign Brain by injecting Soma and Kardia state
//! from KnowledgeStore into every chat request. The gateway uses this context so the
//! agent responds with full awareness of the user's body (BioGate/Soma) and
//! relationship/mental state (Kardia).
//!
//! Non-streaming chat calls `Orchestrator::dispatch` with `Goal::ExecuteSkill { name: "ModelRouter", ... }`.
//! Streaming chat uses the same context but calls `ModelRouter` directly for token stream.

use pagi_core::{KnowledgeStore, MentalState};

/// Builds the full prompt for the LLM by injecting current Soma (body/BioGate) and
/// Kardia (relationship/mental) state from KnowledgeStore. Every chat request must
/// call this so the agent has the user's actual status.
pub fn build_prompt_with_soma_kardia(
    knowledge: &KnowledgeStore,
    agent_id: &str,
    user_id: &str,
    user_prompt: &str,
) -> String {
    // 1) Kardia: relationship and sentiment context for this user
    let kardia_context = knowledge
        .get_kardia_relation(agent_id, user_id)
        .map(|r| r.prompt_context())
        .unwrap_or_default();

    let mut parts: Vec<String> = Vec::new();

    if !kardia_context.is_empty() {
        parts.push(kardia_context);
    }

    // 2) Soma (Body/BioGate): explicit current body state so the agent knows physical context
    let soma = knowledge.get_soma_state();
    let has_soma_data = soma.sleep_hours > 0.0 || soma.readiness_score < 100 || soma.resting_hr > 0 || soma.hrv > 0;
    if has_soma_data {
        let bio_line = format!(
            "[Current body state (Soma/BioGate): sleep {:.1}h, readiness {}, resting HR {} bpm, HRV {} ms. BioGate adjustment: {}.]",
            soma.sleep_hours,
            soma.readiness_score,
            soma.resting_hr,
            soma.hrv,
            if soma.needs_biogate_adjustment() { "active (supportive tone)" } else { "inactive" }
        );
        parts.push(bio_line);
    }

    // 3) Effective mental state (Kardia baseline + Soma/BioGate cross-layer reaction)
    let mental = knowledge.get_effective_mental_state(agent_id);
    if mental.needs_empathetic_tone() {
        parts.push(MentalState::EMPATHETIC_SYSTEM_INSTRUCTION.to_string());
    }
    if mental.has_physical_load_adjustment() {
        parts.push(MentalState::PHYSICAL_LOAD_SYSTEM_INSTRUCTION.to_string());
    }

    // 4) Shadow (Slot 9): compassionate routing when emotional anchors are active
    if let Some(shadow_instruction) = knowledge.check_mental_load() {
        parts.push(shadow_instruction);
    }

    let system_prefix = if parts.is_empty() {
        String::new()
    } else {
        format!("{}\n\n", parts.join("\n"))
    };

    format!("{}{}", system_prefix, user_prompt)
}
