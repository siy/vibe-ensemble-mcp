use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Json},
};

use crate::{database::tickets::Ticket, error::AppError, server::AppState};

/// GET /api/projects/:project_id/tickets - List all tickets for a project
pub async fn list_tickets(
    State(state): State<AppState>,
    Path(project_id): Path<String>,
) -> Result<impl IntoResponse, AppError> {
    // list_by_project expects (project_id: Option<&str>, status_filter: Option<&str>)
    let tickets = Ticket::list_by_project(&state.db, Some(&project_id), None).await?;

    Ok((StatusCode::OK, Json(tickets)))
}

/// GET /api/projects/:project_id/tickets/:ticket_id - Get specific ticket with comments
pub async fn get_ticket_with_comments(
    State(state): State<AppState>,
    Path((project_id, ticket_id)): Path<(String, String)>,
) -> Result<impl IntoResponse, AppError> {
    let ticket_with_comments = Ticket::get_by_id(&state.db, &ticket_id).await?;

    match ticket_with_comments {
        Some(t) => {
            // Verify ticket belongs to the specified project
            if t.ticket.project_id != project_id {
                return Err(AppError::NotFound(format!(
                    "Ticket '{}' not found in project '{}'",
                    ticket_id, project_id
                )));
            }
            Ok((StatusCode::OK, Json(t)))
        }
        None => Err(AppError::NotFound(format!(
            "Ticket '{}' not found",
            ticket_id
        ))),
    }
}
