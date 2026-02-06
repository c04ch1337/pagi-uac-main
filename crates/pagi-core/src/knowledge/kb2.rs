use super::{KnowledgeSource, KnowledgeStore};
use std::sync::Arc;

pub struct Kb2(pub(crate) Arc<KnowledgeStore>);

impl KnowledgeSource for Kb2 {
    fn slot_id(&self) -> u8 {
        2
    }
    fn name(&self) -> &str {
        "kb2_sales"
    }
    fn query(&self, query_key: &str) -> Option<String> {
        self.0
            .get(2, query_key)
            .ok()
            .flatten()
            .and_then(|v| String::from_utf8(v).ok())
    }
}
