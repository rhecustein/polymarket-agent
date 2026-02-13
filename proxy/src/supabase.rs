use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use tracing::{info, warn};

use crate::config::ProxyConfig;

/// A trade report as received from agents via the /api/contribute endpoint.
#[derive(Debug, Clone, Serialize, Deserialize)]
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

/// Supabase REST API client using the service role key.
#[derive(Clone)]
pub struct SupabaseClient {
    http: reqwest::Client,
    base_url: String,
    api_key: String,
}

impl SupabaseClient {
    pub fn new(config: &ProxyConfig) -> Self {
        let http = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(15))
            .build()
            .expect("Failed to build HTTP client");

        Self {
            http,
            base_url: config.supabase_url.trim_end_matches('/').to_string(),
            api_key: config.supabase_service_key.clone(),
        }
    }

    /// Build the REST URL for a given table.
    fn table_url(&self, table: &str) -> String {
        format!("{}/rest/v1/{}", self.base_url, table)
    }

    /// Common headers for Supabase REST calls.
    fn auth_headers(&self) -> Vec<(&str, String)> {
        vec![
            ("apikey", self.api_key.clone()),
            ("Authorization", format!("Bearer {}", self.api_key)),
        ]
    }

    /// Insert a trade report and return the inserted row ID.
    pub async fn insert_trade(&self, report: &TradeReport) -> Result<i64> {
        let url = self.table_url("trades");

        let body = serde_json::json!({
            "agent_hash": report.agent_hash,
            "category": report.category,
            "trade_mode": report.trade_mode,
            "direction": report.direction,
            "entry_edge_pct": report.entry_edge_pct,
            "judge_confidence": report.judge_confidence,
            "judge_model": report.judge_model,
            "result": report.result,
            "pnl_pct": report.pnl_pct,
            "hold_hours": report.hold_hours,
            "exit_reason": report.exit_reason,
            "market_type": report.market_type,
            "volume_bucket": report.volume_bucket,
            "specialist_desk": report.specialist_desk,
            "bull_confidence": report.bull_confidence,
            "bear_confidence": report.bear_confidence,
            "agent_version": report.agent_version,
        });

        let mut req = self.http
            .post(&url)
            .header("Content-Type", "application/json")
            .header("Prefer", "return=representation");

        for (key, val) in self.auth_headers() {
            req = req.header(key, val);
        }

        let resp = req.json(&body).send().await.context("Supabase insert_trade request")?;
        let status = resp.status();

        if !status.is_success() {
            let err_text = resp.text().await.unwrap_or_default();
            anyhow::bail!("Supabase insert_trade failed ({}): {}", status, err_text);
        }

        // Parse the returned row to extract the ID
        let rows: Vec<serde_json::Value> = resp.json().await.context("Parse insert response")?;
        let id = rows
            .first()
            .and_then(|r| r.get("id"))
            .and_then(|v| v.as_i64())
            .unwrap_or(0);

        info!("Inserted trade id={} from agent={}", id, &report.agent_hash[..8]);
        Ok(id)
    }

    /// Get the latest aggregated insights for a given period (e.g. "7d", "30d").
    pub async fn get_latest_insights(&self, period: &str) -> Result<serde_json::Value> {
        let url = format!(
            "{}?period=eq.{}&order=computed_at.desc&limit=1",
            self.table_url("insights"),
            period
        );

        let mut req = self.http.get(&url);
        for (key, val) in self.auth_headers() {
            req = req.header(key, val);
        }

        let resp = req.send().await.context("Supabase get_latest_insights")?;
        let status = resp.status();

        if !status.is_success() {
            let err_text = resp.text().await.unwrap_or_default();
            anyhow::bail!("Supabase get_insights failed ({}): {}", status, err_text);
        }

        let rows: Vec<serde_json::Value> = resp.json().await?;
        if let Some(row) = rows.into_iter().next() {
            // Return the data field which contains the full insights JSON
            if let Some(data) = row.get("data") {
                return Ok(data.clone());
            }
            return Ok(row);
        }

        // Return a sensible default if no insights exist yet
        Ok(serde_json::json!({
            "computed_at": null,
            "period": period,
            "total_trades": 0,
            "total_agents": 0,
            "overall_win_rate": 0.0,
            "category_stats": {},
            "mode_stats": {},
            "model_stats": {},
            "recommended_config": null,
            "golden_rules": []
        }))
    }

    /// Get public aggregate stats (no per-agent details).
    pub async fn get_public_stats(&self) -> Result<serde_json::Value> {
        let url = format!(
            "{}?select=id&order=id.desc&limit=1",
            self.table_url("trades")
        );

        let mut req = self.http.get(&url);
        for (key, val) in self.auth_headers() {
            req = req.header(key, val);
        }

        let resp = req.send().await.context("Supabase get_public_stats")?;
        let status = resp.status();

        if !status.is_success() {
            warn!("Supabase stats query failed ({})", status);
            return Ok(serde_json::json!({
                "total_trades": 0,
                "total_agents": 0,
                "uptime_hours": 0
            }));
        }

        // Get total trade count from the latest ID
        let rows: Vec<serde_json::Value> = resp.json().await?;
        let total_trades = rows
            .first()
            .and_then(|r| r.get("id"))
            .and_then(|v| v.as_i64())
            .unwrap_or(0);

        // Count distinct agents
        let agents_url = format!(
            "{}?select=agent_hash&order=agent_hash",
            self.table_url("trades")
        );

        let mut req2 = self.http.get(&agents_url)
            .header("Prefer", "count=exact");
        for (key, val) in self.auth_headers() {
            req2 = req2.header(key, val);
        }

        let agent_count = match req2.send().await {
            Ok(resp2) => {
                // Parse distinct agents from response
                let agents: Vec<serde_json::Value> = resp2.json().await.unwrap_or_default();
                let unique: std::collections::HashSet<String> = agents
                    .iter()
                    .filter_map(|r| r.get("agent_hash").and_then(|v| v.as_str()).map(String::from))
                    .collect();
                unique.len() as i64
            }
            Err(_) => 0,
        };

        Ok(serde_json::json!({
            "total_trades": total_trades,
            "total_agents": agent_count,
            "version": "1.0.0"
        }))
    }

    /// Get golden rules derived from community trade data.
    pub async fn get_golden_rules(&self) -> Result<serde_json::Value> {
        let url = format!(
            "{}?order=computed_at.desc&limit=1",
            self.table_url("golden_rules")
        );

        let mut req = self.http.get(&url);
        for (key, val) in self.auth_headers() {
            req = req.header(key, val);
        }

        let resp = req.send().await.context("Supabase get_golden_rules")?;
        let status = resp.status();

        if !status.is_success() {
            // Return sensible defaults if table doesn't exist yet
            return Ok(serde_json::json!({
                "rules": [
                    "Minimum 5% edge before entry",
                    "Never risk more than 10% of portfolio per trade",
                    "Wait for Devil's Advocate confidence > 60%",
                    "Crypto category has highest win rate",
                    "Scalp mode works best for edges < 8%"
                ],
                "computed_at": null
            }));
        }

        let rows: Vec<serde_json::Value> = resp.json().await?;
        if let Some(row) = rows.into_iter().next() {
            return Ok(row);
        }

        Ok(serde_json::json!({
            "rules": [
                "Minimum 5% edge before entry",
                "Never risk more than 10% of portfolio per trade",
                "Wait for Devil's Advocate confidence > 60%",
                "Crypto category has highest win rate",
                "Scalp mode works best for edges < 8%"
            ],
            "computed_at": null
        }))
    }

    /// Get recommended configuration parameters.
    pub async fn get_recommended_params(&self) -> Result<serde_json::Value> {
        let url = format!(
            "{}?order=computed_at.desc&limit=1",
            self.table_url("recommended_params")
        );

        let mut req = self.http.get(&url);
        for (key, val) in self.auth_headers() {
            req = req.header(key, val);
        }

        let resp = req.send().await.context("Supabase get_recommended_params")?;
        let status = resp.status();

        if !status.is_success() {
            return Ok(default_params());
        }

        let rows: Vec<serde_json::Value> = resp.json().await?;
        if let Some(row) = rows.into_iter().next() {
            return Ok(row);
        }

        Ok(default_params())
    }

    /// Register a new agent hash (upsert to avoid duplicates).
    pub async fn register_agent(&self, agent_hash: &str) -> Result<()> {
        let url = self.table_url("agents");

        let body = serde_json::json!({
            "agent_hash": agent_hash,
            "registered_at": chrono::Utc::now().to_rfc3339(),
        });

        let mut req = self.http
            .post(&url)
            .header("Content-Type", "application/json")
            .header("Prefer", "resolution=merge-duplicates");

        for (key, val) in self.auth_headers() {
            req = req.header(key, val);
        }

        let resp = req.json(&body).send().await.context("Supabase register_agent")?;
        let status = resp.status();

        if !status.is_success() {
            let err_text = resp.text().await.unwrap_or_default();
            anyhow::bail!("Supabase register_agent failed ({}): {}", status, err_text);
        }

        info!("Registered agent: {}...", &agent_hash[..8.min(agent_hash.len())]);
        Ok(())
    }

    /// Insert a batch of trade reports. Returns count of successfully inserted rows.
    pub async fn insert_trades_batch(&self, reports: &[TradeReport]) -> Result<usize> {
        let url = self.table_url("trades");

        let bodies: Vec<serde_json::Value> = reports
            .iter()
            .map(|r| {
                serde_json::json!({
                    "agent_hash": r.agent_hash,
                    "category": r.category,
                    "trade_mode": r.trade_mode,
                    "direction": r.direction,
                    "entry_edge_pct": r.entry_edge_pct,
                    "judge_confidence": r.judge_confidence,
                    "judge_model": r.judge_model,
                    "result": r.result,
                    "pnl_pct": r.pnl_pct,
                    "hold_hours": r.hold_hours,
                    "exit_reason": r.exit_reason,
                    "market_type": r.market_type,
                    "volume_bucket": r.volume_bucket,
                    "specialist_desk": r.specialist_desk,
                    "bull_confidence": r.bull_confidence,
                    "bear_confidence": r.bear_confidence,
                    "agent_version": r.agent_version,
                })
            })
            .collect();

        let mut req = self.http
            .post(&url)
            .header("Content-Type", "application/json")
            .header("Prefer", "return=representation");

        for (key, val) in self.auth_headers() {
            req = req.header(key, val);
        }

        let resp = req.json(&bodies).send().await.context("Supabase batch insert")?;
        let status = resp.status();

        if !status.is_success() {
            let err_text = resp.text().await.unwrap_or_default();
            anyhow::bail!("Supabase batch insert failed ({}): {}", status, err_text);
        }

        let rows: Vec<serde_json::Value> = resp.json().await?;
        let count = rows.len();
        info!("Batch inserted {} trades", count);
        Ok(count)
    }

    /// Run the aggregation query to compute insights from raw trades.
    /// Called periodically by the background aggregator.
    pub async fn compute_and_store_insights(&self) -> Result<()> {
        // Fetch all trades from the last 7 days
        let since = (chrono::Utc::now() - chrono::Duration::days(7))
            .to_rfc3339()
            .replace("+", "%2B");
        let url = format!(
            "{}?created_at=gte.{}&order=created_at.desc",
            self.table_url("trades"),
            since
        );

        let mut req = self.http.get(&url);
        for (key, val) in self.auth_headers() {
            req = req.header(key, val);
        }

        let resp = match req.send().await {
            Ok(r) => r,
            Err(e) => {
                info!("Aggregation skipped: Supabase unreachable ({})", e);
                return Ok(());
            }
        };
        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            info!("Aggregation skipped: Supabase returned {} (tables may not exist yet)", status);
            if !body.is_empty() {
                info!("  Detail: {}", &body[..body.len().min(200)]);
            }
            return Ok(());
        }

        let trades: Vec<serde_json::Value> = resp.json().await?;
        if trades.is_empty() {
            info!("No trades to aggregate");
            return Ok(());
        }

        // Compute aggregate stats
        let total_trades = trades.len() as i64;
        let mut agents = std::collections::HashSet::new();
        let mut wins = 0i64;
        let mut category_map: std::collections::HashMap<String, (i64, i64, f64, f64)> =
            std::collections::HashMap::new();
        let mut mode_map: std::collections::HashMap<String, (i64, i64, f64)> =
            std::collections::HashMap::new();
        let mut model_map: std::collections::HashMap<String, (i64, i64, f64)> =
            std::collections::HashMap::new();

        for t in &trades {
            if let Some(ah) = t.get("agent_hash").and_then(|v| v.as_str()) {
                agents.insert(ah.to_string());
            }

            let is_win = t
                .get("result")
                .and_then(|v| v.as_str())
                .map(|r| r == "win")
                .unwrap_or(false);
            if is_win {
                wins += 1;
            }

            let category = t
                .get("category")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown")
                .to_string();
            let edge = t.get("entry_edge_pct").and_then(|v| v.as_f64()).unwrap_or(0.0);
            let pnl = t.get("pnl_pct").and_then(|v| v.as_f64()).unwrap_or(0.0);
            let entry = category_map.entry(category).or_insert((0, 0, 0.0, 0.0));
            entry.0 += 1;
            if is_win {
                entry.1 += 1;
            }
            entry.2 += edge;
            entry.3 += pnl;

            let mode = t
                .get("trade_mode")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown")
                .to_string();
            let hold = t.get("hold_hours").and_then(|v| v.as_f64()).unwrap_or(0.0);
            let mode_entry = mode_map.entry(mode).or_insert((0, 0, 0.0));
            mode_entry.0 += 1;
            if is_win {
                mode_entry.1 += 1;
            }
            mode_entry.2 += hold;

            let model = t
                .get("judge_model")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown")
                .to_string();
            let conf = t.get("judge_confidence").and_then(|v| v.as_f64()).unwrap_or(0.0);
            let model_entry = model_map.entry(model).or_insert((0, 0, 0.0));
            model_entry.0 += 1;
            if is_win {
                model_entry.1 += 1;
            }
            model_entry.2 += conf;
        }

        let overall_win_rate = if total_trades > 0 {
            (wins as f64 / total_trades as f64) * 100.0
        } else {
            0.0
        };

        // Build category stats
        let mut category_stats = serde_json::Map::new();
        for (cat, (count, cat_wins, sum_edge, sum_pnl)) in &category_map {
            let wr = if *count > 0 {
                (*cat_wins as f64 / *count as f64) * 100.0
            } else {
                0.0
            };
            let avg_edge = if *count > 0 { sum_edge / *count as f64 } else { 0.0 };
            let avg_pnl = if *count > 0 { sum_pnl / *count as f64 } else { 0.0 };
            let rec = if wr >= 60.0 {
                "RECOMMENDED"
            } else if wr >= 45.0 {
                "NEUTRAL"
            } else {
                "AVOID"
            };
            category_stats.insert(
                cat.clone(),
                serde_json::json!({
                    "trades": count,
                    "win_rate": (wr * 10.0).round() / 10.0,
                    "avg_edge": (avg_edge * 100.0).round() / 100.0,
                    "avg_pnl_pct": (avg_pnl * 100.0).round() / 100.0,
                    "recommendation": rec,
                }),
            );
        }

        // Build mode stats
        let mut mode_stats = serde_json::Map::new();
        for (mode, (count, mode_wins, sum_hold)) in &mode_map {
            let wr = if *count > 0 {
                (*mode_wins as f64 / *count as f64) * 100.0
            } else {
                0.0
            };
            let avg_hold = if *count > 0 { sum_hold / *count as f64 } else { 0.0 };
            mode_stats.insert(
                mode.clone(),
                serde_json::json!({
                    "trades": count,
                    "win_rate": (wr * 10.0).round() / 10.0,
                    "avg_hold_hours": (avg_hold * 10.0).round() / 10.0,
                }),
            );
        }

        // Build model stats
        let mut model_stats_json = serde_json::Map::new();
        for (model, (count, model_wins, sum_conf)) in &model_map {
            let wr = if *count > 0 {
                (*model_wins as f64 / *count as f64) * 100.0
            } else {
                0.0
            };
            let avg_conf = if *count > 0 { sum_conf / *count as f64 } else { 0.0 };
            model_stats_json.insert(
                model.clone(),
                serde_json::json!({
                    "trades": count,
                    "win_rate": (wr * 10.0).round() / 10.0,
                    "avg_confidence": (avg_conf * 100.0).round() / 100.0,
                }),
            );
        }

        // Derive recommended config from data
        let avoid_cats: Vec<String> = category_map
            .iter()
            .filter(|(_, (count, cat_wins, _, _))| {
                *count >= 5
                    && (*cat_wins as f64 / *count as f64) < 0.45
            })
            .map(|(cat, _)| cat.clone())
            .collect();

        let recommended_config = serde_json::json!({
            "min_edge": 5.0,
            "min_confidence": 60.0,
            "kelly_fraction": 0.25,
            "best_hours_utc": [14, 15, 16, 17, 18, 19, 20],
            "avoid_categories": avoid_cats,
            "sonnet_top_n": 3
        });

        // Build golden rules from data
        let mut golden_rules = vec![
            "Minimum 5% edge before entry".to_string(),
            "Never risk more than 10% of portfolio per trade".to_string(),
        ];
        if overall_win_rate > 55.0 {
            golden_rules.push(format!(
                "Community win rate is {:.1}% -- keep filtering for high-edge trades",
                overall_win_rate
            ));
        }
        for (cat, (count, _, _, _)) in &category_map {
            let wr = if *count > 0 {
                (category_map[cat].1 as f64 / *count as f64) * 100.0
            } else {
                0.0
            };
            if *count >= 10 && wr >= 65.0 {
                golden_rules.push(format!("{} category shows {:.0}% win rate -- favor it", cat, wr));
            }
            if *count >= 10 && wr < 40.0 {
                golden_rules.push(format!("{} category shows {:.0}% win rate -- avoid it", cat, wr));
            }
        }

        // Compose the full insights payload
        let insights = serde_json::json!({
            "computed_at": chrono::Utc::now().to_rfc3339(),
            "period": "7d",
            "total_trades": total_trades,
            "total_agents": agents.len(),
            "overall_win_rate": (overall_win_rate * 10.0).round() / 10.0,
            "category_stats": category_stats,
            "mode_stats": mode_stats,
            "model_stats": model_stats_json,
            "recommended_config": recommended_config,
            "golden_rules": golden_rules,
        });

        // Upsert into insights table
        let insights_url = self.table_url("insights");
        let row = serde_json::json!({
            "period": "7d",
            "data": insights,
            "computed_at": chrono::Utc::now().to_rfc3339(),
        });

        let mut req = self.http
            .post(&insights_url)
            .header("Content-Type", "application/json")
            .header("Prefer", "resolution=merge-duplicates");

        for (key, val) in self.auth_headers() {
            req = req.header(key, val);
        }

        let resp = req.json(&row).send().await.context("Supabase upsert insights")?;
        if !resp.status().is_success() {
            let err_text = resp.text().await.unwrap_or_default();
            warn!("Failed to upsert insights: {}", err_text);
        } else {
            info!(
                "Aggregated insights: {} trades, {} agents, {:.1}% WR",
                total_trades,
                agents.len(),
                overall_win_rate
            );
        }

        // Also upsert golden_rules table
        let rules_url = self.table_url("golden_rules");
        let rules_row = serde_json::json!({
            "id": 1,
            "rules": golden_rules,
            "computed_at": chrono::Utc::now().to_rfc3339(),
        });

        let mut req2 = self.http
            .post(&rules_url)
            .header("Content-Type", "application/json")
            .header("Prefer", "resolution=merge-duplicates");

        for (key, val) in self.auth_headers() {
            req2 = req2.header(key, val);
        }

        let _ = req2.json(&rules_row).send().await;

        // Upsert recommended_params table
        let params_url = self.table_url("recommended_params");
        let params_row = serde_json::json!({
            "id": 1,
            "min_edge": 5.0,
            "min_confidence": 60.0,
            "kelly_fraction": 0.25,
            "best_hours_utc": [14, 15, 16, 17, 18, 19, 20],
            "avoid_categories": avoid_cats,
            "sonnet_top_n": 3,
            "computed_at": chrono::Utc::now().to_rfc3339(),
        });

        let mut req3 = self.http
            .post(&params_url)
            .header("Content-Type", "application/json")
            .header("Prefer", "resolution=merge-duplicates");

        for (key, val) in self.auth_headers() {
            req3 = req3.header(key, val);
        }

        let _ = req3.json(&params_row).send().await;

        Ok(())
    }
}

/// Default recommended parameters when no data is available.
fn default_params() -> serde_json::Value {
    serde_json::json!({
        "min_edge": 5.0,
        "min_confidence": 60.0,
        "kelly_fraction": 0.25,
        "best_hours_utc": [14, 15, 16, 17, 18, 19, 20],
        "avoid_categories": [],
        "sonnet_top_n": 3,
        "computed_at": null
    })
}
