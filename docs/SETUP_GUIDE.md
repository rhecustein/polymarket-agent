# Setup Guide

Get the Polymarket Agent running in under 10 minutes.

## Prerequisites

### Required
- **Rust 1.75+**: Install from [rustup.rs](https://rustup.rs/)
- **Gemini API Key**: Get one free at [ai.google.dev](https://ai.google.dev/). Free tier works fine; paid tier ($0.10/1M input tokens) removes rate limits.

### Optional
- **Claude API Key**: For higher-quality adversarial analysis on top candidates. Get one at [console.anthropic.com](https://console.anthropic.com/). Without it, all analysis uses Gemini.
- **Telegram Bot**: For mobile monitoring and commands. Create a bot via [@BotFather](https://t.me/BotFather).
- **Gmail SMTP**: For email trade reports. Requires an [App Password](https://myaccount.google.com/apppasswords).
- **Docker**: For containerized deployment (alternative to native Rust build).

## Step 1: Clone and Configure

```bash
git clone https://github.com/yourorg/polymarket-agent.git
cd polymarket-agent
make setup
```

This creates `agent/.env` from the example template. Open it and fill in your keys:

```bash
# Required
GEMINI_API_KEY=your_gemini_key_here

# Optional - Claude for premium analysis
CLAUDE_API_KEY=                # Leave empty to use Gemini only

# Optional - Telegram alerts
TELEGRAM_BOT_TOKEN=            # From @BotFather
TELEGRAM_CHAT_ID=              # Your chat ID (message @userinfobot)

# Optional - Email reports
SMTP_FROM=you@gmail.com
SMTP_PASSWORD=                 # Gmail App Password
SMTP_TO=you@gmail.com

# Trading parameters (defaults are conservative)
TRADING_MODE=paper             # paper or live
INITIAL_BALANCE=30.00          # Starting virtual balance
SCAN_INTERVAL_SECS=3600        # How often to scan (seconds)
MAX_CANDIDATES=10              # Markets to evaluate per cycle
MAX_DEEP_ANALYSIS=5            # Markets for full debate pipeline
```

## Step 2: Build

```bash
make build
```

This compiles both the agent and the knowledge-sharing proxy in release mode. First build takes 2-5 minutes depending on your machine.

## Step 3: Run (Paper Trading)

```bash
make run-paper
```

The agent will:
1. Scan Polymarket for active markets
2. Filter and rank candidates
3. Run the full 14-agent analysis pipeline
4. Execute virtual trades
5. Wait for `SCAN_INTERVAL_SECS` and repeat

## Step 4: Verify It Works

Watch the console output for these indicators of a healthy start:

```
[INFO] Polymarket Agent v1.0 starting...
[INFO] Mode: paper | Balance: $30.00
[INFO] Scout found 247 active markets
[INFO] Filtered to 10 candidates
[INFO] Running analysis pipeline...
```

If you see `Scout found X markets` with X > 0, the agent is working correctly.

## Docker Alternative

If you prefer Docker over a native Rust build:

```bash
docker-compose up -d --build
```

This starts both the agent and the knowledge proxy. View logs with:

```bash
docker-compose logs -f agent
```

## Troubleshooting

**"GEMINI_API_KEY not set"**: Make sure your `.env` file is in the `agent/` directory and contains a valid key.

**"Failed to fetch markets"**: Polymarket's API may be temporarily unavailable. The agent will retry automatically on the next cycle.

**"Rate limited"**: If using the free Gemini tier, you may hit rate limits with `MAX_DEEP_ANALYSIS > 3`. Either upgrade to paid tier or reduce the value.

**Build fails on Windows**: Ensure you have the Visual Studio C++ Build Tools installed. Run `rustup show` to verify your toolchain.

**Build fails on Linux**: Install OpenSSL dev headers: `sudo apt install libssl-dev pkg-config`.
