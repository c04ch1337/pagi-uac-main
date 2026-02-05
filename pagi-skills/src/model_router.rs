//! Model Router skill: sends contextual prompt to an LLM (mock or live API) and returns generated text.

use pagi_orchestrator::AgentSkill;
use pagi_shared::TenantContext;

const SKILL_NAME: &str = "ModelRouter";
const ENV_LLM_MODE: &str = "PAGI_LLM_MODE";
const ENV_LLM_API_URL: &str = "PAGI_LLM_API_URL";
const ENV_LLM_API_KEY: &str = "PAGI_LLM_API_KEY";

/// Mode for LLM invocation: mock (returns simulated generation) or live (calls external API when configured).
#[derive(Clone, Copy, Debug, Default)]
pub enum LlmMode {
    #[default]
    Mock,
    Live,
}

impl LlmMode {
    fn from_env() -> Self {
        match std::env::var(ENV_LLM_MODE).as_deref() {
            Ok("live") => LlmMode::Live,
            _ => LlmMode::Mock,
        }
    }
}

/// Routes a prompt string to a mock LLM or a live API (Gemini/OpenAI placeholder).
pub struct ModelRouter {
    mode: LlmMode,
}

impl ModelRouter {
    pub fn new() -> Self {
        Self {
            mode: LlmMode::from_env(),
        }
    }

    pub fn with_mode(mode: LlmMode) -> Self {
        Self { mode }
    }

    /// Mock LLM: returns a deterministic "generated" response based on the prompt.
    /// When the prompt contains "Call to action: ...", that CTA is echoed so tests can verify sales closure.
    fn mock_generate(&self, prompt: &str) -> String {
        let preview = prompt
            .chars()
            .take(80)
            .chain(if prompt.len() > 80 { "…" } else { "" }.chars())
            .collect::<String>();
        let base = format!(
            "[Generated – Mock LLM]\n\nBased on your context ({}), here is a personalized response:\n\nThank you for reaching out. We appreciate you getting in touch and will follow up with you shortly. We hope you're doing well in your neighborhood and look forward to connecting.",
            preview
        );
        let cta_suffix = prompt
            .split("Call to action:")
            .nth(1)
            .map(|s| s.lines().next().unwrap_or(s).trim())
            .filter(|s| !s.is_empty());
        match cta_suffix {
            Some(cta) => format!("{}\n\nWe'd love to help: {}.\n\nBest regards", base, cta),
            None => format!("{}\n\nBest regards", base),
        }
    }

    /// Live API placeholder: when PAGI_LLM_API_URL and key are set, would call Gemini/OpenAI; otherwise falls back to mock.
    async fn live_generate(&self, prompt: &str) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        let url = std::env::var(ENV_LLM_API_URL).ok();
        let _key = std::env::var(ENV_LLM_API_KEY).ok();
        match (url.as_deref(), _key.as_deref()) {
            (Some(_url), Some(_key)) => {
                // Placeholder: real implementation would build request and parse response.
                // For now return mock so we can wire the chain without requiring API keys.
                Ok(self.mock_generate(prompt))
            }
            _ => Ok(self.mock_generate(prompt)),
        }
    }
}

impl Default for ModelRouter {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl AgentSkill for ModelRouter {
    fn name(&self) -> &str {
        SKILL_NAME
    }

    async fn execute(
        &self,
        _ctx: &TenantContext,
        payload: Option<serde_json::Value>,
    ) -> Result<serde_json::Value, Box<dyn std::error::Error + Send + Sync>> {
        let prompt = payload
            .as_ref()
            .and_then(|p| p.get("prompt").or(p.get("draft")))
            .and_then(|v| v.as_str())
            .ok_or("ModelRouter requires payload: { prompt: string } (or draft)")?
            .to_string();

        let generated = match self.mode {
            LlmMode::Mock => self.mock_generate(&prompt),
            LlmMode::Live => self.live_generate(&prompt).await?,
        };

        Ok(serde_json::json!({
            "status": "ok",
            "skill": SKILL_NAME,
            "mode": format!("{:?}", self.mode).to_lowercase(),
            "generated": generated,
            "prompt_preview_len": prompt.len()
        }))
    }
}
