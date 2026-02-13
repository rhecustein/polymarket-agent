use crate::paper::PortfolioStats;
use crate::types::{Analysis, Trade};
use anyhow::{Context, Result};
use rust_decimal::Decimal;
use rusqlite::Connection;

pub struct StateStore {
    conn: Connection,
    json_log_path: String,
}

impl StateStore {
    pub fn new(db_path: &str) -> Result<Self> {
        let conn = Connection::open(db_path)
            .with_context(|| format!("Open DB: {db_path}"))?;

        conn.execute_batch(
            "
            CREATE TABLE IF NOT EXISTS trades (
                id TEXT PRIMARY KEY,
                timestamp TEXT NOT NULL,
                market_id TEXT NOT NULL,
                question TEXT NOT NULL,
                direction TEXT NOT NULL,
                entry_price TEXT NOT NULL,
                fair_value TEXT NOT NULL,
                edge TEXT NOT NULL,
                bet_size TEXT NOT NULL,
                shares TEXT NOT NULL,
                status TEXT NOT NULL,
                exit_price TEXT,
                pnl TEXT NOT NULL,
                balance_after TEXT NOT NULL,
                order_id TEXT,
                trade_mode TEXT,
                take_profit TEXT,
                stop_loss TEXT,
                max_hold_until TEXT,
                category TEXT,
                specialist_desk TEXT,
                bull_probability REAL,
                bear_probability REAL,
                judge_fair_value REAL,
                judge_confidence REAL,
                judge_model TEXT,
                exit_reason TEXT,
                hold_duration_hours REAL,
                token_id TEXT,
                raw_entry_price TEXT,
                raw_exit_price TEXT,
                entry_gas_fee TEXT DEFAULT '0',
                exit_gas_fee TEXT DEFAULT '0',
                entry_slippage TEXT DEFAULT '0',
                exit_slippage TEXT DEFAULT '0',
                platform_fee TEXT DEFAULT '0',
                maker_taker_fee TEXT DEFAULT '0'
            );

            CREATE TABLE IF NOT EXISTS analyses (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                timestamp TEXT NOT NULL,
                market_id TEXT NOT NULL,
                question TEXT NOT NULL,
                current_price TEXT NOT NULL,
                fair_value TEXT NOT NULL,
                edge TEXT NOT NULL,
                direction TEXT NOT NULL,
                should_trade INTEGER NOT NULL,
                reasoning TEXT,
                model TEXT,
                api_cost TEXT NOT NULL,
                confidence TEXT,
                enrichment_summary TEXT
            );

            CREATE TABLE IF NOT EXISTS daily_snapshots (
                date TEXT PRIMARY KEY,
                balance TEXT NOT NULL,
                pnl TEXT NOT NULL,
                roi TEXT NOT NULL,
                win_count INTEGER,
                loss_count INTEGER,
                api_cost TEXT NOT NULL,
                trades_count INTEGER,
                win_rate TEXT,
                max_drawdown TEXT
            );

            CREATE TABLE IF NOT EXISTS cycles (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                timestamp TEXT NOT NULL,
                markets_scanned INTEGER,
                markets_analyzed INTEGER,
                trades_placed INTEGER,
                api_cost_cycle TEXT,
                balance_after TEXT
            );

            CREATE TABLE IF NOT EXISTS market_cache (
                market_id TEXT PRIMARY KEY,
                question TEXT NOT NULL,
                last_analyzed TEXT NOT NULL,
                fair_value TEXT,
                confidence TEXT,
                direction TEXT,
                should_trade INTEGER
            );

            CREATE TABLE IF NOT EXISTS price_log (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                timestamp TEXT NOT NULL,
                market_id TEXT NOT NULL,
                token_id TEXT,
                bid TEXT NOT NULL,
                ask TEXT NOT NULL,
                mid TEXT NOT NULL,
                spread TEXT NOT NULL,
                source TEXT DEFAULT 'clob.polymarket.com'
            );

            CREATE TABLE IF NOT EXISTS cycle_log (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                timestamp TEXT NOT NULL,
                cycle_number INTEGER NOT NULL,
                markets_scanned INTEGER,
                candidates_found INTEGER,
                trades_opened INTEGER,
                trades_closed INTEGER,
                balance_after TEXT,
                open_positions_count INTEGER,
                api_cost_estimated TEXT,
                duration_secs REAL
            );

            CREATE INDEX IF NOT EXISTS idx_price_log_market ON price_log(market_id, timestamp);
            CREATE INDEX IF NOT EXISTS idx_cycle_log_time ON cycle_log(timestamp);
            
            CREATE TABLE IF NOT EXISTS agent_status (
                id TEXT PRIMARY KEY CHECK (id = 'current'),
                phase TEXT NOT NULL,
                details TEXT,
                updated_at TEXT NOT NULL
            );
            ",
        )
        .context("Create tables")?;

        // Migrate simulation columns for existing DBs
        migrate_simulation_columns(&conn);

        let json_log_path = db_path.replace(".db", "_trades.jsonl");

        Ok(Self { conn, json_log_path })
    }

    /// Update the agent's current status/phase
    pub fn update_status(&self, phase: &str, details: &str) -> Result<()> {
        self.conn.execute(
            "INSERT OR REPLACE INTO agent_status (id, phase, details, updated_at)
             VALUES ('current', ?1, ?2, ?3)",
            rusqlite::params![
                phase,
                details,
                chrono::Utc::now().to_rfc3339(),
            ],
        )?;
        Ok(())
    }

    /// Check if a market was recently analyzed (within `hours` hours)
    pub fn was_recently_analyzed(&self, market_id: &str, hours: i64) -> bool {
        let cutoff = (chrono::Utc::now() - chrono::Duration::hours(hours))
            .to_rfc3339();

        self.conn
            .query_row(
                "SELECT COUNT(*) FROM market_cache WHERE market_id = ?1 AND last_analyzed > ?2",
                rusqlite::params![market_id, cutoff],
                |row| row.get::<_, i64>(0),
            )
            .unwrap_or(0)
            > 0
    }

    /// Cache an analysis result for a market
    pub fn cache_analysis(&self, analysis: &Analysis) -> Result<()> {
        self.conn.execute(
            "INSERT OR REPLACE INTO market_cache (market_id, question, last_analyzed, fair_value, confidence, direction, should_trade)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            rusqlite::params![
                analysis.market_id,
                analysis.question,
                chrono::Utc::now().to_rfc3339(),
                analysis.fair_value_yes.to_string(),
                analysis.confidence.to_string(),
                format!("{}", analysis.direction),
                analysis.should_trade as i32,
            ],
        )?;
        Ok(())
    }

    pub fn save_trade(&self, trade: &Trade) -> Result<()> {
        self.conn.execute(
            "INSERT OR REPLACE INTO trades (id, timestamp, market_id, question, direction,
             entry_price, fair_value, edge, bet_size, shares, status, exit_price, pnl, balance_after, order_id,
             trade_mode, take_profit, stop_loss, max_hold_until, category, specialist_desk,
             bull_probability, bear_probability, judge_fair_value, judge_confidence, judge_model,
             exit_reason, hold_duration_hours, token_id,
             raw_entry_price, raw_exit_price, entry_gas_fee, exit_gas_fee,
             entry_slippage, exit_slippage, platform_fee, maker_taker_fee)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15,
                     ?16, ?17, ?18, ?19, ?20, ?21, ?22, ?23, ?24, ?25, ?26, ?27, ?28, ?29,
                     ?30, ?31, ?32, ?33, ?34, ?35, ?36, ?37)",
            rusqlite::params![
                trade.id,
                trade.timestamp.to_rfc3339(),
                trade.market_id,
                trade.question,
                format!("{}", trade.direction),
                trade.entry_price.to_string(),
                trade.fair_value.to_string(),
                trade.edge.to_string(),
                trade.bet_size.to_string(),
                trade.shares.to_string(),
                format!("{:?}", trade.status),
                trade.exit_price.map(|p| p.to_string()),
                trade.pnl.to_string(),
                trade.balance_after.to_string(),
                trade.order_id,
                trade.trade_mode,
                trade.take_profit.map(|p| p.to_string()),
                trade.stop_loss.map(|p| p.to_string()),
                trade.max_hold_until.map(|d| d.to_rfc3339()),
                trade.category,
                trade.specialist_desk,
                trade.bull_probability,
                trade.bear_probability,
                trade.judge_fair_value,
                trade.judge_confidence,
                trade.judge_model,
                trade.exit_reason.map(|r| format!("{}", r)),
                trade.hold_duration_hours,
                trade.token_id,
                trade.raw_entry_price.map(|p| p.to_string()),
                trade.raw_exit_price.map(|p| p.to_string()),
                trade.entry_gas_fee.to_string(),
                trade.exit_gas_fee.to_string(),
                trade.entry_slippage.to_string(),
                trade.exit_slippage.to_string(),
                trade.platform_fee.to_string(),
                trade.maker_taker_fee.to_string(),
            ],
        )?;

        // Append to JSON log
        if let Ok(json) = serde_json::to_string(trade) {
            use std::io::Write;
            if let Ok(mut f) = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(&self.json_log_path)
            {
                let _ = writeln!(f, "{json}");
            }
        }

        Ok(())
    }

    pub fn save_analysis(&self, a: &Analysis) -> Result<()> {
        self.conn.execute(
            "INSERT INTO analyses (timestamp, market_id, question, current_price, fair_value,
             edge, direction, should_trade, reasoning, model, api_cost, confidence, enrichment_summary)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
            rusqlite::params![
                chrono::Utc::now().to_rfc3339(),
                a.market_id,
                a.question,
                a.current_yes_price.to_string(),
                a.fair_value_yes.to_string(),
                a.edge.to_string(),
                format!("{}", a.direction),
                a.should_trade as i32,
                a.reasoning,
                a.model_used,
                a.api_cost_usd.to_string(),
                a.confidence.to_string(),
                a.enrichment_data,
            ],
        )?;

        // Also cache the analysis
        self.cache_analysis(a).ok();

        Ok(())
    }

    pub fn save_cycle(
        &self,
        markets_scanned: usize,
        markets_analyzed: usize,
        trades_placed: usize,
        api_cost: &str,
        balance: &str,
    ) -> Result<()> {
        self.conn.execute(
            "INSERT INTO cycles (timestamp, markets_scanned, markets_analyzed, trades_placed, api_cost_cycle, balance_after)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            rusqlite::params![
                chrono::Utc::now().to_rfc3339(),
                markets_scanned,
                markets_analyzed,
                trades_placed,
                api_cost,
                balance,
            ],
        )?;
        Ok(())
    }

    /// Log a price observation for audit trail
    pub fn log_price(
        &self,
        market_id: &str,
        token_id: Option<&str>,
        bid: Decimal,
        ask: Decimal,
        mid: Decimal,
        spread: Decimal,
    ) -> Result<()> {
        self.conn.execute(
            "INSERT INTO price_log (timestamp, market_id, token_id, bid, ask, mid, spread)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            rusqlite::params![
                chrono::Utc::now().to_rfc3339(),
                market_id,
                token_id,
                bid.to_string(),
                ask.to_string(),
                mid.to_string(),
                spread.to_string(),
            ],
        )?;
        Ok(())
    }

    /// Log a full cycle with enhanced metrics
    pub fn log_cycle(
        &self,
        cycle_number: u64,
        markets_scanned: usize,
        candidates_found: usize,
        trades_opened: usize,
        trades_closed: usize,
        balance_after: Decimal,
        open_count: usize,
        api_cost: Decimal,
        duration_secs: f64,
    ) -> Result<()> {
        self.conn.execute(
            "INSERT INTO cycle_log (timestamp, cycle_number, markets_scanned, candidates_found, trades_opened, trades_closed, balance_after, open_positions_count, api_cost_estimated, duration_secs)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            rusqlite::params![
                chrono::Utc::now().to_rfc3339(),
                cycle_number,
                markets_scanned,
                candidates_found,
                trades_opened,
                trades_closed,
                balance_after.to_string(),
                open_count,
                api_cost.to_string(),
                duration_secs,
            ],
        )?;
        Ok(())
    }

    /// Save enhanced trade with paper trading fields
    pub fn save_paper_trade(&self, trade: &Trade) -> Result<()> {
        // Use the standard save_trade for core fields
        self.save_trade(trade)?;
        Ok(())
    }

    pub fn save_daily_snapshot(&self, stats: &PortfolioStats) -> Result<()> {
        let today = chrono::Utc::now().format("%Y-%m-%d").to_string();
        self.conn.execute(
            "INSERT OR REPLACE INTO daily_snapshots (date, balance, pnl, roi, win_count, loss_count, api_cost, trades_count, win_rate, max_drawdown)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            rusqlite::params![
                today,
                stats.balance.to_string(),
                stats.total_pnl.to_string(),
                stats.roi.to_string(),
                stats.win_count,
                stats.loss_count,
                stats.total_api_cost.to_string(),
                stats.win_count + stats.loss_count,
                format!("{:.1}", stats.win_rate),
                stats.max_drawdown_pct.to_string(),
            ],
        )?;
        Ok(())
    }
}

/// Migrate simulation columns for existing databases
fn migrate_simulation_columns(conn: &Connection) {
    let columns = [
        ("raw_entry_price", "TEXT"),
        ("raw_exit_price", "TEXT"),
        ("entry_gas_fee", "TEXT DEFAULT '0'"),
        ("exit_gas_fee", "TEXT DEFAULT '0'"),
        ("entry_slippage", "TEXT DEFAULT '0'"),
        ("exit_slippage", "TEXT DEFAULT '0'"),
        ("platform_fee", "TEXT DEFAULT '0'"),
        ("maker_taker_fee", "TEXT DEFAULT '0'"),
    ];

    for (col, typ) in &columns {
        // Check if column exists by trying a query
        let exists = conn
            .prepare(&format!("SELECT {col} FROM trades LIMIT 0"))
            .is_ok();
        if !exists {
            let sql = format!("ALTER TABLE trades ADD COLUMN {col} {typ}");
            conn.execute_batch(&sql).ok();
        }
    }
}
