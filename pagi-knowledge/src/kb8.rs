use crate::{KnowledgeSource, KnowledgeStore};
use std::sync::Arc;

pub struct Kb8(pub(crate) Arc<KnowledgeStore>);

impl KnowledgeSource for Kb8 {
    fn slot_id(&self) -> u8 {
        8
    }
    fn name(&self) -> &str {
        "kb8_custom"
    }
    fn query(&self, query_key: &str) -> Option<String> {
        self.0
            .get(8, query_key)
            .ok()
            .flatten()
            .and_then(|v| String::from_utf8(v).ok())
    }
}
