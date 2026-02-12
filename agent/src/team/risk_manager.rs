use crate::config::Config;
use crate::paper::Portfolio;
use crate::strategy::{kelly_bet, survival_adjust};
use crate::team::types::{DevilsVerdict, RiskDecision};
use crate::types::Direction;
use rust_decimal::Decimal;
use rust_decimal::prelude::*;
use tracing::info;

/// Agent 7: Risk Manager — Position sizing + portfolio risk checks (no AI)
/// Wraps kelly_bet() + survival_adjust() with additional portfolio-level checks.
pub fn check(
    verdict: &DevilsVerdict,
    portfolio: &Portfolio,
    config: &Config,
    effective_max_pct: Decimal,
) -> RiskDecision {
    let mut adjustments = Vec::new();

    let direction = verdict.direction_enum();
    if direction == Direction::Skip {
        return RiskDecision {
            approved: false,
            position_size: Decimal::ZERO,
            reason: "Direction is SKIP".to_string(),
            adjustments,
        };
    }

    let balance = portfolio.balance();

    // Check 1: Balance above kill threshold
    if !portfolio.is_alive(config.kill_threshold) {
        return RiskDecision {
            approved: false,
            position_size: Decimal::ZERO,
            reason: format!("Balance ${} below kill threshold ${}", balance, config.kill_threshold),
            adjustments,
        };
    }

    // Check 2: Reserve protection
    let reserve = config.initial_balance * config.balance_reserve_pct;
    let available = balance - reserve;
    if available <= Decimal::ZERO {
        return RiskDecision {
            approved: false,
            position_size: Decimal::ZERO,
            reason: format!("Balance ${} <= reserve ${}", balance, reserve),
            adjustments,
        };
    }

    // Check 3: Confidence threshold
    let confidence = Decimal::from_f64(verdict.confidence).unwrap_or(Decimal::ZERO);
    if confidence < config.min_confidence {
        return RiskDecision {
            approved: false,
            position_size: Decimal::ZERO,
            reason: format!("Confidence {:.2} < min {}", confidence, config.min_confidence),
            adjustments,
        };
    }

    // Check 4: Survival mode adjustment
    let (adj_max_pct, is_dead) = survival_adjust(balance, config.kill_threshold, effective_max_pct);
    if is_dead {
        return RiskDecision {
            approved: false,
            position_size: Decimal::ZERO,
            reason: "Agent should be dead".to_string(),
            adjustments,
        };
    }
    let actual_max_pct = adj_max_pct;
    if actual_max_pct < effective_max_pct {
        adjustments.push(format!("Survival mode: max_pct reduced to {:.1}%", actual_max_pct * Decimal::from(100)));
    }

    // Check 5: Edge sanity
    let fair_value = Decimal::from_f64(verdict.fair_value_yes).unwrap_or(Decimal::new(50, 2));
    let edge = (fair_value - Decimal::new(50, 2)).abs(); // Rough edge check
    if edge > Decimal::new(35, 2) {
        return RiskDecision {
            approved: false,
            position_size: Decimal::ZERO,
            reason: format!("Edge {:.2} too large (>35%) — likely calibration error", edge),
            adjustments,
        };
    }

    // Calculate Kelly bet size
    // We need the market price to compute Kelly — use fair_value as proxy for direction
    // The actual market price will be used in the orchestrator
    let kelly = kelly_bet(
        balance,
        fair_value,
        // For Kelly, we need market price — approximate from verdict
        Decimal::from_f64(
            if direction == Direction::Yes {
                verdict.fair_value_yes - (verdict.fair_value_yes - 0.5).abs() * 0.5 // rough market estimate
            } else {
                1.0 - verdict.fair_value_yes + (verdict.fair_value_yes - 0.5).abs() * 0.5
            }
        ).unwrap_or(Decimal::new(50, 2)),
        direction,
        actual_max_pct,
        config.kelly_fraction,
    );

    if kelly.bet_size <= Decimal::ZERO {
        return RiskDecision {
            approved: false,
            position_size: Decimal::ZERO,
            reason: "Kelly bet size is zero".to_string(),
            adjustments,
        };
    }

    // Cap to available funds
    let mut bet_size = kelly.bet_size.min(available);

    // Confidence scaling
    let confidence_scale = if confidence >= Decimal::new(80, 2) {
        Decimal::ONE
    } else {
        (confidence / Decimal::new(80, 2)).min(Decimal::ONE)
    };
    bet_size = (bet_size * confidence_scale).round_dp(2);

    if bet_size <= Decimal::ZERO {
        return RiskDecision {
            approved: false,
            position_size: Decimal::ZERO,
            reason: "Bet size after confidence scaling is zero".to_string(),
            adjustments,
        };
    }

    adjustments.push(format!("Kelly: {:.2}% | Conf scale: {:.2}", kelly.adjusted_kelly * Decimal::from(100), confidence_scale));

    info!(
        "Risk APPROVED: ${} (Kelly={:.2}% conf_scale={:.2})",
        bet_size,
        kelly.adjusted_kelly * Decimal::from(100),
        confidence_scale,
    );

    RiskDecision {
        approved: true,
        position_size: bet_size,
        reason: format!("Approved: ${} ({} risk)", bet_size, kelly.risk_level),
        adjustments,
    }
}
