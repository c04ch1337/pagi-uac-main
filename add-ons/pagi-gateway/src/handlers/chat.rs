//! Chat handler: Sovereign Brain context for chat.
//!
//! The **system prompt** (Mission Directive) is built by `KnowledgeStore::build_system_directive`
//! in pagi-core and sent as a dedicated `role: "system"` message from the Gateway — never
//! from the UI. That ensures the LLM receives Identity (Pneuma), Soma, Kardia, Ethos, Oikos, and Shadow.
//!
//! Non-streaming: `Orchestrator::dispatch(ModelRouter)` with `system_prompt` + `prompt`.
//! Streaming: `ModelRouter::stream_generate(Some(system_directive), user_prompt, ...)`.

use pagi_core::{KnowledgeStore, MentalState};

/// Optional: builds a single prompt string with Soma/Kardia prefix (for flows that do not use
/// a separate system message). The main chat path uses `KnowledgeStore::build_system_directive` instead.
#[allow(dead_code)]
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
