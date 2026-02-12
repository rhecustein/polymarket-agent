mod analyzer;
mod config;
mod data;
mod db;
mod email;
mod knowledge;
mod live;
mod paper;
mod strategy;
mod team;
mod telegram;
mod types;

use crate::analyzer::claude::ClaudeClient;
use crate::analyzer::gemini::GeminiClient;
use crate::config::Config;
use crate::data::Enricher;
use crate::data::polymarket::GammaScanner;
use crate::db::StateStore;
use crate::email::EmailAlert;
use crate::live::ClobClient;
use crate::paper::Portfolio;
use crate::strategy::{check_consecutive_losses, survival_adjust, LossAction};
use crate::telegram::{TelegramAlert, TelegramCommand};
use anyhow::Result;
use clap::Parser;
use rust_decimal::Decimal;
use tracing::{error, info, warn};

#[derive(Parser)]
#[command(name = "polyagent", about = "Autonomous AI Trading Agent for Polymarket — v2.0 BATTLE TEST")]
struct Cli {
    /// Run one scan cycle then exit
    #[arg(long)]
    once: bool,

    /// Override scan interval (seconds)
    #[arg(long)]
    interval: Option<u64>,

    /// Load config from a specific .env file
    #[arg(long)]
    config_file: Option<String>,

    /// Agent identifier (uses data/{id}.db, prefixes logs)
    #[arg(long)]
    agent_id: Option<String>,

    /// Knowledge-only mode: analyze markets but skip all trade execution
    #[arg(long)]
    knowledge_only: bool,
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info".into()),
        )
        .init();

    let cli = Cli::parse();
    let mut cfg = Config::from_env_file(cli.config_file.as_deref())?;

    if let Some(ref agent_id) = cli.agent_id {
        cfg.db_path = format!("data/{}.db", agent_id);
    }

    let knowledge_only = cli.knowledge_only || cfg.knowledge_only;
    let interval = cli.interval.unwrap_or(cfg.scan_interval_secs);

    // ═══════════════════════════════════════════
    let agent_label = cli.agent_id.as_deref().unwrap_or("default");
    let tp_label = if cfg.exit_tp_pct > Decimal::ZERO {
        format!("{}%", cfg.exit_tp_pct * Decimal::from(100))
    } else { "DYNAMIC".to_string() };
    let sl_label = if cfg.exit_sl_pct > Decimal::ZERO {
        format!("{}%", cfg.exit_sl_pct * Decimal::from(100))
    } else { "DYNAMIC".to_string() };

    info!("══════════════════════════════════════════════════════");
    info!("  POLYMARKET AGENT v2.0 — BATTLE TEST [{}]", agent_label);
    info!("  Mode: {}", if knowledge_only { "KNOWLEDGE ONLY (no trades)" }
        else if cfg.paper_trading { "PAPER TRADING (real data, virtual money)" } else { "LIVE TRADING" });
    info!("  AI: Gemini Flash 2.0 + Claude Sonnet (Judge top 3)");
    info!("  Balance: ${} | Kill: ${}", cfg.initial_balance, cfg.kill_threshold);
    info!("  Max Position: {}% | Kelly: 1/{:.0}", cfg.max_position_pct * Decimal::from(100), Decimal::ONE / cfg.kelly_fraction);
    info!("  Min Edge: {}% | Min Confidence: {}", cfg.min_edge_threshold * Decimal::from(100), cfg.min_confidence);
    info!("  TP: {} | SL: {} | Price-check: {}s", tp_label, sl_label, cfg.price_check_secs);
    info!("  Reserve: {}% | Max Open: {} | Max Spread: {}%",
        cfg.balance_reserve_pct * Decimal::from(100), cfg.max_open_positions,
        cfg.max_spread * Decimal::from(100));
    info!("  Candidates: {} | Deep: {} | Scan: {}s | Markets: {}",
        cfg.max_candidates, cfg.max_deep_analysis, interval, cfg.max_markets_to_scan);
    info!("  Desks: Crypto + Weather + Sports + General");
    info!("  Reports: every {}h | Stop: Ctrl+C or touch STOP file", cfg.report_interval_hours);
    info!("══════════════════════════════════════════════════════");

    if cfg.gemini_api_key.is_empty() {
        error!("GEMINI_API_KEY must be set");
        std::process::exit(1);
    }

    // Initialize components
    let gamma = GammaScanner::new(&cfg.gamma_api_base);
    let clob = ClobClient::new(&cfg.polymarket_clob_api);
    let gemini = GeminiClient::new(&cfg.gemini_api_key);
    let claude = ClaudeClient::new(&cfg.claude_api_key);
    let enricher = Enricher::new();

    if claude.is_configured() {
        info!("Claude Sonnet configured (Judge top 3 candidates)");
    } else {
        warn!("Claude API key not set — Judge will use Gemini for all candidates");
    }
    let portfolio = Portfolio::new(cfg.initial_balance);
    let store = StateStore::new(&cfg.db_path)?;
    let emailer = EmailAlert::new(
        &cfg.smtp_host, cfg.smtp_port, &cfg.smtp_user, &cfg.smtp_pass,
        &cfg.alert_from, &cfg.alert_to,
    );
    let telegram = TelegramAlert::new(&cfg.telegram_bot_token, &cfg.telegram_chat_id);

    // Optional: live engine for non-paper trading
    let mut _live_engine = if !cfg.paper_trading {
        match live::LiveEngine::new(
            &cfg.polymarket_clob_api, &cfg.wallet_private_key, cfg.initial_balance,
        ) {
            Ok(engine) => { info!("Live trading engine initialized"); Some(engine) }
            Err(e) => { error!("Failed to initialize live engine: {e}. Falling back to paper"); None }
        }
    } else { None };

    if emailer.is_configured() {
        info!("Email alerts configured -> {}", cfg.alert_to);
    } else {
        warn!("Email alerts NOT configured (set SMTP_* env vars)");
    }
    if telegram.is_configured() {
        info!("Telegram alerts configured -> chat {}", cfg.telegram_chat_id);
    } else {
        warn!("Telegram alerts NOT configured (set TELEGRAM_* env vars)");
    }

    // Graceful shutdown: Ctrl+C + STOP file
    let (shutdown_tx, mut shutdown_rx) = tokio::sync::watch::channel(false);
    let stop_tx1 = shutdown_tx.clone();
    tokio::spawn(async move {
        tokio::signal::ctrl_c().await.ok();
        info!("STOP SIGNAL (Ctrl+C)");
        stop_tx1.send(true).ok();
    });
    // STOP file watcher
    let stop_tx2 = shutdown_tx.clone();
    tokio::spawn(async move {
        loop {
            if std::path::Path::new("STOP").exists() {
                info!("STOP SIGNAL (STOP file detected)");
                stop_tx2.send(true).ok();
                // Remove the file so it doesn't trigger next time
                std::fs::remove_file("STOP").ok();
                break;
            }
            tokio::time::sleep(std::time::Duration::from_secs(5)).await;
        }
    });

    // Send startup notification
    telegram.send_message(&format!(
        "<b>PAPER TRADING STARTED</b>\nBalance: ${}\nMode: BATTLE TEST\nStop: Ctrl+C or touch STOP file",
        cfg.initial_balance
    )).await.ok();

    let mut cycle: u64 = 0;
    let mut last_daily_report = chrono::Utc::now().date_naive();
    let mut last_periodic_report = chrono::Utc::now();
    let mut loss_reduction_trades_left: u32 = 0;
    let mut paused = false;
    let mut audit_trade_count: usize = 0;
    let start_time = std::time::Instant::now();
    let mut tg_update_id: i64 = 0;

    // Compute deterministic jitter (0-60s) from agent_id
    let scan_jitter_secs: u64 = {
        let id_str = cli.agent_id.as_deref().unwrap_or("default");
        let hash: u64 = id_str.bytes().fold(0u64, |h, b| h.wrapping_mul(31).wrapping_add(b as u64));
        hash % 61
    };
    if scan_jitter_secs > 0 {
        info!("Scan jitter: {}s delay", scan_jitter_secs);
        if !sleep_or_shutdown(&mut shutdown_rx, scan_jitter_secs).await {
            return Ok(());
        }
    }

    loop {
        cycle += 1;
        let cycle_start = std::time::Instant::now();
        info!("━━━━━━━━━ CYCLE #{cycle} ━━━━━━━━━");

        // ── Step 1: Survival Check ──
        let mut effective_max_pct = cfg.max_position_pct;
        if !knowledge_only {
            if !portfolio.is_alive(cfg.kill_threshold) {
                error!("AGENT DEAD - Balance ${} below kill threshold ${}",
                    portfolio.balance(), cfg.kill_threshold);
                let stats = portfolio.stats();
                error!("{stats}");
                store.save_daily_snapshot(&stats).ok();
                emailer.send_alert("AGENT DEAD",
                    &format!("Balance fell below ${}. Final: ${}\n\n{stats}",
                        cfg.kill_threshold, stats.balance)).await.ok();
                telegram.send_critical_alert(&format!(
                    "AGENT DEAD - Balance ${} below kill threshold ${}",
                    stats.balance, cfg.kill_threshold)).await.ok();
                std::process::exit(1);
            }

            let loss_action = check_consecutive_losses(portfolio.consecutive_losses());
            match loss_action {
                LossAction::Pause => {
                    if !paused {
                        error!("PAUSED: {} consecutive losses", portfolio.consecutive_losses());
                        emailer.send_alert("TRADING PAUSED",
                            &format!("{} consecutive losses.", portfolio.consecutive_losses())).await.ok();
                        telegram.send_critical_alert(&format!(
                            "TRADING PAUSED: {} consecutive losses.",
                            portfolio.consecutive_losses())).await.ok();
                        paused = true;
                    }
                    // Still monitor positions during pause
                    resolve_open_trades(&portfolio, &gamma, &store, &telegram, &cfg, &mut audit_trade_count).await;
                    sleep_or_shutdown(&mut shutdown_rx, interval).await;
                    continue;
                }
                LossAction::ReduceSize => {
                    if loss_reduction_trades_left == 0 {
                        warn!("8+ losses: reducing position 50% for 3 trades");
                        loss_reduction_trades_left = 3;
                    }
                }
                LossAction::SkipCycle => {
                    warn!("5+ losses: skipping this cycle");
                    resolve_open_trades(&portfolio, &gamma, &store, &telegram, &cfg, &mut audit_trade_count).await;
                    sleep_or_shutdown(&mut shutdown_rx, interval).await;
                    continue;
                }
                LossAction::Continue => { paused = false; }
            }

            let (adj_max_pct, is_dead) = survival_adjust(
                portfolio.balance(), cfg.kill_threshold, cfg.max_position_pct);
            effective_max_pct = adj_max_pct;
            if is_dead {
                error!("Agent dead - shutting down");
                std::process::exit(1);
            }
            if effective_max_pct < cfg.max_position_pct {
                warn!("SURVIVAL MODE: Position cap {:.1}%", effective_max_pct * Decimal::from(100));
            }
            if loss_reduction_trades_left > 0 {
                effective_max_pct = effective_max_pct * Decimal::new(5, 1);
            }
        }

        // ── Step 2: Resolve Open Trades ──
        if !knowledge_only {
            resolve_open_trades(&portfolio, &gamma, &store, &telegram, &cfg, &mut audit_trade_count).await;
        }

        // ── Step 3: Run Team Pipeline ──
        // Skip opening new trades if at max open positions
        let at_max_positions = !knowledge_only
            && portfolio.open_position_count() >= cfg.max_open_positions;

        if at_max_positions {
            info!("At max open positions ({}/{}), monitoring only",
                portfolio.open_position_count(), cfg.max_open_positions);
        }

        let team_stats = if !at_max_positions {
            team::run_cycle(
                &cfg, &gemini, &claude, &enricher, &gamma, &clob,
                &portfolio, &store, &telegram,
                effective_max_pct, cfg.max_candidates, cfg.max_deep_analysis,
            ).await
        } else {
            team::types::TeamCycleStats::default()
        };

        let cycle_duration = cycle_start.elapsed().as_secs_f64();

        info!("Team: scanned={} researched={} analyzed={} traded={} ({:.1}s)",
            team_stats.markets_scanned, team_stats.markets_researched,
            team_stats.markets_analyzed, team_stats.trades_placed, cycle_duration);

        // Save enhanced cycle log
        store.log_cycle(
            cycle,
            team_stats.markets_scanned,
            team_stats.markets_passed_quality,
            team_stats.trades_placed,
            0, // trades_closed tracked separately
            portfolio.balance(),
            portfolio.open_position_count(),
            team_stats.api_cost,
            cycle_duration,
        ).ok();

        // Legacy cycle save
        store.save_cycle(
            team_stats.markets_scanned, team_stats.markets_analyzed,
            team_stats.trades_placed, &team_stats.api_cost.to_string(),
            &portfolio.balance().to_string(),
        ).ok();

        // ── Step 4: Periodic Audit ──
        if audit_trade_count >= 10 {
            info!("Running Auditor ({} trades since last audit)...", audit_trade_count);
            let closed_trades = portfolio.closed_trades();
            let recent: Vec<_> = closed_trades.iter().rev().take(20).cloned().collect();
            if !recent.is_empty() {
                match team::auditor::audit(&gemini, &recent).await {
                    Ok(insight) => {
                        info!("Auditor: win_rate={:.0}% cal_error={:.2} bull={:.2} bear={:.2}",
                            insight.win_rate * 100.0, insight.avg_calibration_error,
                            insight.bull_accuracy, insight.bear_accuracy);
                        for i in &insight.insights { info!("  Insight: {}", i); }
                        team::auditor::save_insights(&insight).ok();
                    }
                    Err(e) => warn!("Audit failed: {e}"),
                }
            }
            audit_trade_count = 0;
        }

        // ── Dashboard stats ──
        let markets_for_stats = gamma.scan(50).await.unwrap_or_default();
        let stats = portfolio.stats_with_markets(&markets_for_stats);
        info!("\n{stats}");

        // ── Periodic Report (every N hours) ──
        let hours_since_report = (chrono::Utc::now() - last_periodic_report).num_hours();
        if hours_since_report >= cfg.report_interval_hours as i64 {
            info!("Sending {}h periodic report...", cfg.report_interval_hours);
            store.save_daily_snapshot(&stats).ok();
            let in_survival = effective_max_pct < cfg.max_position_pct;
            emailer.send_periodic_report(&stats, cycle, in_survival, &portfolio).await.ok();
            telegram.send_daily_summary(&stats, cycle as u32).await.ok();
            last_periodic_report = chrono::Utc::now();
        }

        // ── Daily Report (legacy, at midnight UTC) ──
        let today = chrono::Utc::now().date_naive();
        if today > last_daily_report {
            store.save_daily_snapshot(&stats).ok();
            last_daily_report = today;
        }

        // ── Telegram commands ──
        let tg_cmds = telegram.poll_commands(&mut tg_update_id).await;
        for cmd in tg_cmds {
            match cmd {
                TelegramCommand::Status => {
                    let open = portfolio.open_trades();
                    telegram.send_status(&stats, &open).await.ok();
                }
                TelegramCommand::Stop => {
                    info!("STOP SIGNAL (Telegram /stop)");
                    graceful_shutdown(&portfolio, &gamma, &store, &emailer, &telegram, cycle, start_time).await;
                    return Ok(());
                }
                TelegramCommand::Trades => {
                    let closed = portfolio.closed_trades();
                    let recent: Vec<_> = closed.iter().rev().take(5).cloned().collect();
                    let mut msg = String::from("<b>Last 5 Trades</b>\n");
                    for t in &recent {
                        let pnl_sign = if t.pnl > Decimal::ZERO { "+" } else { "" };
                        let status = if t.status == crate::types::TradeStatus::Won { "WIN" } else { "LOSS" };
                        msg.push_str(&format!(
                            "\n[{}] {} {} | {}${}\n<i>{}</i>\n",
                            status, t.direction, t.trade_mode.as_deref().unwrap_or("?"),
                            pnl_sign, t.pnl, &t.question[..t.question.len().min(50)]
                        ));
                    }
                    telegram.send_message(&msg).await.ok();
                }
                TelegramCommand::OpenPositions => {
                    let open = portfolio.open_trades();
                    let mut msg = format!("<b>Open Positions ({})</b>\n", open.len());
                    for t in &open {
                        msg.push_str(&format!(
                            "\n{} {} ({})\nEntry: ${} | Size: ${}\n<i>{}</i>\n",
                            t.direction, t.trade_mode.as_deref().unwrap_or("?"),
                            t.category.as_deref().unwrap_or("?"),
                            t.entry_price, t.bet_size,
                            &t.question[..t.question.len().min(50)]
                        ));
                    }
                    if open.is_empty() { msg.push_str("\nNo open positions"); }
                    telegram.send_message(&msg).await.ok();
                }
                TelegramCommand::Help => {
                    telegram.send_message(
                        "<b>Commands:</b>\n/status - Portfolio snapshot\n/trades - Last 5 closed\n/open - Open positions\n/stop - Graceful shutdown\n/help - This message"
                    ).await.ok();
                }
            }
        }

        // ── Low balance alert ──
        let balance_pct = if cfg.initial_balance > Decimal::ZERO {
            (portfolio.balance() / cfg.initial_balance * Decimal::from(100))
        } else { Decimal::from(100) };
        if balance_pct < Decimal::from(70) {
            telegram.send_critical_alert(&format!(
                "BALANCE LOW: ${} ({:.0}% of initial)",
                portfolio.balance(), balance_pct)).await.ok();
        }

        if cli.once {
            info!("Single run mode - exiting.");
            break;
        }

        // ── Fast Price-Check Loop ──
        let pc_secs = cfg.price_check_secs;
        let has_open = !knowledge_only && portfolio.open_position_count() > 0;
        let use_fast_loop = has_open && pc_secs > 0 && pc_secs < interval;

        if use_fast_loop {
            let checks = interval / pc_secs;
            info!("Fast price-check: {} open, every {}s ({} checks)",
                portfolio.open_position_count(), pc_secs, checks);

            let mut shutdown = false;
            for check_i in 1..=checks {
                // Check STOP file during fast loop too
                if *shutdown_rx.borrow() {
                    shutdown = true;
                    break;
                }
                if !sleep_or_shutdown(&mut shutdown_rx, pc_secs).await {
                    shutdown = true;
                    break;
                }
                if portfolio.open_position_count() == 0 {
                    info!("All positions closed, waiting for next cycle");
                    let remaining = (checks - check_i) * pc_secs;
                    if remaining > 0 {
                        if !sleep_or_shutdown(&mut shutdown_rx, remaining).await {
                            shutdown = true;
                        }
                    }
                    break;
                }
                match gamma.scan(cfg.max_markets_to_scan).await {
                    Ok(fresh_markets) => {
                        let resolved = portfolio.resolve_with_prices(
                            &fresh_markets, cfg.exit_tp_pct, cfg.exit_sl_pct);
                        for trade in &resolved {
                            store.save_trade(trade).ok();
                            let reason = trade.exit_reason.map(|r| format!("{}", r))
                                .unwrap_or_else(|| "?".to_string());
                            info!("FAST-{}: {} | PnL ${} | {}",
                                reason, &trade.question[..trade.question.len().min(35)],
                                trade.pnl, trade.direction);
                            telegram.send_trade_closed_alert(trade).await.ok();
                        }
                        if !resolved.is_empty() {
                            info!("[{}/{}] {} resolved, {} open",
                                check_i, checks, resolved.len(), portfolio.open_position_count());
                            audit_trade_count += resolved.len();
                        }
                    }
                    Err(e) => warn!("Price check scan failed: {e}"),
                }
            }

            if shutdown {
                graceful_shutdown(&portfolio, &gamma, &store, &emailer, &telegram, cycle, start_time).await;
                break;
            }
        } else {
            if !sleep_or_shutdown(&mut shutdown_rx, interval).await {
                graceful_shutdown(&portfolio, &gamma, &store, &emailer, &telegram, cycle, start_time).await;
                break;
            }
        }
    }

    Ok(())
}

/// Resolve open trades with real market prices
async fn resolve_open_trades(
    portfolio: &Portfolio,
    gamma: &GammaScanner,
    store: &StateStore,
    telegram: &TelegramAlert,
    cfg: &Config,
    audit_count: &mut usize,
) {
    let markets = match gamma.scan(cfg.max_markets_to_scan).await {
        Ok(m) => m,
        Err(e) => { error!("Pre-resolve scan failed: {e}"); return; }
    };
    let resolved = portfolio.resolve_with_prices(&markets, cfg.exit_tp_pct, cfg.exit_sl_pct);
    for trade in &resolved {
        store.save_trade(trade).ok();
        telegram.send_trade_closed_alert(trade).await.ok();
    }
    if !resolved.is_empty() {
        info!("{} trade(s) resolved this cycle", resolved.len());
        *audit_count += resolved.len();
    }
}

/// Graceful shutdown: mark positions, send final report, save state
async fn graceful_shutdown(
    portfolio: &Portfolio,
    gamma: &GammaScanner,
    store: &StateStore,
    emailer: &EmailAlert,
    telegram: &TelegramAlert,
    cycle: u64,
    start_time: std::time::Instant,
) {
    info!("═══ GRACEFUL SHUTDOWN ═══");

    // Step 1: Mark all open positions to market
    let markets = gamma.scan(200).await.unwrap_or_default();
    let closed = portfolio.close_all_positions(&markets);
    info!("Step 1: Closed {} open positions", closed.len());
    for trade in &closed {
        store.save_trade(trade).ok();
    }

    // Step 2: Calculate final stats
    let final_stats = portfolio.stats_with_markets(&markets);
    info!("Step 2: Final stats calculated");
    info!("\n{final_stats}");

    // Step 3: Save final state
    store.save_daily_snapshot(&final_stats).ok();
    info!("Step 3: State saved to database");

    // Step 4: Send final report
    let runtime_hours = start_time.elapsed().as_secs_f64() / 3600.0;
    let summary = format!(
        "PAPER TRADING STOPPED\n\
        Runtime: {:.1}h | Cycles: {}\n\
        Balance: ${} (start ${})\n\
        P&L: ${} ({}%)\n\
        Trades: {} (W:{} L:{} = {:.0}%)\n\
        Max DD: {}%\n\
        Open at shutdown: {} (marked to market)",
        runtime_hours, cycle,
        final_stats.balance, final_stats.initial_balance,
        final_stats.total_pnl, final_stats.roi,
        final_stats.win_count + final_stats.loss_count,
        final_stats.win_count, final_stats.loss_count, final_stats.win_rate,
        final_stats.max_drawdown_pct,
        closed.len(),
    );

    emailer.send_alert("PAPER TRADING STOPPED - Final Report", &summary).await.ok();
    telegram.send_message(&format!("<b>PAPER TRADING STOPPED</b>\n{}", summary)).await.ok();
    info!("Step 4: Final report sent");

    info!("═══ SHUTDOWN COMPLETE ═══");
}

/// Sleep for `secs` or return false if shutdown signal received
async fn sleep_or_shutdown(
    rx: &mut tokio::sync::watch::Receiver<bool>,
    secs: u64,
) -> bool {
    tokio::select! {
        _ = tokio::time::sleep(std::time::Duration::from_secs(secs)) => true,
        _ = rx.changed() => false,
    }
}
