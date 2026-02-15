use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::fmt;

/// Token info from CLOB
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenInfo {
    pub token_id: String,
    pub outcome: String,
    pub price: Decimal,
}

/// Raw market from Polymarket Gamma API
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Market {
    pub id: String,
    pub question: String,
    pub description: String,
    pub category: String,
    pub end_date: String,
    pub yes_price: Decimal,
    pub no_price: Decimal,
    pub volume: Decimal,
    pub liquidity: Decimal,
    pub tokens: Vec<TokenInfo>,
    pub slug: String,
    pub fetched_at: DateTime<Utc>,
}

/// Pre-filter score for Tier 0 heuristic ranking (Legacy - not used in v2.0)
#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct ScoredMarket {
    pub market: Market,
    pub heuristic_score: f64, // 0-100, higher = more likely mispriced
    pub reason: String,
}

/// Haiku quick-screen result (Tier 1) (Legacy - team v2.0 uses different pipeline)
#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuickScreen {
    pub market_id: String,
    pub worth_deep_analysis: bool,
    pub estimated_edge: Decimal,
    pub category_match: bool,
    pub reasoning: String,
}

/// Full Claude analysis result (Tier 2)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Analysis {
    pub market_id: String,
    pub question: String,
    pub current_yes_price: Decimal,
    pub fair_value_yes: Decimal,
    pub edge: Decimal,
    pub confidence: Decimal,
    pub direction: Direction,
    pub should_trade: bool,
    pub reasoning: String,
    pub api_cost_usd: Decimal,
    pub model_used: String,
    pub enrichment_data: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub enum Direction {
    Yes,
    No,
    Skip,
}

impl std::fmt::Display for Direction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Direction::Yes => write!(f, "YES"),
            Direction::No => write!(f, "NO"),
            Direction::Skip => write!(f, "SKIP"),
        }
    }
}

/// Trade record
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Trade {
    pub id: String,
    pub timestamp: DateTime<Utc>,
    pub market_id: String,
    pub question: String,
    pub direction: Direction,
    pub entry_price: Decimal,
    pub fair_value: Decimal,
    pub edge: Decimal,
    pub bet_size: Decimal,
    pub shares: Decimal,
    pub status: TradeStatus,
    pub exit_price: Option<Decimal>,
    pub pnl: Decimal,
    pub balance_after: Decimal,
    pub order_id: Option<String>,
    // Paper trading enrichment fields
    pub trade_mode: Option<String>,        // "SCALP", "SWING", "CONVICTION"
    pub take_profit: Option<Decimal>,      // TP price level
    pub stop_loss: Option<Decimal>,        // SL price level
    pub max_hold_until: Option<DateTime<Utc>>,
    pub category: Option<String>,          // "crypto", "weather", "sports", "general"
    pub specialist_desk: Option<String>,
    pub bull_probability: Option<f64>,
    pub bear_probability: Option<f64>,
    pub judge_fair_value: Option<f64>,
    pub judge_confidence: Option<f64>,
    pub judge_model: Option<String>,       // "sonnet" or "gemini"
    pub exit_reason: Option<ExitReason>,
    pub hold_duration_hours: Option<f64>,
    pub token_id: Option<String>,          // YES/NO token ID for CLOB pricing
    // Paper Trading Plus — simulation tracking
    pub raw_entry_price: Option<Decimal>,
    pub raw_exit_price: Option<Decimal>,
    pub entry_gas_fee: Decimal,
    pub exit_gas_fee: Decimal,
    pub entry_slippage: Decimal,
    pub exit_slippage: Decimal,
    pub platform_fee: Decimal,
    pub maker_taker_fee: Decimal,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub enum TradeStatus {
    Open,
    Won,
    Lost,
    Cancelled,
}

/// Exit reason for closed trades (paper trading)
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub enum ExitReason {
    TakeProfit,
    StopLoss,
    TimeExpiry,
    MarketResolved,
    ManualStop,
    SafetyValve,    // Conviction trade, loss > 50%
    EdgeCaptured,   // Swing trade, price moved 50%+ toward fair value
}

impl fmt::Display for ExitReason {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ExitReason::TakeProfit => write!(f, "TP"),
            ExitReason::StopLoss => write!(f, "SL"),
            ExitReason::TimeExpiry => write!(f, "TIME"),
            ExitReason::MarketResolved => write!(f, "RESOLVED"),
            ExitReason::ManualStop => write!(f, "MANUAL"),
            ExitReason::SafetyValve => write!(f, "SAFETY"),
            ExitReason::EdgeCaptured => write!(f, "EDGE"),
        }
    }
}

/// Enrichment data from external sources
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct EnrichmentData {
    pub crypto_signals: Option<CryptoSignal>,
    pub weather_data: Option<WeatherData>,
    pub news_headlines: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CryptoSignal {
    pub asset: String,
    pub price_24h_change_pct: f64,
    pub volume_24h_change_pct: f64,
    pub fear_greed_index: Option<u32>,
    pub current_price: f64,
    pub price_7d_change_pct: f64,
    pub rsi_14: Option<f64>,
    pub market_cap_rank: Option<u32>,
    pub btc_dominance: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DayForecast {
    pub date: String,
    pub high_f: f64,
    pub low_f: f64,
    pub condition: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WeatherData {
    pub location: String,
    pub temperature_f: f64,
    pub condition: String,
    pub forecast_summary: String,
    pub forecast_3day: Vec<DayForecast>,
    pub alerts: Vec<String>,
}
