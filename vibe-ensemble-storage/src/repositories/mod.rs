//! Repository implementations for all domain entities

pub mod agent;
pub mod issue;
pub mod knowledge;
pub mod knowledge_intelligence;
pub mod message;
pub mod prompt;
pub mod template;

pub use agent::AgentRepository;
pub use issue::IssueRepository;
pub use knowledge::KnowledgeRepository;
pub use knowledge_intelligence::KnowledgeIntelligenceRepository;
pub use message::MessageRepository;
pub use prompt::PromptRepository;
pub use template::TemplateRepository;
