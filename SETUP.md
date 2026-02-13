# Polymarket AI Agent — Platform Setup Guide

Step-by-step instructions for configuring all external services required by the agent.

Copy `.env.example` to `.env` before starting:

```bash
cp .env.example .env
```

---

## Table of Contents

1. [Google Gemini API](#1-google-gemini-api) (Required)
2. [Anthropic Claude API](#2-anthropic-claude-api) (Optional)
3. [Telegram Bot](#3-telegram-bot) (Recommended)
4. [Email Alerts (SMTP)](#4-email-alerts-smtp) (Optional)
5. [Supabase](#5-supabase) (Required for Proxy)
6. [Polymarket Wallet](#6-polymarket-wallet) (Live Trading Only)
7. [Environment Variables Reference](#7-environment-variables-reference)

---

## 1. Google Gemini API

**Purpose:** Primary AI model for market screening and deep analysis. Gemini is the default and recommended model — it's fast and cost-effective.

### Steps

1. Go to [Google AI Studio](https://aistudio.google.com/)
2. Sign in with your Google account
3. Click **"Get API Key"** in the left sidebar
4. Click **"Create API Key"**
5. Select a Google Cloud project (or create a new one)
6. Copy the generated API key

### .env Configuration

```env
GEMINI_API_KEY=AIzaSy...your-key-here
SCREEN_MODEL=gemini
DEEP_MODEL=gemini
```

### Pricing

- Gemini 2.0 Flash: Free tier available (15 RPM, 1M tokens/day)
- Paid tier: $0.10 per 1M input tokens, $0.40 per 1M output tokens
- For most users, the free tier is sufficient for paper trading

### Verify

Test your key with curl:

```bash
curl "https://generativelanguage.googleapis.com/v1beta/models?key=YOUR_API_KEY"
```

A successful response returns a list of available models.

---

## 2. Anthropic Claude API

**Purpose:** Alternative AI model. Claude Haiku is used for fast market screening, Claude Sonnet for deep analysis. You can use Claude instead of or alongside Gemini.

### Steps

1. Go to [Anthropic Console](https://console.anthropic.com/)
2. Sign up or log in
3. Navigate to **Settings > API Keys**
4. Click **"Create Key"**
5. Name your key (e.g., `polymarket-agent`)
6. Copy the key (starts with `sk-ant-api03-`)

### .env Configuration

```env
CLAUDE_API_KEY=sk-ant-api03-xxxxx
CLAUDE_MODEL_HAIKU=claude-haiku-4-5-20251001
CLAUDE_MODEL_SONNET=claude-sonnet-4-5-20250929
```

To use Claude as the primary model instead of Gemini:

```env
SCREEN_MODEL=haiku
DEEP_MODEL=sonnet
```

### Pricing

- Haiku: $0.25 / 1M input, $1.25 / 1M output
- Sonnet: $3.00 / 1M input, $15.00 / 1M output
- Requires adding credit balance in the Anthropic Console

### Verify

```bash
curl https://api.anthropic.com/v1/messages \
  -H "x-api-key: YOUR_API_KEY" \
  -H "anthropic-version: 2023-06-01" \
  -H "content-type: application/json" \
  -d '{"model":"claude-haiku-4-5-20251001","max_tokens":10,"messages":[{"role":"user","content":"Hi"}]}'
```

---

## 3. Telegram Bot

**Purpose:** Real-time notifications for trade opens/closes, daily summaries, portfolio status, and remote commands (`/status`, `/stop`, `/trades`, `/open`, `/help`).

### Step 1: Create a Bot

1. Open Telegram and search for **@BotFather**
2. Send `/newbot`
3. Choose a name (e.g., `Polymarket Trading Bot`)
4. Choose a username (must end in `bot`, e.g., `my_polyagent_bot`)
5. BotFather will reply with your **bot token** — copy it

```
Use this token to access the HTTP API:
7123456789:AAH1bGciOiJIUzI1NiIsInR5cCI6Ikp...
```

### Step 2: Get Your Chat ID

1. Start a conversation with your new bot (search its username and click **Start**)
2. Send any message to the bot (e.g., "hello")
3. Open this URL in your browser (replace `YOUR_BOT_TOKEN`):

```
https://api.telegram.org/botYOUR_BOT_TOKEN/getUpdates
```

4. Find `"chat":{"id":123456789}` in the response — that number is your **chat ID**

### Step 3: (Optional) Set Bot Commands

Send this to @BotFather:

```
/setcommands
```

Select your bot, then paste:

```
status - Show portfolio status and balance
trades - Show recent closed trades
open - Show open positions
stop - Gracefully stop the agent
help - Show available commands
```

### .env Configuration

```env
TELEGRAM_BOT_TOKEN=7123456789:AAH1bGciOiJIUzI1NiIsInR5cCI6Ikp...
TELEGRAM_CHAT_ID=123456789
```

### What You'll Receive

| Event | Message |
|-------|---------|
| Trade opened | Direction, price, size, edge, confidence, specialist desk |
| Trade closed | Win/Loss, P&L, hold duration, exit reason |
| Daily summary | Balance, ROI, win rate, drawdown, streak |
| Critical alert | Survival mode, agent paused, low balance |

### Verify

```bash
curl "https://api.telegram.org/botYOUR_BOT_TOKEN/sendMessage" \
  -d "chat_id=YOUR_CHAT_ID" \
  -d "text=Test message from Polymarket Agent"
```

---

## 4. Email Alerts (SMTP)

**Purpose:** Periodic HTML reports with detailed stats, trade breakdowns by category/mode/model, portfolio visualization, and critical alerts.

### Option A: Gmail

1. Go to [Google Account Security](https://myaccount.google.com/security)
2. Enable **2-Step Verification** (required)
3. Go to [App Passwords](https://myaccount.google.com/apppasswords)
4. Select **Mail** and your device
5. Click **Generate** — copy the 16-character app password

```env
SMTP_HOST=smtp.gmail.com
SMTP_PORT=587
SMTP_USER=youremail@gmail.com
SMTP_PASS=abcd efgh ijkl mnop
ALERT_FROM=Polyagent <youremail@gmail.com>
ALERT_TO=youremail@gmail.com
```

### Option B: Hostinger

```env
SMTP_HOST=smtp.hostinger.com
SMTP_PORT=587
SMTP_USER=alert@yourdomain.com
SMTP_PASS=your-email-password
ALERT_FROM=Polyagent <alert@yourdomain.com>
ALERT_TO=youremail@gmail.com
```

### Option C: Other Providers

| Provider | SMTP Host | Port |
|----------|-----------|------|
| Gmail | smtp.gmail.com | 587 |
| Outlook | smtp-mail.outlook.com | 587 |
| Yahoo | smtp.mail.yahoo.com | 587 |
| Hostinger | smtp.hostinger.com | 587 |
| Zoho | smtp.zoho.com | 587 |
| SendGrid | smtp.sendgrid.net | 587 |

### Reports You'll Receive

- **Periodic report** (every 12h by default): Full HTML email with balance, P&L, trades by category/mode/model, open positions
- **Critical alerts**: Agent paused, survival mode activated, kill threshold hit
- Configure report interval with `REPORT_INTERVAL_HOURS` (default: 12)

---

## 5. Supabase

**Purpose:** Backend database for the proxy server. Stores shared knowledge contributions, aggregated insights, and community stats. Required only if running the proxy.

### Steps

1. Go to [Supabase Dashboard](https://supabase.com/dashboard)
2. Sign up or log in (GitHub login works)
3. Click **"New Project"**
4. Choose an organization and fill in:
   - **Name:** `polymarket-agent` (or any name)
   - **Database Password:** choose a strong password (save it)
   - **Region:** choose the closest to you
5. Wait for the project to finish setting up (~2 minutes)
6. Go to **Settings > API** in the left sidebar
7. Copy:
   - **Project URL** (e.g., `https://abcdefgh.supabase.co`)
   - **service_role key** (under "Project API keys" — use the `service_role` key, not `anon`)

### .env Configuration (proxy/.env)

```env
SUPABASE_URL=https://abcdefgh.supabase.co
SUPABASE_SERVICE_KEY=eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9...
HMAC_SECRET=generate-a-random-64-char-hex-string
PORT=3000
```

Generate an HMAC secret:

```bash
openssl rand -hex 32
```

Or use any random 64-character hex string.

### Database Tables

The proxy will auto-create tables via Supabase's API. No manual SQL setup is needed.

---

## 6. Polymarket Wallet

**Purpose:** Required only for **live trading** (real money). Paper trading mode does not need a wallet.

> **WARNING:** Live trading involves real money. Start with paper trading (`PAPER_TRADING=true`) to validate your strategy before risking real funds.

### Steps (for live trading only)

1. Create a Polygon (MATIC) wallet:
   - Use [MetaMask](https://metamask.io/) or any Ethereum-compatible wallet
   - Switch to **Polygon network**
   - Fund with USDC on Polygon

2. Export your private key:
   - MetaMask: Account menu > Account Details > Export Private Key
   - **Never share your private key with anyone**

3. Get Polymarket CLOB API credentials:
   - Visit [Polymarket](https://polymarket.com)
   - Connect your wallet
   - The API credentials are derived from your wallet connection

### .env Configuration

```env
# Only set these for live trading (PAPER_TRADING=false)
WALLET_PRIVATE_KEY=0xabc123...your-private-key
POLY_API_KEY=your-clob-api-key
POLY_SECRET=your-clob-secret
POLY_PASSPHRASE=your-clob-passphrase

# SAFETY: Keep paper trading ON until you're confident
PAPER_TRADING=true
```

### Safety Notes

- The agent refuses to start in live mode without `WALLET_PRIVATE_KEY`
- Always test with paper trading first
- Set a conservative `KILL_THRESHOLD` (default: $3.00 — agent stops if balance drops below this)
- Use `BALANCE_RESERVE_PCT=0.10` to keep 10% of balance untouchable

---

## 7. Environment Variables Reference

### Required

| Variable | Description | Default |
|----------|-------------|---------|
| `GEMINI_API_KEY` | Google Gemini API key | *(none)* |

### AI Models

| Variable | Description | Default |
|----------|-------------|---------|
| `CLAUDE_API_KEY` | Anthropic Claude API key | *(empty)* |
| `CLAUDE_MODEL_HAIKU` | Claude Haiku model ID | `claude-haiku-4-5-20251001` |
| `CLAUDE_MODEL_SONNET` | Claude Sonnet model ID | `claude-sonnet-4-5-20250929` |
| `SCREEN_MODEL` | Model for market screening | `gemini` |
| `DEEP_MODEL` | Model for deep analysis | `gemini` |

### Trading Parameters

| Variable | Description | Default |
|----------|-------------|---------|
| `INITIAL_BALANCE` | Starting paper balance (USD) | `30.00` |
| `PAPER_TRADING` | Enable paper trading mode | `true` |
| `MAX_POSITION_PCT` | Max % of balance per trade | `0.08` |
| `MIN_EDGE_THRESHOLD` | Minimum edge to trade | `0.05` |
| `KELLY_FRACTION` | Kelly criterion fraction | `0.40` |
| `KILL_THRESHOLD` | Stop agent if balance below | `3.00` |
| `MAX_OPEN_POSITIONS` | Max concurrent open trades | `8` |
| `MAX_SPREAD` | Max acceptable bid-ask spread | `0.05` |

### Scan & Cycle

| Variable | Description | Default |
|----------|-------------|---------|
| `SCAN_INTERVAL_SECS` | Seconds between full scan cycles | `1800` |
| `PRICE_CHECK_SECS` | Seconds between price checks | `180` |
| `MAX_MARKETS_SCAN` | Max markets to fetch per cycle | `700` |
| `MAX_CANDIDATES` | Scout output limit | `20` |
| `MAX_DEEP_ANALYSIS` | Deep analysis limit per cycle | `10` |
| `CATEGORY_FILTER` | Market category filter | `all` |
| `MIN_CONFIDENCE` | Minimum confidence to trade | `0.50` |

### Exit Strategy

| Variable | Description | Default |
|----------|-------------|---------|
| `EXIT_TP_PCT` | Take-profit threshold (0 = disabled) | `0` |
| `EXIT_SL_PCT` | Stop-loss threshold (0 = disabled) | `0` |
| `BALANCE_RESERVE_PCT` | Untouchable balance reserve | `0.10` |

### Notifications

| Variable | Description | Default |
|----------|-------------|---------|
| `TELEGRAM_BOT_TOKEN` | Telegram bot token from BotFather | *(empty)* |
| `TELEGRAM_CHAT_ID` | Your Telegram chat ID | *(empty)* |
| `SMTP_HOST` | SMTP server hostname | `smtp.gmail.com` |
| `SMTP_PORT` | SMTP server port | `587` |
| `SMTP_USER` | SMTP username/email | *(empty)* |
| `SMTP_PASS` | SMTP password or app password | *(empty)* |
| `ALERT_FROM` | Sender email address | *(empty)* |
| `ALERT_TO` | Recipient email address | *(empty)* |
| `REPORT_INTERVAL_HOURS` | Hours between email reports | `12` |

### Polymarket API

| Variable | Description | Default |
|----------|-------------|---------|
| `GAMMA_API` | Gamma market data API | `https://gamma-api.polymarket.com` |
| `POLYMARKET_CLOB_API` | Polymarket CLOB API | `https://clob.polymarket.com` |
| `POLYMARKET_HOST` | Polymarket website URL | `https://polymarket.com` |
| `WALLET_PRIVATE_KEY` | Wallet private key (live only) | *(empty)* |

### Proxy (proxy/.env)

| Variable | Description | Default |
|----------|-------------|---------|
| `SUPABASE_URL` | Supabase project URL | *(required)* |
| `SUPABASE_SERVICE_KEY` | Supabase service role key | *(required)* |
| `HMAC_SECRET` | HMAC signing secret | *(required)* |
| `PORT` | Proxy server port | `3000` |

### Dashboard

| Variable | Description | Default |
|----------|-------------|---------|
| `DASHBOARD_PORT` | Dashboard web UI port | `8080` |

---

## Quick Start Checklist

1. **Minimum setup (paper trading with Gemini):**
   - [ ] Get Gemini API key
   - [ ] Set `GEMINI_API_KEY` in `.env`
   - [ ] Set `PAPER_TRADING=true`
   - [ ] Run `./run.sh`

2. **Recommended additions:**
   - [ ] Set up Telegram bot for real-time alerts
   - [ ] Configure email for detailed reports
   - [ ] Set up Supabase for the proxy

3. **Live trading (advanced):**
   - [ ] Fund a Polygon wallet with USDC
   - [ ] Set `WALLET_PRIVATE_KEY`
   - [ ] Set `PAPER_TRADING=false`
   - [ ] Start with small `INITIAL_BALANCE`
