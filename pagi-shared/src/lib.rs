//! Shared types used across all UAC crates.

use serde::{Deserialize, Serialize};

/// Tenant context for multi-tenant isolation across the UAC system.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TenantContext {
    /// Unique tenant identifier.
    pub tenant_id: String,
    /// Optional correlation id for request tracing.
    pub correlation_id: Option<String>,
}

/// High-level goal types the orchestrator can delegate.
/// Generic (use-case agnostic) variants support template/clone deployments.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Goal {
    /// Execute a named skill with optional payload.
    ExecuteSkill { name: String, payload: Option<serde_json::Value> },
    /// Query the knowledge base by slot index (1–8).
    QueryKnowledge { slot_id: u8, query: String },
    /// Read or write memory at a path.
    MemoryOp { path: String, value: Option<serde_json::Value> },
    /// Generic data ingestion (e.g. lead capture, form submit). Payload is use-case specific.
    IngestData { payload: Option<serde_json::Value> },
    /// Assemble context from memory and knowledge slots for a given context id (e.g. lead_id).
    AssembleContext { context_id: String },
    /// Chain: AssembleContext -> ModelRouter to produce a final generated response.
    GenerateFinalResponse { context_id: String },
    /// Dynamic: Blueprint maps intent to skill list; orchestrator runs the chain.
    AutonomousGoal { intent: String, context: Option<serde_json::Value> },
    /// Update a knowledge slot (1–8) from an external source (URL or inline HTML).
    UpdateKnowledgeSlot {
        slot_id: u8,
        source_url: Option<String>,
        source_html: Option<String>,
    },
    /// Custom goal for extension.
    Custom(String),
}
