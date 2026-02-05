//! Trait-based agent capability registry and concrete skills.

pub use pagi_orchestrator::{AgentSkill, SkillRegistry};

mod community_pulse;
mod community_scraper;
mod draft_response;
mod knowledge_insert;
mod knowledge_pruner;
mod knowledge_query;
mod lead_capture;
mod model_router;
mod research_audit;
mod sales_closer;

pub use community_pulse::CommunityPulse;
pub use community_scraper::CommunityScraper;
pub use draft_response::DraftResponse;
pub use knowledge_insert::KnowledgeInsert;
pub use knowledge_pruner::KnowledgePruner;
pub use knowledge_query::KnowledgeQuery;
pub use lead_capture::LeadCapture;
pub use model_router::{LlmMode, ModelRouter};
pub use research_audit::ResearchAudit;
pub use sales_closer::SalesCloser;
