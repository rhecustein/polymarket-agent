<p align="center">
  <h1 align="center">Polymarket AI Trading Agent</h1>
  <p align="center"><strong>14 AI agents. One prediction market. Autonomous trading.</strong></p>
</p>

<p align="center">
  <a href="LICENSE"><img src="https://img.shields.io/badge/license-MIT-blue.svg" alt="License: MIT"></a>
  <a href="https://www.rust-lang.org/"><img src="https://img.shields.io/badge/rust-1.75%2B-orange.svg" alt="Rust"></a>
  <a href="docker-compose.yml"><img src="https://img.shields.io/badge/docker-supported-2496ED.svg" alt="Docker"></a>
</p>

---

An autonomous AI trading agent that uses **14 specialized AI agents** organized as a virtual company to trade prediction markets on [Polymarket](https://polymarket.com). Paper trading by default. Supports crypto, weather, sports, and general categories. Community knowledge sharing lets all agents learn from each other.

## Architecture

The agent operates as a **virtual company** with 4 divisions:

```
                        +---------------------------+
                        |        CEO (Main Loop)     |
                        +---------------------------+
                                    |
            +-----------+-----------+-----------+
            |           |           |           |
    +-------v---+ +-----v-----+ +--v--------+ +v----------+
    |INTELLIGENCE| |SPECIALIST | |  DEBATE   | |  C-SUITE  |
    |  DIVISION  | |   DESK    | |   ROOM    | |           |
    +-----------+ +-----------+ +-----------+ +-----------+
    | Scout     | | Crypto    | | Bull      | | Risk Mgr  |
    | Researcher| | Weather   | | Bear      | | Strategist|
    | Data      | | Sports    | | Devil's   | | Executor  |
    | Analyst   | | General   | | Advocate  | | Auditor   |
    +-----------+ +-----------+ +-----------+ +-----------+

    Phase 1:         Phase 2:       Phase 3:      Phase 4:
    Discover &       Category-      Adversarial   Size, Plan,
    Research         Specific       Debate &      Execute &
    Markets          Analysis       Judgment      Learn
```

**Intelligence Division** discovers and researches markets. **Specialist Desks** provide category-specific analysis (crypto price data, weather forecasts, sports stats, news). The **Debate Room** forces adversarial analysis -- Bull builds the YES case, Bear builds the NO case, and Devil's Advocate judges impartially. The **C-Suite** sizes positions with Kelly Criterion, plans trade modes (Scalp/Swing/Conviction), executes, and audits performance.

## Quick Start

```bash
# 1. Clone the repo
git clone https://github.com/bintangworks/polymarket-agent.git
cd polymarket-agent

# 2. Configure environment
cp .env.example .env
# Edit .env with your API keys (at minimum: GEMINI_API_KEY)

# 3. Run in paper trading mode (default)
cargo run --release
```

That's it. The agent will start scanning Polymarket, analyzing opportunities, and paper trading autonomously.

## Features

| Feature | Description |
|---------|-------------|
| **14-Agent Company** | Specialized agents organized into 4 divisions for thorough market analysis |
| **Adversarial Debate** | Bull and Bear analysts argue opposing sides; Devil's Advocate judges impartially |
| **4 Specialist Desks** | Category-specific analysis for crypto, weather, sports, and general markets |
| **Paper Trading** | Safe by default -- no real money until you explicitly enable live trading |
| **Mode-Based Exits** | Scalp (tight stops), Swing (dynamic trailing), Conviction (long hold) |
| **Kelly Criterion Sizing** | Mathematically optimal position sizing with survival protection |
| **Claude Sonnet Judge** | Optional top-tier AI judge for highest-confidence candidates |
| **Community Knowledge** | Anonymous trade data sharing so all agents learn collectively |
| **Telegram Bot** | Real-time commands: `/status`, `/stop`, `/trades`, `/open`, `/help` |
| **Email Reports** | 12-hour periodic portfolio and performance summaries |
| **Web Dashboard** | Local dashboard at `localhost:3000` for monitoring |
| **Graceful Shutdown** | Create a `STOP` file to cleanly exit after the current cycle |
| **Docker Support** | One-command deployment with `docker-compose up` |
| **SQLite Persistence** | All state persisted locally -- survives restarts |

## Configuration

Key environment variables (see `.env.example` for full list):

```bash
# Required
GEMINI_API_KEY=your_gemini_api_key

# Optional - for live trading
POLYMARKET_API_KEY=your_api_key
POLYMARKET_SECRET=your_secret
POLYMARKET_PASSPHRASE=your_passphrase

# Optional - for notifications
TELEGRAM_BOT_TOKEN=your_bot_token
TELEGRAM_CHAT_ID=your_chat_id
SMTP_HOST=smtp.gmail.com
SMTP_USERNAME=you@gmail.com
SMTP_PASSWORD=your_app_password
ALERT_EMAIL=you@gmail.com

# Trading parameters
PAPER_TRADING=true
INITIAL_BALANCE=30.0
MAX_CANDIDATES=10
MAX_DEEP_ANALYSIS=5
SCAN_INTERVAL_SECS=3600
```

## Documentation

- [Architecture Deep Dive](docs/ARCHITECTURE.md)
- [Setup Guide](docs/SETUP_GUIDE.md)
- [Paper Trading](docs/PAPER_TRADING.md)
- [Live Trading](docs/LIVE_TRADING.md)
- [Knowledge System](docs/KNOWLEDGE_SYSTEM.md)
- [Proxy API Reference](docs/API_REFERENCE.md)
- [FAQ](docs/FAQ.md)

## Community

This project thrives on community participation:

- **Share your results** -- Open a [Trade Result](https://github.com/bintangworks/polymarket-agent/issues/new?template=trade_result.md) issue
- **Report bugs** -- Use the [Bug Report](https://github.com/bintangworks/polymarket-agent/issues/new?template=bug_report.md) template
- **Request features** -- Use the [Feature Request](https://github.com/bintangworks/polymarket-agent/issues/new?template=feature_request.md) template
- **Contribute code** -- See [CONTRIBUTING.md](CONTRIBUTING.md)
- **Knowledge sharing** -- Enable `KNOWLEDGE_UPLOAD=true` to anonymously contribute trade insights

## Support

If this project is useful to you, consider supporting development:

- [Ko-fi](https://ko-fi.com/bintangworks)
- [Saweria](https://saweria.co/bintangworks)
- BTC: `bc1qxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx`
- ETH: `0xXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXX`

## License

[MIT](LICENSE) -- Copyright (c) 2026 bintangworks

---

<p align="center">
  Built with Rust. Powered by Gemini & Claude. Trades on Polymarket.
</p>
