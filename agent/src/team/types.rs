use crate::types::{Direction, EnrichmentData, Market};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Trade mode classification from Strategist
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub enum TradeMode {
    Scalp,      // Short-term, tight TP/SL
    Swing,      // Medium-term, dynamic exits
    Conviction, // High-confidence, hold to resolution
}

impl std::fmt::Display for TradeMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TradeMode::Scalp => write!(f, "SCALP"),
            TradeMode::Swing => write!(f, "SWING"),
            TradeMode::Conviction => write!(f, "CONVICTION"),
        }
    }
}

/// Case strength rating from Bull/Bear analysts
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub enum CaseStrength {
    Weak,
    Moderate,
    Strong,
    Overwhelming,
}

impl std::fmt::Display for CaseStrength {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CaseStrength::Weak => write!(f, "WEAK"),
            CaseStrength::Moderate => write!(f, "MODERATE"),
            CaseStrength::Strong => write!(f, "STRONG"),
            CaseStrength::Overwhelming => write!(f, "OVERWHELMING"),
        }
    }
}

/// Single market candidate from Scout
#[derive(Debug, Clone)]
pub struct MarketCandidate {
    pub market: Market,
    pub quality_score: f64,
    #[allow(dead_code)]
    pub reason: String,
}

/// Scout output: filtered and scored market candidates
#[derive(Debug, Clone)]
pub struct ScoutReport {
    pub candidates: Vec<MarketCandidate>,
    pub total_scanned: usize,
    pub total_passed_quality: usize,
}

/// Data Analyst output: quantitative data per candidate
#[derive(Debug, Clone)]
pub struct DataPack {
    pub market_id: String,
    pub enrichment: EnrichmentData,
    #[allow(dead_code)]
    pub price_trend_24h: Option<f64>,
    #[allow(dead_code)]
    pub volume_trend: Option<f64>,
    pub order_book_spread: Option<Decimal>,
    pub order_book_bid_depth: Option<Decimal>,
    pub order_book_ask_depth: Option<Decimal>,
}

/// Researcher output: AI-researched context per candidate
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResearchDossier {
    pub market_id: String,
    pub news_relevance: String,
    pub fact_check: String,
    pub base_rate: f64,
    pub counter_arguments: String,
    pub key_factors: Vec<String>,
}

/// Bull Analyst output: strongest YES case
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BullCase {
    pub market_id: String,
    pub probability_yes: f64,
    pub case_strength: String, // Parsed to CaseStrength
    pub arguments: Vec<String>,
    pub evidence: Vec<String>,
    pub reasoning: String,
}

impl BullCase {
    pub fn strength(&self) -> CaseStrength {
        match self.case_strength.to_uppercase().as_str() {
            "OVERWHELMING" => CaseStrength::Overwhelming,
            "STRONG" => CaseStrength::Strong,
            "MODERATE" => CaseStrength::Moderate,
            _ => CaseStrength::Weak,
        }
    }
}

/// Bear Analyst output: strongest NO case
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BearCase {
    pub market_id: String,
    pub probability_no: f64,
    pub case_strength: String,
    pub arguments: Vec<String>,
    pub evidence: Vec<String>,
    pub reasoning: String,
}

impl BearCase {
    pub fn strength(&self) -> CaseStrength {
        match self.case_strength.to_uppercase().as_str() {
            "OVERWHELMING" => CaseStrength::Overwhelming,
            "STRONG" => CaseStrength::Strong,
            "MODERATE" => CaseStrength::Moderate,
            _ => CaseStrength::Weak,
        }
    }
}

/// Devil's Advocate final verdict
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DevilsVerdict {
    pub market_id: String,
    pub fair_value_yes: f64,
    pub confidence: f64,
    pub direction: String, // "YES", "NO", "SKIP"
    pub reasoning: String,
    pub bull_flaws: String,
    pub bear_flaws: String,
}

impl DevilsVerdict {
    pub fn direction_enum(&self) -> Direction {
        match self.direction.to_uppercase().as_str() {
            "YES" => Direction::Yes,
            "NO" => Direction::No,
            _ => Direction::Skip,
        }
    }
}

/// Risk Manager output
#[derive(Debug, Clone)]
pub struct RiskDecision {
    pub approved: bool,
    pub position_size: Decimal,
    pub reason: String,
    #[allow(dead_code)]
    pub adjustments: Vec<String>,
}

/// Strategist output: full trade execution plan
#[derive(Debug, Clone)]
pub struct TradePlan {
    pub market: Market,
    pub direction: Direction,
    pub fair_value_yes: Decimal,
    pub edge: Decimal,
    pub confidence: Decimal,
    pub mode: TradeMode,
    pub bet_size: Decimal,
    #[allow(dead_code)]
    pub entry_price: Decimal,
    pub take_profit_pct: Decimal,
    pub stop_loss_pct: Decimal,
    pub max_hold_hours: u64,
    #[allow(dead_code)]
    pub check_interval_secs: u64,
    pub reasoning: String,
    // Agent trail for paper trading
    pub specialist_desk: Option<String>,
    pub bull_probability: Option<f64>,
    pub bear_probability: Option<f64>,
    pub judge_model: Option<String>,
}

/// Cycle statistics for the team pipeline
#[derive(Debug, Clone, Default)]
pub struct TeamCycleStats {
    pub markets_scanned: usize,
    pub markets_passed_quality: usize,
    pub markets_researched: usize,
    pub markets_analyzed: usize, // Bull/Bear/Devil treatment
    pub markets_approved: usize,
    pub trades_placed: usize,
    pub api_cost: Decimal,
}

/// Auditor insight from post-trade learning
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditInsight {
    pub timestamp: String,
    pub trade_count: usize,
    pub win_rate: f64,
    pub avg_calibration_error: f64,
    pub insights: Vec<String>,
    pub bull_accuracy: f64,
    pub bear_accuracy: f64,
    #[serde(default)]
    pub desk_accuracy: HashMap<String, f64>,
}

/// Specialist Desk type — routes markets to domain experts
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum DeskType {
    Crypto,
    Weather,
    Sports,
    General,
}

impl std::fmt::Display for DeskType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DeskType::Crypto => write!(f, "CRYPTO"),
            DeskType::Weather => write!(f, "WEATHER"),
            DeskType::Sports => write!(f, "SPORTS"),
            DeskType::General => write!(f, "GENERAL"),
        }
    }
}

/// Detect desk type from market question and category
pub fn detect_desk(question: &str, category: &str) -> DeskType {
    let q = question.to_lowercase();
    let c = category.to_lowercase();

    // Crypto detection
    if c.contains("crypto")
        || q.contains("bitcoin")
        || q.contains("btc")
        || q.contains("ethereum")
        || q.contains("eth ")
        || q.contains("solana")
        || q.contains("sol ")
        || q.contains("crypto")
        || q.contains("token")
        || q.contains("defi")
        || q.contains("blockchain")
    {
        return DeskType::Crypto;
    }

    // Weather detection
    if c.contains("weather")
        || q.contains("temperature")
        || q.contains("rain")
        || q.contains("snow")
        || q.contains("hurricane")
        || q.contains("weather")
        || q.contains("forecast")
        || q.contains("degrees")
        || q.contains("celsius")
        || q.contains("fahrenheit")
    {
        return DeskType::Weather;
    }

    // Sports detection
    if c.contains("sports")
        || c.contains("nfl")
        || c.contains("nba")
        || c.contains("mlb")
        || c.contains("soccer")
        || q.contains("win the")
        || q.contains("championship")
        || q.contains("super bowl")
        || q.contains("world cup")
        || q.contains("playoffs")
        || q.contains("mvp")
        || q.contains("score")
    {
        return DeskType::Sports;
    }

    DeskType::General
}

/// Specialist Desk report — domain expert analysis
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeskReport {
    pub market_id: String,
    pub desk: DeskType,
    pub specialist_probability: f64,
    pub key_factors: Vec<String>,
    pub risk_assessment: String,
    pub data_summary: String,
    pub confidence_in_data: f64,
}
