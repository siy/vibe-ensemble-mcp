//! Authentication handlers for the web interface

use crate::{Error, Result};
use axum::{
    extract::{Query, State},
    http::StatusCode,
    response::{Html, IntoResponse, Redirect, Response},
    Form, Json,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use vibe_ensemble_security::{
    AuthService, CreateUserRequest, LoginRequest, LoginResponse, 
    RefreshTokenRequest, ChangePasswordRequest, UserInfo, 
    UserRole, Permission,
};

/// Authentication handlers state
pub struct AuthHandlers {
    pub auth_service: Arc<AuthService>,
}

/// Login page query parameters
#[derive(Deserialize)]
pub struct LoginQuery {
    redirect: Option<String>,
    error: Option<String>,
}

/// User registration form
#[derive(Deserialize)]
pub struct RegisterForm {
    username: String,
    email: Option<String>,
    password: String,
    confirm_password: String,
}

/// Password change form
#[derive(Deserialize)]
pub struct ChangePasswordForm {
    current_password: String,
    new_password: String,
    confirm_password: String,
}

impl AuthHandlers {
    pub fn new(auth_service: Arc<AuthService>) -> Self {
        Self { auth_service }
    }

    /// Show login page
    pub async fn login_page(Query(query): Query<LoginQuery>) -> Html<String> {
        let error_message = query.error.unwrap_or_default();
        let redirect_url = query.redirect.unwrap_or_else(|| "/dashboard".to_string());
        
        let html = format!(
            r#"
<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>Login - Vibe Ensemble</title>
    <style>
        body {{ 
            font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, "Helvetica Neue", Arial, sans-serif;
            background: linear-gradient(135deg, #667eea 0%, #764ba2 100%);
            margin: 0;
            padding: 0;
            min-height: 100vh;
            display: flex;
            align-items: center;
            justify-content: center;
        }}
        
        .login-container {{ 
            background: white; 
            padding: 3rem; 
            border-radius: 12px; 
            box-shadow: 0 10px 25px rgba(0, 0, 0, 0.1); 
            width: 100%;
            max-width: 400px;
        }}
        
        .logo {{
            text-align: center;
            margin-bottom: 2rem;
        }}
        
        .logo h1 {{
            color: #333;
            font-size: 2rem;
            margin: 0;
            font-weight: 300;
        }}
        
        .logo p {{
            color: #666;
            margin: 0.5rem 0 0 0;
            font-size: 0.9rem;
        }}
        
        .form-group {{ 
            margin-bottom: 1.5rem; 
        }}
        
        .form-group label {{ 
            display: block; 
            margin-bottom: 0.5rem; 
            color: #333;
            font-weight: 500;
        }}
        
        .form-group input {{ 
            width: 100%; 
            padding: 0.875rem; 
            border: 2px solid #e1e5e9; 
            border-radius: 6px; 
            box-sizing: border-box;
            font-size: 1rem;
            transition: border-color 0.2s ease;
        }}
        
        .form-group input:focus {{
            outline: none;
            border-color: #667eea;
        }}
        
        .btn {{ 
            width: 100%; 
            padding: 0.875rem; 
            background: linear-gradient(135deg, #667eea 0%, #764ba2 100%);
            color: white; 
            border: none; 
            border-radius: 6px; 
            cursor: pointer;
            font-size: 1rem;
            font-weight: 500;
            transition: transform 0.2s ease;
        }}
        
        .btn:hover {{ 
            transform: translateY(-1px);
        }}
        
        .btn:active {{
            transform: translateY(0);
        }}
        
        .error {{ 
            background-color: #fef2f2;
            border: 1px solid #fecaca;
            color: #dc2626; 
            text-align: center; 
            margin-bottom: 1.5rem;
            padding: 0.75rem;
            border-radius: 6px;
            font-size: 0.9rem;
        }}
        
        .demo-info {{ 
            background-color: #f0f9ff;
            border: 1px solid #bae6fd;
            color: #0369a1;
            padding: 1rem; 
            border-radius: 6px; 
            margin-bottom: 1.5rem; 
            font-size: 0.9rem; 
        }}
        
        .demo-info strong {{
            display: block;
            margin-bottom: 0.5rem;
        }}
        
        .links {{
            text-align: center;
            margin-top: 1.5rem;
        }}
        
        .links a {{
            color: #667eea;
            text-decoration: none;
            font-size: 0.9rem;
        }}
        
        .links a:hover {{
            text-decoration: underline;
        }}
    </style>
</head>
<body>
    <div class="login-container">
        <div class="logo">
            <h1>Vibe Ensemble</h1>
            <p>Team Coordination Platform</p>
        </div>
        
        {}
        
        <div class="demo-info">
            <strong>Default Admin Account:</strong>
            Username: admin<br>
            Password: admin<br>
            <small>Please change the password after first login</small>
        </div>
        
        <form method="post" action="/auth/login">
            <input type="hidden" name="redirect" value="{}">
            
            <div class="form-group">
                <label for="username">Username:</label>
                <input type="text" id="username" name="username" required autocomplete="username">
            </div>
            
            <div class="form-group">
                <label for="password">Password:</label>
                <input type="password" id="password" name="password" required autocomplete="current-password">
            </div>
            
            <button type="submit" class="btn">Sign In</button>
        </form>
        
        <div class="links">
            <a href="/auth/register">Don't have an account? Register here</a>
        </div>
    </div>
</body>
</html>
            "#,
            if error_message.is_empty() { 
                String::new() 
            } else { 
                format!(r#"<div class="error">{}</div>"#, error_message) 
            },
            redirect_url
        );

        Html(html)
    }

    /// Handle login form submission
    pub async fn login_post(
        State(handlers): State<Arc<AuthHandlers>>,
        Form(login_data): Form<LoginRequest>,
    ) -> Result<Response> {
        match handlers.auth_service.authenticate(&login_data.username, &login_data.password, None).await {
            Ok(token_pair) => {
                // Set secure HTTP-only cookie with JWT token
                let cookie = format!(
                    "session_token={}; Path=/; HttpOnly; SameSite=Strict; Max-Age={}",
                    token_pair.access_token,
                    token_pair.expires_in
                );

                let mut response = Redirect::to("/dashboard").into_response();
                response.headers_mut().insert(
                    axum::http::header::SET_COOKIE,
                    cookie.parse().unwrap(),
                );
                
                Ok(response)
            }
            Err(e) => {
                let error_msg = match e {
                    vibe_ensemble_security::SecurityError::AuthenticationFailed(msg) => msg,
                    _ => "Authentication failed".to_string(),
                };
                
                Ok(Redirect::to(&format!("/auth/login?error={}", urlencoding::encode(&error_msg))).into_response())
            }
        }
    }

    /// API login endpoint
    pub async fn api_login(
        State(handlers): State<Arc<AuthHandlers>>,
        Json(login_data): Json<LoginRequest>,
    ) -> Result<Json<LoginResponse>> {
        let token_pair = handlers.auth_service.authenticate(&login_data.username, &login_data.password, None).await
            .map_err(|e| Error::Authentication(e.to_string()))?;

        let user = handlers.auth_service.get_user_by_username(&login_data.username).await
            .map_err(|e| Error::Database(e.to_string()))?
            .ok_or_else(|| Error::Authentication("User not found".to_string()))?;

        let response = LoginResponse {
            access_token: token_pair.access_token,
            refresh_token: token_pair.refresh_token,
            token_type: token_pair.token_type,
            expires_in: token_pair.expires_in,
            user: UserInfo {
                id: user.id,
                username: user.username,
                email: user.email,
                role: user.role,
                permissions: user.role.permissions(),
            },
        };

        Ok(Json(response))
    }

    /// Handle logout
    pub async fn logout() -> Redirect {
        let mut response = Redirect::to("/auth/login");
        // Clear the session cookie
        // Note: This is simplified - in production you'd also invalidate the token
        Redirect::to("/auth/login")
    }

    /// Refresh token endpoint
    pub async fn refresh_token(
        State(handlers): State<Arc<AuthHandlers>>,
        Json(refresh_data): Json<RefreshTokenRequest>,
    ) -> Result<Json<LoginResponse>> {
        let token_pair = handlers.auth_service.refresh_token(&refresh_data.refresh_token).await
            .map_err(|e| Error::Authentication(e.to_string()))?;

        // Get user info from the new token
        let claims = handlers.auth_service.jwt_manager().validate_access_token(&token_pair.access_token)
            .map_err(|e| Error::Authentication(e.to_string()))?;

        let user = handlers.auth_service.get_user_by_id(&claims.sub).await
            .map_err(|e| Error::Database(e.to_string()))?
            .ok_or_else(|| Error::Authentication("User not found".to_string()))?;

        let response = LoginResponse {
            access_token: token_pair.access_token,
            refresh_token: token_pair.refresh_token,
            token_type: token_pair.token_type,
            expires_in: token_pair.expires_in,
            user: UserInfo {
                id: user.id,
                username: user.username,
                email: user.email,
                role: user.role,
                permissions: user.role.permissions(),
            },
        };

        Ok(Json(response))
    }

    /// Registration page
    pub async fn register_page() -> Html<String> {
        let html = r#"
<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>Register - Vibe Ensemble</title>
    <style>
        body { 
            font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, "Helvetica Neue", Arial, sans-serif;
            background: linear-gradient(135deg, #667eea 0%, #764ba2 100%);
            margin: 0;
            padding: 0;
            min-height: 100vh;
            display: flex;
            align-items: center;
            justify-content: center;
        }
        .register-container { 
            background: white; 
            padding: 3rem; 
            border-radius: 12px; 
            box-shadow: 0 10px 25px rgba(0, 0, 0, 0.1); 
            width: 100%;
            max-width: 400px;
        }
        .logo { text-align: center; margin-bottom: 2rem; }
        .logo h1 { color: #333; font-size: 2rem; margin: 0; font-weight: 300; }
        .logo p { color: #666; margin: 0.5rem 0 0 0; font-size: 0.9rem; }
        .form-group { margin-bottom: 1.5rem; }
        .form-group label { display: block; margin-bottom: 0.5rem; color: #333; font-weight: 500; }
        .form-group input, .form-group select { 
            width: 100%; 
            padding: 0.875rem; 
            border: 2px solid #e1e5e9; 
            border-radius: 6px; 
            box-sizing: border-box;
            font-size: 1rem;
            transition: border-color 0.2s ease;
        }
        .form-group input:focus, .form-group select:focus {
            outline: none;
            border-color: #667eea;
        }
        .btn { 
            width: 100%; 
            padding: 0.875rem; 
            background: linear-gradient(135deg, #667eea 0%, #764ba2 100%);
            color: white; 
            border: none; 
            border-radius: 6px; 
            cursor: pointer;
            font-size: 1rem;
            font-weight: 500;
            transition: transform 0.2s ease;
        }
        .btn:hover { transform: translateY(-1px); }
        .btn:active { transform: translateY(0); }
        .links { text-align: center; margin-top: 1.5rem; }
        .links a { color: #667eea; text-decoration: none; font-size: 0.9rem; }
        .links a:hover { text-decoration: underline; }
        .info {
            background-color: #fef3c7;
            border: 1px solid #f59e0b;
            color: #92400e;
            padding: 1rem;
            border-radius: 6px;
            margin-bottom: 1.5rem;
            font-size: 0.9rem;
        }
    </style>
</head>
<body>
    <div class="register-container">
        <div class="logo">
            <h1>Vibe Ensemble</h1>
            <p>Create Account</p>
        </div>
        
        <div class="info">
            <strong>Note:</strong> Registration creates an Agent account. For Coordinator or Admin privileges, contact your administrator.
        </div>
        
        <form method="post" action="/auth/register">
            <div class="form-group">
                <label for="username">Username:</label>
                <input type="text" id="username" name="username" required>
            </div>
            
            <div class="form-group">
                <label for="email">Email (optional):</label>
                <input type="email" id="email" name="email">
            </div>
            
            <div class="form-group">
                <label for="password">Password:</label>
                <input type="password" id="password" name="password" required>
            </div>
            
            <div class="form-group">
                <label for="confirm_password">Confirm Password:</label>
                <input type="password" id="confirm_password" name="confirm_password" required>
            </div>
            
            <button type="submit" class="btn">Create Account</button>
        </form>
        
        <div class="links">
            <a href="/auth/login">Already have an account? Sign in</a>
        </div>
    </div>
</body>
</html>
        "#;

        Html(html.to_string())
    }

    /// Handle user registration
    pub async fn register_post(
        State(handlers): State<Arc<AuthHandlers>>,
        Form(register_data): Form<RegisterForm>,
    ) -> Result<Response> {
        // Validate password confirmation
        if register_data.password != register_data.confirm_password {
            return Ok(Redirect::to("/auth/register?error=Passwords do not match").into_response());
        }

        // Create user with Agent role (default for self-registration)
        let create_request = CreateUserRequest {
            username: register_data.username,
            email: register_data.email,
            password: register_data.password,
            role: UserRole::Agent,
        };

        match handlers.auth_service.create_user(
            &create_request.username,
            create_request.email.as_deref(),
            &create_request.password,
            create_request.role,
            "self_registration", // Special marker for self-registration
        ).await {
            Ok(_user) => {
                Ok(Redirect::to("/auth/login?success=Account created successfully. Please sign in.").into_response())
            }
            Err(e) => {
                let error_msg = match e {
                    vibe_ensemble_security::SecurityError::AuthenticationFailed(msg) => msg,
                    _ => "Registration failed".to_string(),
                };
                
                Ok(Redirect::to(&format!("/auth/register?error={}", urlencoding::encode(&error_msg))).into_response())
            }
        }
    }

    /// Change password page
    pub async fn change_password_page() -> Html<String> {
        let html = r#"
<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>Change Password - Vibe Ensemble</title>
    <style>
        /* Similar styling as login/register pages */
        body { 
            font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, "Helvetica Neue", Arial, sans-serif;
            background: linear-gradient(135deg, #667eea 0%, #764ba2 100%);
            margin: 0;
            padding: 0;
            min-height: 100vh;
            display: flex;
            align-items: center;
            justify-content: center;
        }
        .password-container { 
            background: white; 
            padding: 3rem; 
            border-radius: 12px; 
            box-shadow: 0 10px 25px rgba(0, 0, 0, 0.1); 
            width: 100%;
            max-width: 400px;
        }
        .logo { text-align: center; margin-bottom: 2rem; }
        .logo h1 { color: #333; font-size: 2rem; margin: 0; font-weight: 300; }
        .logo p { color: #666; margin: 0.5rem 0 0 0; font-size: 0.9rem; }
        .form-group { margin-bottom: 1.5rem; }
        .form-group label { display: block; margin-bottom: 0.5rem; color: #333; font-weight: 500; }
        .form-group input { 
            width: 100%; 
            padding: 0.875rem; 
            border: 2px solid #e1e5e9; 
            border-radius: 6px; 
            box-sizing: border-box;
            font-size: 1rem;
            transition: border-color 0.2s ease;
        }
        .form-group input:focus {
            outline: none;
            border-color: #667eea;
        }
        .btn { 
            width: 100%; 
            padding: 0.875rem; 
            background: linear-gradient(135deg, #667eea 0%, #764ba2 100%);
            color: white; 
            border: none; 
            border-radius: 6px; 
            cursor: pointer;
            font-size: 1rem;
            font-weight: 500;
            transition: transform 0.2s ease;
        }
        .btn:hover { transform: translateY(-1px); }
        .btn:active { transform: translateY(0); }
        .links { text-align: center; margin-top: 1.5rem; }
        .links a { color: #667eea; text-decoration: none; font-size: 0.9rem; }
        .links a:hover { text-decoration: underline; }
    </style>
</head>
<body>
    <div class="password-container">
        <div class="logo">
            <h1>Change Password</h1>
            <p>Update your account security</p>
        </div>
        
        <form method="post" action="/auth/change-password">
            <div class="form-group">
                <label for="current_password">Current Password:</label>
                <input type="password" id="current_password" name="current_password" required>
            </div>
            
            <div class="form-group">
                <label for="new_password">New Password:</label>
                <input type="password" id="new_password" name="new_password" required>
            </div>
            
            <div class="form-group">
                <label for="confirm_password">Confirm New Password:</label>
                <input type="password" id="confirm_password" name="confirm_password" required>
            </div>
            
            <button type="submit" class="btn">Update Password</button>
        </form>
        
        <div class="links">
            <a href="/dashboard">Back to Dashboard</a>
        </div>
    </div>
</body>
</html>
        "#;

        Html(html.to_string())
    }

    /// Handle password change
    pub async fn change_password_post(
        State(handlers): State<Arc<AuthHandlers>>,
        // TODO: Extract user from session/JWT
        Form(password_data): Form<ChangePasswordForm>,
    ) -> Result<Response> {
        // Validate password confirmation
        if password_data.new_password != password_data.confirm_password {
            return Ok(Redirect::to("/auth/change-password?error=New passwords do not match").into_response());
        }

        // TODO: Get current user ID from session/JWT
        let user_id = "current_user_id"; // This should come from authentication middleware
        
        match handlers.auth_service.change_password(
            user_id,
            &password_data.current_password,
            &password_data.new_password,
        ).await {
            Ok(_) => {
                Ok(Redirect::to("/dashboard?success=Password changed successfully").into_response())
            }
            Err(e) => {
                let error_msg = match e {
                    vibe_ensemble_security::SecurityError::AuthenticationFailed(msg) => msg,
                    _ => "Password change failed".to_string(),
                };
                
                Ok(Redirect::to(&format!("/auth/change-password?error={}", urlencoding::encode(&error_msg))).into_response())
            }
        }
    }
}

/// Utility function for URL encoding
mod urlencoding {
    pub fn encode(s: &str) -> String {
        s.chars()
            .map(|c| match c {
                'A'..='Z' | 'a'..='z' | '0'..='9' | '-' | '_' | '.' | '~' => c.to_string(),
                _ => format!("%{:02X}", c as u8),
            })
            .collect()
    }
}