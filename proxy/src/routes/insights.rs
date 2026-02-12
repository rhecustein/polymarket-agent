use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::Json;
use serde::Deserialize;
use serde_json::json;
use tracing::warn;

use crate::AppState;

#[derive(Deserialize)]
pub struct InsightsQuery {
    /// Time period for insights: "7d" (default), "30d", "90d"
    pub period: Option<String>,
}

/// GET /api/insights
///
/// Returns aggregated community insights for the given period.
/// No authentication required -- this is public data.
pub async fn get_insights(
    State(state): State<AppState>,
    Query(params): Query<InsightsQuery>,
) -> impl IntoResponse {
    let period = params.period.unwrap_or_else(|| "7d".to_string());

    // Validate period
    let valid_periods = ["7d", "30d", "90d"];
    if !valid_periods.contains(&period.as_str()) {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": "Invalid period. Use 7d, 30d, or 90d" })),
        );
    }

    match state.supabase.get_latest_insights(&period).await {
        Ok(insights) => (StatusCode::OK, Json(insights)),
        Err(e) => {
            warn!("Failed to fetch insights: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": "Failed to fetch insights" })),
            )
        }
    }
}

/// GET /api/stats
///
/// Returns high-level public statistics (no per-agent details).
pub async fn get_stats(State(state): State<AppState>) -> impl IntoResponse {
    match state.supabase.get_public_stats().await {
        Ok(stats) => (StatusCode::OK, Json(stats)),
        Err(e) => {
            warn!("Failed to fetch stats: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": "Failed to fetch stats" })),
            )
        }
    }
}

/// GET /api/golden-rules
///
/// Returns data-driven trading rules derived from community performance.
pub async fn get_golden_rules(State(state): State<AppState>) -> impl IntoResponse {
    match state.supabase.get_golden_rules().await {
        Ok(rules) => (StatusCode::OK, Json(rules)),
        Err(e) => {
            warn!("Failed to fetch golden rules: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": "Failed to fetch golden rules" })),
            )
        }
    }
}
