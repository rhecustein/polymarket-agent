mod aggregator;
mod config;
mod middleware;
mod routes;
mod supabase;

use std::net::SocketAddr;

use axum::routing::{get, post};
use axum::Router;
use tower_http::cors::{Any, CorsLayer};
use tracing::info;

use crate::config::ProxyConfig;
use crate::middleware::rate_limit::RateLimiter;
use crate::supabase::SupabaseClient;

/// Shared application state passed to all route handlers via Axum's State extractor.
#[derive(Clone)]
pub struct AppState {
    pub supabase: SupabaseClient,
    pub hmac_secret: String,
    pub rate_limiter: RateLimiter,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Load .env file if present (ignored in production where env vars are set externally)
    let _ = dotenvy::dotenv();

    // Initialize tracing (respects RUST_LOG env var)
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    // Load configuration
    let config = ProxyConfig::from_env()?;
    info!("Configuration loaded (port={})", config.port);

    // Build shared state
    let supabase = SupabaseClient::new(&config);
    let rate_limiter = RateLimiter::new(
        config.max_trades_per_agent_per_day,
        config.max_requests_per_ip_per_minute,
    );

    let state = AppState {
        supabase: supabase.clone(),
        hmac_secret: config.hmac_secret.clone(),
        rate_limiter: rate_limiter.clone(),
    };

    // CORS layer: allow requests from any origin (public API)
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    // Build router
    let app = Router::new()
        // Health check
        .route("/api/health", get(routes::health::health))
        // Public read endpoints (no auth required)
        .route("/api/insights", get(routes::insights::get_insights))
        .route("/api/stats", get(routes::insights::get_stats))
        .route("/api/golden-rules", get(routes::insights::get_golden_rules))
        .route("/api/parameters", get(routes::parameters::get_parameters))
        // Authenticated write endpoints (HMAC signature required)
        .route("/api/register", post(routes::register::register))
        .route("/api/contribute", post(routes::contribute::contribute))
        .route(
            "/api/contribute/batch",
            post(routes::contribute::contribute_batch),
        )
        .layer(cors)
        .with_state(state);

    // Spawn background aggregator task
    let agg_supabase = supabase.clone();
    let agg_limiter = rate_limiter.clone();
    let agg_interval = config.aggregate_interval_secs;
    tokio::spawn(async move {
        aggregator::run_aggregator(agg_supabase, agg_limiter, agg_interval).await;
    });

    // Start server
    let addr = SocketAddr::from(([0, 0, 0, 0], config.port));
    info!("Polymarket Agent Proxy v1.0.0 listening on {}", addr);
    info!("Routes:");
    info!("  GET  /api/health");
    info!("  GET  /api/insights?period=7d");
    info!("  GET  /api/stats");
    info!("  GET  /api/golden-rules");
    info!("  GET  /api/parameters");
    info!("  POST /api/register");
    info!("  POST /api/contribute");
    info!("  POST /api/contribute/batch");

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}
