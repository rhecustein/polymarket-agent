use crate::paper::PortfolioStats;
use crate::types::Trade;
use anyhow::Result;
use serde::Deserialize;
use tracing::{debug, info, warn};

/// Telegram Bot API alert sender
pub struct TelegramAlert {
    bot_token: String,
    chat_id: String,
    client: reqwest::Client,
}

impl TelegramAlert {
    pub fn new(bot_token: &str, chat_id: &str) -> Self {
        Self {
            bot_token: bot_token.to_string(),
            chat_id: chat_id.to_string(),
            client: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(10))
                .build()
                .expect("HTTP client"),
        }
    }

    pub fn is_configured(&self) -> bool {
        !self.bot_token.is_empty() && !self.chat_id.is_empty()
    }

    /// Send a plain text message
    pub async fn send_message(&self, text: &str) -> Result<()> {
        if !self.is_configured() {
            return Ok(());
        }

        let url = format!(
            "https://api.telegram.org/bot{}/sendMessage",
            self.bot_token
        );

        let body = serde_json::json!({
            "chat_id": self.chat_id,
            "text": text,
            "parse_mode": "HTML",
            "disable_web_page_preview": true,
        });

        let resp = self.client.post(&url).json(&body).send().await?;

        if resp.status().is_success() {
            debug!("Telegram message sent");
        } else {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            warn!("Telegram send failed {status}: {}", &body[..body.len().min(200)]);
        }

        Ok(())
    }

    /// Send a trade alert (basic version - send_paper_trade_alert is preferred for v2.0)
    #[allow(dead_code)]
    pub async fn send_trade_alert(&self, trade: &Trade) -> Result<()> {
        if !self.is_configured() {
            return Ok(());
        }

        let emoji = match trade.direction {
            crate::types::Direction::Yes => "UP",
            crate::types::Direction::No => "DOWN",
            crate::types::Direction::Skip => return Ok(()),
        };

        let text = format!(
            "<b>NEW TRADE [{emoji}]</b>\n\
            {dir} | ${size} @ {price}\n\
            Edge: {edge} | Fair: {fair}\n\
            <i>{q}</i>",
            dir = trade.direction,
            size = trade.bet_size,
            price = trade.entry_price,
            edge = trade.edge,
            fair = trade.fair_value,
            q = &trade.question[..trade.question.len().min(80)],
        );

        self.send_message(&text).await
    }

    /// Send enriched paper trading alert with agent trail
    pub async fn send_paper_trade_alert(&self, trade: &Trade) -> Result<()> {
        if !self.is_configured() {
            return Ok(());
        }

        let dir_label = match trade.direction {
            crate::types::Direction::Yes => "YES",
            crate::types::Direction::No => "NO",
            crate::types::Direction::Skip => return Ok(()),
        };

        let mode = trade.trade_mode.as_deref().unwrap_or("?");
        let desk = trade.specialist_desk.as_deref().unwrap_or("?");
        let judge = trade.judge_model.as_deref().unwrap_or("?");
        let conf = trade.judge_confidence.unwrap_or(0.0);

        let text = format!(
            "<b>PAPER TRADE OPENED</b>\n\
            {dir} {cat} '{q}'\n\
            Price: ${price} | Size: ${size}\n\
            Mode: {mode} | Edge: {edge}%\n\
            Conf: {conf:.0}% | Judge: {judge}\n\
            Desk: {desk}",
            dir = dir_label,
            cat = trade.category.as_deref().unwrap_or(""),
            q = &trade.question[..trade.question.len().min(60)],
            price = trade.entry_price,
            size = trade.bet_size,
            mode = mode,
            edge = (trade.edge * rust_decimal::Decimal::from(100)).round_dp(1),
            conf = conf * 100.0,
            judge = judge,
            desk = desk,
        );

        self.send_message(&text).await
    }

    /// Send trade closed alert with TP/SL-specific rich notifications
    /// Send detailed trade closed notification
    pub async fn send_trade_closed_alert(&self, trade: &Trade) -> Result<()> {
        if !self.is_configured() {
            return Ok(());
        }

        let pnl_sign = if trade.pnl > rust_decimal::Decimal::ZERO { "+" } else { "" };
        let hold = trade.hold_duration_hours.unwrap_or(0.0);
        let exit_price = trade.exit_price.unwrap_or_default();
        let pnl_pct = if trade.bet_size > rust_decimal::Decimal::ZERO {
            (trade.pnl / trade.bet_size * rust_decimal::Decimal::from(100)).to_string().parse::<f64>().unwrap_or(0.0)
        } else { 0.0 };

        let reason = trade.exit_reason;
        let mode = trade.trade_mode.as_deref().unwrap_or("?");

        // TP/SL-specific header with distinct formatting
        let (header, target_line) = match reason {
            Some(crate::types::ExitReason::TakeProfit) => {
                let tp_str = trade.take_profit
                    .map(|tp| format!("TP Target: ${} (HIT)", tp))
                    .unwrap_or_else(|| "TP Target: HIT".to_string());
                (
                    format!("<b>[TP HIT] TRADE CLOSED</b>"),
                    format!("\n{}", tp_str),
                )
            }
            Some(crate::types::ExitReason::StopLoss) => {
                let sl_str = trade.stop_loss
                    .map(|sl| format!("SL Level: ${} (TRIGGERED)", sl))
                    .unwrap_or_else(|| "SL Level: TRIGGERED".to_string());
                (
                    format!("<b>[SL HIT] TRADE CLOSED</b>"),
                    format!("\n{}", sl_str),
                )
            }
            Some(crate::types::ExitReason::EdgeCaptured) => (
                format!("<b>[EDGE CAPTURED] TRADE CLOSED</b>"),
                String::new(),
            ),
            Some(crate::types::ExitReason::SafetyValve) => (
                format!("<b>[SAFETY VALVE] TRADE CLOSED</b>"),
                String::new(),
            ),
            Some(crate::types::ExitReason::TimeExpiry) => (
                format!("<b>[TIME EXPIRY] TRADE CLOSED</b>"),
                String::new(),
            ),
            Some(crate::types::ExitReason::MarketResolved) => (
                format!("<b>[MARKET RESOLVED] TRADE CLOSED</b>"),
                String::new(),
            ),
            _ => {
                let emoji = if trade.pnl > rust_decimal::Decimal::ZERO { "WIN" } else { "LOSS" };
                (format!("<b>[{emoji}] TRADE CLOSED</b>"), String::new())
            }
        };

        let win_loss = if trade.pnl > rust_decimal::Decimal::ZERO { "WIN" } else { "LOSS" };

        let text = format!(
            "{header}\n\
            {dir} [{mode}] '{q}'\n\
            Entry: ${entry} → Exit: ${exit} ({pnl_sign}{pnl_pct:.1}%)\n\
            P&amp;L: {pnl_sign}${pnl} | {win_loss} | Held: {hold:.1}h{target}",
            dir = trade.direction,
            q = &trade.question[..trade.question.len().min(60)],
            entry = trade.entry_price,
            exit = exit_price,
            pnl = trade.pnl.abs(),
            target = target_line,
        );

        self.send_message(&text).await
    }

    /// Send portfolio status (for /status command)
    pub async fn send_status(&self, stats: &PortfolioStats, open_trades: &[Trade]) -> Result<()> {
        if !self.is_configured() {
            return Ok(());
        }

        let pnl_sign = if stats.total_pnl > rust_decimal::Decimal::ZERO { "+" } else { "" };
        let mut text = format!(
            "<b>PAPER TRADING STATUS</b>\n\
            Balance: <b>${balance}</b> ({pnl_sign}{roi}%)\n\
            Open P&amp;L: ${unreal}\n\
            Total Value: ${total}\n\n\
            Stats ({total_trades} trades):\n\
            Win Rate: {wr:.0}% ({w}W / {l}L)\n\
            Max DD: {dd}%\n\
            Streak: {streak}",
            balance = stats.balance,
            roi = stats.roi,
            unreal = stats.unrealized_pnl,
            total = stats.balance + stats.unrealized_pnl,
            total_trades = stats.win_count + stats.loss_count,
            wr = stats.win_rate,
            w = stats.win_count,
            l = stats.loss_count,
            dd = stats.max_drawdown_pct,
            streak = stats.consecutive_losses,
        );

        if !open_trades.is_empty() {
            text.push_str(&format!("\n\nOpen Positions: {}", open_trades.len()));
            for t in open_trades.iter().take(5) {
                let dir = format!("{}", t.direction);
                let mode = t.trade_mode.as_deref().unwrap_or("?");
                text.push_str(&format!(
                    "\n  {} '{}' ({})",
                    dir,
                    &t.question[..t.question.len().min(30)],
                    mode,
                ));
            }
        }

        text.push_str(&format!("\n\nRunning: {:.1}h | API: ${}", stats.elapsed_hours, stats.total_api_cost));
        self.send_message(&text).await
    }

    /// Send daily performance summary
    pub async fn send_daily_summary(&self, stats: &PortfolioStats, cycles: u32) -> Result<()> {
        if !self.is_configured() {
            return Ok(());
        }

        let pnl_emoji = if stats.total_pnl > rust_decimal::Decimal::ZERO {
            "+"
        } else {
            ""
        };

        let text = format!(
            "<b>DAILY REPORT</b>\n\
            Balance: <b>${balance}</b> ({pnl_emoji}${pnl})\n\
            ROI: {roi}% | Peak: ${peak}\n\
            Max DD: {dd}%\n\
            Trades: {total} (W:{w} L:{l} = {wr:.0}%)\n\
            API Cost: ${api}\n\
            Cycles: {cycles} | Streak: {streak}",
            balance = stats.balance,
            pnl = stats.total_pnl,
            roi = stats.roi,
            peak = stats.peak_balance,
            dd = stats.max_drawdown_pct,
            total = stats.win_count + stats.loss_count,
            w = stats.win_count,
            l = stats.loss_count,
            wr = stats.win_rate,
            api = stats.total_api_cost,
            streak = stats.consecutive_losses,
        );

        self.send_message(&text).await
    }

    /// Send critical alert (PAUSED, DEAD, etc.)
    pub async fn send_critical_alert(&self, message: &str) -> Result<()> {
        if !self.is_configured() {
            return Ok(());
        }

        let text = format!("<b>CRITICAL ALERT</b>\n{message}");
        self.send_message(&text).await
    }

    /// Poll for Telegram commands using getUpdates long polling.
    /// Returns a list of commands received since last_update_id.
    pub async fn poll_commands(&self, last_update_id: &mut i64) -> Vec<TelegramCommand> {
        if !self.is_configured() {
            return vec![];
        }

        let url = format!(
            "https://api.telegram.org/bot{}/getUpdates?offset={}&timeout=1&allowed_updates=[\"message\"]",
            self.bot_token, *last_update_id + 1
        );

        #[derive(Deserialize)]
        struct TgResponse {
            ok: bool,
            result: Option<Vec<TgUpdate>>,
        }

        #[derive(Deserialize)]
        struct TgUpdate {
            update_id: i64,
            message: Option<TgMessage>,
        }

        #[derive(Deserialize)]
        struct TgMessage {
            text: Option<String>,
            chat: TgChat,
        }

        #[derive(Deserialize)]
        struct TgChat {
            id: i64,
        }

        let resp = match self.client.get(&url).send().await {
            Ok(r) => r,
            Err(_) => return vec![],
        };

        let data: TgResponse = match resp.json().await {
            Ok(d) => d,
            Err(_) => return vec![],
        };

        if !data.ok {
            return vec![];
        }

        let mut commands = vec![];
        for update in data.result.unwrap_or_default() {
            *last_update_id = update.update_id;
            if let Some(msg) = update.message {
                let chat_id_str = msg.chat.id.to_string();
                if chat_id_str != self.chat_id {
                    continue; // Ignore messages from other chats
                }
                if let Some(text) = msg.text {
                    let cmd = text.trim().to_lowercase();
                    match cmd.as_str() {
                        "/status" => commands.push(TelegramCommand::Status),
                        "/stop" => commands.push(TelegramCommand::Stop),
                        "/trades" => commands.push(TelegramCommand::Trades),
                        "/open" => commands.push(TelegramCommand::OpenPositions),
                        "/help" => commands.push(TelegramCommand::Help),
                        _ => {}
                    }
                }
            }
        }

        if !commands.is_empty() {
            info!("Telegram: {} command(s) received", commands.len());
        }

        commands
    }
}

/// Telegram commands that can be received from the bot
#[derive(Debug, Clone)]
pub enum TelegramCommand {
    Status,
    Stop,
    Trades,
    OpenPositions,
    Help,
}
