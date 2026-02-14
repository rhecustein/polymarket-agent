//! Local Knowledge Collection System
//!
//! Tracks trading performance metrics for strategy optimization:
//! - Win rate per market category
//! - Fee & slippage impact analysis
//! - Entry timing & confidence correlation
//! - Strategy parameter optimization data

use crate::db::StateStore;
use crate::types::{Trade, TradeStatus};
use anyhow::Result;
use rust_decimal::Decimal;
use tracing::{debug, info};

/// Knowledge collector for local analytics
pub struct KnowledgeCollector<'a> {
    store: &'a StateStore,
}

impl<'a> KnowledgeCollector<'a> {
    pub fn new(store: &'a StateStore) -> Self {
        Self { store }
    }

    /// Collect all knowledge when a trade closes
    pub fn collect_on_trade_close(&self, trade: &Trade) -> Result<()> {
        // Only collect for closed trades (Won, Lost, Cancelled)
        if matches!(trade.status, TradeStatus::Open) {
            return Ok(());
        }

        debug!("Collecting knowledge for trade {}", trade.id);

        // 1. Update category win rate stats
        if let Some(category) = &trade.category {
            self.store.update_category_stats(
                category,
                trade.trade_mode.as_deref(),
                trade,
            )?;
        }

        // 2. Record cost impact (fees + slippage)
        self.collect_cost_impact(trade)?;

        // 3. Record timing analysis
        self.store.record_timing_analysis(trade)?;

        info!(
            "Knowledge collected for {} trade in {:?} (PnL: {})",
            trade.direction,
            trade.category,
            trade.pnl
        );

        Ok(())
    }

    /// Calculate and record cost impact from fees and slippage
    fn collect_cost_impact(&self, trade: &Trade) -> Result<()> {
        let total_fees = trade.entry_gas_fee
            + trade.exit_gas_fee
            + trade.platform_fee
            + trade.maker_taker_fee;

        let total_slippage = trade.entry_slippage + trade.exit_slippage;

        // Calculate PnL before costs
        let pnl_before_costs = trade.pnl + total_fees + total_slippage;

        self.store.record_cost_impact(
            &trade.id,
            trade.bet_size,
            total_fees,
            total_slippage,
            pnl_before_costs,
            trade.pnl,
            trade.category.as_deref(),
        )?;

        Ok(())
    }

    /// Generate summary of collected knowledge
    pub fn get_summary(&self) -> Result<KnowledgeSummary> {
        let mut summary = KnowledgeSummary::default();

        // Category performance
        let mut stmt = self.store.conn.prepare(
            "SELECT category, trade_mode, total_trades, win_rate, total_pnl, avg_edge
             FROM knowledge_category_stats
             ORDER BY win_rate DESC"
        )?;

        let rows = stmt.query_map([], |row| {
            Ok(CategoryPerformance {
                category: row.get(0)?,
                trade_mode: row.get(1)?,
                total_trades: row.get(2)?,
                win_rate: row.get(3)?,
                total_pnl: row.get::<_, String>(4)?.parse().ok(),
                avg_edge: row.get(5)?,
            })
        })?;

        summary.category_performance = rows.filter_map(|r| r.ok()).collect();

        // Cost impact averages
        let (avg_fee_pct, avg_slip_pct, avg_cost_impact): (Option<f64>, Option<f64>, Option<f64>) =
            self.store.conn.query_row(
                "SELECT AVG(fee_pct_of_size), AVG(slippage_pct_of_size), AVG(cost_impact_pct)
                 FROM knowledge_cost_impact",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            ).unwrap_or((None, None, None));

        summary.avg_fee_pct = avg_fee_pct.unwrap_or(0.0);
        summary.avg_slippage_pct = avg_slip_pct.unwrap_or(0.0);
        summary.avg_cost_impact_pct = avg_cost_impact.unwrap_or(0.0);

        // Best timing patterns
        let mut timing_stmt = self.store.conn.prepare(
            "SELECT entry_hour, COUNT(*) as cnt, AVG(pnl_pct) as avg_pnl, SUM(CASE WHEN result = 'win' THEN 1 ELSE 0 END) * 100.0 / COUNT(*) as win_rate
             FROM knowledge_timing_analysis
             GROUP BY entry_hour
             HAVING cnt >= 3
             ORDER BY win_rate DESC
             LIMIT 5"
        )?;

        let timing_rows = timing_stmt.query_map([], |row| {
            Ok(TimingPattern {
                entry_hour: row.get(0)?,
                trade_count: row.get(1)?,
                avg_pnl_pct: row.get(2)?,
                win_rate: row.get(3)?,
            })
        })?;

        summary.best_timing_patterns = timing_rows.filter_map(|r| r.ok()).collect();

        Ok(summary)
    }
}

#[derive(Debug, Default)]
pub struct KnowledgeSummary {
    pub category_performance: Vec<CategoryPerformance>,
    pub avg_fee_pct: f64,
    pub avg_slippage_pct: f64,
    pub avg_cost_impact_pct: f64,
    pub best_timing_patterns: Vec<TimingPattern>,
}

#[derive(Debug)]
pub struct CategoryPerformance {
    pub category: String,
    pub trade_mode: Option<String>,
    pub total_trades: i64,
    pub win_rate: f64,
    pub total_pnl: Option<Decimal>,
    pub avg_edge: f64,
}

#[derive(Debug)]
pub struct TimingPattern {
    pub entry_hour: i64,
    pub trade_count: i64,
    pub avg_pnl_pct: f64,
    pub win_rate: f64,
}

impl KnowledgeSummary {
    /// Get best performing category
    pub fn best_category(&self) -> Option<&CategoryPerformance> {
        self.category_performance.first()
    }

    /// Get categories to avoid (win rate < 40%)
    pub fn poor_categories(&self) -> Vec<&CategoryPerformance> {
        self.category_performance
            .iter()
            .filter(|c| c.win_rate < 0.40 && c.total_trades >= 5)
            .collect()
    }

    /// Format as human-readable report
    pub fn to_report(&self) -> String {
        let mut report = String::from("═══ KNOWLEDGE SUMMARY ═══\n\n");

        // Category performance
        report.push_str("📊 Category Performance:\n");
        for cat in &self.category_performance {
            let mode_str = cat.trade_mode.as_deref().unwrap_or("all");
            let pnl_str = cat.total_pnl
                .map(|p| format!("${:.2}", p))
                .unwrap_or_else(|| "$0.00".to_string());
            report.push_str(&format!(
                "  {} ({}): {:.1}% win rate, {} trades, {} PnL, {:.1}% avg edge\n",
                cat.category,
                mode_str,
                cat.win_rate * 100.0,
                cat.total_trades,
                pnl_str,
                cat.avg_edge * 100.0,
            ));
        }

        // Cost impact
        report.push_str(&format!(
            "\n💰 Cost Impact:\n  Avg Fees: {:.2}% of bet size\n  Avg Slippage: {:.2}% of bet size\n  Avg Total Impact: {:.2}%\n",
            self.avg_fee_pct,
            self.avg_slippage_pct,
            self.avg_cost_impact_pct,
        ));

        // Best timing
        if !self.best_timing_patterns.is_empty() {
            report.push_str("\n⏰ Best Entry Times (UTC):\n");
            for pattern in &self.best_timing_patterns {
                report.push_str(&format!(
                    "  {:02}:00-{:02}:59: {:.1}% win rate, {} trades, {:.1}% avg PnL\n",
                    pattern.entry_hour,
                    pattern.entry_hour,
                    pattern.win_rate,
                    pattern.trade_count,
                    pattern.avg_pnl_pct,
                ));
            }
        }

        report
    }
}
