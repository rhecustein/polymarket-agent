use crate::config::Config;
use crate::data::polymarket::GammaScanner;
use crate::team::types::{MarketCandidate, ScoutReport};
use crate::types::Market;
use anyhow::Result;
use chrono::Utc;
use rust_decimal::prelude::*;
use tracing::info;

/// Agent 1: Scout — Market discovery (no AI)
/// Scans ALL categories, applies quality filters + heuristic scoring.
/// Returns top N candidates sorted by quality score.
pub async fn scan(
    scanner: &GammaScanner,
    config: &Config,
    max_candidates: usize,
) -> Result<ScoutReport> {
    let markets = scanner.scan(config.max_markets_to_scan).await?;
    let total_scanned = markets.len();

    // Apply basic quality filters (no category restriction)
    let filtered: Vec<Market> = markets
        .into_iter()
        .filter(|m| passes_quality_filter(m))
        .collect();

    let total_passed_quality = filtered.len();

    // Score and rank candidates
    let mut candidates: Vec<MarketCandidate> = filtered
        .into_iter()
        .map(|m| {
            let (score, reason) = score_candidate(&m);
            MarketCandidate {
                market: m,
                quality_score: score,
                reason,
            }
        })
        .filter(|c| c.quality_score > 10.0)
        .collect();

    candidates.sort_by(|a, b| b.quality_score.partial_cmp(&a.quality_score).unwrap_or(std::cmp::Ordering::Equal));
    candidates.truncate(max_candidates);

    info!(
        "Scout: {} scanned -> {} quality -> {} candidates (top {})",
        total_scanned, total_passed_quality, candidates.len(), max_candidates
    );

    Ok(ScoutReport {
        candidates,
        total_scanned,
        total_passed_quality,
    })
}

/// Basic quality filter — accepts all categories
fn passes_quality_filter(m: &Market) -> bool {
    let yes = m.yes_price.to_f64().unwrap_or(0.5);
    // Skip extreme prices (almost certainly resolved or illiquid)
    if yes < 0.03 || yes > 0.97 {
        return false;
    }
    // Skip zero-volume markets
    let vol = m.volume.to_f64().unwrap_or(0.0);
    if vol < 100.0 {
        return false;
    }
    true
}

/// Category-agnostic scoring with domain bonuses
fn score_candidate(m: &Market) -> (f64, String) {
    let mut score = 0.0_f64;
    let mut reasons = Vec::new();
    let q = m.question.to_lowercase();
    let c = m.category.to_lowercase();

    // ── Price Sweet Spot (most important — biggest room for edge) ──
    let yes = m.yes_price.to_f64().unwrap_or(0.5);
    let distance_from_center = (yes - 0.5).abs();
    if distance_from_center < 0.15 {
        score += 25.0;
        reasons.push("sweet_spot");
    } else if distance_from_center < 0.30 {
        score += 15.0;
        reasons.push("moderate_odds");
    } else {
        score += 5.0;
    }

    // ── Volume (signals market activity) ──
    let vol = m.volume.to_f64().unwrap_or(0.0);
    if vol > 100_000.0 {
        score += 20.0;
        reasons.push("high_vol");
    } else if vol > 10_000.0 {
        score += 12.0;
        reasons.push("med_vol");
    } else if vol > 1_000.0 {
        score += 5.0;
        reasons.push("low_vol");
    }

    // ── Liquidity (can we actually trade?) ──
    let liq = m.liquidity.to_f64().unwrap_or(0.0);
    if liq > 50_000.0 {
        score += 15.0;
        reasons.push("high_liq");
    } else if liq > 10_000.0 {
        score += 8.0;
        reasons.push("med_liq");
    }

    // ── Time Pressure (closer = more actionable, strongly prefer short-term) ──
    if !m.end_date.is_empty() {
        if let Some(days) = parse_days_remaining(&m.end_date) {
            if days <= 3 {
                score += 30.0; // Strong preference for urgent markets
                reasons.push("urgent_<3d");
            } else if days <= 7 {
                score += 22.0; // Short-term (<7d) — best for SCALP
                reasons.push("soon_<7d");
            } else if days <= 14 {
                score += 12.0;
                reasons.push("near_<14d");
            } else if days <= 30 {
                score += 6.0;
                reasons.push("month_<30d");
            }
            // Long-term (>30d) gets no bonus — capital locked too long
        }
    }

    // ── Category Bonuses (domain expertise advantage) ──
    // Crypto: we have CoinGecko data + RSI + Fear/Greed
    if c.contains("crypto") || q.contains("bitcoin") || q.contains("btc")
        || q.contains("ethereum") || q.contains("eth ") || q.contains("solana")
        || q.contains("crypto") || q.contains("token") || q.contains("defi")
    {
        score += 15.0;
        reasons.push("crypto");

        // Specific crypto asset bonus (we have direct price data)
        if q.contains("bitcoin") || q.contains("btc") {
            score += 5.0;
        } else if q.contains("ethereum") || q.contains("eth ") {
            score += 4.0;
        } else if q.contains("solana") {
            score += 3.0;
        }

        // Quantitative target bonus
        if q.contains("hit $") || q.contains("above $") || q.contains("below $")
            || q.contains("reach $") || q.contains("over $") || q.contains("under $")
        {
            score += 10.0;
            reasons.push("price_target");
        }
    }

    // Weather: short-term forecasts are very reliable
    if c.contains("weather") || q.contains("temperature") || q.contains("rain")
        || q.contains("snow") || q.contains("weather") || q.contains("degrees")
    {
        score += 12.0;
        reasons.push("weather");
    }

    // Sports: statistical models work well
    if c.contains("sports") || c.contains("nfl") || c.contains("nba")
        || c.contains("mlb") || c.contains("soccer")
        || q.contains("win the") || q.contains("championship")
        || q.contains("super bowl") || q.contains("playoffs")
    {
        score += 10.0;
        reasons.push("sports");
    }

    // Politics/General: moderate bonus
    if c.contains("politics") || q.contains("election") || q.contains("president") {
        score += 8.0;
        reasons.push("politics");
    }

    // ── Timing: US Market Hours bonus (14:00-22:00 UTC = 9AM-5PM EST) ──
    let now = Utc::now();
    let hour = now.format("%H").to_string().parse::<u32>().unwrap_or(12);
    if hour >= 14 && hour < 22 {
        score += 10.0;
        reasons.push("us_hours");
    }

    // ── Weekend penalty: low liquidity = wider spreads, worse fills ──
    let weekday = now.format("%u").to_string().parse::<u32>().unwrap_or(1); // 1=Mon, 7=Sun
    if weekday >= 6 {
        score -= 15.0;
        reasons.push("weekend_penalty");
    }

    (score, reasons.join(", "))
}

fn parse_days_remaining(end_date: &str) -> Option<i64> {
    let now = chrono::Utc::now().date_naive();

    // Try ISO datetime
    let cleaned = end_date.replace('Z', "+00:00");
    if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(&cleaned) {
        let days = (dt.date_naive() - now).num_days();
        return Some(days.max(0));
    }

    // Try date-only
    if end_date.len() >= 10 {
        if let Ok(d) = chrono::NaiveDate::parse_from_str(&end_date[..10], "%Y-%m-%d") {
            return Some((d - now).num_days().max(0));
        }
    }

    None
}
