//! Knowledge domain model and related types
//!
//! This module provides the core knowledge model for managing patterns, practices,
//! guidelines, and other knowledge artifacts in the Vibe Ensemble system.
//!
//! # Examples
//!
//! Creating a new knowledge entry:
//!
//! ```rust
//! use vibe_ensemble_core::knowledge::*;
//! use uuid::Uuid;
//!
//! let knowledge = Knowledge::builder()
//!     .title("REST API Design Patterns")
//!     .content("Best practices for designing RESTful APIs...")
//!     .knowledge_type(KnowledgeType::Pattern)
//!     .created_by(Uuid::new_v4())
//!     .access_level(AccessLevel::Public)
//!     .tag("api")
//!     .tag("rest")
//!     .tag("design")
//!     .build()
//!     .unwrap();
//! ```
//!
//! Creating knowledge relationships:
//!
//! ```rust
//! use vibe_ensemble_core::knowledge::*;
//! use uuid::Uuid;
//!
//! let relation = KnowledgeRelation::new(
//!     Uuid::new_v4(), // source knowledge ID
//!     Uuid::new_v4(), // target knowledge ID
//!     RelationType::References,
//! );
//! ```

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use crate::{Error, Result};

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
    /// Create a new knowledge entry with validation
    pub fn new(
        title: String,
        content: String,
        knowledge_type: KnowledgeType,
        created_by: Uuid,
        access_level: AccessLevel,
    ) -> Result<Self> {
        Self::validate_title(&title)?;
        Self::validate_content(&content)?;
        
        let now = Utc::now();
        Ok(Self {
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
        })
    }

    /// Create a builder for constructing a Knowledge entry
    pub fn builder() -> KnowledgeBuilder {
        KnowledgeBuilder::new()
    }

    /// Validate knowledge title
    fn validate_title(title: &str) -> Result<()> {
        if title.trim().is_empty() {
            return Err(Error::Validation {
                message: "Knowledge title cannot be empty".to_string(),
            });
        }
        if title.len() > 300 {
            return Err(Error::Validation {
                message: "Knowledge title cannot exceed 300 characters".to_string(),
            });
        }
        Ok(())
    }

    /// Validate knowledge content
    fn validate_content(content: &str) -> Result<()> {
        if content.trim().is_empty() {
            return Err(Error::Validation {
                message: "Knowledge content cannot be empty".to_string(),
            });
        }
        if content.len() > 50000 {
            return Err(Error::Validation {
                message: "Knowledge content cannot exceed 50000 characters".to_string(),
            });
        }
        Ok(())
    }

    /// Update the knowledge content and increment version
    pub fn update_content(&mut self, content: String) -> Result<()> {
        Self::validate_content(&content)?;
        self.content = content;
        self.updated_at = Utc::now();
        self.version += 1;
        Ok(())
    }

    /// Add a tag to the knowledge entry
    pub fn add_tag(&mut self, tag: String) -> Result<()> {
        if tag.trim().is_empty() {
            return Err(Error::Validation {
                message: "Tag cannot be empty".to_string(),
            });
        }
        if tag.len() > 50 {
            return Err(Error::Validation {
                message: "Tag cannot exceed 50 characters".to_string(),
            });
        }
        if !self.tags.contains(&tag) {
            self.tags.push(tag);
            self.updated_at = Utc::now();
        }
        Ok(())
    }

    /// Remove a tag from the knowledge entry
    pub fn remove_tag(&mut self, tag: &str) {
        if let Some(pos) = self.tags.iter().position(|t| t == tag) {
            self.tags.remove(pos);
            self.updated_at = Utc::now();
        }
    }

    /// Check if the knowledge has a specific tag
    pub fn has_tag(&self, tag: &str) -> bool {
        self.tags.contains(&tag.to_string())
    }

    /// Update the access level
    pub fn set_access_level(&mut self, access_level: AccessLevel) {
        if self.access_level != access_level {
            self.access_level = access_level;
            self.updated_at = Utc::now();
        }
    }

    /// Get the age of the knowledge entry in seconds
    pub fn age_seconds(&self) -> i64 {
        Utc::now().signed_duration_since(self.created_at).num_seconds()
    }

    /// Get the time since last update in seconds
    pub fn time_since_update_seconds(&self) -> i64 {
        Utc::now().signed_duration_since(self.updated_at).num_seconds()
    }

    /// Search content for a specific term (case-insensitive)
    pub fn contains_term(&self, term: &str) -> bool {
        let term_lower = term.to_lowercase();
        self.title.to_lowercase().contains(&term_lower)
            || self.content.to_lowercase().contains(&term_lower)
            || self.tags.iter().any(|tag| tag.to_lowercase().contains(&term_lower))
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

impl KnowledgeRelation {
    /// Create a new knowledge relation
    pub fn new(
        source_id: Uuid,
        target_id: Uuid,
        relation_type: RelationType,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            source_id,
            target_id,
            relation_type,
            created_at: Utc::now(),
        }
    }

    /// Check if this relation connects two specific knowledge entries
    pub fn connects(&self, id1: Uuid, id2: Uuid) -> bool {
        (self.source_id == id1 && self.target_id == id2)
            || (self.source_id == id2 && self.target_id == id1)
    }

    /// Check if this relation involves a specific knowledge entry
    pub fn involves(&self, knowledge_id: Uuid) -> bool {
        self.source_id == knowledge_id || self.target_id == knowledge_id
    }

    /// Get the age of the relation in seconds
    pub fn age_seconds(&self) -> i64 {
        Utc::now().signed_duration_since(self.created_at).num_seconds()
    }
}

/// Builder for constructing Knowledge instances with validation
#[derive(Debug, Clone)]
pub struct KnowledgeBuilder {
    title: Option<String>,
    content: Option<String>,
    knowledge_type: Option<KnowledgeType>,
    created_by: Option<Uuid>,
    access_level: Option<AccessLevel>,
    tags: Vec<String>,
}

impl KnowledgeBuilder {
    /// Create a new knowledge builder
    pub fn new() -> Self {
        Self {
            title: None,
            content: None,
            knowledge_type: None,
            created_by: None,
            access_level: None,
            tags: Vec::new(),
        }
    }

    /// Set the knowledge title
    pub fn title<S: Into<String>>(mut self, title: S) -> Self {
        self.title = Some(title.into());
        self
    }

    /// Set the knowledge content
    pub fn content<S: Into<String>>(mut self, content: S) -> Self {
        self.content = Some(content.into());
        self
    }

    /// Set the knowledge type
    pub fn knowledge_type(mut self, knowledge_type: KnowledgeType) -> Self {
        self.knowledge_type = Some(knowledge_type);
        self
    }

    /// Set the creator ID
    pub fn created_by(mut self, created_by: Uuid) -> Self {
        self.created_by = Some(created_by);
        self
    }

    /// Set the access level
    pub fn access_level(mut self, access_level: AccessLevel) -> Self {
        self.access_level = Some(access_level);
        self
    }

    /// Add a tag
    pub fn tag<S: Into<String>>(mut self, tag: S) -> Self {
        self.tags.push(tag.into());
        self
    }

    /// Add multiple tags
    pub fn tags<I, S>(mut self, tags: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.tags.extend(tags.into_iter().map(|t| t.into()));
        self
    }

    /// Build the Knowledge instance
    pub fn build(self) -> Result<Knowledge> {
        let title = self.title.ok_or_else(|| Error::Validation {
            message: "Knowledge title is required".to_string(),
        })?;
        let content = self.content.ok_or_else(|| Error::Validation {
            message: "Knowledge content is required".to_string(),
        })?;
        let knowledge_type = self.knowledge_type.ok_or_else(|| Error::Validation {
            message: "Knowledge type is required".to_string(),
        })?;
        let created_by = self.created_by.ok_or_else(|| Error::Validation {
            message: "Creator ID is required".to_string(),
        })?;
        let access_level = self.access_level.ok_or_else(|| Error::Validation {
            message: "Access level is required".to_string(),
        })?;

        let mut knowledge = Knowledge::new(title, content, knowledge_type, created_by, access_level)?;
        
        // Add tags
        for tag in self.tags {
            knowledge.add_tag(tag)?;
        }
        
        Ok(knowledge)
    }
}

impl Default for KnowledgeBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_knowledge_creation_with_builder() {
        let creator_id = Uuid::new_v4();
        
        let knowledge = Knowledge::builder()
            .title("Test Pattern")
            .content("This is a test pattern for validation purposes.")
            .knowledge_type(KnowledgeType::Pattern)
            .created_by(creator_id)
            .access_level(AccessLevel::Public)
            .tag("test")
            .tag("pattern")
            .tag("validation")
            .build()
            .unwrap();

        assert_eq!(knowledge.title, "Test Pattern");
        assert_eq!(knowledge.knowledge_type, KnowledgeType::Pattern);
        assert_eq!(knowledge.created_by, creator_id);
        assert_eq!(knowledge.access_level, AccessLevel::Public);
        assert_eq!(knowledge.version, 1);
        assert_eq!(knowledge.tags.len(), 3);
        assert!(knowledge.has_tag("test"));
        assert!(knowledge.has_tag("pattern"));
        assert!(knowledge.has_tag("validation"));
    }

    #[test]
    fn test_knowledge_title_validation() {
        let creator_id = Uuid::new_v4();
        
        // Empty title should fail
        let result = Knowledge::builder()
            .title("")
            .content("Valid content")
            .knowledge_type(KnowledgeType::Pattern)
            .created_by(creator_id)
            .access_level(AccessLevel::Public)
            .build();
        assert!(result.is_err());

        // Too long title should fail
        let long_title = "a".repeat(301);
        let result = Knowledge::builder()
            .title(long_title)
            .content("Valid content")
            .knowledge_type(KnowledgeType::Pattern)
            .created_by(creator_id)
            .access_level(AccessLevel::Public)
            .build();
        assert!(result.is_err());
    }

    #[test]
    fn test_knowledge_content_validation() {
        let creator_id = Uuid::new_v4();
        
        // Empty content should fail
        let result = Knowledge::builder()
            .title("Valid Title")
            .content("")
            .knowledge_type(KnowledgeType::Pattern)
            .created_by(creator_id)
            .access_level(AccessLevel::Public)
            .build();
        assert!(result.is_err());

        // Too long content should fail
        let long_content = "a".repeat(50001);
        let result = Knowledge::builder()
            .title("Valid Title")
            .content(long_content)
            .knowledge_type(KnowledgeType::Pattern)
            .created_by(creator_id)
            .access_level(AccessLevel::Public)
            .build();
        assert!(result.is_err());
    }

    #[test]
    fn test_knowledge_content_update() {
        let creator_id = Uuid::new_v4();
        
        let mut knowledge = Knowledge::builder()
            .title("Test Pattern")
            .content("Original content")
            .knowledge_type(KnowledgeType::Pattern)
            .created_by(creator_id)
            .access_level(AccessLevel::Public)
            .build()
            .unwrap();

        let initial_version = knowledge.version;
        let initial_updated_at = knowledge.updated_at;
        
        std::thread::sleep(std::time::Duration::from_millis(10));
        knowledge.update_content("Updated content".to_string()).unwrap();
        
        assert_eq!(knowledge.content, "Updated content");
        assert_eq!(knowledge.version, initial_version + 1);
        assert!(knowledge.updated_at > initial_updated_at);

        // Test invalid content update
        let result = knowledge.update_content("".to_string());
        assert!(result.is_err());
    }

    #[test]
    fn test_knowledge_tag_operations() {
        let creator_id = Uuid::new_v4();
        
        let mut knowledge = Knowledge::builder()
            .title("Test Pattern")
            .content("Test content")
            .knowledge_type(KnowledgeType::Pattern)
            .created_by(creator_id)
            .access_level(AccessLevel::Public)
            .build()
            .unwrap();

        assert!(!knowledge.has_tag("test"));
        
        knowledge.add_tag("test".to_string()).unwrap();
        assert!(knowledge.has_tag("test"));
        
        // Adding duplicate tag should not error
        knowledge.add_tag("test".to_string()).unwrap();
        assert_eq!(knowledge.tags.len(), 1);
        
        // Adding empty tag should fail
        let result = knowledge.add_tag("".to_string());
        assert!(result.is_err());
        
        // Adding too long tag should fail
        let long_tag = "a".repeat(51);
        let result = knowledge.add_tag(long_tag);
        assert!(result.is_err());
        
        knowledge.remove_tag("test");
        assert!(!knowledge.has_tag("test"));
    }

    #[test]
    fn test_knowledge_access_control() {
        let creator_id = Uuid::new_v4();
        let other_id = Uuid::new_v4();
        
        // Test public access
        let public_knowledge = Knowledge::builder()
            .title("Public Pattern")
            .content("Public content")
            .knowledge_type(KnowledgeType::Pattern)
            .created_by(creator_id)
            .access_level(AccessLevel::Public)
            .build()
            .unwrap();

        assert!(public_knowledge.is_accessible_by(creator_id));
        assert!(public_knowledge.is_accessible_by(other_id));

        // Test private access
        let private_knowledge = Knowledge::builder()
            .title("Private Pattern")
            .content("Private content")
            .knowledge_type(KnowledgeType::Pattern)
            .created_by(creator_id)
            .access_level(AccessLevel::Private)
            .build()
            .unwrap();

        assert!(private_knowledge.is_accessible_by(creator_id));
        assert!(!private_knowledge.is_accessible_by(other_id));

        // Test team access (currently treats all agents as team members)
        let team_knowledge = Knowledge::builder()
            .title("Team Pattern")
            .content("Team content")
            .knowledge_type(KnowledgeType::Pattern)
            .created_by(creator_id)
            .access_level(AccessLevel::Team)
            .build()
            .unwrap();

        assert!(team_knowledge.is_accessible_by(creator_id));
        assert!(team_knowledge.is_accessible_by(other_id));
    }

    #[test]
    fn test_knowledge_search() {
        let creator_id = Uuid::new_v4();
        
        let knowledge = Knowledge::builder()
            .title("REST API Design")
            .content("Guidelines for designing RESTful APIs with proper HTTP methods.")
            .knowledge_type(KnowledgeType::Guideline)
            .created_by(creator_id)
            .access_level(AccessLevel::Public)
            .tag("api")
            .tag("rest")
            .build()
            .unwrap();

        // Test title search
        assert!(knowledge.contains_term("REST"));
        assert!(knowledge.contains_term("rest"));
        assert!(knowledge.contains_term("API"));
        
        // Test content search
        assert!(knowledge.contains_term("HTTP"));
        assert!(knowledge.contains_term("guidelines"));
        
        // Test tag search
        assert!(knowledge.contains_term("api"));
        assert!(knowledge.contains_term("rest"));
        
        // Test non-existent term
        assert!(!knowledge.contains_term("python"));
    }

    #[test]
    fn test_knowledge_age_and_updates() {
        let creator_id = Uuid::new_v4();
        
        let knowledge = Knowledge::builder()
            .title("Test Pattern")
            .content("Test content")
            .knowledge_type(KnowledgeType::Pattern)
            .created_by(creator_id)
            .access_level(AccessLevel::Public)
            .build()
            .unwrap();

        let age = knowledge.age_seconds();
        let time_since_update = knowledge.time_since_update_seconds();
        
        assert!(age >= 0);
        assert!(age < 60); // Should be very recent
        assert!(time_since_update >= 0);
        assert!(time_since_update < 60);
    }

    #[test]
    fn test_knowledge_access_level_update() {
        let creator_id = Uuid::new_v4();
        
        let mut knowledge = Knowledge::builder()
            .title("Test Pattern")
            .content("Test content")
            .knowledge_type(KnowledgeType::Pattern)
            .created_by(creator_id)
            .access_level(AccessLevel::Public)
            .build()
            .unwrap();

        let initial_updated_at = knowledge.updated_at;
        std::thread::sleep(std::time::Duration::from_millis(10));
        
        knowledge.set_access_level(AccessLevel::Private);
        assert_eq!(knowledge.access_level, AccessLevel::Private);
        assert!(knowledge.updated_at > initial_updated_at);
    }

    #[test]
    fn test_knowledge_relation() {
        let source_id = Uuid::new_v4();
        let target_id = Uuid::new_v4();
        
        let relation = KnowledgeRelation::new(
            source_id,
            target_id,
            RelationType::References,
        );

        assert_eq!(relation.source_id, source_id);
        assert_eq!(relation.target_id, target_id);
        assert_eq!(relation.relation_type, RelationType::References);
        
        assert!(relation.connects(source_id, target_id));
        assert!(relation.connects(target_id, source_id));
        assert!(relation.involves(source_id));
        assert!(relation.involves(target_id));
        assert!(!relation.involves(Uuid::new_v4()));
        
        let age = relation.age_seconds();
        assert!(age >= 0);
        assert!(age < 60);
    }

    #[test]
    fn test_knowledge_builder_validation() {
        let creator_id = Uuid::new_v4();
        
        // Missing title should fail
        let result = Knowledge::builder()
            .content("Valid content")
            .knowledge_type(KnowledgeType::Pattern)
            .created_by(creator_id)
            .access_level(AccessLevel::Public)
            .build();
        assert!(result.is_err());

        // Missing content should fail
        let result = Knowledge::builder()
            .title("Valid Title")
            .knowledge_type(KnowledgeType::Pattern)
            .created_by(creator_id)
            .access_level(AccessLevel::Public)
            .build();
        assert!(result.is_err());

        // Missing knowledge type should fail
        let result = Knowledge::builder()
            .title("Valid Title")
            .content("Valid content")
            .created_by(creator_id)
            .access_level(AccessLevel::Public)
            .build();
        assert!(result.is_err());

        // Missing creator should fail
        let result = Knowledge::builder()
            .title("Valid Title")
            .content("Valid content")
            .knowledge_type(KnowledgeType::Pattern)
            .access_level(AccessLevel::Public)
            .build();
        assert!(result.is_err());

        // Missing access level should fail
        let result = Knowledge::builder()
            .title("Valid Title")
            .content("Valid content")
            .knowledge_type(KnowledgeType::Pattern)
            .created_by(creator_id)
            .build();
        assert!(result.is_err());
    }
}