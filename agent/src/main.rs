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
use crate::knowledge::collector::KnowledgeCollector;
use crate::live::ClobClient;
use crate::paper::{Portfolio, SimConfig};
use crate::strategy::{check_consecutive_losses, survival_adjust, LossAction};
use crate::telegram::{TelegramAlert, TelegramCommand};
use anyhow::Result;
use clap::Parser;
use rust_decimal::Decimal;
use std::io::{self, Write};
use std::str::FromStr;
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

    /// Skip interactive configuration and use defaults/env
    #[arg(long, short = 'y')]
    yes: bool,

    /// Generate knowledge report from collected data and exit
    #[arg(long)]
    knowledge_report: bool,
}

#[tokio::main]
async fn main() -> Result<()> {
    eprintln!("[AGENT STARTING] Binary executing..."); // DEBUG: Before anything else

    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info".into()),
        )
        .init();

    eprintln!("[AGENT] Logger initialized"); // DEBUG: After logger init

    let cli = Cli::parse();
    eprintln!("[AGENT] CLI parsed: agent_id={:?}", cli.agent_id); // DEBUG: After CLI parse

    let mut cfg = match Config::from_env_file(cli.config_file.as_deref()) {
        Ok(c) => {
            eprintln!("[AGENT] Config loaded successfully");
            c
        }
        Err(e) => {
            eprintln!("[AGENT ERROR] Failed to load config: {}", e);
            return Err(e);
        }
    };

    if let Some(ref agent_id) = cli.agent_id {
        cfg.db_path = format!("data/{}.db", agent_id);
    }

    let knowledge_only = cli.knowledge_only || cfg.knowledge_only;
    let interval = cli.interval.unwrap_or(cfg.scan_interval_secs);

    // ═══ Knowledge Report Mode ═══
    if cli.knowledge_report {
        generate_knowledge_report(&cfg)?;
        return Ok(());
    }

    // Interactive Setup (unless --yes passed or skipped via env)
    // We check raw args because we haven't added --yes to Clap yet, or we can just add it to Clap.
    // Let's add it to Clap struct below in next edit, for now just call the function.
    if !cli.yes {
        interactive_configuration(&mut cfg)?;
    }

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
    info!("  AI: Gemini Flash 2.0 ONLY (Sonnet disabled for cost optimization)");
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

    // Claude Sonnet: AKTIF sebagai Hakim Akhir (Final Validator)
    if claude.is_configured() {
        info!("✅ Claude Sonnet 4.5 AKTIF sebagai Hakim Akhir (threshold 60%)");
    } else {
        warn!("⚠️  CLAUDE_API_KEY tidak diset - validasi akhir DISABLED!");
    }
    let portfolio = Portfolio::new(cfg.initial_balance);
    let sim = SimConfig::from_config(&cfg);
    info!("Simulation: fees={} slippage={} fills={} impact={}",
        sim.fees_enabled, sim.slippage_enabled, sim.fills_enabled, sim.impact_enabled);
    let store = StateStore::new(&cfg.db_path)?;

    // Record strategy parameters for knowledge collection
    store.record_strategy_params(
        cfg.generation,
        cli.agent_id.as_deref(),
        cfg.min_confidence,
        cfg.min_edge_threshold,
        cfg.max_position_pct,
        cfg.kelly_fraction,
        cfg.exit_tp_pct,
        cfg.exit_sl_pct,
        &cfg.category_filter,
    ).ok();

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

    // Compute deterministic jitter (0-300s) from agent_id to spread API load across agents
    let scan_jitter_secs: u64 = {
        let id_str = cli.agent_id.as_deref().unwrap_or("default");
        let hash: u64 = id_str.bytes().fold(0u64, |h, b| h.wrapping_mul(31).wrapping_add(b as u64));
        hash % 301
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
                    resolve_open_trades(&portfolio, &gamma, &store, &telegram, &emailer, &cfg, &mut audit_trade_count, &sim).await;
                    sleep_or_shutdown(&mut shutdown_rx, interval).await;
                    continue;
                }
                LossAction::ReduceSize => {
                    if loss_reduction_trades_left == 0 {
                        warn!("4+ losses: reducing position 50% for 3 trades");
                        loss_reduction_trades_left = 3;
                    }
                }
                LossAction::SkipCycle => {
                    warn!("3+ losses: skipping this cycle");
                    resolve_open_trades(&portfolio, &gamma, &store, &telegram, &emailer, &cfg, &mut audit_trade_count, &sim).await;
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
            resolve_open_trades(&portfolio, &gamma, &store, &telegram, &emailer, &cfg, &mut audit_trade_count, &sim).await;
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

        // ── Daily Report (at midnight UTC) ──
        let today = chrono::Utc::now().date_naive();
        if today > last_daily_report {
            info!("Sending daily summary (midnight UTC)...");
            store.save_daily_snapshot(&stats).ok();
            let in_survival = effective_max_pct < cfg.max_position_pct;
            emailer.send_daily_summary(&stats, cycle as u32, in_survival).await.ok();
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
                    graceful_shutdown(&portfolio, &gamma, &store, &emailer, &telegram, cycle, start_time, &sim).await;
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
            portfolio.balance() / cfg.initial_balance * Decimal::from(100)
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
                            &fresh_markets, cfg.exit_tp_pct, cfg.exit_sl_pct, &sim);
                        for trade in &resolved {
                            store.save_trade(trade).ok();
                            let reason = trade.exit_reason.map(|r| format!("{}", r))
                                .unwrap_or_else(|| "?".to_string());
                            info!("FAST-{}: {} | PnL ${} | {}",
                                reason, &trade.question[..trade.question.len().min(35)],
                                trade.pnl, trade.direction);
                            telegram.send_trade_closed_alert(trade).await.ok();
                            emailer.send_trade_closed(trade).await.ok();
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
                graceful_shutdown(&portfolio, &gamma, &store, &emailer, &telegram, cycle, start_time, &sim).await;
                break;
            }
        } else {
            if !sleep_or_shutdown(&mut shutdown_rx, interval).await {
                graceful_shutdown(&portfolio, &gamma, &store, &emailer, &telegram, cycle, start_time, &sim).await;
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
    emailer: &EmailAlert,
    cfg: &Config,
    audit_count: &mut usize,
    sim: &SimConfig,
) {
    let markets = match gamma.scan(cfg.max_markets_to_scan).await {
        Ok(m) => m,
        Err(e) => { error!("Pre-resolve scan failed: {e}"); return; }
    };
    let resolved = portfolio.resolve_with_prices(&markets, cfg.exit_tp_pct, cfg.exit_sl_pct, sim);

    // Collect knowledge from closed trades
    let knowledge = KnowledgeCollector::new(store);

    for trade in &resolved {
        store.save_trade(trade).ok();

        // Send notifications (Telegram + Email)
        telegram.send_trade_closed_alert(trade).await.ok();
        emailer.send_trade_closed(trade).await.ok();

        // Collect knowledge for future improvements
        if let Err(e) = knowledge.collect_on_trade_close(trade) {
            warn!("Failed to collect knowledge for trade {}: {}", trade.id, e);
        }
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
    sim: &SimConfig,
) {
    info!("═══ GRACEFUL SHUTDOWN ═══");

    // Step 1: Mark all open positions to market
    let markets = gamma.scan(200).await.unwrap_or_default();
    let closed = portfolio.close_all_positions(&markets, sim);
    info!("Step 1: Closed {} open positions", closed.len());

    let knowledge = KnowledgeCollector::new(store);
    for trade in &closed {
        store.save_trade(trade).ok();
        // Collect knowledge from shutdown trades
        knowledge.collect_on_trade_close(trade).ok();
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

fn interactive_configuration(cfg: &mut Config) -> Result<()> {
    // If not TTY, we might want to skip, but for now we assume TTY if yes is false.
    // Actually we can check if it's a TTY, but let's keep it simple.

    println!("\n╔══════════════════════════════════════════════════════════╗");
    println!("║     POLYMARKET AGENT SETUP — v2.0 BATTLE TEST            ║");
    println!("╚══════════════════════════════════════════════════════════╝");
    
    // 1. Trading Mode
    println!("\n[1/3] TRADING MODE SELECTION");
    println!("  1) PAPER TRADING (Simulation - Recommended for testing)");
    println!("  2) LIVE TRADING  (REAL MONEY - RISKY)");
    print!("\n  Select Mode [default: 1]: ");
    io::stdout().flush()?;

    let mut input = String::new();
    io::stdin().read_line(&mut input)?;
    match input.trim() {
        "2" => cfg.paper_trading = false,
        _ => cfg.paper_trading = true,
    }

    // 2. Initial Balance
    println!("\n[2/3] INITIAL BALANCE CONFIGURATION");
    println!("  Current Configured Balance: ${}", cfg.initial_balance);
    if !cfg.paper_trading {
         println!("  ⚠️  LIVE MODE: This must match your wallet balance for risk calc.");
    } else {
         println!("  (Virtual balance for simulation)");
    }
    print!("\n  Enter Initial Balance [default: ${}]: ", cfg.initial_balance);
    io::stdout().flush()?;

    input.clear();
    io::stdin().read_line(&mut input)?;
    let input_trim = input.trim();
    if !input_trim.is_empty() {
        if let Ok(val) = Decimal::from_str(input_trim) {
            cfg.initial_balance = val;
        } else {
             println!("  Invalid number, keeping default.");
        }
    }

    // 3. Balance Alert / Confirmation
    println!("\n[3/3] PRE-FLIGHT CHECK");
    println!("  • Mode:            {}", if cfg.paper_trading { "PAPER TRADING (Safe)" } else { "LIVE TRADING (Real Funds)" });
    println!("  • Initial Balance: ${}", cfg.initial_balance);
    println!("  • Kill Threshold:  ${}", cfg.kill_threshold);
    
    if !cfg.paper_trading {
        println!("\n  🛑 CRITICAL WARNING: YOU ARE ABOUT TO START LIVE TRADING.");
        println!("  Ensure your wallet has at least ${} + gas fees.", cfg.initial_balance);
        println!("  The agent will execute real transactions on Polymarket.");
    }

    print!("\n  >>> START AGENT? [Y/n]: ");
    io::stdout().flush()?;
    input.clear();
    io::stdin().read_line(&mut input)?;

    if input.trim().eq_ignore_ascii_case("n") {
        println!("\nAborted by user.");
        std::process::exit(0);
    }
    println!("\nStarting engine...");
    Ok(())
}

/// Generate and display knowledge report from collected data
fn generate_knowledge_report(cfg: &Config) -> Result<()> {
    println!("\n═══════════════════════════════════════════════════════");
    println!("  KNOWLEDGE REPORT — Paper Trading Insights");
    println!("═══════════════════════════════════════════════════════\n");

    let store = StateStore::new(&cfg.db_path)?;
    let collector = KnowledgeCollector::new(&store);

    match collector.get_summary() {
        Ok(summary) => {
            println!("{}", summary.to_report());

            // Actionable recommendations
            println!("\n🎯 ACTIONABLE RECOMMENDATIONS:\n");

            if let Some(best) = summary.best_category() {
                println!(
                    "✅ FOCUS ON: {} markets ({:.1}% win rate, {} trades)",
                    best.category,
                    best.win_rate * 100.0,
                    best.total_trades
                );
            }

            let poor = summary.poor_categories();
            if !poor.is_empty() {
                println!("\n⚠️  AVOID THESE CATEGORIES:");
                for cat in poor {
                    println!(
                        "   • {} ({:.1}% win rate, {} trades)",
                        cat.category,
                        cat.win_rate * 100.0,
                        cat.total_trades
                    );
                }
            }

            if summary.avg_cost_impact_pct > 5.0 {
                println!(
                    "\n💸 COST OPTIMIZATION: Fees+slippage eating {:.1}% of profits — consider reducing position sizes",
                    summary.avg_cost_impact_pct
                );
            }

            if !summary.best_timing_patterns.is_empty() {
                println!("\n⏰ OPTIMAL TRADING HOURS (UTC):");
                for (i, pattern) in summary.best_timing_patterns.iter().take(3).enumerate() {
                    println!(
                        "   {}. {:02}:00-{:02}:59 ({:.1}% win rate)",
                        i + 1,
                        pattern.entry_hour,
                        pattern.entry_hour,
                        pattern.win_rate
                    );
                }
            }

            // Export recommendations
            println!("\n📊 DATA EXPORT:\n");
            println!("  Database: {}", cfg.db_path);
            println!("  Query examples:");
            println!("    sqlite3 {} \"SELECT category, win_rate, total_trades FROM knowledge_category_stats ORDER BY win_rate DESC\"", cfg.db_path);
            println!("    sqlite3 {} \"SELECT entry_hour, win_rate FROM knowledge_timing_analysis GROUP BY entry_hour\"", cfg.db_path);
        }
        Err(e) => {
            println!("❌ Failed to generate report: {}", e);
            println!("\nPossible reasons:");
            println!("  • No trades completed yet (database empty)");
            println!("  • Database file not found: {}", cfg.db_path);
        }
    }

    println!("\n═══════════════════════════════════════════════════════\n");
    Ok(())
}

