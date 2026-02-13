use crate::config::Config;
use crate::types::{Direction, ExitReason, Market, Trade, TradeStatus};
use chrono::Utc;
use rand::Rng;
use rust_decimal::Decimal;
use rust_decimal::prelude::*;
use std::sync::Mutex;
use tracing::{info, warn};

#[derive(Debug, Clone)]
pub struct SimConfig {
    pub fees_enabled: bool,
    pub slippage_enabled: bool,
    pub fills_enabled: bool,
    pub impact_enabled: bool,
    pub gas_fee_min: Decimal,
    pub gas_fee_max: Decimal,
    pub platform_fee_pct: Decimal,
    pub maker_fee_pct: Decimal,
    pub taker_fee_pct: Decimal,
    pub base_slippage_pct: Decimal,
    pub size_penalty_pct: Decimal,
    pub size_penalty_threshold: Decimal,
    pub reject_probability: Decimal,
    pub partial_fill_probability: Decimal,
    pub min_liquidity_volume: Decimal,
    pub impact_threshold: Decimal,
    pub impact_per_dollar_pct: Decimal,
}

impl SimConfig {
    pub fn from_config(cfg: &Config) -> Self {
        Self {
            fees_enabled: cfg.sim_fees_enabled,
            slippage_enabled: cfg.sim_slippage_enabled,
            fills_enabled: cfg.sim_fills_enabled,
            impact_enabled: cfg.sim_impact_enabled,
            gas_fee_min: cfg.sim_gas_fee_min,
            gas_fee_max: cfg.sim_gas_fee_max,
            platform_fee_pct: cfg.sim_platform_fee_pct,
            maker_fee_pct: cfg.sim_maker_fee_pct,
            taker_fee_pct: cfg.sim_taker_fee_pct,
            base_slippage_pct: cfg.sim_base_slippage_pct,
            size_penalty_pct: cfg.sim_size_penalty_pct,
            size_penalty_threshold: cfg.sim_size_penalty_threshold,
            reject_probability: cfg.sim_reject_probability,
            partial_fill_probability: cfg.sim_partial_fill_probability,
            min_liquidity_volume: cfg.sim_min_liquidity_volume,
            impact_threshold: cfg.sim_impact_threshold,
            impact_per_dollar_pct: cfg.sim_impact_per_dollar_pct,
        }
    }

    pub fn disabled() -> Self {
        Self {
            fees_enabled: false,
            slippage_enabled: false,
            fills_enabled: false,
            impact_enabled: false,
            gas_fee_min: Decimal::ZERO,
            gas_fee_max: Decimal::ZERO,
            platform_fee_pct: Decimal::ZERO,
            maker_fee_pct: Decimal::ZERO,
            taker_fee_pct: Decimal::ZERO,
            base_slippage_pct: Decimal::ZERO,
            size_penalty_pct: Decimal::ZERO,
            size_penalty_threshold: Decimal::ZERO,
            reject_probability: Decimal::ZERO,
            partial_fill_probability: Decimal::ZERO,
            min_liquidity_volume: Decimal::ZERO,
            impact_threshold: Decimal::ZERO,
            impact_per_dollar_pct: Decimal::ZERO,
        }
    }
}

/// Random gas fee between min and max
fn random_gas_fee(min: Decimal, max: Decimal) -> Decimal {
    if min >= max { return min; }
    let mut rng = rand::thread_rng();
    let min_f = min.to_f64().unwrap_or(0.01);
    let max_f = max.to_f64().unwrap_or(0.05);
    let val = rng.gen_range(min_f..=max_f);
    Decimal::from_f64(val).unwrap_or(min).round_dp(4)
}

/// Calculate slippage percentage based on bet size and spread
fn calculate_slippage_pct(sim: &SimConfig, bet_size: Decimal, spread: Decimal) -> Decimal {
    let spread_factor = if spread > Decimal::ZERO {
        (spread / Decimal::new(5, 2)).min(Decimal::from(3)) // normalize to 0.05 spread, cap 3x
    } else {
        Decimal::ONE
    };
    let mut slippage = sim.base_slippage_pct * spread_factor;
    if bet_size > sim.size_penalty_threshold {
        let excess = bet_size - sim.size_penalty_threshold;
        slippage += sim.size_penalty_pct * excess;
    }
    slippage
}

/// Calculate market impact percentage for large orders
fn calculate_impact_pct(sim: &SimConfig, bet_size: Decimal) -> Decimal {
    if bet_size <= sim.impact_threshold {
        return Decimal::ZERO;
    }
    let excess = bet_size - sim.impact_threshold;
    sim.impact_per_dollar_pct * excess
}

/// Simulate fill: returns None if rejected, Some(adjusted_size) if filled (possibly partial)
fn simulate_fill(sim: &SimConfig, bet_size: Decimal, volume: Decimal) -> Option<Decimal> {
    let mut rng = rand::thread_rng();
    let roll: f64 = rng.gen();

    // Reject with probability
    let reject_prob = sim.reject_probability.to_f64().unwrap_or(0.05);
    if roll < reject_prob {
        return None;
    }

    // Low liquidity = higher chance of partial fill
    let partial_prob = sim.partial_fill_probability.to_f64().unwrap_or(0.15);
    let liquidity_factor = if volume < sim.min_liquidity_volume && volume > Decimal::ZERO {
        2.0 // double partial fill probability for low liquidity
    } else {
        1.0
    };

    let roll2: f64 = rng.gen();
    if roll2 < partial_prob * liquidity_factor {
        // Partial fill: 50-90% of original
        let fill_pct = rng.gen_range(0.50..=0.90);
        let filled = bet_size * Decimal::from_f64(fill_pct).unwrap_or(Decimal::new(7, 1));
        return Some(filled.round_dp(4));
    }

    Some(bet_size)
}

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
        sim: &SimConfig,
        market_volume: Decimal,
    ) -> Option<Trade> {
        let mut inner = self.inner.lock().unwrap();

        if bet_size > inner.balance {
            bet_size = inner.balance;
        }
        if bet_size <= Decimal::ZERO {
            return None;
        }

        // Sim: fill simulation (rejection / partial fill)
        if sim.fills_enabled {
            match simulate_fill(sim, bet_size, market_volume) {
                None => {
                    warn!("SIM ORDER REJECTED: {} (reject_prob={})", &question[..question.len().min(40)], sim.reject_probability);
                    return None;
                }
                Some(filled_size) => {
                    if filled_size < bet_size {
                        info!("SIM PARTIAL FILL: ${:.4} of ${:.4} (vol=${})", filled_size, bet_size, market_volume);
                    }
                    bet_size = filled_size;
                }
            }
        }

        let raw_price = match direction {
            Direction::Yes => market_yes_price,
            Direction::No => Decimal::ONE - market_yes_price,
            Direction::Skip => return None,
        };

        if raw_price <= Decimal::ZERO {
            return None;
        }

        // Sim: market impact
        let impact_pct = if sim.impact_enabled {
            calculate_impact_pct(sim, bet_size)
        } else {
            Decimal::ZERO
        };

        // Sim: slippage
        let spread = (market_yes_price - (Decimal::ONE - market_yes_price)).abs();
        let slippage_pct = if sim.slippage_enabled {
            calculate_slippage_pct(sim, bet_size, spread)
        } else {
            Decimal::ZERO
        };

        // Adjusted entry price (buying = worse fill = higher price for YES, lower for NO)
        let adjustment = slippage_pct + impact_pct;
        let adjusted_price = match direction {
            Direction::Yes => (raw_price * (Decimal::ONE + adjustment)).min(Decimal::new(99, 2)),
            Direction::No => (raw_price * (Decimal::ONE + adjustment)).min(Decimal::new(99, 2)),
            Direction::Skip => unreachable!(),
        };

        let entry_slippage_cost = (adjusted_price - raw_price).abs() * bet_size / adjusted_price;

        // Sim: gas fee
        let gas_fee = if sim.fees_enabled {
            random_gas_fee(sim.gas_fee_min, sim.gas_fee_max)
        } else {
            Decimal::ZERO
        };

        // Sim: maker/taker fee
        let maker_taker = if sim.fees_enabled {
            bet_size * sim.taker_fee_pct
        } else {
            Decimal::ZERO
        };

        // Total deduction = bet_size + gas + maker/taker
        let total_deduction = bet_size + gas_fee + maker_taker;
        if total_deduction > inner.balance {
            bet_size = (inner.balance - gas_fee - maker_taker).max(Decimal::ZERO);
            if bet_size <= Decimal::ZERO {
                return None;
            }
        }

        let shares = (bet_size / adjusted_price).round_dp(4);

        let trade = Trade {
            id: uuid::Uuid::new_v4().to_string()[..8].to_string(),
            timestamp: Utc::now(),
            market_id: market_id.to_string(),
            question: question.to_string(),
            direction,
            entry_price: adjusted_price,
            fair_value,
            edge,
            bet_size,
            shares,
            status: TradeStatus::Open,
            exit_price: None,
            pnl: Decimal::ZERO,
            balance_after: inner.balance - bet_size - gas_fee - maker_taker,
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
            raw_entry_price: Some(raw_price),
            raw_exit_price: None,
            entry_gas_fee: gas_fee,
            exit_gas_fee: Decimal::ZERO,
            entry_slippage: entry_slippage_cost,
            exit_slippage: Decimal::ZERO,
            platform_fee: Decimal::ZERO,
            maker_taker_fee: maker_taker,
        };

        inner.balance -= bet_size + gas_fee + maker_taker;
        inner.trades.push(trade.clone());
        inner.open_trades.push(trade.clone());

        if gas_fee > Decimal::ZERO || entry_slippage_cost > Decimal::ZERO {
            info!("SIM: gas=${:.4} slip=${:.4} impact={:.4}% size=${:.4}",
                gas_fee, entry_slippage_cost, impact_pct * Decimal::from(100), bet_size);
        }

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
        sim: &SimConfig,
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

                // Mode-based exit logic (uses RAW market price for trigger decisions)
                let mode = trade.trade_mode.as_deref().unwrap_or("SWING");

                match mode {
                    "SCALP" => {
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
                        if exit_reason.is_none() {
                            if let Some(max_hold) = trade.max_hold_until {
                                if Utc::now() > max_hold {
                                    exit_reason = Some(ExitReason::TimeExpiry);
                                }
                            }
                        }
                    }
                    "SWING" => {
                        if let Some(tp) = trade.take_profit {
                            if current_price >= tp {
                                exit_reason = Some(ExitReason::TakeProfit);
                            }
                        }
                        if exit_reason.is_none() {
                            if let Some(jfv) = trade.judge_fair_value {
                                let fair_dec = Decimal::from_f64(jfv).unwrap_or(trade.fair_value);
                                let total_edge = fair_dec - trade.entry_price;
                                if total_edge.abs() > Decimal::ZERO {
                                    let captured = (current_price - trade.entry_price) / total_edge;
                                    if captured >= Decimal::new(60, 2) {
                                        exit_reason = Some(ExitReason::EdgeCaptured);
                                    }
                                }
                            }
                        }
                        if exit_reason.is_none() {
                            if let Some(sl) = trade.stop_loss {
                                if current_price <= sl {
                                    exit_reason = Some(ExitReason::StopLoss);
                                }
                            }
                        }
                        if exit_reason.is_none() {
                            if let Some(max_hold) = trade.max_hold_until {
                                if Utc::now() > max_hold {
                                    exit_reason = Some(ExitReason::TimeExpiry);
                                }
                            }
                        }
                    }
                    "CONVICTION" => {
                        let unrealized_pnl = (current_price - trade.entry_price) * trade.shares;
                        let pnl_pct = if trade.bet_size > Decimal::ZERO {
                            (unrealized_pnl / trade.bet_size * Decimal::from(100)).to_f64().unwrap_or(0.0)
                        } else { 0.0 };
                        let conf = trade.judge_confidence.unwrap_or(0.5);
                        if pnl_pct < -30.0 && conf < 0.70 {
                            exit_reason = Some(ExitReason::SafetyValve);
                        }
                    }
                    _ => {
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
                current_price = trade.entry_price;
                exit_reason = Some(ExitReason::MarketResolved);
            }

            if let Some(reason) = exit_reason {
                // Store raw exit price
                trade.raw_exit_price = Some(current_price);

                // Sim: exit slippage (selling = worse fill)
                let exit_slippage_pct = if sim.slippage_enabled {
                    let spread = Decimal::new(3, 2); // estimate spread at exit
                    calculate_slippage_pct(sim, trade.bet_size, spread)
                } else {
                    Decimal::ZERO
                };

                // Actual exit price with slippage (selling = lower price)
                let actual_exit_price = current_price * (Decimal::ONE - exit_slippage_pct);
                let exit_slippage_cost = (current_price - actual_exit_price).abs() * trade.shares;

                // Gross PnL (using adjusted entry from execute_trade and slippage-adjusted exit)
                let gross_pnl = (actual_exit_price - trade.entry_price) * trade.shares;

                // Sim: exit gas fee
                let exit_gas = if sim.fees_enabled {
                    random_gas_fee(sim.gas_fee_min, sim.gas_fee_max)
                } else {
                    Decimal::ZERO
                };

                // Sim: exit maker/taker fee
                let exit_value = (actual_exit_price * trade.shares).abs();
                let exit_maker_taker = if sim.fees_enabled {
                    exit_value * sim.taker_fee_pct
                } else {
                    Decimal::ZERO
                };

                // Sim: platform fee (only on profitable trades)
                let plat_fee = if sim.fees_enabled && gross_pnl > Decimal::ZERO {
                    gross_pnl * sim.platform_fee_pct
                } else {
                    Decimal::ZERO
                };

                // Store simulation tracking
                trade.exit_slippage = exit_slippage_cost;
                trade.exit_gas_fee = exit_gas;
                trade.platform_fee = plat_fee;
                // Accumulate exit maker/taker into total
                trade.maker_taker_fee = trade.maker_taker_fee + exit_maker_taker;

                trade.exit_price = Some(actual_exit_price);
                trade.pnl = gross_pnl;
                trade.exit_reason = Some(reason);
                let hold_hours = (Utc::now() - trade.timestamp).num_minutes() as f64 / 60.0;
                trade.hold_duration_hours = Some(hold_hours);

                // Win/loss based on gross PnL (trade quality)
                if gross_pnl > Decimal::ZERO {
                    trade.status = TradeStatus::Won;
                    inner.win_count += 1;
                    inner.consecutive_losses = 0;
                } else {
                    trade.status = TradeStatus::Lost;
                    inner.loss_count += 1;
                    inner.consecutive_losses += 1;
                }

                // Return capital: bet_size + gross_pnl - exit_fees
                let return_amount = trade.bet_size + gross_pnl - exit_gas - exit_maker_taker - plat_fee;
                inner.balance += if return_amount > Decimal::ZERO { return_amount } else { Decimal::ZERO };
                trade.balance_after = inner.balance;

                if let Some(t) = inner.trades.iter_mut().find(|t| t.id == trade.id) {
                    *t = trade.clone();
                }

                let total_fees = trade.entry_gas_fee + exit_gas + trade.entry_slippage + exit_slippage_cost + plat_fee + trade.maker_taker_fee;
                let net_pnl = gross_pnl - total_fees + trade.entry_gas_fee; // entry gas already deducted from balance
                info!("CLOSED [{}]: {} {} | PnL ${} (net ${}) | Fees ${} | Reason: {} | Held {:.1}h",
                    trade.trade_mode.as_deref().unwrap_or("?"),
                    trade.direction,
                    &trade.question[..trade.question.len().min(35)],
                    gross_pnl.round_dp(4), net_pnl.round_dp(4), total_fees.round_dp(4),
                    reason, hold_hours);

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
    pub fn close_all_positions(&self, markets: &[Market], sim: &SimConfig) -> Vec<Trade> {
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

            trade.raw_exit_price = Some(current_price);

            // Sim: exit slippage
            let exit_slippage_pct = if sim.slippage_enabled {
                let spread = Decimal::new(3, 2);
                calculate_slippage_pct(sim, trade.bet_size, spread)
            } else {
                Decimal::ZERO
            };
            let actual_exit_price = current_price * (Decimal::ONE - exit_slippage_pct);
            let exit_slippage_cost = (current_price - actual_exit_price).abs() * trade.shares;

            let gross_pnl = (actual_exit_price - trade.entry_price) * trade.shares;
            let hold_hours = (Utc::now() - trade.timestamp).num_minutes() as f64 / 60.0;

            // Sim: exit fees
            let exit_gas = if sim.fees_enabled {
                random_gas_fee(sim.gas_fee_min, sim.gas_fee_max)
            } else {
                Decimal::ZERO
            };
            let exit_value = (actual_exit_price * trade.shares).abs();
            let exit_maker_taker = if sim.fees_enabled {
                exit_value * sim.taker_fee_pct
            } else {
                Decimal::ZERO
            };
            let plat_fee = if sim.fees_enabled && gross_pnl > Decimal::ZERO {
                gross_pnl * sim.platform_fee_pct
            } else {
                Decimal::ZERO
            };

            trade.exit_slippage = exit_slippage_cost;
            trade.exit_gas_fee = exit_gas;
            trade.platform_fee = plat_fee;
            trade.maker_taker_fee = trade.maker_taker_fee + exit_maker_taker;

            trade.exit_price = Some(actual_exit_price);
            trade.pnl = gross_pnl;
            trade.exit_reason = Some(ExitReason::ManualStop);
            trade.hold_duration_hours = Some(hold_hours);
            trade.status = if gross_pnl > Decimal::ZERO { TradeStatus::Won } else { TradeStatus::Lost };

            let return_amount = trade.bet_size + gross_pnl - exit_gas - exit_maker_taker - plat_fee;
            inner.balance += if return_amount > Decimal::ZERO { return_amount } else { Decimal::ZERO };
            trade.balance_after = inner.balance;

            if gross_pnl > Decimal::ZERO {
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

        // Aggregate simulation fees from all trades
        let mut total_gas = Decimal::ZERO;
        let mut total_slippage = Decimal::ZERO;
        let mut total_platform = Decimal::ZERO;
        let mut total_maker_taker = Decimal::ZERO;
        for t in &inner.trades {
            total_gas += t.entry_gas_fee + t.exit_gas_fee;
            total_slippage += t.entry_slippage + t.exit_slippage;
            total_platform += t.platform_fee;
            total_maker_taker += t.maker_taker_fee;
        }
        let total_fees = total_gas + total_slippage + total_platform + total_maker_taker;

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
            total_fees_paid: total_fees,
            total_slippage_cost: total_slippage,
            total_gas_fees: total_gas,
            total_platform_fees: total_platform,
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
    pub total_fees_paid: Decimal,
    pub total_slippage_cost: Decimal,
    pub total_gas_fees: Decimal,
    pub total_platform_fees: Decimal,
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
║  Sim Fees:    ${} (gas=${} slip=${} plat=${})
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
            self.total_fees_paid.round_dp(4),
            self.total_gas_fees.round_dp(4),
            self.total_slippage_cost.round_dp(4),
            self.total_platform_fees.round_dp(4),
            self.total_api_cost,
            self.consecutive_losses,
        )
    }
}
