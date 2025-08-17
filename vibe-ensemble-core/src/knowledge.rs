//! Knowledge domain model and related types

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Represents a knowledge entry in the system
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Knowledge {
    pub id: Uuid,
    pub title: String,
    pub content: String,
    pub knowledge_type: KnowledgeType,
    pub tags: Vec<String>,
    pub created_by: Uuid,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub version: u32,
    pub access_level: AccessLevel,
}

/// Type of knowledge entry
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum KnowledgeType {
    Pattern,
    Practice,
    Guideline,
    Solution,
    Reference,
}

/// Access level for knowledge entries
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum AccessLevel {
    Public,
    Team,
    Private,
}

/// Represents a relationship between knowledge entries
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct KnowledgeRelation {
    pub id: Uuid,
    pub source_id: Uuid,
    pub target_id: Uuid,
    pub relation_type: RelationType,
    pub created_at: DateTime<Utc>,
}

/// Type of relationship between knowledge entries
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum RelationType {
    References,
    Supersedes,
    Conflicts,
    Complements,
    Implements,
}

impl Knowledge {
    /// Create a new knowledge entry
    pub fn new(
        title: String,
        content: String,
        knowledge_type: KnowledgeType,
        created_by: Uuid,
        access_level: AccessLevel,
    ) -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::new_v4(),
            title,
            content,
            knowledge_type,
            tags: Vec::new(),
            created_by,
            created_at: now,
            updated_at: now,
            version: 1,
            access_level,
        }
    }

    /// Update the knowledge content and increment version
    pub fn update_content(&mut self, content: String) {
        self.content = content;
        self.updated_at = Utc::now();
        self.version += 1;
    }

    /// Add a tag to the knowledge entry
    pub fn add_tag(&mut self, tag: String) {
        if !self.tags.contains(&tag) {
            self.tags.push(tag);
            self.updated_at = Utc::now();
        }
    }

    /// Check if the knowledge is accessible by a given agent
    pub fn is_accessible_by(&self, agent_id: Uuid) -> bool {
        match self.access_level {
            AccessLevel::Public => true,
            AccessLevel::Team => true, // For now, all agents are considered team members
            AccessLevel::Private => self.created_by == agent_id,
        }
    }
}