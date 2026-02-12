use anyhow::{Context, Result};

#[derive(Debug, Clone)]
pub struct ProxyConfig {
    pub supabase_url: String,
    pub supabase_service_key: String,
    pub hmac_secret: String,
    pub port: u16,
    pub max_trades_per_agent_per_day: u32,
    pub max_requests_per_ip_per_minute: u32,
    pub aggregate_interval_secs: u64,
}

impl ProxyConfig {
    pub fn from_env() -> Result<Self> {
        Ok(Self {
            supabase_url: std::env::var("SUPABASE_URL")
                .context("SUPABASE_URL is required")?,
            supabase_service_key: std::env::var("SUPABASE_SERVICE_KEY")
                .context("SUPABASE_SERVICE_KEY is required")?,
            hmac_secret: std::env::var("HMAC_SECRET")
                .context("HMAC_SECRET is required")?,
            port: std::env::var("PORT")
                .unwrap_or_else(|_| "8080".to_string())
                .parse()
                .context("PORT must be a valid u16")?,
            max_trades_per_agent_per_day: std::env::var("MAX_TRADES_PER_AGENT_PER_DAY")
                .unwrap_or_else(|_| "50".to_string())
                .parse()
                .context("MAX_TRADES_PER_AGENT_PER_DAY must be a valid u32")?,
            max_requests_per_ip_per_minute: std::env::var("MAX_REQUESTS_PER_IP_PER_MINUTE")
                .unwrap_or_else(|_| "60".to_string())
                .parse()
                .context("MAX_REQUESTS_PER_IP_PER_MINUTE must be a valid u32")?,
            aggregate_interval_secs: std::env::var("AGGREGATE_INTERVAL_SECS")
                .unwrap_or_else(|_| "3600".to_string())
                .parse()
                .context("AGGREGATE_INTERVAL_SECS must be a valid u64")?,
        })
    }
}
