//! 9-slot modular knowledge base system (L2 Memory + Shadow Vault).
//!
//! ## Knowledge Base Architecture
//!
//! The PAGI Orchestrator uses 8 standard Knowledge Bases (Holistic Ontology)
//! plus a 9th encrypted Shadow slot for sensitive emotional data:
//!
//! | Slot | KbType  | Purpose (Cognitive Domain)                          | Security       |
//! |------|--------|------------------------------------------------------|----------------|
//! | 1    | Pneuma | Vision: identity, mission, evolving playbook        | Standard (Sled)|
//! | 2    | Oikos  | Context: workspace scan, "where"                     | Standard (Sled)|
//! | 3    | Logos  | Pure knowledge: research, distilled information     | Standard (Sled)|
//! | 4    | Chronos| Temporal: conversation history                       | Standard (Sled)|
//! | 5    | Techne | Capability: skills, blueprints                       | Standard (Sled)|
//! | 6    | Ethos  | Guardrails: security, audit                          | Standard (Sled)|
//! | 7    | Kardia | Affective: user preferences, "who"                  | Standard (Sled)|
//! | 8    | Soma   | Execution: physical interface, buffer                | Standard (Sled)|
//! | 9    | Shadow | The Vault: trauma, anchors, private journaling      | **AES-256-GCM**|

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
pub mod vault;

pub use bootstrap::{initialize_core_identity, initialize_core_skills, initialize_ethos_policy, verify_identity, IdentityStatus};
pub use kb1::Kb1;
pub use kb2::Kb2;
pub use kb3::Kb3;
pub use kb4::Kb4;
pub use kb5::Kb5;
pub use kb6::Kb6;
pub use kb7::Kb7;
pub use kb8::Kb8;
pub use store::{pagi_kb_slot_label, AgentMessage, AlignmentResult, EventRecord, KbRecord, KbStatus, KbType, KnowledgeStore, PolicyRecord, RelationRecord, SovereignState, ETHOS_DEFAULT_POLICY_KEY, SLOT_LABELS, kardia_relation_key};
pub use store::SkillRecord;
pub use vault::{EmotionalAnchor, SecretVault, VaultError};

/// Common trait for all knowledge base slots.
pub trait KnowledgeSource: Send + Sync {
    /// Slot identifier (1–8).
    fn slot_id(&self) -> u8;

    /// Human-readable name for this knowledge source.
    fn name(&self) -> &str;

    /// Query this source by key; returns the stored value as UTF-8 string if present.
    fn query(&self, query_key: &str) -> Option<String>;
}
