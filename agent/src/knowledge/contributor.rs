use anyhow::Result;
use serde::Serialize;
use tracing::{info, warn};

#[derive(Serialize)]
pub struct TradeReport {
    pub agent_hash: String,
    pub category: String,
    pub trade_mode: String,
    pub direction: String,
    pub entry_edge_pct: f64,
    pub judge_confidence: f64,
    pub judge_model: String,
    pub result: String,
    pub pnl_pct: f64,
    pub hold_hours: f64,
    pub exit_reason: String,
    pub market_type: Option<String>,
    pub volume_bucket: Option<String>,
    pub specialist_desk: Option<String>,
    pub bull_confidence: Option<f64>,
    pub bear_confidence: Option<f64>,
    pub signature: String,
    pub agent_version: Option<String>,
}

pub struct KnowledgeContributor {
    client: reqwest::Client,
    agent_hash: String,
    enabled: bool,
}

impl KnowledgeContributor {
    pub fn new(enabled: bool) -> Self {
        let machine_id = get_machine_id();
        let agent_hash = sha256_hex(&format!("polyagent_{}", machine_id));

        Self {
            client: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(10))
                .build()
                .expect("HTTP client"),
            agent_hash,
            enabled,
        }
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    pub async fn contribute(&self, report: &TradeReport) -> Result<()> {
        if !self.enabled {
            return Ok(());
        }

        let url = format!("{}/api/contribute", super::PROXY_BASE_URL);

        match self.client.post(&url).json(report).send().await {
            Ok(resp) if resp.status().is_success() => {
                info!("Contributed trade to community knowledge base");
            }
            Ok(resp) => {
                let status = resp.status();
                warn!("Contribute failed ({})", status);
            }
            Err(e) => {
                warn!("Contribute offline: {}", e);
            }
        }

        Ok(())
    }

    pub fn agent_hash(&self) -> &str {
        &self.agent_hash
    }

    pub fn sign(&self, payload: &str) -> String {
        use hmac::{Hmac, Mac};
        use sha2::Sha256;
        type HmacSha256 = Hmac<Sha256>;

        let secret = super::get_hmac_secret();
        let mut mac = HmacSha256::new_from_slice(secret.as_bytes())
            .expect("HMAC key error");
        mac.update(payload.as_bytes());
        hex::encode(mac.finalize().into_bytes())
    }
}

fn get_machine_id() -> String {
    let hostname = hostname::get()
        .map(|h| h.to_string_lossy().to_string())
        .unwrap_or_else(|_| "unknown".to_string());
    hostname
}

fn sha256_hex(input: &str) -> String {
    use sha2::{Sha256, Digest};
    let mut hasher = Sha256::new();
    hasher.update(input.as_bytes());
    hex::encode(hasher.finalize())
}
