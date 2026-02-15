use crate::types::Direction;
use rust_decimal::Decimal;

#[derive(Debug, Clone)]
pub struct KellyResult {
    pub full_kelly: Decimal,
    pub adjusted_kelly: Decimal, // full * kelly_fraction
    pub bet_size: Decimal,
    #[allow(dead_code)]
    pub max_allowed: Decimal,
    pub risk_level: &'static str,
    pub expected_value: Decimal,
}

/// Action to take based on consecutive loss count
#[derive(Debug, Clone, PartialEq)]
pub enum LossAction {
    /// Normal trading
    Continue,
    /// Skip this cycle (3 consecutive losses)
    SkipCycle,
    /// Reduce position size by 50% for next 3 trades (4 consecutive losses)
    ReduceSize,
    /// Pause all trading and send critical alert (5 consecutive losses — hard stop)
    Pause,
}

/// Check consecutive losses and return appropriate action.
/// Tightened thresholds: skip at 3, reduce at 4, full pause at 5 losses.
pub fn check_consecutive_losses(consecutive: u32) -> LossAction {
    if consecutive >= 5 {
        LossAction::Pause
    } else if consecutive >= 4 {
        LossAction::ReduceSize
    } else if consecutive >= 3 {
        LossAction::SkipCycle
    } else {
        LossAction::Continue
    }
}

/// Calculate optimal bet size using Kelly Criterion.
///
/// For Polymarket binary shares:
///   Buy YES at price P → pays $1 if YES wins
///   Net odds b = (1-P)/P
///   f* = (p*b - q) / b
///
/// We use `kelly_fraction` (e.g., 1/3) for ultra-conservative sizing.
pub fn kelly_bet(
    bankroll: Decimal,
    fair_prob: Decimal,
    market_price: Decimal,
    direction: Direction,
    max_pct: Decimal,
    kelly_fraction: Decimal,
) -> KellyResult {
    let zero = Decimal::ZERO;
    let one = Decimal::ONE;

    let mut result = KellyResult {
        full_kelly: zero,
        adjusted_kelly: zero,
        bet_size: zero,
        max_allowed: bankroll * max_pct,
        risk_level: "NONE",
        expected_value: zero,
    };

    if direction == Direction::Skip {
        return result;
    }

    // Effective probability and price based on direction
    let (p, price) = match direction {
        Direction::Yes => (fair_prob, market_price),
        Direction::No => (one - fair_prob, one - market_price),
        Direction::Skip => return result,
    };

    if price <= zero || price >= one || p <= zero || p >= one {
        return result;
    }

    // Net odds: profit per $1 risked
    let b = (one - price) / price;
    let q = one - p;

    // Kelly fraction: f* = (p*b - q) / b
    let kelly = (p * b - q) / b;

    if kelly <= zero {
        return result;
    }

    result.full_kelly = kelly;
    result.adjusted_kelly = kelly * kelly_fraction;

    // AUTO-SCALING: Increase position size as bankroll grows
    // Start with base max_pct, then scale up linearly with bankroll
    // Example: $100 → 1.0x, $200 → 1.5x, $500 → 2.5x, $1000 → 3.5x
    let initial_bankroll = Decimal::new(100, 0); // Assume $100 starting point
    let scale_factor = if bankroll > initial_bankroll {
        let growth_ratio = bankroll / initial_bankroll;
        // Scale factor: 1.0 + (growth_ratio - 1) * 0.5
        // This means every 2x bankroll growth → 1.5x position size increase
        Decimal::ONE + (growth_ratio - Decimal::ONE) * Decimal::new(5, 1)
    } else {
        Decimal::ONE
    };

    let scaled_max_pct = (max_pct * scale_factor).min(Decimal::new(80, 2)); // Cap at 80% max

    // Cap at scaled max position size
    let bet_fraction = result.adjusted_kelly.min(scaled_max_pct);
    result.bet_size = (bankroll * bet_fraction).round_dp(2);

    // Minimum trade size check ($0.10 for paper simulation, $0.50 for live)
    let min_trade = Decimal::new(10, 2); // $0.10 (lowered for paper trading sim)
    if result.bet_size < min_trade {
        result.bet_size = zero;
        result.risk_level = "BELOW_MIN";
        return result;
    }

    // Don't bet more than we have
    if result.bet_size > bankroll {
        result.bet_size = bankroll;
    }

    // Risk classification
    result.risk_level = if bet_fraction < Decimal::new(2, 2) {
        "LOW"
    } else if bet_fraction < Decimal::new(3, 2) {
        "MEDIUM"
    } else {
        "HIGH"
    };

    // Expected value
    let profit_if_win = result.bet_size * b;
    let loss_if_lose = result.bet_size;
    result.expected_value = (p * profit_if_win - q * loss_if_lose).round_dp(4);

    result
}

/// Survival mode adjustments when balance is low
pub fn survival_adjust(
    bankroll: Decimal,
    kill_threshold: Decimal,
    normal_max_pct: Decimal,
) -> (Decimal, bool) {
    let buffer_zone = kill_threshold * Decimal::new(3, 0); // 3x kill threshold

    if bankroll <= kill_threshold {
        // DEAD
        return (Decimal::ZERO, true);
    }

    if bankroll < buffer_zone {
        // Survival mode: reduce position size proportionally
        let ratio = (bankroll - kill_threshold) / (buffer_zone - kill_threshold);
        let reduced_pct = normal_max_pct * ratio * Decimal::new(5, 1); // Max 50% of normal
        return (reduced_pct.max(Decimal::new(1, 2)), false); // At least 1%
    }

    (normal_max_pct, false)
}
