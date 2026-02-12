# Paper Trading Guide

Paper trading lets you test the agent's strategy with virtual money against real market data. No funds at risk.

## How It Works

In paper trading mode, the agent:
- Scans **real** Polymarket markets via the public CLOB API
- Runs the **full** 14-agent analysis pipeline (same as live)
- Executes trades in a **virtual** portfolio tracked in SQLite
- Monitors positions against **real** price movements
- Closes positions when take-profit, stop-loss, or time limits are hit

The only difference from live mode is that no orders are placed on-chain. Everything else is identical.

## Trade Modes

The Strategist agent classifies every trade into one of three modes based on edge size, confidence, and time to resolution:

### Scalp
- **Target**: Quick edge capture on small mispricings
- **Take-profit**: Tight (e.g., 3-5% price move)
- **Stop-loss**: Tight (e.g., 2-3% adverse move)
- **Time limit**: 4 hours maximum
- **Typical scenario**: Market at 0.52 when fair value is 0.58

### Swing
- **Target**: Capture ~50% of the identified edge
- **Take-profit**: Dynamic based on edge size
- **Stop-loss**: Wider than Scalp
- **Time limit**: 48 hours maximum
- **Typical scenario**: Market at 0.40 when fair value is 0.55

### Conviction
- **Target**: Hold to resolution for maximum payout
- **Take-profit**: None (hold until market resolves)
- **Stop-loss**: Wide or none
- **Time limit**: Until market resolution
- **Typical scenario**: Market at 0.30 when strong evidence suggests resolution at 1.00

## Monitoring

### Telegram Commands
If you configured a Telegram bot, use these commands:
- `/status` - Current balance, open positions, P&L
- `/trades` - Recent trade history
- `/open` - Details on all open positions
- `/stop` - Graceful shutdown after current cycle

### Email Reports
If SMTP is configured, the agent sends a summary report every 12 hours containing:
- Portfolio value and change
- Trades opened and closed
- Win rate and average return
- Top performing and worst trades

### Web Dashboard
Access the dashboard at **http://localhost:3000** for a real-time view of:
- Portfolio balance over time
- Open and closed positions
- Per-category performance
- Agent pipeline status

Start the dashboard separately:
```bash
cargo run --release --bin dashboard
```

## Graceful Shutdown

To stop the agent cleanly (finishes the current cycle before exiting):

**Option 1**: Create a STOP file in the working directory:
```bash
touch STOP
```

**Option 2**: Send the Telegram `/stop` command.

**Option 3**: Press `Ctrl+C` (the agent handles SIGINT gracefully).

## Understanding the Logs

### Trade Opened
```
[TRADE] OPENED: BTC > $100k by March? | Side: YES @ 0.42 | Size: $2.50 | Mode: SWING
  Edge: 0.15 | Confidence: 0.72 | TP: 0.53 | SL: 0.37 | Expires: 48h
```

### Trade Closed
```
[TRADE] CLOSED: BTC > $100k by March? | Result: WIN +$1.20 (+48.0%)
  Entry: 0.42 | Exit: 0.53 | Reason: take_profit | Duration: 6h 23m
```

### Cycle Summary
```
[CYCLE] #47 complete | Balance: $38.50 | Open: 3 | Win rate: 62% (31/50)
  Scanned: 247 markets | Analyzed: 5 | Traded: 1 | Skipped: 4
```

## Data Files

- `data/portfolio.db` - SQLite database with all trades and portfolio history
- `data/trades.jsonl` - Append-only trade log (one JSON object per line)
- `data/knowledge.json` - Auditor's accumulated insights

## Tips for Paper Trading

1. **Run for at least 48 hours** before evaluating performance. A single cycle is not representative.
2. **Watch the win rate by category**. Some categories may perform better than others in your configuration.
3. **Compare against market movement**. A 60%+ win rate on resolved trades is a good signal.
4. **Check the Auditor's insights** in `knowledge.json` after 10+ trades for patterns.
5. **Adjust parameters gradually**. Change one thing at a time and observe the effect over multiple cycles.
