//! Repository implementations for all domain entities

pub mod agent;
pub mod issue;
pub mod knowledge;
pub mod message;
pub mod prompt;

pub use agent::AgentRepository;
pub use issue::IssueRepository;
pub use knowledge::KnowledgeRepository;
pub use message::MessageRepository;
pub use prompt::PromptRepository;