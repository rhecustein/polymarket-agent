use anyhow::{Context, Result};
use rust_decimal::Decimal;
use std::str::FromStr;

#[derive(Debug, Clone)]
pub struct Config {
    pub claude_api_key: String,
    pub claude_model_haiku: String,
    pub claude_model_sonnet: String,
    pub initial_balance: Decimal,
    pub max_position_pct: Decimal,
    pub min_edge_threshold: Decimal,
    pub kill_threshold: Decimal,
    pub kelly_fraction: Decimal,
    pub scan_interval_secs: u64,
    pub max_markets_to_scan: usize,
    pub max_haiku_per_cycle: usize,
    pub max_sonnet_per_cycle: usize,
    pub gamma_api_base: String,
    pub polymarket_clob_api: String,
    pub polymarket_host: String,
    pub wallet_private_key: String,
    pub poly_api_key: String,
    pub poly_secret: String,
    pub poly_passphrase: String,
    pub smtp_host: String,
    pub smtp_port: u16,
    pub smtp_user: String,
    pub smtp_pass: String,
    pub alert_from: String,
    pub alert_to: String,
    pub telegram_bot_token: String,
    pub telegram_chat_id: String,
    pub db_path: String,
    pub paper_trading: bool,
    pub simulate_ai: bool,
    // v0.3 Genetic algorithm fields
    pub gemini_api_key: String,
    pub screen_model: String,  // haiku, sonnet, gemini, simulated
    pub deep_model: String,    // haiku, sonnet, gemini, cached
    pub min_confidence: Decimal,
    pub category_filter: String, // "all", "crypto", "weather", "sports", "politics", "no_politics", etc.
    pub exit_tp_pct: Decimal,    // take-profit threshold (0 = disabled)
    pub exit_sl_pct: Decimal,    // stop-loss threshold (0 = disabled)
    pub price_check_secs: u64,   // fast price-check interval between full cycles (0 = disabled)
    pub generation: u32,         // genetic algorithm generation number
    pub knowledge_only: bool,    // skip trade execution, collect analysis only
    pub balance_reserve_pct: Decimal, // keep this % of initial balance as untouchable reserve
    // v1.0 Team fields
    pub max_candidates: usize,     // Scout output limit (default 10)
    pub max_deep_analysis: usize,  // Bull/Bear/Devil treatment limit (default 5)
    // v2.0 Paper trading fields
    pub max_open_positions: usize, // max concurrent open positions (default 8)
    pub report_interval_hours: u64, // periodic email report interval (default 12)
    pub max_spread: Decimal,       // max acceptable bid-ask spread (default 0.05)
    // Paper Trading Plus — Realistic Simulation
    pub sim_fees_enabled: bool,
    pub sim_slippage_enabled: bool,
    pub sim_fills_enabled: bool,
    pub sim_impact_enabled: bool,
    pub sim_gas_fee_min: Decimal,
    pub sim_gas_fee_max: Decimal,
    pub sim_platform_fee_pct: Decimal,
    pub sim_maker_fee_pct: Decimal,
    pub sim_taker_fee_pct: Decimal,
    pub sim_base_slippage_pct: Decimal,
    pub sim_size_penalty_pct: Decimal,
    pub sim_size_penalty_threshold: Decimal,
    pub sim_reject_probability: Decimal,
    pub sim_partial_fill_probability: Decimal,
    pub sim_min_liquidity_volume: Decimal,
    pub sim_impact_threshold: Decimal,
    pub sim_impact_per_dollar_pct: Decimal,
}

impl Config {
    /// Load config from a specific .env file, or the default `.env` if None.
    pub fn from_env_file(path: Option<&str>) -> Result<Self> {
        match path {
            Some(p) => { dotenvy::from_filename(p).ok(); }
            None => { dotenvy::dotenv().ok(); }
        }
        Self::build_from_env()
    }

    pub fn from_env() -> Result<Self> {
        dotenvy::dotenv().ok();
        Self::build_from_env()
    }

    fn build_from_env() -> Result<Self> {
        let paper_trading = env("PAPER_TRADING", "true") == "true";
        let wallet_private_key = env("WALLET_PRIVATE_KEY", "");

        // Safety check: refuse to start live trading without a wallet key
        if !paper_trading && wallet_private_key.is_empty() {
            panic!("WALLET_PRIVATE_KEY must be set for live trading (PAPER_TRADING=false)");
        }

        Ok(Self {
            claude_api_key: env("CLAUDE_API_KEY", ""),
            claude_model_haiku: env("CLAUDE_MODEL_HAIKU", "claude-haiku-4-5-20251001"),
            claude_model_sonnet: env("CLAUDE_MODEL_SONNET", "claude-sonnet-4-5-20250929"),
            initial_balance: env_decimal("INITIAL_BALANCE", "30.00")?,
            max_position_pct: env_decimal("MAX_POSITION_PCT", "0.08")?,
            min_edge_threshold: env_decimal("MIN_EDGE_THRESHOLD", "0.08")?,
            kill_threshold: env_decimal("KILL_THRESHOLD", "15.00")?,
            kelly_fraction: env_decimal("KELLY_FRACTION", "0.40")?,
            scan_interval_secs: env("SCAN_INTERVAL_SECS", "1800").parse().unwrap_or(1800),
            max_markets_to_scan: env("MAX_MARKETS_SCAN", "700").parse().unwrap_or(700),
            max_haiku_per_cycle: env("MAX_HAIKU_PER_CYCLE", "30").parse().unwrap_or(30),
            max_sonnet_per_cycle: env("MAX_SONNET_PER_CYCLE", "5").parse().unwrap_or(5),
            gamma_api_base: env("GAMMA_API", "https://gamma-api.polymarket.com"),
            polymarket_clob_api: env("POLYMARKET_CLOB_API", "https://clob.polymarket.com"),
            polymarket_host: env("POLYMARKET_HOST", "https://polymarket.com"),
            wallet_private_key,
            poly_api_key: env("POLY_API_KEY", ""),
            poly_secret: env("POLY_SECRET", ""),
            poly_passphrase: env("POLY_PASSPHRASE", ""),
            smtp_host: env("SMTP_HOST", "smtp.gmail.com"),
            smtp_port: env("SMTP_PORT", "587").parse().unwrap_or(587),
            smtp_user: env("SMTP_USER", ""),
            smtp_pass: env("SMTP_PASS", ""),
            alert_from: env("ALERT_FROM", ""),
            alert_to: env("ALERT_TO", ""),
            telegram_bot_token: env("TELEGRAM_BOT_TOKEN", ""),
            telegram_chat_id: env("TELEGRAM_CHAT_ID", ""),
            db_path: env("DB_PATH", "agent.db"),
            paper_trading,
            simulate_ai: env("SIMULATE_AI", "false") == "true",
            // v0.3 Genetic algorithm fields
            gemini_api_key: env("GEMINI_API_KEY", ""),
            screen_model: env("SCREEN_MODEL", "gemini"),
            deep_model: env("DEEP_MODEL", "gemini"),
            min_confidence: env_decimal("MIN_CONFIDENCE", "0.60")?,
            category_filter: env("CATEGORY_FILTER", "all"),
            exit_tp_pct: env_decimal("EXIT_TP_PCT", "0")?,
            exit_sl_pct: env_decimal("EXIT_SL_PCT", "0")?,
            price_check_secs: env("PRICE_CHECK_SECS", "90").parse().unwrap_or(90),
            generation: env("GENERATION", "1").parse().unwrap_or(1),
            knowledge_only: env("KNOWLEDGE_ONLY", "false") == "true",
            balance_reserve_pct: env_decimal("BALANCE_RESERVE_PCT", "0.10")?,
            // v1.0 Team fields
            max_candidates: env("MAX_CANDIDATES", "20").parse().unwrap_or(20),
            max_deep_analysis: env("MAX_DEEP_ANALYSIS", "10").parse().unwrap_or(10),
            // v2.0 Paper trading fields
            max_open_positions: env("MAX_OPEN_POSITIONS", "8").parse().unwrap_or(8),
            report_interval_hours: env("REPORT_INTERVAL_HOURS", "12").parse().unwrap_or(12),
            max_spread: env_decimal("MAX_SPREAD", "0.05")?,
            // Paper Trading Plus — Realistic Simulation
            sim_fees_enabled: env("SIM_FEES_ENABLED", "true") == "true",
            sim_slippage_enabled: env("SIM_SLIPPAGE_ENABLED", "true") == "true",
            sim_fills_enabled: env("SIM_FILLS_ENABLED", "true") == "true",
            sim_impact_enabled: env("SIM_IMPACT_ENABLED", "true") == "true",
            sim_gas_fee_min: env_decimal("SIM_GAS_FEE_MIN", "0.01")?,
            sim_gas_fee_max: env_decimal("SIM_GAS_FEE_MAX", "0.05")?,
            sim_platform_fee_pct: env_decimal("SIM_PLATFORM_FEE_PCT", "0.02")?,
            sim_maker_fee_pct: env_decimal("SIM_MAKER_FEE_PCT", "0.00")?,
            sim_taker_fee_pct: env_decimal("SIM_TAKER_FEE_PCT", "0.00")?,
            sim_base_slippage_pct: env_decimal("SIM_BASE_SLIPPAGE_PCT", "0.001")?,
            sim_size_penalty_pct: env_decimal("SIM_SIZE_PENALTY_PCT", "0.005")?,
            sim_size_penalty_threshold: env_decimal("SIM_SIZE_PENALTY_THRESHOLD", "1.00")?,
            sim_reject_probability: env_decimal("SIM_REJECT_PROBABILITY", "0.05")?,
            sim_partial_fill_probability: env_decimal("SIM_PARTIAL_FILL_PROBABILITY", "0.15")?,
            sim_min_liquidity_volume: env_decimal("SIM_MIN_LIQUIDITY_VOLUME", "10.00")?,
            sim_impact_threshold: env_decimal("SIM_IMPACT_THRESHOLD", "2.00")?,
            sim_impact_per_dollar_pct: env_decimal("SIM_IMPACT_PER_DOLLAR_PCT", "0.003")?,
        })
    }
}

fn env(key: &str, default: &str) -> String {
    std::env::var(key).unwrap_or_else(|_| default.to_string())
}

fn env_decimal(key: &str, default: &str) -> Result<Decimal> {
    let val = env(key, default);
    Decimal::from_str(&val).with_context(|| format!("Invalid decimal for {key}: {val}"))
}
