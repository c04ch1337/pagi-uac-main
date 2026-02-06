use super::{KnowledgeSource, KnowledgeStore};
use std::sync::Arc;

pub struct Kb7(pub(crate) Arc<KnowledgeStore>);

impl KnowledgeSource for Kb7 {
    fn slot_id(&self) -> u8 {
        7
    }
    fn name(&self) -> &str {
        "kb7_policies"
    }
    fn query(&self, query_key: &str) -> Option<String> {
        self.0
            .get(7, query_key)
            .ok()
            .flatten()
            .and_then(|v| String::from_utf8(v).ok())
    }
}
