use super::{KnowledgeSource, KnowledgeStore};
use std::sync::Arc;

pub struct Kb5(pub(crate) Arc<KnowledgeStore>);

impl KnowledgeSource for Kb5 {
    fn slot_id(&self) -> u8 {
        5
    }
    fn name(&self) -> &str {
        "kb5_community"
    }
    fn query(&self, query_key: &str) -> Option<String> {
        self.0
            .get(5, query_key)
            .ok()
            .flatten()
            .and_then(|v| String::from_utf8(v).ok())
    }
}
