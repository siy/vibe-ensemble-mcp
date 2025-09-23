use anyhow::Result;
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use std::collections::{HashMap, HashSet, VecDeque};
use tracing::{debug, warn};

use super::DbPool;

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct TicketDependency {
    pub parent_ticket_id: String,
    pub child_ticket_id: String,
    pub dependency_type: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DependencyGraph {
    pub nodes: Vec<String>,
    pub edges: Vec<(String, String)>,
    pub levels: HashMap<String, usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DagValidationError {
    pub error_type: String,
    pub message: String,
    pub cycle_path: Option<Vec<String>>,
}

impl TicketDependency {
    /// Create a new dependency relationship with cycle validation
    pub async fn create(
        pool: &DbPool,
        parent_ticket_id: &str,
        child_ticket_id: &str,
        dependency_type: &str,
    ) -> Result<TicketDependency> {
        // Validate against self-dependency (already prevented by DB constraint but double-check)
        if parent_ticket_id == child_ticket_id {
            return Err(anyhow::anyhow!(
                "Cannot create dependency from ticket to itself: {}",
                parent_ticket_id
            ));
        }

        // Validate that both tickets exist
        let parent_exists = Self::ticket_exists(pool, parent_ticket_id).await?;
        let child_exists = Self::ticket_exists(pool, child_ticket_id).await?;

        if !parent_exists {
            return Err(anyhow::anyhow!(
                "Parent ticket '{}' does not exist",
                parent_ticket_id
            ));
        }

        if !child_exists {
            return Err(anyhow::anyhow!(
                "Child ticket '{}' does not exist",
                child_ticket_id
            ));
        }

        // Critical: Check for cycle before creating dependency
        if Self::would_create_cycle(pool, parent_ticket_id, child_ticket_id).await? {
            let cycle_path = Self::find_cycle_path(pool, parent_ticket_id, child_ticket_id).await?;
            return Err(anyhow::anyhow!(
                "Adding dependency from '{}' to '{}' would create a cycle: {}",
                parent_ticket_id,
                child_ticket_id,
                cycle_path.join(" -> ")
            ));
        }

        let dependency = sqlx::query_as::<_, TicketDependency>(
            r#"
            INSERT INTO ticket_dependencies (parent_ticket_id, child_ticket_id, dependency_type)
            VALUES (?1, ?2, ?3)
            RETURNING parent_ticket_id, child_ticket_id, dependency_type, created_at
        "#,
        )
        .bind(parent_ticket_id)
        .bind(child_ticket_id)
        .bind(dependency_type)
        .fetch_one(pool)
        .await?;

        debug!(
            "Created dependency: {} -> {} (type: {})",
            parent_ticket_id, child_ticket_id, dependency_type
        );

        Ok(dependency)
    }

    /// Remove a dependency relationship
    pub async fn remove(
        pool: &DbPool,
        parent_ticket_id: &str,
        child_ticket_id: &str,
    ) -> Result<()> {
        let rows_affected = sqlx::query(
            "DELETE FROM ticket_dependencies WHERE parent_ticket_id = ?1 AND child_ticket_id = ?2",
        )
        .bind(parent_ticket_id)
        .bind(child_ticket_id)
        .execute(pool)
        .await?
        .rows_affected();

        if rows_affected == 0 {
            return Err(anyhow::anyhow!(
                "Dependency from '{}' to '{}' does not exist",
                parent_ticket_id,
                child_ticket_id
            ));
        }

        debug!(
            "Removed dependency: {} -> {}",
            parent_ticket_id, child_ticket_id
        );

        Ok(())
    }

    /// Get all dependencies for a ticket (both as parent and child)
    pub async fn get_for_ticket(pool: &DbPool, ticket_id: &str) -> Result<Vec<TicketDependency>> {
        let dependencies = sqlx::query_as::<_, TicketDependency>(
            r#"
            SELECT parent_ticket_id, child_ticket_id, dependency_type, created_at
            FROM ticket_dependencies
            WHERE parent_ticket_id = ?1 OR child_ticket_id = ?1
            ORDER BY created_at DESC
        "#,
        )
        .bind(ticket_id)
        .fetch_all(pool)
        .await?;

        Ok(dependencies)
    }

    /// Get all direct children of a ticket
    pub async fn get_children(pool: &DbPool, parent_ticket_id: &str) -> Result<Vec<String>> {
        let children = sqlx::query_as::<_, (String,)>(
            "SELECT child_ticket_id FROM ticket_dependencies WHERE parent_ticket_id = ?1",
        )
        .bind(parent_ticket_id)
        .fetch_all(pool)
        .await?
        .into_iter()
        .map(|(id,)| id)
        .collect();

        Ok(children)
    }

    /// Get all direct parents of a ticket
    pub async fn get_parents(pool: &DbPool, child_ticket_id: &str) -> Result<Vec<String>> {
        let parents = sqlx::query_as::<_, (String,)>(
            "SELECT parent_ticket_id FROM ticket_dependencies WHERE child_ticket_id = ?1",
        )
        .bind(child_ticket_id)
        .fetch_all(pool)
        .await?
        .into_iter()
        .map(|(id,)| id)
        .collect();

        Ok(parents)
    }

    /// Build complete dependency graph for a project
    pub async fn build_project_graph(pool: &DbPool, project_id: &str) -> Result<DependencyGraph> {
        // Get all tickets in the project
        let tickets =
            sqlx::query_as::<_, (String,)>("SELECT ticket_id FROM tickets WHERE project_id = ?1")
                .bind(project_id)
                .fetch_all(pool)
                .await?
                .into_iter()
                .map(|(id,)| id)
                .collect::<Vec<_>>();

        // Get all dependencies within the project
        let edges = sqlx::query_as::<_, (String, String)>(
            r#"
            SELECT td.parent_ticket_id, td.child_ticket_id
            FROM ticket_dependencies td
            JOIN tickets tp ON td.parent_ticket_id = tp.ticket_id
            JOIN tickets tc ON td.child_ticket_id = tc.ticket_id
            WHERE tp.project_id = ?1 AND tc.project_id = ?1
        "#,
        )
        .bind(project_id)
        .fetch_all(pool)
        .await?;

        // Calculate levels using topological sort
        let levels = Self::calculate_dependency_levels(&tickets, &edges)?;

        Ok(DependencyGraph {
            nodes: tickets,
            edges,
            levels,
        })
    }

    /// Check if adding a dependency would create a cycle
    async fn would_create_cycle(
        pool: &DbPool,
        parent_ticket_id: &str,
        child_ticket_id: &str,
    ) -> Result<bool> {
        // Use DFS to check if there's already a path from child to parent
        let mut visited = HashSet::new();
        let mut stack = vec![child_ticket_id.to_string()];

        while let Some(current) = stack.pop() {
            if current == parent_ticket_id {
                return Ok(true); // Cycle detected
            }

            if visited.contains(&current) {
                continue;
            }

            visited.insert(current.clone());

            // Get all children of current ticket
            let children = Self::get_children(pool, &current).await?;
            stack.extend(children);
        }

        Ok(false)
    }

    /// Find the cycle path for error reporting
    async fn find_cycle_path(
        pool: &DbPool,
        parent_ticket_id: &str,
        child_ticket_id: &str,
    ) -> Result<Vec<String>> {
        let mut path = vec![child_ticket_id.to_string()];
        let mut current = child_ticket_id.to_string();

        // Follow the path until we find the parent or detect a cycle
        while let Ok(children) = Self::get_children(pool, &current).await {
            if children.is_empty() {
                break;
            }

            // Take the first child for simplicity (could be enhanced to find shortest path)
            current = children[0].clone();
            path.push(current.clone());

            if current == parent_ticket_id {
                break;
            }

            // Prevent infinite loops
            if path.len() > 100 {
                warn!("Cycle path search exceeded maximum depth");
                break;
            }
        }

        Ok(path)
    }

    /// Calculate dependency levels for topological ordering
    fn calculate_dependency_levels(
        nodes: &[String],
        edges: &[(String, String)],
    ) -> Result<HashMap<String, usize>> {
        let mut graph: HashMap<String, Vec<String>> = HashMap::new();
        let mut in_degree: HashMap<String, usize> = HashMap::new();

        // Initialize nodes
        for node in nodes {
            graph.insert(node.clone(), Vec::new());
            in_degree.insert(node.clone(), 0);
        }

        // Build adjacency list and calculate in-degrees
        for (parent, child) in edges {
            if let Some(children) = graph.get_mut(parent) {
                children.push(child.clone());
            }
            if let Some(degree) = in_degree.get_mut(child) {
                *degree += 1;
            }
        }

        // Topological sort to calculate levels
        let mut queue = VecDeque::new();
        let mut levels = HashMap::new();

        // Start with nodes that have no dependencies (level 0)
        for (node, &degree) in &in_degree {
            if degree == 0 {
                queue.push_back((node.clone(), 0));
                levels.insert(node.clone(), 0);
            }
        }

        while let Some((current, level)) = queue.pop_front() {
            if let Some(children) = graph.get(&current) {
                for child in children {
                    if let Some(degree) = in_degree.get_mut(child) {
                        *degree -= 1;
                        if *degree == 0 {
                            let child_level = level + 1;
                            queue.push_back((child.clone(), child_level));
                            levels.insert(child.clone(), child_level);
                        }
                    }
                }
            }
        }

        // Check if all nodes were processed (no cycles)
        if levels.len() != nodes.len() {
            return Err(anyhow::anyhow!(
                "Cycle detected in dependency graph - not all nodes could be leveled"
            ));
        }

        Ok(levels)
    }

    /// Check if a ticket exists
    async fn ticket_exists(pool: &DbPool, ticket_id: &str) -> Result<bool> {
        let exists =
            sqlx::query_as::<_, (i64,)>("SELECT 1 FROM tickets WHERE ticket_id = ?1 LIMIT 1")
                .bind(ticket_id)
                .fetch_optional(pool)
                .await?
                .is_some();

        Ok(exists)
    }

    /// Check if all dependencies of a ticket are satisfied (completed)
    pub async fn all_dependencies_satisfied(pool: &DbPool, ticket_id: &str) -> Result<bool> {
        let blocking_dependencies = sqlx::query_as::<_, (String,)>(
            r#"
            SELECT td.parent_ticket_id
            FROM ticket_dependencies td
            JOIN tickets t ON td.parent_ticket_id = t.ticket_id
            WHERE td.child_ticket_id = ?1
            AND td.dependency_type = 'blocks'
            AND t.state != 'closed'
        "#,
        )
        .bind(ticket_id)
        .fetch_all(pool)
        .await?;

        Ok(blocking_dependencies.is_empty())
    }

    /// Get all tickets that are blocked by a specific ticket
    pub async fn get_blocked_by(pool: &DbPool, blocking_ticket_id: &str) -> Result<Vec<String>> {
        let blocked_tickets = sqlx::query_as::<_, (String,)>(
            r#"
            SELECT child_ticket_id
            FROM ticket_dependencies
            WHERE parent_ticket_id = ?1 AND dependency_type = 'blocks'
        "#,
        )
        .bind(blocking_ticket_id)
        .fetch_all(pool)
        .await?
        .into_iter()
        .map(|(id,)| id)
        .collect();

        Ok(blocked_tickets)
    }
}
