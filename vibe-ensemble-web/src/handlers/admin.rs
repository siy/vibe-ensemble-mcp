use axum::{
    extract::State,
    response::{Html, IntoResponse, Response},
};
use vibe_ensemble_security::Session;
use vibe_ensemble_storage::StorageManager;

use crate::Result;

pub async fn admin_dashboard(
    session: Session,
    State(storage): State<StorageManager>,
) -> Result<Response> {
    if !session.is_admin {
        return Ok(Html("<h1>Access Denied</h1><p>Admin access required.</p>").into_response());
    }

    let _stats = storage.stats().await?;

    let html = r#"
<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <title>Admin Dashboard - Vibe Ensemble</title>
    <style>
        body { font-family: Arial, sans-serif; margin: 20px; }
        .card { background: white; padding: 20px; margin: 10px 0; border-radius: 8px; }
    </style>
</head>
<body>
    <h1>System Administration</h1>
    <div class="card">
        <h2>System Overview</h2>
        <p>Vibe Ensemble MCP Server - Admin Dashboard</p>
        <p>Status: Running</p>
    </div>
</body>
</html>"#;

    Ok(Html(html).into_response())
}

pub async fn admin_sessions(
    session: Session,
    State(_storage): State<StorageManager>,
) -> Result<Response> {
    if !session.is_admin {
        return Ok(Html("<h1>Access Denied</h1>").into_response());
    }

    let html = format!(
        r#"
<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <title>Sessions - Vibe Ensemble</title>
    <style>
        body {{ font-family: Arial, sans-serif; margin: 20px; }}
        .card {{ background: white; padding: 20px; margin: 10px 0; border-radius: 8px; }}
    </style>
</head>
<body>
    <h1>Active Sessions</h1>
    <div class="card">
        <h2>Current Session</h2>
        <p>Session ID: {}</p>
        <p>Username: {}</p>
        <p>Role: {}</p>
    </div>
</body>
</html>"#,
        session.id,
        session.username,
        if session.is_admin { "Admin" } else { "User" }
    );

    Ok(Html(html).into_response())
}
