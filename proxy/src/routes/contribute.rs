use axum::extract::State;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::Json;
use serde::Deserialize;
use serde_json::{json, Value};
use tracing::{info, warn};

use crate::middleware::signature::verify_signature;
use crate::supabase::TradeReport;
use crate::AppState;

/// POST /api/contribute
///
/// Accepts a single trade report from an agent. Validates the HMAC signature,
/// checks rate limits, performs basic validation, then stores in Supabase.
pub async fn contribute(
    State(state): State<AppState>,
    Json(report): Json<TradeReport>,
) -> impl IntoResponse {
    // 1. Verify HMAC signature
    if !verify_signature(&state.hmac_secret, &report.agent_hash, &report.signature) {
        warn!("Invalid signature from agent_hash={}...", &report.agent_hash.get(..8).unwrap_or("?"));
        return (
            StatusCode::UNAUTHORIZED,
            Json(json!({ "error": "Invalid signature" })),
        );
    }

    // 2. Check agent daily rate limit
    if let Err(msg) = state.rate_limiter.check_agent_limit(&report.agent_hash) {
        warn!("Rate limited agent: {}", msg);
        return (
            StatusCode::TOO_MANY_REQUESTS,
            Json(json!({ "error": msg })),
        );
    }

    // 3. Basic validation
    if let Err(msg) = validate_report(&report) {
        warn!("Invalid report: {}", msg);
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": msg })),
        );
    }

    // 4. Insert into Supabase
    match state.supabase.insert_trade(&report).await {
        Ok(id) => {
            info!("Trade {} contributed by agent {}...", id, &report.agent_hash.get(..8).unwrap_or("?"));
            (
                StatusCode::CREATED,
                Json(json!({
                    "status": "accepted",
                    "trade_id": id
                })),
            )
        }
        Err(e) => {
            warn!("Failed to insert trade: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": "Failed to store trade" })),
            )
        }
    }
}

/// POST /api/contribute/batch
///
/// Accepts multiple trade reports in a single request.
/// Each report must have a valid HMAC signature.
#[derive(Deserialize)]
pub struct BatchRequest {
    pub trades: Vec<TradeReport>,
}

pub async fn contribute_batch(
    State(state): State<AppState>,
    Json(batch): Json<BatchRequest>,
) -> impl IntoResponse {
    if batch.trades.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": "No trades provided" })),
        );
    }

    if batch.trades.len() > 100 {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": "Maximum 100 trades per batch" })),
        );
    }

    // Validate all reports first
    let mut valid_reports: Vec<TradeReport> = Vec::new();
    let mut errors: Vec<Value> = Vec::new();

    for (i, report) in batch.trades.iter().enumerate() {
        // Verify signature
        if !verify_signature(&state.hmac_secret, &report.agent_hash, &report.signature) {
            errors.push(json!({
                "index": i,
                "error": "Invalid signature"
            }));
            continue;
        }

        // Check rate limit
        if let Err(msg) = state.rate_limiter.check_agent_limit(&report.agent_hash) {
            errors.push(json!({
                "index": i,
                "error": msg
            }));
            continue;
        }

        // Validate fields
        if let Err(msg) = validate_report(report) {
            errors.push(json!({
                "index": i,
                "error": msg
            }));
            continue;
        }

        valid_reports.push(report.clone());
    }

    if valid_reports.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({
                "error": "No valid trades in batch",
                "details": errors
            })),
        );
    }

    // Insert valid trades
    match state.supabase.insert_trades_batch(&valid_reports).await {
        Ok(count) => {
            info!("Batch: inserted {} trades ({} rejected)", count, errors.len());
            (
                StatusCode::CREATED,
                Json(json!({
                    "status": "accepted",
                    "inserted": count,
                    "rejected": errors.len(),
                    "errors": errors
                })),
            )
        }
        Err(e) => {
            warn!("Batch insert failed: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": "Failed to store trades" })),
            )
        }
    }
}

/// Validate a trade report's fields for sanity.
fn validate_report(report: &TradeReport) -> Result<(), String> {
    if report.agent_hash.len() < 16 {
        return Err("agent_hash too short".into());
    }

    if report.category.is_empty() {
        return Err("category is required".into());
    }

    let valid_modes = ["scalp", "swing", "conviction", "Scalp", "Swing", "Conviction", "SCALP", "SWING", "CONVICTION"];
    if !valid_modes.contains(&report.trade_mode.as_str()) {
        return Err(format!("Invalid trade_mode: {}", report.trade_mode));
    }

    let valid_directions = ["yes", "no", "YES", "NO", "Yes", "No"];
    if !valid_directions.contains(&report.direction.as_str()) {
        return Err(format!("Invalid direction: {}", report.direction));
    }

    let valid_results = ["win", "loss", "pending", "Win", "Loss", "Pending", "WIN", "LOSS", "PENDING"];
    if !valid_results.contains(&report.result.as_str()) {
        return Err(format!("Invalid result: {}", report.result));
    }

    if report.entry_edge_pct < -100.0 || report.entry_edge_pct > 100.0 {
        return Err("entry_edge_pct out of range [-100, 100]".into());
    }

    if report.judge_confidence < 0.0 || report.judge_confidence > 100.0 {
        return Err("judge_confidence out of range [0, 100]".into());
    }

    if report.hold_hours < 0.0 {
        return Err("hold_hours cannot be negative".into());
    }

    Ok(())
}
