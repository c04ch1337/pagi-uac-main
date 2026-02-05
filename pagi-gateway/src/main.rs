//! Axum-based API Gateway: entry point for UAC.

use axum::{
    extract::{Path, State},
    extract::Json,
    routing::{get, post},
    Router,
};
use pagi_knowledge::KnowledgeStore;
use pagi_orchestrator::{BlueprintRegistry, Orchestrator, SkillRegistry};
use pagi_shared::{Goal, TenantContext};
use pagi_skills::{
    CommunityPulse, CommunityScraper, DraftResponse, KnowledgeInsert, KnowledgeQuery, LeadCapture,
    ModelRouter, ResearchAudit, SalesCloser,
};
use std::sync::Arc;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[tokio::main]
async fn main() {
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::new(
            std::env::var("RUST_LOG").unwrap_or_else(|_| "info".into()),
        ))
        .with(tracing_subscriber::fmt::layer())
        .init();

    let memory = Arc::new(
        pagi_memory::MemoryManager::new().expect("open pagi_vault"),
    );
    let knowledge = Arc::new(
        pagi_knowledge::KnowledgeStore::new().expect("open pagi_knowledge"),
    );
    let mut registry = SkillRegistry::new();
    registry.register(Arc::new(LeadCapture::new(Arc::clone(&memory))));
    registry.register(Arc::new(KnowledgeQuery::new(Arc::clone(&knowledge))));
    registry.register(Arc::new(KnowledgeInsert::new(Arc::clone(&knowledge))));
    registry.register(Arc::new(CommunityPulse::new(Arc::clone(&knowledge))));
    registry.register(Arc::new(DraftResponse::new(
        Arc::clone(&memory),
        Arc::clone(&knowledge),
    )));
    registry.register(Arc::new(ModelRouter::new()));
    registry.register(Arc::new(ResearchAudit::new(Arc::clone(&knowledge))));
    registry.register(Arc::new(CommunityScraper::new(Arc::clone(&knowledge))));
    registry.register(Arc::new(SalesCloser::new(Arc::clone(&knowledge))));
    let blueprint_path = std::env::var("PAGI_BLUEPRINT_PATH")
        .unwrap_or_else(|_| "config/blueprint.json".to_string());
    let blueprint = Arc::new(BlueprintRegistry::load_json_path(&blueprint_path));
    let orchestrator = Arc::new(Orchestrator::with_blueprint(
        Arc::new(registry),
        Arc::clone(&blueprint),
    ));

    let app = Router::new()
        .route("/v1/execute", post(execute))
        .route("/v1/research/trace/:trace_id", get(get_research_trace))
        .with_state(AppState {
            orchestrator,
            knowledge,
        });

    let addr = std::net::SocketAddr::from(([0, 0, 0, 0], 3000));
    tracing::info!("pagi-gateway listening on {}", addr);
    axum::serve(
        tokio::net::TcpListener::bind(addr).await.unwrap(),
        app,
    )
    .await
    .unwrap();
}

#[derive(Clone)]
pub(crate) struct AppState {
    pub(crate) orchestrator: Arc<Orchestrator>,
    pub(crate) knowledge: Arc<KnowledgeStore>,
}

#[derive(serde::Deserialize)]
struct ExecuteRequest {
    tenant_id: String,
    correlation_id: Option<String>,
    goal: Goal,
}

async fn execute(
    State(state): State<AppState>,
    Json(req): Json<ExecuteRequest>,
) -> axum::Json<serde_json::Value> {
    let ctx = TenantContext {
        tenant_id: req.tenant_id,
        correlation_id: req.correlation_id,
    };
    match state.orchestrator.dispatch(&ctx, req.goal).await {
        Ok(result) => axum::Json(result),
        Err(e) => axum::Json(serde_json::json!({
            "error": e.to_string(),
            "status": "error"
        })),
    }
}

const KB_SLOT_INTERNAL_RESEARCH: u8 = 8;

async fn get_research_trace(
    State(state): State<AppState>,
    Path(trace_id): Path<String>,
) -> Result<axum::Json<serde_json::Value>, axum::http::StatusCode> {
    let value = state
        .knowledge
        .get(KB_SLOT_INTERNAL_RESEARCH, &trace_id)
        .map_err(|_| axum::http::StatusCode::INTERNAL_SERVER_ERROR)?
        .and_then(|b| String::from_utf8(b).ok());
    let value = value.ok_or(axum::http::StatusCode::NOT_FOUND)?;
    let trace: serde_json::Value =
        serde_json::from_str(&value).map_err(|_| axum::http::StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(axum::Json(trace))
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use tower::ServiceExt;

    #[tokio::test]
    async fn test_execute_lead_capture() {
        let memory = Arc::new(pagi_memory::MemoryManager::new().unwrap());
        let knowledge = Arc::new(
            pagi_knowledge::KnowledgeStore::open_path("./data/pagi_knowledge_lead_test").unwrap(),
        );
        let mut registry = SkillRegistry::new();
        registry.register(Arc::new(LeadCapture::new(Arc::clone(&memory))));
        let orchestrator = Arc::new(Orchestrator::new(Arc::new(registry)));
        let app = Router::new()
            .route("/v1/execute", post(execute))
            .with_state(AppState { orchestrator, knowledge });

        let body = serde_json::json!({
            "tenant_id": "test-tenant",
            "goal": {
                "IngestData": {
                    "payload": { "email": "lead@example.com", "message": "Customer inquiry" }
                }
            }
        });
        let req = Request::builder()
            .method("POST")
            .uri("/v1/execute")
            .header("content-type", "application/json")
            .body(Body::from(serde_json::to_string(&body).unwrap()))
            .unwrap();
        let res = app.oneshot(req).await.unwrap();
        assert_eq!(res.status(), StatusCode::OK);
        let bytes = axum::body::to_bytes(res.into_body(), usize::MAX)
            .await
            .unwrap();
        let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(json["status"], "saved");
        assert_eq!(json["skill"], "LeadCapture");
        assert!(json.get("lead_id").is_some());
    }

    #[tokio::test]
    async fn test_kb1_brand_voice_retrieve() {
        let knowledge = Arc::new(
            pagi_knowledge::KnowledgeStore::open_path("./data/pagi_knowledge_test")
                .unwrap(),
        );
        knowledge
            .insert(1, "brand_voice", b"Friendly and professional")
            .unwrap();

        let mut registry = SkillRegistry::new();
        registry.register(Arc::new(KnowledgeQuery::new(Arc::clone(&knowledge))));
        let orchestrator = Arc::new(Orchestrator::new(Arc::new(registry)));
        let app = Router::new()
            .route("/v1/execute", post(execute))
            .with_state(AppState { orchestrator, knowledge });

        let body = serde_json::json!({
            "tenant_id": "test-tenant",
            "goal": {
                "QueryKnowledge": { "slot_id": 1, "query": "brand_voice" }
            }
        });
        let req = Request::builder()
            .method("POST")
            .uri("/v1/execute")
            .header("content-type", "application/json")
            .body(Body::from(serde_json::to_string(&body).unwrap()))
            .unwrap();
        let res = app.oneshot(req).await.unwrap();
        assert_eq!(res.status(), StatusCode::OK);
        let bytes = axum::body::to_bytes(res.into_body(), usize::MAX)
            .await
            .unwrap();
        let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(json["status"], "ok");
        assert_eq!(json["skill"], "KnowledgeQuery");
        assert_eq!(json["slot_id"], 1);
        assert_eq!(json["query_key"], "brand_voice");
        assert_eq!(json["value"], "Friendly and professional");
    }

    #[tokio::test]
    async fn test_kb2_insert_and_retrieve_welcome_template() {
        let knowledge = Arc::new(
            pagi_knowledge::KnowledgeStore::open_path("./data/pagi_kb2_test").unwrap(),
        );
        let mut registry = SkillRegistry::new();
        registry.register(Arc::new(KnowledgeInsert::new(Arc::clone(&knowledge))));
        registry.register(Arc::new(KnowledgeQuery::new(Arc::clone(&knowledge))));
        let orchestrator = Arc::new(Orchestrator::new(Arc::new(registry)));
        let app = Router::new()
            .route("/v1/execute", post(execute))
            .with_state(AppState { orchestrator, knowledge });

        let insert_body = serde_json::json!({
            "tenant_id": "test-tenant",
            "goal": {
                "ExecuteSkill": {
                    "name": "KnowledgeInsert",
                    "payload": {
                        "slot_id": 2,
                        "key": "welcome_email_template",
                        "value": "Welcome! We're glad you reached out. A team member will follow up within 24 hours."
                    }
                }
            }
        });
        let insert_req = Request::builder()
            .method("POST")
            .uri("/v1/execute")
            .header("content-type", "application/json")
            .body(Body::from(serde_json::to_string(&insert_body).unwrap()))
            .unwrap();
        let insert_res = app.clone().oneshot(insert_req).await.unwrap();
        assert_eq!(insert_res.status(), StatusCode::OK);
        let insert_bytes = axum::body::to_bytes(insert_res.into_body(), usize::MAX)
            .await
            .unwrap();
        let insert_json: serde_json::Value = serde_json::from_slice(&insert_bytes).unwrap();
        assert_eq!(insert_json["status"], "ok");
        assert_eq!(insert_json["skill"], "KnowledgeInsert");
        assert_eq!(insert_json["slot_id"], 2);
        assert_eq!(insert_json["key"], "welcome_email_template");

        let query_body = serde_json::json!({
            "tenant_id": "test-tenant",
            "goal": {
                "QueryKnowledge": { "slot_id": 2, "query": "welcome_email_template" }
            }
        });
        let query_req = Request::builder()
            .method("POST")
            .uri("/v1/execute")
            .header("content-type", "application/json")
            .body(Body::from(serde_json::to_string(&query_body).unwrap()))
            .unwrap();
        let query_res = app.oneshot(query_req).await.unwrap();
        assert_eq!(query_res.status(), StatusCode::OK);
        let query_bytes = axum::body::to_bytes(query_res.into_body(), usize::MAX)
            .await
            .unwrap();
        let query_json: serde_json::Value = serde_json::from_slice(&query_bytes).unwrap();
        assert_eq!(query_json["status"], "ok");
        assert_eq!(query_json["skill"], "KnowledgeQuery");
        assert_eq!(query_json["value"], "Welcome! We're glad you reached out. A team member will follow up within 24 hours.");
    }

    #[tokio::test]
    async fn test_draft_response_includes_brand_voice_and_local_event() {
        let memory = Arc::new(pagi_memory::MemoryManager::open_path("./data/pagi_vault_draft_test").unwrap());
        let knowledge = Arc::new(
            pagi_knowledge::KnowledgeStore::open_path("./data/pagi_knowledge_draft_test").unwrap(),
        );

        // Set Brand Voice in KB-1
        knowledge.insert(1, "brand_voice", b"Warm, neighborly, and helpful").unwrap();

        // Set Local Event in KB-5 via CommunityPulse
        let mut registry = SkillRegistry::new();
        registry.register(Arc::new(LeadCapture::new(Arc::clone(&memory))));
        registry.register(Arc::new(KnowledgeInsert::new(Arc::clone(&knowledge))));
        registry.register(Arc::new(CommunityPulse::new(Arc::clone(&knowledge))));
        registry.register(Arc::new(DraftResponse::new(
            Arc::clone(&memory),
            Arc::clone(&knowledge),
        )));
        let orchestrator = Arc::new(Orchestrator::new(Arc::new(registry)));
        let app = Router::new()
            .route("/v1/execute", post(execute))
            .with_state(AppState { orchestrator, knowledge });

        // 1. Capture a lead to get lead_id (IngestData)
        let lead_body = serde_json::json!({
            "tenant_id": "test-tenant",
            "goal": {
                "IngestData": {
                    "payload": { "email": "customer@example.com", "message": "Interested in services" }
                }
            }
        });
        let lead_req = Request::builder()
            .method("POST")
            .uri("/v1/execute")
            .header("content-type", "application/json")
            .body(Body::from(serde_json::to_string(&lead_body).unwrap()))
            .unwrap();
        let lead_res = app.clone().oneshot(lead_req).await.unwrap();
        assert_eq!(lead_res.status(), StatusCode::OK);
        let lead_bytes = axum::body::to_bytes(lead_res.into_body(), usize::MAX).await.unwrap();
        let lead_json: serde_json::Value = serde_json::from_slice(&lead_bytes).unwrap();
        let lead_id = lead_json["lead_id"].as_str().unwrap().to_string();

        // 2. Set Community Pulse (e.g. Strawberry Festival) in KB-5
        let pulse_body = serde_json::json!({
            "tenant_id": "test-tenant",
            "goal": {
                "ExecuteSkill": {
                    "name": "CommunityPulse",
                    "payload": {
                        "location": "Stockdale",
                        "trend": "rainy week",
                        "event": "Strawberry Festival this weekend"
                    }
                }
            }
        });
        let pulse_req = Request::builder()
            .method("POST")
            .uri("/v1/execute")
            .header("content-type", "application/json")
            .body(Body::from(serde_json::to_string(&pulse_body).unwrap()))
            .unwrap();
        let pulse_res = app.clone().oneshot(pulse_req).await.unwrap();
        assert_eq!(pulse_res.status(), StatusCode::OK);

        // 3. Execute AssembleContext (draft for this context_id)
        let draft_body = serde_json::json!({
            "tenant_id": "test-tenant",
            "goal": {
                "AssembleContext": { "context_id": lead_id }
            }
        });
        let draft_req = Request::builder()
            .method("POST")
            .uri("/v1/execute")
            .header("content-type", "application/json")
            .body(Body::from(serde_json::to_string(&draft_body).unwrap()))
            .unwrap();
        let draft_res = app.oneshot(draft_req).await.unwrap();
        assert_eq!(draft_res.status(), StatusCode::OK);
        let draft_bytes = axum::body::to_bytes(draft_res.into_body(), usize::MAX).await.unwrap();
        let draft_json: serde_json::Value = serde_json::from_slice(&draft_bytes).unwrap();
        assert_eq!(draft_json["status"], "ok");
        assert_eq!(draft_json["skill"], "DraftResponse");

        let draft_text = draft_json["draft"].as_str().unwrap();
        assert!(draft_text.contains("Warm, neighborly, and helpful"), "draft should include Brand Voice from KB-1");
        assert!(draft_text.contains("Strawberry Festival this weekend"), "draft should include Local Event from KB-5");
        assert!(draft_text.contains("Local Context:"), "draft should include Local Context section");
    }

    #[tokio::test]
    async fn test_generate_final_response_chain_returns_generated_string() {
        let memory = Arc::new(
            pagi_memory::MemoryManager::open_path("./data/pagi_vault_generate_test").unwrap(),
        );
        let knowledge = Arc::new(
            pagi_knowledge::KnowledgeStore::open_path("./data/pagi_knowledge_generate_test").unwrap(),
        );
        knowledge.insert(1, "brand_voice", b"Warm and professional").unwrap();

        let mut registry = SkillRegistry::new();
        registry.register(Arc::new(LeadCapture::new(Arc::clone(&memory))));
        registry.register(Arc::new(DraftResponse::new(
            Arc::clone(&memory),
            Arc::clone(&knowledge),
        )));
        registry.register(Arc::new(ModelRouter::new()));
        let orchestrator = Arc::new(Orchestrator::new(Arc::new(registry)));
        let app = Router::new()
            .route("/v1/execute", post(execute))
            .with_state(AppState { orchestrator, knowledge });

        // 1. Capture a lead (IngestData)
        let lead_body = serde_json::json!({
            "tenant_id": "test-tenant",
            "goal": {
                "IngestData": {
                    "payload": { "email": "guest@example.com", "message": "Hello" }
                }
            }
        });
        let lead_req = Request::builder()
            .method("POST")
            .uri("/v1/execute")
            .header("content-type", "application/json")
            .body(Body::from(serde_json::to_string(&lead_body).unwrap()))
            .unwrap();
        let lead_res = app.clone().oneshot(lead_req).await.unwrap();
        assert_eq!(lead_res.status(), StatusCode::OK);
        let lead_bytes = axum::body::to_bytes(lead_res.into_body(), usize::MAX).await.unwrap();
        let lead_json: serde_json::Value = serde_json::from_slice(&lead_bytes).unwrap();
        let lead_id = lead_json["lead_id"].as_str().unwrap().to_string();

        // 2. Generate final response (AssembleContext -> ModelRouter chain)
        let gen_body = serde_json::json!({
            "tenant_id": "test-tenant",
            "goal": {
                "GenerateFinalResponse": { "context_id": lead_id }
            }
        });
        let gen_req = Request::builder()
            .method("POST")
            .uri("/v1/execute")
            .header("content-type", "application/json")
            .body(Body::from(serde_json::to_string(&gen_body).unwrap()))
            .unwrap();
        let gen_res = app.oneshot(gen_req).await.unwrap();
        assert_eq!(gen_res.status(), StatusCode::OK);
        let gen_bytes = axum::body::to_bytes(gen_res.into_body(), usize::MAX).await.unwrap();
        let gen_json: serde_json::Value = serde_json::from_slice(&gen_bytes).unwrap();

        assert_eq!(gen_json["status"], "ok");
        assert_eq!(gen_json["goal"], "GenerateFinalResponse");
        assert_eq!(gen_json["context_id"], lead_id);
        let generated = gen_json["generated"].as_str().expect("response must contain 'generated' string");
        assert!(!generated.is_empty(), "generated text must not be empty");
        assert!(
            generated.contains("Generated") || generated.contains("personalized") || generated.contains("Thank you"),
            "generated should be LLM-style output, not just the raw mock draft template"
        );
    }

    #[tokio::test]
    async fn test_autonomous_goal_respond_to_lead_triggers_generation_chain() {
        let memory = Arc::new(
            pagi_memory::MemoryManager::open_path("./data/pagi_vault_autonomous_test").unwrap(),
        );
        let knowledge = Arc::new(
            pagi_knowledge::KnowledgeStore::open_path("./data/pagi_knowledge_autonomous_test").unwrap(),
        );
        knowledge.insert(1, "brand_voice", b"Friendly and local").unwrap();

        let mut registry = SkillRegistry::new();
        registry.register(Arc::new(LeadCapture::new(Arc::clone(&memory))));
        registry.register(Arc::new(DraftResponse::new(
            Arc::clone(&memory),
            Arc::clone(&knowledge),
        )));
        registry.register(Arc::new(SalesCloser::new(Arc::clone(&knowledge))));
        registry.register(Arc::new(ModelRouter::new()));
        registry.register(Arc::new(ResearchAudit::new(Arc::clone(&knowledge))));
        let orchestrator = Arc::new(Orchestrator::new(Arc::new(registry)));
        let app = Router::new()
            .route("/v1/execute", post(execute))
            .route("/v1/research/trace/:trace_id", get(get_research_trace))
            .with_state(AppState { orchestrator, knowledge });

        // 1. Capture a lead (IngestData)
        let lead_body = serde_json::json!({
            "tenant_id": "test-tenant",
            "goal": {
                "IngestData": {
                    "payload": { "email": "neighbor@town.com", "message": "Interested in events" }
                }
            }
        });
        let lead_req = Request::builder()
            .method("POST")
            .uri("/v1/execute")
            .header("content-type", "application/json")
            .body(Body::from(serde_json::to_string(&lead_body).unwrap()))
            .unwrap();
        let lead_res = app.clone().oneshot(lead_req).await.unwrap();
        assert_eq!(lead_res.status(), StatusCode::OK);
        let lead_bytes = axum::body::to_bytes(lead_res.into_body(), usize::MAX).await.unwrap();
        let lead_json: serde_json::Value = serde_json::from_slice(&lead_bytes).unwrap();
        let lead_id = lead_json["lead_id"].as_str().unwrap().to_string();

        // 2. AutonomousGoal "respond to lead" with context.lead_id
        let autonomous_body = serde_json::json!({
            "tenant_id": "test-tenant",
            "goal": {
                "AutonomousGoal": {
                    "intent": "respond to lead",
                    "context": { "lead_id": lead_id }
                }
            }
        });
        let autonomous_req = Request::builder()
            .method("POST")
            .uri("/v1/execute")
            .header("content-type", "application/json")
            .body(Body::from(serde_json::to_string(&autonomous_body).unwrap()))
            .unwrap();
        let autonomous_res = app.clone().oneshot(autonomous_req).await.unwrap();
        assert_eq!(autonomous_res.status(), StatusCode::OK);
        let autonomous_bytes = axum::body::to_bytes(autonomous_res.into_body(), usize::MAX).await.unwrap();
        let auto_json: serde_json::Value = serde_json::from_slice(&autonomous_bytes).unwrap();

        assert_eq!(auto_json["goal"], "AutonomousGoal");
        assert_eq!(auto_json["intent"], "respond to lead");
        assert_eq!(
            auto_json["plan_steps"],
            serde_json::json!(["DraftResponse", "SalesCloser", "ModelRouter"])
        );
        let generated = auto_json["generated"].as_str().expect("response must contain 'generated' from chain");
        assert!(!generated.is_empty());
        assert!(
            generated.contains("Generated") || generated.contains("personalized") || generated.contains("Thank you"),
            "autonomous chain should produce LLM-style generated text"
        );
        let trace_id = auto_json["trace_id"].as_str().expect("ResearchAudit should return trace_id");
        assert!(!trace_id.is_empty());

        // 3. Retrieve Thought Log from research endpoint
        let trace_req = Request::builder()
            .method("GET")
            .uri(format!("/v1/research/trace/{}", trace_id))
            .body(Body::empty())
            .unwrap();
        let trace_res = app.oneshot(trace_req).await.unwrap();
        assert_eq!(trace_res.status(), StatusCode::OK);
        let trace_bytes = axum::body::to_bytes(trace_res.into_body(), usize::MAX).await.unwrap();
        let trace_json: serde_json::Value = serde_json::from_slice(&trace_bytes).unwrap();
        assert_eq!(trace_json["trace_id"], trace_id);
        let trace_inner = &trace_json["trace"];
        assert_eq!(trace_inner["intent"], "respond to lead");
        assert_eq!(
            trace_inner["plan_steps"],
            serde_json::json!(["DraftResponse", "SalesCloser", "ModelRouter"])
        );
        let steps = trace_inner["steps"].as_array().expect("trace should have steps array");
        assert_eq!(steps.len(), 3, "respond to lead has three steps");
        assert_eq!(steps[0]["skill"], "DraftResponse");
        assert_eq!(steps[1]["skill"], "SalesCloser");
        assert_eq!(steps[2]["skill"], "ModelRouter");
        assert!(trace_inner.get("final_result").is_some(), "trace should have final_result");
    }

    #[tokio::test]
    async fn test_community_scraper_extracts_event_and_saves_to_kb5() {
        let knowledge = Arc::new(
            pagi_knowledge::KnowledgeStore::open_path("./data/pagi_knowledge_scraper_test").unwrap(),
        );
        let mut registry = SkillRegistry::new();
        registry.register(Arc::new(CommunityScraper::new(Arc::clone(&knowledge))));
        registry.register(Arc::new(KnowledgeQuery::new(Arc::clone(&knowledge))));
        let orchestrator = Arc::new(Orchestrator::new(Arc::new(registry)));
        let app = Router::new()
            .route("/v1/execute", post(execute))
            .with_state(AppState { orchestrator, knowledge });

        let mock_html = r#"<!DOCTYPE html>
<html><body>
<h1>Stockdale Fair 2025</h1>
<h2>Local events this weekend</h2>
<article><h2>Farmers Market Sunday</h2></article>
</body></html>"#;

        let scrape_body = serde_json::json!({
            "tenant_id": "test-tenant",
            "goal": {
                "ExecuteSkill": {
                    "name": "CommunityScraper",
                    "payload": {
                        "url": "https://example.com/local-news",
                        "html": mock_html
                    }
                }
            }
        });
        let scrape_req = Request::builder()
            .method("POST")
            .uri("/v1/execute")
            .header("content-type", "application/json")
            .body(Body::from(serde_json::to_string(&scrape_body).unwrap()))
            .unwrap();
        let scrape_res = app.clone().oneshot(scrape_req).await.unwrap();
        assert_eq!(scrape_res.status(), StatusCode::OK);
        let scrape_bytes = axum::body::to_bytes(scrape_res.into_body(), usize::MAX).await.unwrap();
        let scrape_json: serde_json::Value = serde_json::from_slice(&scrape_bytes).unwrap();
        assert_eq!(scrape_json["status"], "ok");
        assert_eq!(scrape_json["skill"], "CommunityScraper");
        assert_eq!(scrape_json["slot_id"], 5);
        assert!(scrape_json["event"].as_str().unwrap().contains("Stockdale Fair 2025"));
        assert!(scrape_json["event"].as_str().unwrap().contains("Local events this weekend"));
        assert!(scrape_json["event"].as_str().unwrap().contains("Farmers Market Sunday"));

        let query_body = serde_json::json!({
            "tenant_id": "test-tenant",
            "goal": {
                "QueryKnowledge": { "slot_id": 5, "query": "current_pulse" }
            }
        });
        let query_req = Request::builder()
            .method("POST")
            .uri("/v1/execute")
            .header("content-type", "application/json")
            .body(Body::from(serde_json::to_string(&query_body).unwrap()))
            .unwrap();
        let query_res = app.oneshot(query_req).await.unwrap();
        assert_eq!(query_res.status(), StatusCode::OK);
        let query_bytes = axum::body::to_bytes(query_res.into_body(), usize::MAX).await.unwrap();
        let query_json: serde_json::Value = serde_json::from_slice(&query_bytes).unwrap();
        assert_eq!(query_json["status"], "ok");
        assert_eq!(query_json["slot_id"], 5);
        assert_eq!(query_json["query_key"], "current_pulse");
        let value = query_json["value"].as_str().expect("current_pulse value");
        let pulse: serde_json::Value = serde_json::from_str(value).unwrap();
        assert_eq!(pulse["location"], "Stockdale");
        assert_eq!(pulse["trend"], "Scraped");
        assert!(pulse["event"].as_str().unwrap().contains("Stockdale Fair 2025"));
    }

    #[tokio::test]
    async fn test_refresh_local_context_dispatches_community_scraper() {
        let knowledge = Arc::new(
            pagi_knowledge::KnowledgeStore::open_path("./data/pagi_knowledge_refresh_test").unwrap(),
        );
        let mut registry = SkillRegistry::new();
        registry.register(Arc::new(CommunityScraper::new(Arc::clone(&knowledge))));
        registry.register(Arc::new(KnowledgeQuery::new(Arc::clone(&knowledge))));
        let orchestrator = Arc::new(Orchestrator::new(Arc::new(registry)));
        let app = Router::new()
            .route("/v1/execute", post(execute))
            .with_state(AppState { orchestrator, knowledge });

        let mock_html = r#"<html><body><h1>Fall Festival Next Week</h1></body></html>"#;
        let body = serde_json::json!({
            "tenant_id": "test-tenant",
            "goal": {
                "UpdateKnowledgeSlot": {
                    "slot_id": 5,
                    "source_url": "https://example.com/news",
                    "source_html": mock_html
                }
            }
        });
        let req = Request::builder()
            .method("POST")
            .uri("/v1/execute")
            .header("content-type", "application/json")
            .body(Body::from(serde_json::to_string(&body).unwrap()))
            .unwrap();
        let res = app.oneshot(req).await.unwrap();
        assert_eq!(res.status(), StatusCode::OK);
        let bytes = axum::body::to_bytes(res.into_body(), usize::MAX).await.unwrap();
        let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(json["status"], "ok");
        assert_eq!(json["skill"], "CommunityScraper");
        assert!(json["event"].as_str().unwrap().contains("Fall Festival Next Week"));
    }

    #[tokio::test]
    async fn test_sales_closer_cta_in_final_response() {
        let memory = Arc::new(
            pagi_memory::MemoryManager::open_path("./data/pagi_vault_sales_test").unwrap(),
        );
        let knowledge = Arc::new(
            pagi_knowledge::KnowledgeStore::open_path("./data/pagi_knowledge_sales_test").unwrap(),
        );
        knowledge.insert(1, "brand_voice", b"Warm and professional").unwrap();
        knowledge
            .insert(2, "closing_strategy", b"Book a free consultation today")
            .unwrap();

        let mut registry = SkillRegistry::new();
        registry.register(Arc::new(LeadCapture::new(Arc::clone(&memory))));
        registry.register(Arc::new(DraftResponse::new(
            Arc::clone(&memory),
            Arc::clone(&knowledge),
        )));
        registry.register(Arc::new(SalesCloser::new(Arc::clone(&knowledge))));
        registry.register(Arc::new(ModelRouter::new()));
        let orchestrator = Arc::new(Orchestrator::new(Arc::new(registry)));
        let app = Router::new()
            .route("/v1/execute", post(execute))
            .with_state(AppState { orchestrator, knowledge });

        let lead_body = serde_json::json!({
            "tenant_id": "test-tenant",
            "goal": {
                "IngestData": {
                    "payload": { "email": "lead@example.com", "message": "Interested in services" }
                }
            }
        });
        let lead_req = Request::builder()
            .method("POST")
            .uri("/v1/execute")
            .header("content-type", "application/json")
            .body(Body::from(serde_json::to_string(&lead_body).unwrap()))
            .unwrap();
        let lead_res = app.clone().oneshot(lead_req).await.unwrap();
        assert_eq!(lead_res.status(), StatusCode::OK);
        let lead_bytes = axum::body::to_bytes(lead_res.into_body(), usize::MAX).await.unwrap();
        let lead_json: serde_json::Value = serde_json::from_slice(&lead_bytes).unwrap();
        let lead_id = lead_json["lead_id"].as_str().unwrap().to_string();

        let auto_body = serde_json::json!({
            "tenant_id": "test-tenant",
            "goal": {
                "AutonomousGoal": {
                    "intent": "respond to lead",
                    "context": { "lead_id": lead_id }
                }
            }
        });
        let auto_req = Request::builder()
            .method("POST")
            .uri("/v1/execute")
            .header("content-type", "application/json")
            .body(Body::from(serde_json::to_string(&auto_body).unwrap()))
            .unwrap();
        let auto_res = app.oneshot(auto_req).await.unwrap();
        assert_eq!(auto_res.status(), StatusCode::OK);
        let auto_bytes = axum::body::to_bytes(auto_res.into_body(), usize::MAX).await.unwrap();
        let auto_json: serde_json::Value = serde_json::from_slice(&auto_bytes).unwrap();
        let generated = auto_json["generated"].as_str().expect("generated");
        assert!(
            generated.to_lowercase().contains("free consultation"),
            "final generated response should include the KB-2 sales push (free consultation); got: {}",
            generated
        );
    }

    #[tokio::test]
    async fn test_blueprint_alternate_intent_summarize_news() {
        let knowledge = Arc::new(
            pagi_knowledge::KnowledgeStore::open_path("./data/pagi_knowledge_blueprint_test").unwrap(),
        );
        let mut registry = SkillRegistry::new();
        registry.register(Arc::new(CommunityScraper::new(Arc::clone(&knowledge))));
        registry.register(Arc::new(ModelRouter::new()));

        let mut intents = std::collections::HashMap::new();
        intents.insert(
            "summarize news".to_string(),
            vec!["CommunityScraper".to_string(), "ModelRouter".to_string()],
        );
        let blueprint = Arc::new(BlueprintRegistry::from_intents(intents));
        let orchestrator = Arc::new(Orchestrator::with_blueprint(
            Arc::new(registry),
            Arc::clone(&blueprint),
        ));
        let app = Router::new()
            .route("/v1/execute", post(execute))
            .with_state(AppState { orchestrator, knowledge });

        let body = serde_json::json!({
            "tenant_id": "test-tenant",
            "goal": {
                "AutonomousGoal": {
                    "intent": "summarize news",
                    "context": {
                        "slot_id": 5,
                        "html": "<html><body><h1>Local Election Results</h1><h2>Budget approved</h2></body></html>"
                    }
                }
            }
        });
        let req = Request::builder()
            .method("POST")
            .uri("/v1/execute")
            .header("content-type", "application/json")
            .body(Body::from(serde_json::to_string(&body).unwrap()))
            .unwrap();
        let res = app.oneshot(req).await.unwrap();
        assert_eq!(res.status(), StatusCode::OK);
        let bytes = axum::body::to_bytes(res.into_body(), usize::MAX).await.unwrap();
        let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(json["goal"], "AutonomousGoal");
        assert_eq!(json["intent"], "summarize news");
        assert_eq!(
            json["plan_steps"],
            serde_json::json!(["CommunityScraper", "ModelRouter"])
        );
        let generated = json["generated"].as_str().expect("generated");
        assert!(
            generated.contains("Election") || generated.contains("Budget") || generated.contains("personalized"),
            "generated should reflect scraped content or mock; got: {}",
            generated
        );
    }
}
