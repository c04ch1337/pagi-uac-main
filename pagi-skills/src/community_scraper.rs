//! Community Scraper skill: fetches a URL (or uses provided HTML), extracts headlines/events, and updates KB-5 (Community Pulse).

use pagi_knowledge::KnowledgeStore;
use pagi_orchestrator::AgentSkill;
use pagi_shared::TenantContext;
use scraper::{Html, Selector};
use std::sync::Arc;

const SKILL_NAME: &str = "CommunityScraper";
const KB_SLOT_COMMUNITY: u8 = 5;
const CURRENT_PULSE_KEY: &str = "current_pulse";
const DEFAULT_LOCATION: &str = "Stockdale";
const DEFAULT_TREND: &str = "Scraped";

/// Fetches a page (or uses provided HTML), extracts headings/article text, and writes to KB-5.
pub struct CommunityScraper {
    knowledge: Arc<KnowledgeStore>,
}

impl CommunityScraper {
    pub fn new(knowledge: Arc<KnowledgeStore>) -> Self {
        Self { knowledge }
    }
}

/// Extract text content from HTML using common news/article selectors.
fn extract_headlines_and_events(html: &str) -> String {
    let document = Html::parse_document(html);
    let mut parts: Vec<String> = Vec::new();

    let selectors = [
        "h1",
        "h2",
        "h3",
        "article h2",
        "article h3",
        ".headline",
        ".title",
        "[class*='headline']",
        "[class*='title']",
    ];

    for selector_str in selectors {
        if let Ok(sel) = Selector::parse(selector_str) {
            for el in document.select(&sel) {
                let text = el.text().collect::<Vec<_>>().join(" ").trim().to_string();
                if !text.is_empty() && !parts.contains(&text) {
                    parts.push(text);
                }
            }
        }
    }

    if parts.is_empty() {
        "(no events extracted)".to_string()
    } else {
        parts.join(". ")
    }
}

#[async_trait::async_trait]
impl AgentSkill for CommunityScraper {
    fn name(&self) -> &str {
        SKILL_NAME
    }

    async fn execute(
        &self,
        _ctx: &TenantContext,
        payload: Option<serde_json::Value>,
    ) -> Result<serde_json::Value, Box<dyn std::error::Error + Send + Sync>> {
        let payload = payload.ok_or("CommunityScraper requires payload: { url: string } or { slot_id?: 1..8, url?, html? }")?;
        let slot_id = payload
            .get("slot_id")
            .and_then(|v| v.as_u64())
            .map(|n| n as u8)
            .unwrap_or(KB_SLOT_COMMUNITY);
        if !(1..=8).contains(&slot_id) {
            return Err("slot_id must be 1–8".into());
        }
        let url = payload
            .get("url")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
        let html_override = payload.get("html").and_then(|v| v.as_str()).map(|s| s.to_string());
        let location = payload
            .get("location")
            .and_then(|v| v.as_str())
            .unwrap_or(DEFAULT_LOCATION)
            .to_string();

        let html = if let Some(html) = html_override {
            html
        } else {
            let url = url.ok_or("CommunityScraper requires 'url' when 'html' is not provided")?;
            let client = reqwest::Client::builder()
                .user_agent("UAC-CommunityScraper/1.0")
                .build()?;
            let resp = client.get(&url).send().await?;
            resp.text().await?
        };

        let event = extract_headlines_and_events(&html);
        let pulse = serde_json::json!({
            "location": location,
            "trend": DEFAULT_TREND,
            "event": event
        });
        let value = pulse.to_string();
        self.knowledge
            .insert(slot_id, CURRENT_PULSE_KEY, value.as_bytes())?;

        Ok(serde_json::json!({
            "status": "ok",
            "skill": SKILL_NAME,
            "slot_id": slot_id,
            "key": CURRENT_PULSE_KEY,
            "location": location,
            "trend": DEFAULT_TREND,
            "event": event
        }))
    }
}
