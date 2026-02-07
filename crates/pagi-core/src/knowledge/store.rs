//! Sled-backed store with one tree per KB slot (kb1–kb9).
//! Slot metadata can be initialized with `pagi_init_kb_metadata()`.
//!
//! ## L2 Memory Architecture — Holistic Ontology (Distributed Cognitive Map)
//!
//! | Slot | KbType  | Purpose (Cognitive Domain)                          | Security       |
//! |------|--------|------------------------------------------------------|----------------|
//! | 1    | Pneuma | Vision: Agent identity, mission, evolving playbook  | Standard (Sled)|
//! | 2    | Oikos  | Context: Workspace scan, "where" the system lives    | Standard (Sled)|
//! | 3    | Logos  | Pure knowledge: Research, distilled information     | Standard (Sled)|
//! | 4    | Chronos| Temporal: Conversation history, short/long-term     | Standard (Sled)|
//! | 5    | Techne | Capability: Skills registry, blueprints, how-to      | Standard (Sled)|
//! | 6    | Ethos  | Guardrails: Security, audit, "should" constraints   | Standard (Sled)|
//! | 7    | Kardia | Affective: User preferences, "who" and vibe        | Standard (Sled)|
//! | 8    | Soma   | Execution: Physical interface, side effects, buffer  | Standard (Sled)|
//! | 9    | Shadow | The Vault: Trauma, anchors, private journaling      | **AES-256-GCM**|

use crate::shared::{
    BiometricState, EthosPolicy, GovernedTask, MentalState, PersonRecord, SomaState,
    KARDIA_PEOPLE_PREFIX, MENTAL_STATE_KEY,
};
use super::vault::{EmotionalAnchor, SecretVault, VaultError};
use serde::{Deserialize, Serialize};
use sled::Db;
use std::path::Path;
use uuid::Uuid;

const DEFAULT_PATH: &str = "./data/pagi_knowledge";

/// Tree names for the 9 KB slots (internal Sled tree identifiers).
const TREE_NAMES: [&str; 9] = [
    "kb1_identity",
    "kb2_techdocs",
    "kb3_research",
    "kb4_memory",
    "kb5_skills",
    "kb6_security",
    "kb7_personal",
    "kb8_buffer",
    "kb9_shadow",
];

/// Human-readable names for the 9 knowledge base slots (Holistic Ontology + Shadow Vault).
pub const SLOT_LABELS: [&str; 9] = [
    "Pneuma (Vision)",      // KB-1: Identity, mission, evolving playbook
    "Oikos (Context)",      // KB-2: Workspace, "where" the system lives
    "Logos (Knowledge)",    // KB-3: Research, distilled information
    "Chronos (Temporal)",   // KB-4: Memory, conversation history
    "Techne (Capability)",  // KB-5: Skills, blueprints, how-to
    "Ethos (Guardrails)",   // KB-6: Security, audit, constraints
    "Kardia (Affective)",   // KB-7: User prefs, "who" and vibe
    "Soma (Execution)",     // KB-8: Physical interface, buffer, side effects
    "Shadow (The Vault)",   // KB-9: Encrypted emotional data, trauma, anchors
];

/// Knowledge Base type enum for type-safe slot references (Holistic Ontology + Shadow Vault).
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
    /// KB-9: Shadow (The Vault) — AES-256-GCM encrypted emotional data
    Shadow = 9,
}

/// The Shadow slot ID constant for convenience.
pub const SHADOW_SLOT_ID: u8 = 9;

impl KbType {
    /// Returns the slot ID (1-9) for this KB type.
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

    /// Returns `true` if this slot requires encryption (Shadow Vault).
    #[inline]
    pub fn is_encrypted(&self) -> bool {
        matches!(self, Self::Shadow)
    }

    /// Creates a KbType from a slot ID (1-9). Returns None if out of range.
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
            9 => Some(Self::Shadow),
            _ => None,
        }
    }

    /// Returns all KB types in order (Holistic Ontology), **excluding** Shadow.
    /// Use `all_with_shadow()` to include the encrypted slot.
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

    /// Returns all 9 KB types including the Shadow Vault.
    pub fn all_with_shadow() -> [Self; 9] {
        [
            Self::Pneuma,
            Self::Oikos,
            Self::Logos,
            Self::Chronos,
            Self::Techne,
            Self::Ethos,
            Self::Kardia,
            Self::Soma,
            Self::Shadow,
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

/// Returns the descriptive label for a slot (1..=9). Falls back to "Unknown" if out of range.
#[inline]
pub fn pagi_kb_slot_label(slot_id: u8) -> &'static str {
    if (1..=9).contains(&slot_id) {
        SLOT_LABELS[slot_id as usize - 1]
    } else {
        "Unknown"
    }
}

/// Store with 9 Sled trees (8 standard + 1 encrypted Shadow), one per knowledge base slot.
/// Provides the L2 Memory layer for the PAGI Orchestrator.
///
/// **Slot 9 (Shadow)** is special: all data written to it is automatically encrypted
/// via AES-256-GCM using the `SecretVault`. If no master key is provided, Slot 9
/// remains locked and all operations on it return errors.
pub struct KnowledgeStore {
    db: Db,
    /// The Secret Vault for Slot 9 (Shadow_KB). Initialized from `PAGI_SHADOW_KEY` env var.
    vault: SecretVault,
}

impl KnowledgeStore {
    /// Opens or creates the knowledge DB at `./data/pagi_knowledge`.
    /// The Shadow Vault is initialized from the `PAGI_SHADOW_KEY` environment variable.
    pub fn new() -> Result<Self, sled::Error> {
        Self::open_path(DEFAULT_PATH)
    }

    /// Opens or creates the knowledge DB at the given path.
    /// The Shadow Vault is initialized from the `PAGI_SHADOW_KEY` environment variable.
    pub fn open_path<P: AsRef<Path>>(path: P) -> Result<Self, sled::Error> {
        let db = sled::open(path)?;
        let vault = SecretVault::from_env();
        Ok(Self { db, vault })
    }

    /// Opens or creates the knowledge DB with an explicit master key for the Shadow Vault.
    /// Pass `None` to create a store with a locked vault.
    pub fn open_with_key<P: AsRef<Path>>(path: P, master_key: Option<&[u8; 32]>) -> Result<Self, sled::Error> {
        let db = sled::open(path)?;
        let vault = SecretVault::new(master_key);
        Ok(Self { db, vault })
    }

    /// Returns a reference to the Shadow Vault for direct vault operations.
    pub fn vault(&self) -> &SecretVault {
        &self.vault
    }

    /// Returns `true` if the Shadow Vault (Slot 9) is unlocked and accessible.
    pub fn is_shadow_unlocked(&self) -> bool {
        self.vault.is_unlocked()
    }

    fn tree_name(slot_id: u8) -> &'static str {
        if (1..=9).contains(&slot_id) {
            TREE_NAMES[slot_id as usize - 1]
        } else {
            TREE_NAMES[0]
        }
    }

    /// Returns the value at `key` in the tree for `slot_id` (1–9).
    ///
    /// **Slot 9 (Shadow):** Returns the raw encrypted bytes. Use `get_shadow_anchor()`
    /// or `get_shadow_decrypted()` for automatic decryption.
    pub fn get(&self, slot_id: u8, key: &str) -> Result<Option<Vec<u8>>, sled::Error> {
        let tree = self.db.open_tree(Self::tree_name(slot_id))?;
        let v = tree.get(key.as_bytes())?;
        Ok(v.map(|iv| iv.to_vec()))
    }

    /// Inserts `value` at `key` in the tree for `slot_id` (1–9).
    ///
    /// **Slot 9 (Shadow):** Data is automatically encrypted via AES-256-GCM before storage.
    /// If the Shadow Vault is locked, returns an error. Use `insert_shadow_anchor()` for
    /// typed anchor storage.
    ///
    /// Logs the write operation to the tracing system.
    pub fn insert(
        &self,
        slot_id: u8,
        key: &str,
        value: &[u8],
    ) -> Result<Option<Vec<u8>>, sled::Error> {
        // Slot 9 (Shadow): auto-encrypt before writing
        let effective_value: std::borrow::Cow<'_, [u8]> = if slot_id == SHADOW_SLOT_ID {
            match self.vault.encrypt_blob(value) {
                Ok(encrypted) => std::borrow::Cow::Owned(encrypted),
                Err(VaultError::Locked) => {
                    tracing::warn!(
                        target: "pagi::vault",
                        key = key,
                        "Slot 9 (Shadow) write REJECTED — vault is locked (no master key)"
                    );
                    return Err(sled::Error::Unsupported(
                        "Shadow Vault is locked: provide PAGI_SHADOW_KEY to enable Slot 9".into(),
                    ));
                }
                Err(e) => {
                    tracing::error!(
                        target: "pagi::vault",
                        key = key,
                        error = %e,
                        "Slot 9 (Shadow) encryption failed"
                    );
                    return Err(sled::Error::Unsupported(format!("Shadow encryption error: {}", e).into()));
                }
            }
        } else {
            std::borrow::Cow::Borrowed(value)
        };

        let tree_name = Self::tree_name(slot_id);
        let tree = self.db.open_tree(tree_name)?;
        let prev = tree.insert(key.as_bytes(), effective_value.as_ref())?;
        
        // Log KB write for observability (never log Shadow content)
        let kb_label = pagi_kb_slot_label(slot_id);
        let is_update = prev.is_some();
        if slot_id == SHADOW_SLOT_ID {
            tracing::info!(
                target: "pagi::vault",
                kb_slot = slot_id,
                kb_name = kb_label,
                key = key,
                encrypted_bytes = effective_value.len(),
                action = if is_update { "UPDATE" } else { "INSERT" },
                "KB-9 [Shadow] {} key '{}' ({} encrypted bytes) 🔐",
                if is_update { "updated" } else { "inserted" },
                key,
                effective_value.len()
            );
        } else {
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
        }
        
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

    /// Returns status information for all 9 KB slots (including Shadow Vault).
    pub fn get_all_status(&self) -> Vec<KbStatus> {
        KbType::all_with_shadow()
            .iter()
            .map(|kb_type| {
                let slot_id = kb_type.slot_id();
                let tree_result = self.db.open_tree(kb_type.tree_name());
                match tree_result {
                    Ok(tree) => {
                        let mut status = KbStatus {
                            slot_id,
                            name: kb_type.label().to_string(),
                            tree_name: kb_type.tree_name().to_string(),
                            connected: true,
                            entry_count: tree.len(),
                            error: None,
                        };
                        // Shadow slot: indicate lock status
                        if kb_type.is_encrypted() && !self.vault.is_unlocked() {
                            status.error = Some("LOCKED (no master key)".to_string());
                        }
                        status
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

    /// Returns the active philosophical policy from **KB_ETHOS**, if present.
    /// Stored under key [`crate::ETHOS_POLICY_KEY`] (`ethos/current`).
    pub fn get_ethos_philosophical_policy(&self) -> Option<crate::EthosPolicy> {
        let slot_id = KbType::Ethos.slot_id();
        self.get(slot_id, crate::ETHOS_POLICY_KEY)
            .ok()
            .flatten()
            .and_then(|b| crate::EthosPolicy::from_bytes(&b))
    }

    /// Writes the philosophical policy to **KB_ETHOS** under `ethos/current`.
    pub fn set_ethos_philosophical_policy(
        &self,
        policy: &crate::EthosPolicy,
    ) -> Result<(), sled::Error> {
        let slot_id = KbType::Ethos.slot_id();
        self.insert(slot_id, crate::ETHOS_POLICY_KEY, &policy.to_bytes())?;
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

    /// Key for a person in the Relational Map: `people/{name_slug}`.
    pub fn kardia_person_key(name_slug: &str) -> String {
        format!("{}{}", KARDIA_PEOPLE_PREFIX, name_slug)
    }

    /// Returns a **PersonRecord** from the Relational Map (KB_KARDIA) by name slug.
    pub fn get_person(&self, name_slug: &str) -> Option<PersonRecord> {
        let slot_id = KbType::Kardia.slot_id();
        let key = Self::kardia_person_key(name_slug);
        self.get(slot_id, &key)
            .ok()
            .flatten()
            .and_then(|b| serde_json::from_slice(&b).ok())
    }

    /// Writes a **PersonRecord** to the Relational Map (KB_KARDIA) under `people/{name_slug}`.
    pub fn set_person(&self, record: &PersonRecord) -> Result<(), sled::Error> {
        let slot_id = KbType::Kardia.slot_id();
        let slug = PersonRecord::name_slug(&record.name);
        let key = Self::kardia_person_key(&slug);
        let bytes = serde_json::to_vec(record).unwrap_or_default();
        self.insert(slot_id, &key, &bytes)?;
        Ok(())
    }

    /// Returns all **PersonRecord**s in the Relational Map (KB_KARDIA) with key prefix `people/`.
    pub fn list_people(&self) -> Result<Vec<PersonRecord>, sled::Error> {
        let slot_id = KbType::Kardia.slot_id();
        let kv = self.scan_kv(slot_id)?;
        let prefix = KARDIA_PEOPLE_PREFIX;
        let mut out: Vec<PersonRecord> = kv
            .into_iter()
            .filter(|(k, _)| k.starts_with(prefix))
            .filter_map(|(_, bytes)| serde_json::from_slice(&bytes).ok())
            .collect();
        out.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(out)
    }

    /// Returns the **MentalState** (Emotional Context Layer) from **KB_KARDIA**.
    /// Stored under a global key so the Cognitive Governor can modulate tone and demand.
    pub fn get_mental_state(&self, _owner_agent_id: &str) -> MentalState {
        let slot_id = KbType::Kardia.slot_id();
        match self.get(slot_id, MENTAL_STATE_KEY) {
            Ok(Some(bytes)) => serde_json::from_slice(&bytes).unwrap_or_default(),
            _ => MentalState::default(),
        }
    }

    /// Writes the **MentalState** to **KB_KARDIA**. Used by JournalSkill and gateway.
    pub fn set_mental_state(&self, _owner_agent_id: &str, state: &MentalState) -> Result<(), sled::Error> {
        let slot_id = KbType::Kardia.slot_id();
        let bytes = serde_json::to_vec(state).unwrap_or_default();
        self.insert(slot_id, MENTAL_STATE_KEY, &bytes)?;
        Ok(())
    }

    /// Key in **KB_SOMA** (Slot 8) where the current BiometricState is stored (BioGate).
    pub const BIOMETRIC_STATE_KEY: &str = "biometric/current";

    /// Returns the **BiometricState** (Physical Load) from **KB_SOMA** (Slot 8).
    pub fn get_biometric_state(&self) -> BiometricState {
        let slot_id = KbType::Soma.slot_id();
        match self.get(slot_id, Self::BIOMETRIC_STATE_KEY) {
            Ok(Some(bytes)) => serde_json::from_slice(&bytes).unwrap_or_default(),
            _ => BiometricState::default(),
        }
    }

    /// Writes the **BiometricState** to **KB_SOMA** (Slot 8). Used by BioGateSync skill.
    pub fn set_biometric_state(&self, state: &BiometricState) -> Result<(), sled::Error> {
        let slot_id = KbType::Soma.slot_id();
        let bytes = serde_json::to_vec(state).unwrap_or_default();
        self.insert(slot_id, Self::BIOMETRIC_STATE_KEY, &bytes)?;
        Ok(())
    }

    /// Key in **KB_SOMA** (Slot 8) where the current SomaState is stored (BioGate v2).
    pub const SOMA_STATE_KEY: &str = "soma/current";

    /// Returns the **SomaState** (BioGate health metrics) from **KB_SOMA** (Slot 8).
    pub fn get_soma_state(&self) -> SomaState {
        let slot_id = KbType::Soma.slot_id();
        match self.get(slot_id, Self::SOMA_STATE_KEY) {
            Ok(Some(bytes)) => serde_json::from_slice(&bytes).unwrap_or_default(),
            _ => SomaState::default(),
        }
    }

    /// Writes the **SomaState** to **KB_SOMA** (Slot 8). Used by BioGateSync skill.
    pub fn set_soma_state(&self, state: &SomaState) -> Result<(), sled::Error> {
        let slot_id = KbType::Soma.slot_id();
        let bytes = serde_json::to_vec(state).unwrap_or_default();
        self.insert(slot_id, Self::SOMA_STATE_KEY, &bytes)?;
        Ok(())
    }

    /// Returns the **effective** MentalState for the Cognitive Governor: Kardia baseline
    /// merged with Soma (BioGate) physical load.
    ///
    /// **Cross-layer reaction (BioGate v2 — SomaState):**
    /// If `readiness_score < 50` **OR** `sleep_hours < 6.0`:
    /// - `burnout_risk` is incremented by **+0.15**
    /// - `grace_multiplier` is set to **1.6**
    ///
    /// **Legacy fallback (BiometricState):**
    /// If `sleep_score < 60`, burnout_risk is increased by 0.2 and grace_multiplier set to 1.5.
    pub fn get_effective_mental_state(&self, owner_agent_id: &str) -> MentalState {
        let mut mental = self.get_mental_state(owner_agent_id);

        // BioGate v2: SomaState cross-layer reaction (takes priority)
        let soma = self.get_soma_state();
        if soma.needs_biogate_adjustment() {
            mental.burnout_risk = (mental.burnout_risk + SomaState::BURNOUT_RISK_INCREMENT).min(1.0);
            mental.grace_multiplier = SomaState::GRACE_MULTIPLIER_OVERRIDE;
        } else {
            // Legacy fallback: BiometricState
            let bio = self.get_biometric_state();
            if bio.poor_sleep() {
                mental.burnout_risk = (mental.burnout_risk + 0.2).min(1.0);
                mental.grace_multiplier = 1.5;
            }
        }

        mental.clamp();
        mental
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

    // ─────────────────────────────────────────────────────────────────────────
    // Shadow Vault (Slot 9) — Encrypted Emotional Data
    // ─────────────────────────────────────────────────────────────────────────

    /// Stores an `EmotionalAnchor` in Slot 9 (Shadow), encrypted via AES-256-GCM.
    ///
    /// Key convention: `anchor/{anchor_type}` or `anchor/{label}`.
    /// Returns `Err` if the vault is locked.
    pub fn insert_shadow_anchor(
        &self,
        key: &str,
        anchor: &EmotionalAnchor,
    ) -> Result<(), sled::Error> {
        let bytes = anchor.to_bytes();
        self.insert(SHADOW_SLOT_ID, key, &bytes)?;
        Ok(())
    }

    /// Retrieves and decrypts an `EmotionalAnchor` from Slot 9 (Shadow).
    ///
    /// Returns `Ok(None)` if the key doesn't exist.
    /// Returns `Err` if the vault is locked or decryption fails.
    pub fn get_shadow_anchor(&self, key: &str) -> Result<Option<EmotionalAnchor>, String> {
        let encrypted = match self.get(SHADOW_SLOT_ID, key) {
            Ok(Some(data)) => data,
            Ok(None) => return Ok(None),
            Err(e) => return Err(format!("sled error: {}", e)),
        };
        match self.vault.decrypt_anchor(&encrypted) {
            Ok(anchor) => Ok(Some(anchor)),
            Err(VaultError::Locked) => Err("Shadow Vault is locked".to_string()),
            Err(e) => Err(format!("decrypt error: {}", e)),
        }
    }

    /// Retrieves and decrypts raw bytes from Slot 9 (Shadow) as a UTF-8 string.
    ///
    /// Returns `Ok(None)` if the key doesn't exist.
    /// Returns `Err` if the vault is locked or decryption fails.
    pub fn get_shadow_decrypted(&self, key: &str) -> Result<Option<String>, String> {
        let encrypted = match self.get(SHADOW_SLOT_ID, key) {
            Ok(Some(data)) => data,
            Ok(None) => return Ok(None),
            Err(e) => return Err(format!("sled error: {}", e)),
        };
        match self.vault.decrypt_str(&encrypted) {
            Ok(s) => Ok(Some(s)),
            Err(VaultError::Locked) => Err("Shadow Vault is locked".to_string()),
            Err(e) => Err(format!("decrypt error: {}", e)),
        }
    }

    /// Returns all active `EmotionalAnchor`s from Slot 9 (Shadow).
    ///
    /// Scans all keys with prefix `anchor/` and decrypts each one.
    /// Silently skips entries that fail to decrypt (e.g. corrupted).
    /// Returns an empty vec if the vault is locked.
    pub fn get_active_shadow_anchors(&self) -> Vec<(String, EmotionalAnchor)> {
        if !self.vault.is_unlocked() {
            return Vec::new();
        }
        let tree = match self.db.open_tree(Self::tree_name(SHADOW_SLOT_ID)) {
            Ok(t) => t,
            Err(_) => return Vec::new(),
        };
        let mut anchors = Vec::new();
        for item in tree.iter() {
            let (k, v) = match item {
                Ok(kv) => kv,
                Err(_) => continue,
            };
            let key = match String::from_utf8(k.to_vec()) {
                Ok(s) => s,
                Err(_) => continue,
            };
            if !key.starts_with("anchor/") {
                continue;
            }
            let encrypted = v.to_vec();
            if let Ok(anchor) = self.vault.decrypt_anchor(&encrypted) {
                if anchor.active {
                    anchors.push((key, anchor));
                }
            }
        }
        anchors
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Dynamic Task Governance (Oikos) — Slot 2 task management
    // ─────────────────────────────────────────────────────────────────────────

    /// Stores a [`GovernedTask`] in **KB_OIKOS** (Slot 2) under `oikos/tasks/{task_id}`.
    pub fn set_governed_task(&self, task: &crate::GovernedTask) -> Result<(), sled::Error> {
        let slot_id = KbType::Oikos.slot_id();
        let key = format!("{}{}", crate::OIKOS_TASK_PREFIX, task.task_id);
        self.insert(slot_id, &key, &task.to_bytes())?;
        Ok(())
    }

    /// Retrieves a [`GovernedTask`] from **KB_OIKOS** (Slot 2) by task_id.
    pub fn get_governed_task(&self, task_id: &str) -> Option<crate::GovernedTask> {
        let slot_id = KbType::Oikos.slot_id();
        let key = format!("{}{}", crate::OIKOS_TASK_PREFIX, task_id);
        self.get(slot_id, &key)
            .ok()
            .flatten()
            .and_then(|b| crate::GovernedTask::from_bytes(&b))
    }

    /// Returns all governed tasks from **KB_OIKOS** (Slot 2), sorted by effective priority descending.
    pub fn list_governed_tasks(&self) -> Result<Vec<crate::GovernedTask>, sled::Error> {
        let slot_id = KbType::Oikos.slot_id();
        let kv = self.scan_kv(slot_id)?;
        let prefix = crate::OIKOS_TASK_PREFIX;
        let mut tasks: Vec<crate::GovernedTask> = kv
            .into_iter()
            .filter(|(k, _)| k.starts_with(prefix))
            .filter_map(|(_, bytes)| crate::GovernedTask::from_bytes(&bytes))
            .collect();
        tasks.sort_by(|a, b| {
            b.effective_priority
                .partial_cmp(&a.effective_priority)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        Ok(tasks)
    }

    /// Removes a governed task from **KB_OIKOS** (Slot 2) by task_id.
    pub fn remove_governed_task(&self, task_id: &str) -> Result<bool, sled::Error> {
        let slot_id = KbType::Oikos.slot_id();
        let key = format!("{}{}", crate::OIKOS_TASK_PREFIX, task_id);
        let prev = self.remove(slot_id, &key)?;
        Ok(prev.is_some())
    }

    /// Creates a [`TaskGovernor`] from the current cross-layer state (Soma + Kardia + Ethos).
    ///
    /// This is the primary entry point for task governance: it reads the current biological,
    /// emotional, and philosophical state from the knowledge store and returns a governor
    /// that can evaluate tasks.
    pub fn create_task_governor(&self, agent_id: &str) -> crate::TaskGovernor {
        let soma = self.get_soma_state();
        let mental = self.get_effective_mental_state(agent_id);
        let ethos = self.get_ethos_philosophical_policy();
        crate::TaskGovernor::new(soma, mental, ethos)
    }

    /// Evaluates all governed tasks using the current cross-layer state and persists the results.
    ///
    /// Returns the evaluated tasks sorted by effective priority.
    pub fn evaluate_and_persist_tasks(&self, agent_id: &str) -> Result<Vec<crate::GovernedTask>, sled::Error> {
        let governor = self.create_task_governor(agent_id);
        let tasks = self.list_governed_tasks()?;
        let evaluated = governor.evaluate_batch(&tasks);

        // Persist each evaluated task back to Oikos
        for task in &evaluated {
            self.set_governed_task(task)?;
        }

        // Persist governance summary
        let summary = governor.governance_summary(&tasks);
        let slot_id = KbType::Oikos.slot_id();
        self.insert(slot_id, crate::OIKOS_GOVERNANCE_SUMMARY_KEY, summary.as_bytes())?;

        Ok(evaluated)
    }

    /// Returns the last persisted governance summary from **KB_OIKOS** (Slot 2), if present.
    pub fn get_governance_summary(&self) -> Option<String> {
        let slot_id = KbType::Oikos.slot_id();
        self.get(slot_id, crate::OIKOS_GOVERNANCE_SUMMARY_KEY)
            .ok()
            .flatten()
            .and_then(|b| String::from_utf8(b).ok())
    }

    /// **Compassionate Routing Helper:** Checks the Shadow_KB for active emotional anchors
    /// and returns an optional system instruction to inject into the LLM prompt.
    ///
    /// This is the bridge between the encrypted Shadow_KB and the Cognitive Governor.
    /// If any active anchor with `intensity > 0.5` is found, returns a supportive
    /// system instruction. Never logs the anchor content.
    pub fn check_mental_load(&self) -> Option<String> {
        let anchors = self.get_active_shadow_anchors();
        if anchors.is_empty() {
            return None;
        }

        // Find the highest-intensity active anchor
        let max_intensity = anchors
            .iter()
            .map(|(_, a)| a.intensity)
            .fold(0.0_f32, f32::max);

        if max_intensity > 0.5 {
            let anchor_types: Vec<&str> = anchors
                .iter()
                .filter(|(_, a)| a.intensity > 0.5)
                .map(|(_, a)| a.anchor_type.as_str())
                .collect();

            // Build a compassionate system instruction without revealing specifics
            let instruction = format!(
                "The user is currently handling heavy personal issues (detected anchors: {}). \
                 Be supportive and prioritize cognitive ease over performance metrics. \
                 Use a warm, patient tone. Avoid adding pressure or urgency.",
                anchor_types.join(", ")
            );
            tracing::debug!(
                target: "pagi::vault",
                anchor_count = anchors.len(),
                max_intensity = max_intensity,
                "Shadow_KB: compassionate routing activated (mental load detected)"
            );
            Some(instruction)
        } else {
            None
        }
    }

    /// Returns the full sovereign state for the dashboard and Live Status API.
    /// Aggregates Soma, Ethos, Kardia (mental + people), Oikos (summary + tasks), Shadow lock, and 9-slot status.
    pub fn get_full_sovereign_state(&self, agent_id: &str) -> SovereignState {
        let kb_statuses = self.get_all_status();
        let soma = self.get_soma_state();
        let bio_gate_active = soma.needs_biogate_adjustment();
        let ethos = self.get_ethos_philosophical_policy();
        let mental = self.get_effective_mental_state(agent_id);
        let people = self.list_people().unwrap_or_default();
        let governance_summary = self.get_governance_summary();
        let governed_tasks = self.list_governed_tasks().unwrap_or_default();
        let shadow_unlocked = self.is_shadow_unlocked();

        SovereignState {
            kb_statuses,
            soma,
            bio_gate_active,
            ethos,
            mental,
            people,
            governance_summary,
            governed_tasks,
            shadow_unlocked,
        }
    }
}

/// Full cross-layer state for the Sovereign Dashboard and Live Status API.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SovereignState {
    /// 9-slot knowledge matrix (connection, entry counts, errors).
    pub kb_statuses: Vec<KbStatus>,
    /// Soma (Slot 8): sleep, readiness, HR, HRV.
    pub soma: SomaState,
    /// True when BioGate adjustment is active (supportive tone, grace multiplier).
    pub bio_gate_active: bool,
    /// Ethos (Slot 6): philosophical lens, if set.
    pub ethos: Option<EthosPolicy>,
    /// Effective mental state (Kardia + Soma merge): stress, burnout, grace.
    pub mental: MentalState,
    /// Relational map (Kardia Slot 7): people with trust and attachment.
    pub people: Vec<PersonRecord>,
    /// Oikos (Slot 2): last governance summary text, if any.
    pub governance_summary: Option<String>,
    /// Oikos: governed tasks (evaluated by TaskGovernor).
    pub governed_tasks: Vec<GovernedTask>,
    /// Shadow (Slot 9): true when vault is unlocked (PAGI_SHADOW_KEY set).
    pub shadow_unlocked: bool,
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
