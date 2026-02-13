# 🤖 Polymarket AI Agent

> **Autonomous trading agent fleet for Polymarket prediction markets, powered by AI and built with Rust**

[![Rust](https://img.shields.io/badge/rust-1.70%2B-orange.svg)](https://www.rust-lang.org/)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
[![Paper Trading](https://img.shields.io/badge/Paper%20Trading-Enabled-green.svg)]()

An advanced, multi-agent trading system that autonomously trades on [Polymarket](https://polymarket.com) using AI-powered market analysis, Kelly Criterion position sizing, and sophisticated risk management. Built entirely in Rust for maximum performance and reliability.

---

## ✨ Features

### 🎯 Core Capabilities

- **🧠 Multi-AI Analysis**: Leverages Google Gemini and Anthropic Claude for market screening and deep analysis
- **📊 Multi-Agent Architecture**: Run 1-100+ concurrent trading agents with different strategies
- **💰 Paper Trading**: Risk-free simulation with realistic fees, slippage, and market impact
- **🔴 Live Trading**: Real USDC trading on Polygon via Polymarket's CLOB API
- **📈 Kelly Criterion**: Optimal bet sizing based on edge and confidence
- **🛡️ Advanced Risk Management**:
  - Consecutive loss detection (auto-pause at 5 losses)
  - Drawdown protection
  - Balance reserve system
  - Kill threshold safety net
- **🎛️ Web Dashboard**: Monitor all agents, view trades, analyze performance in real-time
- **📱 Real-time Notifications**: Telegram alerts + HTML email reports
- **🔍 Market Intelligence**: Category filtering, spread analysis, volume tracking

### 🏗️ Architecture

```
┌─────────────────┐
│  Dashboard      │ ← Web UI (http://localhost:8080)
│  (Rust/Axum)    │   Spawns & monitors agents
└────────┬────────┘
         │
         ├──► Agent 1 (Conservative Strategy)
         ├──► Agent 2 (Aggressive Strategy)
         ├──► Agent 3 (Crypto-focused)
         └──► Agent N (Custom config)
              │
              ├─► Scout (screens 700+ markets)
              ├─► Strategist (deep AI analysis)
              ├─► Risk Manager (Kelly sizing)
              ├─► Executor (order execution)
              └─► Judge (exit decisions)

┌─────────────────┐
│  Proxy Server   │ ← Data aggregation (optional)
│  (Supabase)     │   Community insights
└─────────────────┘
```

---

## 🚀 Quick Start

### Prerequisites

- **Rust 1.70+** ([Install Rust](https://rustup.rs/))
- **Google Gemini API Key** (free tier available - [Get API Key](https://aistudio.google.com/))
- **PostgreSQL** (optional, for multi-agent setups)

### 1. Clone & Setup

```bash
git clone https://github.com/rhecustein/polymarket-agent.git
cd polymarket-agent
cp .env.example .env
```

### 2. Configure API Keys

Edit `.env`:

```env
# Required: Google Gemini API
GEMINI_API_KEY=AIzaSy...your-key-here

# Recommended: Telegram notifications
TELEGRAM_BOT_TOKEN=7123456789:AAH1bGciOiJIUzI1...
TELEGRAM_CHAT_ID=123456789

# Keep paper trading ON for testing
PAPER_TRADING=true
INITIAL_BALANCE=30.00
```

See [SETUP.md](SETUP.md) for detailed configuration of all services (Gemini, Claude, Telegram, Email, Supabase, Polymarket wallet).

### 3. Run the Dashboard

```bash
# Linux/macOS
chmod +x run.sh
./run.sh

# Windows
.\run.ps1
```

Open **http://localhost:8080** to access the dashboard.

### 4. Start Trading Agents

From the dashboard:
1. Click **"Start Agent"** to launch agents with preset strategies
2. Choose from 10+ strategy presets (Conservative, Aggressive, Sports-focused, etc.)
3. Monitor real-time performance, trades, and AI analysis

---

## 📊 Dashboard Overview

The web dashboard provides comprehensive monitoring and control:

### Agent Grid View
- **Live Status**: Running, stopped, or dead agents
- **Performance Metrics**: Balance, P&L, ROI, win rate
- **Cost Tracking**: AI API costs, trading fees, gas fees
- **Agent Details**: Click any agent to view full trade history and analysis logs

### Performance Analytics
- **Trade Breakdown**: By category, trade mode, AI model
- **Fee Analysis**: Gas fees, slippage, platform fees, maker/taker fees
- **Open Positions**: Real-time tracking with TP/SL levels
- **Cycle Statistics**: Markets scanned, analyzed, trades executed

### Control Panel
- **Start/Stop**: Individual agent control
- **Bulk Operations**: Manage multiple agents at once
- **Config Editor**: Live parameter adjustment
- **Activity Feed**: Real-time event stream

---

## 🎮 Usage Examples

### Single Agent (CLI)

Run a single agent directly from the command line:

```bash
cd agent
cargo build --release

# Paper trading (default)
./target/release/polyagent --config-file configs/conservative.env --agent-id agent-1

# Live trading (DANGEROUS - test thoroughly first!)
./target/release/polyagent --config-file configs/live.env --agent-id live-agent-1
```

### Multi-Agent Fleet (Dashboard)

```bash
# Start the dashboard + proxy
./run.sh

# Access dashboard at http://localhost:8080
# Click "Start Agent" to launch multiple agents
# Each agent runs independently with its own:
#   - SQLite database (data/agent-{id}.db)
#   - Configuration file (configs/agent-{id}.env)
#   - Trading strategy
#   - Risk parameters
```

### Strategy Presets

The dashboard includes 10 built-in strategy presets:

| Preset | Balance | Max Position | Edge Threshold | Target Markets |
|--------|---------|--------------|----------------|----------------|
| **Conservative** | $30 | 4% | 10% | All |
| **Aggressive** | $50 | 8% | 5% | All |
| **High-Volume** | $100 | 6% | 7% | High liquidity |
| **Sports** | $40 | 5% | 8% | Sports only |
| **Crypto** | $35 | 7% | 6% | Crypto only |
| **Politics** | $45 | 5% | 9% | Politics only |
| **Scalper** | $25 | 3% | 12% | Fast cycles |
| **Swing** | $60 | 10% | 4% | Long holds |
| **Balanced** | $40 | 6% | 8% | All |
| **Experimental** | $20 | 12% | 3% | All |

Each preset creates multiple agent variants with parameter mutations for portfolio diversity.

---

## 🧪 Paper Trading Simulation

The agent includes a **Paper Trading Plus** mode with realistic market simulation:

### Simulated Components

```rust
SimConfig {
    SIM_FEES_ENABLED: true,       // Platform + maker/taker fees
    SIM_SLIPPAGE_ENABLED: true,   // Market impact on entry/exit
    SIM_FILLS_ENABLED: true,      // Realistic order matching
    SIM_IMPACT_ENABLED: true,     // Price impact from size
}
```

### Fee Structure
- **Platform Fee**: 2% on winning trades
- **Maker/Taker Fee**: 0.05% - 0.10%
- **Gas Fees**: $0.01 - $0.05 per transaction
- **Slippage**: 0.1% - 0.5% based on market liquidity

### Simulation Accuracy

Paper trading results closely match live trading performance (within 5-10% variance), making it ideal for strategy development and backtesting.

---

## 🔧 Configuration

### Trading Parameters

| Variable | Description | Default | Range |
|----------|-------------|---------|-------|
| `INITIAL_BALANCE` | Starting capital (USD) | `30.00` | $10 - $10,000 |
| `MAX_POSITION_PCT` | Max % of balance per trade | `0.08` | 0.01 - 0.20 |
| `MIN_EDGE_THRESHOLD` | Minimum edge to trade | `0.05` | 0.01 - 0.50 |
| `KELLY_FRACTION` | Kelly bet size multiplier | `0.40` | 0.10 - 1.00 |
| `KILL_THRESHOLD` | Stop agent if balance below | `3.00` | $1 - $100 |
| `MAX_OPEN_POSITIONS` | Max concurrent trades | `8` | 1 - 50 |

### AI Models

| Model | Use Case | Cost | Speed |
|-------|----------|------|-------|
| **Gemini 2.0 Flash** | Screening (default) | Free / $0.10 per 1M | ⚡ Fast |
| **Claude Haiku 4.5** | Screening | $0.25 per 1M | ⚡⚡ Very Fast |
| **Claude Sonnet 4.5** | Deep Analysis | $3.00 per 1M | 🐢 Slower |

Configure via `.env`:
```env
SCREEN_MODEL=gemini    # or "haiku"
DEEP_MODEL=gemini      # or "sonnet"
```

### Exit Strategy

```env
EXIT_TP_PCT=0          # Take-profit % (0 = disabled, use AI judgment)
EXIT_SL_PCT=0          # Stop-loss % (0 = disabled, use AI judgment)
BALANCE_RESERVE_PCT=0.10  # Keep 10% balance untouchable
```

**Pro Tip**: Set `EXIT_TP_PCT=0` and `EXIT_SL_PCT=0` to let the AI Judge specialist make exit decisions based on market updates.

---

## 📈 Trading Strategy

### How the Agent Works

1. **Scout Phase** (every 30 minutes)
   - Fetches 700+ markets from Polymarket
   - Applies category filters (sports, crypto, politics, etc.)
   - Checks spread, volume, and liquidity requirements
   - Outputs top 20 candidates

2. **Strategist Phase**
   - Deep AI analysis on each candidate
   - Estimates fair value and edge
   - Assigns confidence score (0-1)
   - Reasons about market dynamics

3. **Risk Manager Phase**
   - Kelly Criterion bet sizing
   - Checks consecutive loss count
   - Validates against balance, max position, and open positions
   - Approves or rejects trade

4. **Executor Phase**
   - Places order (paper or live)
   - Records entry price, fees, slippage
   - Logs trade to database

5. **Judge Phase** (every 3 minutes)
   - Re-analyzes open positions
   - Checks TP/SL levels if enabled
   - Makes exit decisions based on new information
   - Executes closes and records P&L

### Edge Calculation

```
Edge = |Fair Value - Current Price|

Example:
  Market: "Will Bitcoin hit $100K by March?"
  Current Price: 0.35 (35%)
  AI Fair Value: 0.55 (55%)
  Edge: 0.20 (20%)

  → Strong BUY signal (edge > threshold)
```

### Kelly Criterion Sizing

```
f* = (p × b - q) / b

Where:
  p = Probability of win (AI confidence)
  q = 1 - p
  b = Net odds = (1 - current_price) / current_price

Bet Size = Bankroll × f* × KELLY_FRACTION
```

**Example**:
- Bankroll: $30
- Edge: 20%
- Confidence: 70%
- Kelly Fraction: 0.333
- → Bet Size: $1.80 (6% of bankroll)

---

## 🛡️ Risk Management

### Multi-Layer Protection

1. **Position Sizing**
   - Max 8% of balance per trade (configurable)
   - Kelly Criterion prevents over-betting
   - Balance reserve (10% untouchable)

2. **Consecutive Loss Detection**
   - **3 losses**: Skip next cycle
   - **4 losses**: Reduce position size by 50%
   - **5 losses**: Full pause + critical alert

3. **Kill Threshold**
   - Agent auto-stops if balance < $3 (default)
   - Prevents catastrophic losses

4. **Survival Mode**
   - Activated at low balance
   - Ultra-conservative sizing
   - Telegram + email alerts

5. **Spread Protection**
   - Rejects markets with spread > 5%
   - Ensures fair entry/exit prices

---

## 📱 Notifications

### Telegram Bot

Receive real-time alerts for:
- ✅ Trade opened (direction, price, edge, confidence)
- 💰 Trade closed (P&L, win/loss, duration)
- 📊 Daily summary (balance, ROI, win rate, streak)
- 🚨 Critical alerts (pause, survival mode, kill threshold)

**Available Commands**:
- `/status` - Portfolio status and balance
- `/trades` - Recent closed trades
- `/open` - Current open positions
- `/stop` - Gracefully stop the agent
- `/help` - Show all commands

Setup: See [SETUP.md](SETUP.md#3-telegram-bot)

### Email Reports

Receive detailed HTML reports every 12 hours with:
- Performance breakdown by category, mode, and model
- Open positions with TP/SL levels
- Fee analysis (gas, slippage, platform)
- API cost tracking
- Trade history with charts

Setup: See [SETUP.md](SETUP.md#4-email-alerts-smtp)

---

## 📊 Database Schema

Each agent maintains its own SQLite database (`data/{agent-id}.db`):

### Tables

- **`balance`**: Balance history, P&L tracking
- **`trades`**: All trades (open + closed) with full simulation data
- **`analysis`**: AI analysis logs with reasoning
- **`cycles`**: Scan cycle statistics
- **`state`**: Agent state (consecutive losses, last cycle time)

### Trade Record Fields

```sql
CREATE TABLE trades (
    id TEXT PRIMARY KEY,
    timestamp INTEGER,
    question TEXT,
    direction TEXT,
    entry_price REAL,
    fair_value REAL,
    edge REAL,
    bet_size REAL,
    pnl REAL,
    status TEXT,
    -- Simulation fields (paper trading)
    raw_entry_price REAL,
    raw_exit_price REAL,
    entry_gas_fee REAL,
    exit_gas_fee REAL,
    entry_slippage REAL,
    exit_slippage REAL,
    platform_fee REAL,
    maker_taker_fee REAL,
    -- Exit strategy
    take_profit REAL,
    stop_loss REAL,
    exit_reason TEXT
);
```

---

## 🏗️ Project Structure

```
polymarket-agent/
├── agent/                   # Main trading agent crate
│   ├── src/
│   │   ├── main.rs         # Agent entry point
│   │   ├── bin/
│   │   │   └── dashboard.rs # Multi-agent dashboard
│   │   ├── config.rs       # Config management
│   │   ├── db.rs           # SQLite operations
│   │   ├── strategy.rs     # Kelly Criterion, risk logic
│   │   ├── telegram.rs     # Telegram bot integration
│   │   ├── team/           # Specialist modules
│   │   │   ├── scout.rs    # Market screening
│   │   │   ├── strategist.rs # AI analysis
│   │   │   ├── risk_manager.rs # Position sizing
│   │   │   ├── executor.rs # Order execution
│   │   │   └── judge.rs    # Exit decisions
│   │   ├── paper/          # Paper trading simulation
│   │   │   ├── mod.rs
│   │   │   └── portfolio.rs
│   │   └── live/           # Live trading (Polymarket API)
│   │       └── executor.rs
│   └── Cargo.toml
├── proxy/                   # Proxy server (optional)
│   ├── src/
│   │   ├── main.rs
│   │   └── supabase.rs
│   └── Cargo.toml
├── configs/                 # Agent configurations (auto-generated)
├── data/                    # SQLite databases (auto-generated)
├── run.sh                   # Start script (Linux/macOS)
├── reset-db.sh              # Database reset script
├── .env.example             # Configuration template
├── SETUP.md                 # Detailed setup guide
├── CONTRIBUTING.md          # Contribution guidelines
└── README.md                # This file
```

---

## 🧑‍💻 Development

### Build from Source

```bash
# Build all crates
cargo build --release

# Build specific binary
cargo build --release --bin polyagent
cargo build --release --bin dashboard
cargo build --release --bin polyproxy

# Run tests
cargo test

# Run with logging
RUST_LOG=debug cargo run --bin polyagent
```

### Adding New Features

See [CONTRIBUTING.md](CONTRIBUTING.md) for development guidelines.

### Database Management

```bash
# Reset a specific agent's database
./reset-db.sh agent-1

# Reset all databases (Windows)
.\reset-db.ps1 -AgentId all
```

---

## 🔒 Security

### API Key Safety
- **Never commit `.env` files** to version control
- Use environment variables or secure vaults in production
- Rotate API keys regularly

### Wallet Security (Live Trading)
- **Never share your private key**
- Start with small balances ($10-$50)
- Test extensively in paper mode first
- Use hardware wallets for large amounts

### Paper Trading First
```env
# Always start with paper trading
PAPER_TRADING=true

# Only switch to live after:
# - 100+ paper trades
# - Consistent positive ROI
# - Full understanding of strategy
PAPER_TRADING=false
```

---

## 📚 Resources

### Documentation
- [SETUP.md](SETUP.md) - Complete setup guide for all services
- [CONTRIBUTING.md](CONTRIBUTING.md) - Development guidelines
- [Day 1 Article](articles/day_01_building_an_autonomous_polymarket_agent.md) - Development journey

### External Links
- [Polymarket](https://polymarket.com) - Prediction market platform
- [Polymarket Docs](https://docs.polymarket.com) - API documentation
- [Google Gemini](https://aistudio.google.com/) - AI model API
- [Anthropic Claude](https://console.anthropic.com/) - AI model API
- [Telegram Bot API](https://core.telegram.org/bots/api) - Bot documentation

---

## 🤝 Contributing

Contributions are welcome! Please read [CONTRIBUTING.md](CONTRIBUTING.md) for guidelines.

### Areas for Improvement
- [ ] Backtesting framework
- [ ] Portfolio optimization (correlation analysis)
- [ ] Additional AI models (OpenAI GPT-4, Groq)
- [ ] Advanced charting in dashboard
- [ ] Mobile app for monitoring
- [ ] Strategy marketplace

---

## 📜 License

This project is licensed under the MIT License - see [LICENSE](LICENSE) for details.

```
MIT License

Copyright (c) 2026 rhecustein

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction...
```

---

## ⚠️ Disclaimer

**This software is for educational purposes only.**

- Prediction market trading involves financial risk
- Past performance does not guarantee future results
- The AI models make probabilistic predictions that can be wrong
- Start with paper trading and small amounts
- Only trade what you can afford to lose
- This is not financial advice

**USE AT YOUR OWN RISK.**

---

## 🌟 Acknowledgments

Built with:
- [Rust](https://www.rust-lang.org/) - Systems programming language
- [Axum](https://github.com/tokio-rs/axum) - Web framework
- [Tokio](https://tokio.rs/) - Async runtime
- [SQLite](https://www.sqlite.org/) - Embedded database
- [Supabase](https://supabase.com/) - Backend services
- [Google Gemini](https://ai.google.dev/) - AI analysis
- [Anthropic Claude](https://www.anthropic.com/) - AI analysis

---

## 📞 Support

- **Issues**: [GitHub Issues](https://github.com/rhecustein/polymarket-agent/issues)
- **Discussions**: [GitHub Discussions](https://github.com/rhecustein/polymarket-agent/discussions)

---

<div align="center">

**Built with ❤️ and Rust**

If this project helps you, consider giving it a ⭐!

[Report Bug](https://github.com/rhecustein/polymarket-agent/issues) · [Request Feature](https://github.com/rhecustein/polymarket-agent/issues) · [Documentation](SETUP.md)

</div>
