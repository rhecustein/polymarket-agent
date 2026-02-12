use crate::telegram::TelegramAlert;
use crate::paper::Portfolio;
use crate::db::StateStore;
use crate::team::types::TradePlan;
use crate::types::{Analysis, Direction, Trade};
use chrono::Utc;
use rust_decimal::Decimal;
use rust_decimal::prelude::*;
use tracing::{error, info};

/// Agent 9: Executor — Trade execution + monitoring (no AI)
/// Wraps Portfolio for trade execution, records to StateStore, sends alerts.
/// Enriches trade with full agent trail for paper trading battle test.
pub async fn execute(
    plan: &TradePlan,
    portfolio: &Portfolio,
    store: &StateStore,
    telegram: &TelegramAlert,
) -> Option<Trade> {
    if plan.direction == Direction::Skip {
        return None;
    }

    // Execute the trade via Portfolio
    let mut trade = portfolio.execute_trade(
        &plan.market.id,
        &plan.market.question,
        plan.direction,
        plan.market.yes_price,
        plan.fair_value_yes,
        plan.edge,
        plan.bet_size,
    )?;

    // Enrich with paper trading agent trail
    trade.trade_mode = Some(format!("{}", plan.mode));
    trade.category = Some(plan.market.category.clone());
    trade.specialist_desk = plan.specialist_desk.clone();
    trade.bull_probability = plan.bull_probability;
    trade.bear_probability = plan.bear_probability;
    trade.judge_fair_value = Some(plan.fair_value_yes.to_f64().unwrap_or(0.0));
    trade.judge_confidence = Some(plan.confidence.to_f64().unwrap_or(0.0));
    trade.judge_model = plan.judge_model.clone();

    // Set TP/SL price levels from percentages
    if plan.take_profit_pct > Decimal::ZERO {
        trade.take_profit = Some(trade.entry_price * (Decimal::ONE + plan.take_profit_pct));
    }
    if plan.stop_loss_pct > Decimal::ZERO {
        trade.stop_loss = Some(trade.entry_price * (Decimal::ONE - plan.stop_loss_pct));
    }
    if plan.max_hold_hours > 0 {
        trade.max_hold_until = Some(Utc::now() + chrono::Duration::hours(plan.max_hold_hours as i64));
    }

    // Set token_id for CLOB price tracking
    let token_id = plan.market.tokens.iter()
        .find(|t| {
            (plan.direction == Direction::Yes && t.outcome == "Yes")
            || (plan.direction == Direction::No && t.outcome == "No")
        })
        .map(|t| t.token_id.clone());
    trade.token_id = token_id;

    info!(
        "EXECUTE [{}]: {} {} @ {} | ${} | edge={:.1}% conf={:.2} | desk={} judge={}",
        plan.mode,
        trade.direction,
        &trade.question[..trade.question.len().min(35)],
        trade.entry_price,
        trade.bet_size,
        (plan.edge * Decimal::from(100)),
        plan.confidence,
        plan.specialist_desk.as_deref().unwrap_or("?"),
        plan.judge_model.as_deref().unwrap_or("?"),
    );

    // Save trade to database
    if let Err(e) = store.save_trade(&trade) {
        error!("Failed to save trade: {e}");
    }

    // Save analysis record
    let analysis = Analysis {
        market_id: plan.market.id.clone(),
        question: plan.market.question.clone(),
        current_yes_price: plan.market.yes_price,
        fair_value_yes: plan.fair_value_yes,
        edge: plan.edge,
        confidence: plan.confidence,
        direction: plan.direction,
        should_trade: true,
        reasoning: plan.reasoning.clone(),
        api_cost_usd: Decimal::ZERO,
        model_used: format!("team-v2.0-{}", plan.mode),
        enrichment_data: None,
    };
    store.save_analysis(&analysis).ok();

    // Send Telegram alert with richer info
    telegram.send_paper_trade_alert(&trade).await.ok();

    Some(trade)
}
