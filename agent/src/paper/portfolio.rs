use crate::types::{Direction, ExitReason, Market, Trade, TradeStatus};
use chrono::Utc;
use rust_decimal::Decimal;
use rust_decimal::prelude::*;
use std::sync::Mutex;
use tracing::info;

pub struct Portfolio {
    inner: Mutex<PortfolioInner>,
}

struct PortfolioInner {
    balance: Decimal,
    initial_balance: Decimal,
    trades: Vec<Trade>,
    open_trades: Vec<Trade>,
    win_count: u32,
    loss_count: u32,
    total_api_cost: Decimal,
    peak_balance: Decimal,
    max_drawdown: Decimal,
    start_time: chrono::DateTime<Utc>,
    consecutive_losses: u32,
}

impl Portfolio {
    pub fn new(initial_balance: Decimal) -> Self {
        Self {
            inner: Mutex::new(PortfolioInner {
                balance: initial_balance,
                initial_balance,
                trades: Vec::new(),
                open_trades: Vec::new(),
                win_count: 0,
                loss_count: 0,
                total_api_cost: Decimal::ZERO,
                peak_balance: initial_balance,
                max_drawdown: Decimal::ZERO,
                start_time: Utc::now(),
                consecutive_losses: 0,
            }),
        }
    }

    pub fn balance(&self) -> Decimal {
        self.inner.lock().unwrap().balance
    }

    pub fn is_alive(&self, kill_threshold: Decimal) -> bool {
        self.inner.lock().unwrap().balance > kill_threshold
    }

    pub fn consecutive_losses(&self) -> u32 {
        self.inner.lock().unwrap().consecutive_losses
    }

    pub fn open_position_count(&self) -> usize {
        self.inner.lock().unwrap().open_trades.len()
    }

    pub fn execute_trade(
        &self,
        market_id: &str,
        question: &str,
        direction: Direction,
        market_yes_price: Decimal,
        fair_value: Decimal,
        edge: Decimal,
        mut bet_size: Decimal,
    ) -> Option<Trade> {
        let mut inner = self.inner.lock().unwrap();

        if bet_size > inner.balance {
            bet_size = inner.balance;
        }
        if bet_size <= Decimal::ZERO {
            return None;
        }

        let price = match direction {
            Direction::Yes => market_yes_price,
            Direction::No => Decimal::ONE - market_yes_price,
            Direction::Skip => return None,
        };

        if price <= Decimal::ZERO {
            return None;
        }

        let shares = (bet_size / price).round_dp(4);

        let trade = Trade {
            id: uuid::Uuid::new_v4().to_string()[..8].to_string(),
            timestamp: Utc::now(),
            market_id: market_id.to_string(),
            question: question.to_string(),
            direction,
            entry_price: price,
            fair_value,
            edge,
            bet_size,
            shares,
            status: TradeStatus::Open,
            exit_price: None,
            pnl: Decimal::ZERO,
            balance_after: inner.balance - bet_size,
            order_id: None,
            trade_mode: None,
            take_profit: None,
            stop_loss: None,
            max_hold_until: None,
            category: None,
            specialist_desk: None,
            bull_probability: None,
            bear_probability: None,
            judge_fair_value: None,
            judge_confidence: None,
            judge_model: None,
            exit_reason: None,
            hold_duration_hours: None,
            token_id: None,
        };

        inner.balance -= bet_size;
        inner.trades.push(trade.clone());
        inner.open_trades.push(trade.clone());

        Some(trade)
    }

    /// Resolve open trades using real market prices with mode-based exit logic.
    /// Supports Scalp (TP/SL price levels), Swing (50% edge captured), Conviction (hold to resolution + safety valve).
    /// Returns a vec of trades that were closed this cycle (for DB persistence).
    pub fn resolve_with_prices(
        &self,
        markets: &[Market],
        exit_tp_pct: Decimal,
        exit_sl_pct: Decimal,
    ) -> Vec<Trade> {
        let mut inner = self.inner.lock().unwrap();
        let mut resolved = Vec::new();

        let pending: Vec<Trade> = inner.open_trades.drain(..).collect();
        let mut still_open = Vec::new();

        for mut trade in pending {
            if trade.direction == Direction::Skip {
                continue;
            }

            let market_opt = markets.iter().find(|m| m.id == trade.market_id);

            let mut exit_reason: Option<ExitReason> = None;
            let current_price;

            if let Some(market) = market_opt {
                let yes_price = market.yes_price;
                current_price = match trade.direction {
                    Direction::Yes => yes_price,
                    Direction::No => Decimal::ONE - yes_price,
                    Direction::Skip => unreachable!(),
                };

                // Mode-based exit logic
                let mode = trade.trade_mode.as_deref().unwrap_or("SWING");

                match mode {
                    "SCALP" => {
                        // TP/SL at specific price levels (set by Strategist)
                        if let Some(tp) = trade.take_profit {
                            if current_price >= tp {
                                exit_reason = Some(ExitReason::TakeProfit);
                            }
                        }
                        if exit_reason.is_none() {
                            if let Some(sl) = trade.stop_loss {
                                if current_price <= sl {
                                    exit_reason = Some(ExitReason::StopLoss);
                                }
                            }
                        }
                        // Time expiry
                        if exit_reason.is_none() {
                            if let Some(max_hold) = trade.max_hold_until {
                                if Utc::now() > max_hold {
                                    exit_reason = Some(ExitReason::TimeExpiry);
                                }
                            }
                        }
                    }
                    "SWING" => {
                        // Edge captured: price moved 50%+ toward fair value
                        if let Some(jfv) = trade.judge_fair_value {
                            let fair_dec = Decimal::from_f64(jfv).unwrap_or(trade.fair_value);
                            let total_edge = fair_dec - trade.entry_price;
                            if total_edge.abs() > Decimal::ZERO {
                                let captured = (current_price - trade.entry_price) / total_edge;
                                if captured >= Decimal::new(50, 2) {
                                    exit_reason = Some(ExitReason::EdgeCaptured);
                                }
                            }
                        }
                        // SL check
                        if exit_reason.is_none() {
                            if let Some(sl) = trade.stop_loss {
                                if current_price <= sl {
                                    exit_reason = Some(ExitReason::StopLoss);
                                }
                            }
                        }
                        // Time expiry
                        if exit_reason.is_none() {
                            if let Some(max_hold) = trade.max_hold_until {
                                if Utc::now() > max_hold {
                                    exit_reason = Some(ExitReason::TimeExpiry);
                                }
                            }
                        }
                    }
                    "CONVICTION" => {
                        // Safety valve: loss > 50% and confidence < 0.75
                        let unrealized_pnl = (current_price - trade.entry_price) * trade.shares;
                        let pnl_pct = if trade.bet_size > Decimal::ZERO {
                            (unrealized_pnl / trade.bet_size * Decimal::from(100)).to_f64().unwrap_or(0.0)
                        } else { 0.0 };
                        let conf = trade.judge_confidence.unwrap_or(0.5);
                        if pnl_pct < -50.0 && conf < 0.75 {
                            exit_reason = Some(ExitReason::SafetyValve);
                        }
                        // Otherwise: HOLD until market resolves
                    }
                    _ => {
                        // Fallback: percentage-based TP/SL (legacy behavior)
                        let unrealized_pnl = (current_price - trade.entry_price) * trade.shares;
                        let change_pct = if trade.bet_size > Decimal::ZERO {
                            unrealized_pnl / trade.bet_size
                        } else { Decimal::ZERO };

                        if exit_tp_pct > Decimal::ZERO && change_pct >= exit_tp_pct {
                            exit_reason = Some(ExitReason::TakeProfit);
                        } else if exit_sl_pct > Decimal::ZERO && change_pct <= -exit_sl_pct {
                            exit_reason = Some(ExitReason::StopLoss);
                        }
                    }
                }
            } else {
                // Market disappeared (resolved on Polymarket)
                current_price = trade.entry_price;
                exit_reason = Some(ExitReason::MarketResolved);
            }

            if let Some(reason) = exit_reason {
                let pnl = (current_price - trade.entry_price) * trade.shares;
                let hold_hours = (Utc::now() - trade.timestamp).num_minutes() as f64 / 60.0;

                trade.exit_price = Some(current_price);
                trade.pnl = pnl;
                trade.exit_reason = Some(reason);
                trade.hold_duration_hours = Some(hold_hours);

                if trade.pnl > Decimal::ZERO {
                    trade.status = TradeStatus::Won;
                    inner.win_count += 1;
                    inner.consecutive_losses = 0;
                } else {
                    trade.status = TradeStatus::Lost;
                    inner.loss_count += 1;
                    inner.consecutive_losses += 1;
                }

                // Return capital: bet_size + pnl
                let return_amount = trade.bet_size + trade.pnl;
                inner.balance += if return_amount > Decimal::ZERO { return_amount } else { Decimal::ZERO };
                trade.balance_after = inner.balance;

                if let Some(t) = inner.trades.iter_mut().find(|t| t.id == trade.id) {
                    *t = trade.clone();
                }

                info!("CLOSED [{}]: {} {} | PnL ${} | Reason: {} | Held {:.1}h",
                    trade.trade_mode.as_deref().unwrap_or("?"),
                    trade.direction,
                    &trade.question[..trade.question.len().min(35)],
                    trade.pnl, reason, hold_hours);

                resolved.push(trade);
            } else {
                still_open.push(trade);
            }
        }

        inner.open_trades = still_open;

        if inner.balance > inner.peak_balance {
            inner.peak_balance = inner.balance;
        }
        if inner.peak_balance > Decimal::ZERO {
            let dd = (inner.peak_balance - inner.balance) / inner.peak_balance;
            if dd > inner.max_drawdown {
                inner.max_drawdown = dd;
            }
        }

        resolved
    }

    /// Close all open positions at current prices (for graceful shutdown)
    pub fn close_all_positions(&self, markets: &[Market]) -> Vec<Trade> {
        let mut inner = self.inner.lock().unwrap();
        let mut closed = Vec::new();

        let pending: Vec<Trade> = inner.open_trades.drain(..).collect();

        for mut trade in pending {
            let market_opt = markets.iter().find(|m| m.id == trade.market_id);
            let current_price = if let Some(market) = market_opt {
                match trade.direction {
                    Direction::Yes => market.yes_price,
                    Direction::No => Decimal::ONE - market.yes_price,
                    Direction::Skip => trade.entry_price,
                }
            } else {
                trade.entry_price
            };

            let pnl = (current_price - trade.entry_price) * trade.shares;
            let hold_hours = (Utc::now() - trade.timestamp).num_minutes() as f64 / 60.0;

            trade.exit_price = Some(current_price);
            trade.pnl = pnl;
            trade.exit_reason = Some(ExitReason::ManualStop);
            trade.hold_duration_hours = Some(hold_hours);
            trade.status = if pnl > Decimal::ZERO { TradeStatus::Won } else { TradeStatus::Lost };

            let return_amount = trade.bet_size + pnl;
            inner.balance += if return_amount > Decimal::ZERO { return_amount } else { Decimal::ZERO };
            trade.balance_after = inner.balance;

            if pnl > Decimal::ZERO {
                inner.win_count += 1;
                inner.consecutive_losses = 0;
            } else {
                inner.loss_count += 1;
                inner.consecutive_losses += 1;
            }

            if let Some(t) = inner.trades.iter_mut().find(|t| t.id == trade.id) {
                *t = trade.clone();
            }

            closed.push(trade);
        }

        closed
    }

    pub fn add_api_cost(&self, cost: Decimal) {
        self.inner.lock().unwrap().total_api_cost += cost;
    }

    pub fn stats_with_markets(&self, markets: &[Market]) -> PortfolioStats {
        let inner = self.inner.lock().unwrap();
        let elapsed = Utc::now() - inner.start_time;
        let total_trades = inner.win_count + inner.loss_count;
        let win_rate = if total_trades > 0 {
            (inner.win_count as f64 / total_trades as f64) * 100.0
        } else {
            0.0
        };
        let realized_pnl = inner.balance - inner.initial_balance;

        // Calculate locked balance + unrealized PnL from open positions
        let locked_balance: Decimal = inner.open_trades.iter().map(|t| t.bet_size).sum();
        let mut unrealized_pnl = Decimal::ZERO;
        for trade in &inner.open_trades {
            if let Some(market) = markets.iter().find(|m| m.id == trade.market_id) {
                let current_price = match trade.direction {
                    Direction::Yes => market.yes_price,
                    Direction::No => Decimal::ONE - market.yes_price,
                    Direction::Skip => continue,
                };
                unrealized_pnl += (current_price - trade.entry_price) * trade.shares;
            }
        }

        let total_pnl = realized_pnl + unrealized_pnl;
        let roi = if inner.initial_balance > Decimal::ZERO {
            (total_pnl / inner.initial_balance * Decimal::from(100)).round_dp(1)
        } else {
            Decimal::ZERO
        };

        PortfolioStats {
            balance: inner.balance,
            initial_balance: inner.initial_balance,
            realized_pnl,
            unrealized_pnl,
            total_pnl,
            roi,
            peak_balance: inner.peak_balance,
            max_drawdown_pct: (inner.max_drawdown * Decimal::from(100)).round_dp(1),
            win_count: inner.win_count,
            loss_count: inner.loss_count,
            win_rate,
            total_api_cost: inner.total_api_cost,
            open_positions: inner.open_trades.len(),
            locked_balance,
            elapsed_hours: elapsed.num_minutes() as f64 / 60.0,
            consecutive_losses: inner.consecutive_losses,
            recent_trades: inner.trades.iter().rev().take(5).cloned().collect(),
        }
    }

    pub fn stats(&self) -> PortfolioStats {
        self.stats_with_markets(&[])
    }

    /// Get all closed trades (for reporting)
    pub fn closed_trades(&self) -> Vec<Trade> {
        let inner = self.inner.lock().unwrap();
        inner.trades.iter()
            .filter(|t| t.status == TradeStatus::Won || t.status == TradeStatus::Lost)
            .cloned()
            .collect()
    }

    /// Get all open trades (for reporting)
    pub fn open_trades(&self) -> Vec<Trade> {
        let inner = self.inner.lock().unwrap();
        inner.open_trades.clone()
    }

    /// Get total trades count (open + closed)
    pub fn total_trade_count(&self) -> usize {
        let inner = self.inner.lock().unwrap();
        inner.trades.len()
    }
}

#[derive(Debug, Clone)]
pub struct PortfolioStats {
    pub balance: Decimal,
    pub initial_balance: Decimal,
    pub realized_pnl: Decimal,
    pub unrealized_pnl: Decimal,
    pub total_pnl: Decimal,
    pub roi: Decimal,
    pub peak_balance: Decimal,
    pub max_drawdown_pct: Decimal,
    pub win_count: u32,
    pub loss_count: u32,
    pub win_rate: f64,
    pub total_api_cost: Decimal,
    pub open_positions: usize,
    pub locked_balance: Decimal,
    pub elapsed_hours: f64,
    pub consecutive_losses: u32,
    pub recent_trades: Vec<Trade>,
}

impl std::fmt::Display for PortfolioStats {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let roi_str = if self.roi >= Decimal::ZERO {
            format!("+{}%", self.roi)
        } else {
            format!("{}%", self.roi)
        };

        let rpnl_sign = if self.realized_pnl >= Decimal::ZERO { "+" } else { "" };
        let upnl_sign = if self.unrealized_pnl >= Decimal::ZERO { "+" } else { "" };

        write!(
            f,
            "\
╔══════════════════════════════════════════╗
║   POLYMARKET AGENT — PRICE TRACKING      ║
╠══════════════════════════════════════════╣
║  Runtime:     {:.1}h
║  Balance:     ${} (start ${})
║  Locked:      ${} ({} open positions)
║  Available:   ${}
║  Realized:    {}${}
║  Unrealized:  {}${} ({} open)
║  Total P&L:   ${} ({})
║  Peak:        ${}
║  Max DD:      {}%
║  Trades:      {} closed (W:{} L:{} = {:.0}%)
║  API Cost:    ${}
║  Loss Streak: {}
╚══════════════════════════════════════════╝",
            self.elapsed_hours,
            self.balance,
            self.initial_balance,
            self.locked_balance,
            self.open_positions,
            self.balance - self.locked_balance,
            rpnl_sign,
            self.realized_pnl,
            upnl_sign,
            self.unrealized_pnl,
            self.open_positions,
            self.total_pnl,
            roi_str,
            self.peak_balance,
            self.max_drawdown_pct,
            self.win_count + self.loss_count,
            self.win_count,
            self.loss_count,
            self.win_rate,
            self.total_api_cost,
            self.consecutive_losses,
        )
    }
}
