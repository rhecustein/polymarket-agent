pub mod contributor;
pub mod consumer;

/// Proxy URL for knowledge sharing (public endpoint)
pub const PROXY_BASE_URL: &str = "https://polymarket-agent-proxy.fly.dev";

/// HMAC secret parts for signing trade reports
const HMAC_SECRET_PARTS: [&str; 4] = [
    "poly_", "agent_", "hmac_", "v1_secret_key_2026"
];

pub fn get_hmac_secret() -> String {
    HMAC_SECRET_PARTS.join("")
}
