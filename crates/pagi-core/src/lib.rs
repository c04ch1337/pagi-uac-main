//! pagi-core: AGI core library (shared types, orchestrator, memory, 8-slot knowledge base).
//!
//! Re-exports the former pagi-shared, pagi-orchestrator, pagi-memory, and pagi-knowledge
//! so add-ons and the gateway keep a consistent public API.

mod knowledge;
mod memory;
mod orchestrator;
mod shared;

// Shared (former pagi-shared)
pub use shared::{CoreConfig, Goal, TenantContext, DEFAULT_AGENT_ID};

// Memory (former pagi-memory)
pub use memory::MemoryManager;

// Knowledge (former pagi-knowledge) - L2 Memory System
pub use knowledge::{
    initialize_core_identity, initialize_core_skills, initialize_ethos_policy, pagi_kb_slot_label, verify_identity, IdentityStatus, AgentMessage, AlignmentResult, EventRecord, Kb1, Kb2, Kb3,
    Kb4, Kb5, Kb6, Kb7, Kb8, KbRecord, KbStatus, KbType, KnowledgeSource, KnowledgeStore,
    PolicyRecord, RelationRecord, ETHOS_DEFAULT_POLICY_KEY, SkillRecord, SLOT_LABELS, kardia_relation_key,
};

// Orchestrator (former pagi-orchestrator)
pub use orchestrator::{
    AgentSkill, BlueprintRegistry, ControlPanelMessage, ControlPanelReceiver, Orchestrator, Plan,
    SkillRegistry,
};
