//! Model Router skill: sends contextual prompt to an LLM (mock or live API) and returns generated text.
//! Supports both non-streaming (JSON response) and streaming (SSE) modes.

use pagi_core::{AgentSkill, KnowledgeStore, TenantContext};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::mpsc;

const SKILL_NAME: &str = "ModelRouter";
const ENV_LLM_MODE: &str = "PAGI_LLM_MODE";
const ENV_LLM_API_URL: &str = "PAGI_LLM_API_URL";
const ENV_LLM_API_KEY: &str = "PAGI_LLM_API_KEY";
const ENV_LLM_MODEL: &str = "PAGI_LLM_MODEL";
const ENV_EMBEDDINGS_API_URL: &str = "PAGI_EMBEDDINGS_API_URL";
const ENV_EMBEDDINGS_MODEL: &str = "PAGI_EMBEDDINGS_MODEL";
const DEFAULT_API_URL: &str = "https://openrouter.ai/api/v1/chat/completions";
const DEFAULT_EMBEDDINGS_API_URL: &str = "https://openrouter.ai/api/v1/embeddings";
const DEFAULT_MODEL: &str = "anthropic/claude-3.5-sonnet";
const DEFAULT_EMBEDDINGS_MODEL: &str = "text-embedding-3-small";

/// Mode for LLM invocation: mock (returns simulated generation) or live (calls external API).
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

// OpenAI-compatible request/response structures
#[derive(Serialize)]
struct ChatRequest {
    model: String,
    messages: Vec<ChatMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_tokens: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    stream: Option<bool>,
}

/// Streaming chunk from OpenAI-compatible API (SSE data format)
#[derive(Deserialize, Debug)]
struct StreamChunk {
    choices: Vec<StreamChoice>,
}

#[derive(Deserialize, Debug)]
struct StreamChoice {
    delta: StreamDelta,
    #[serde(default)]
    finish_reason: Option<String>,
}

#[derive(Deserialize, Debug)]
struct StreamDelta {
    #[serde(default)]
    content: Option<String>,
}

#[derive(Serialize)]
struct ChatMessage {
    role: String,
    content: String,
}

// OpenAI-compatible embeddings request/response structures
#[derive(Serialize)]
struct EmbeddingsRequest {
    model: String,
    input: String,
}

#[derive(Deserialize, Debug)]
struct EmbeddingsResponse {
    data: Vec<EmbeddingsData>,
}

#[derive(Deserialize, Debug)]
struct EmbeddingsData {
    embedding: Vec<f32>,
}

#[derive(Deserialize)]
struct ChatResponse {
    choices: Vec<ChatChoice>,
    #[serde(default)]
    usage: Option<TokenUsage>,
}

#[derive(Deserialize)]
struct ChatChoice {
    message: ChatMessageResponse,
}

#[derive(Deserialize)]
struct ChatMessageResponse {
    content: String,
}

#[derive(Deserialize, Default)]
struct TokenUsage {
    #[serde(default)]
    prompt_tokens: u32,
    #[serde(default)]
    completion_tokens: u32,
    #[serde(default)]
    total_tokens: u32,
}

/// Routes a prompt string to a mock LLM or a live API (OpenRouter/OpenAI-compatible).
pub struct ModelRouter {
    mode: LlmMode,
    client: reqwest::Client,
    knowledge: Option<Arc<KnowledgeStore>>,
}

impl ModelRouter {
    pub fn new() -> Self {
        Self {
            mode: LlmMode::from_env(),
            client: reqwest::Client::new(),
            knowledge: None,
        }
    }

    /// Constructs a ModelRouter that can query KB-5 Skill Registry to enrich prompts.
    pub fn with_knowledge(store: Arc<KnowledgeStore>) -> Self {
        Self {
            mode: LlmMode::from_env(),
            client: reqwest::Client::new(),
            knowledge: Some(store),
        }
    }

    pub fn with_mode(mode: LlmMode) -> Self {
        Self {
            mode,
            client: reqwest::Client::new(),
            knowledge: None,
        }
    }

    fn build_system_prompt_from_skills(&self) -> String {
        let Some(store) = &self.knowledge else {
            return String::new();
        };
        let skills = store.get_skills();
        if skills.is_empty() {
            return String::new();
        }

        let mut out = String::new();
        out.push_str("\n\n## Available Skills\n");
        for s in skills {
            let schema = serde_json::to_string_pretty(&s.schema).unwrap_or_else(|_| "{}".to_string());
            out.push_str(&format!("- **{}**: {}\n  - schema: {}\n", s.slug, s.description, schema.replace('\n', " ")));
        }
        out
    }

    /// Mock LLM: returns a deterministic "generated" response based on the prompt.
    fn mock_generate(&self, prompt: &str) -> String {
        let prompt = format!("{}{}", prompt, self.build_system_prompt_from_skills());
        let preview = prompt
            .chars()
            .take(80)
            .chain(if prompt.len() > 80 { "…" } else { "" }.chars())
            .collect::<String>();
        let base = format!(
            "[Generated – Mock LLM]\n\nBased on your context ({}), here is a personalized response:\n\nThank you for reaching out. We appreciate you getting in touch and will follow up with you shortly.",
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

    /// Live API: calls OpenRouter/OpenAI-compatible endpoint.
    async fn live_generate(
        &self,
        prompt: &str,
        model_override: Option<&str>,
        temperature: Option<f32>,
        max_tokens: Option<u32>,
    ) -> Result<(String, Option<TokenUsage>), Box<dyn std::error::Error + Send + Sync>> {
        let prompt = format!("{}{}", prompt, self.build_system_prompt_from_skills());
        let url = std::env::var(ENV_LLM_API_URL).unwrap_or_else(|_| DEFAULT_API_URL.to_string());
        let key = std::env::var(ENV_LLM_API_KEY)?;
        let model = model_override
            .map(|s| s.to_string())
            .or_else(|| std::env::var(ENV_LLM_MODEL).ok())
            .unwrap_or_else(|| DEFAULT_MODEL.to_string());

        eprintln!("[ModelRouter] Dispatching to OpenRouter (model: {})...", model);

        let request_body = ChatRequest {
            model: model.clone(),
            messages: vec![ChatMessage {
                role: "user".to_string(),
                content: prompt.to_string(),
            }],
            temperature,
            max_tokens,
            stream: None, // Non-streaming mode
        };

        let response = self
            .client
            .post(&url)
            .header("Authorization", format!("Bearer {}", key))
            .header("HTTP-Referer", "https://pagi-orchestrator.local")
            .header("X-Title", "PAGI-Master-Orchestrator")
            .header("Content-Type", "application/json")
            .json(&request_body)
            .send()
            .await?;

        let status = response.status();
        
        if !status.is_success() {
            let error_text = response.text().await.unwrap_or_else(|_| "Unknown error".to_string());
            eprintln!("[ModelRouter] HTTP {} from OpenRouter: {}", status, error_text);
            return Err(format!("OpenRouter API error ({}): {}", status, error_text).into());
        }

        eprintln!("[ModelRouter] HTTP {} OK from OpenRouter", status);

        let chat_response: ChatResponse = response.json().await?;
        
        let generated = chat_response
            .choices
            .first()
            .map(|c| c.message.content.clone())
            .unwrap_or_else(|| "[No response from LLM]".to_string());

        if let Some(ref usage) = chat_response.usage {
            eprintln!(
                "[ModelRouter] Tokens used: {} prompt + {} completion = {} total",
                usage.prompt_tokens, usage.completion_tokens, usage.total_tokens
            );
        }

        Ok((generated, chat_response.usage))
    }

    /// **Reflection path:** generates a supportive reframing from the LLM.
    /// - Prompt is used as-is (no skills appendix). Content is never logged.
    /// - Used by ReflectShadowSkill for vault content; ensures volatile memory only.
    pub async fn generate_reflection(
        &self,
        prompt: &str,
    ) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        match self.mode {
            LlmMode::Mock => {
                // Ethos-aware mock: if a philosophical policy is set, reflect its school.
                if let Some(store) = &self.knowledge {
                    if let Some(phil) = store.get_ethos_philosophical_policy() {
                        let school = &phil.active_school;
                        return Ok(format!(
                            "Here is a gentle reframe using {} principles: What you're feeling makes sense. \
                             Focus on what is within your control — your own reaction and choices — \
                             rather than the other person's behavior. You have agency in how you hold this experience.",
                            school,
                        ));
                    }
                }
                // Fallback: generic supportive reframing without logging prompt content.
                Ok(
                    "Here is a gentle reframe: What you're feeling makes sense. \
                     Consider viewing this as a moment to pause and choose your response \
                     rather than react. You have agency in how you hold this experience."
                        .to_string(),
                )
            }
            LlmMode::Live => {
                let url = std::env::var(ENV_LLM_API_URL).unwrap_or_else(|_| DEFAULT_API_URL.to_string());
                let key = std::env::var(ENV_LLM_API_KEY)?;
                let model = std::env::var(ENV_LLM_MODEL).unwrap_or_else(|_| DEFAULT_MODEL.to_string());
                tracing::debug!(
                    target: "pagi::model_router",
                    len = prompt.len(),
                    "[ModelRouter] Reflection request (prompt length only; content not logged)"
                );
                let request_body = ChatRequest {
                    model: model.clone(),
                    messages: vec![ChatMessage {
                        role: "user".to_string(),
                        content: prompt.to_string(),
                    }],
                    temperature: Some(0.5),
                    max_tokens: Some(1024),
                    stream: None,
                };
                let response = self
                    .client
                    .post(&url)
                    .header("Authorization", format!("Bearer {}", key))
                    .header("HTTP-Referer", "https://pagi-orchestrator.local")
                    .header("X-Title", "PAGI-Reflection")
                    .header("Content-Type", "application/json")
                    .json(&request_body)
                    .send()
                    .await?;
                if !response.status().is_success() {
                    let err = response.text().await.unwrap_or_else(|_| "Unknown error".to_string());
                    return Err(format!("Reflection LLM error: {}", err).into());
                }
                let chat_response: ChatResponse = response.json().await?;
                let text = chat_response
                    .choices
                    .first()
                    .map(|c| c.message.content.trim().to_string())
                    .unwrap_or_else(|| String::new());
                Ok(text)
            }
        }
    }

    /// Generates text from the LLM using the given prompt as-is (no skills appendix).
    /// Used by the Thalamus/cognitive router for classification tasks.
    pub async fn generate_text_raw(
        &self,
        prompt: &str,
    ) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        match self.mode {
            LlmMode::Mock => {
                // Deterministic mock for classification: match on the user input only (between quotes after "Information to classify:").
                let s = prompt.to_lowercase();
                let content = s
                    .find("information to classify:")
                    .and_then(|p| s[p..].find('"').map(|q| p + q + 1))
                    .and_then(|start| s[start..].find('"').map(|end| s[start..start + end].to_string()))
                    .unwrap_or_else(|| s.clone());
                let mock = if content.contains("system log") || (content.contains("workspace") && content.contains("scan")) || content.contains("crate") {
                    "Oikos"
                } else if content.contains("goal") || content.contains("mission") || content.contains("identity") {
                    "Pneuma"
                } else if content.contains("code") || content.contains("snippet") || content.contains("research") || content.contains("finding") {
                    "Logos"
                } else if content.contains("preference") || content.contains("vibe") {
                    "Kardia"
                } else if content.contains("conversation") || content.contains("history") || content.contains("session") {
                    "Chronos"
                } else if content.contains("skill") || content.contains("function") || content.contains("blueprint") {
                    "Techne"
                } else if content.contains("guardrail") || content.contains("security") || content.contains("audit") {
                    "Ethos"
                } else if content.contains("execution") || content.contains("side effect") || content.contains("file write") {
                    "Soma"
                } else {
                    "Logos"
                };
                Ok(mock.to_string())
            }
            LlmMode::Live => {
                let url = std::env::var(ENV_LLM_API_URL).unwrap_or_else(|_| DEFAULT_API_URL.to_string());
                let key = std::env::var(ENV_LLM_API_KEY)?;
                let model = std::env::var(ENV_LLM_MODEL).unwrap_or_else(|_| DEFAULT_MODEL.to_string());
                let request_body = ChatRequest {
                    model: model.clone(),
                    messages: vec![ChatMessage {
                        role: "user".to_string(),
                        content: prompt.to_string(),
                    }],
                    temperature: Some(0.0),
                    max_tokens: Some(32),
                    stream: None,
                };
                let response = self
                    .client
                    .post(&url)
                    .header("Authorization", format!("Bearer {}", key))
                    .header("HTTP-Referer", "https://pagi-orchestrator.local")
                    .header("X-Title", "PAGI-Thalamus")
                    .header("Content-Type", "application/json")
                    .json(&request_body)
                    .send()
                    .await?;
                if !response.status().is_success() {
                    let err = response.text().await.unwrap_or_else(|_| "Unknown error".to_string());
                    return Err(format!("LLM API error: {}", err).into());
                }
                let chat_response: ChatResponse = response.json().await?;
                let text = chat_response
                    .choices
                    .first()
                    .map(|c| c.message.content.trim().to_string())
                    .unwrap_or_else(|| "Logos".to_string());
                Ok(text)
            }
        }
    }

    /// Live API with streaming: streams tokens via a channel.
    /// Returns a receiver that yields string chunks as they arrive.
    pub async fn stream_generate(
        &self,
        prompt: &str,
        model_override: Option<&str>,
        temperature: Option<f32>,
        max_tokens: Option<u32>,
    ) -> Result<mpsc::Receiver<String>, Box<dyn std::error::Error + Send + Sync>> {
        let prompt = format!("{}{}", prompt, self.build_system_prompt_from_skills());
        let url = std::env::var(ENV_LLM_API_URL).unwrap_or_else(|_| DEFAULT_API_URL.to_string());
        let key = std::env::var(ENV_LLM_API_KEY)?;
        let model = model_override
            .map(|s| s.to_string())
            .or_else(|| std::env::var(ENV_LLM_MODEL).ok())
            .unwrap_or_else(|| DEFAULT_MODEL.to_string());

        tracing::info!(
            target: "pagi::model_router",
            model = %model,
            "[ModelRouter] Streaming session started for model: {}",
            model
        );

        let request_body = ChatRequest {
            model: model.clone(),
            messages: vec![ChatMessage {
                role: "user".to_string(),
                content: prompt.to_string(),
            }],
            temperature,
            max_tokens,
            stream: Some(true),
        };

        let response = self
            .client
            .post(&url)
            .header("Authorization", format!("Bearer {}", key))
            .header("HTTP-Referer", "https://pagi-orchestrator.local")
            .header("X-Title", "PAGI-Master-Orchestrator")
            .header("Content-Type", "application/json")
            .json(&request_body)
            .send()
            .await?;

        let status = response.status();
        if !status.is_success() {
            let error_text = response.text().await.unwrap_or_else(|_| "Unknown error".to_string());
            tracing::error!(
                target: "pagi::model_router",
                status = %status,
                "[ModelRouter] HTTP {} from OpenRouter: {}",
                status,
                error_text
            );
            return Err(format!("OpenRouter API error ({}): {}", status, error_text).into());
        }

        tracing::info!(
            target: "pagi::model_router",
            "[ModelRouter] HTTP {} OK - SSE stream established",
            status
        );

        // Create a channel to send tokens to the caller
        let (tx, rx) = mpsc::channel::<String>(100);

        // Spawn a task to read the stream and send tokens
        let model_for_log = model.clone();
        tokio::spawn(async move {
            use futures_util::TryStreamExt;
            let mut stream = response.bytes_stream();
            let mut buffer = String::new();

            while let Ok(Some(bytes)) = stream.try_next().await {
                let text = String::from_utf8_lossy(&bytes);
                buffer.push_str(&text);

                // Process complete SSE lines
                while let Some(newline_pos) = buffer.find('\n') {
                    let line = buffer[..newline_pos].trim().to_string();
                    buffer = buffer[newline_pos + 1..].to_string();

                    if line.is_empty() {
                        continue;
                    }

                    // SSE format: "data: {...}" or "data: [DONE]"
                    if let Some(data) = line.strip_prefix("data: ") {
                        if data == "[DONE]" {
                            tracing::info!(
                                target: "pagi::model_router",
                                "[ModelRouter] Stream completed for model: {}",
                                model_for_log
                            );
                            return;
                        }

                        // Parse the JSON chunk
                        match serde_json::from_str::<StreamChunk>(data) {
                            Ok(chunk) => {
                                if let Some(choice) = chunk.choices.first() {
                                    if let Some(content) = &choice.delta.content {
                                        if !content.is_empty() {
                                            if tx.send(content.clone()).await.is_err() {
                                                // Receiver dropped, stop processing
                                                return;
                                            }
                                        }
                                    }
                                }
                            }
                            Err(e) => {
                                tracing::debug!(
                                    target: "pagi::model_router",
                                    "[ModelRouter] Failed to parse SSE chunk: {} - data: {}",
                                    e,
                                    data
                                );
                            }
                        }
                    }
                }
            }
        });

        Ok(rx)
    }

    /// Mock streaming: yields words with delays to simulate streaming.
    pub fn mock_stream_generate(
        &self,
        prompt: &str,
    ) -> mpsc::Receiver<String> {
        let (tx, rx) = mpsc::channel::<String>(100);
        let mock_response = self.mock_generate(prompt);

        tokio::spawn(async move {
            // Split into words and stream with small delays
            for word in mock_response.split_inclusive(' ') {
                if tx.send(word.to_string()).await.is_err() {
                    break;
                }
                // Simulate streaming delay (50-100ms per word)
                tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
            }
        });

        rx
    }

    /// Live embeddings: calls OpenRouter/OpenAI-compatible embeddings endpoint.
    ///
    /// Env:
    /// - `PAGI_EMBEDDINGS_API_URL` (default: OpenRouter `/v1/embeddings`)
    /// - `PAGI_EMBEDDINGS_MODEL` (default: `text-embedding-3-small`)
    /// - `PAGI_LLM_API_KEY` (shared)
    pub async fn live_embedding(
        &self,
        input: &str,
        model_override: Option<&str>,
    ) -> Result<Vec<f32>, Box<dyn std::error::Error + Send + Sync>> {
        let url = std::env::var(ENV_EMBEDDINGS_API_URL)
            .unwrap_or_else(|_| DEFAULT_EMBEDDINGS_API_URL.to_string());
        let key = std::env::var(ENV_LLM_API_KEY)?;
        let model = model_override
            .map(|s| s.to_string())
            .or_else(|| std::env::var(ENV_EMBEDDINGS_MODEL).ok())
            .unwrap_or_else(|| DEFAULT_EMBEDDINGS_MODEL.to_string());

        tracing::info!(
            target: "pagi::model_router",
            model = %model,
            "[ModelRouter] Fetching embedding"
        );

        let request_body = EmbeddingsRequest {
            model,
            input: input.to_string(),
        };

        let response = self
            .client
            .post(&url)
            .header("Authorization", format!("Bearer {}", key))
            .header("HTTP-Referer", "https://pagi-orchestrator.local")
            .header("X-Title", "PAGI-Master-Orchestrator")
            .header("Content-Type", "application/json")
            .json(&request_body)
            .send()
            .await?;

        let status = response.status();
        if !status.is_success() {
            let error_text = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
            tracing::error!(
                target: "pagi::model_router",
                status = %status,
                "[ModelRouter] HTTP {} from embeddings endpoint: {}",
                status,
                error_text
            );
            return Err(format!("Embeddings API error ({}): {}", status, error_text).into());
        }

        let embeddings_response: EmbeddingsResponse = response.json().await?;
        let embedding = embeddings_response
            .data
            .into_iter()
            .next()
            .map(|d| d.embedding)
            .ok_or("Embeddings response missing data[0].embedding")?;

        Ok(embedding)
    }

    fn mock_embedding(input: &str) -> Vec<f32> {
        // Deterministic low-cost embedding for offline/mock mode.
        // Not semantically strong, but enables exercising the end-to-end pipeline.
        const DIMS: usize = 64;
        let mut v = vec![0f32; DIMS];
        for (i, b) in input.as_bytes().iter().enumerate() {
            let idx = (i.wrapping_mul(31) ^ (*b as usize)) % DIMS;
            v[idx] += (*b as f32) / 255.0;
        }
        // L2-normalize
        let norm = v.iter().map(|x| x * x).sum::<f32>().sqrt();
        if norm > 0.0 {
            for x in &mut v {
                *x /= norm;
            }
        }
        v
    }

    /// Embedding helper that respects the configured mode.
    ///
    /// - `Mock` => deterministic local embedding
    /// - `Live` => calls the configured embeddings endpoint
    pub async fn embedding(
        &self,
        input: &str,
        model_override: Option<&str>,
    ) -> Result<Vec<f32>, Box<dyn std::error::Error + Send + Sync>> {
        match self.mode {
            LlmMode::Mock => Ok(Self::mock_embedding(input)),
            LlmMode::Live => self.live_embedding(input, model_override).await,
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

        // Extract optional parameters from payload
        let model_override = payload
            .as_ref()
            .and_then(|p| p.get("model"))
            .and_then(|v| v.as_str());
        let temperature = payload
            .as_ref()
            .and_then(|p| p.get("temperature"))
            .and_then(|v| v.as_f64())
            .map(|t| t as f32);
        let max_tokens = payload
            .as_ref()
            .and_then(|p| p.get("max_tokens"))
            .and_then(|v| v.as_u64())
            .map(|t| t as u32);

        let (generated, usage) = match self.mode {
            LlmMode::Mock => (self.mock_generate(&prompt), None),
            LlmMode::Live => {
                match self.live_generate(&prompt, model_override, temperature, max_tokens).await {
                    Ok((text, usage)) => (text, usage),
                    Err(e) => {
                        eprintln!("[ModelRouter] Live generation failed: {}. Falling back to mock.", e);
                        (format!("[Live LLM Error: {}]\n\n{}", e, self.mock_generate(&prompt)), None)
                    }
                }
            }
        };

        let mut result = serde_json::json!({
            "status": "ok",
            "skill": SKILL_NAME,
            "mode": format!("{:?}", self.mode).to_lowercase(),
            "generated": generated,
            "prompt_preview_len": prompt.len()
        });

        // Add token usage if available
        if let Some(usage) = usage {
            result["token_usage"] = serde_json::json!({
                "prompt_tokens": usage.prompt_tokens,
                "completion_tokens": usage.completion_tokens,
                "total_tokens": usage.total_tokens
            });
        }

        Ok(result)
    }
}
