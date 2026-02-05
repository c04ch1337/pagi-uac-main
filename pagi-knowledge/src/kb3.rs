use crate::{KnowledgeSource, KnowledgeStore};
use std::sync::Arc;

pub struct Kb3(pub(crate) Arc<KnowledgeStore>);

impl KnowledgeSource for Kb3 {
    fn slot_id(&self) -> u8 {
        3
    }
    fn name(&self) -> &str {
        "kb3_finance"
    }
    fn query(&self, query_key: &str) -> Option<String> {
        self.0
            .get(3, query_key)
            .ok()
            .flatten()
            .and_then(|v| String::from_utf8(v).ok())
    }
}
