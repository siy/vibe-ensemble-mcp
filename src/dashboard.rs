use axum::{
    body::Body,
    http::{header, StatusCode, Uri},
    response::{IntoResponse, Response},
};
use rust_embed::RustEmbed;

#[derive(RustEmbed)]
#[folder = "dashboard/dist"]
struct DashboardAssets;

/// Serve the dashboard SPA with proper fallback to index.html for client-side routing
pub async fn serve_dashboard(uri: Uri) -> impl IntoResponse {
    let path = uri.path().trim_start_matches('/');

    // Determine the actual file path to serve
    let file_path = if let Some(stripped) = path.strip_prefix("dashboard/") {
        stripped.to_string()
    } else if path == "dashboard" || path.is_empty() {
        "index.html".to_string()
    } else {
        // For /assets/* routes, keep the full path
        path.to_string()
    };

    // Try to serve the requested file
    if let Some(content) = DashboardAssets::get(&file_path) {
        return serve_file(&file_path, content.data.into());
    }

    // Try index.html for SPA routing (all unknown routes serve index.html)
    if let Some(content) = DashboardAssets::get("index.html") {
        return serve_file("index.html", content.data.into());
    }

    // Fallback if dashboard is not built
    (
        StatusCode::NOT_FOUND,
        "Dashboard not found. Run 'cd dashboard && npm run build' to build the dashboard.",
    )
        .into_response()
}

fn serve_file(path: &str, data: Vec<u8>) -> Response {
    let mime_type = mime_guess::from_path(path)
        .first_or_octet_stream()
        .as_ref()
        .to_string();

    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, mime_type)
        .body(Body::from(data))
        .unwrap()
}
