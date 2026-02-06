//! 8-slot modular knowledge base system (L2 Memory).
//!
//! ## Knowledge Base Architecture
//!
//! The PAGI Orchestrator uses 8 distinct Knowledge Bases (Holistic Ontology):
//!
//! | Slot | KbType  | Purpose (Cognitive Domain)                          |
//! |------|--------|------------------------------------------------------|
//! | 1    | Pneuma | Vision: identity, mission, evolving playbook        |
//! | 2    | Oikos  | Context: workspace scan, "where"                     |
//! | 3    | Logos  | Pure knowledge: research, distilled information     |
//! | 4    | Chronos| Temporal: conversation history                       |
//! | 5    | Techne | Capability: skills, blueprints                       |
//! | 6    | Ethos  | Guardrails: security, audit                          |
//! | 7    | Kardia | Affective: user preferences, "who"                  |
//! | 8    | Soma   | Execution: physical interface, buffer                |

mod bootstrap;
mod kb1;
mod kb2;
mod kb3;
mod kb4;
mod kb5;
mod kb6;
mod kb7;
mod kb8;
mod store;

pub use bootstrap::{initialize_core_identity, initialize_core_skills, initialize_ethos_policy, verify_identity, IdentityStatus};
pub use kb1::Kb1;
pub use kb2::Kb2;
pub use kb3::Kb3;
pub use kb4::Kb4;
pub use kb5::Kb5;
pub use kb6::Kb6;
pub use kb7::Kb7;
pub use kb8::Kb8;
pub use store::{pagi_kb_slot_label, AgentMessage, AlignmentResult, EventRecord, KbRecord, KbStatus, KbType, KnowledgeStore, PolicyRecord, RelationRecord, ETHOS_DEFAULT_POLICY_KEY, SLOT_LABELS, kardia_relation_key};
pub use store::SkillRecord;

/// Common trait for all knowledge base slots.
pub trait KnowledgeSource: Send + Sync {
    /// Slot identifier (1–8).
    fn slot_id(&self) -> u8;

    /// Human-readable name for this knowledge source.
    fn name(&self) -> &str;

    /// Query this source by key; returns the stored value as UTF-8 string if present.
    fn query(&self, query_key: &str) -> Option<String>;
}
