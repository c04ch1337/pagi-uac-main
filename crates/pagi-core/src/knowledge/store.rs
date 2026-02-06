//! Sled-backed store with one tree per KB slot (kb1–kb8).
//! Slot metadata can be initialized with `pagi_init_kb_metadata()`.
//!
//! ## L2 Memory Architecture — Holistic Ontology (Distributed Cognitive Map)
//!
//! | Slot | KbType  | Purpose (Cognitive Domain)                          |
//! |------|--------|------------------------------------------------------|
//! | 1    | Pneuma | Vision: Agent identity, mission, evolving playbook  |
//! | 2    | Oikos  | Context: Workspace scan, "where" the system lives    |
//! | 3    | Logos  | Pure knowledge: Research, distilled information     |
//! | 4    | Chronos| Temporal: Conversation history, short/long-term     |
//! | 5    | Techne | Capability: Skills registry, blueprints, how-to      |
//! | 6    | Ethos  | Guardrails: Security, audit, "should" constraints   |
//! | 7    | Kardia | Affective: User preferences, "who" and vibe        |
//! | 8    | Soma   | Execution: Physical interface, side effects, buffer  |

use serde::{Deserialize, Serialize};
use sled::Db;
use std::path::Path;
use uuid::Uuid;

const DEFAULT_PATH: &str = "./data/pagi_knowledge";

/// Tree names for the 8 KB slots (internal Sled tree identifiers).
const TREE_NAMES: [&str; 8] = [
    "kb1_identity",
    "kb2_techdocs",
    "kb3_research",
    "kb4_memory",
    "kb5_skills",
    "kb6_security",
    "kb7_personal",
    "kb8_buffer",
];

/// Human-readable names for the 8 knowledge base slots (Holistic Ontology).
pub const SLOT_LABELS: [&str; 8] = [
    "Pneuma (Vision)",      // KB-1: Identity, mission, evolving playbook
    "Oikos (Context)",      // KB-2: Workspace, "where" the system lives
    "Logos (Knowledge)",    // KB-3: Research, distilled information
    "Chronos (Temporal)",   // KB-4: Memory, conversation history
    "Techne (Capability)",  // KB-5: Skills, blueprints, how-to
    "Ethos (Guardrails)",   // KB-6: Security, audit, constraints
    "Kardia (Affective)",   // KB-7: User prefs, "who" and vibe
    "Soma (Execution)",     // KB-8: Physical interface, buffer, side effects
];

/// Knowledge Base type enum for type-safe slot references (Holistic Ontology).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum KbType {
    /// KB-1: Vision — agent identity, mission, evolving playbook
    Pneuma = 1,
    /// KB-2: Context — workspace scan, "where" the system lives
    Oikos = 2,
    /// KB-3: Pure knowledge — research, distilled information (Internal Wikipedia)
    Logos = 3,
    /// KB-4: Temporal — conversation history, short/long-term
    Chronos = 4,
    /// KB-5: Capability — skills registry, blueprints, "how"
    Techne = 5,
    /// KB-6: Guardrails — security, audit, "should"
    Ethos = 6,
    /// KB-7: Affective — user preferences, "who" and vibe
    Kardia = 7,
    /// KB-8: Execution — physical interface, side effects, buffer
    Soma = 8,
}

impl KbType {
    /// Returns the slot ID (1-8) for this KB type.
    #[inline]
    pub fn slot_id(&self) -> u8 {
        *self as u8
    }

    /// Returns the human-readable label for this KB type.
    #[inline]
    pub fn label(&self) -> &'static str {
        SLOT_LABELS[self.slot_id() as usize - 1]
    }

    /// Returns the internal tree name for this KB type.
    #[inline]
    pub fn tree_name(&self) -> &'static str {
        TREE_NAMES[self.slot_id() as usize - 1]
    }

    /// Creates a KbType from a slot ID (1-8). Returns None if out of range.
    pub fn from_slot_id(slot_id: u8) -> Option<Self> {
        match slot_id {
            1 => Some(Self::Pneuma),
            2 => Some(Self::Oikos),
            3 => Some(Self::Logos),
            4 => Some(Self::Chronos),
            5 => Some(Self::Techne),
            6 => Some(Self::Ethos),
            7 => Some(Self::Kardia),
            8 => Some(Self::Soma),
            _ => None,
        }
    }

    /// Returns all KB types in order (Holistic Ontology).
    pub fn all() -> [Self; 8] {
        [
            Self::Pneuma,
            Self::Oikos,
            Self::Logos,
            Self::Chronos,
            Self::Techne,
            Self::Ethos,
            Self::Kardia,
            Self::Soma,
        ]
    }
}

/// Standard record structure for KB entries.
/// Designed for future vector/semantic search capabilities.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KbRecord {
    /// Unique identifier for this record.
    pub id: Uuid,
    /// The main content/value stored in this record.
    pub content: String,
    /// Flexible metadata for tags, model_id, embeddings, etc.
    /// Reserved keys: `tags`, `model_id`, `embedding_model`, `vector_dims`
    pub metadata: serde_json::Value,
    /// Optional semantic embedding vector for the record content.
    ///
    /// Intended primarily for KB-3 (Research) semantic search.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub embedding: Option<Vec<f32>>,
    /// Unix timestamp (milliseconds) when this record was created/updated.
    pub timestamp: i64,
}

/// Record stored in KB-5 for skill discovery (Skill Registry / KB-5).
///
/// This is a minimal, LLM-oriented manifest schema:
/// - `slug`: stable identifier (e.g. "fs_workspace_analyzer")
/// - `description`: natural language capability description
/// - `schema`: JSON schema-ish object describing arguments
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillRecord {
    pub slug: String,
    pub description: String,
    pub schema: serde_json::Value,
}

/// Episodic memory event for **KB_CHRONOS** (the Historian).
///
/// Every successful skill execution or significant update can create a timestamped
/// "Memory Event" so the Agent can reason about past actions and self-correct.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventRecord {
    /// Unix timestamp (milliseconds) when the event occurred.
    pub timestamp_ms: i64,
    /// Source cognitive domain (e.g. "Soma", "Pneuma", "Logos").
    pub source_kb: String,
    /// Skill or action name, if applicable.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub skill_name: Option<String>,
    /// Human-readable summary: what was done and why it matters.
    pub reflection: String,
    /// Optional outcome summary (e.g. "inserted key X", "returned 5 results").
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub outcome: Option<String>,
}

impl EventRecord {
    /// Creates an event with the current timestamp.
    pub fn now(source_kb: impl Into<String>, reflection: impl Into<String>) -> Self {
        let timestamp_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as i64)
            .unwrap_or(0);
        Self {
            timestamp_ms,
            source_kb: source_kb.into(),
            skill_name: None,
            reflection: reflection.into(),
            outcome: None,
        }
    }

    pub fn with_skill(mut self, name: impl Into<String>) -> Self {
        self.skill_name = Some(name.into());
        self
    }

    pub fn with_outcome(mut self, outcome: impl Into<String>) -> Self {
        self.outcome = Some(outcome.into());
        self
    }

    /// Serializes to JSON bytes for storage in Chronos.
    pub fn to_bytes(&self) -> Vec<u8> {
        serde_json::to_vec(self).unwrap_or_default()
    }

    /// Deserializes from JSON bytes.
    pub fn from_bytes(bytes: &[u8]) -> Option<Self> {
        serde_json::from_slice(bytes).ok()
    }
}

/// Default key for the active safety policy in **KB_ETHOS**.
pub const ETHOS_DEFAULT_POLICY_KEY: &str = "policy/default";

/// Guardrail policy record for **KB_ETHOS** (the Sage / Safe Operating Parameters).
///
/// Consulted before executing skills to ensure actions align with the 2026 mission.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PolicyRecord {
    /// Skill names or action patterns that are always forbidden.
    #[serde(default)]
    pub forbidden_actions: Vec<String>,
    /// Keywords that, if present in payload content, trigger block or approval.
    /// E.g. "api_key", "secret", "password" — do not write these to the sandbox.
    #[serde(default)]
    pub sensitive_keywords: Vec<String>,
    /// When true, actions that match sensitive_keywords are blocked (no automatic approval).
    #[serde(default = "default_true")]
    pub approval_required: bool,
}

fn default_true() -> bool {
    true
}

impl Default for PolicyRecord {
    fn default() -> Self {
        Self {
            forbidden_actions: Vec::new(),
            sensitive_keywords: vec![
                "api_key".to_string(),
                "apikey".to_string(),
                "secret".to_string(),
                "password".to_string(),
                "token".to_string(),
                "credentials".to_string(),
            ],
            approval_required: true,
        }
    }
}

impl PolicyRecord {
    /// Serializes to JSON bytes for storage in Ethos.
    pub fn to_bytes(&self) -> Vec<u8> {
        serde_json::to_vec(self).unwrap_or_default()
    }

    /// Deserializes from JSON bytes.
    pub fn from_bytes(bytes: &[u8]) -> Option<Self> {
        serde_json::from_slice(bytes).ok()
    }

    /// Returns true if the intended action is allowed; false if it violates policy.
    /// `content_for_scan` is the string to check for sensitive keywords (e.g. payload content).
    pub fn allows(&self, skill_name: &str, content_for_scan: &str) -> AlignmentResult {
        let skill_lower = skill_name.to_lowercase();
        for forbidden in &self.forbidden_actions {
            if skill_lower.contains(&forbidden.to_lowercase()) {
                return AlignmentResult::Fail {
                    reason: format!("Skill '{}' is forbidden by policy", skill_name),
                };
            }
        }
        let content_lower = content_for_scan.to_lowercase();
        for kw in &self.sensitive_keywords {
            if content_lower.contains(&kw.to_lowercase()) && self.approval_required {
                return AlignmentResult::Fail {
                    reason: format!(
                        "Content contains sensitive keyword '{}'; policy requires approval",
                        kw
                    ),
                };
            }
        }
        AlignmentResult::Pass
    }
}

/// Result of an Ethos alignment check.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AlignmentResult {
    Pass,
    Fail { reason: String },
}

/// Key for relation records in **KB_KARDIA**. Full key: `relation/{owner_agent_id}/{target_id}`.
/// In multi-agent mode, each agent has its own view of relations (to users and other agents).
pub fn kardia_relation_key(owner_agent_id: &str, target_id: &str) -> String {
    let owner = if owner_agent_id.is_empty() {
        "default"
    } else {
        owner_agent_id
    };
    format!("relation/{}/{}", owner, target_id)
}

/// Inter-agent message stored in **KB_SOMA** inbox (`inbox/{target_agent_id}/{key}`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentMessage {
    pub id: String,
    pub from_agent_id: String,
    pub target_agent_id: String,
    pub payload: serde_json::Value,
    pub timestamp_ms: i64,
    /// Heartbeat inbox acknowledgment flag.
    ///
    /// When true, the Heartbeat should skip this message to avoid repeated auto-replies.
    /// Defaults to false for backwards compatibility with older records.
    #[serde(default)]
    pub is_processed: bool,
}

impl AgentMessage {
    pub fn to_bytes(&self) -> Vec<u8> {
        serde_json::to_vec(self).unwrap_or_default()
    }
    pub fn from_bytes(bytes: &[u8]) -> Option<Self> {
        serde_json::from_slice(bytes).ok()
    }
}

/// Relationship/social record for **KB_KARDIA** (the Heart).
///
/// Stores interaction sentiment, communication style, and trust so the agent
/// can adapt its voice (Pneuma) based on the user (Kardia).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RelationRecord {
    /// User or tenant identifier.
    pub user_id: String,
    /// Trust/rapport score in [0.0, 1.0]. Optional for backward compatibility.
    #[serde(default = "default_trust")]
    pub trust_score: f32,
    /// Detected or preferred communication style (e.g. formal, witty, urgent, casual).
    #[serde(default)]
    pub communication_style: String,
    /// Last inferred sentiment (e.g. frustrated, neutral, positive, angry).
    #[serde(default)]
    pub last_sentiment: String,
    /// Unix timestamp (ms) of last update.
    #[serde(default)]
    pub last_updated_ms: i64,
}

fn default_trust() -> f32 {
    0.5
}

impl RelationRecord {
    pub fn new(user_id: impl Into<String>) -> Self {
        let user_id = user_id.into();
        let last_updated_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as i64)
            .unwrap_or(0);
        Self {
            user_id: user_id.clone(),
            trust_score: 0.5,
            communication_style: String::new(),
            last_sentiment: String::new(),
            last_updated_ms,
        }
    }

    pub fn with_trust_score(mut self, score: f32) -> Self {
        self.trust_score = score.clamp(0.0, 1.0);
        self
    }

    pub fn with_communication_style(mut self, style: impl Into<String>) -> Self {
        self.communication_style = style.into();
        self
    }

    pub fn with_sentiment(mut self, sentiment: impl Into<String>) -> Self {
        self.last_sentiment = sentiment.into();
        self.last_updated_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as i64)
            .unwrap_or(0);
        self
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        serde_json::to_vec(self).unwrap_or_default()
    }

    pub fn from_bytes(bytes: &[u8]) -> Option<Self> {
        serde_json::from_slice(bytes).ok()
    }

    /// One-line context string for injection into LLM prompts.
    pub fn prompt_context(&self) -> String {
        let mut parts = Vec::new();
        if !self.last_sentiment.is_empty() {
            parts.push(format!("User sentiment: {}", self.last_sentiment));
        }
        if !self.communication_style.is_empty() {
            parts.push(format!("Communication style: {}", self.communication_style));
        }
        if parts.is_empty() {
            return String::new();
        }
        format!("[Relationship context: {}. Adjust your tone accordingly.]\n\n", parts.join(". "))
    }
}

impl KbRecord {
    /// Creates a new KbRecord with the given content.
    pub fn new(content: impl Into<String>) -> Self {
        Self {
            id: Uuid::new_v4(),
            content: content.into(),
            metadata: serde_json::json!({}),
            embedding: None,
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_millis() as i64)
                .unwrap_or(0),
        }
    }

    /// Creates a new KbRecord with content and metadata.
    pub fn with_metadata(content: impl Into<String>, metadata: serde_json::Value) -> Self {
        Self {
            id: Uuid::new_v4(),
            content: content.into(),
            metadata,
            embedding: None,
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_millis() as i64)
                .unwrap_or(0),
        }
    }

    /// Creates a new KbRecord with content, metadata, and an embedding vector.
    pub fn with_embedding(
        content: impl Into<String>,
        metadata: serde_json::Value,
        embedding: Vec<f32>,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            content: content.into(),
            metadata,
            embedding: Some(embedding),
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_millis() as i64)
                .unwrap_or(0),
        }
    }

    /// Serializes this record to JSON bytes for storage.
    pub fn to_bytes(&self) -> Vec<u8> {
        serde_json::to_vec(self).unwrap_or_default()
    }

    /// Deserializes a record from JSON bytes.
    pub fn from_bytes(bytes: &[u8]) -> Option<Self> {
        serde_json::from_slice(bytes).ok()
    }
}

/// Returns the descriptive label for a slot (1..=8). Falls back to "Unknown" if out of range.
#[inline]
pub fn pagi_kb_slot_label(slot_id: u8) -> &'static str {
    if (1..=8).contains(&slot_id) {
        SLOT_LABELS[slot_id as usize - 1]
    } else {
        "Unknown"
    }
}

/// Store with 8 Sled trees, one per knowledge base slot.
/// Provides the L2 Memory layer for the PAGI Orchestrator.
pub struct KnowledgeStore {
    db: Db,
}

impl KnowledgeStore {
    /// Opens or creates the knowledge DB at `./data/pagi_knowledge`.
    pub fn new() -> Result<Self, sled::Error> {
        Self::open_path(DEFAULT_PATH)
    }

    /// Opens or creates the knowledge DB at the given path.
    pub fn open_path<P: AsRef<Path>>(path: P) -> Result<Self, sled::Error> {
        let db = sled::open(path)?;
        Ok(Self { db })
    }

    fn tree_name(slot_id: u8) -> &'static str {
        if (1..=8).contains(&slot_id) {
            TREE_NAMES[slot_id as usize - 1]
        } else {
            TREE_NAMES[0]
        }
    }

    /// Returns the value at `key` in the tree for `slot_id` (1–8).
    pub fn get(&self, slot_id: u8, key: &str) -> Result<Option<Vec<u8>>, sled::Error> {
        let tree = self.db.open_tree(Self::tree_name(slot_id))?;
        let v = tree.get(key.as_bytes())?;
        Ok(v.map(|iv| iv.to_vec()))
    }

    /// Inserts `value` at `key` in the tree for `slot_id` (1–8).
    /// Logs the write operation to the tracing system.
    pub fn insert(
        &self,
        slot_id: u8,
        key: &str,
        value: &[u8],
    ) -> Result<Option<Vec<u8>>, sled::Error> {
        let tree_name = Self::tree_name(slot_id);
        let tree = self.db.open_tree(tree_name)?;
        let prev = tree.insert(key.as_bytes(), value)?;
        
        // Log KB write for observability
        let kb_label = pagi_kb_slot_label(slot_id);
        let is_update = prev.is_some();
        tracing::info!(
            target: "pagi::knowledge",
            kb_slot = slot_id,
            kb_name = kb_label,
            key = key,
            bytes = value.len(),
            action = if is_update { "UPDATE" } else { "INSERT" },
            "KB-{} [{}] {} key '{}' ({} bytes)",
            slot_id,
            kb_label,
            if is_update { "updated" } else { "inserted" },
            key,
            value.len()
        );
        
        Ok(prev.map(|iv| iv.to_vec()))
    }

    /// Inserts a KbRecord at the specified key in the tree for `slot_id` (1–8).
    /// This is the preferred method for storing structured records.
    pub fn insert_record(
        &self,
        slot_id: u8,
        key: &str,
        record: &KbRecord,
    ) -> Result<Option<Vec<u8>>, sled::Error> {
        self.insert(slot_id, key, &record.to_bytes())
    }

    /// Retrieves a KbRecord from the specified key in the tree for `slot_id` (1–8).
    pub fn get_record(&self, slot_id: u8, key: &str) -> Result<Option<KbRecord>, sled::Error> {
        let bytes = self.get(slot_id, key)?;
        Ok(bytes.and_then(|b| KbRecord::from_bytes(&b)))
    }

    /// Removes the key in the tree for `slot_id` (1–8). Returns the previous value if present.
    /// Logs the removal operation to the tracing system.
    pub fn remove(&self, slot_id: u8, key: &str) -> Result<Option<Vec<u8>>, sled::Error> {
        let tree = self.db.open_tree(Self::tree_name(slot_id))?;
        let prev = tree.remove(key.as_bytes())?;
        
        if prev.is_some() {
            let kb_label = pagi_kb_slot_label(slot_id);
            tracing::info!(
                target: "pagi::knowledge",
                kb_slot = slot_id,
                kb_name = kb_label,
                key = key,
                action = "REMOVE",
                "KB-{} [{}] removed key '{}'",
                slot_id,
                kb_label,
                key
            );
        }
        
        Ok(prev.map(|iv| iv.to_vec()))
    }

    /// Returns all keys in the tree for `slot_id` (1–8). Order is not guaranteed.
    pub fn scan_keys(&self, slot_id: u8) -> Result<Vec<String>, sled::Error> {
        let tree = self.db.open_tree(Self::tree_name(slot_id))?;
        let keys: Vec<String> = tree
            .iter()
            .keys()
            .filter_map(|k| k.ok())
            .filter_map(|k| String::from_utf8(k.to_vec()).ok())
            .collect();
        Ok(keys)
    }

    /// Returns all key/value pairs in the tree for `slot_id` (1–8).
    ///
    /// This is useful for implementing higher-level search (including semantic search)
    /// without exposing the underlying sled `Tree`.
    pub fn scan_kv(&self, slot_id: u8) -> Result<Vec<(String, Vec<u8>)>, sled::Error> {
        let tree = self.db.open_tree(Self::tree_name(slot_id))?;
        let mut out = Vec::new();
        for item in tree.iter() {
            let (k, v) = item?;
            let key = String::from_utf8(k.to_vec()).unwrap_or_default();
            out.push((key, v.to_vec()));
        }
        Ok(out)
    }

    /// Returns all successfully-deserialized [`KbRecord`](crates/pagi-core/src/knowledge/store.rs:119)
    /// values from the given slot.
    pub fn scan_records(&self, slot_id: u8) -> Result<Vec<(String, KbRecord)>, sled::Error> {
        let kv = self.scan_kv(slot_id)?;
        let mut out = Vec::new();
        for (k, bytes) in kv {
            if let Some(rec) = KbRecord::from_bytes(&bytes) {
                out.push((k, rec));
            }
        }
        Ok(out)
    }

    /// Returns the number of entries in the tree for `slot_id` (1–8).
    pub fn count(&self, slot_id: u8) -> Result<usize, sled::Error> {
        let tree = self.db.open_tree(Self::tree_name(slot_id))?;
        Ok(tree.len())
    }

    /// Returns status information for all 8 KB slots.
    pub fn get_all_status(&self) -> Vec<KbStatus> {
        KbType::all()
            .iter()
            .map(|kb_type| {
                let slot_id = kb_type.slot_id();
                let tree_result = self.db.open_tree(kb_type.tree_name());
                match tree_result {
                    Ok(tree) => KbStatus {
                        slot_id,
                        name: kb_type.label().to_string(),
                        tree_name: kb_type.tree_name().to_string(),
                        connected: true,
                        entry_count: tree.len(),
                        error: None,
                    },
                    Err(e) => KbStatus {
                        slot_id,
                        name: kb_type.label().to_string(),
                        tree_name: kb_type.tree_name().to_string(),
                        connected: false,
                        entry_count: 0,
                        error: Some(e.to_string()),
                    },
                }
            })
            .collect()
    }

    /// Initializes the 8 Sled trees by inserting a `metadata` key in each tree describing its purpose.
    /// Safe to call multiple times (overwrites existing metadata). Call after opening the store (e.g. at startup).
    pub fn pagi_init_kb_metadata(&self) -> Result<(), sled::Error> {
        tracing::info!(target: "pagi::knowledge", "Initializing 8 Knowledge Base trees (L2 Memory)...");
        
        for kb_type in KbType::all() {
            let slot_id = kb_type.slot_id();
            let label = kb_type.label();
            let tree_name = kb_type.tree_name();
            
            let metadata = serde_json::json!({
                "slot_id": slot_id,
                "name": label,
                "tree_name": tree_name,
                "purpose": label,
                "kb_type": format!("{:?}", kb_type),
                "initialized_at": std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.as_millis() as i64)
                    .unwrap_or(0),
                "vector_metadata": {
                    "embedding_model": null,
                    "vector_dims": null,
                    "semantic_search_enabled": false
                }
            });
            let bytes = metadata.to_string().into_bytes();
            
            // Use direct tree insert to avoid double-logging during init
            let tree = self.db.open_tree(tree_name)?;
            tree.insert("__kb_metadata__", bytes.as_slice())?;
            
            tracing::info!(
                target: "pagi::knowledge",
                kb_slot = slot_id,
                kb_name = label,
                tree = tree_name,
                "KB-{} [{}] initialized (tree: {})",
                slot_id,
                label,
                tree_name
            );
        }
        
        tracing::info!(target: "pagi::knowledge", "✓ All 8 Knowledge Bases initialized successfully");
        Ok(())
    }

    /// Appends an episodic memory event to **KB_CHRONOS** (the Historian).
    ///
    /// Key format: `event/{agent_id}/{timestamp_ms}_{uuid}` so each agent has its own memory stream.
    /// Use `agent_id` = `"default"` for single-agent mode.
    pub fn append_chronos_event(
        &self,
        agent_id: &str,
        event: &EventRecord,
    ) -> Result<(), sled::Error> {
        let slot_id = KbType::Chronos.slot_id();
        let agent_prefix = if agent_id.is_empty() { "default" } else { agent_id };
        let key = format!(
            "event/{}/{}_{}",
            agent_prefix,
            event.timestamp_ms,
            Uuid::new_v4().simple()
        );
        self.insert(slot_id, &key, &event.to_bytes())?;
        tracing::debug!(
            target: "pagi::chronos",
            agent_id = %agent_prefix,
            key = %key,
            source = %event.source_kb,
            "Chronos: episodic event recorded"
        );
        Ok(())
    }

    /// Returns the most recent episodic events from **KB_CHRONOS** for the given agent, newest first.
    ///
    /// Used by the "recall_past_actions" skill so the Agent can answer "What did you do recently?"
    pub fn get_recent_chronos_events(
        &self,
        agent_id: &str,
        limit: usize,
    ) -> Result<Vec<EventRecord>, sled::Error> {
        let slot_id = KbType::Chronos.slot_id();
        let agent_prefix = if agent_id.is_empty() { "default" } else { agent_id };
        let prefix = format!("event/{}", agent_prefix);
        let mut events: Vec<(i64, EventRecord)> = self
            .scan_kv(slot_id)?
            .into_iter()
            .filter(|(k, _)| k.starts_with(&prefix))
            .filter_map(|(_, bytes)| EventRecord::from_bytes(&bytes).map(|e| (e.timestamp_ms, e)))
            .collect();
        events.sort_by(|a, b| b.0.cmp(&a.0));
        Ok(events.into_iter().take(limit).map(|(_, e)| e).collect())
    }

    /// Returns the active safety policy from **KB_ETHOS**, if present.
    pub fn get_ethos_policy(&self) -> Option<PolicyRecord> {
        let slot_id = KbType::Ethos.slot_id();
        self.get(slot_id, ETHOS_DEFAULT_POLICY_KEY)
            .ok()
            .flatten()
            .and_then(|b| PolicyRecord::from_bytes(&b))
    }

    /// Writes the active safety policy to **KB_ETHOS**.
    pub fn set_ethos_policy(&self, policy: &PolicyRecord) -> Result<(), sled::Error> {
        let slot_id = KbType::Ethos.slot_id();
        self.insert(slot_id, ETHOS_DEFAULT_POLICY_KEY, &policy.to_bytes())?;
        Ok(())
    }

    /// Returns the relation record from **KB_KARDIA** for (owner_agent_id, target_id).
    /// Use owner_agent_id = "default" for single-agent mode.
    pub fn get_kardia_relation(
        &self,
        owner_agent_id: &str,
        target_id: &str,
    ) -> Option<RelationRecord> {
        let slot_id = KbType::Kardia.slot_id();
        let key = kardia_relation_key(owner_agent_id, target_id);
        self.get(slot_id, &key).ok().flatten().and_then(|b| RelationRecord::from_bytes(&b))
    }

    /// Writes the relation record to **KB_KARDIA** under (owner_agent_id, record.user_id).
    pub fn set_kardia_relation(
        &self,
        owner_agent_id: &str,
        record: &RelationRecord,
    ) -> Result<(), sled::Error> {
        let slot_id = KbType::Kardia.slot_id();
        let key = kardia_relation_key(owner_agent_id, &record.user_id);
        self.insert(slot_id, &key, &record.to_bytes())?;
        Ok(())
    }

    /// Pushes an inter-agent message to **KB_SOMA** (inbox for target agent).
    /// Key: `inbox/{target_agent_id}/{timestamp_ms}_{uuid}`. Returns the message id.
    pub fn push_agent_message(
        &self,
        from_agent_id: &str,
        target_agent_id: &str,
        payload: &serde_json::Value,
    ) -> Result<String, sled::Error> {
        let slot_id = KbType::Soma.slot_id();
        let ts = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as i64)
            .unwrap_or(0);
        let id = Uuid::new_v4().simple().to_string();
        let key = format!("inbox/{}/{}_{}", target_agent_id, ts, id);
        let msg = AgentMessage {
            id: id.clone(),
            from_agent_id: from_agent_id.to_string(),
            target_agent_id: target_agent_id.to_string(),
            payload: payload.clone(),
            timestamp_ms: ts,
            is_processed: false,
        };
        self.insert(slot_id, &key, &msg.to_bytes())?;
        Ok(id)
    }

    /// Returns the most recent messages for an agent from **KB_SOMA** inbox, newest first,
    /// including the underlying inbox key.
    ///
    /// This is primarily used by the Heartbeat so it can mark messages as processed
    /// without deleting them (preserving KB_SOMA history).
    pub fn get_agent_messages_with_keys(
        &self,
        target_agent_id: &str,
        limit: usize,
    ) -> Result<Vec<(String, AgentMessage)>, sled::Error> {
        let slot_id = KbType::Soma.slot_id();
        let prefix = format!("inbox/{}/", target_agent_id);
        let mut messages: Vec<(i64, String, AgentMessage)> = self
            .scan_kv(slot_id)?
            .into_iter()
            .filter(|(k, _)| k.starts_with(&prefix))
            .filter_map(|(k, bytes)| AgentMessage::from_bytes(&bytes).map(|m| (m.timestamp_ms, k, m)))
            .collect();
        messages.sort_by(|a, b| b.0.cmp(&a.0));
        Ok(messages
            .into_iter()
            .take(limit)
            .map(|(_ts, k, m)| (k, m))
            .collect())
    }

    /// Returns the most recent messages for an agent from **KB_SOMA** inbox, newest first.
    pub fn get_agent_messages(
        &self,
        target_agent_id: &str,
        limit: usize,
    ) -> Result<Vec<AgentMessage>, sled::Error> {
        let slot_id = KbType::Soma.slot_id();
        let prefix = format!("inbox/{}", target_agent_id);
        let mut messages: Vec<(i64, AgentMessage)> = self
            .scan_kv(slot_id)?
            .into_iter()
            .filter(|(k, _)| k.starts_with(&prefix))
            .filter_map(|(_, bytes)| AgentMessage::from_bytes(&bytes).map(|m| (m.timestamp_ms, m)))
            .collect();
        messages.sort_by(|a, b| b.0.cmp(&a.0));
        Ok(messages.into_iter().take(limit).map(|(_, m)| m).collect())
    }

    /// Returns all skill manifests stored in KB-5 (Techne / Skills & Blueprints).
    ///
    /// Convention:
    /// - KB slot: 5
    /// - key prefix: `skills/`
    /// - value: JSON-encoded [`SkillRecord`](crates/pagi-core/src/knowledge/store.rs:1)
    pub fn get_skills(&self) -> Vec<SkillRecord> {
        let slot_id = KbType::Techne.slot_id();
        let tree = match self.db.open_tree(Self::tree_name(slot_id)) {
            Ok(t) => t,
            Err(_) => return Vec::new(),
        };

        let mut out = Vec::new();
        for item in tree.iter() {
            let (k, v) = match item {
                Ok(kv) => kv,
                Err(_) => continue,
            };
            let key = match String::from_utf8(k.to_vec()) {
                Ok(s) => s,
                Err(_) => continue,
            };
            if !key.starts_with("skills/") {
                continue;
            }
            let bytes = v.to_vec();
            if let Ok(rec) = serde_json::from_slice::<SkillRecord>(&bytes) {
                out.push(rec);
            }
        }

        // Stable ordering for deterministic prompts
        out.sort_by(|a, b| a.slug.cmp(&b.slug));
        out
    }
}

/// Status information for a single KB slot.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KbStatus {
    pub slot_id: u8,
    pub name: String,
    pub tree_name: String,
    pub connected: bool,
    pub entry_count: usize,
    pub error: Option<String>,
}
