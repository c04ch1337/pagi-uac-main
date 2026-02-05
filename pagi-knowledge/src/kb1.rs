use crate::{KnowledgeSource, KnowledgeStore};
use std::sync::Arc;

pub struct Kb1(pub(crate) Arc<KnowledgeStore>);

impl KnowledgeSource for Kb1 {
    fn slot_id(&self) -> u8 {
        1
    }
    fn name(&self) -> &str {
        "kb1_marketing"
    }
    fn query(&self, query_key: &str) -> Option<String> {
        self.0
            .get(1, query_key)
            .ok()
            .flatten()
            .and_then(|v| String::from_utf8(v).ok())
    }
}
