use crate::{KnowledgeSource, KnowledgeStore};
use std::sync::Arc;

pub struct Kb4(pub(crate) Arc<KnowledgeStore>);

impl KnowledgeSource for Kb4 {
    fn slot_id(&self) -> u8 {
        4
    }
    fn name(&self) -> &str {
        "kb4_operations"
    }
    fn query(&self, query_key: &str) -> Option<String> {
        self.0
            .get(4, query_key)
            .ok()
            .flatten()
            .and_then(|v| String::from_utf8(v).ok())
    }
}
