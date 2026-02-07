//! Shared types used across all UAC crates.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

// -----------------------------------------------------------------------------
// Emotional Context Layer — Cognitive Governor state (Kardia/Soma as state-modifiers for Logos)
// -----------------------------------------------------------------------------

/// Mental state used to modulate agent tone and demand level (Contextual Grace).
/// Stored in KB_KARDIA under key `MENTAL_STATE_KEY`; gateway and JournalSkill read/write.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MentalState {
    /// Relational / emotional load (0.0 = calm, 1.0 = high stress). Drives tone shift to empathetic when > 0.7.
    #[serde(default)]
    pub relational_stress: f32,
    /// Burnout risk indicator (0.0–1.0). Used for grace multiplier and future NVC/suggestions.
    #[serde(default)]
    pub burnout_risk: f32,
    /// Multiplier applied to agent deadlines and demand level (e.g. < 1.0 = reduce pressure).
    #[serde(default = "default_grace_multiplier")]
    pub grace_multiplier: f32,
}

fn default_grace_multiplier() -> f32 {
    1.0
}

impl Default for MentalState {
    fn default() -> Self {
        Self {
            relational_stress: 0.0,
            burnout_risk: 0.0,
            grace_multiplier: 1.0,
        }
    }
}

impl MentalState {
    /// Clamps all fields to valid ranges.
    pub fn clamp(&mut self) {
        self.relational_stress = self.relational_stress.clamp(0.0, 1.0);
        self.burnout_risk = self.burnout_risk.clamp(0.0, 1.0);
        self.grace_multiplier = self.grace_multiplier.clamp(0.2, 2.0);
    }

    /// True when the system should adopt a supportive, low-pressure tone (e.g. append empathetic system instruction).
    #[inline]
    pub fn needs_empathetic_tone(&self) -> bool {
        self.relational_stress > 0.7
    }

    /// Hidden system instruction appended when `needs_empathetic_tone()` is true. Never logged as raw emotional data.
    pub const EMPATHETIC_SYSTEM_INSTRUCTION: &'static str = "User is currently under high emotional load. Adopt a supportive, low-pressure, and highly empathetic tone. Prioritize brevity and reassurance.";

    /// When true, Cognitive Governor has applied Soma (physical load) adjustment — use supportive tone.
    #[inline]
    pub fn has_physical_load_adjustment(&self) -> bool {
        self.grace_multiplier >= 1.5
    }

    /// System instruction when physical load (e.g. poor sleep) is applied. Never logged as raw biometric data.
    pub const PHYSICAL_LOAD_SYSTEM_INSTRUCTION: &'static str = "User's physical load is elevated (e.g. reduced sleep). Be supportive, less demanding, and gentle. Prioritize clarity and avoid overwhelming requests.";
}

/// Key in KB_KARDIA (slot 7) where the serialized MentalState is stored.
pub const MENTAL_STATE_KEY: &str = "mental_state";

// -----------------------------------------------------------------------------
// Relational Map (Kardia) — Person records in Slot 7
// -----------------------------------------------------------------------------

/// Prefix in **KB_KARDIA** (Slot 7) for person records. Full key: `people/{name_slug}`.
pub const KARDIA_PEOPLE_PREFIX: &str = "people/";

/// One person in the Relational Map (Kardia). Tracks relationship, trust, attachment style, and triggers
/// so the AGI can give context-aware advice when the user mentions them (e.g. in journal or conflict).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PersonRecord {
    /// Display name (e.g. "Project Manager", "Sarah").
    pub name: String,
    /// Relationship role (e.g. "Boss", "Partner", "Mother", "Colleague").
    #[serde(default)]
    pub relationship: String,
    /// Trust level 0.0–1.0. Low values suggest caution in reframing or suggestions.
    #[serde(default)]
    pub trust_score: f32,
    /// Attachment style (e.g. "Avoidant", "Anxious", "Secure"). Informs tone of advice.
    #[serde(default)]
    pub attachment_style: String,
    /// Known triggers (e.g. "criticism", "silent treatment"). Injected into reflection context.
    #[serde(default)]
    pub triggers: Vec<String>,
    /// Optional summary of a recent interaction; updated by KardiaMap upsert.
    #[serde(default)]
    pub last_interaction_summary: Option<String>,
}

impl Default for PersonRecord {
    fn default() -> Self {
        Self {
            name: String::new(),
            relationship: String::new(),
            trust_score: 0.5,
            attachment_style: String::new(),
            triggers: Vec::new(),
            last_interaction_summary: None,
        }
    }
}

impl PersonRecord {
    /// Clamps trust_score to [0.0, 1.0].
    pub fn clamp(&mut self) {
        self.trust_score = self.trust_score.clamp(0.0, 1.0);
    }

    /// Slug for storage key: lowercase, non-alphanumeric replaced with underscore, collapsed.
    pub fn name_slug(name: &str) -> String {
        let s: String = name
            .chars()
            .map(|c| {
                if c.is_ascii_alphanumeric() {
                    c.to_lowercase().next().unwrap_or(c)
                } else if c.is_whitespace() {
                    '_'
                } else {
                    '_'
                }
            })
            .collect();
        let mut s = s.replace("__", "_");
        while s.contains("__") {
            s = s.replace("__", "_");
        }
        let s = s.trim_matches('_').to_string();
        if s.is_empty() {
            "unnamed".to_string()
        } else {
            s
        }
    }
}

// -----------------------------------------------------------------------------
// Soma (BioGate) — Physical / biometric state (Slot 8)
// -----------------------------------------------------------------------------

/// Biometric state stored in KB_SOMA (Slot 8). "Hardware monitoring" for the human;
/// used to modulate MentalState (e.g. low sleep → higher burnout_risk, gentler tone).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BiometricState {
    /// Sleep quality score (0–100). Values &lt; 60 trigger grace_multiplier and burnout_risk adjustment.
    #[serde(default)]
    pub sleep_score: f32,
    /// Heart rate variability (e.g. RMSSD in ms or normalized 0–1). Optional.
    #[serde(default)]
    pub heart_rate_variability: f32,
    /// Activity level (0–1 or 0–100 scale). Optional.
    #[serde(default)]
    pub activity_level: f32,
}

impl Default for BiometricState {
    fn default() -> Self {
        Self {
            sleep_score: 0.0,
            heart_rate_variability: 0.0,
            activity_level: 0.0,
        }
    }
}

impl BiometricState {
    /// Clamps fields to typical ranges.
    pub fn clamp(&mut self) {
        self.sleep_score = self.sleep_score.clamp(0.0, 100.0);
        self.heart_rate_variability = self.heart_rate_variability.clamp(0.0, 1.0);
        self.activity_level = self.activity_level.clamp(0.0, 100.0);
    }

    /// True when physical load should lower expectations (poor sleep).
    #[inline]
    pub fn poor_sleep(&self) -> bool {
        self.sleep_score > 0.0 && self.sleep_score < 60.0
    }
}

// -----------------------------------------------------------------------------
// SomaState — Structured health metrics for BioGate integration (Slot 8)
// -----------------------------------------------------------------------------

/// Structured health/biometric state for the BioGate (Soma) integration.
/// Stored in KB_SOMA (Slot 8) under key `soma/current`. Provides richer
/// health metrics than `BiometricState` and drives the cross-layer reaction:
///
/// - If `readiness_score < 50` **OR** `sleep_hours < 6.0`:
///   - `burnout_risk` is incremented by **+0.15**
///   - `grace_multiplier` is set to **1.6** (forcing supportive, less demanding tone)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SomaState {
    /// Hours of sleep in the last cycle (e.g. 4.5, 7.0, 8.5).
    #[serde(default)]
    pub sleep_hours: f32,
    /// Resting heart rate in BPM (e.g. 55, 72).
    #[serde(default)]
    pub resting_hr: u32,
    /// Heart rate variability in ms (e.g. RMSSD: 35, 60, 90).
    #[serde(default)]
    pub hrv: u32,
    /// Overall readiness score (0–100). Composite metric from wearable/health data.
    /// Values < 50 trigger the BioGate cross-layer reaction.
    #[serde(default)]
    pub readiness_score: u32,
}

impl Default for SomaState {
    fn default() -> Self {
        Self {
            sleep_hours: 0.0,
            resting_hr: 0,
            hrv: 0,
            readiness_score: 100,
        }
    }
}

impl SomaState {
    /// Clamps fields to physiologically reasonable ranges.
    pub fn clamp(&mut self) {
        self.sleep_hours = self.sleep_hours.clamp(0.0, 24.0);
        // resting_hr: 0 means "not set"; cap at 250 BPM
        self.resting_hr = self.resting_hr.min(250);
        // hrv: 0 means "not set"; cap at 500 ms
        self.hrv = self.hrv.min(500);
        self.readiness_score = self.readiness_score.min(100);
    }

    /// True when the BioGate should trigger a cross-layer reaction:
    /// `readiness_score < 50` **OR** `sleep_hours < 6.0` (and data has been set).
    #[inline]
    pub fn needs_biogate_adjustment(&self) -> bool {
        let has_data = self.sleep_hours > 0.0 || self.readiness_score < 100;
        has_data && (self.readiness_score < 50 || self.sleep_hours < 6.0)
    }

    /// The burnout_risk increment applied when `needs_biogate_adjustment()` is true.
    pub const BURNOUT_RISK_INCREMENT: f32 = 0.15;

    /// The grace_multiplier forced when `needs_biogate_adjustment()` is true.
    pub const GRACE_MULTIPLIER_OVERRIDE: f32 = 1.6;
}

// -----------------------------------------------------------------------------
// Ethos (Philosophical Lens) — Slot 1 / Slot 6 overlay
// -----------------------------------------------------------------------------

/// Key in **KB_ETHOS** (Slot 6) where the philosophical policy is stored.
pub const ETHOS_POLICY_KEY: &str = "ethos/current";

/// Philosophical personality engine for SAGE_BOT. Stored in KB_ETHOS (Slot 6)
/// under key [`ETHOS_POLICY_KEY`]. Determines the reframing lens used during
/// Shadow Reflections and general advisory tone.
///
/// ## Supported Schools
///
/// | `active_school`          | Reframing Approach                                                |
/// |--------------------------|-------------------------------------------------------------------|
/// | `Stoic`                  | Dichotomy of Control — focus on your reaction, not external events |
/// | `Growth-Mindset`         | Challenges as learning opportunities; neuroplasticity framing      |
/// | `Compassionate-Witness`  | Non-judgmental observation; self-compassion and NVC principles      |
/// | `Taoist`                 | Wu-wei — path of least resistance; acceptance of natural flow      |
/// | `Existentialist`         | Radical freedom — create your own meaning from the situation       |
///
/// Custom schools are allowed; the `core_maxims` provide the LLM with specific
/// principles to apply regardless of the school name.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct EthosPolicy {
    /// Active philosophical school (e.g. "Stoic", "Growth-Mindset", "Compassionate-Witness").
    pub active_school: String,
    /// Core maxims / principles the LLM should apply when reframing.
    /// E.g. for Stoic: ["Focus on what you can control", "Virtue is the sole good"].
    #[serde(default)]
    pub core_maxims: Vec<String>,
    /// Weight (0.0–1.0) controlling how strongly the philosophical lens influences tone.
    /// 1.0 = fully philosophical; 0.0 = generic supportive tone only.
    #[serde(default = "default_tone_weight")]
    pub tone_weight: f32,
}

fn default_tone_weight() -> f32 {
    0.8
}

impl Default for EthosPolicy {
    fn default() -> Self {
        Self {
            active_school: "Stoic".to_string(),
            core_maxims: vec![
                "Focus on what you can control (Dichotomy of Control).".to_string(),
                "Virtue is the sole good; external events are indifferent.".to_string(),
                "Respond with reason, not reactive emotion.".to_string(),
            ],
            tone_weight: 0.8,
        }
    }
}

impl EthosPolicy {
    /// Clamps tone_weight to [0.0, 1.0].
    pub fn clamp(&mut self) {
        self.tone_weight = self.tone_weight.clamp(0.0, 1.0);
    }

    /// Serializes to JSON bytes for storage in Ethos slot.
    pub fn to_bytes(&self) -> Vec<u8> {
        serde_json::to_vec(self).unwrap_or_default()
    }

    /// Deserializes from JSON bytes.
    pub fn from_bytes(bytes: &[u8]) -> Option<Self> {
        serde_json::from_slice(bytes).ok()
    }

    /// Returns the philosophical prompt (system instruction) for the LLM based on the active school and maxims.
    /// Alias for `to_system_instruction()`; used by ReflectShadow to inject school-specific reframing (e.g. Stoicism = Dichotomy of Control).
    #[inline]
    pub fn get_philosophical_prompt(&self) -> String {
        self.to_system_instruction()
    }

    /// Returns a system instruction string for the LLM based on the active school and maxims.
    /// Used by ReflectShadow and other skills to inject philosophical context.
    pub fn to_system_instruction(&self) -> String {
        if self.core_maxims.is_empty() {
            format!(
                "Use {} principles to reframe this situation. (tone_weight={:.1})",
                self.active_school, self.tone_weight
            )
        } else {
            let maxims_str = self
                .core_maxims
                .iter()
                .enumerate()
                .map(|(i, m)| format!("{}. {}", i + 1, m))
                .collect::<Vec<_>>()
                .join(" ");
            format!(
                "Use {} principles to reframe this situation. Core maxims: {} (tone_weight={:.1})",
                self.active_school, maxims_str, self.tone_weight
            )
        }
    }

    /// Returns a preset `EthosPolicy` for a well-known school name.
    /// Returns `None` if the school is not recognized (caller can still create a custom policy).
    pub fn preset(school: &str) -> Option<Self> {
        match school {
            "Stoic" => Some(Self::default()),
            "Growth-Mindset" => Some(Self {
                active_school: "Growth-Mindset".to_string(),
                core_maxims: vec![
                    "Challenges are opportunities for growth, not threats.".to_string(),
                    "Effort and learning matter more than innate talent.".to_string(),
                    "Setbacks provide data; reframe failure as feedback.".to_string(),
                ],
                tone_weight: 0.8,
            }),
            "Compassionate-Witness" => Some(Self {
                active_school: "Compassionate-Witness".to_string(),
                core_maxims: vec![
                    "Observe without judgment; feelings are valid data.".to_string(),
                    "Practice self-compassion before problem-solving.".to_string(),
                    "Use Non-Violent Communication: observe, feel, need, request.".to_string(),
                ],
                tone_weight: 0.8,
            }),
            "Taoist" => Some(Self {
                active_school: "Taoist".to_string(),
                core_maxims: vec![
                    "Wu-wei: act through non-forcing; find the path of least resistance.".to_string(),
                    "Accept the natural flow of events; resistance creates suffering.".to_string(),
                    "Balance yin and yang; every difficulty contains its opposite.".to_string(),
                ],
                tone_weight: 0.8,
            }),
            "Existentialist" => Some(Self {
                active_school: "Existentialist".to_string(),
                core_maxims: vec![
                    "You have radical freedom to choose your response and meaning.".to_string(),
                    "Authenticity: act in alignment with your true values, not external pressure.".to_string(),
                    "Embrace uncertainty; meaning is created, not discovered.".to_string(),
                ],
                tone_weight: 0.8,
            }),
            _ => None,
        }
    }
}

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

// -----------------------------------------------------------------------------
// Dynamic Task Governance (Oikos) — Slot 2 / Slot 4 overlay
// -----------------------------------------------------------------------------

/// Key prefix in **KB_OIKOS** (Slot 2) where governed tasks are stored.
/// Full key: `oikos/tasks/{task_id}`.
pub const OIKOS_TASK_PREFIX: &str = "oikos/tasks/";

/// Key in **KB_OIKOS** (Slot 2) where the governance summary is stored.
pub const OIKOS_GOVERNANCE_SUMMARY_KEY: &str = "oikos/governance_summary";

/// Cognitive difficulty tier for a task. Determines how much the task is affected
/// by biological state (Soma) and emotional load (Kardia).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskDifficulty {
    /// Low cognitive load (e.g. routine admin, filing, simple replies).
    Low,
    /// Medium cognitive load (e.g. code review, writing, planning).
    Medium,
    /// High cognitive load (e.g. architecture decisions, conflict resolution, deep research).
    High,
    /// Critical: must be done regardless of state (e.g. emergency, hard deadline).
    Critical,
}

impl Default for TaskDifficulty {
    fn default() -> Self {
        Self::Medium
    }
}

impl TaskDifficulty {
    /// Returns the cognitive load weight (0.0–1.0) for this difficulty tier.
    /// Higher values mean the task is more affected by poor biological state.
    pub fn cognitive_weight(&self) -> f32 {
        match self {
            Self::Low => 0.2,
            Self::Medium => 0.5,
            Self::High => 0.85,
            Self::Critical => 0.0, // Critical tasks are never postponed
        }
    }
}

/// The governance decision for a single task after cross-layer evaluation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GovernanceAction {
    /// Task should proceed as scheduled.
    Proceed,
    /// Task should be postponed (with a reason derived from Ethos/Soma context).
    Postpone { reason: String },
    /// Task should be simplified / broken into smaller steps.
    Simplify { suggestion: String },
    /// Task should be delegated or deprioritized.
    Deprioritize { reason: String },
}

impl GovernanceAction {
    /// Returns true if the task should proceed without modification.
    pub fn is_proceed(&self) -> bool {
        matches!(self, Self::Proceed)
    }

    /// Returns true if the task was postponed.
    pub fn is_postpone(&self) -> bool {
        matches!(self, Self::Postpone { .. })
    }
}

/// A task managed by the Dynamic Task Governor. Stored in KB_OIKOS (Slot 2)
/// under key `oikos/tasks/{task_id}`.
///
/// The Governor evaluates each task against the current Ethos (philosophical lens),
/// Soma (biological state), and Kardia (emotional/relational load) to produce a
/// [`GovernanceAction`] — proceed, postpone, simplify, or deprioritize.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GovernedTask {
    /// Unique task identifier (slug or UUID).
    pub task_id: String,
    /// Human-readable task title.
    pub title: String,
    /// Optional description or context for the task.
    #[serde(default)]
    pub description: String,
    /// Cognitive difficulty tier.
    #[serde(default)]
    pub difficulty: TaskDifficulty,
    /// Base priority (0.0 = lowest, 1.0 = highest). Before governance adjustment.
    #[serde(default = "default_priority")]
    pub base_priority: f32,
    /// Effective priority after governance adjustment (set by TaskGovernor).
    #[serde(default)]
    pub effective_priority: f32,
    /// The governance action determined by the TaskGovernor.
    #[serde(default = "default_governance_action")]
    pub action: GovernanceAction,
    /// Optional tags for categorization (e.g. "work", "personal", "health").
    #[serde(default)]
    pub tags: Vec<String>,
    /// Unix timestamp (ms) when this task was created.
    #[serde(default)]
    pub created_at_ms: i64,
    /// Unix timestamp (ms) when governance was last evaluated.
    #[serde(default)]
    pub last_evaluated_ms: i64,
}

fn default_priority() -> f32 {
    0.5
}

fn default_governance_action() -> GovernanceAction {
    GovernanceAction::Proceed
}

impl Default for GovernedTask {
    fn default() -> Self {
        Self {
            task_id: String::new(),
            title: String::new(),
            description: String::new(),
            difficulty: TaskDifficulty::Medium,
            base_priority: 0.5,
            effective_priority: 0.5,
            action: GovernanceAction::Proceed,
            tags: Vec::new(),
            created_at_ms: 0,
            last_evaluated_ms: 0,
        }
    }
}

impl GovernedTask {
    /// Creates a new task with the given ID, title, and difficulty.
    pub fn new(task_id: impl Into<String>, title: impl Into<String>, difficulty: TaskDifficulty) -> Self {
        let now_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as i64)
            .unwrap_or(0);
        Self {
            task_id: task_id.into(),
            title: title.into(),
            difficulty,
            base_priority: 0.5,
            effective_priority: 0.5,
            action: GovernanceAction::Proceed,
            created_at_ms: now_ms,
            ..Default::default()
        }
    }

    /// Sets the base priority (clamped to [0.0, 1.0]).
    pub fn with_priority(mut self, priority: f32) -> Self {
        self.base_priority = priority.clamp(0.0, 1.0);
        self.effective_priority = self.base_priority;
        self
    }

    /// Adds tags to the task.
    pub fn with_tags(mut self, tags: Vec<String>) -> Self {
        self.tags = tags;
        self
    }

    /// Sets the description.
    pub fn with_description(mut self, desc: impl Into<String>) -> Self {
        self.description = desc.into();
        self
    }

    /// Serializes to JSON bytes for storage.
    pub fn to_bytes(&self) -> Vec<u8> {
        serde_json::to_vec(self).unwrap_or_default()
    }

    /// Deserializes from JSON bytes.
    pub fn from_bytes(bytes: &[u8]) -> Option<Self> {
        serde_json::from_slice(bytes).ok()
    }
}

/// The Dynamic Task Governor: evaluates tasks against the current biological,
/// emotional, and philosophical state to produce governance decisions.
///
/// ## Cross-Layer Synthesis
///
/// The Governor reads from three layers:
/// - **Soma** (Slot 8): `SomaState` — sleep hours, readiness score, HRV
/// - **Kardia** (Slot 7): `MentalState` — relational stress, burnout risk
/// - **Ethos** (Slot 6): `EthosPolicy` — philosophical lens for reframing
///
/// It then computes an **effective priority** and a **governance action** for each task:
///
/// | Condition | Effect |
/// |-----------|--------|
/// | `sleep_hours < 6` | High-difficulty tasks → Postpone |
/// | `readiness_score < 50` | High-difficulty tasks → Postpone |
/// | `burnout_risk > 0.7` | Medium+ tasks → Simplify or Deprioritize |
/// | `relational_stress > 0.7` | Tasks tagged "conflict" → Postpone |
/// | Critical difficulty | Always Proceed (never postponed) |
///
/// The philosophical lens (Ethos) shapes the *reason* text in governance actions,
/// providing school-specific reframing (e.g. Stoic: "Focus on what you can control").
pub struct TaskGovernor {
    /// Current biological state snapshot.
    pub soma: SomaState,
    /// Current emotional/mental state snapshot (already merged with Soma via `get_effective_mental_state`).
    pub mental: MentalState,
    /// Current philosophical policy (if set).
    pub ethos: Option<EthosPolicy>,
}

impl TaskGovernor {
    /// Creates a new TaskGovernor from the current cross-layer state.
    pub fn new(soma: SomaState, mental: MentalState, ethos: Option<EthosPolicy>) -> Self {
        Self { soma, mental, ethos }
    }

    /// Computes the biological penalty factor (0.0 = no penalty, 1.0 = maximum penalty).
    /// Based on sleep deprivation and readiness score.
    pub fn bio_penalty(&self) -> f32 {
        let mut penalty = 0.0_f32;

        // Sleep deprivation penalty
        if self.soma.sleep_hours > 0.0 && self.soma.sleep_hours < 7.0 {
            // Linear ramp: 7h = 0 penalty, 4h = 0.6 penalty, 0h = 1.0 penalty
            penalty += ((7.0 - self.soma.sleep_hours) / 7.0).clamp(0.0, 1.0);
        }

        // Readiness score penalty (0–100 scale; < 50 is concerning)
        if self.soma.readiness_score < 100 && self.soma.readiness_score < 70 {
            // Linear ramp: 70 = 0 penalty, 30 = 0.57 penalty, 0 = 1.0 penalty
            penalty += ((70 - self.soma.readiness_score) as f32 / 70.0).clamp(0.0, 1.0);
        }

        // Burnout risk amplifier
        if self.mental.burnout_risk > 0.5 {
            penalty += (self.mental.burnout_risk - 0.5) * 0.5;
        }

        penalty.clamp(0.0, 1.0)
    }

    /// Computes the emotional penalty factor (0.0 = calm, 1.0 = maximum stress).
    pub fn emotional_penalty(&self) -> f32 {
        let mut penalty = 0.0_f32;

        if self.mental.relational_stress > 0.3 {
            penalty += (self.mental.relational_stress - 0.3) * 1.0;
        }

        if self.mental.burnout_risk > 0.3 {
            penalty += (self.mental.burnout_risk - 0.3) * 0.5;
        }

        penalty.clamp(0.0, 1.0)
    }

    /// Returns a philosophical reason string based on the active Ethos school.
    /// Falls back to a generic supportive message if no Ethos is set.
    fn philosophical_reason(&self, base_reason: &str) -> String {
        match &self.ethos {
            Some(policy) => {
                let school = &policy.active_school;
                let maxim = policy.core_maxims.first().map(|m| m.as_str()).unwrap_or("");
                let weight = policy.tone_weight;

                if weight < 0.3 {
                    // Low philosophical influence — generic reason
                    base_reason.to_string()
                } else {
                    match school.as_str() {
                        "Stoic" => format!(
                            "{} — Stoic guidance: Focus on what you can control right now. \
                             This task involves factors outside your current capacity; \
                             postponing is the rational choice.",
                            base_reason
                        ),
                        "Growth-Mindset" => format!(
                            "{} — Growth-Mindset: This isn't failure; it's strategic timing. \
                             Your brain needs recovery to learn effectively. \
                             Rescheduling maximizes your growth potential.",
                            base_reason
                        ),
                        "Compassionate-Witness" => format!(
                            "{} — Compassionate-Witness: Be gentle with yourself. \
                             Your body is asking for rest, and honoring that need \
                             is an act of self-compassion, not weakness.",
                            base_reason
                        ),
                        "Taoist" => format!(
                            "{} — Taoist wisdom: Wu-wei — forcing this task now goes against \
                             the natural flow. Wait for the right moment; \
                             the path of least resistance will reveal itself.",
                            base_reason
                        ),
                        "Existentialist" => format!(
                            "{} — Existentialist: You have the radical freedom to choose \
                             when to engage. Choosing rest now is an authentic act \
                             of self-determination, not avoidance.",
                            base_reason
                        ),
                        _ => format!(
                            "{} — {} principle: {}",
                            base_reason, school, maxim
                        ),
                    }
                }
            }
            None => base_reason.to_string(),
        }
    }

    /// Evaluates a single task and returns the governance action + adjusted priority.
    ///
    /// This is the core decision engine. It considers:
    /// 1. Task difficulty vs. biological penalty
    /// 2. Emotional load vs. task tags (e.g. "conflict")
    /// 3. Philosophical lens for reason text
    pub fn evaluate(&self, task: &GovernedTask) -> (GovernanceAction, f32) {
        let bio = self.bio_penalty();
        let emo = self.emotional_penalty();
        let cog_weight = task.difficulty.cognitive_weight();

        // Critical tasks always proceed
        if task.difficulty == TaskDifficulty::Critical {
            return (GovernanceAction::Proceed, task.base_priority);
        }

        // Combined load: how much the current state impacts this task
        let combined_load = (bio * cog_weight + emo * 0.3).clamp(0.0, 1.0);

        // Effective priority: base priority reduced by combined load
        let effective_priority = (task.base_priority * (1.0 - combined_load * 0.5)).clamp(0.0, 1.0);

        // Decision thresholds
        let should_postpone = combined_load > 0.65 && task.difficulty == TaskDifficulty::High;
        let should_simplify = combined_load > 0.5 && task.difficulty == TaskDifficulty::High;
        let should_deprioritize = combined_load > 0.6 && task.difficulty == TaskDifficulty::Medium;

        // Check for conflict-tagged tasks under high emotional stress
        let is_conflict_task = task.tags.iter().any(|t| {
            let lower = t.to_lowercase();
            lower.contains("conflict") || lower.contains("confrontation") || lower.contains("difficult_person")
        });
        let emotional_postpone = is_conflict_task && self.mental.relational_stress > 0.7;

        // Severe sleep deprivation: postpone anything High
        let severe_sleep_deprivation = self.soma.sleep_hours > 0.0 && self.soma.sleep_hours < 5.0;
        let sleep_postpone = severe_sleep_deprivation && task.difficulty == TaskDifficulty::High;

        if sleep_postpone || (should_postpone && bio > 0.5) {
            let reason = self.philosophical_reason(
                &format!(
                    "Sleep: {:.1}h, Readiness: {}. High-difficulty task '{}' postponed due to physical load.",
                    self.soma.sleep_hours, self.soma.readiness_score, task.title
                ),
            );
            (GovernanceAction::Postpone { reason }, effective_priority)
        } else if emotional_postpone {
            let reason = self.philosophical_reason(
                &format!(
                    "Relational stress: {:.2}. Conflict-related task '{}' postponed to protect emotional bandwidth.",
                    self.mental.relational_stress, task.title
                ),
            );
            (GovernanceAction::Postpone { reason }, effective_priority)
        } else if should_simplify {
            let suggestion = format!(
                "Break '{}' into smaller, less cognitively demanding steps. \
                 Current bio penalty: {:.2}, emotional load: {:.2}.",
                task.title, bio, emo
            );
            (GovernanceAction::Simplify { suggestion }, effective_priority)
        } else if should_deprioritize {
            let reason = self.philosophical_reason(
                &format!(
                    "Combined load ({:.2}) exceeds threshold for medium-difficulty task '{}'. \
                     Consider tackling easier tasks first.",
                    combined_load, task.title
                ),
            );
            (GovernanceAction::Deprioritize { reason }, effective_priority)
        } else {
            (GovernanceAction::Proceed, effective_priority)
        }
    }

    /// Evaluates a batch of tasks and returns them sorted by effective priority (highest first).
    /// Each task is updated with its governance action and effective priority.
    pub fn evaluate_batch(&self, tasks: &[GovernedTask]) -> Vec<GovernedTask> {
        let now_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as i64)
            .unwrap_or(0);

        let mut evaluated: Vec<GovernedTask> = tasks
            .iter()
            .map(|task| {
                let (action, effective_priority) = self.evaluate(task);
                let mut t = task.clone();
                t.action = action;
                t.effective_priority = effective_priority;
                t.last_evaluated_ms = now_ms;
                t
            })
            .collect();

        // Sort: Proceed tasks first, then by effective priority descending
        evaluated.sort_by(|a, b| {
            let a_proceed = a.action.is_proceed() as u8;
            let b_proceed = b.action.is_proceed() as u8;
            b_proceed
                .cmp(&a_proceed)
                .then(b.effective_priority.partial_cmp(&a.effective_priority).unwrap_or(std::cmp::Ordering::Equal))
        });

        evaluated
    }

    /// Generates a human-readable governance summary for the current state.
    pub fn governance_summary(&self, tasks: &[GovernedTask]) -> String {
        let evaluated = self.evaluate_batch(tasks);
        let proceed_count = evaluated.iter().filter(|t| t.action.is_proceed()).count();
        let postpone_count = evaluated.iter().filter(|t| t.action.is_postpone()).count();
        let other_count = evaluated.len() - proceed_count - postpone_count;

        let bio = self.bio_penalty();
        let emo = self.emotional_penalty();
        let school = self.ethos.as_ref().map(|e| e.active_school.as_str()).unwrap_or("None");

        format!(
            "=== Task Governance Summary ===\n\
             Philosophical Lens: {}\n\
             Bio Penalty: {:.2} | Emotional Penalty: {:.2}\n\
             Sleep: {:.1}h | Readiness: {} | Burnout Risk: {:.2}\n\
             ---\n\
             Tasks: {} total | {} proceed | {} postponed | {} other\n\
             ---\n{}",
            school,
            bio,
            emo,
            self.soma.sleep_hours,
            self.soma.readiness_score,
            self.mental.burnout_risk,
            evaluated.len(),
            proceed_count,
            postpone_count,
            other_count,
            evaluated
                .iter()
                .map(|t| {
                    let action_str = match &t.action {
                        GovernanceAction::Proceed => "✅ PROCEED".to_string(),
                        GovernanceAction::Postpone { reason } => format!("⏸️  POSTPONE: {}", reason),
                        GovernanceAction::Simplify { suggestion } => format!("🔧 SIMPLIFY: {}", suggestion),
                        GovernanceAction::Deprioritize { reason } => format!("⬇️  DEPRIORITIZE: {}", reason),
                    };
                    format!(
                        "  [{:.2}] {} ({:?}) — {}",
                        t.effective_priority, t.title, t.difficulty, action_str
                    )
                })
                .collect::<Vec<_>>()
                .join("\n")
        )
    }
}
