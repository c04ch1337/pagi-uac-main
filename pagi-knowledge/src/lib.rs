//! 8-slot modular knowledge base system.

mod kb1;
mod kb2;
mod kb3;
mod kb4;
mod kb5;
mod kb6;
mod kb7;
mod kb8;
mod store;

pub use kb1::Kb1;
pub use kb2::Kb2;
pub use kb3::Kb3;
pub use kb4::Kb4;
pub use kb5::Kb5;
pub use kb6::Kb6;
pub use kb7::Kb7;
pub use kb8::Kb8;
pub use store::KnowledgeStore;

/// Common trait for all knowledge base slots.
pub trait KnowledgeSource: Send + Sync {
    /// Slot identifier (1–8).
    fn slot_id(&self) -> u8;

    /// Human-readable name for this knowledge source.
    fn name(&self) -> &str;

    /// Query this source by key; returns the stored value as UTF-8 string if present.
    fn query(&self, query_key: &str) -> Option<String>;
}
