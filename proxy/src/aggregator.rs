use std::time::Duration;
use tracing::{error, info};

use crate::middleware::rate_limit::RateLimiter;
use crate::supabase::SupabaseClient;

/// Background aggregator that periodically computes community insights
/// from raw trade data stored in Supabase.
///
/// Runs on an interval (default: 3600 seconds / 1 hour).
/// Also cleans up stale rate limiter entries to prevent memory growth.
pub async fn run_aggregator(
    supabase: SupabaseClient,
    rate_limiter: RateLimiter,
    interval_secs: u64,
) {
    let interval = Duration::from_secs(interval_secs);

    info!(
        "Aggregator started (interval: {}s / {:.1}h)",
        interval_secs,
        interval_secs as f64 / 3600.0
    );

    // Wait a short delay before the first run to let the server start up
    tokio::time::sleep(Duration::from_secs(10)).await;

    loop {
        info!("Running aggregation cycle...");

        // 1. Compute and store insights
        match supabase.compute_and_store_insights().await {
            Ok(()) => {
                info!("Aggregation cycle completed successfully");
            }
            Err(e) => {
                error!("Aggregation cycle failed: {}", e);
            }
        }

        // 2. Clean up stale rate limiter entries
        rate_limiter.cleanup_stale_entries();

        // 3. Sleep until next cycle
        tokio::time::sleep(interval).await;
    }
}
