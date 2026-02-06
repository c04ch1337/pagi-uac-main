//! Cognitive Router (Thalamus): routes incoming information to the correct holistic KB.
//!
//! This module implements the Mapping Layer that classifies data into the Holistic Ontology:
//! Logos, Soma, Pneuma, Kardia, Chronos, Techne, Oikos, Ethos.

use pagi_core::KbType;
use serde::Deserialize;
use std::collections::HashMap;

use crate::model_router::ModelRouter;

/// Metadata for routing context (source, tags, hints).
#[derive(Debug, Clone, Default, Deserialize)]
pub struct RouteMetadata {
    /// Optional source label (e.g. "fs_workspace_analyzer", "user_input").
    #[serde(default)]
    pub source: Option<String>,
    /// Optional tags that may influence routing.
    #[serde(default)]
    pub tags: Option<Vec<String>>,
    /// Optional free-form hints for the classifier.
    #[serde(default)]
    pub hint: Option<String>,
    /// Extra key-value context.
    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}

fn build_classification_prompt(input: &str, context: &str) -> String {
    format!(
        r#"You are a cognitive router. Given a piece of information, choose exactly ONE knowledge base where it belongs. Reply with only that word, nothing else.

Knowledge bases (reply with exactly one word):
- Logos: pure distilled information, research, papers, findings, code snippets, "Internal Wikipedia"
- Soma: physical interface, execution, side effects on hardware, file writes, buffer/staging
- Pneuma: identity, mission, goals, "why", evolving playbook, vision
- Kardia: user preferences, "who", vibe, personal notes
- Chronos: conversation history, temporal thread, short/long-term memory
- Techne: skills registry, blueprints, how-to, specialized functions
- Oikos: workspace context, "where", system logs, crate layout, project structure
- Ethos: guardrails, security, audit, "should", constraints

Information to classify:
"{}"

Context: {}
Reply with exactly one word from the list above."#,
        input,
        context
    )
}

/// Routes text into one of the 8 holistic ontology domains using the LLM.
///
/// Convenience wrapper with no metadata; use [`route_information`] when you have source/tags/hints.
///
/// Examples:
/// - "System logs" -> Oikos (The World)
/// - "New project goal" -> Pneuma (Spirit/Vision)
/// - "Code snippet" -> Logos (Logic/Facts)
pub async fn route_to_ontology(
    router: &ModelRouter,
    input: &str,
) -> Result<KbType, Box<dyn std::error::Error + Send + Sync>> {
    route_information(router, input, &RouteMetadata::default()).await
}

/// Routes a piece of information to the appropriate holistic KB using the LLM (ModelRouter).
///
/// Examples:
/// - "System logs" -> Oikos
/// - "New project goal" -> Pneuma
/// - "Code snippet" -> Logos
pub async fn route_information(
    router: &ModelRouter,
    input: &str,
    metadata: &RouteMetadata,
) -> Result<KbType, Box<dyn std::error::Error + Send + Sync>> {
    let context = build_context(metadata);
    let input_trimmed = input.chars().take(2000).collect::<String>();
    let prompt = build_classification_prompt(&input_trimmed, &context);
    let raw = router.generate_text_raw(&prompt).await?;
    parse_kb_type_from_response(&raw)
}

fn build_context(metadata: &RouteMetadata) -> String {
    let mut parts = Vec::new();
    if let Some(ref s) = metadata.source {
        parts.push(format!("source={}", s));
    }
    if let Some(ref t) = metadata.tags {
        parts.push(format!("tags={}", t.join(",")));
    }
    if let Some(ref h) = metadata.hint {
        parts.push(format!("hint={}", h));
    }
    if parts.is_empty() {
        return "none".to_string();
    }
    parts.join("; ")
}

/// Parses LLM output to a single KbType. Case-insensitive; defaults to Logos if unparseable.
fn parse_kb_type_from_response(response: &str) -> Result<KbType, Box<dyn std::error::Error + Send + Sync>> {
    let word = response
        .split_whitespace()
        .next()
        .unwrap_or("")
        .trim_matches(|c: char| !c.is_alphabetic())
        .to_lowercase();
    let kb = match word.as_str() {
        "logos" => KbType::Logos,
        "soma" => KbType::Soma,
        "pneuma" => KbType::Pneuma,
        "kardia" => KbType::Kardia,
        "chronos" => KbType::Chronos,
        "techne" => KbType::Techne,
        "oikos" => KbType::Oikos,
        "ethos" => KbType::Ethos,
        _ => {
            tracing::debug!(
                target: "pagi::thalamus",
                response = %response,
                "Thalamus: unparseable response, defaulting to Logos"
            );
            KbType::Logos
        }
    };
    Ok(kb)
}

#[cfg(test)]
mod tests {
    use super::*;
    use pagi_core::KbType;

    #[tokio::test]
    async fn route_to_ontology_system_logs_routes_to_oikos() {
        let router = ModelRouter::new(); // Mock mode
        let kb = route_to_ontology(&router, "System logs from the gateway").await.unwrap();
        assert_eq!(kb, KbType::Oikos, "System logs -> Oikos (The World)");
    }

    #[tokio::test]
    async fn route_to_ontology_goal_routes_to_pneuma() {
        let router = ModelRouter::new();
        let kb = route_to_ontology(&router, "New project goal: achieve AGI by 2026").await.unwrap();
        assert_eq!(kb, KbType::Pneuma, "Goals/mission -> Pneuma (Spirit/Vision)");
    }

    #[tokio::test]
    async fn route_to_ontology_research_routes_to_logos() {
        let router = ModelRouter::new();
        let kb = route_to_ontology(&router, "Research finding: code snippet from paper").await.unwrap();
        assert_eq!(kb, KbType::Logos, "Research/code -> Logos (Logic/Facts)");
    }
}
