//! Trait-based agent capability registry and concrete skills.

pub use pagi_core::{AgentSkill, SkillRegistry};

mod community_pulse;
mod community_scraper;
mod draft_response;
mod knowledge_insert;
mod knowledge_pruner;
mod knowledge_query;
mod lead_capture;
mod fs_tools;
mod model_router;
mod analyze_sentiment;
mod check_alignment;
mod recall_past_actions;
mod research_semantic;
mod research_audit;
mod sales_closer;
mod thalamus;
mod message_agent;
mod get_agent_messages;

pub use analyze_sentiment::AnalyzeSentiment;
pub use check_alignment::CheckAlignment;
pub use community_pulse::CommunityPulse;
pub use community_scraper::CommunityScraper;
pub use draft_response::DraftResponse;
pub use knowledge_insert::KnowledgeInsert;
pub use knowledge_pruner::KnowledgePruner;
pub use knowledge_query::KnowledgeQuery;
pub use lead_capture::LeadCapture;
pub use fs_tools::{analyze_workspace, FsWorkspaceAnalyzer, WriteSandboxFile};
pub use model_router::{LlmMode, ModelRouter};
pub use research_semantic::{ResearchEmbedInsert, ResearchSemanticSearch};
pub use recall_past_actions::RecallPastActions;
pub use research_audit::ResearchAudit;
pub use sales_closer::SalesCloser;
pub use thalamus::{route_information, route_to_ontology, RouteMetadata};
pub use message_agent::MessageAgent;
pub use get_agent_messages::GetAgentMessages;
