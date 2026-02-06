//! KB-3 (Logos) semantic insert + search — pure knowledge / research.

use pagi_core::{AgentSkill, KbRecord, KbType, KnowledgeStore, TenantContext};
use serde::Deserialize;
use std::sync::Arc;

use crate::model_router::ModelRouter;

const SKILL_INSERT: &str = "ResearchEmbedInsert";
const SKILL_SEARCH: &str = "ResearchSemanticSearch";

fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != b.len() || a.is_empty() {
        return 0.0;
    }
    let mut dot = 0.0f32;
    let mut na = 0.0f32;
    let mut nb = 0.0f32;
    for i in 0..a.len() {
        let x = a[i];
        let y = b[i];
        dot += x * y;
        na += x * x;
        nb += y * y;
    }
    let denom = na.sqrt() * nb.sqrt();
    if denom > 0.0 { dot / denom } else { 0.0 }
}

#[derive(Debug, Deserialize)]
struct InsertArgs {
    /// Key to store the record under (sled key).
    key: String,
    /// Natural language content to embed + store.
    content: String,
    /// Optional metadata object.
    #[serde(default)]
    metadata: Option<serde_json::Value>,
    /// Optional embedding model override.
    #[serde(default)]
    embedding_model: Option<String>,
}

/// Inserts a KB-3 record with an inline embedding vector.
pub struct ResearchEmbedInsert {
    store: Arc<KnowledgeStore>,
    router: ModelRouter,
}

impl ResearchEmbedInsert {
    pub fn new(store: Arc<KnowledgeStore>) -> Self {
        Self {
            store,
            router: ModelRouter::new(),
        }
    }
}

#[async_trait::async_trait]
impl AgentSkill for ResearchEmbedInsert {
    fn name(&self) -> &str {
        SKILL_INSERT
    }

    async fn execute(
        &self,
        _ctx: &TenantContext,
        payload: Option<serde_json::Value>,
    ) -> Result<serde_json::Value, Box<dyn std::error::Error + Send + Sync>> {
        let payload = payload.ok_or("ResearchEmbedInsert requires payload: { key, content, metadata? }")?;
        let args: InsertArgs = serde_json::from_value(payload)?;

        let embedding = self
            .router
            .embedding(&args.content, args.embedding_model.as_deref())
            .await?;

        let mut md = args.metadata.unwrap_or_else(|| serde_json::json!({}));
        md["embedding_model"] = serde_json::json!(args.embedding_model.clone().unwrap_or_else(|| "default".to_string()));
        md["vector_dims"] = serde_json::json!(embedding.len());

        let record = KbRecord::with_embedding(args.content, md, embedding);
        let slot_id = KbType::Logos.slot_id();
        self.store.insert_record(slot_id, &args.key, &record)?;

        Ok(serde_json::json!({
            "status": "ok",
            "skill": SKILL_INSERT,
            "slot_id": slot_id,
            "key": args.key,
            "vector_dims": record.embedding.as_ref().map(|v| v.len()).unwrap_or(0)
        }))
    }
}

#[derive(Debug, Deserialize)]
struct SearchArgs {
    /// Natural language query.
    query: String,
    /// Maximum results.
    #[serde(default = "default_limit")]
    limit: usize,
    /// Optional embedding model override.
    #[serde(default)]
    embedding_model: Option<String>,
}

fn default_limit() -> usize {
    5
}

/// Performs a brute-force cosine-similarity search over KB-3 records that have embeddings.
pub struct ResearchSemanticSearch {
    store: Arc<KnowledgeStore>,
    router: ModelRouter,
}

impl ResearchSemanticSearch {
    pub fn new(store: Arc<KnowledgeStore>) -> Self {
        Self {
            store,
            router: ModelRouter::new(),
        }
    }
}

#[async_trait::async_trait]
impl AgentSkill for ResearchSemanticSearch {
    fn name(&self) -> &str {
        SKILL_SEARCH
    }

    async fn execute(
        &self,
        _ctx: &TenantContext,
        payload: Option<serde_json::Value>,
    ) -> Result<serde_json::Value, Box<dyn std::error::Error + Send + Sync>> {
        let payload = payload.ok_or("ResearchSemanticSearch requires payload: { query, limit? }")?;
        let args: SearchArgs = serde_json::from_value(payload)?;

        let qv = self
            .router
            .embedding(&args.query, args.embedding_model.as_deref())
            .await?;

        let slot_id = KbType::Logos.slot_id();
        let records = self.store.scan_records(slot_id)?;

        let mut scored: Vec<serde_json::Value> = Vec::new();
        for (key, rec) in records {
            let Some(ev) = rec.embedding.as_deref() else {
                continue;
            };
            if ev.len() != qv.len() {
                continue;
            }
            let score = cosine_similarity(&qv, ev);
            let preview = rec.content.chars().take(200).collect::<String>();
            scored.push(serde_json::json!({
                "key": key,
                "score": score,
                "preview": preview,
                "metadata": rec.metadata,
                "timestamp": rec.timestamp
            }));
        }

        scored.sort_by(|a, b| {
            let sa = a.get("score").and_then(|v| v.as_f64()).unwrap_or(0.0);
            let sb = b.get("score").and_then(|v| v.as_f64()).unwrap_or(0.0);
            sb.partial_cmp(&sa).unwrap_or(std::cmp::Ordering::Equal)
        });

        scored.truncate(args.limit.max(1));

        Ok(serde_json::json!({
            "status": "ok",
            "skill": SKILL_SEARCH,
            "slot_id": slot_id,
            "query": args.query,
            "vector_dims": qv.len(),
            "result_count": scored.len(),
            "results": scored
        }))
    }
}

