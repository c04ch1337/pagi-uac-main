//! Shared types used across all UAC crates.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

/// Default agent ID when not specified (single-agent mode).
pub const DEFAULT_AGENT_ID: &str = "default";

/// Tenant context for multi-tenant and multi-agent isolation across the UAC system.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TenantContext {
    /// Unique tenant identifier.
    pub tenant_id: String,
    /// Optional correlation id for request tracing.
    pub correlation_id: Option<String>,
    /// Agent instance ID for multi-agent mode. Chronos and Kardia are keyed by this.
    /// When None or empty, [`DEFAULT_AGENT_ID`] is used.
    #[serde(default)]
    pub agent_id: Option<String>,
}

impl TenantContext {
    /// Resolved agent ID (never empty).
    pub fn resolved_agent_id(&self) -> &str {
        self.agent_id
            .as_deref()
            .filter(|s| !s.is_empty())
            .unwrap_or(DEFAULT_AGENT_ID)
    }
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

/// Global application configuration (Gateway + identity). Load from TOML or env.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoreConfig {
    /// Application identity (e.g. "Stockdale Middleman", "Fintech Support").
    pub app_name: String,
    /// HTTP port for the gateway.
    pub port: u16,
    /// Base directory for Sled DBs (memory vault and knowledge store paths are derived from this).
    pub storage_path: String,
    /// LLM mode (e.g. "mock", "openai", "local").
    pub llm_mode: String,

    /// If true, `pagi-gateway` will serve the static UI from `pagi-frontend/`. (Config alias: `ui_enabled`)
    #[serde(default, alias = "ui_enabled")]
    pub frontend_enabled: bool,
    /// Human-readable labels for knowledge slots 1–8. Keys in file are string numerals "1".."8".
    #[serde(default)]
    pub slot_labels: HashMap<String, String>,
}

impl CoreConfig {
    /// Slot labels as `u8` -> label. Keys that are not 1–8 are skipped.
    pub fn slot_labels_map(&self) -> HashMap<u8, String> {
        self.slot_labels
            .iter()
            .filter_map(|(k, v)| k.parse::<u8>().ok().filter(|&n| (1..=8).contains(&n)).map(|n| (n, v.clone())))
            .collect()
    }

    /// Load config from file and environment. Precedence: env `PAGI_CONFIG` path > `config/gateway.toml` > defaults.
    pub fn load() -> Result<Self, config::ConfigError> {
        let config_path = std::env::var("PAGI_CONFIG").unwrap_or_else(|_| "config/gateway".to_string());
        let builder = config::Config::builder()
            .set_default("app_name", "UAC Gateway")?
            .set_default("port", 8001_i64)?
            .set_default("storage_path", "./data")?
            .set_default("llm_mode", "mock")?
            .set_default("frontend_enabled", false)?;

        let path = Path::new(&config_path);
        let builder = if path.exists() {
            builder.add_source(config::File::from(path))
        } else {
            builder
        };

        let built = builder
            .add_source(config::Environment::with_prefix("PAGI").separator("__"))
            .build()?;

        built.try_deserialize()
    }
}
