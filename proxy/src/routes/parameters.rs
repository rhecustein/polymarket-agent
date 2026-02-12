use axum::extract::State;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::Json;
use serde_json::json;
use tracing::warn;

use crate::AppState;

/// GET /api/parameters
///
/// Returns recommended configuration parameters derived from community data.
/// Agents can use these to auto-tune their settings.
pub async fn get_parameters(State(state): State<AppState>) -> impl IntoResponse {
    match state.supabase.get_recommended_params().await {
        Ok(params) => (StatusCode::OK, Json(params)),
        Err(e) => {
            warn!("Failed to fetch parameters: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": "Failed to fetch parameters" })),
            )
        }
    }
}
