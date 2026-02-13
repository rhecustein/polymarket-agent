use crate::team::types::{DevilsVerdict, RiskDecision, TradeMode, TradePlan};
use crate::types::{Direction, Market};
use rust_decimal::Decimal;
use rust_decimal::prelude::*;
use tracing::info;

/// Agent 8: Strategist — Trade mode classification + exit plan (no AI)
/// Classifies trades into SCALP / SWING / CONVICTION based on edge, confidence, and time.
pub fn plan(
    verdict: &DevilsVerdict,
    risk: &RiskDecision,
    market: &Market,
) -> TradePlan {
    let direction = verdict.direction_enum();
    let fair_value = Decimal::from_f64(verdict.fair_value_yes).unwrap_or(Decimal::new(50, 2));
    let confidence = Decimal::from_f64(verdict.confidence).unwrap_or(Decimal::ZERO);

    let market_price = match direction {
        Direction::Yes => market.yes_price,
        Direction::No => market.no_price,
        Direction::Skip => market.yes_price,
    };

    let edge = (fair_value - market.yes_price).abs();

    // Determine days until expiry
    let days_left = parse_days_remaining(&market.end_date).unwrap_or(30);

    // Classify trade mode
    let mode = classify_mode(edge, confidence, days_left);

    // Set TP/SL/max_hold based on mode
    let (tp_pct, sl_pct, max_hold_hours, check_interval) = match mode {
        TradeMode::Scalp => {
            // Fast exit: favorable reward:risk 1.5:1, tight time limit
            (
                Decimal::new(12, 2),  // 12% TP
                Decimal::new(8, 2),   // 8% SL
                24u64,                // 24h max hold (fast rotation)
                30u64,                // Check every 30s (rapid monitoring)
            )
        }
        TradeMode::Swing => {
            // Medium hold, tighter SL for faster loss cutting
            let dynamic_tp = (edge * Decimal::new(8, 1)).max(Decimal::new(5, 2)); // 80% of edge or 5%
            (
                dynamic_tp.min(Decimal::new(20, 2)), // Cap at 20%
                Decimal::new(10, 2),                  // 10% SL (was 15% — cut losers faster)
                168u64,                               // 7 days max (was 14d — faster capital rotation)
                90u64,                                // Check every 90s (was 180s — faster monitoring)
            )
        }
        TradeMode::Conviction => {
            // Hold to resolution, with faster monitoring
            (
                Decimal::ZERO, // No TP — hold to resolution
                Decimal::ZERO, // No SL — conviction hold
                0u64,          // No max hold — hold to resolution
                180u64,        // Check every 3 min (was 5 min)
            )
        }
    };

    // Spread check per mode
    let spread = (market.yes_price + market.no_price - Decimal::ONE).abs();
    let max_spread = match mode {
        TradeMode::Scalp => Decimal::new(2, 2),      // 2%
        TradeMode::Swing => Decimal::new(4, 2),       // 4%
        TradeMode::Conviction => Decimal::new(5, 2),  // 5%
    };

    let reasoning = format!(
        "[{}] edge={:.1}% conf={:.2} days_left={} spread={:.1}%{}",
        mode,
        (edge * Decimal::from(100)).to_f64().unwrap_or(0.0),
        confidence.to_f64().unwrap_or(0.0),
        days_left,
        (spread * Decimal::from(100)).to_f64().unwrap_or(0.0),
        if spread > max_spread {
            format!(" (WARN: spread exceeds {}% limit)", (max_spread * Decimal::from(100)).to_f64().unwrap_or(0.0))
        } else {
            String::new()
        },
    );

    info!("Strategist: {}", reasoning);

    TradePlan {
        market: market.clone(),
        direction,
        fair_value_yes: fair_value,
        edge,
        confidence,
        mode,
        bet_size: risk.position_size,
        entry_price: market_price,
        take_profit_pct: tp_pct,
        stop_loss_pct: sl_pct,
        max_hold_hours,
        check_interval_secs: check_interval,
        reasoning,
        specialist_desk: None,
        bull_probability: None,
        bear_probability: None,
        judge_model: None,
    }
}

/// Classify trade into SCALP / SWING / CONVICTION
fn classify_mode(edge: Decimal, confidence: Decimal, days_left: i64) -> TradeMode {
    let edge_f = edge.to_f64().unwrap_or(0.0);
    let conf_f = confidence.to_f64().unwrap_or(0.0);

    // CONVICTION: high edge + high confidence
    if edge_f > 0.20 && conf_f >= 0.75 {
        return TradeMode::Conviction;
    }

    // SCALP: good edge + good confidence + market ending soon
    if edge_f > 0.15 && conf_f >= 0.70 && days_left <= 7 {
        return TradeMode::Scalp;
    }

    // Default: SWING
    TradeMode::Swing
}

fn parse_days_remaining(end_date: &str) -> Option<i64> {
    let now = chrono::Utc::now().date_naive();

    let cleaned = end_date.replace('Z', "+00:00");
    if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(&cleaned) {
        return Some((dt.date_naive() - now).num_days().max(0));
    }

    if end_date.len() >= 10 {
        if let Ok(d) = chrono::NaiveDate::parse_from_str(&end_date[..10], "%Y-%m-%d") {
            return Some((d - now).num_days().max(0));
        }
    }

    None
}
