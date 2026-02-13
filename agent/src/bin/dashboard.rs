//! Polymarket Multi-Agent Dashboard
//!
//! Web dashboard for launching and monitoring 1-100 trading agents.
//! Manages agent processes, generates diverse configs, reads from SQLite databases.
//!
//! Usage: cargo run --bin dashboard
//! Then open http://localhost:3000

use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Instant;

use axum::extract::{Path as AxumPath, State};
use axum::response::{Html, Json};
use axum::routing::{get, post};
use axum::Router;
use rusqlite::OpenFlags;
use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;
use tower_http::cors::CorsLayer;

// ── Types ──

#[derive(Serialize)]
struct AgentStatus {
    balance: f64,
    initial_balance: f64,
    pnl: f64,
    roi: f64,
    win_rate: f64,
    trades_count: i64,
    wins: i64,
    losses: i64,
    open_count: i64,
    locked_balance: f64,
    api_cost: f64,
    total_fees: f64,
    total_gas: f64,
    total_slippage: f64,
    total_platform: f64,
    last_cycle: String,
    markets_scanned: i64,
    markets_analyzed: i64,
    uptime_secs: u64,
    db_found: bool,
    total_agents: usize,
    running_agents: usize,
    dead_agents: usize,
}

#[derive(Serialize)]
struct TradeRecord {
    id: String,
    timestamp: String,
    question: String,
    direction: String,
    entry_price: String,
    fair_value: String,
    edge: String,
    bet_size: String,
    pnl: String,
    status: String,
    balance_after: String,
    category: String,
    trade_mode: String,
    exit_reason: String,
    take_profit: String,
    stop_loss: String,
    entry_gas_fee: String,
    exit_gas_fee: String,
    entry_slippage: String,
    exit_slippage: String,
    platform_fee: String,
    maker_taker_fee: String,
    total_fees: String,
    net_pnl: String,
}

#[derive(Serialize)]
struct AnalysisRecord {
    timestamp: String,
    question: String,
    current_price: String,
    fair_value: String,
    edge: String,
    direction: String,
    should_trade: bool,
    confidence: String,
    model: String,
    api_cost: String,
    reasoning: String,
}

#[derive(Serialize)]
struct CycleRecord {
    timestamp: String,
    markets_scanned: i64,
    markets_analyzed: i64,
    trades_placed: i64,
    api_cost: String,
    balance_after: String,
}

#[derive(Serialize)]
struct ActivityEvent {
    timestamp: String,
    event_type: String,
    summary: String,
    agent_id: String,
}

#[derive(Serialize)]
struct OpenPosition {
    market_id: String,
    question: String,
    direction: String,
    entry_price: f64,
    current_price: f64,
    pnl_pct: f64,
    tp_price: f64,
    sl_price: f64,
    last_price_check: Option<String>,
    bet_size: f64,
    unrealized_pnl: f64,
    timestamp: String,
}

#[derive(Serialize)]
struct AgentInfo {
    id: String,
    strategy: String,
    status: String, // "running", "stopped", "dead"
    pnl: f64,
    balance: f64,
    initial_balance: f64,
    trades_count: i64,
    win_rate: f64,
    last_cycle: String,
    category: String,
    tp_sl: String,
    phase: String,
    phase_detail: String,
    interval: u64,
    price_check_interval: u64,
    judge_model: String, // "gemini" or "sonnet" (default gemini)
    open_position: Option<OpenPosition>,
}

#[derive(Serialize)]
struct AgentDetail {
    info: AgentInfo,
    trades: Vec<TradeRecord>,
    analyses: Vec<AnalysisRecord>,
    cycles: Vec<CycleRecord>,
    open_positions: Vec<OpenPositionDetail>,
    performance: PerformanceStats,
    tp_sl_settings: TpSlSettings,
}

#[derive(Serialize, Default)]
struct OpenPositionDetail {
    market_id: String,
    question: String,
    direction: String,
    entry_price: f64,
    bet_size: f64,
    take_profit: Option<f64>,
    stop_loss: Option<f64>,
    trade_mode: String,
    timestamp: String,
    current_price: f64,
    pnl_pct: f64,
    unrealized_pnl: f64,
    distance_to_tp_pct: f64,
    distance_to_sl_pct: f64,
    hours_open: f64,
    last_price_check: Option<String>,
}

#[derive(Serialize, Default)]
struct PerformanceStats {
    avg_hold_hours: f64,
    best_trade_pnl: f64,
    worst_trade_pnl: f64,
    avg_pnl: f64,
    tp_hits: i64,
    sl_hits: i64,
    total_closed: i64,
    total_fees: f64,
    total_slippage: f64,
    total_gas: f64,
    total_platform: f64,
    avg_fee_per_trade: f64,
}

#[derive(Serialize, Default)]
struct TpSlSettings {
    exit_tp_pct: String,
    exit_sl_pct: String,
    price_check_secs: u64,
    kill_threshold: String,
    min_confidence: String,
    min_edge: String,
    max_open_positions: String,
}

#[derive(Deserialize)]
struct StartRequest {
    count: usize,
    category: String,
    tp_sl: String,
    capital: f64,
    mode: String, // "paper" or "live"
}

#[derive(Serialize)]
struct StartResponse {
    ok: bool,
    message: String,
    agents_started: usize,
}

#[derive(Serialize)]
struct StopResponse {
    ok: bool,
    message: String,
    agents_stopped: usize,
}

#[derive(Serialize, Clone)]
struct DailyStats {
    date: String,
    pnl: f64,
    balance: f64,
    trades_count: i64,
    wins: i64,
    losses: i64,
}

#[derive(Serialize)]
struct CalendarResponse {
    stats: Vec<DailyStats>,
}

struct ChildAgent {
    process: tokio::process::Child,
    #[allow(dead_code)]
    agent_id: String,
    strategy: String,
    category: String,
    tp_sl: String,
    initial_balance: f64,
    interval: u64,
}

struct AppState {
    start_time: Instant,
    agents: Mutex<HashMap<String, ChildAgent>>,
}

type SharedState = Arc<AppState>;

// ── Strategy Presets ──

struct StrategyPreset {
    name: &'static str,
    kelly: f64,
    edge: f64,
    position: f64,
    tp: f64,
    sl: f64,
    interval: u64,
}

const PRESETS: [StrategyPreset; 13] = [
    StrategyPreset { name: "Balanced",        kelly: 0.40, edge: 0.07, position: 0.10, tp: 0.0, sl: 0.0, interval: 3600 },
    StrategyPreset { name: "Conservative",    kelly: 0.30, edge: 0.10, position: 0.07, tp: 0.0, sl: 0.0, interval: 3600 },
    StrategyPreset { name: "Patient",         kelly: 0.35, edge: 0.07, position: 0.08, tp: 0.10, sl: 0.07, interval: 3600 },
    StrategyPreset { name: "Win-Biased",      kelly: 0.40, edge: 0.10, position: 0.10, tp: 0.08, sl: 0.05, interval: 3600 },
    StrategyPreset { name: "High Conviction", kelly: 0.50, edge: 0.12, position: 0.12, tp: 0.08, sl: 0.05, interval: 3600 },
    StrategyPreset { name: "Tight Risk",      kelly: 0.35, edge: 0.10, position: 0.07, tp: 0.05, sl: 0.03, interval: 3600 },
    StrategyPreset { name: "Swing Trader",    kelly: 0.40, edge: 0.07, position: 0.10, tp: 0.15, sl: 0.10, interval: 7200 },
    StrategyPreset { name: "Ultra Safe",      kelly: 0.20, edge: 0.15, position: 0.05, tp: 0.05, sl: 0.05, interval: 3600 },
    StrategyPreset { name: "Scalper",         kelly: 0.45, edge: 0.05, position: 0.08, tp: 0.03, sl: 0.03, interval: 1800 },
    StrategyPreset { name: "Aggressive",      kelly: 0.50, edge: 0.05, position: 0.12, tp: 0.05, sl: 0.05, interval: 3600 },
    StrategyPreset { name: "Berserker",       kelly: 0.60, edge: 0.15, position: 0.60, tp: 0.12, sl: 0.05, interval: 3600 },
    StrategyPreset { name: "YOLO",            kelly: 0.70, edge: 0.12, position: 0.80, tp: 0.15, sl: 0.06, interval: 1800 },
    StrategyPreset { name: "All-In",          kelly: 0.80, edge: 0.20, position: 1.00, tp: 0.25, sl: 0.00, interval: 3600 },
];

fn parse_tp_sl(preset: &str) -> (f64, f64) {
    match preset {
        "fast"    => (0.03, 0.03),
        "normal"  => (0.05, 0.05),
        "patient" => (0.10, 0.07),
        "wide"    => (0.15, 0.10),
        "extreme" => (0.12, 0.05),
        _ => (0.05, 0.05),
    }
}

struct GeneratedAgent {
    id: String,
    strategy: String,
    kelly: f64,
    edge: f64,
    position: f64,
    tp: f64,
    sl: f64,
    interval: u64,
    capital: f64,
    category: String,
    paper_trading: bool,
}

fn generate_agents(count: usize, category: &str, tp_sl_preset: &str, total_capital: f64, paper_trading: bool) -> Vec<GeneratedAgent> {
    let (user_tp, user_sl) = parse_tp_sl(tp_sl_preset);
    let capital_per_agent = total_capital / count as f64;
    let mut agents = Vec::with_capacity(count);

    for i in 0..count {
        let preset_idx = i % PRESETS.len();
        let preset = &PRESETS[preset_idx];
        let variant = i / PRESETS.len(); // 0 for first 10, 1 for next 10, etc.

        // For preset slots (first 10), use preset TP/SL only if user chose "normal"
        // Otherwise user choice overrides
        let (tp, sl) = if preset.tp > 0.0 && tp_sl_preset == "normal" {
            (preset.tp, preset.sl)
        } else {
            (user_tp, user_sl)
        };

        // Mutate parameters for variants beyond the first 10
        let kelly_mut = if variant > 0 {
            let offset = ((variant as f64) * 0.02) * if variant % 2 == 0 { 1.0 } else { -1.0 };
            (preset.kelly + offset).clamp(0.15, 0.85)
        } else {
            preset.kelly
        };

        let edge_mut = if variant > 0 {
            let offset = ((variant as f64) * 0.01) * if variant % 2 == 0 { -1.0 } else { 1.0 };
            (preset.edge + offset).clamp(0.03, 0.20)
        } else {
            preset.edge
        };

        let position_mut = if variant > 0 {
            let offset = ((variant as f64) * 0.01) * if variant % 2 == 0 { 1.0 } else { -1.0 };
            (preset.position + offset).clamp(0.03, 1.00)
        } else {
            preset.position
        };

        let strategy_name = if variant > 0 {
            format!("{}-v{}", preset.name, variant + 1)
        } else {
            preset.name.to_string()
        };

        let agent_id = format!("agent-{:03}", i + 1);

        agents.push(GeneratedAgent {
            id: agent_id,
            strategy: strategy_name,
            kelly: kelly_mut,
            edge: edge_mut,
            position: position_mut,
            tp,
            sl,
            interval: preset.interval,
            capital: capital_per_agent,
            category: category.to_string(),
            paper_trading,
        });
    }

    agents
}

fn load_base_env() -> HashMap<String, String> {
    let mut env_vars = HashMap::new();
    // Read from root .env
    if let Ok(content) = fs::read_to_string(".env") {
        for line in content.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            if let Some((key, val)) = line.split_once('=') {
                env_vars.insert(key.trim().to_string(), val.trim().to_string());
            }
        }
    }
    // Also check parent dir
    if let Ok(content) = fs::read_to_string("../.env") {
        for line in content.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            if let Some((key, val)) = line.split_once('=') {
                env_vars.entry(key.trim().to_string())
                    .or_insert_with(|| val.trim().to_string());
            }
        }
    }
    env_vars
}

fn generate_env_file(agent: &GeneratedAgent, base_env: &HashMap<String, String>) -> String {
    let mut lines = Vec::new();
    lines.push(format!("# Auto-generated config for {} ({})", agent.id, agent.strategy));

    // Copy all base env vars first
    let override_keys = [
        "INITIAL_BALANCE", "KELLY_FRACTION", "MIN_EDGE_THRESHOLD",
        "MAX_POSITION_PCT", "EXIT_TP_PCT", "EXIT_SL_PCT",
        "SCAN_INTERVAL_SECS", "CATEGORY_FILTER", "DB_PATH",
        "PAPER_TRADING", "MAX_OPEN_POSITIONS", "SIM_FILLS_ENABLED",
        "MIN_CONFIDENCE",
    ];

    for (key, val) in base_env {
        if !override_keys.contains(&key.as_str()) {
            lines.push(format!("{}={}", key, val));
        }
    }

    // Agent-specific overrides
    lines.push(format!("INITIAL_BALANCE={:.2}", agent.capital));
    lines.push(format!("KELLY_FRACTION={:.2}", agent.kelly));
    lines.push(format!("MIN_EDGE_THRESHOLD={:.2}", agent.edge));
    lines.push(format!("MAX_POSITION_PCT={:.2}", agent.position));
    lines.push(format!("EXIT_TP_PCT={:.2}", agent.tp));
    lines.push(format!("EXIT_SL_PCT={:.2}", agent.sl));
    lines.push(format!("SCAN_INTERVAL_SECS={}", agent.interval));
    lines.push(format!("CATEGORY_FILTER={}", agent.category));
    lines.push(format!("DB_PATH=data/{}.db", agent.id));
    lines.push(format!("PAPER_TRADING={}", if agent.paper_trading { "true" } else { "false" }));

    // Extreme preset overrides: focus fire + no fill rejection + high confidence
    if agent.position >= 0.50 {
        lines.push("MAX_OPEN_POSITIONS=1".to_string());
        lines.push("SIM_FILLS_ENABLED=false".to_string());
        lines.push("MIN_CONFIDENCE=0.70".to_string());
    }

    lines.join("\n")
}

fn find_binary() -> Option<PathBuf> {
    // Check common locations for polyagent binary
    let candidates = [
        // Workspace root target (when built from workspace)
        "../target/release/polyagent.exe",
        "../target/release/polyagent",
        "../target/debug/polyagent.exe",
        "../target/debug/polyagent",
        // Per-crate target (when built from agent/)
        "target/release/polyagent.exe",
        "target/release/polyagent",
        "../agent/target/release/polyagent.exe",
        "../agent/target/release/polyagent",
        "target/debug/polyagent.exe",
        "target/debug/polyagent",
    ];
    for c in &candidates {
        let p = Path::new(c);
        if p.exists() {
            return Some(p.to_path_buf());
        }
    }
    None
}

fn check_process_health(agents: &mut HashMap<String, ChildAgent>) -> Vec<String> {
    let mut dead = Vec::new();
    for (id, agent) in agents.iter_mut() {
        match agent.process.try_wait() {
            Ok(Some(_status)) => {
                dead.push(id.clone());
            }
            Ok(None) => {} // still running
            Err(_) => {
                dead.push(id.clone());
            }
        }
    }
    dead
}

// ── DB helpers ──

fn open_db_for(agent_id: &str) -> Option<rusqlite::Connection> {
    let path = format!("data/{}.db", agent_id);
    if !Path::new(&path).exists() {
        return None;
    }
    rusqlite::Connection::open_with_flags(
        &path,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .ok()
}

fn find_all_agent_dbs() -> Vec<String> {
    let mut ids = Vec::new();
    if let Ok(entries) = fs::read_dir("data") {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            if name.ends_with(".db") && name.starts_with("agent-") {
                let id = name.trim_end_matches(".db").to_string();
                ids.push(id);
            }
        }
    }
    ids.sort();
    ids
}

fn read_agent_metrics(agent_id: &str, initial_bal: f64, interval: u64) -> AgentInfo {
    let mut info = AgentInfo {
        id: agent_id.to_string(),
        strategy: String::new(),
        status: "stopped".to_string(),
        pnl: 0.0,
        balance: initial_bal,
        initial_balance: initial_bal,
        trades_count: 0,
        win_rate: 0.0,
        last_cycle: String::new(),
        category: String::new(),
        tp_sl: String::new(),
        phase: "stopped".to_string(),
        phase_detail: String::new(),
        interval,
        price_check_interval: 180, // default 3 minutes
        judge_model: "gemini".to_string(), // default to gemini
        open_position: None,
    };

    let Some(conn) = open_db_for(agent_id) else {
        return info;
    };

    // Balance from cycles
    if let Ok(bal) = conn.query_row(
        "SELECT balance_after FROM cycles ORDER BY id DESC LIMIT 1",
        [],
        |row| row.get::<_, String>(0),
    ) {
        info.balance = bal.parse().unwrap_or(initial_bal);
        info.pnl = info.balance - initial_bal;
    }

    // Trade counts
    if let Ok((total, wins, losses)) = conn.query_row(
        "SELECT COUNT(*), \
         SUM(CASE WHEN status='Won' THEN 1 ELSE 0 END), \
         SUM(CASE WHEN status='Lost' THEN 1 ELSE 0 END) \
         FROM trades",
        [],
        |row| Ok((
            row.get::<_, i64>(0).unwrap_or(0),
            row.get::<_, i64>(1).unwrap_or(0),
            row.get::<_, i64>(2).unwrap_or(0),
        )),
    ) {
        info.trades_count = total;
        if wins + losses > 0 {
            info.win_rate = (wins as f64 / (wins + losses) as f64) * 100.0;
        }
    } else if let Err(e) = conn.query_row(
        "SELECT COUNT(*) FROM trades", [], |_| Ok(()),
    ) {
        // Log error if query failed (except purely missing table)
        if !e.to_string().contains("no such table") {
            eprintln!("Error reading trades metrics for {}: {}", agent_id, e);
        }
    }

    // Last cycle (intentionally not used, just checking if exists)
    let _ = conn.query_row(
        "SELECT timestamp FROM cycles ORDER BY id DESC LIMIT 1",
        [],
        |row| row.get::<_, String>(0),
    );

    // Agent Phase/Status
    if let Ok((phase, detail)) = conn.query_row(
        "SELECT phase, details FROM agent_status WHERE id = 'current'",
        [],
        |row| Ok((
            row.get::<_, String>(0).unwrap_or_else(|_| "stopped".to_string()),
            row.get::<_, String>(1).unwrap_or_default(),
        )),
    ) {
        info.phase = phase;
        info.phase_detail = detail;
    }

    // Load price_check_interval and judge_model from config
    let config_path = format!("configs/{}.env", agent_id);
    if let Ok(content) = fs::read_to_string(&config_path) {
        for line in content.lines() {
            if let Some(val) = line.strip_prefix("PRICE_CHECK_SECS=") {
                info.price_check_interval = val.trim().parse().unwrap_or(180);
            }
            if let Some(val) = line.strip_prefix("JUDGE_MODEL=") {
                info.judge_model = val.trim().to_lowercase();
            }
        }
    }

    // Open Position details
    if let Ok((mid, q, dir, entry, tp, sl, mkt_id, bet_size_str, ts)) = conn.query_row(
        "SELECT
            (SELECT mid FROM price_log WHERE market_id = t.market_id ORDER BY id DESC LIMIT 1) as current_price,
            t.question, t.direction, t.entry_price, t.take_profit, t.stop_loss, t.market_id,
            COALESCE(t.bet_size, '0'), t.timestamp
         FROM trades t
         WHERE t.status = 'Open'
         ORDER BY t.rowid DESC LIMIT 1",
        [],
        |row| Ok((
            row.get::<_, String>(0).ok(),
            row.get::<_, String>(1).unwrap_or_default(),
            row.get::<_, String>(2).unwrap_or_default(),
            row.get::<_, String>(3).unwrap_or_default(),
            row.get::<_, String>(4).unwrap_or_default(),
            row.get::<_, String>(5).unwrap_or_default(),
            row.get::<_, String>(6).unwrap_or_default(),
            row.get::<_, String>(7).unwrap_or_default(),
            row.get::<_, String>(8).unwrap_or_default(),
        )),
    ) {
        if let Some(price_str) = mid {
             let current: f64 = price_str.parse().unwrap_or(0.0);
             let entry_f: f64 = entry.parse().unwrap_or(0.0);
             let tp_f: f64 = tp.parse().unwrap_or(0.0);
             let sl_f: f64 = sl.parse().unwrap_or(0.0);
             let bet_size_f: f64 = bet_size_str.parse().unwrap_or(0.0);

             let pnl_pct = if entry_f > 0.0 {
                 if dir == "Long" {
                     (current - entry_f) / entry_f * 100.0
                 } else {
                     (entry_f - current) / entry_f * 100.0
                 }
             } else { 0.0 };

             let unrealized_pnl = pnl_pct / 100.0 * bet_size_f;

             // Get last price check time from cycles table
             let last_check = conn.prepare("SELECT timestamp FROM cycles ORDER BY id DESC LIMIT 1")
                 .and_then(|mut stmt| stmt.query_row([], |row| row.get::<_, String>(0)))
                 .ok();

             info.open_position = Some(OpenPosition {
                 market_id: mkt_id,
                 question: q,
                 direction: dir,
                 entry_price: entry_f,
                 current_price: current,
                 pnl_pct,
                 tp_price: tp_f,
                 sl_price: sl_f,
                 last_price_check: last_check,
                 bet_size: bet_size_f,
                 unrealized_pnl,
                 timestamp: ts,
             });
        }
    }

    
    info
}

fn read_daily_stats() -> Vec<DailyStats> {
    let mut workbook: HashMap<String, DailyStats> = HashMap::new();
    let all_ids = find_all_agent_dbs();
    let mut total_initial = 0.0;

    for agent_id in all_ids {
        // Get initial balance for this agent
        let mut initial = 100.0;
        let config_path = format!("configs/{}.env", agent_id);
        if let Ok(content) = fs::read_to_string(&config_path) {
            for line in content.lines() {
                if let Some(val) = line.strip_prefix("INITIAL_BALANCE=") {
                    initial = val.trim().parse().unwrap_or(100.0);
                }
            }
        }
        total_initial += initial;

        if let Some(conn) = open_db_for(&agent_id) {
            // Query trades grouped by day
            let mut stmt = conn.prepare(
                "SELECT STRFTIME('%Y-%m-%d', timestamp) as day, 
                        SUM(pnl), 
                        COUNT(*),
                        SUM(CASE WHEN status='Won' THEN 1 ELSE 0 END),
                        SUM(CASE WHEN status='Lost' THEN 1 ELSE 0 END)
                 FROM trades 
                 GROUP BY day"
            ).unwrap();
            
            let rows = stmt.query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, f64>(1).unwrap_or(0.0),
                    row.get::<_, i64>(2).unwrap_or(0),
                    row.get::<_, i64>(3).unwrap_or(0),
                    row.get::<_, i64>(4).unwrap_or(0),
                ))
            }).unwrap();

            for row in rows {
                if let Ok((day, pnl, count, wins, losses)) = row {
                    let entry = workbook.entry(day.clone()).or_insert(DailyStats {
                        date: day,
                        pnl: 0.0,
                        balance: 0.0,
                        trades_count: 0,
                        wins: 0,
                        losses: 0,
                    });
                    entry.pnl += pnl;
                    entry.trades_count += count;
                    entry.wins += wins;
                    entry.losses += losses;
                }
            }
        }
    }

    let mut result: Vec<DailyStats> = workbook.into_values().collect();
    result.sort_by(|a, b| a.date.cmp(&b.date));

    // Calculate cumulative balance
    let mut current_balance = total_initial;
    for day_stat in &mut result {
        current_balance += day_stat.pnl;
        day_stat.balance = current_balance;
    }

    result
}

fn read_agent_trades(agent_id: &str) -> Vec<TradeRecord> {
    let Some(conn) = open_db_for(agent_id) else {
        return vec![];
    };

    conn.prepare(
        "SELECT id, timestamp, question, direction, entry_price, fair_value, \
         edge, bet_size, pnl, status, balance_after, \
         COALESCE(category,''), COALESCE(trade_mode,''), \
         COALESCE(exit_reason,''), COALESCE(take_profit,''), COALESCE(stop_loss,''), \
         COALESCE(entry_gas_fee,'0'), COALESCE(exit_gas_fee,'0'), \
         COALESCE(entry_slippage,'0'), COALESCE(exit_slippage,'0'), \
         COALESCE(platform_fee,'0'), COALESCE(maker_taker_fee,'0') \
         FROM trades ORDER BY rowid DESC LIMIT 50",
    )
    .and_then(|mut stmt| {
        stmt.query_map([], |row| {
            let pnl_str: String = row.get(8).unwrap_or_default();
            let entry_gas: String = row.get(16).unwrap_or_default();
            let exit_gas: String = row.get(17).unwrap_or_default();
            let entry_slip: String = row.get(18).unwrap_or_default();
            let exit_slip: String = row.get(19).unwrap_or_default();
            let plat_fee: String = row.get(20).unwrap_or_default();
            let mt_fee: String = row.get(21).unwrap_or_default();

            let pnl_f: f64 = pnl_str.parse().unwrap_or(0.0);
            let eg: f64 = entry_gas.parse().unwrap_or(0.0);
            let xg: f64 = exit_gas.parse().unwrap_or(0.0);
            let es: f64 = entry_slip.parse().unwrap_or(0.0);
            let xs: f64 = exit_slip.parse().unwrap_or(0.0);
            let pf: f64 = plat_fee.parse().unwrap_or(0.0);
            let mt: f64 = mt_fee.parse().unwrap_or(0.0);
            let total = eg + xg + es + xs + pf + mt;
            let net = pnl_f - xg - xs - pf; // entry gas/slip already deducted from balance

            Ok(TradeRecord {
                id: row.get(0).unwrap_or_default(),
                timestamp: row.get(1).unwrap_or_default(),
                question: row.get(2).unwrap_or_default(),
                direction: row.get(3).unwrap_or_default(),
                entry_price: row.get(4).unwrap_or_default(),
                fair_value: row.get(5).unwrap_or_default(),
                edge: row.get(6).unwrap_or_default(),
                bet_size: row.get(7).unwrap_or_default(),
                pnl: pnl_str,
                status: row.get(9).unwrap_or_default(),
                balance_after: row.get(10).unwrap_or_default(),
                category: row.get(11).unwrap_or_default(),
                trade_mode: row.get(12).unwrap_or_default(),
                exit_reason: row.get(13).unwrap_or_default(),
                take_profit: row.get(14).unwrap_or_default(),
                stop_loss: row.get(15).unwrap_or_default(),
                entry_gas_fee: entry_gas,
                exit_gas_fee: exit_gas,
                entry_slippage: entry_slip,
                exit_slippage: exit_slip,
                platform_fee: plat_fee,
                maker_taker_fee: mt_fee,
                total_fees: format!("{:.4}", total),
                net_pnl: format!("{:.4}", net),
            })
        })
        .map(|rows| rows.flatten().collect())
    })
    .unwrap_or_default()
}

fn read_agent_analyses(agent_id: &str) -> Vec<AnalysisRecord> {
    let Some(conn) = open_db_for(agent_id) else {
        return vec![];
    };

    conn.prepare(
        "SELECT timestamp, question, current_price, fair_value, edge, direction, \
         should_trade, COALESCE(confidence,''), COALESCE(model,''), api_cost, \
         COALESCE(reasoning,'') \
         FROM analyses ORDER BY id DESC LIMIT 30",
    )
    .and_then(|mut stmt| {
        stmt.query_map([], |row| {
            Ok(AnalysisRecord {
                timestamp: row.get(0).unwrap_or_default(),
                question: row.get(1).unwrap_or_default(),
                current_price: row.get(2).unwrap_or_default(),
                fair_value: row.get(3).unwrap_or_default(),
                edge: row.get(4).unwrap_or_default(),
                direction: row.get(5).unwrap_or_default(),
                should_trade: row.get::<_, i64>(6).unwrap_or(0) != 0,
                confidence: row.get(7).unwrap_or_default(),
                model: row.get(8).unwrap_or_default(),
                api_cost: row.get(9).unwrap_or_default(),
                reasoning: row.get(10).unwrap_or_default(),
            })
        })
        .map(|rows| rows.flatten().collect())
    })
    .unwrap_or_default()
}

fn read_agent_cycles(agent_id: &str) -> Vec<CycleRecord> {
    let Some(conn) = open_db_for(agent_id) else {
        return vec![];
    };

    conn.prepare(
        "SELECT timestamp, markets_scanned, markets_analyzed, trades_placed, \
         COALESCE(api_cost_cycle,'0'), COALESCE(balance_after,'0') \
         FROM cycles ORDER BY id DESC LIMIT 20",
    )
    .and_then(|mut stmt| {
        stmt.query_map([], |row| {
            Ok(CycleRecord {
                timestamp: row.get(0).unwrap_or_default(),
                markets_scanned: row.get(1).unwrap_or(0),
                markets_analyzed: row.get(2).unwrap_or(0),
                trades_placed: row.get(3).unwrap_or(0),
                api_cost: row.get(4).unwrap_or_default(),
                balance_after: row.get(5).unwrap_or_default(),
            })
        })
        .map(|rows| rows.flatten().collect())
    })
    .unwrap_or_default()
}

// ── Main ──

#[tokio::main]
async fn main() {
    let port: u16 = std::env::var("DASHBOARD_PORT")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(3000);

    // Ensure required directories exist
    fs::create_dir_all("data").ok();
    fs::create_dir_all("configs").ok();

    println!("══════════════════════════════════════════════");
    println!("  Polymarket Multi-Agent Dashboard");
    println!("  http://localhost:{}", port);
    println!("══════════════════════════════════════════════");

    let state: SharedState = Arc::new(AppState {
        start_time: Instant::now(),
        agents: Mutex::new(HashMap::new()),
    });

    let app = Router::new()
        .route("/", get(serve_html))
        .route("/api/status", get(api_status))
        .route("/api/agents", get(api_agents))
        .route("/api/agent/:id", get(api_agent_detail))
        .route("/api/activity", get(api_activity))
        .route("/api/calendar", get(api_calendar))
        .route("/api/start", post(api_start))
        .route("/api/stop", post(api_stop_all))
        .route("/api/stop/:id", post(api_stop_one))
        .route("/api/reset-db/:id", post(api_reset_db))
        .route("/api/reset-all/:id", post(api_reset_all))
        .layer(CorsLayer::permissive())
        .with_state(state);

    let addr = format!("0.0.0.0:{}", port);
    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .expect("Failed to bind dashboard port");
    println!("[+] Dashboard running at http://localhost:{}", port);
    axum::serve(listener, app).await.expect("serve failed");
}

// ── API Handlers ──

async fn serve_html() -> Html<&'static str> {
    Html(DASHBOARD_HTML)
}

async fn api_status(State(state): State<SharedState>) -> Json<AgentStatus> {
    let mut agents_map = state.agents.lock().await;

    // Check health
    let dead_ids = check_process_health(&mut agents_map);
    let dead_count = dead_ids.len();
    let running_count = agents_map.len() - dead_count;
    let total_count = agents_map.len();

    // Aggregate metrics across all agent DBs
    let all_ids = find_all_agent_dbs();
    let mut total_balance = 0.0;
    let mut total_initial = 0.0;
    let mut total_trades: i64 = 0;
    let mut total_wins: i64 = 0;
    let mut total_losses: i64 = 0;
    let mut total_open: i64 = 0;
    let mut total_locked = 0.0;
    let mut total_api_cost = 0.0;
    let mut total_fees_all = 0.0;
    let mut total_gas_all = 0.0;
    let mut total_slippage_all = 0.0;
    let mut total_platform_all = 0.0;
    let mut latest_cycle = String::new();
    let mut total_scanned: i64 = 0;
    let mut total_analyzed: i64 = 0;
    let mut any_db = false;

    for agent_id in &all_ids {
        let initial_bal = agents_map.get(agent_id)
            .map(|a| a.initial_balance)
            .unwrap_or(100.0);

        let Some(conn) = open_db_for(agent_id) else { continue };
        any_db = true;

        // Initial balance from config
        let config_path = format!("configs/{}.env", agent_id);
        let mut init_bal = initial_bal;
        if let Ok(content) = fs::read_to_string(&config_path) {
            for line in content.lines() {
                if let Some(val) = line.strip_prefix("INITIAL_BALANCE=") {
                    init_bal = val.trim().parse().unwrap_or(initial_bal);
                }
            }
        }
        total_initial += init_bal;

        // Balance
        if let Ok(bal) = conn.query_row(
            "SELECT balance_after FROM cycles ORDER BY id DESC LIMIT 1",
            [],
            |row| row.get::<_, String>(0),
        ) {
            total_balance += bal.parse::<f64>().unwrap_or(init_bal);
        } else {
            total_balance += init_bal;
        }

        // Trades
        if let Ok((count, wins, losses, open)) = conn.query_row(
            "SELECT COUNT(*), \
             SUM(CASE WHEN status='Won' THEN 1 ELSE 0 END), \
             SUM(CASE WHEN status='Lost' THEN 1 ELSE 0 END), \
             SUM(CASE WHEN status='Open' THEN 1 ELSE 0 END) \
             FROM trades",
            [],
            |row| Ok((
                row.get::<_, i64>(0).unwrap_or(0),
                row.get::<_, i64>(1).unwrap_or(0),
                row.get::<_, i64>(2).unwrap_or(0),
                row.get::<_, i64>(3).unwrap_or(0),
            )),
        ) {
            total_trades += count;
            total_wins += wins;
            total_losses += losses;
            total_open += open;
        }

        // Locked
        if let Ok(locked) = conn.query_row(
            "SELECT COALESCE(SUM(CAST(bet_size AS REAL)), 0) FROM trades WHERE status='Open'",
            [],
            |row| row.get::<_, f64>(0),
        ) {
            total_locked += locked;
        }

        // Latest cycle
        if let Ok((ts, scanned, analyzed)) = conn.query_row(
            "SELECT timestamp, markets_scanned, markets_analyzed FROM cycles ORDER BY id DESC LIMIT 1",
            [],
            |row| Ok((
                row.get::<_, String>(0).unwrap_or_default(),
                row.get::<_, i64>(1).unwrap_or(0),
                row.get::<_, i64>(2).unwrap_or(0),
            )),
        ) {
            if ts > latest_cycle {
                latest_cycle = ts;
            }
            total_scanned += scanned;
            total_analyzed += analyzed;
        }

        // API cost
        if let Ok(cost) = conn.query_row(
            "SELECT COALESCE(SUM(CAST(api_cost_cycle AS REAL)), 0) FROM cycles",
            [],
            |row| row.get::<_, f64>(0),
        ) {
            total_api_cost += cost;
        }

        // Fees (gas, slippage, platform)
        if let Ok((gas, slip, plat)) = conn.query_row(
            "SELECT \
             COALESCE(SUM(CAST(COALESCE(entry_gas_fee,'0') AS REAL) + CAST(COALESCE(exit_gas_fee,'0') AS REAL)), 0), \
             COALESCE(SUM(CAST(COALESCE(entry_slippage,'0') AS REAL) + CAST(COALESCE(exit_slippage,'0') AS REAL)), 0), \
             COALESCE(SUM(CAST(COALESCE(platform_fee,'0') AS REAL)), 0) \
             FROM trades",
            [],
            |row| Ok((
                row.get::<_, f64>(0).unwrap_or(0.0),
                row.get::<_, f64>(1).unwrap_or(0.0),
                row.get::<_, f64>(2).unwrap_or(0.0),
            )),
        ) {
            total_gas_all += gas;
            total_slippage_all += slip;
            total_platform_all += plat;
            total_fees_all += gas + slip + plat;
        }
    }

    // If no agent DBs found, try legacy single agent.db
    if !any_db {
        if let Some(conn) = open_legacy_db() {
            any_db = true;
            total_initial = 100.0;
            if let Ok(bal) = conn.query_row(
                "SELECT balance_after FROM cycles ORDER BY id DESC LIMIT 1",
                [],
                |row| row.get::<_, String>(0),
            ) {
                total_balance = bal.parse().unwrap_or(100.0);
            } else {
                total_balance = 100.0;
            }
        }
    }

    let total_pnl = total_balance - total_initial;
    let roi = if total_initial > 0.0 { (total_pnl / total_initial) * 100.0 } else { 0.0 };
    let win_rate = if total_wins + total_losses > 0 {
        (total_wins as f64 / (total_wins + total_losses) as f64) * 100.0
    } else {
        0.0
    };

    Json(AgentStatus {
        balance: total_balance,
        initial_balance: total_initial,
        pnl: total_pnl,
        roi,
        win_rate,
        trades_count: total_trades,
        wins: total_wins,
        losses: total_losses,
        open_count: total_open,
        locked_balance: total_locked,
        api_cost: total_api_cost,
        total_fees: total_fees_all,
        total_gas: total_gas_all,
        total_slippage: total_slippage_all,
        total_platform: total_platform_all,
        last_cycle: latest_cycle,
        markets_scanned: total_scanned,
        markets_analyzed: total_analyzed,
        uptime_secs: state.start_time.elapsed().as_secs(),
        db_found: any_db,
        total_agents: total_count,
        running_agents: running_count,
        dead_agents: dead_count,
    })
}

async fn api_agents(State(state): State<SharedState>) -> Json<Vec<AgentInfo>> {
    let mut agents_map = state.agents.lock().await;
    let dead_ids = check_process_health(&mut agents_map);
    let dead_set: HashSet<_> = dead_ids.iter().collect();

    let mut result = Vec::new();
    let mut running_ids = HashSet::new();

    // 1. Running (or Dead) agents from memory
    for (id, child_agent) in agents_map.iter() {
        running_ids.insert(id.clone());
        let mut info = read_agent_metrics(id, child_agent.initial_balance, child_agent.interval);
        info.strategy = child_agent.strategy.clone();
        info.category = child_agent.category.clone();
        info.tp_sl = child_agent.tp_sl.clone();
        
        if dead_set.contains(id) {
            info.status = "dead".to_string();
            info.phase = "dead".to_string();
        } else {
            info.status = "running".to_string();
        }
        
        result.push(info);
    }

    // 2. Stopped agents (from DBs)
    let all_ids = find_all_agent_dbs();
    for id in all_ids {
        if !running_ids.contains(&id) {
            let mut info = read_agent_metrics(&id, 100.0, 0); // Default 100 if unknown
            info.status = "stopped".to_string();
            result.push(info);
        }
    }

    result.sort_by(|a, b| a.id.cmp(&b.id));
    Json(result)
}

async fn api_agent_detail(
    AxumPath(id): AxumPath<String>,
    State(state): State<SharedState>,
) -> Json<AgentDetail> {
    let mut info = read_agent_metrics(&id, 100.0, 0);

    // If running, override fields
    let agents_map = state.agents.lock().await;
    if let Some(child) = agents_map.get(&id) {
        info.strategy = child.strategy.clone();
        info.category = child.category.clone();
        info.tp_sl = child.tp_sl.clone();
        info.status = "running".to_string();
        info.initial_balance = child.initial_balance;
        info.interval = child.interval;
        if info.balance == 0.0 { info.balance = child.initial_balance; }
    }
    drop(agents_map);

    let trades = read_agent_trades(&id);
    let analyses = read_agent_analyses(&id);
    let cycles = read_agent_cycles(&id);
    let open_positions = read_open_positions(&id);
    let performance = read_performance_stats(&id);
    let tp_sl_settings = read_tp_sl_settings(&id);

    Json(AgentDetail { info, trades, analyses, cycles, open_positions, performance, tp_sl_settings })
}

async fn api_activity(State(state): State<SharedState>) -> Json<Vec<ActivityEvent>> {
    let agents_map = state.agents.lock().await;
    let all_ids = find_all_agent_dbs();
    let mut events: Vec<ActivityEvent> = Vec::new();

    for agent_id in &all_ids {
        let Some(conn) = open_db_for(agent_id) else { continue };

        // Recent trades
        let trades: Vec<(String, String, String, String, String, String)> = conn
            .prepare("SELECT timestamp, question, direction, entry_price, pnl, status FROM trades ORDER BY rowid DESC LIMIT 3")
            .and_then(|mut stmt| {
                stmt.query_map([], |row| {
                    Ok((
                        row.get::<_, String>(0).unwrap_or_default(),
                        row.get::<_, String>(1).unwrap_or_default(),
                        row.get::<_, String>(2).unwrap_or_default(),
                        row.get::<_, String>(3).unwrap_or_default(),
                        row.get::<_, String>(4).unwrap_or_default(),
                        row.get::<_, String>(5).unwrap_or_default(),
                    ))
                })
                .map(|rows| rows.flatten().collect())
            })
            .unwrap_or_default();

        for row in trades {
            let q = if row.1.len() > 35 { format!("{}...", &row.1[..35]) } else { row.1.clone() };
            let pnl_f: f64 = row.4.parse().unwrap_or(0.0);
            let summary = if row.5 == "Open" {
                format!("Opened {} '{}' @ ${}", row.2, q, row.3)
            } else {
                let sign = if pnl_f >= 0.0 { "+" } else { "" };
                format!("{} '{}' {}${:.4} PnL", row.5, q, sign, pnl_f)
            };
            events.push(ActivityEvent {
                timestamp: row.0,
                event_type: "trade".to_string(),
                summary,
                agent_id: agent_id.clone(),
            });
        }

        // Recent cycles
        let cycles: Vec<(String, i64, i64, i64)> = conn
            .prepare("SELECT timestamp, markets_scanned, markets_analyzed, trades_placed FROM cycles ORDER BY id DESC LIMIT 2")
            .and_then(|mut stmt| {
                stmt.query_map([], |row| {
                    Ok((
                        row.get::<_, String>(0).unwrap_or_default(),
                        row.get::<_, i64>(1).unwrap_or(0),
                        row.get::<_, i64>(2).unwrap_or(0),
                        row.get::<_, i64>(3).unwrap_or(0),
                    ))
                })
                .map(|rows| rows.flatten().collect())
            })
            .unwrap_or_default();

        for row in cycles {
            events.push(ActivityEvent {
                timestamp: row.0,
                event_type: "cycle".to_string(),
                summary: format!("Scanned {} markets, analyzed {}, traded {}", row.1, row.2, row.3),
                agent_id: agent_id.clone(),
            });
        }
    }

    // Also check legacy DB if no agent DBs found
    if all_ids.is_empty() {
        if let Some(conn) = open_legacy_db() {
            let trades: Vec<(String, String, String, String, String, String)> = conn
                .prepare("SELECT timestamp, question, direction, entry_price, pnl, status FROM trades ORDER BY rowid DESC LIMIT 5")
                .and_then(|mut stmt| {
                    stmt.query_map([], |row| {
                        Ok((
                            row.get::<_, String>(0).unwrap_or_default(),
                            row.get::<_, String>(1).unwrap_or_default(),
                            row.get::<_, String>(2).unwrap_or_default(),
                            row.get::<_, String>(3).unwrap_or_default(),
                            row.get::<_, String>(4).unwrap_or_default(),
                            row.get::<_, String>(5).unwrap_or_default(),
                        ))
                    })
                    .map(|rows| rows.flatten().collect())
                })
                .unwrap_or_default();

            for row in trades {
                let q = if row.1.len() > 35 { format!("{}...", &row.1[..35]) } else { row.1.clone() };
                let pnl_f: f64 = row.4.parse().unwrap_or(0.0);
                let summary = if row.5 == "Open" {
                    format!("Opened {} '{}' @ ${}", row.2, q, row.3)
                } else {
                    let sign = if pnl_f >= 0.0 { "+" } else { "" };
                    format!("{} '{}' {}${:.4} PnL", row.5, q, sign, pnl_f)
                };
                events.push(ActivityEvent {
                    timestamp: row.0,
                    event_type: "trade".to_string(),
                    summary,
                    agent_id: "legacy".to_string(),
                });
            }
        }
    }

    drop(agents_map);

    events.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));
    events.truncate(50);

    Json(events)
}

async fn api_calendar() -> Json<CalendarResponse> {
    let stats = read_daily_stats();
    Json(CalendarResponse { stats })
}

async fn api_start(
    State(state): State<SharedState>,
    Json(req): Json<StartRequest>,
) -> Json<StartResponse> {
    let count = req.count.clamp(1, 100);
    let capital = if req.capital > 0.0 { req.capital } else { 100.0 };
    let category = if req.category.is_empty() { "all".to_string() } else { req.category };
    let tp_sl = if req.tp_sl.is_empty() { "normal".to_string() } else { req.tp_sl };
    let paper_trading = req.mode != "live";

    // Validate: live trading requires WALLET_PRIVATE_KEY in base env
    let base_env = load_base_env();
    if !paper_trading {
        let has_wallet = base_env.get("WALLET_PRIVATE_KEY")
            .map(|v| !v.is_empty())
            .unwrap_or(false);
        if !has_wallet {
            return Json(StartResponse {
                ok: false,
                message: "LIVE TRADING requires WALLET_PRIVATE_KEY in .env file. Set it first or use Paper Trading mode.".to_string(),
                agents_started: 0,
            });
        }
    }

    let binary = match find_binary() {
        Some(b) => {
            println!("Resolved polyagent binary: {:?}", b);
            b
        },
        None => {
            let cwd = std::env::current_dir().unwrap_or_default();
            println!("polyagent binary not found. Checked relevant paths from {:?}", cwd);
            return Json(StartResponse {
                ok: false,
                message: format!("polyagent binary not found. Run 'cargo build --release' first. CWD: {:?}", cwd),
                agents_started: 0,
            });
        }
    };

    let generated = generate_agents(count, &category, &tp_sl, capital, paper_trading);
    let mut started = 0;

    let mut agents_map = state.agents.lock().await;

    for (idx, agent) in generated.iter().enumerate() {
        // Skip if already running
        if agents_map.contains_key(&agent.id) {
            continue;
        }

        // Write config file
        let config_content = generate_env_file(agent, &base_env);
        let config_path = format!("configs/{}.env", agent.id);
        if fs::write(&config_path, &config_content).is_err() {
            continue;
        }

        // Stagger agent startup to avoid API rate limits
        // Delay: 0s for first agent, then 2s between each
        if idx > 0 {
            drop(agents_map); // Release lock during sleep
            tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
            agents_map = state.agents.lock().await;
        }

        // Spawn process
        let child = tokio::process::Command::new(&binary)
            .arg("--config-file")
            .arg(&config_path)
            .arg("--agent-id")
            .arg(&agent.id)
            .arg("--yes") // Ensure non-interactive mode
            .stdout(std::process::Stdio::inherit())
            .stderr(std::process::Stdio::inherit())
            .spawn();

        match child {
            Ok(process) => {
                let tp_sl_label = format!("{:.0}%/{:.0}%", agent.tp * 100.0, agent.sl * 100.0);
                agents_map.insert(agent.id.clone(), ChildAgent {
                    process,
                    agent_id: agent.id.clone(),
                    strategy: agent.strategy.clone(),
                    category: agent.category.clone(),
                    tp_sl: tp_sl_label,
                    initial_balance: agent.capital,
                    interval: agent.interval,
                });
                started += 1;
            }
            Err(e) => {
                eprintln!("Failed to spawn {}: {}", agent.id, e);
            }
        }
    }

    Json(StartResponse {
        ok: started > 0,
        message: format!("Started {} of {} agents", started, count),
        agents_started: started,
    })
}

async fn api_stop_all(State(state): State<SharedState>) -> Json<StopResponse> {
    let mut agents_map = state.agents.lock().await;
    let count = agents_map.len();

    for (_id, agent) in agents_map.iter_mut() {
        agent.process.kill().await.ok();
    }
    agents_map.clear();

    Json(StopResponse {
        ok: true,
        message: format!("Stopped {} agents", count),
        agents_stopped: count,
    })
}

async fn api_stop_one(
    AxumPath(id): AxumPath<String>,
    State(state): State<SharedState>,
) -> Json<StopResponse> {
    let mut agents_map = state.agents.lock().await;

    if let Some(mut agent) = agents_map.remove(&id) {
        agent.process.kill().await.ok();
        Json(StopResponse {
            ok: true,
            message: format!("Stopped agent {}", id),
            agents_stopped: 1,
        })
    } else {
        Json(StopResponse {
            ok: false,
            message: format!("Agent {} not found", id),
            agents_stopped: 0,
        })
    }
}

fn read_open_positions(agent_id: &str) -> Vec<OpenPositionDetail> {
    let Some(conn) = open_db_for(agent_id) else { return vec![] };

    // Get last price check time
    let last_check: Option<String> = conn.prepare("SELECT timestamp FROM cycles ORDER BY id DESC LIMIT 1")
        .and_then(|mut stmt| stmt.query_row([], |row| row.get::<_, String>(0)))
        .ok();

    conn.prepare(
        "SELECT t.market_id, t.question, t.direction, t.entry_price, t.bet_size, \
         COALESCE(t.take_profit,''), COALESCE(t.stop_loss,''), COALESCE(t.trade_mode,''), t.timestamp, \
         (SELECT mid FROM price_log WHERE market_id = t.market_id ORDER BY id DESC LIMIT 1) as current_price \
         FROM trades t WHERE t.status = 'Open' ORDER BY t.rowid DESC LIMIT 20",
    )
    .and_then(|mut stmt| {
        stmt.query_map([], |row| {
            let tp_str: String = row.get(5).unwrap_or_default();
            let sl_str: String = row.get(6).unwrap_or_default();
            let entry_price: f64 = row.get::<_, String>(3).unwrap_or_default().parse().unwrap_or(0.0);
            let bet_size: f64 = row.get::<_, String>(4).unwrap_or_default().parse().unwrap_or(0.0);
            let direction: String = row.get(2).unwrap_or_default();
            let timestamp: String = row.get(8).unwrap_or_default();
            let current_price: f64 = row.get::<_, String>(9).ok()
                .and_then(|s| s.parse().ok()).unwrap_or(0.0);
            let tp: Option<f64> = tp_str.parse().ok();
            let sl: Option<f64> = sl_str.parse().ok();

            // Compute P&L
            let pnl_pct = if entry_price > 0.0 && current_price > 0.0 {
                if direction == "Long" {
                    (current_price - entry_price) / entry_price * 100.0
                } else {
                    (entry_price - current_price) / entry_price * 100.0
                }
            } else { 0.0 };
            let unrealized_pnl = pnl_pct / 100.0 * bet_size;

            // Distance to TP/SL as percentage of entry
            let distance_to_tp_pct = if let Some(tp_val) = tp {
                if entry_price > 0.0 && current_price > 0.0 {
                    if direction == "Long" {
                        (tp_val - current_price) / entry_price * 100.0
                    } else {
                        (current_price - tp_val) / entry_price * 100.0
                    }
                } else { 0.0 }
            } else { 0.0 };

            let distance_to_sl_pct = if let Some(sl_val) = sl {
                if entry_price > 0.0 && current_price > 0.0 {
                    if direction == "Long" {
                        (current_price - sl_val) / entry_price * 100.0
                    } else {
                        (sl_val - current_price) / entry_price * 100.0
                    }
                } else { 0.0 }
            } else { 0.0 };

            // Hours open (parse timestamp like "2025-01-15 10:30:00")
            let hours_open = parse_hours_since(&timestamp);

            Ok(OpenPositionDetail {
                market_id: row.get(0).unwrap_or_default(),
                question: row.get(1).unwrap_or_default(),
                direction,
                entry_price,
                bet_size,
                take_profit: tp,
                stop_loss: sl,
                trade_mode: row.get(7).unwrap_or_default(),
                timestamp,
                current_price,
                pnl_pct,
                unrealized_pnl,
                distance_to_tp_pct,
                distance_to_sl_pct,
                hours_open,
                last_price_check: last_check.clone(),
            })
        })
        .map(|rows| rows.flatten().collect())
    })
    .unwrap_or_default()
}

fn parse_hours_since(timestamp: &str) -> f64 {
    // Parse "YYYY-MM-DD HH:MM:SS" format, compute hours since then
    use std::time::{SystemTime, UNIX_EPOCH};
    // Simple manual parse for "YYYY-MM-DD HH:MM:SS"
    if timestamp.len() < 19 { return 0.0; }
    let parts: Vec<&str> = timestamp.split(|c| c == '-' || c == ' ' || c == ':').collect();
    if parts.len() < 6 { return 0.0; }
    let year: i64 = parts[0].parse().unwrap_or(0);
    let month: i64 = parts[1].parse().unwrap_or(0);
    let day: i64 = parts[2].parse().unwrap_or(0);
    let hour: i64 = parts[3].parse().unwrap_or(0);
    let min: i64 = parts[4].parse().unwrap_or(0);
    let sec: i64 = parts[5].parse().unwrap_or(0);
    if year == 0 { return 0.0; }
    // Approximate days since epoch using a simple formula
    let days = (year - 1970) * 365 + (year - 1969) / 4
        + match month {
            1 => 0, 2 => 31, 3 => 59, 4 => 90, 5 => 120, 6 => 151,
            7 => 181, 8 => 212, 9 => 243, 10 => 273, 11 => 304, 12 => 334,
            _ => 0,
        } + day - 1;
    let entry_secs = days * 86400 + hour * 3600 + min * 60 + sec;
    let now_secs = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs() as i64;
    let diff = now_secs - entry_secs;
    if diff > 0 { diff as f64 / 3600.0 } else { 0.0 }
}

fn read_performance_stats(agent_id: &str) -> PerformanceStats {
    let Some(conn) = open_db_for(agent_id) else { return PerformanceStats::default() };

    let mut stats = PerformanceStats::default();

    // Avg hold, best/worst trade, avg pnl from closed trades
    let _ = conn.query_row(
        "SELECT \
            AVG(CAST(hold_duration_hours AS REAL)), \
            MAX(CAST(pnl AS REAL)), \
            MIN(CAST(pnl AS REAL)), \
            AVG(CAST(pnl AS REAL)), \
            COUNT(*) \
         FROM trades WHERE status IN ('Won', 'Lost')",
        [],
        |row| {
            stats.avg_hold_hours = row.get::<_, f64>(0).unwrap_or(0.0);
            stats.best_trade_pnl = row.get::<_, f64>(1).unwrap_or(0.0);
            stats.worst_trade_pnl = row.get::<_, f64>(2).unwrap_or(0.0);
            stats.avg_pnl = row.get::<_, f64>(3).unwrap_or(0.0);
            stats.total_closed = row.get::<_, i64>(4).unwrap_or(0);
            Ok(())
        },
    );

    // TP/SL hit counts
    let _ = conn.query_row(
        "SELECT \
            SUM(CASE WHEN exit_reason = 'TakeProfit' THEN 1 ELSE 0 END), \
            SUM(CASE WHEN exit_reason = 'StopLoss' THEN 1 ELSE 0 END) \
         FROM trades WHERE status IN ('Won', 'Lost')",
        [],
        |row| {
            stats.tp_hits = row.get::<_, i64>(0).unwrap_or(0);
            stats.sl_hits = row.get::<_, i64>(1).unwrap_or(0);
            Ok(())
        },
    );

    // Fee aggregates
    let _ = conn.query_row(
        "SELECT \
            COALESCE(SUM(CAST(COALESCE(entry_gas_fee,'0') AS REAL) + CAST(COALESCE(exit_gas_fee,'0') AS REAL)), 0), \
            COALESCE(SUM(CAST(COALESCE(entry_slippage,'0') AS REAL) + CAST(COALESCE(exit_slippage,'0') AS REAL)), 0), \
            COALESCE(SUM(CAST(COALESCE(platform_fee,'0') AS REAL)), 0) \
         FROM trades",
        [],
        |row| {
            stats.total_gas = row.get::<_, f64>(0).unwrap_or(0.0);
            stats.total_slippage = row.get::<_, f64>(1).unwrap_or(0.0);
            stats.total_platform = row.get::<_, f64>(2).unwrap_or(0.0);
            stats.total_fees = stats.total_gas + stats.total_slippage + stats.total_platform;
            if stats.total_closed > 0 {
                stats.avg_fee_per_trade = stats.total_fees / stats.total_closed as f64;
            }
            Ok(())
        },
    );

    stats
}

fn read_tp_sl_settings(agent_id: &str) -> TpSlSettings {
    let mut settings = TpSlSettings::default();
    let config_path = format!("configs/{}.env", agent_id);
    if let Ok(content) = fs::read_to_string(&config_path) {
        for line in content.lines() {
            let line = line.trim();
            if let Some(val) = line.strip_prefix("EXIT_TP_PCT=") { settings.exit_tp_pct = val.to_string(); }
            if let Some(val) = line.strip_prefix("EXIT_SL_PCT=") { settings.exit_sl_pct = val.to_string(); }
            if let Some(val) = line.strip_prefix("PRICE_CHECK_SECS=") { settings.price_check_secs = val.parse().unwrap_or(90); }
            if let Some(val) = line.strip_prefix("KILL_THRESHOLD=") { settings.kill_threshold = val.to_string(); }
            if let Some(val) = line.strip_prefix("MIN_CONFIDENCE=") { settings.min_confidence = val.to_string(); }
            if let Some(val) = line.strip_prefix("MIN_EDGE_THRESHOLD=") { settings.min_edge = val.to_string(); }
            if let Some(val) = line.strip_prefix("MAX_OPEN_POSITIONS=") { settings.max_open_positions = val.to_string(); }
        }
    }
    settings
}

/// Reset DB only (keep config) — POST /api/reset-db/{id}
async fn api_reset_db(
    AxumPath(id): AxumPath<String>,
    State(state): State<SharedState>,
) -> Json<StopResponse> {
    // Safety: refuse if agent is running
    let agents_map = state.agents.lock().await;
    if agents_map.contains_key(&id) {
        return Json(StopResponse {
            ok: false,
            message: format!("Agent {} is still running. Stop it first.", id),
            agents_stopped: 0,
        });
    }
    drop(agents_map);

    let db_path = format!("data/{}.db", id);
    if Path::new(&db_path).exists() {
        if let Err(e) = fs::remove_file(&db_path) {
            return Json(StopResponse {
                ok: false,
                message: format!("Failed to delete {}: {}", db_path, e),
                agents_stopped: 0,
            });
        }
    }
    // Also remove WAL/SHM files if they exist
    let _ = fs::remove_file(format!("{}-wal", db_path));
    let _ = fs::remove_file(format!("{}-shm", db_path));

    Json(StopResponse {
        ok: true,
        message: format!("Database reset for agent {}. Config preserved.", id),
        agents_stopped: 0,
    })
}

/// Full reset (delete DB + config) — POST /api/reset-all/{id}
async fn api_reset_all(
    AxumPath(id): AxumPath<String>,
    State(state): State<SharedState>,
) -> Json<StopResponse> {
    // Safety: refuse if agent is running
    let agents_map = state.agents.lock().await;
    if agents_map.contains_key(&id) {
        return Json(StopResponse {
            ok: false,
            message: format!("Agent {} is still running. Stop it first.", id),
            agents_stopped: 0,
        });
    }
    drop(agents_map);

    let mut deleted = Vec::new();

    let db_path = format!("data/{}.db", id);
    if Path::new(&db_path).exists() {
        if fs::remove_file(&db_path).is_ok() {
            deleted.push("database");
        }
    }
    let _ = fs::remove_file(format!("{}-wal", db_path));
    let _ = fs::remove_file(format!("{}-shm", db_path));

    let config_path = format!("configs/{}.env", id);
    if Path::new(&config_path).exists() {
        if fs::remove_file(&config_path).is_ok() {
            deleted.push("config");
        }
    }

    Json(StopResponse {
        ok: true,
        message: format!("Full reset for agent {}: deleted {}", id, deleted.join(" + ")),
        agents_stopped: 0,
    })
}

fn open_legacy_db() -> Option<rusqlite::Connection> {
    if Path::new("agent.db").exists() {
        return rusqlite::Connection::open_with_flags(
            "agent.db",
            OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
        ).ok();
    }
    None
}

// ── Embedded HTML ──

const DASHBOARD_HTML: &str = r##"<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="UTF-8">
<meta name="viewport" content="width=device-width, initial-scale=1.0">
<title>Polymarket Multi-Agent Dashboard</title>
<style>
* { margin: 0; padding: 0; box-sizing: border-box; }
body { background: #0a0e17; color: #c9d1d9; font-family: 'Cascadia Code', 'Fira Code', 'JetBrains Mono', monospace; font-size: 13px; }

.header { background: #161b22; border-bottom: 1px solid #30363d; padding: 16px 24px; display: flex; justify-content: space-between; align-items: center; }
.header h1 { font-size: 18px; color: #58a6ff; font-weight: 600; display: flex; align-items: center; gap: 8px; }
.header .right { display: flex; align-items: center; gap: 16px; }
.header .uptime { color: #8b949e; font-size: 12px; }
.live-clock { color: #c9d1d9; font-size: 13px; font-weight: 600; }
.conn-dot { width: 8px; height: 8px; border-radius: 50%; display: inline-block; }
.conn-dot.ok { background: #3fb950; }
.conn-dot.fail { background: #f85149; }

/* Controls bar */
.controls { background: #161b22; border-bottom: 1px solid #30363d; padding: 12px 24px; display: flex; gap: 16px; align-items: center; flex-wrap: wrap; }
.control-group { display: flex; flex-direction: column; gap: 3px; }
.control-group label { font-size: 10px; color: #8b949e; text-transform: uppercase; letter-spacing: 0.5px; }
.control-group select, .control-group input { background: #0d1117; color: #c9d1d9; border: 1px solid #30363d; border-radius: 4px; padding: 6px 10px; font-size: 12px; font-family: inherit; }
.control-group select:focus, .control-group input:focus { border-color: #58a6ff; outline: none; }
.control-group input[type="number"] { width: 90px; }
.btn { padding: 8px 20px; border: none; border-radius: 6px; font-size: 12px; font-weight: 700; cursor: pointer; font-family: inherit; text-transform: uppercase; letter-spacing: 0.5px; transition: opacity 0.2s; }
.btn:hover { opacity: 0.85; }
.btn:disabled { opacity: 0.4; cursor: not-allowed; }
.btn-start { background: #238636; color: #fff; }
.btn-stop { background: #da3633; color: #fff; }
.btn-sm { padding: 4px 12px; font-size: 11px; }
.controls-right { margin-left: auto; display: flex; gap: 8px; align-items: flex-end; }

/* Metrics */
.metrics { display: flex; gap: 12px; padding: 16px 24px; flex-wrap: wrap; }
.metric-card { background: #161b22; border: 1px solid #30363d; border-radius: 8px; padding: 14px 18px; flex: 1; min-width: 120px; }
.metric-card .label { font-size: 11px; color: #8b949e; text-transform: uppercase; letter-spacing: 0.5px; }
.metric-card .value { font-size: 22px; font-weight: 700; margin-top: 4px; }
.metric-card .sub { font-size: 10px; color: #8b949e; margin-top: 2px; }
.metric-card .value.positive { color: #3fb950; }
.metric-card .value.negative { color: #f85149; }
.metric-card .value.neutral { color: #58a6ff; }

/* Agent Grid */
.section { padding: 0 24px 16px; }
.section-title { font-size: 12px; color: #8b949e; text-transform: uppercase; letter-spacing: 0.5px; margin-bottom: 8px; font-weight: 600; padding-top: 12px; }

.agent-grid { display: grid; grid-template-columns: repeat(auto-fill, minmax(130px, 1fr)); gap: 8px; padding: 0 24px 16px; }
.agent-cell { background: #161b22; border: 1px solid #30363d; border-radius: 8px; padding: 10px; cursor: pointer; transition: border-color 0.2s, transform 0.1s; position: relative; overflow: hidden; }
.agent-cell:hover { border-color: #58a6ff; transform: translateY(-1px); }
.agent-cell .agent-id { font-size: 11px; font-weight: 700; color: #58a6ff; margin-bottom: 4px; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
.agent-cell .agent-strategy { font-size: 9px; color: #8b949e; margin-bottom: 6px; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
.agent-cell .model-badge { font-size: 8px; padding: 2px 5px; border-radius: 3px; margin-left: 4px; font-weight: 600; white-space: nowrap; }
.agent-cell .model-badge.model-gemini { background: rgba(88,166,255,0.15); color: #58a6ff; border: 1px solid rgba(88,166,255,0.3); }
.agent-cell .model-badge.model-sonnet { background: rgba(187,128,255,0.15); color: #bc8cff; border: 1px solid rgba(187,128,255,0.3); }
.extreme-badge { font-size:7px; padding:1px 4px; border-radius:3px; background:rgba(248,81,73,0.2); color:#f85149; border:1px solid rgba(248,81,73,0.4); margin-left:3px; font-weight:700; letter-spacing:0.5px; }
.extreme-badge-modal { font-size:9px; padding:2px 6px; border-radius:4px; background:rgba(248,81,73,0.2); color:#f85149; border:1px solid rgba(248,81,73,0.4); margin-left:6px; font-weight:700; letter-spacing:0.5px; }
.agent-cell .agent-pnl { font-size: 16px; font-weight: 700; }
.agent-cell .agent-pnl.positive { color: #3fb950; }
.agent-cell .agent-pnl.negative { color: #f85149; }
.agent-cell .agent-pnl.neutral { color: #8b949e; }
.agent-cell .agent-meta { font-size: 9px; color: #484f58; margin-top: 4px; }
.agent-cell .status-dot { position: absolute; top: 8px; right: 8px; width: 6px; height: 6px; border-radius: 50%; }
.agent-cell .status-dot.running { background: #3fb950; }
.agent-cell .status-dot.stopped { background: #8b949e; }
.agent-cell .status-dot.dead { background: #f85149; }

/* Improved Pixel Art Icons */
.icon-container { width: 48px; height: 48px; display: flex; align-items: center; justify-content: center; border-radius: 8px; background: rgba(255,255,255,0.05); transition: all 0.3s; }
.agent-cell:hover .icon-container { transform: scale(1.1); background: rgba(255,255,255,0.1); }

.icon-scanning svg { width: 32px; height: 32px; animation: radar-spin 2s infinite linear; color: #58a6ff; }
.icon-analyzing svg { width: 32px; height: 32px; animation: brain-pulse 1.5s infinite alternate; color: #d29922; }
.icon-trading svg { width: 32px; height: 32px; animation: coin-flip 2s infinite; color: #3fb950; }
.icon-stopped svg, .icon-idle svg { width: 28px; height: 28px; opacity: 0.5; color: #8b949e; }
.icon-dead svg { width: 28px; height: 28px; color: #f85149; }

@keyframes radar-spin { from { transform: rotate(0deg); } to { transform: rotate(360deg); } }
@keyframes brain-pulse { from { transform: scale(1); opacity: 0.8; } to { transform: scale(1.1); opacity: 1; } }
@keyframes coin-flip { 0% { transform: rotateY(0); } 50% { transform: rotateY(180deg); } 100% { transform: rotateY(360deg); } }

/* Countdown & Progress */
.next-scan-timer { font-size: 10px; color: #8b949e; margin-top: 4px; display: flex; align-items: center; gap: 4px; }
.timer-bar { height: 2px; background: #21262d; width: 100%; border-radius: 2px; overflow: hidden; }
.timer-fill { height: 100%; background: #58a6ff; width: 0%; transition: width 1s linear; }

/* TP/SL Countdown Timer */
.tpsl-timer { margin-top: 6px; padding: 4px 6px; background: rgba(88,166,255,0.08); border-radius: 4px; border: 1px solid rgba(88,166,255,0.2); }
.tpsl-timer-label { font-size: 9px; color: #8b949e; margin-bottom: 3px; display: flex; align-items: center; gap: 4px; }
.tpsl-countdown { font-weight: 700; color: #58a6ff; font-family: 'Courier New', monospace; }
.tpsl-timer-bar { height: 3px; background: #21262d; width: 100%; border-radius: 2px; overflow: hidden; margin-top: 2px; }
.tpsl-timer-fill { height: 100%; background: linear-gradient(90deg, #3fb950 0%, #58a6ff 50%, #f85149 100%); width: 0%; transition: width 1s linear; }

/* Trade Active View */
.trade-active { margin-top: 8px; background: rgba(13,17,23,0.5); border-radius: 6px; padding: 6px; border: 1px solid #30363d; }
.trade-row { display: flex; justify-content: space-between; font-size: 10px; margin-bottom: 4px; }
.trade-pnl-live { font-weight: 700; }
.trade-progress-container { position: relative; height: 6px; background: #21262d; border-radius: 3px; margin-top: 6px; overflow: visible; }
.trade-marker { position: absolute; top: -2px; width: 2px; height: 10px; background: #fff; z-index: 2; }
.trade-marker.entry { background: #8b949e; }
.trade-marker.current { background: #fff; box-shadow: 0 0 4px #fff; z-index: 3; }
.trade-zone { position: absolute; height: 100%; top: 0; opacity: 0.3; }
.trade-zone.loss { background: #f85149; left: 0; }
.trade-zone.profit { background: #3fb950; right: 0; }

/* Position Cards (Modal) */
.position-card { background: #0d1117; border: 1px solid #30363d; border-radius: 8px; padding: 12px; margin-bottom: 8px; transition: border-color 0.2s; }
.position-card:hover { border-color: #58a6ff; }
.pos-header { display: flex; justify-content: space-between; align-items: flex-start; gap: 12px; }
.pos-progress-bar { height: 6px; background: #21262d; border-radius: 3px; overflow: hidden; }
.pos-progress-fill { height: 100%; border-radius: 3px; transition: width 0.5s ease; }
.pos-progress-fill.tp { background: linear-gradient(90deg, #238636, #3fb950); }
.pos-progress-fill.sl { background: linear-gradient(90deg, #f85149, #da3633); }
.pos-stat { display: inline-block; font-size: 10px; color: #8b949e; background: #161b22; border: 1px solid #30363d; border-radius: 4px; padding: 2px 6px; }

.activity-feed { background: #0d1117; border: 1px solid #21262d; border-radius: 8px; max-height: 220px; overflow-y: auto; padding: 8px 12px; font-size: 11px; line-height: 1.7; }
.event { display: flex; gap: 8px; align-items: baseline; padding: 2px 0; }
.ev-time { color: #484f58; white-space: nowrap; }
.ev-badge { font-size: 9px; font-weight: 700; border-radius: 3px; padding: 1px 5px; text-transform: uppercase; white-space: nowrap; }
.ev-badge.trade { background: #238636; color: #fff; }
.ev-badge.analysis { background: #1f6feb; color: #fff; }
.ev-badge.cycle { background: #30363d; color: #8b949e; }
.ev-agent { font-size: 9px; color: #58a6ff; white-space: nowrap; }
.ev-summary { color: #8b949e; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }

/* Modal */
.modal-overlay { display: none; position: fixed; top: 0; left: 0; right: 0; bottom: 0; background: rgba(0,0,0,0.7); z-index: 100; justify-content: center; align-items: center; }
.modal-overlay.active { display: flex; }
.modal { background: #161b22; border: 1px solid #30363d; border-radius: 12px; width: 90%; max-width: 900px; max-height: 85vh; overflow: hidden; display: flex; flex-direction: column; }
.modal-header { padding: 16px 20px; border-bottom: 1px solid #30363d; display: flex; justify-content: space-between; align-items: center; }
.modal-header h2 { font-size: 16px; color: #58a6ff; }
.modal-close { background: none; border: none; color: #8b949e; font-size: 20px; cursor: pointer; padding: 4px 8px; }
.modal-close:hover { color: #c9d1d9; }
.modal-body { overflow-y: auto; flex: 1; }
.modal-metrics { display: flex; gap: 12px; padding: 16px 20px; flex-wrap: wrap; border-bottom: 1px solid #21262d; }
.modal-metric { text-align: center; min-width: 80px; }
.modal-metric .label { font-size: 10px; color: #8b949e; text-transform: uppercase; }
.modal-metric .value { font-size: 18px; font-weight: 700; margin-top: 2px; }

.modal-tabs { display: flex; border-bottom: 1px solid #30363d; padding: 0 20px; }
.modal-tab { padding: 10px 16px; cursor: pointer; color: #8b949e; border-bottom: 2px solid transparent; font-size: 12px; font-weight: 600; }
.modal-tab:hover { color: #c9d1d9; }
.modal-tab.active { color: #58a6ff; border-bottom-color: #58a6ff; }
.modal-tab-content { display: none; padding: 12px 20px; }
.modal-tab-content.active { display: block; }

.data-table { width: 100%; border-collapse: collapse; font-size: 11px; }
.data-table th { text-align: left; padding: 6px 8px; background: #0d1117; color: #8b949e; font-weight: 600; text-transform: uppercase; font-size: 9px; border-bottom: 1px solid #30363d; position: sticky; top: 0; }
.data-table td { padding: 5px 8px; border-bottom: 1px solid #21262d; color: #c9d1d9; max-width: 200px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.data-table tr:hover td { background: #1c2333; }
.table-wrap { max-height: 300px; overflow-y: auto; background: #0d1117; border: 1px solid #21262d; border-radius: 6px; }

.status-badge { display: inline-block; font-size: 9px; font-weight: 700; border-radius: 4px; padding: 2px 6px; text-transform: uppercase; }
.status-badge.open { background: #d29922; color: #fff; }
.status-badge.won { background: #238636; color: #fff; }
.status-badge.lost { background: #da3633; color: #fff; }

.empty-state { text-align: center; padding: 30px 24px; color: #484f58; }
.empty-state h2 { font-size: 16px; margin-bottom: 6px; }

.reasoning-cell { max-width: 250px; cursor: pointer; }
.reasoning-cell:hover { white-space: normal; word-break: break-word; }

.spinner { display: inline-block; width: 12px; height: 12px; border: 2px solid #30363d; border-top: 2px solid #58a6ff; border-radius: 50%; animation: spin 0.8s linear infinite; vertical-align: middle; }
@keyframes spin { to { transform: rotate(360deg); } }

.refresh-indicator { font-size: 11px; color: #484f58; }

.agent-status-bar { padding: 6px 24px; display: flex; gap: 16px; align-items: center; background: #0d1117; border-bottom: 1px solid #21262d; }
.agent-status-bar .status-item { font-size: 12px; color: #8b949e; }
.agent-status-bar .status-item strong { color: #c9d1d9; }

/* Mode selector */
#ctl-mode { font-weight: 700; }
#ctl-mode option[value="paper"] { color: #3fb950; }
#ctl-mode option[value="live"] { color: #f85149; }
.mode-paper #ctl-mode { border-color: #238636; color: #3fb950; background: #0d1117; }
.mode-live #ctl-mode { border-color: #da3633; color: #f85149; background: #1c0c0c; }
.mode-live .btn-start { background: #da3633; }

/* Alert banner */
.alert-banner { padding: 10px 24px; font-size: 12px; font-weight: 600; display: flex; align-items: center; gap: 8px; }
.alert-warning { background: #2d2000; border-bottom: 1px solid #d29922; color: #d29922; }
.alert-danger { background: #2d0000; border-bottom: 1px solid #f85149; color: #f85149; }
.alert-info { background: #0d1a2d; border-bottom: 1px solid #58a6ff; color: #58a6ff; }

/* Confirm modal */
.confirm-overlay { display: none; position: fixed; top: 0; left: 0; right: 0; bottom: 0; background: rgba(0,0,0,0.8); z-index: 200; justify-content: center; align-items: center; }
.confirm-overlay.active { display: flex; }
.confirm-box { background: #161b22; border: 1px solid #30363d; border-radius: 12px; padding: 24px; max-width: 480px; width: 90%; }
.confirm-box h3 { font-size: 16px; margin-bottom: 16px; }
.confirm-box .confirm-detail { font-size: 12px; color: #8b949e; line-height: 1.8; margin-bottom: 20px; }
.confirm-box .confirm-detail .val { color: #c9d1d9; font-weight: 700; }
.confirm-box .confirm-detail .warn { color: #f85149; font-weight: 700; }
.confirm-box .confirm-detail .safe { color: #3fb950; font-weight: 700; }
.confirm-box .confirm-buttons { display: flex; gap: 10px; justify-content: flex-end; }
.confirm-box .btn-cancel { background: #30363d; color: #c9d1d9; }
    
/* Global Status Pulse */
.running-pulse {
    animation: pulse-green 2s infinite;
    background: #238636;
    color: #fff;
    box-shadow: 0 0 10px rgba(35, 134, 54, 0.5);
}
.status-badge.stopped { background: #30363d; color: #8b949e; }

@keyframes pulse-green {
    0% { transform: scale(1); box-shadow: 0 0 0 0 rgba(35, 134, 54, 0.7); }
    70% { transform: scale(1); box-shadow: 0 0 0 10px rgba(35, 134, 54, 0); }
    100% { transform: scale(1); box-shadow: 0 0 0 0 rgba(35, 134, 54, 0); }
}

/* Calendar Styles */
.calendar-grid { display: grid; grid-template-columns: repeat(7, 1fr); gap: 4px; padding: 16px; }
.cal-day-header { text-align: center; color: #8b949e; font-size: 10px; padding-bottom: 8px; text-transform: uppercase; }
.cal-day { background: #0d1117; border: 1px solid #21262d; border-radius: 4px; min-height: 80px; padding: 6px; position: relative; cursor: pointer; transition: transform 0.1s; }
.cal-day:hover { transform: scale(1.05); z-index: 10; border-color: #58a6ff; }
.cal-date { font-size: 10px; color: #8b949e; position: absolute; top: 4px; right: 4px; }
.cal-pnl { font-size: 12px; font-weight: 700; margin-top: 16px; text-align: center; }
.cal-meta { font-size: 9px; color: #484f58; text-align: center; margin-top: 4px; }

/* Heatmap Colors */
.cal-gain-1 { background: rgba(35, 134, 54, 0.1); border-color: rgba(35, 134, 54, 0.3); }
.cal-gain-2 { background: rgba(35, 134, 54, 0.2); border-color: rgba(35, 134, 54, 0.5); }
.cal-gain-3 { background: rgba(35, 134, 54, 0.4); border-color: rgba(35, 134, 54, 0.7); }
.cal-loss-1 { background: rgba(218, 54, 51, 0.1); border-color: rgba(218, 54, 51, 0.3); }
.cal-loss-2 { background: rgba(218, 54, 51, 0.2); border-color: rgba(218, 54, 51, 0.5); }
.cal-loss-3 { background: rgba(218, 54, 51, 0.4); border-color: rgba(218, 54, 51, 0.7); }

</style>
</head>
<body>

<div class="header">
    <h1><span class="conn-dot ok" id="conn-dot"></span> POLYMARKET MULTI-AGENT DASHBOARD <span id="global-status-badge" class="status-badge stopped" style="margin-left:12px; font-size:10px; padding:3px 8px; vertical-align:middle;">SYSTEM STOPPED</span></h1>
    <div class="right">
        <span class="live-clock" id="live-clock"></span>
        <button class="btn btn-sm" onclick="openCalendar()" style="background:#1f6feb;color:#fff;margin-right:12px;border:none;border-radius:4px;cursor:pointer;">📅 PnL Calendar</button>
        <span class="refresh-indicator" id="refresh-status">Auto-refresh: 5s</span>
        <span class="uptime" id="uptime"></span>
    </div>
</div>


<!-- Controls -->
<div class="controls">
    <div class="control-group">
        <label>Mode</label>
        <select id="ctl-mode" onchange="onModeChange()">
            <option value="paper" selected>Paper Trading</option>
            <option value="live">LIVE Trading</option>
        </select>
    </div>
    <div class="control-group">
        <label>Agents</label>
        <select id="ctl-count">
            <option value="1">1</option>
            <option value="3" selected>3</option>
            <option value="5">5</option>
            <option value="10">10</option>
            <option value="20">20</option>
            <option value="50">50</option>
            <option value="100">100</option>
        </select>
    </div>
    <div class="control-group">
        <label>Category</label>
        <select id="ctl-category">
            <option value="all">All Topics</option>
            <option value="crypto">Crypto</option>
            <option value="politics">Politics</option>
            <option value="sports">Sports</option>
            <option value="weather">Weather</option>
        </select>
    </div>
    <div class="control-group">
        <label>TP / SL</label>
        <select id="ctl-tpsl">
            <option value="fast">Fast (3%/3%)</option>
            <option value="normal" selected>Normal (5%/5%)</option>
            <option value="patient">Patient (10%/7%)</option>
            <option value="wide">Wide (15%/10%)</option>
            <option value="extreme">EXTREME (12%/5%)</option>
        </select>
    </div>
    <div class="control-group">
        <label>Total Balance ($)</label>
        <input type="number" id="ctl-capital" value="100" min="1" step="10">
    </div>
    <div class="controls-right">
        <button class="btn btn-start" id="btn-start" onclick="startAgents()">START</button>
        <button class="btn btn-stop" id="btn-stop" onclick="stopAll()">STOP ALL</button>
    </div>
</div>

<!-- Alert banner (hidden by default) -->
<div id="alert-banner" style="display:none; padding:10px 24px; font-size:12px; font-weight:600; display:none; align-items:center; gap:8px;">
    <span id="alert-icon"></span>
    <span id="alert-text"></span>
    <button onclick="dismissAlert()" style="background:none;border:none;color:inherit;cursor:pointer;font-size:16px;margin-left:auto;">&times;</button>
</div>

<!-- Status bar -->
<div class="agent-status-bar" id="status-bar">
    <div class="status-item">Agents: <strong id="sb-agents">0 / 0</strong></div>
    <div class="status-item">DB: <strong id="sb-db">--</strong></div>
    <div class="status-item">Last Cycle: <strong id="sb-cycle">--</strong></div>
    <div class="status-item">Scanned: <strong id="sb-scanned">--</strong></div>
    <div class="status-item">Analyzed: <strong id="sb-analyzed">--</strong></div>
</div>

<!-- Metrics -->
<div class="metrics" id="metrics">
    <div class="metric-card" style="border-color:#58a6ff">
        <div class="label">Total Balance</div>
        <div class="value neutral" id="m-balance">--</div>
        <div class="sub" id="m-initial"></div>
    </div>
    <div class="metric-card">
        <div class="label">Total P&amp;L</div>
        <div class="value neutral" id="m-pnl">--</div>
        <div class="sub" id="m-roi"></div>
    </div>
    <div class="metric-card">
        <div class="label">Locked</div>
        <div class="value neutral" id="m-locked">--</div>
        <div class="sub" id="m-available"></div>
    </div>
    <div class="metric-card">
        <div class="label">Trades</div>
        <div class="value neutral" id="m-trades">--</div>
        <div class="sub" id="m-wl"></div>
    </div>
    <div class="metric-card">
        <div class="label">Open</div>
        <div class="value neutral" id="m-open">--</div>
    </div>
    <div class="metric-card">
        <div class="label">Win Rate</div>
        <div class="value neutral" id="m-winrate">--</div>
    </div>
    <div class="metric-card">
        <div class="label">API Cost</div>
        <div class="value neutral" id="m-cost">--</div>
    </div>
    <div class="metric-card" style="border-color:#da8b45">
        <div class="label">Total Fees</div>
        <div class="value neutral" id="m-fees">--</div>
        <div class="sub" id="m-fees-sub"></div>
    </div>
    <div class="metric-card" style="border-color:#3fb950">
        <div class="label">Net P&amp;L</div>
        <div class="value neutral" id="m-net">--</div>
        <div class="sub" id="m-net-sub"></div>
    </div>
</div>

<!-- Agent Grid -->
<div class="section">
    <div class="section-title">Agent Fleet</div>
</div>
<div class="agent-grid" id="agent-grid">
    <div class="empty-state" style="grid-column:1/-1"><p>No agents yet. Configure and click START above.</p></div>
</div>

<!-- Activity Feed -->
<div class="section" style="margin-top:8px">
    <div class="section-title">Live Activity Feed</div>
    <div class="activity-feed" id="activity-feed">
        <div style="color:#484f58;text-align:center;padding:20px">Waiting for events...</div>
    </div>
</div>

<!-- Detail Modal -->
<div class="modal-overlay" id="modal-overlay" onclick="closeModal(event)">
    <div class="modal" onclick="event.stopPropagation()">
        <div class="modal-header">
            <h2 id="modal-title">Agent Detail</h2>
            <div style="display:flex;gap:8px;align-items:center">
                <button class="btn btn-stop btn-sm" id="modal-stop-btn" onclick="stopAgent()">STOP</button>
                <button class="modal-close" onclick="closeModal()">&times;</button>
            </div>
        </div>
        <div class="modal-body">
            <div class="modal-metrics" id="modal-metrics"></div>
            <div id="modal-tpsl-settings" style="padding:8px 20px;border-bottom:1px solid #21262d"></div>
            <div id="modal-open-positions" style="padding:8px 20px;border-bottom:1px solid #21262d"></div>
            <div id="modal-performance" style="padding:8px 20px;border-bottom:1px solid #21262d"></div>
            <div class="modal-tabs">
                <div class="modal-tab active" onclick="switchModalTab(event, 'mtab-trades')">Trades</div>
                <div class="modal-tab" onclick="switchModalTab(event, 'mtab-analyses')">Analyses</div>
                <div class="modal-tab" onclick="switchModalTab(event, 'mtab-cycles')">Cycles</div>
            </div>
            <div class="modal-tab-content active" id="mtab-trades">
                <div class="table-wrap" id="modal-trades"></div>
            </div>
            <div class="modal-tab-content" id="mtab-analyses">
                <div class="table-wrap" id="modal-analyses"></div>
            </div>
            <div class="modal-tab-content" id="mtab-cycles">
                <div class="table-wrap" id="modal-cycles"></div>
            </div>
        </div>
    </div>
</div>

<!-- Confirm Start Modal -->
<div class="confirm-overlay" id="confirm-overlay">
    <div class="confirm-box">
        <h3 id="confirm-title" style="color:#58a6ff">Confirm Start Agents</h3>
        <div class="confirm-detail" id="confirm-detail"></div>
        <div class="confirm-buttons">
            <button class="btn btn-cancel" onclick="cancelStart()">CANCEL</button>
            <button class="btn btn-start" id="confirm-go-btn" onclick="confirmStart()">START</button>
        </div>
    </div>
</div>

<!-- Calendar Modal -->
<div class="modal-overlay" id="cal-overlay" onclick="closeCalendar(event)">
    <div class="modal" onclick="event.stopPropagation()" style="max-width:1000px">
        <div class="modal-header">
            <h2>PnL Calendar</h2>
            <button class="modal-close" onclick="closeCalendar()">&times;</button>
        </div>
        <div class="modal-body">
            <div id="calendar-view" class="calendar-grid"></div>
        </div>
    </div>
</div>


<script>
let currentModalAgent = null;

function fmt$(v) {
    const n = parseFloat(v) || 0;
    const sign = n >= 0 ? '+' : '';
    return sign + '$' + n.toFixed(2);
}

function pnlClass(v) {
    const n = parseFloat(v) || 0;
    if (n > 0.001) return 'positive';
    if (n < -0.001) return 'negative';
    return 'neutral';
}

function formatUptime(secs) {
    const h = Math.floor(secs / 3600);
    const m = Math.floor((secs % 3600) / 60);
    return h > 0 ? h + 'h ' + m + 'm' : m + 'm';
}

function truncate(s, max) {
    return s && s.length > max ? s.substring(0, max) + '...' : (s || '');
}

function escHtml(s) {
    return (s || '').replace(/&/g, '&amp;').replace(/"/g, '&quot;').replace(/</g, '&lt;').replace(/>/g, '&gt;');
}

function isExtremePreset(strategy) {
    if (!strategy) return false;
    const s = strategy.toLowerCase();
    return s.startsWith('berserker') || s.startsWith('yolo') || s.startsWith('all-in') || s.startsWith('all in');
}

function updateClock() {
    const now = new Date();
    document.getElementById('live-clock').textContent =
        String(now.getHours()).padStart(2,'0') + ':' +
        String(now.getMinutes()).padStart(2,'0') + ':' +
        String(now.getSeconds()).padStart(2,'0');
}
setInterval(updateClock, 1000);
updateClock();

async function fetchJSON(url) {
    const res = await fetch(url);
    if (!res.ok) throw new Error(res.statusText);
    return res.json();
}

async function refresh() {
    try {
        const [status, agents, activity, calendar] = await Promise.all([
            fetchJSON('/api/status'),
            fetchJSON('/api/agents'),
            fetchJSON('/api/activity'),
            fetchJSON('/api/calendar'),
        ]);

        document.getElementById('conn-dot').className = 'conn-dot ok';
        renderMetrics(status);
        renderAgentGrid(agents);
        renderActivity(activity);
        
        // Store calendar data globally for the modal
        window.calendarStats = calendar.stats;
        if (document.getElementById('cal-overlay').classList.contains('active')) {
            renderCalendarGUI();
        }

        document.getElementById('refresh-status').textContent = 'Updated: ' + new Date().toLocaleTimeString();

        // UI Polish: Disable Start button if running
        const btnStart = document.getElementById('btn-start');
        const GLOBAL_STATUS = document.getElementById('global-status-badge');
        
        if (status.running_agents > 0) {
            btnStart.disabled = true;
            btnStart.textContent = "SYSTEM ACTIVE " + (status.running_agents) + "/" + (status.total_agents);
            btnStart.style.opacity = "0.5";
            btnStart.style.cursor = "not-allowed";
            
            if (GLOBAL_STATUS) {
                GLOBAL_STATUS.className = "status-badge running-pulse";
                GLOBAL_STATUS.textContent = "SYSTEM ACTIVE";
            }
        } else {
            btnStart.disabled = false;
            btnStart.textContent = "START";
            btnStart.style.opacity = "1";
            btnStart.style.cursor = "pointer";

            if (GLOBAL_STATUS) {
                GLOBAL_STATUS.className = "status-badge stopped";
                GLOBAL_STATUS.textContent = "SYSTEM STOPPED";
            }
        }
    } catch (e) {
        document.getElementById('conn-dot').className = 'conn-dot fail';
        document.getElementById('refresh-status').textContent = 'Error: ' + e.message;
    }
}

function renderMetrics(s) {
    const balEl = document.getElementById('m-balance');
    balEl.textContent = '$' + s.balance.toFixed(2);
    balEl.className = 'value ' + (s.pnl >= 0 ? 'positive' : 'negative');
    document.getElementById('m-initial').textContent = 'Initial: $' + s.initial_balance.toFixed(2);

    const pnlEl = document.getElementById('m-pnl');
    pnlEl.textContent = fmt$(s.pnl);
    pnlEl.className = 'value ' + pnlClass(s.pnl);
    const roiSign = s.roi >= 0 ? '+' : '';
    document.getElementById('m-roi').textContent = 'ROI: ' + roiSign + s.roi.toFixed(2) + '%';

    document.getElementById('m-locked').textContent = '$' + s.locked_balance.toFixed(2);
    const avail = s.balance - s.locked_balance;
    document.getElementById('m-available').textContent = 'Available: $' + avail.toFixed(2);

    document.getElementById('m-trades').textContent = s.trades_count;
    document.getElementById('m-wl').textContent = s.wins + 'W / ' + s.losses + 'L';
    document.getElementById('m-open').textContent = s.open_count;

    const wrEl = document.getElementById('m-winrate');
    wrEl.textContent = s.win_rate.toFixed(1) + '%';
    wrEl.className = 'value ' + (s.win_rate >= 55 ? 'positive' : s.win_rate > 0 && s.win_rate < 45 ? 'negative' : 'neutral');

    document.getElementById('m-cost').textContent = '$' + s.api_cost.toFixed(4);

    // Fee cards
    const feesEl = document.getElementById('m-fees');
    feesEl.textContent = '$' + s.total_fees.toFixed(4);
    feesEl.className = 'value ' + (s.total_fees > 0 ? 'negative' : 'neutral');
    document.getElementById('m-fees-sub').textContent = 'Gas:$' + s.total_gas.toFixed(4) + ' Slip:$' + s.total_slippage.toFixed(4) + ' Plat:$' + s.total_platform.toFixed(4);

    const netPnl = s.pnl - s.total_fees;
    const netEl = document.getElementById('m-net');
    netEl.textContent = (netPnl >= 0 ? '+' : '') + '$' + netPnl.toFixed(2);
    netEl.className = 'value ' + pnlClass(netPnl);
    document.getElementById('m-net-sub').textContent = 'P&L $' + s.pnl.toFixed(2) + ' - Fees $' + s.total_fees.toFixed(4);

    document.getElementById('uptime').textContent = 'Uptime: ' + formatUptime(s.uptime_secs);

    // Status bar
    document.getElementById('sb-agents').textContent = s.running_agents + ' running / ' + s.total_agents + ' total' + (s.dead_agents > 0 ? ' (' + s.dead_agents + ' dead)' : '');
    document.getElementById('sb-db').textContent = s.db_found ? 'Connected' : 'No data';
    document.getElementById('sb-db').style.color = s.db_found ? '#3fb950' : '#f85149';
    document.getElementById('sb-cycle').textContent = s.last_cycle ? truncate(s.last_cycle, 19) : 'None';
    document.getElementById('sb-scanned').textContent = s.markets_scanned;
    document.getElementById('sb-analyzed').textContent = s.markets_analyzed;
}

function renderAgentGrid(agents) {
    const grid = document.getElementById('agent-grid');
    if (!agents || agents.length === 0) {
        grid.innerHTML = '<div class="empty-state" style="grid-column:1/-1"><p>No agents yet. Configure and click START above.</p></div>';
        return;
    }

    let html = '';
    agents.forEach(a => {
        const pnlVal = a.pnl || 0;
        const pnlSign = pnlVal >= 0 ? '+' : '';
        const pnlCls = pnlVal > 0.001 ? 'positive' : pnlVal < -0.001 ? 'negative' : 'neutral';
        const lastCycle = a.last_cycle ? a.last_cycle.substring(11, 19) : '--:--:--';
        
        let phase = (a.phase || 'stopped').toLowerCase();
        let phaseDisplay = a.phase_detail || phase.toUpperCase();
        if (phase === 'stopped') phaseDisplay = 'STOPPED';
        if (phase === 'dead') phaseDisplay = 'OFFLINE';

        // Select Icon SVG
        let iconSvg = '';
        if (phase === 'stopped') {
             // Ensure stopped icon is visible
             iconSvg = '<svg width="24" height="24" viewBox="0 0 24 24" fill="currentColor"><circle cx="12" cy="12" r="10" fill-opacity="0.2"/><rect x="8" y="8" width="8" height="8" fill="currentColor"/></svg>';
        } else if (phase === 'scanning') {
            iconSvg = '<svg width="24" height="24" viewBox="0 0 24 24" fill="currentColor"><path d="M12 2C6.48 2 2 6.48 2 12s4.48 10 10 10 10-4.48 10-10S17.52 2 12 2zm0 18c-4.41 0-8-3.59-8-8s3.59-8 8-8 8 3.59 8 8-3.59 8-8 8z"/><circle cx="12" cy="12" r="3"/></svg>';
        } else if (phase === 'analyzing') {
            // Brain / Chart
            iconSvg = '<svg width="24" height="24" viewBox="0 0 24 24" fill="currentColor"><path d="M9 21c0 .55.45 1 1 1h4c.55 0 1-.45 1-1v-1H9v1zm3-19C8.14 2 5 5.14 5 9c0 2.38 1.19 4.47 3 5.74V17c0 .55.45 1 1 1h6c.55 0 1-.45 1-1v-2.26c1.81-1.27 3-3.36 3-5.74 0-3.86-3.14-7-7-7zm2.85 11.1l-.85.6V16h-4v-2.3l-.85-.6A4.997 4.997 0 0 1 7 9c0-2.76 2.24-5 5-5s5 2.24 5 5c0 1.63-.8 3.16-2.15 4.1z"/></svg>';
        } else if (phase === 'trading') {
            // Lightning / Coin
            iconSvg = '<svg width="24" height="24" viewBox="0 0 24 24" fill="currentColor"><path d="M7 2v11h3v9l7-12h-4l4-8z"/></svg>';
        } else if (phase === 'idle') {
            // Coffee / Zzz
            iconSvg = '<svg width="24" height="24" viewBox="0 0 24 24" fill="currentColor"><path d="M20 2H4c-1.1 0-2 .9-2 2v18l4-4h14c1.1 0 2-.9 2-2V4c0-1.1-.9-2-2-2z"/></svg>';
        } else {
            // Stopped / Dead
            iconSvg = '<svg width="24" height="24" viewBox="0 0 24 24" fill="currentColor"><path d="M12 2C6.48 2 2 6.48 2 12s4.48 10 10 10 10-4.48 10-10S17.52 2 12 2zm0 18c-4.41 0-8-3.59-8-8s3.59-8 8-8 8 3.59 8 8-3.59 8-8 8z"/></svg>';
        }

        // Countdown
        let countdownHtml = '';
        if (phase !== 'stopped' && phase !== 'dead' && a.last_cycle && a.interval > 0) {
             const lastTime = new Date(a.last_cycle).getTime();
             const now = new Date().getTime();
             const nextScan = lastTime + (a.interval * 1000);
             let diff = nextScan - now;
             if (diff < 0) diff = 0;
             
             const pct = Math.max(0, Math.min(100, (diff / (a.interval * 1000)) * 100));
             const seconds = Math.ceil(diff / 1000);
             
             const label = phase === 'idle' ? 'Scanning in ' + seconds + 's' : phase.toUpperCase();
             
             countdownHtml = '<div class="next-scan-timer">' +
                 '<div style="flex:1">' + label + '</div>' +
                 '</div>' + 
                 '<div class="timer-bar"><div class="timer-fill" style="width:' + pct + '%"></div></div>';
        }

        // Active Trade Visualization
        let tradeHtml = '';
        if (a.open_position) {
            const op = a.open_position;
            const pnlColor = op.pnl_pct >= 0 ? '#3fb950' : '#f85149';
            const pnlTxt = (op.pnl_pct >= 0 ? '+' : '') + op.pnl_pct.toFixed(2) + '%';
            
            // Calc progress bar for Price vs SL/TP
            // Need a normalized range. Min = SL, Max = TP.
            // But wait, if Long: SL < Entry < TP.
            // If Short: TP < Entry < SL.
            
            let min, max, currentPct;
            
            // Normalize for visual slider (0% = SL, 100% = TP)
            if (op.direction === 'Long') {
                min = op.sl_price;
                max = op.tp_price;
                currentPct = (op.current_price - min) / (max - min) * 100.0;
            } else {
                 min = op.tp_price;
                 max = op.sl_price;
                 // For short, lower price is better (towards TP). 
                 // So if price is at SL (high), it's 0% (bad). If at TP (low), it's 100% (good).
                 // Wait, usually Green is right. Let's make 0% = Bad (SL), 100% = Good (TP).
                 // Long: Price goes UP to TP.
                 // Short: Price goes DOWN to TP.
                 
                 // Let's stick to Price Scale? 
                 // Left = Low Price, Right = High Price.
                 // Long: SL(Left) ... Entry ... TP(Right). Target Right.
                 // Short: TP(Left) ... Entry ... SL(Right). Target Left.
                 
                 // Let's use simple Logic: 0 = Loss Limit, 100 = Profit Target.
                 // Current distance to SL vs TP.
                 // Let's visualize strictly Price.
                 min = Math.min(op.sl_price, op.tp_price, op.current_price, op.entry_price) * 0.99;
                 max = Math.max(op.sl_price, op.tp_price, op.current_price, op.entry_price) * 1.01;
                 
                 currentPct = (op.current_price - min) / (max - min) * 100.0;
            }
            
            // Clamp
            currentPct = Math.max(0, Math.min(100, currentPct));
            
            // Markers
            const entryPct = (op.entry_price - min) / (max - min) * 100.0;
            const tpPct = (op.tp_price - min) / (max - min) * 100.0;
            const slPct = (op.sl_price - min) / (max - min) * 100.0;

            // TP/SL Countdown Timer
            let tpslTimerHtml = '';
            if (op.last_price_check && a.price_check_interval > 0) {
                const lastCheckTime = new Date(op.last_price_check).getTime();
                const now = new Date().getTime();
                const nextCheck = lastCheckTime + (a.price_check_interval * 1000);
                let timeLeft = nextCheck - now;
                if (timeLeft < 0) timeLeft = 0;

                const totalSeconds = Math.ceil(timeLeft / 1000);
                const minutes = Math.floor(totalSeconds / 60);
                const seconds = totalSeconds % 60;
                const timeDisplay = minutes > 0 ? minutes + 'm ' + seconds + 's' : seconds + 's';

                const progressPct = Math.max(0, Math.min(100, (timeLeft / (a.price_check_interval * 1000)) * 100));

                tpslTimerHtml = '<div class="tpsl-timer">' +
                    '<div class="tpsl-timer-label">⏱ Next TP/SL Check: <span class="tpsl-countdown" data-agent="' + a.id + '">' + timeDisplay + '</span></div>' +
                    '<div class="tpsl-timer-bar"><div class="tpsl-timer-fill" style="width:' + progressPct + '%"></div></div>' +
                '</div>';
            }

            // Unrealized P&L ($) and time open
            const uPnl = op.unrealized_pnl || 0;
            const uPnlSign = uPnl >= 0 ? '+' : '';
            const uPnlColor = uPnl >= 0 ? '#3fb950' : '#f85149';
            const hoursOpen = op.timestamp ? estimateHoursOpen(op.timestamp) : 0;
            const timeStr = hoursOpen >= 1 ? hoursOpen.toFixed(1) + 'h' : Math.round(hoursOpen * 60) + 'm';

            // TP/SL distance from current price
            let tpDistLabel = '';
            let slDistLabel = '';
            if (op.tp_price > 0 && op.entry_price > 0 && op.current_price > 0) {
                const tpDist = op.direction === 'Long'
                    ? ((op.tp_price - op.current_price) / op.entry_price * 100)
                    : ((op.current_price - op.tp_price) / op.entry_price * 100);
                tpDistLabel = '<span class="pos-stat" style="border-color:#238636"><span style="color:#3fb950">TP ' + tpDist.toFixed(1) + '%</span></span>';
            }
            if (op.sl_price > 0 && op.entry_price > 0 && op.current_price > 0) {
                const slDist = op.direction === 'Long'
                    ? ((op.current_price - op.sl_price) / op.entry_price * 100)
                    : ((op.sl_price - op.current_price) / op.entry_price * 100);
                slDistLabel = '<span class="pos-stat" style="border-color:#da3633"><span style="color:#f85149">SL ' + slDist.toFixed(1) + '%</span></span>';
            }

            tradeHtml = '<div class="trade-active">' +
                '<div class="trade-row"><span>' + truncate(op.question, 25) + '</span><span class="trade-pnl-live" style="color:' + pnlColor + '">' + pnlTxt + '</span></div>' +
                '<div class="trade-row"><span style="color:#8b949e">' + op.direction + ' @ ' + op.entry_price.toFixed(2) + '</span><span>Cur: ' + op.current_price.toFixed(2) + '</span></div>' +
                '<div class="trade-row"><span style="color:' + uPnlColor + ';font-weight:700">' + uPnlSign + '$' + Math.abs(uPnl).toFixed(2) + '</span>' +
                    '<span style="color:#8b949e;font-size:9px">⏱ ' + timeStr + '</span>' +
                    '<span>' + tpDistLabel + slDistLabel + '</span></div>' +
                '<div class="trade-progress-container">' +
                    '<div class="trade-marker entry" style="left:' + entryPct + '%" title="Entry"></div>' +
                    '<div class="trade-marker current" style="left:' + currentPct + '%" title="Current"></div>' +
                    '<div class="trade-marker" style="left:' + tpPct + '%; background:#3fb950; height:8px; top:-1px; width:3px" title="TP"></div>' +
                    '<div class="trade-marker" style="left:' + slPct + '%; background:#f85149; height:8px; top:-1px; width:3px" title="SL"></div>' +
                '</div>' +
                tpslTimerHtml +
            '</div>';
        }

        html += '<div class="agent-cell" onclick="openAgent(\'' + escHtml(a.id) + '\')">' +
            '<div class="agent-header">' +
                '<div class="icon-container icon-' + phase + '">' + iconSvg + '</div>' +
                '<div style="text-align:right">' +
                    '<div class="agent-id">' + escHtml(a.id) + '</div>' +
                    '<div class="agent-pnl ' + pnlCls + '">' + pnlSign + '$' + pnlVal.toFixed(2) + '</div>' +
                '</div>' +
            '</div>' +
            '<div class="agent-strategy">' + escHtml(a.strategy || 'unknown') +
                ' <span class="model-badge model-' + a.judge_model + '">' +
                (a.judge_model === 'sonnet' ? '🧠 Sonnet' : '💎 Gemini') +
                '</span>' +
                (isExtremePreset(a.strategy) ? '<span class="extreme-badge">EXTREME</span>' : '') +
                '</div>' +
            '<div class="agent-meta">' + a.trades_count + ' trades | WR ' + a.win_rate.toFixed(0) + '%</div>' +
            
            (tradeHtml ? tradeHtml : 
             (''+ countdownHtml + 
              '<div class="phase-label phase-' + phase + '" style="margin-top:6px">' + truncate(phaseDisplay, 25) + '</div>')
            ) +
            
            '</div>';
    });
    grid.innerHTML = html;
}

function renderActivity(events) {
    const feed = document.getElementById('activity-feed');
    if (!events || events.length === 0) {
        feed.innerHTML = '<div style="color:#484f58;text-align:center;padding:20px">No activity yet. Start agents to populate this feed.</div>';
        return;
    }

    let html = '';
    events.forEach(ev => {
        const badgeCls = ev.event_type === 'trade' ? 'trade' : ev.event_type === 'analysis' ? 'analysis' : 'cycle';
        let timeStr = ev.timestamp || '';
        if (timeStr.length >= 19) timeStr = timeStr.substring(11, 19);
        html += '<div class="event">' +
            '<span class="ev-time">[' + timeStr + ']</span>' +
            '<span class="ev-badge ' + badgeCls + '">' + ev.event_type.toUpperCase() + '</span>' +
            '<span class="ev-agent">' + escHtml(ev.agent_id) + '</span>' +
            '<span class="ev-summary">' + escHtml(ev.summary) + '</span>' +
            '</div>';
    });
    feed.innerHTML = html;
}

// ── Mode ──

function onModeChange() {
    const mode = document.getElementById('ctl-mode').value;
    const body = document.body;
    body.classList.remove('mode-paper', 'mode-live');
    body.classList.add('mode-' + mode);

    if (mode === 'live') {
        showAlert('danger', 'LIVE TRADING MODE: Real money will be used. Pastikan WALLET_PRIVATE_KEY sudah di-set di .env dan saldo wallet cukup.');
    } else {
        dismissAlert();
    }
}

function showAlert(type, msg) {
    const banner = document.getElementById('alert-banner');
    banner.style.display = 'flex';
    banner.className = 'alert-banner alert-' + type;
    const icons = { warning: '!', danger: '!!', info: 'i' };
    document.getElementById('alert-icon').textContent = '[' + (icons[type] || '!') + ']';
    document.getElementById('alert-text').textContent = msg;
}

function dismissAlert() {
    document.getElementById('alert-banner').style.display = 'none';
}

// ── Actions ──

let pendingStartBody = null;

function startAgents() {
    const mode = document.getElementById('ctl-mode').value;
    const count = parseInt(document.getElementById('ctl-count').value);
    const category = document.getElementById('ctl-category').value;
    const tpsl = document.getElementById('ctl-tpsl').value;
    const capital = parseFloat(document.getElementById('ctl-capital').value) || 100;
    const capitalPerAgent = (capital / count).toFixed(2);
    const isLive = mode === 'live';

    pendingStartBody = { count, category, tp_sl: tpsl, capital, mode };

    // Build confirmation detail
    const modeLabel = isLive ? '<span class="warn">LIVE TRADING (REAL MONEY)</span>' : '<span class="safe">PAPER TRADING (Simulated)</span>';
    const tpslLabels = { fast: '3%/3%', normal: '5%/5%', patient: '10%/7%', wide: '15%/10%' };
    const catLabels = { all: 'All Topics', crypto: 'Crypto', politics: 'Politics', sports: 'Sports', weather: 'Weather' };

    let detail = '';
    detail += 'Mode: ' + modeLabel + '<br>';
    detail += 'Agents: <span class="val">' + count + '</span><br>';
    detail += 'Category: <span class="val">' + (catLabels[category] || category) + '</span><br>';
    detail += 'TP/SL: <span class="val">' + (tpslLabels[tpsl] || tpsl) + '</span><br>';
    detail += 'Total Capital: <span class="val">$' + capital.toFixed(2) + '</span><br>';
    detail += 'Per Agent: <span class="val">$' + capitalPerAgent + '</span><br>';

    if (isLive) {
        detail += '<br><span class="warn">PERINGATAN KRITIS:</span><br>';
        detail += '- Pastikan saldo wallet USDC Polygon cukup: <span class="warn">$' + capital.toFixed(2) + '+</span><br>';
        detail += '- WALLET_PRIVATE_KEY harus sudah di-set di .env<br>';
        detail += '- Semua kerugian adalah UANG NYATA<br>';
        detail += '- Tidak bisa dibatalkan setelah trade terbuka<br>';
    } else {
        detail += '<br><span class="safe">Mode aman:</span> Tidak menggunakan uang nyata.<br>';
        detail += 'Saldo awal per agent: <span class="val">$' + capitalPerAgent + '</span> (virtual)<br>';
    }

    // Balance check warning
    if (capital < count * 5) {
        detail += '<br><span class="warn">Peringatan:</span> Modal per agent sangat kecil ($' + capitalPerAgent + '). Agent bisa mati (kill threshold) dengan cepat.<br>';
    }
    if (capital > 1000 && isLive) {
        detail += '<br><span class="warn">Modal besar terdeteksi: $' + capital.toFixed(2) + '</span>. Pastikan ini disengaja.<br>';
    }

    document.getElementById('confirm-detail').innerHTML = detail;
    document.getElementById('confirm-title').textContent = isLive ? 'CONFIRM LIVE TRADING' : 'Confirm Start Agents';
    document.getElementById('confirm-title').style.color = isLive ? '#f85149' : '#58a6ff';
    document.getElementById('confirm-go-btn').textContent = isLive ? 'START LIVE' : 'START';
    document.getElementById('confirm-go-btn').className = isLive ? 'btn btn-stop' : 'btn btn-start';
    document.getElementById('confirm-overlay').classList.add('active');
}

function cancelStart() {
    document.getElementById('confirm-overlay').classList.remove('active');
    pendingStartBody = null;
}

async function confirmStart() {
    document.getElementById('confirm-overlay').classList.remove('active');
    if (!pendingStartBody) return;

    const btn = document.getElementById('btn-start');
    btn.disabled = true;
    btn.textContent = 'STARTING...';

    try {
        const res = await fetch('/api/start', {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify(pendingStartBody),
        });
        const data = await res.json();
        if (data.ok) {
            btn.textContent = 'STARTED ' + data.agents_started;
            const isLive = pendingStartBody.mode === 'live';
            if (isLive) {
                showAlert('warning', 'LIVE agents running! ' + data.agents_started + ' agents menggunakan uang nyata. Monitor saldo secara berkala.');
            } else {
                showAlert('info', data.agents_started + ' paper trading agents started. Capital: $' + pendingStartBody.capital.toFixed(2));
            }
        } else {
            console.error('Start failed:', data);
            btn.textContent = 'FAILED';
            showAlert('danger', data.message || 'Failed to start agents. Check console for details.');
        }
        setTimeout(() => { btn.textContent = 'START'; btn.disabled = false; }, 2000);
        refresh();
    } catch (e) {
        console.error('Start network error:', e);
        btn.textContent = 'ERROR';
        showAlert('danger', 'Network error: ' + e.message + '. Ensure dashboard is running.');
        setTimeout(() => { btn.textContent = 'START'; btn.disabled = false; }, 2000);
    }
    pendingStartBody = null;
}

async function stopAll() {
    const btn = document.getElementById('btn-stop');
    btn.disabled = true;
    btn.textContent = 'STOPPING...';

    try {
        const res = await fetch('/api/stop', { method: 'POST' });
        const data = await res.json();
        btn.textContent = 'STOPPED ' + data.agents_stopped;
        dismissAlert();
        setTimeout(() => { btn.textContent = 'STOP ALL'; btn.disabled = false; }, 2000);
        refresh();
    } catch (e) {
        btn.textContent = 'ERROR';
        setTimeout(() => { btn.textContent = 'STOP ALL'; btn.disabled = false; }, 2000);
    }
}

async function stopAgent() {
    if (!currentModalAgent) return;
    try {
        await fetch('/api/stop/' + currentModalAgent, { method: 'POST' });
        closeModal();
        refresh();
    } catch (e) {
        console.error('Stop agent failed:', e);
    }
}

// ── Modal ──

async function openAgent(id) {
    currentModalAgent = id;
    document.getElementById('modal-overlay').classList.add('active');
    document.getElementById('modal-title').textContent = id;

    // Reset tabs
    document.querySelectorAll('.modal-tab').forEach((t,i) => t.classList.toggle('active', i===0));
    document.querySelectorAll('.modal-tab-content').forEach((t,i) => t.classList.toggle('active', i===0));

    try {
        const detail = await fetchJSON('/api/agent/' + id);
        renderModalMetrics(detail.info);
        renderTpSlSettings(detail.tp_sl_settings);
        renderOpenPositions(detail.open_positions);
        renderPerformance(detail.performance);
        renderModalTrades(detail.trades);
        renderModalAnalyses(detail.analyses);
        renderModalCycles(detail.cycles);
    } catch (e) {
        document.getElementById('modal-metrics').innerHTML = '<div class="empty-state" style="color:#f85149">Failed to load agent detail: ' + escHtml(e.message) + '</div>';
        console.error('Load agent failed:', e);
    }
}

function closeModal(e) {
    if (e && e.target !== document.getElementById('modal-overlay')) return;
    if (!e) document.getElementById('modal-overlay').classList.remove('active');
    else document.getElementById('modal-overlay').classList.remove('active');
    currentModalAgent = null;
    currentModalAgent = null;
}

// ── Calendar Logic ──

function openCalendar() {
    document.getElementById('cal-overlay').classList.add('active');
    renderCalendarGUI();
}

function closeCalendar(e) {
    if (e && e.target !== document.getElementById('cal-overlay')) return;
    document.getElementById('cal-overlay').classList.remove('active');
}

function renderCalendarGUI() {
    const grid = document.getElementById('calendar-view');
    if (!grid) return;
    
    // Get current month stats from window.calendarStats
    const stats = window.calendarStats || [];
    
    // Header
    const days = ['Sun', 'Mon', 'Tue', 'Wed', 'Thu', 'Fri', 'Sat'];
    let html = '';
    days.forEach(d => html += '<div class="cal-day-header">' + d + '</div>');
    
    // Simple 30-day view (mockup for now, ideally strictly aligns with real dates)
    // For now we map available stats to a grid
    // If we want a real calendar, we need JS Date logic. keeping it simple first.
    
    // Just show all available days in reverse order or proper order?
    // Let's just show the last 28 days
    const displayStats = stats.slice(-28); // Show last 4 weeks?
    
    // Fill gaps if empty
    if (displayStats.length === 0) {
        html += '<div class="empty-state" style="grid-column:1/-1">No data available for calendar</div>';
        grid.innerHTML = html;
        return;
    }

    displayStats.forEach(day => {
        let pnlClass = '';
        if (day.pnl > 50) pnlClass = 'cal-gain-3';
        else if (day.pnl > 10) pnlClass = 'cal-gain-2';
        else if (day.pnl > 0) pnlClass = 'cal-gain-1';
        else if (day.pnl < -50) pnlClass = 'cal-loss-3';
        else if (day.pnl < -10) pnlClass = 'cal-loss-2';
        else if (day.pnl < 0) pnlClass = 'cal-loss-1';

        const dateStr = day.date.substring(5); // MM-DD
        const pnlStr = (day.pnl >= 0 ? '+' : '') + '$' + day.pnl.toFixed(2);
        
        html += '<div class="cal-day ' + pnlClass + '">' +
            '<div class="cal-date">' + dateStr + '</div>' +
            '<div class="cal-pnl" style="color:' + (day.pnl>=0?'#3fb950':'#f85149') + '">' + pnlStr + '</div>' +
            '<div class="cal-meta">' + day.trades_count + ' trades</div>' +
            '<div class="cal-meta">' + day.wins + 'W ' + day.losses + 'L</div>' +
            '</div>';
    });
    
    grid.innerHTML = html;
}

function renderModalMetrics(info) {
    const pnlCls = info.pnl > 0 ? 'positive' : info.pnl < 0 ? 'negative' : 'neutral';
    const roi = info.initial_balance > 0 ? ((info.pnl / info.initial_balance) * 100).toFixed(2) : '0.00';
    document.getElementById('modal-metrics').innerHTML =
        '<div class="modal-metric"><div class="label">Balance</div><div class="value" style="color:#58a6ff">$' + info.balance.toFixed(2) + '</div></div>' +
        '<div class="modal-metric"><div class="label">P&L</div><div class="value ' + pnlCls + '">' + (info.pnl >= 0 ? '+' : '') + '$' + info.pnl.toFixed(2) + '</div></div>' +
        '<div class="modal-metric"><div class="label">ROI</div><div class="value ' + pnlCls + '">' + roi + '%</div></div>' +
        '<div class="modal-metric"><div class="label">Trades</div><div class="value" style="color:#c9d1d9">' + info.trades_count + '</div></div>' +
        '<div class="modal-metric"><div class="label">Win Rate</div><div class="value" style="color:#c9d1d9">' + info.win_rate.toFixed(1) + '%</div></div>' +
        '<div class="modal-metric"><div class="label">Strategy</div><div class="value" style="color:#8b949e;font-size:12px">' + escHtml(info.strategy) + (isExtremePreset(info.strategy) ? ' <span class="extreme-badge-modal">EXTREME MODE</span>' : '') + '</div></div>' +
        '<div class="modal-metric"><div class="label">Status</div><div class="value" style="color:' + (info.status==='running'?'#3fb950':info.status==='dead'?'#f85149':'#8b949e') + ';font-size:12px">' + info.status.toUpperCase() + '</div></div>';
}

function estimateHoursOpen(timestamp) {
    if (!timestamp || timestamp.length < 19) return 0;
    const d = new Date(timestamp.replace(' ', 'T'));
    if (isNaN(d.getTime())) return 0;
    return Math.max(0, (Date.now() - d.getTime()) / 3600000);
}

function estimateCompletion(entry, current, tp, hoursOpen, direction) {
    if (!entry || !current || !tp || hoursOpen <= 0) return '';
    const velocity = direction === 'Long'
        ? (current - entry) / hoursOpen
        : (entry - current) / hoursOpen;
    if (velocity <= 0) {
        return '<span class="pos-stat" style="border-color:#d29922;color:#d29922">Stalled</span>';
    }
    const remaining = direction === 'Long'
        ? (tp - current)
        : (current - tp);
    if (remaining <= 0) {
        return '<span class="pos-stat" style="border-color:#3fb950;color:#3fb950">TP overshot!</span>';
    }
    const estHours = remaining / velocity;
    const label = estHours >= 1 ? '~' + estHours.toFixed(1) + 'h to TP' : '~' + Math.round(estHours * 60) + 'm to TP';
    return '<span class="pos-stat" style="border-color:#58a6ff;color:#58a6ff">' + label + '</span>';
}

function exitReasonBadge(reason) {
    if (!reason) return '';
    const colors = {
        'TakeProfit': 'background:#238636;color:#fff',
        'StopLoss': 'background:#da3633;color:#fff',
        'EdgeCaptured': 'background:#1f6feb;color:#fff',
        'SafetyValve': 'background:#d29922;color:#fff',
        'TimeExpiry': 'background:#30363d;color:#8b949e',
        'MarketResolved': 'background:#6e40c9;color:#fff',
    };
    const labels = {
        'TakeProfit': 'TP',
        'StopLoss': 'SL',
        'EdgeCaptured': 'EDGE',
        'SafetyValve': 'SAFE',
        'TimeExpiry': 'TIME',
        'MarketResolved': 'RESOLVED',
    };
    const style = colors[reason] || 'background:#30363d;color:#8b949e';
    const label = labels[reason] || reason;
    return '<span style="font-size:9px;font-weight:700;border-radius:3px;padding:1px 5px;' + style + '">' + label + '</span>';
}

function renderTpSlSettings(settings) {
    const wrap = document.getElementById('modal-tpsl-settings');
    if (!settings) { wrap.innerHTML = ''; return; }
    const items = [
        { label: 'Take Profit', value: settings.exit_tp_pct || '0', suffix: '%' },
        { label: 'Stop Loss', value: settings.exit_sl_pct || '0', suffix: '%' },
        { label: 'Price Check', value: settings.price_check_secs || 0, suffix: 's' },
        { label: 'Kill Threshold', value: '$' + (settings.kill_threshold || '0'), suffix: '' },
        { label: 'Min Confidence', value: settings.min_confidence || '0', suffix: '' },
        { label: 'Min Edge', value: settings.min_edge || '0', suffix: '' },
        { label: 'Max Positions', value: settings.max_open_positions || '0', suffix: '' },
    ];
    let html = '<div style="display:flex;flex-wrap:wrap;gap:8px;padding:4px 0">';
    items.forEach(item => {
        html += '<div style="background:#161b22;border:1px solid #30363d;border-radius:6px;padding:4px 10px;font-size:11px">' +
            '<span style="color:#8b949e">' + item.label + ':</span> ' +
            '<span style="color:#c9d1d9;font-weight:600">' + item.value + item.suffix + '</span></div>';
    });
    html += '</div>';
    wrap.innerHTML = html;
}

function renderOpenPositions(positions) {
    const wrap = document.getElementById('modal-open-positions');
    if (!positions || positions.length === 0) {
        wrap.innerHTML = '<div style="color:#8b949e;font-size:11px;padding:4px 0">No open positions</div>';
        return;
    }
    let html = '<div style="font-size:11px;color:#8b949e;margin-bottom:8px">Open Positions (' + positions.length + ')</div>';
    positions.forEach(p => {
        const hasCurrent = p.current_price > 0;
        const pnlPct = p.pnl_pct || 0;
        const uPnl = p.unrealized_pnl || 0;
        const pnlColor = pnlPct >= 0 ? '#3fb950' : '#f85149';
        const pnlSign = pnlPct >= 0 ? '+' : '';
        const uPnlSign = uPnl >= 0 ? '+' : '';
        const hoursOpen = p.hours_open || 0;
        const timeStr = hoursOpen >= 1 ? hoursOpen.toFixed(1) + 'h' : Math.round(hoursOpen * 60) + 'm';
        const dirColor = p.direction === 'Long' ? '#3fb950' : '#f85149';

        // TP progress bar (0-100% towards TP)
        let tpProgressPct = 0;
        let tpDistPct = p.distance_to_tp_pct || 0;
        if (p.take_profit && hasCurrent && p.entry_price > 0) {
            const totalRange = Math.abs(p.take_profit - p.entry_price);
            if (totalRange > 0) {
                const traveled = p.direction === 'Long'
                    ? (p.current_price - p.entry_price)
                    : (p.entry_price - p.current_price);
                tpProgressPct = Math.max(0, Math.min(100, (traveled / totalRange) * 100));
            }
        }

        // SL safety bar (100% = safe at entry, 0% = at SL)
        let slSafetyPct = 100;
        let slDistPct = p.distance_to_sl_pct || 0;
        if (p.stop_loss && hasCurrent && p.entry_price > 0) {
            const totalRange = Math.abs(p.entry_price - p.stop_loss);
            if (totalRange > 0) {
                const distToSl = p.direction === 'Long'
                    ? (p.current_price - p.stop_loss)
                    : (p.stop_loss - p.current_price);
                slSafetyPct = Math.max(0, Math.min(100, (distToSl / totalRange) * 100));
            }
        }

        // Estimated completion
        const estHtml = estimateCompletion(p.entry_price, p.current_price, p.take_profit, hoursOpen, p.direction);

        const tp = p.take_profit ? '$' + p.take_profit.toFixed(2) : '--';
        const sl = p.stop_loss ? '$' + p.stop_loss.toFixed(2) : '--';
        const lastCheck = p.last_price_check ? truncate(p.last_price_check, 19) : '--';

        html += '<div class="position-card">' +
            '<div class="pos-header">' +
                '<div style="flex:1;min-width:0">' +
                    '<div style="font-size:12px;font-weight:600;color:#c9d1d9;white-space:nowrap;overflow:hidden;text-overflow:ellipsis" title="' + escHtml(p.question) + '">' + truncate(p.question, 40) + '</div>' +
                    '<div style="font-size:10px;margin-top:2px"><span style="color:' + dirColor + ';font-weight:700">' + p.direction + '</span> <span style="color:#8b949e">' + (p.trade_mode || '?') + '</span></div>' +
                '</div>' +
                '<div style="text-align:right">' +
                    '<div style="font-size:14px;font-weight:700;color:' + pnlColor + '">' + pnlSign + pnlPct.toFixed(2) + '%</div>' +
                    '<div style="font-size:11px;color:' + pnlColor + '">' + uPnlSign + '$' + Math.abs(uPnl).toFixed(2) + '</div>' +
                '</div>' +
            '</div>' +

            '<div style="display:flex;gap:12px;margin:8px 0;font-size:11px">' +
                '<div><span style="color:#8b949e">Entry:</span> <span style="color:#c9d1d9;font-weight:600">$' + p.entry_price.toFixed(2) + '</span></div>' +
                (hasCurrent ? '<div><span style="color:#8b949e">Current:</span> <span style="color:#fff;font-weight:600">$' + p.current_price.toFixed(2) + '</span></div>' : '') +
                '<div><span style="color:#8b949e">Size:</span> <span style="color:#c9d1d9;font-weight:600">$' + p.bet_size.toFixed(2) + '</span></div>' +
            '</div>' +

            // TP progress
            '<div style="margin:6px 0">' +
                '<div style="display:flex;justify-content:space-between;font-size:10px;margin-bottom:3px">' +
                    '<span style="color:#3fb950">TP: ' + tp + '</span>' +
                    '<span style="color:#8b949e">' + tpDistPct.toFixed(1) + '% away</span>' +
                '</div>' +
                '<div class="pos-progress-bar"><div class="pos-progress-fill tp" style="width:' + tpProgressPct + '%"></div></div>' +
            '</div>' +

            // SL safety
            '<div style="margin:6px 0">' +
                '<div style="display:flex;justify-content:space-between;font-size:10px;margin-bottom:3px">' +
                    '<span style="color:#f85149">SL: ' + sl + '</span>' +
                    '<span style="color:#8b949e">' + slDistPct.toFixed(1) + '% safe</span>' +
                '</div>' +
                '<div class="pos-progress-bar"><div class="pos-progress-fill sl" style="width:' + slSafetyPct + '%"></div></div>' +
            '</div>' +

            // Footer stats
            '<div style="display:flex;flex-wrap:wrap;gap:6px;margin-top:8px;font-size:10px">' +
                '<span class="pos-stat">⏱ ' + timeStr + ' open</span>' +
                '<span class="pos-stat">Last check: ' + lastCheck + '</span>' +
                estHtml +
            '</div>' +
        '</div>';
    });
    wrap.innerHTML = html;
}

function renderPerformance(perf) {
    const wrap = document.getElementById('modal-performance');
    if (!perf || perf.total_closed === 0) {
        wrap.innerHTML = '<div style="color:#8b949e;font-size:11px;padding:4px 0">No closed trades yet</div>';
        return;
    }
    const bestColor = perf.best_trade_pnl >= 0 ? '#3fb950' : '#f85149';
    const worstColor = perf.worst_trade_pnl >= 0 ? '#3fb950' : '#f85149';
    const avgColor = perf.avg_pnl >= 0 ? '#3fb950' : '#f85149';
    let html = '<div style="display:flex;flex-wrap:wrap;gap:8px;padding:4px 0">' +
        '<div style="background:#161b22;border:1px solid #30363d;border-radius:6px;padding:4px 10px;font-size:11px">' +
        '<span style="color:#8b949e">Avg Hold:</span> <span style="color:#c9d1d9;font-weight:600">' + perf.avg_hold_hours.toFixed(1) + 'h</span></div>' +
        '<div style="background:#161b22;border:1px solid #30363d;border-radius:6px;padding:4px 10px;font-size:11px">' +
        '<span style="color:#8b949e">Best:</span> <span style="color:' + bestColor + ';font-weight:600">$' + perf.best_trade_pnl.toFixed(2) + '</span></div>' +
        '<div style="background:#161b22;border:1px solid #30363d;border-radius:6px;padding:4px 10px;font-size:11px">' +
        '<span style="color:#8b949e">Worst:</span> <span style="color:' + worstColor + ';font-weight:600">$' + perf.worst_trade_pnl.toFixed(2) + '</span></div>' +
        '<div style="background:#161b22;border:1px solid #30363d;border-radius:6px;padding:4px 10px;font-size:11px">' +
        '<span style="color:#8b949e">Avg P&L:</span> <span style="color:' + avgColor + ';font-weight:600">$' + perf.avg_pnl.toFixed(2) + '</span></div>' +
        '<div style="background:#161b22;border:1px solid #30363d;border-radius:6px;padding:4px 10px;font-size:11px">' +
        '<span style="color:#8b949e">TP Hits:</span> <span style="color:#3fb950;font-weight:600">' + perf.tp_hits + '</span></div>' +
        '<div style="background:#161b22;border:1px solid #30363d;border-radius:6px;padding:4px 10px;font-size:11px">' +
        '<span style="color:#8b949e">SL Hits:</span> <span style="color:#f85149;font-weight:600">' + perf.sl_hits + '</span></div>' +
        '<div style="background:#161b22;border:1px solid #30363d;border-radius:6px;padding:4px 10px;font-size:11px">' +
        '<span style="color:#8b949e">Closed:</span> <span style="color:#c9d1d9;font-weight:600">' + perf.total_closed + '</span></div>' +
        '</div>';
    if (perf.total_fees > 0) {
        html += '<div style="display:flex;flex-wrap:wrap;gap:8px;padding:4px 0;margin-top:4px">' +
            '<div style="background:#161b22;border:1px solid #da8b45;border-radius:6px;padding:4px 10px;font-size:11px">' +
            '<span style="color:#da8b45">Total Fees:</span> <span style="color:#da8b45;font-weight:600">$' + perf.total_fees.toFixed(4) + '</span></div>' +
            '<div style="background:#161b22;border:1px solid #30363d;border-radius:6px;padding:4px 10px;font-size:11px">' +
            '<span style="color:#8b949e">Gas:</span> <span style="color:#da8b45;font-weight:600">$' + perf.total_gas.toFixed(4) + '</span></div>' +
            '<div style="background:#161b22;border:1px solid #30363d;border-radius:6px;padding:4px 10px;font-size:11px">' +
            '<span style="color:#8b949e">Slippage:</span> <span style="color:#da8b45;font-weight:600">$' + perf.total_slippage.toFixed(4) + '</span></div>' +
            '<div style="background:#161b22;border:1px solid #30363d;border-radius:6px;padding:4px 10px;font-size:11px">' +
            '<span style="color:#8b949e">Platform:</span> <span style="color:#da8b45;font-weight:600">$' + perf.total_platform.toFixed(4) + '</span></div>' +
            '<div style="background:#161b22;border:1px solid #30363d;border-radius:6px;padding:4px 10px;font-size:11px">' +
            '<span style="color:#8b949e">Avg/Trade:</span> <span style="color:#da8b45;font-weight:600">$' + perf.avg_fee_per_trade.toFixed(4) + '</span></div>' +
            '</div>';
    }
    wrap.innerHTML = html;
}

function renderModalTrades(trades) {
    const wrap = document.getElementById('modal-trades');
    if (!trades || trades.length === 0) {
        wrap.innerHTML = '<div class="empty-state"><p>No trades yet</p></div>';
        return;
    }
    let html = '<table class="data-table"><thead><tr>' +
        '<th>Time</th><th>Question</th><th>Dir</th><th>Mode</th><th>Entry</th>' +
        '<th>Edge</th><th>Size</th><th>P&L</th><th>Fees</th><th>Net</th><th>Exit</th><th>Status</th>' +
        '</tr></thead><tbody>';
    trades.forEach(t => {
        const pnlF = parseFloat(t.pnl) || 0;
        const pnlColor = pnlF > 0 ? 'color:#3fb950' : pnlF < 0 ? 'color:#f85149' : '';
        const statusCls = t.status === 'Open' ? 'open' : t.status === 'Won' ? 'won' : 'lost';
        const pnlDisplay = t.status === 'Open' ? '--' : (pnlF >= 0 ? '+' : '') + pnlF.toFixed(4);
        const feesF = parseFloat(t.total_fees) || 0;
        const feesDisplay = feesF > 0 ? '$' + feesF.toFixed(4) : '--';
        const netF = parseFloat(t.net_pnl) || 0;
        const netColor = netF > 0 ? 'color:#3fb950' : netF < 0 ? 'color:#f85149' : '';
        const netDisplay = t.status === 'Open' ? '--' : (netF >= 0 ? '+' : '') + netF.toFixed(4);
        html += '<tr>' +
            '<td>' + truncate(t.timestamp, 16) + '</td>' +
            '<td title="' + escHtml(t.question) + '">' + truncate(t.question, 22) + '</td>' +
            '<td>' + t.direction + '</td>' +
            '<td>' + (t.trade_mode || '?') + '</td>' +
            '<td>' + t.entry_price + '</td>' +
            '<td>' + t.edge + '</td>' +
            '<td>$' + t.bet_size + '</td>' +
            '<td style="' + pnlColor + '">' + pnlDisplay + '</td>' +
            '<td style="color:#da8b45">' + feesDisplay + '</td>' +
            '<td style="' + netColor + '">' + netDisplay + '</td>' +
            '<td>' + exitReasonBadge(t.exit_reason) + '</td>' +
            '<td><span class="status-badge ' + statusCls + '">' + t.status + '</span></td>' +
            '</tr>';
    });
    html += '</tbody></table>';
    wrap.innerHTML = html;
}

function renderModalAnalyses(analyses) {
    const wrap = document.getElementById('modal-analyses');
    if (!analyses || analyses.length === 0) {
        wrap.innerHTML = '<div class="empty-state"><p>No analyses yet</p></div>';
        return;
    }
    let html = '<table class="data-table"><thead><tr>' +
        '<th>Time</th><th>Question</th><th>Dir</th><th>Price</th><th>FV</th>' +
        '<th>Edge</th><th>Conf</th><th>Model</th><th>Trade?</th><th>Reasoning</th>' +
        '</tr></thead><tbody>';
    analyses.forEach(a => {
        html += '<tr>' +
            '<td>' + truncate(a.timestamp, 19) + '</td>' +
            '<td title="' + escHtml(a.question) + '">' + truncate(a.question, 20) + '</td>' +
            '<td>' + a.direction + '</td>' +
            '<td>' + a.current_price + '</td>' +
            '<td>' + a.fair_value + '</td>' +
            '<td>' + a.edge + '</td>' +
            '<td>' + a.confidence + '</td>' +
            '<td>' + a.model + '</td>' +
            '<td style="color:' + (a.should_trade ? '#3fb950' : '#8b949e') + '">' + (a.should_trade ? 'YES' : 'no') + '</td>' +
            '<td class="reasoning-cell" title="' + escHtml(a.reasoning) + '">' + truncate(a.reasoning, 30) + '</td>' +
            '</tr>';
    });
    html += '</tbody></table>';
    wrap.innerHTML = html;
}

function renderModalCycles(cycles) {
    const wrap = document.getElementById('modal-cycles');
    if (!cycles || cycles.length === 0) {
        wrap.innerHTML = '<div class="empty-state"><p>No cycles yet</p></div>';
        return;
    }
    let html = '<table class="data-table"><thead><tr>' +
        '<th>Time</th><th>Scanned</th><th>Analyzed</th><th>Traded</th>' +
        '<th>API Cost</th><th>Balance</th>' +
        '</tr></thead><tbody>';
    cycles.forEach(c => {
        html += '<tr>' +
            '<td>' + truncate(c.timestamp, 19) + '</td>' +
            '<td>' + c.markets_scanned + '</td>' +
            '<td>' + c.markets_analyzed + '</td>' +
            '<td>' + c.trades_placed + '</td>' +
            '<td>$' + c.api_cost + '</td>' +
            '<td>$' + c.balance_after + '</td>' +
            '</tr>';
    });
    html += '</tbody></table>';
    wrap.innerHTML = html;
}

function switchModalTab(e, tabId) {
    document.querySelectorAll('.modal-tab').forEach(t => t.classList.remove('active'));
    document.querySelectorAll('.modal-tab-content').forEach(t => t.classList.remove('active'));
    e.target.classList.add('active');
    document.getElementById(tabId).classList.add('active');
}

// Update TP/SL countdowns in real-time (every second)
function updateTPSLCountdowns() {
    const countdowns = document.querySelectorAll('.tpsl-countdown');
    countdowns.forEach(elem => {
        const agentId = elem.getAttribute('data-agent');
        // This will be updated on next refresh, but we can animate the bar
    });

    // Update timer bars
    const timerFills = document.querySelectorAll('.tpsl-timer-fill');
    timerFills.forEach(fill => {
        const currentWidth = parseFloat(fill.style.width || '100');
        // Decrease gradually (will reset on next refresh)
        if (currentWidth > 0) {
            const newWidth = Math.max(0, currentWidth - 0.2); // ~0.2% per second
            fill.style.width = newWidth + '%';
        }
    });
}

// Initial load + auto-refresh
refresh();
setInterval(refresh, 5000);
setInterval(updateTPSLCountdowns, 1000); // Update countdowns every second
</script>
</body>
</html>
"##;
