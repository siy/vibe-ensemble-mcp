pub mod projects;
pub mod tickets;

use axum::{routing::get, Router};

use crate::server::AppState;

/// Create the API router with all endpoint routes
pub fn create_api_router() -> Router<AppState> {
    Router::new()
        .route("/projects", get(projects::list_projects))
        .route("/projects/:project_id", get(projects::get_project))
        .route("/projects/:project_id/tickets", get(tickets::list_tickets))
        .route(
            "/projects/:project_id/tickets/:ticket_id",
            get(tickets::get_ticket_with_comments),
        )
}
