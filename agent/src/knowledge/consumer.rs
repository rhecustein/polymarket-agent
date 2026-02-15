//! Knowledge Consumer - Fetch community insights from proxy
//! NOTE: Currently inactive - Local-only knowledge collection is used instead

#![allow(dead_code)]

use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::Deserialize;
use std::collections::HashMap;
use tracing::{info, warn};

#[derive(Debug, Clone, Deserialize, Default)]
pub struct CommunityInsights {
    pub computed_at: Option<String>,
    pub period: Option<String>,
    pub total_trades: i64,
    pub total_agents: i64,
    pub overall_win_rate: f64,
    pub category_stats: HashMap<String, CategoryStats>,
    pub mode_stats: HashMap<String, ModeStats>,
    pub model_stats: HashMap<String, ModelStats>,
    pub recommended_config: Option<RecommendedConfig>,
    pub golden_rules: Vec<String>,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct CategoryStats {
    pub trades: i64,
    pub win_rate: f64,
    pub avg_edge: f64,
    pub avg_pnl_pct: f64,
    pub recommendation: String,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct ModeStats {
    pub trades: i64,
    pub win_rate: f64,
    pub avg_hold_hours: f64,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct ModelStats {
    pub trades: i64,
    pub win_rate: f64,
    pub avg_confidence: f64,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct RecommendedConfig {
    pub min_edge: f64,
    pub min_confidence: f64,
    pub kelly_fraction: f64,
    pub best_hours_utc: Vec<u8>,
    pub avoid_categories: Vec<String>,
    pub sonnet_top_n: u8,
}

pub struct KnowledgeConsumer {
    client: reqwest::Client,
    cached: Option<CommunityInsights>,
    last_fetch: Option<DateTime<Utc>>,
}

impl KnowledgeConsumer {
    pub fn new() -> Self {
        Self {
            client: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(10))
                .build()
                .expect("HTTP client"),
            cached: None,
            last_fetch: None,
        }
    }

    pub async fn fetch_insights(&mut self) -> Result<CommunityInsights> {
        // Cache for 1 hour
        if let (Some(cached), Some(last)) = (&self.cached, &self.last_fetch) {
            if (Utc::now() - *last).num_minutes() < 60 {
                return Ok(cached.clone());
            }
        }

        // Try proxy server
        let proxy_url = format!("{}/api/insights", super::PROXY_BASE_URL);
        if let Ok(resp) = self.client.get(&proxy_url).send().await {
            if resp.status().is_success() {
                if let Ok(insights) = resp.json::<CommunityInsights>().await {
                    info!("Community: {} agents, {} trades, {:.1}% WR",
                        insights.total_agents, insights.total_trades,
                        insights.overall_win_rate);
                    self.cached = Some(insights.clone());
                    self.last_fetch = Some(Utc::now());
                    return Ok(insights);
                }
            }
        }

        // Try GitHub fallback
        warn!("Proxy unavailable, trying GitHub cache...");
        let github_url = "https://raw.githubusercontent.com/bintangworks/polymarket-agent/main/knowledge/insights.json";
        if let Ok(resp) = self.client.get(github_url).send().await {
            if let Ok(insights) = resp.json::<CommunityInsights>().await {
                info!("Community (GitHub cache): {} trades", insights.total_trades);
                self.cached = Some(insights.clone());
                self.last_fetch = Some(Utc::now());
                return Ok(insights);
            }
        }

        // Return cached even if stale
        if let Some(cached) = &self.cached {
            warn!("Using stale cached insights");
            return Ok(cached.clone());
        }

        warn!("No community insights available, running solo");
        Ok(CommunityInsights::default())
    }
}
