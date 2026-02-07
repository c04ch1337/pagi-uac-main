//! pagi-core: AGI core library (shared types, orchestrator, memory, 8-slot knowledge base).
//!
//! Re-exports the former pagi-shared, pagi-orchestrator, pagi-memory, and pagi-knowledge
//! so add-ons and the gateway keep a consistent public API.

mod knowledge;
mod memory;
mod orchestrator;
mod secure_memory;
mod shadow_store;
mod shared;

// Shared (former pagi-shared) + Emotional Context Layer + Task Governance
pub use shared::{
    BiometricState, CoreConfig, EthosPolicy, Goal, MentalState, MENTAL_STATE_KEY, PersonRecord,
    SomaState, TenantContext, KARDIA_PEOPLE_PREFIX, DEFAULT_AGENT_ID, ETHOS_POLICY_KEY,
    // Dynamic Task Governance (Oikos)
    GovernanceAction, GovernedTask, TaskDifficulty, TaskGovernor,
    OIKOS_TASK_PREFIX, OIKOS_GOVERNANCE_SUMMARY_KEY,
};
pub use shadow_store::{DecryptedEntry, PersonalHistoryEntry, ShadowStore, ShadowStoreHandle};

// Memory (former pagi-memory)
pub use memory::MemoryManager;

// Knowledge (former pagi-knowledge) - L2 Memory System + Shadow Vault
pub use knowledge::{
    initialize_core_identity, initialize_core_skills, initialize_ethos_policy, pagi_kb_slot_label, verify_identity, IdentityStatus, AgentMessage, AlignmentResult, EventRecord, Kb1, Kb2, Kb3,
    Kb4, Kb5, Kb6, Kb7, Kb8, KbRecord, KbStatus, KbType, KnowledgeSource, KnowledgeStore,
    PolicyRecord, RelationRecord, SovereignState, ETHOS_DEFAULT_POLICY_KEY, SkillRecord, SLOT_LABELS, kardia_relation_key,
    EmotionalAnchor, SecretVault, VaultError,
};

// Orchestrator (former pagi-orchestrator)
pub use orchestrator::{
    AgentSkill, BlueprintRegistry, ControlPanelMessage, ControlPanelReceiver, Orchestrator, Plan,
    SkillRegistry,
};
