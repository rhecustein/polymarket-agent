pub mod contributor;
pub mod consumer;
pub mod collector;

/// Proxy URL for knowledge sharing (public endpoint)
/// NOTE: Proxy features currently inactive - local-only mode
#[allow(dead_code)]
pub const PROXY_BASE_URL: &str = "https://polymarket-agent-proxy.fly.dev";

/// HMAC secret parts for signing trade reports
#[allow(dead_code)]
const HMAC_SECRET_PARTS: [&str; 4] = [
    "poly_", "agent_", "hmac_", "v1_secret_key_2026"
];

#[allow(dead_code)]
pub fn get_hmac_secret() -> String {
    HMAC_SECRET_PARTS.join("")
}
