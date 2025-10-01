use anyhow::Result;

/// Generate a project prefix from repository name
/// Examples:
/// - "todo-vue-rust" → "TVR"
/// - "vibe-ensemble-mcp" → "VEM"
/// - "my-awesome-project" → "MAP"
pub fn generate_project_prefix(repo_name: &str) -> String {
    repo_name
        .split('-')
        .filter_map(|word| word.chars().next())
        .map(|c| c.to_uppercase().to_string())
        .collect::<Vec<_>>()
        .join("")
        .chars()
        .take(3) // Max 3 letters
        .collect()
}

/// Infer subsystem from stage names in execution plan
/// Priority: first matching stage name determines subsystem
pub fn infer_subsystem_from_stages(execution_plan: &[String]) -> String {
    for stage in execution_plan {
        let stage_lower = stage.to_lowercase();

        if stage_lower.contains("frontend") || stage_lower.contains("ui") || stage_lower.contains("client") {
            return "FE".to_string();
        } else if stage_lower.contains("backend") || stage_lower.contains("api") || stage_lower.contains("server") {
            return "BE".to_string();
        } else if stage_lower.contains("database") || stage_lower.contains("db") || stage_lower.contains("schema") {
            return "DB".to_string();
        } else if stage_lower.contains("test") {
            return "TEST".to_string();
        } else if stage_lower.contains("deploy") || stage_lower.contains("ops") || stage_lower.contains("infra") {
            return "OPS".to_string();
        } else if stage_lower.contains("doc") {
            return "DOC".to_string();
        } else if stage_lower.contains("design") {
            return "DESIGN".to_string();
        }
    }

    // Default fallback
    "CORE".to_string()
}

/// Get next ticket number for a given project and subsystem (pool version)
pub async fn get_next_ticket_number(
    db: &crate::database::DbPool,
    project_id: &str,
    subsystem: &str,
) -> Result<u32> {
    let query = sqlx::query_scalar::<_, i64>(
        r#"
        SELECT COALESCE(MAX(CAST(SUBSTR(ticket_id, INSTR(ticket_id, '-', INSTR(ticket_id, '-') + 1) + 1) AS INTEGER)), 0) + 1
        FROM tickets
        WHERE project_id = ?1 AND ticket_id LIKE ?2
        "#,
    )
    .bind(project_id)
    .bind(format!("%-{subsystem}-%"));

    let next_num = query.fetch_one(db).await?;
    Ok(next_num as u32)
}

/// Get next ticket number for a given project and subsystem (transaction version)
pub async fn get_next_ticket_number_tx(
    tx: &mut sqlx::SqliteConnection,
    project_id: &str,
    subsystem: &str,
) -> Result<u32> {
    let query = sqlx::query_scalar::<_, i64>(
        r#"
        SELECT COALESCE(MAX(CAST(SUBSTR(ticket_id, INSTR(ticket_id, '-', INSTR(ticket_id, '-') + 1) + 1) AS INTEGER)), 0) + 1
        FROM tickets
        WHERE project_id = ?1 AND ticket_id LIKE ?2
        "#,
    )
    .bind(project_id)
    .bind(format!("%-{subsystem}-%"));

    let next_num = query.fetch_one(&mut *tx).await?;
    Ok(next_num as u32)
}

/// Generate a human-friendly ticket ID (pool version)
/// Format: {PROJECT_PREFIX}-{SUBSYSTEM}-{NUMBER}
/// Example: TVR-FE-001, VEM-BE-042
pub async fn generate_ticket_id(
    db: &crate::database::DbPool,
    project_prefix: &str,
    subsystem: &str,
) -> Result<String> {
    // Get next number for this subsystem
    let next_num = get_next_ticket_number(db, project_prefix, subsystem).await?;
    Ok(format!(
        "{}-{}-{:03}",
        project_prefix.to_uppercase(),
        subsystem.to_uppercase(),
        next_num
    ))
}

/// Generate a human-friendly ticket ID (transaction version)
/// Format: {PROJECT_PREFIX}-{SUBSYSTEM}-{NUMBER}
/// Example: TVR-FE-001, VEM-BE-042
pub async fn generate_ticket_id_tx(
    tx: &mut sqlx::SqliteConnection,
    project_prefix: &str,
    subsystem: &str,
) -> Result<String> {
    // Get next number for this subsystem
    let next_num = get_next_ticket_number_tx(tx, project_prefix, subsystem).await?;
    Ok(format!(
        "{}-{}-{:03}",
        project_prefix.to_uppercase(),
        subsystem.to_uppercase(),
        next_num
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_project_prefix() {
        assert_eq!(generate_project_prefix("todo-vue-rust"), "TVR");
        assert_eq!(generate_project_prefix("vibe-ensemble-mcp"), "VEM");
        assert_eq!(generate_project_prefix("my-awesome-project"), "MAP");
        assert_eq!(generate_project_prefix("single"), "S");
        assert_eq!(generate_project_prefix("a-b-c-d"), "ABC"); // Max 3
    }

    #[test]
    fn test_infer_subsystem_from_stages() {
        assert_eq!(
            infer_subsystem_from_stages(&["frontend_implementation".to_string()]),
            "FE"
        );
        assert_eq!(
            infer_subsystem_from_stages(&["backend_api_development".to_string()]),
            "BE"
        );
        assert_eq!(
            infer_subsystem_from_stages(&["database_schema_design".to_string()]),
            "DB"
        );
        assert_eq!(
            infer_subsystem_from_stages(&["integration_testing".to_string()]),
            "TEST"
        );
        assert_eq!(
            infer_subsystem_from_stages(&["deployment_staging".to_string()]),
            "OPS"
        );
        assert_eq!(
            infer_subsystem_from_stages(&["documentation_writing".to_string()]),
            "DOC"
        );
        assert_eq!(
            infer_subsystem_from_stages(&["unknown_stage".to_string()]),
            "CORE"
        );

        // Test priority: first match wins
        assert_eq!(
            infer_subsystem_from_stages(&["frontend_impl".to_string(), "backend_impl".to_string()]),
            "FE"
        );
    }
}
