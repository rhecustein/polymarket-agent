use crate::paper::{Portfolio, PortfolioStats};
use crate::types::{Trade, TradeStatus};
use anyhow::{Context, Result};
use lettre::message::header::ContentType;
use lettre::transport::smtp::authentication::Credentials;
use lettre::{AsyncSmtpTransport, AsyncTransport, Message, Tokio1Executor};
use rust_decimal::Decimal;
use rust_decimal::prelude::ToPrimitive;
use tracing::{error, info};

pub struct EmailAlert {
    from: String,
    to: String,
    transport: Option<AsyncSmtpTransport<Tokio1Executor>>,
}

impl EmailAlert {
    pub fn new(host: &str, port: u16, user: &str, pass: &str, from: &str, to: &str) -> Self {
        let transport = if !host.is_empty() && !user.is_empty() {
            AsyncSmtpTransport::<Tokio1Executor>::starttls_relay(host)
                .ok()
                .map(|builder| {
                    builder
                        .port(port)
                        .credentials(Credentials::new(user.to_string(), pass.to_string()))
                        .build()
                })
        } else {
            None
        };

        Self {
            from: from.to_string(),
            to: to.to_string(),
            transport,
        }
    }

    pub fn is_configured(&self) -> bool {
        self.transport.is_some() && !self.from.is_empty() && !self.to.is_empty()
    }

    /// Send daily summary email
    pub async fn send_daily_summary(
        &self,
        stats: &PortfolioStats,
        cycle_count: u32,
        in_survival: bool,
    ) -> Result<()> {
        if !self.is_configured() {
            info!("Email not configured, skipping daily summary");
            return Ok(());
        }

        let zero = rust_decimal::Decimal::ZERO;
        let is_profit = stats.total_pnl > zero;
        let is_loss = stats.total_pnl < zero;

        let subject = format!(
            "{} Polyagent: ${} ({}{}%) | {}W {}L",
            if is_profit { "📈" } else if is_loss { "📉" } else { "➡️" },
            stats.balance,
            if stats.roi >= zero { "+" } else { "" },
            stats.roi,
            stats.win_count,
            stats.loss_count,
        );

        let body = build_daily_html(stats, cycle_count, in_survival);
        self.send_html_email(&subject, &body).await
    }

    /// Send periodic paper trading report with detailed stats
    pub async fn send_periodic_report(
        &self,
        stats: &PortfolioStats,
        cycle_count: u64,
        in_survival: bool,
        portfolio: &Portfolio,
    ) -> Result<()> {
        if !self.is_configured() {
            return Ok(());
        }

        let zero = rust_decimal::Decimal::ZERO;
        let is_profit = stats.total_pnl > zero;
        let is_loss = stats.total_pnl < zero;

        let subject = format!(
            "{} Paper Trading: ${} ({}{}%) | {}W {}L | {:.1}h",
            if is_profit { "+" } else if is_loss { "-" } else { "=" },
            stats.balance,
            if stats.roi >= zero { "+" } else { "" },
            stats.roi,
            stats.win_count, stats.loss_count,
            stats.elapsed_hours,
        );

        // Build stats by category and mode from closed trades
        let closed = portfolio.closed_trades();
        let category_stats = build_category_stats(&closed);
        let mode_stats = build_mode_stats(&closed);
        let model_stats = build_model_stats(&closed);

        let body = build_periodic_html(stats, cycle_count, in_survival, &category_stats, &mode_stats, &model_stats, &portfolio.open_trades());
        self.send_html_email(&subject, &body).await
    }

    /// Send individual trade closed notification
    pub async fn send_trade_closed(&self, trade: &Trade) -> Result<()> {
        if !self.is_configured() {
            return Ok(());
        }

        let pnl_sign = if trade.pnl > Decimal::ZERO { "+" } else { "" };
        let pnl_color = if trade.pnl > Decimal::ZERO {
            "#10b981"
        } else if trade.pnl < Decimal::ZERO {
            "#ef4444"
        } else {
            "#6b7280"
        };

        let result_emoji = if trade.pnl > Decimal::ZERO { "✅" } else { "❌" };
        let result_label = if trade.pnl > Decimal::ZERO { "WIN" } else { "LOSS" };

        let hold_hours = trade.hold_duration_hours.unwrap_or(0.0);
        let exit_reason = trade.exit_reason
            .map(|r| format!("{:?}", r))
            .unwrap_or_else(|| "N/A".to_string());

        let pnl_pct = if trade.bet_size > Decimal::ZERO {
            (trade.pnl / trade.bet_size * Decimal::from(100))
                .to_f64()
                .unwrap_or(0.0)
        } else {
            0.0
        };

        let subject = format!(
            "{} Trade {} | {}${} ({}{:.1}%)",
            result_emoji,
            result_label,
            pnl_sign,
            trade.pnl.abs(),
            pnl_sign,
            pnl_pct.abs()
        );

        let body = build_trade_email_html(trade, pnl_color, result_label, &exit_reason, hold_hours, pnl_pct);

        self.send_html_email(&subject, &body).await
    }

    /// Send critical alert (agent dying, big loss, etc.)
    pub async fn send_alert(&self, subject: &str, body: &str) -> Result<()> {
        if !self.is_configured() {
            return Ok(());
        }
        let html = format!(
            r#"<div style="font-family:Arial,sans-serif;max-width:520px;margin:0 auto;padding:20px;background:#1a1a2e;color:#eee;border-radius:10px">
<h2 style="color:#ff4757;margin:0 0 12px">&#x1F6A8; CRITICAL ALERT</h2>
<p style="font-size:15px;line-height:1.5">{body}</p>
<hr style="border:1px solid #333;margin:16px 0">
<p style="color:#666;font-size:12px">Polymarket Agent v0.2.0</p>
</div>"#
        );
        self.send_html_email(&format!("🚨 {subject}"), &html).await
    }

    async fn send_html_email(&self, subject: &str, html_body: &str) -> Result<()> {
        let Some(ref transport) = self.transport else {
            return Ok(());
        };

        let email = Message::builder()
            .from(self.from.parse().context("Parse from address")?)
            .to(self.to.parse().context("Parse to address")?)
            .subject(subject)
            .header(ContentType::TEXT_HTML)
            .body(html_body.to_string())
            .context("Build email")?;

        match transport.send(email).await {
            Ok(_) => {
                info!("Email sent: {subject}");
                Ok(())
            }
            Err(e) => {
                error!("Email send failed: {e}");
                Err(anyhow::anyhow!("Email failed: {e}"))
            }
        }
    }
}

fn build_daily_html(stats: &PortfolioStats, cycle_count: u32, in_survival: bool) -> String {
    let zero = rust_decimal::Decimal::ZERO;
    let is_profit = stats.total_pnl > zero;
    let is_loss = stats.total_pnl < zero;
    let total_trades = stats.win_count + stats.loss_count;

    // Header: good news or bad news
    let (headline, headline_color, headline_icon) = if in_survival {
        ("SURVIVAL MODE AKTIF", "#ff6348", "&#x26A0;&#xFE0F;")
    } else if is_profit {
        ("KABAR BAIK — Kita Profit!", "#2ed573", "&#x1F389;")
    } else if is_loss {
        ("KABAR BURUK — Kita Rugi", "#ff4757", "&#x1F4C9;")
    } else {
        ("SALDO STABIL — Belum Ada Pergerakan", "#ffa502", "&#x2696;&#xFE0F;")
    };

    let pnl_sign = if is_profit { "+" } else { "" };
    let roi_sign = if stats.roi >= zero { "+" } else { "" };
    let pnl_color = if is_profit { "#2ed573" } else if is_loss { "#ff4757" } else { "#ffa502" };

    // Balance bar (visual progress from initial)
    let bar_pct = if stats.initial_balance > zero {
        let ratio = stats.balance / stats.initial_balance * rust_decimal::Decimal::from(100);
        ratio.to_string().parse::<f64>().unwrap_or(100.0).min(200.0).max(5.0)
    } else {
        100.0
    };
    let bar_color = if is_profit { "#2ed573" } else if is_loss { "#ff4757" } else { "#ffa502" };

    let trades_html = format_trades_html(&stats.recent_trades);

    let status_badge = if in_survival {
        r#"<span style="background:#ff6348;color:#fff;padding:3px 10px;border-radius:12px;font-size:12px;font-weight:bold">SURVIVAL</span>"#
    } else {
        r#"<span style="background:#2ed573;color:#1a1a2e;padding:3px 10px;border-radius:12px;font-size:12px;font-weight:bold">ACTIVE</span>"#
    };

    format!(
        r##"<!DOCTYPE html>
<html><head><meta charset="utf-8"></head>
<body style="margin:0;padding:0;background:#0f0f1a;font-family:'Segoe UI',Arial,sans-serif">
<div style="max-width:540px;margin:20px auto;background:#1a1a2e;border-radius:12px;overflow:hidden;border:1px solid #2a2a4a">

<!-- HEADER -->
<div style="background:linear-gradient(135deg,#16213e,#1a1a2e);padding:24px 28px;border-bottom:2px solid {headline_color}">
  <div style="font-size:28px;margin-bottom:6px">{headline_icon}</div>
  <h1 style="margin:0;color:{headline_color};font-size:20px;font-weight:700">{headline}</h1>
  <p style="margin:6px 0 0;color:#8a8a9a;font-size:13px">Laporan Harian &mdash; Polymarket Agent v0.2.0</p>
</div>

<!-- BALANCE HIGHLIGHT -->
<div style="padding:20px 28px;background:#16213e">
  <table width="100%" cellpadding="0" cellspacing="0" style="border-collapse:collapse">
    <tr>
      <td style="color:#8a8a9a;font-size:12px;text-transform:uppercase;letter-spacing:1px">Saldo Awal</td>
      <td style="color:#8a8a9a;font-size:12px;text-transform:uppercase;letter-spacing:1px" align="right">Saldo Sekarang</td>
    </tr>
    <tr>
      <td style="color:#ccc;font-size:22px;font-weight:700;padding-top:4px">${init}</td>
      <td style="color:#fff;font-size:28px;font-weight:700;padding-top:4px" align="right">${balance}</td>
    </tr>
  </table>
  <!-- Progress bar -->
  <div style="margin-top:12px;background:#2a2a4a;border-radius:6px;height:8px;overflow:hidden">
    <div style="width:{bar_pct:.0}%;max-width:100%;height:100%;background:{bar_color};border-radius:6px"></div>
  </div>
  <table width="100%" cellpadding="0" cellspacing="0" style="border-collapse:collapse;margin-top:8px">
    <tr>
      <td style="color:{pnl_color};font-size:15px;font-weight:600">P&amp;L: {pnl_sign}${pnl}</td>
      <td style="color:{pnl_color};font-size:15px;font-weight:600" align="right">ROI: {roi_sign}{roi}%</td>
    </tr>
  </table>
</div>

<!-- STATS TABLE -->
<div style="padding:16px 28px">
  <table width="100%" cellpadding="0" cellspacing="0" style="border-collapse:collapse">
    <tr>
      <td style="padding:8px 0;border-bottom:1px solid #2a2a4a;color:#8a8a9a;font-size:13px">Peak Balance</td>
      <td style="padding:8px 0;border-bottom:1px solid #2a2a4a;color:#eee;font-size:13px;font-weight:600" align="right">${peak}</td>
    </tr>
    <tr>
      <td style="padding:8px 0;border-bottom:1px solid #2a2a4a;color:#8a8a9a;font-size:13px">Max Drawdown</td>
      <td style="padding:8px 0;border-bottom:1px solid #2a2a4a;color:#ff6348;font-size:13px;font-weight:600" align="right">{dd}%</td>
    </tr>
    <tr>
      <td style="padding:8px 0;border-bottom:1px solid #2a2a4a;color:#8a8a9a;font-size:13px">Total Trades</td>
      <td style="padding:8px 0;border-bottom:1px solid #2a2a4a;color:#eee;font-size:13px;font-weight:600" align="right">{total} &nbsp;<span style="color:#2ed573">{w}W</span> / <span style="color:#ff4757">{l}L</span></td>
    </tr>
    <tr>
      <td style="padding:8px 0;border-bottom:1px solid #2a2a4a;color:#8a8a9a;font-size:13px">Win Rate</td>
      <td style="padding:8px 0;border-bottom:1px solid #2a2a4a;color:#eee;font-size:13px;font-weight:600" align="right">{wr:.0}%</td>
    </tr>
    <tr>
      <td style="padding:8px 0;border-bottom:1px solid #2a2a4a;color:#8a8a9a;font-size:13px">Loss Streak</td>
      <td style="padding:8px 0;border-bottom:1px solid #2a2a4a;color:{streak_color};font-size:13px;font-weight:600" align="right">{streak}</td>
    </tr>
    <tr>
      <td style="padding:8px 0;border-bottom:1px solid #2a2a4a;color:#8a8a9a;font-size:13px">Open Positions</td>
      <td style="padding:8px 0;border-bottom:1px solid #2a2a4a;color:#eee;font-size:13px;font-weight:600" align="right">{open}</td>
    </tr>
    <tr>
      <td style="padding:8px 0;color:#8a8a9a;font-size:13px">API Cost</td>
      <td style="padding:8px 0;color:#8a8a9a;font-size:13px" align="right">${api}</td>
    </tr>
  </table>
</div>

<!-- RECENT TRADES -->
<div style="padding:0 28px 16px">
  <h3 style="margin:0 0 10px;color:#8a8a9a;font-size:12px;text-transform:uppercase;letter-spacing:1px">Trade Terakhir</h3>
  <table width="100%" cellpadding="0" cellspacing="0" style="border-collapse:collapse;background:#16213e;border-radius:8px;overflow:hidden">
    <tr style="background:#12122a">
      <th style="padding:8px 10px;color:#666;font-size:11px;text-align:left;font-weight:600">STATUS</th>
      <th style="padding:8px 10px;color:#666;font-size:11px;text-align:left;font-weight:600">DIR</th>
      <th style="padding:8px 10px;color:#666;font-size:11px;text-align:right;font-weight:600">SIZE</th>
      <th style="padding:8px 10px;color:#666;font-size:11px;text-align:right;font-weight:600">P&amp;L</th>
      <th style="padding:8px 10px;color:#666;font-size:11px;text-align:left;font-weight:600">MARKET</th>
    </tr>
    {trades}
  </table>
</div>

<!-- FOOTER -->
<div style="padding:16px 28px;border-top:1px solid #2a2a4a;background:#12122a">
  <table width="100%" cellpadding="0" cellspacing="0" style="border-collapse:collapse">
    <tr>
      <td style="color:#666;font-size:12px">{status_badge} &nbsp; {cycles} cycles &bull; {hours:.1}h runtime</td>
    </tr>
  </table>
  <p style="margin:8px 0 0;color:#444;font-size:11px">Laporan berikutnya dalam 24 jam &bull; Polymarket Agent v0.2.0</p>
</div>

</div>
</body></html>"##,
        headline_icon = headline_icon,
        headline_color = headline_color,
        headline = headline,
        init = stats.initial_balance,
        balance = stats.balance,
        bar_pct = bar_pct,
        bar_color = bar_color,
        pnl_sign = pnl_sign,
        pnl = stats.total_pnl.abs(),
        pnl_color = pnl_color,
        roi_sign = roi_sign,
        roi = stats.roi,
        peak = stats.peak_balance,
        dd = stats.max_drawdown_pct,
        total = total_trades,
        w = stats.win_count,
        l = stats.loss_count,
        wr = stats.win_rate,
        streak = stats.consecutive_losses,
        streak_color = if stats.consecutive_losses >= 5 { "#ff4757" } else if stats.consecutive_losses >= 3 { "#ffa502" } else { "#eee" },
        open = stats.open_positions,
        api = stats.total_api_cost,
        trades = trades_html,
        status_badge = status_badge,
        cycles = cycle_count,
        hours = stats.elapsed_hours,
    )
}

/// Stats breakdown for a group (category, mode, model)
struct GroupStats {
    name: String,
    trades: u32,
    wins: u32,
    pnl: rust_decimal::Decimal,
}

fn build_category_stats(trades: &[Trade]) -> Vec<GroupStats> {
    let mut map = std::collections::HashMap::new();
    for t in trades {
        if t.status != TradeStatus::Won && t.status != TradeStatus::Lost { continue; }
        let cat = t.category.as_deref().unwrap_or("unknown").to_string();
        let entry = map.entry(cat).or_insert((0u32, 0u32, rust_decimal::Decimal::ZERO));
        entry.0 += 1;
        if t.status == TradeStatus::Won { entry.1 += 1; }
        entry.2 += t.pnl;
    }
    map.into_iter().map(|(name, (trades, wins, pnl))| GroupStats { name, trades, wins, pnl }).collect()
}

fn build_mode_stats(trades: &[Trade]) -> Vec<GroupStats> {
    let mut map = std::collections::HashMap::new();
    for t in trades {
        if t.status != TradeStatus::Won && t.status != TradeStatus::Lost { continue; }
        let mode = t.trade_mode.as_deref().unwrap_or("?").to_string();
        let entry = map.entry(mode).or_insert((0u32, 0u32, rust_decimal::Decimal::ZERO));
        entry.0 += 1;
        if t.status == TradeStatus::Won { entry.1 += 1; }
        entry.2 += t.pnl;
    }
    map.into_iter().map(|(name, (trades, wins, pnl))| GroupStats { name, trades, wins, pnl }).collect()
}

fn build_model_stats(trades: &[Trade]) -> Vec<GroupStats> {
    let mut map = std::collections::HashMap::new();
    for t in trades {
        if t.status != TradeStatus::Won && t.status != TradeStatus::Lost { continue; }
        let model = t.judge_model.as_deref().unwrap_or("?").to_string();
        let entry = map.entry(model).or_insert((0u32, 0u32, rust_decimal::Decimal::ZERO));
        entry.0 += 1;
        if t.status == TradeStatus::Won { entry.1 += 1; }
        entry.2 += t.pnl;
    }
    map.into_iter().map(|(name, (trades, wins, pnl))| GroupStats { name, trades, wins, pnl }).collect()
}

fn build_group_rows(groups: &[GroupStats]) -> String {
    groups.iter().map(|g| {
        let wr = if g.trades > 0 { (g.wins as f64 / g.trades as f64) * 100.0 } else { 0.0 };
        let pnl_color = if g.pnl > rust_decimal::Decimal::ZERO { "#2ed573" }
            else if g.pnl < rust_decimal::Decimal::ZERO { "#ff4757" }
            else { "#8a8a9a" };
        let pnl_sign = if g.pnl > rust_decimal::Decimal::ZERO { "+" } else { "" };
        format!(
            r#"<tr>
              <td style="padding:6px 10px;border-top:1px solid #2a2a4a;color:#ccc;font-size:12px">{name}</td>
              <td style="padding:6px 10px;border-top:1px solid #2a2a4a;color:#ccc;font-size:12px;text-align:right">{trades}</td>
              <td style="padding:6px 10px;border-top:1px solid #2a2a4a;color:#ccc;font-size:12px;text-align:right">{wr:.0}%</td>
              <td style="padding:6px 10px;border-top:1px solid #2a2a4a;color:{pnl_color};font-size:12px;text-align:right;font-weight:600">{pnl_sign}${pnl}</td>
            </tr>"#,
            name = g.name, trades = g.trades, wr = wr, pnl_color = pnl_color,
            pnl_sign = pnl_sign, pnl = g.pnl.abs()
        )
    }).collect::<Vec<_>>().join("\n")
}

fn build_open_positions_html(trades: &[Trade]) -> String {
    if trades.is_empty() {
        return r#"<tr><td colspan="4" style="padding:10px;color:#666;font-size:12px;text-align:center">No open positions</td></tr>"#.to_string();
    }
    trades.iter().take(8).map(|t| {
        let mode = t.trade_mode.as_deref().unwrap_or("?");
        let q = &t.question[..t.question.len().min(40)];
        format!(
            r#"<tr>
              <td style="padding:6px 10px;border-top:1px solid #2a2a4a;color:#ccc;font-size:12px">{dir} {mode}</td>
              <td style="padding:6px 10px;border-top:1px solid #2a2a4a;color:#ccc;font-size:12px;text-align:right">${entry}</td>
              <td style="padding:6px 10px;border-top:1px solid #2a2a4a;color:#ccc;font-size:12px;text-align:right">${size}</td>
              <td style="padding:6px 10px;border-top:1px solid #2a2a4a;color:#8a8a9a;font-size:11px">{q}</td>
            </tr>"#,
            dir = t.direction, mode = mode, entry = t.entry_price, size = t.bet_size, q = q
        )
    }).collect::<Vec<_>>().join("\n")
}

fn build_periodic_html(
    stats: &PortfolioStats, cycle_count: u64, in_survival: bool,
    cat_stats: &[GroupStats], mode_stats: &[GroupStats], model_stats: &[GroupStats],
    open_trades: &[Trade],
) -> String {
    let zero = rust_decimal::Decimal::ZERO;
    let is_profit = stats.total_pnl > zero;
    let is_loss = stats.total_pnl < zero;

    let (headline, hcolor) = if in_survival {
        ("SURVIVAL MODE", "#ff6348")
    } else if is_profit {
        ("PROFITABLE", "#2ed573")
    } else if is_loss {
        ("IN DRAWDOWN", "#ff4757")
    } else {
        ("NEUTRAL", "#ffa502")
    };

    let pnl_sign = if is_profit { "+" } else { "" };
    let roi_sign = if stats.roi >= zero { "+" } else { "" };
    let pnl_color = if is_profit { "#2ed573" } else if is_loss { "#ff4757" } else { "#ffa502" };
    let total_trades = stats.win_count + stats.loss_count;

    let cat_rows = build_group_rows(cat_stats);
    let mode_rows = build_group_rows(mode_stats);
    let model_rows = build_group_rows(model_stats);
    let open_rows = build_open_positions_html(open_trades);
    let trades_html = format_trades_html(&stats.recent_trades);

    format!(
        r##"<!DOCTYPE html><html><head><meta charset="utf-8"></head>
<body style="margin:0;padding:0;background:#0f0f1a;font-family:'Segoe UI',Arial,sans-serif">
<div style="max-width:560px;margin:20px auto;background:#1a1a2e;border-radius:12px;overflow:hidden;border:1px solid #2a2a4a">
<div style="background:linear-gradient(135deg,#16213e,#1a1a2e);padding:24px 28px;border-bottom:2px solid {hcolor}">
  <h1 style="margin:0;color:{hcolor};font-size:20px">PAPER TRADING — {headline}</h1>
  <p style="margin:6px 0 0;color:#8a8a9a;font-size:13px">Periodic Report — v2.0 Battle Test | {hours:.1}h runtime</p>
</div>
<div style="padding:20px 28px;background:#16213e">
  <table width="100%" cellpadding="0" cellspacing="0" style="border-collapse:collapse">
    <tr><td style="color:#8a8a9a;font-size:12px">Starting</td><td style="color:#8a8a9a;font-size:12px" align="right">Current</td></tr>
    <tr><td style="color:#ccc;font-size:22px;font-weight:700">${init}</td><td style="color:#fff;font-size:28px;font-weight:700" align="right">${balance}</td></tr>
  </table>
  <table width="100%" cellpadding="0" cellspacing="0" style="border-collapse:collapse;margin-top:8px">
    <tr><td style="color:{pnl_color};font-size:15px;font-weight:600">P&amp;L: {pnl_sign}${pnl}</td>
    <td style="color:{pnl_color};font-size:15px;font-weight:600" align="right">ROI: {roi_sign}{roi}%</td></tr>
  </table>
</div>
<div style="padding:16px 28px">
  <h3 style="margin:0 0 8px;color:#8a8a9a;font-size:12px;text-transform:uppercase">Trading Stats</h3>
  <table width="100%" cellpadding="0" cellspacing="0" style="border-collapse:collapse">
    <tr><td style="padding:6px 0;color:#8a8a9a;font-size:13px">Trades</td><td style="padding:6px 0;color:#eee;font-size:13px" align="right">{total} (<span style="color:#2ed573">{w}W</span>/<span style="color:#ff4757">{l}L</span> = {wr:.0}%)</td></tr>
    <tr><td style="padding:6px 0;color:#8a8a9a;font-size:13px">Open Positions</td><td style="padding:6px 0;color:#eee;font-size:13px" align="right">{open} (${locked} locked)</td></tr>
    <tr><td style="padding:6px 0;color:#8a8a9a;font-size:13px">Unrealized P&amp;L</td><td style="padding:6px 0;color:#eee;font-size:13px" align="right">${unreal}</td></tr>
    <tr><td style="padding:6px 0;color:#8a8a9a;font-size:13px">Max Drawdown</td><td style="padding:6px 0;color:#ff6348;font-size:13px" align="right">{dd}%</td></tr>
    <tr><td style="padding:6px 0;color:#8a8a9a;font-size:13px">Loss Streak</td><td style="padding:6px 0;color:#eee;font-size:13px" align="right">{streak}</td></tr>
    <tr><td style="padding:6px 0;color:#8a8a9a;font-size:13px">API Cost</td><td style="padding:6px 0;color:#8a8a9a;font-size:13px" align="right">${api}</td></tr>
    <tr><td style="padding:6px 0;color:#8a8a9a;font-size:13px">Cycles</td><td style="padding:6px 0;color:#eee;font-size:13px" align="right">{cycles}</td></tr>
  </table>
</div>
<div style="padding:0 28px 16px">
  <h3 style="margin:0 0 8px;color:#8a8a9a;font-size:12px;text-transform:uppercase">By Category</h3>
  <table width="100%" cellpadding="0" cellspacing="0" style="border-collapse:collapse;background:#16213e;border-radius:8px">
    <tr style="background:#12122a"><th style="padding:6px 10px;color:#666;font-size:11px;text-align:left">Category</th><th style="padding:6px 10px;color:#666;font-size:11px;text-align:right">Trades</th><th style="padding:6px 10px;color:#666;font-size:11px;text-align:right">WR</th><th style="padding:6px 10px;color:#666;font-size:11px;text-align:right">P&amp;L</th></tr>
    {cat_rows}
  </table>
</div>
<div style="padding:0 28px 16px">
  <h3 style="margin:0 0 8px;color:#8a8a9a;font-size:12px;text-transform:uppercase">By Trade Mode</h3>
  <table width="100%" cellpadding="0" cellspacing="0" style="border-collapse:collapse;background:#16213e;border-radius:8px">
    <tr style="background:#12122a"><th style="padding:6px 10px;color:#666;font-size:11px;text-align:left">Mode</th><th style="padding:6px 10px;color:#666;font-size:11px;text-align:right">Trades</th><th style="padding:6px 10px;color:#666;font-size:11px;text-align:right">WR</th><th style="padding:6px 10px;color:#666;font-size:11px;text-align:right">P&amp;L</th></tr>
    {mode_rows}
  </table>
</div>
<div style="padding:0 28px 16px">
  <h3 style="margin:0 0 8px;color:#8a8a9a;font-size:12px;text-transform:uppercase">By Judge Model</h3>
  <table width="100%" cellpadding="0" cellspacing="0" style="border-collapse:collapse;background:#16213e;border-radius:8px">
    <tr style="background:#12122a"><th style="padding:6px 10px;color:#666;font-size:11px;text-align:left">Model</th><th style="padding:6px 10px;color:#666;font-size:11px;text-align:right">Trades</th><th style="padding:6px 10px;color:#666;font-size:11px;text-align:right">WR</th><th style="padding:6px 10px;color:#666;font-size:11px;text-align:right">P&amp;L</th></tr>
    {model_rows}
  </table>
</div>
<div style="padding:0 28px 16px">
  <h3 style="margin:0 0 8px;color:#8a8a9a;font-size:12px;text-transform:uppercase">Open Positions</h3>
  <table width="100%" cellpadding="0" cellspacing="0" style="border-collapse:collapse;background:#16213e;border-radius:8px">
    <tr style="background:#12122a"><th style="padding:6px 10px;color:#666;font-size:11px;text-align:left">Dir/Mode</th><th style="padding:6px 10px;color:#666;font-size:11px;text-align:right">Entry</th><th style="padding:6px 10px;color:#666;font-size:11px;text-align:right">Size</th><th style="padding:6px 10px;color:#666;font-size:11px;text-align:left">Market</th></tr>
    {open_rows}
  </table>
</div>
<div style="padding:0 28px 16px">
  <h3 style="margin:0 0 8px;color:#8a8a9a;font-size:12px;text-transform:uppercase">Recent Trades</h3>
  <table width="100%" cellpadding="0" cellspacing="0" style="border-collapse:collapse;background:#16213e;border-radius:8px">
    <tr style="background:#12122a"><th style="padding:6px 10px;color:#666;font-size:11px;text-align:left">Status</th><th style="padding:6px 10px;color:#666;font-size:11px;text-align:left">Dir</th><th style="padding:6px 10px;color:#666;font-size:11px;text-align:right">Size</th><th style="padding:6px 10px;color:#666;font-size:11px;text-align:right">P&amp;L</th><th style="padding:6px 10px;color:#666;font-size:11px;text-align:left">Market</th></tr>
    {trades}
  </table>
</div>
<div style="padding:16px 28px;border-top:1px solid #2a2a4a;background:#12122a">
  <p style="margin:0;color:#666;font-size:12px">Bot running | Next report in 12h | Stop: touch STOP file</p>
  <p style="margin:4px 0 0;color:#444;font-size:11px">Polymarket Agent v2.0 — Battle Test</p>
</div>
</div></body></html>"##,
        hcolor = hcolor, headline = headline, hours = stats.elapsed_hours,
        init = stats.initial_balance, balance = stats.balance,
        pnl_color = pnl_color, pnl_sign = pnl_sign, pnl = stats.total_pnl.abs(),
        roi_sign = roi_sign, roi = stats.roi,
        total = total_trades, w = stats.win_count, l = stats.loss_count, wr = stats.win_rate,
        open = stats.open_positions, locked = stats.locked_balance,
        unreal = stats.unrealized_pnl, dd = stats.max_drawdown_pct,
        streak = stats.consecutive_losses, api = stats.total_api_cost, cycles = cycle_count,
        cat_rows = cat_rows, mode_rows = mode_rows, model_rows = model_rows,
        open_rows = open_rows, trades = trades_html,
    )
}

/// Build modern trade closed email HTML
fn build_trade_email_html(
    trade: &Trade,
    pnl_color: &str,
    result_label: &str,
    exit_reason: &str,
    hold_hours: f64,
    pnl_pct: f64,
) -> String {
    let pnl_sign = if trade.pnl > Decimal::ZERO { "+" } else { "" };
    let exit_price = trade.exit_price.unwrap_or(Decimal::ZERO);
    let mode = trade.trade_mode.as_deref().unwrap_or("N/A");
    let desk = trade.specialist_desk.as_deref().unwrap_or("GENERAL");

    // Calculate fees if available
    let total_fees = trade.entry_gas_fee + trade.exit_gas_fee + trade.platform_fee + trade.maker_taker_fee;
    let _total_slippage = trade.entry_slippage + trade.exit_slippage;
    let fees_line = if total_fees > Decimal::ZERO {
        format!(
            "<tr><td style='padding:10px;color:#6b7280;font-size:14px'>Total Fees</td><td style='padding:10px;text-align:right;color:#ef4444;font-size:14px'>${}</td></tr>",
            total_fees
        )
    } else {
        String::new()
    };

    format!(
        r#"<!DOCTYPE html>
<html><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"></head>
<body style="margin:0;padding:20px;background:#f3f4f6;font-family:-apple-system,BlinkMacSystemFont,'Segoe UI',Roboto,sans-serif">
<div style="max-width:600px;margin:0 auto;background:#ffffff;border-radius:16px;box-shadow:0 4px 6px rgba(0,0,0,0.1);overflow:hidden">

<!-- Header -->
<div style="background:linear-gradient(135deg,{pnl_color} 0%,{pnl_color}dd 100%);padding:32px;text-align:center">
  <div style="font-size:48px;margin-bottom:8px">{emoji}</div>
  <h1 style="margin:0;color:#ffffff;font-size:28px;font-weight:700">{result}</h1>
  <p style="margin:8px 0 0;color:#ffffffcc;font-size:16px">Trade Closed Notification</p>
</div>

<!-- P&L Section -->
<div style="padding:32px;background:#f9fafb;border-bottom:1px solid #e5e7eb">
  <div style="text-align:center">
    <p style="margin:0 0 8px;color:#6b7280;font-size:14px;text-transform:uppercase;letter-spacing:1px">Profit/Loss</p>
    <p style="margin:0;color:{pnl_color};font-size:42px;font-weight:700">{pnl_sign}${pnl}</p>
    <p style="margin:8px 0 0;color:{pnl_color};font-size:20px;font-weight:600">{pnl_sign}{pnl_pct:.2}%</p>
  </div>
</div>

<!-- Market Info -->
<div style="padding:24px 32px;border-bottom:1px solid #e5e7eb">
  <h3 style="margin:0 0 16px;color:#111827;font-size:16px;font-weight:600">Market</h3>
  <p style="margin:0;color:#374151;font-size:15px;line-height:1.6">{question}</p>
</div>

<!-- Trade Details -->
<div style="padding:24px 32px">
  <h3 style="margin:0 0 16px;color:#111827;font-size:16px;font-weight:600">Trade Details</h3>
  <table width="100%" cellpadding="0" cellspacing="0" style="border-collapse:collapse">
    <tr>
      <td style="padding:10px 0;color:#6b7280;font-size:14px;border-bottom:1px solid #f3f4f6">Direction</td>
      <td style="padding:10px 0;text-align:right;color:#111827;font-size:14px;font-weight:600;border-bottom:1px solid #f3f4f6">{direction}</td>
    </tr>
    <tr>
      <td style="padding:10px 0;color:#6b7280;font-size:14px;border-bottom:1px solid #f3f4f6">Mode</td>
      <td style="padding:10px 0;text-align:right;color:#111827;font-size:14px;font-weight:600;border-bottom:1px solid #f3f4f6">{mode}</td>
    </tr>
    <tr>
      <td style="padding:10px 0;color:#6b7280;font-size:14px;border-bottom:1px solid #f3f4f6">Specialist Desk</td>
      <td style="padding:10px 0;text-align:right;color:#111827;font-size:14px;font-weight:600;border-bottom:1px solid #f3f4f6">{desk}</td>
    </tr>
    <tr>
      <td style="padding:10px 0;color:#6b7280;font-size:14px;border-bottom:1px solid #f3f4f6">Position Size</td>
      <td style="padding:10px 0;text-align:right;color:#111827;font-size:14px;font-weight:600;border-bottom:1px solid #f3f4f6">${size}</td>
    </tr>
    <tr>
      <td style="padding:10px 0;color:#6b7280;font-size:14px;border-bottom:1px solid #f3f4f6">Entry Price</td>
      <td style="padding:10px 0;text-align:right;color:#111827;font-size:14px;font-weight:600;border-bottom:1px solid #f3f4f6">${entry}</td>
    </tr>
    <tr>
      <td style="padding:10px 0;color:#6b7280;font-size:14px;border-bottom:1px solid #f3f4f6">Exit Price</td>
      <td style="padding:10px 0;text-align:right;color:#111827;font-size:14px;font-weight:600;border-bottom:1px solid #f3f4f6">${exit}</td>
    </tr>
    <tr>
      <td style="padding:10px 0;color:#6b7280;font-size:14px;border-bottom:1px solid #f3f4f6">Fair Value</td>
      <td style="padding:10px 0;text-align:right;color:#111827;font-size:14px;font-weight:600;border-bottom:1px solid #f3f4f6">${fair}</td>
    </tr>
    <tr>
      <td style="padding:10px 0;color:#6b7280;font-size:14px;border-bottom:1px solid #f3f4f6">Edge</td>
      <td style="padding:10px 0;text-align:right;color:#3b82f6;font-size:14px;font-weight:600;border-bottom:1px solid #f3f4f6">{edge}%</td>
    </tr>
    {fees_line}
    <tr>
      <td style="padding:10px 0;color:#6b7280;font-size:14px;border-bottom:1px solid #f3f4f6">Hold Duration</td>
      <td style="padding:10px 0;text-align:right;color:#111827;font-size:14px;font-weight:600;border-bottom:1px solid #f3f4f6">{hold:.1}h</td>
    </tr>
    <tr>
      <td style="padding:10px 0;color:#6b7280;font-size:14px">Exit Reason</td>
      <td style="padding:10px 0;text-align:right;color:#111827;font-size:14px;font-weight:600">{exit_reason}</td>
    </tr>
  </table>
</div>

<!-- Footer -->
<div style="padding:24px 32px;background:#f9fafb;text-align:center">
  <p style="margin:0;color:#6b7280;font-size:13px">Polymarket Trading Agent v2.0</p>
  <p style="margin:8px 0 0;color:#9ca3af;font-size:12px">Trade ID: {trade_id}</p>
</div>

</div>
</body></html>"#,
        pnl_color = pnl_color,
        emoji = if trade.pnl > Decimal::ZERO { "🎉" } else { "📉" },
        result = result_label,
        pnl_sign = pnl_sign,
        pnl = trade.pnl.abs(),
        pnl_pct = pnl_pct.abs(),
        question = trade.question,
        direction = trade.direction,
        mode = mode,
        desk = desk,
        size = trade.bet_size,
        entry = trade.entry_price,
        exit = exit_price,
        fair = trade.fair_value,
        edge = (trade.edge * Decimal::from(100)).round_dp(1),
        fees_line = fees_line,
        hold = hold_hours,
        exit_reason = exit_reason,
        trade_id = trade.id,
    )
}

fn format_trades_html(trades: &[Trade]) -> String {
    if trades.is_empty() {
        return r#"<tr><td colspan="5" style="padding:12px;color:#666;font-size:13px;text-align:center">Belum ada trade</td></tr>"#.to_string();
    }

    trades
        .iter()
        .map(|t| {
            let (status_label, status_color) = match t.status {
                crate::types::TradeStatus::Open => ("OPEN", "#ffa502"),
                crate::types::TradeStatus::Won => ("WIN", "#2ed573"),
                crate::types::TradeStatus::Lost => ("LOSS", "#ff4757"),
                crate::types::TradeStatus::Cancelled => ("CXLD", "#666"),
            };
            let pnl_color = if t.pnl > rust_decimal::Decimal::ZERO {
                "#2ed573"
            } else if t.pnl < rust_decimal::Decimal::ZERO {
                "#ff4757"
            } else {
                "#8a8a9a"
            };
            let pnl_sign = if t.pnl > rust_decimal::Decimal::ZERO { "+" } else { "" };
            let question = &t.question[..t.question.len().min(35)];
            let ellipsis = if t.question.len() > 35 { "..." } else { "" };

            format!(
                r#"<tr>
      <td style="padding:7px 10px;border-top:1px solid #1e1e3a;font-size:12px"><span style="color:{status_color};font-weight:600">{status_label}</span></td>
      <td style="padding:7px 10px;border-top:1px solid #1e1e3a;color:#ccc;font-size:12px">{dir}</td>
      <td style="padding:7px 10px;border-top:1px solid #1e1e3a;color:#ccc;font-size:12px;text-align:right">${size}</td>
      <td style="padding:7px 10px;border-top:1px solid #1e1e3a;color:{pnl_color};font-size:12px;text-align:right;font-weight:600">{pnl_sign}${pnl}</td>
      <td style="padding:7px 10px;border-top:1px solid #1e1e3a;color:#8a8a9a;font-size:11px">{question}{ellipsis}</td>
    </tr>"#,
                status_color = status_color,
                status_label = status_label,
                dir = t.direction,
                size = t.bet_size,
                pnl_color = pnl_color,
                pnl_sign = pnl_sign,
                pnl = t.pnl.abs(),
                question = question,
                ellipsis = ellipsis,
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}
