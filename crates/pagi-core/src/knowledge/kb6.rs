use super::{KnowledgeSource, KnowledgeStore};
use std::sync::Arc;

pub struct Kb6(pub(crate) Arc<KnowledgeStore>);

impl KnowledgeSource for Kb6 {
    fn slot_id(&self) -> u8 {
        6
    }
    fn name(&self) -> &str {
        "kb6_products"
    }
    fn query(&self, query_key: &str) -> Option<String> {
        self.0
            .get(6, query_key)
            .ok()
            .flatten()
            .and_then(|v| String::from_utf8(v).ok())
    }
}
