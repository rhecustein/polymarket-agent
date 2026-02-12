use super::clob::ClobClient;
use crate::types::{Direction, Market, Trade, TradeStatus};
use anyhow::{Context, Result};
use chrono::Utc;
use ethers::signers::{LocalWallet, Signer};
use rust_decimal::Decimal;
use tracing::{debug, info, warn};

/// Live trading engine that places real orders on Polymarket CLOB
pub struct LiveEngine {
    clob: ClobClient,
    wallet: LocalWallet,
    balance: Decimal,
    initial_balance: Decimal,
    trades: Vec<Trade>,
    win_count: u32,
    loss_count: u32,
    consecutive_losses: u32,
    total_api_cost: Decimal,
    peak_balance: Decimal,
    max_drawdown: Decimal,
}

impl LiveEngine {
    pub fn new(clob_url: &str, private_key: &str, initial_balance: Decimal) -> Result<Self> {
        let wallet: LocalWallet = private_key
            .parse()
            .context("Invalid wallet private key")?;

        info!(
            "Live engine initialized with wallet: {:?}",
            wallet.address()
        );

        Ok(Self {
            clob: ClobClient::new(clob_url),
            wallet,
            balance: initial_balance,
            initial_balance,
            trades: Vec::new(),
            win_count: 0,
            loss_count: 0,
            consecutive_losses: 0,
            total_api_cost: Decimal::ZERO,
            peak_balance: initial_balance,
            max_drawdown: Decimal::ZERO,
        })
    }

    /// Get current CLOB price for a market's YES token
    pub async fn get_clob_price(&self, market: &Market) -> Result<Decimal> {
        let yes_token = market
            .tokens
            .iter()
            .find(|t| t.outcome == "Yes")
            .ok_or_else(|| anyhow::anyhow!("No YES token found for market {}", market.id))?;

        self.clob.get_price(&yes_token.token_id).await
    }

    /// Place a limit order and wait for fill
    pub async fn execute_trade(
        &mut self,
        market: &Market,
        direction: Direction,
        fair_value: Decimal,
        edge: Decimal,
        bet_size: Decimal,
    ) -> Result<Trade> {
        let token = match direction {
            Direction::Yes => market.tokens.iter().find(|t| t.outcome == "Yes"),
            Direction::No => market.tokens.iter().find(|t| t.outcome == "No"),
            Direction::Skip => return Err(anyhow::anyhow!("Cannot trade Skip direction")),
        };

        let token = token
            .ok_or_else(|| anyhow::anyhow!("Token not found for direction {direction}"))?;

        // Get order book to determine limit price
        let book = self.clob.get_order_book(&token.token_id).await?;
        let limit_price = book.best_ask; // Buy at best ask

        if limit_price <= Decimal::ZERO || limit_price >= Decimal::ONE {
            anyhow::bail!("Invalid limit price: {limit_price}");
        }

        let shares = (bet_size / limit_price).round_dp(2);

        info!(
            "LIVE ORDER: {} {} shares @ {} for {}",
            direction,
            shares,
            limit_price,
            &market.question[..market.question.len().min(50)]
        );

        // Place limit order
        let wallet_hex = format!("{:?}", self.wallet.address());
        let order_id = self
            .clob
            .place_order(
                &token.token_id,
                "BUY",
                limit_price,
                shares,
                &wallet_hex,
            )
            .await?;

        info!("Order placed: {order_id}");

        // Poll for fill (5s intervals, 60s timeout)
        let mut filled = false;
        for _ in 0..12 {
            tokio::time::sleep(std::time::Duration::from_secs(5)).await;
            match self.clob.get_order_status(&order_id).await {
                Ok(status) => {
                    debug!("Order {order_id} status: {status}");
                    if status == "FILLED" || status == "filled" {
                        filled = true;
                        break;
                    } else if status == "CANCELLED" || status == "cancelled" || status == "EXPIRED" {
                        warn!("Order {order_id} was {status}");
                        break;
                    }
                }
                Err(e) => warn!("Order status check failed: {e}"),
            }
        }

        if !filled {
            // Cancel unfilled order
            warn!("Order {order_id} not filled after 60s, cancelling");
            self.clob.cancel_order(&order_id).await.ok();

            let trade = Trade {
                id: uuid::Uuid::new_v4().to_string()[..8].to_string(),
                timestamp: Utc::now(),
                market_id: market.id.clone(),
                question: market.question.clone(),
                direction,
                entry_price: limit_price,
                fair_value,
                edge,
                bet_size,
                shares,
                status: TradeStatus::Cancelled,
                exit_price: None,
                pnl: Decimal::ZERO,
                balance_after: self.balance,
                order_id: Some(order_id),
                trade_mode: None, take_profit: None, stop_loss: None,
                max_hold_until: None, category: None, specialist_desk: None,
                bull_probability: None, bear_probability: None,
                judge_fair_value: None, judge_confidence: None, judge_model: None,
                exit_reason: None, hold_duration_hours: None, token_id: None,
            };

            self.trades.push(trade.clone());
            return Ok(trade);
        }

        // Trade filled
        self.balance -= bet_size;
        if self.balance > self.peak_balance {
            self.peak_balance = self.balance;
        }

        let trade = Trade {
            id: uuid::Uuid::new_v4().to_string()[..8].to_string(),
            timestamp: Utc::now(),
            market_id: market.id.clone(),
            question: market.question.clone(),
            direction,
            entry_price: limit_price,
            fair_value,
            edge,
            bet_size,
            shares,
            status: TradeStatus::Open,
            exit_price: None,
            pnl: Decimal::ZERO,
            balance_after: self.balance,
            order_id: Some(order_id),
            trade_mode: None, take_profit: None, stop_loss: None,
            max_hold_until: None, category: None, specialist_desk: None,
            bull_probability: None, bear_probability: None,
            judge_fair_value: None, judge_confidence: None, judge_model: None,
            exit_reason: None, hold_duration_hours: None, token_id: None,
        };

        self.trades.push(trade.clone());
        info!(
            "FILLED: {} {} @ {} | ${bet_size}",
            direction,
            &market.question[..market.question.len().min(40)],
            limit_price
        );

        Ok(trade)
    }

    /// Check resolution status of open trades
    pub async fn check_resolutions(&mut self) -> Vec<Trade> {
        let resolved = Vec::new();

        for trade in &mut self.trades {
            if trade.status != TradeStatus::Open {
                continue;
            }

            // Check market resolution via CLOB
            // In production, this would query the market resolution endpoint
            // For now, we check if the token price has moved to 0 or 1
            let token_id = trade
                .order_id
                .as_deref()
                .unwrap_or("");

            if token_id.is_empty() {
                continue;
            }

            // This is a simplified check - in production you'd query
            // the market resolution status endpoint
            debug!("Checking resolution for trade {}", trade.id);
        }

        resolved
    }

    pub fn record_win(&mut self, trade_id: &str, payout: Decimal) {
        if let Some(trade) = self.trades.iter_mut().find(|t| t.id == trade_id) {
            trade.status = TradeStatus::Won;
            trade.exit_price = Some(Decimal::ONE);
            trade.pnl = payout - trade.bet_size;
            self.balance += payout;
            self.win_count += 1;
            self.consecutive_losses = 0;
            trade.balance_after = self.balance;

            if self.balance > self.peak_balance {
                self.peak_balance = self.balance;
            }
        }
    }

    pub fn record_loss(&mut self, trade_id: &str) {
        if let Some(trade) = self.trades.iter_mut().find(|t| t.id == trade_id) {
            trade.status = TradeStatus::Lost;
            trade.exit_price = Some(Decimal::ZERO);
            trade.pnl = -trade.bet_size;
            self.loss_count += 1;
            self.consecutive_losses += 1;
            trade.balance_after = self.balance;

            // Update drawdown
            if self.peak_balance > Decimal::ZERO {
                let dd = (self.peak_balance - self.balance) / self.peak_balance;
                if dd > self.max_drawdown {
                    self.max_drawdown = dd;
                }
            }
        }
    }

    pub fn balance(&self) -> Decimal {
        self.balance
    }

    pub fn consecutive_losses(&self) -> u32 {
        self.consecutive_losses
    }

    pub fn add_api_cost(&mut self, cost: Decimal) {
        self.total_api_cost += cost;
    }
}
