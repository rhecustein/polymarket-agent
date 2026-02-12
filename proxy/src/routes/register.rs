use axum::extract::State;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::Json;
use serde::Deserialize;
use serde_json::json;
use tracing::{info, warn};

use crate::middleware::signature::verify_signature;
use crate::AppState;

#[derive(Deserialize)]
pub struct RegisterRequest {
    pub agent_hash: String,
    pub signature: String,
    pub agent_version: Option<String>,
}

/// POST /api/register
///
/// Register a new agent with the knowledge sharing network.
/// Requires a valid HMAC signature to prevent spam.
pub async fn register(
    State(state): State<AppState>,
    Json(req): Json<RegisterRequest>,
) -> impl IntoResponse {
    // Verify HMAC signature
    if !verify_signature(&state.hmac_secret, &req.agent_hash, &req.signature) {
        warn!(
            "Invalid registration signature from {}...",
            &req.agent_hash.get(..8).unwrap_or("?")
        );
        return (
            StatusCode::UNAUTHORIZED,
            Json(json!({ "error": "Invalid signature" })),
        );
    }

    // Validate agent_hash length
    if req.agent_hash.len() < 16 {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": "agent_hash too short" })),
        );
    }

    match state.supabase.register_agent(&req.agent_hash).await {
        Ok(()) => {
            info!(
                "Agent registered: {}... (version: {})",
                &req.agent_hash.get(..8).unwrap_or("?"),
                req.agent_version.as_deref().unwrap_or("unknown")
            );
            (
                StatusCode::CREATED,
                Json(json!({
                    "status": "registered",
                    "agent_hash": req.agent_hash
                })),
            )
        }
        Err(e) => {
            warn!("Failed to register agent: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": "Failed to register agent" })),
            )
        }
    }
}
