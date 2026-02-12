use axum::Json;
use serde_json::{json, Value};

/// GET /api/health
///
/// Returns a simple status check with the server version.
pub async fn health() -> Json<Value> {
    Json(json!({
        "status": "ok",
        "version": "1.0.0"
    }))
}
