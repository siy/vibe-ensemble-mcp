use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Json},
};

use crate::{database::projects::Project, error::AppError, server::AppState};

/// GET /api/projects - List all projects
pub async fn list_projects(State(state): State<AppState>) -> Result<impl IntoResponse, AppError> {
    let projects = Project::list_all(&state.db).await?;

    Ok((StatusCode::OK, Json(projects)))
}

/// GET /api/projects/:project_id - Get specific project by ID
pub async fn get_project(
    State(state): State<AppState>,
    Path(project_id): Path<String>,
) -> Result<impl IntoResponse, AppError> {
    let project = Project::get_by_id(&state.db, &project_id).await?;

    match project {
        Some(p) => Ok((StatusCode::OK, Json(p))),
        None => Err(AppError::NotFound(format!(
            "Project '{}' not found",
            project_id
        ))),
    }
}
