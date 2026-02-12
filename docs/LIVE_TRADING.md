# Live Trading Guide

> **WARNING**: Live trading uses real money. You can and will lose funds. Prediction markets are inherently risky. Only trade with money you can afford to lose completely.

## Prerequisites

Before enabling live trading, you must:

1. **Paper trade for at least 1 week** and verify positive results.
2. **Have a Polymarket account** at [polymarket.com](https://polymarket.com).
3. **Fund your wallet** with USDC on Polygon.
4. **Obtain CLOB API credentials** from your Polymarket account settings.

## Configuration

Edit your `agent/.env` file:

```bash
# Switch to live mode
TRADING_MODE=live

# Polymarket CLOB API credentials (from your account)
POLY_API_KEY=your_api_key
POLY_SECRET=your_api_secret
POLY_PASSPHRASE=your_passphrase

# Wallet (the private key of your Polymarket trading wallet)
WALLET_PRIVATE_KEY=0xYOUR_PRIVATE_KEY_HERE

# Risk parameters (start conservative)
INITIAL_BALANCE=30.00          # Your actual USDC balance
MAX_POSITION_PCT=0.10          # Max 10% of balance per trade
KELLY_FRACTION=0.40            # Use 40% of Kelly-optimal size
KILL_THRESHOLD=3.00            # Stop trading if balance drops below $3
MIN_EDGE=0.07                  # Minimum 7% edge to enter
MIN_CONFIDENCE=0.50            # Minimum 50% confidence from Judge
```

## Risk Management Parameters

| Parameter | Default | Description |
|-----------|---------|-------------|
| `MAX_POSITION_PCT` | 0.10 | Maximum percentage of balance for a single trade |
| `KELLY_FRACTION` | 0.40 | Fraction of Kelly-optimal bet size (lower = more conservative) |
| `KILL_THRESHOLD` | 3.00 | Stop all trading if balance drops below this amount |
| `MIN_EDGE` | 0.07 | Minimum edge (probability difference) required to trade |
| `MIN_CONFIDENCE` | 0.50 | Minimum Judge confidence to approve a trade |
| `MAX_CANDIDATES` | 10 | Number of markets to evaluate per cycle |
| `MAX_DEEP_ANALYSIS` | 5 | Number of markets for full debate pipeline |

## Starting Live Trading

```bash
make run-live
```

Or manually:
```bash
TRADING_MODE=live cargo run --release --manifest-path agent/Cargo.toml
```

The agent will confirm live mode on startup:
```
[WARN] LIVE TRADING MODE - Real funds at risk
[INFO] Balance: $30.00 USDC | Kill threshold: $3.00
```

## Recommendations

1. **Start with $30 or less**. The agent is designed to grow from small balances.
2. **Monitor closely for the first 24 hours**. Set up Telegram alerts so you see every trade.
3. **Keep `KELLY_FRACTION` at 0.40 or lower** until you have confidence in the system.
4. **Do not increase `MAX_POSITION_PCT` above 0.15**. Concentration risk is the fastest way to blow up.
5. **Set a `KILL_THRESHOLD`** you are comfortable with. The agent enters survival mode (reduced sizing) as it approaches this level.
6. **Review the Auditor's insights** regularly. If it flags systematic losses in a category, consider excluding that category.

## Emergency Stop

If you need to stop immediately:

1. **Create a STOP file**: `touch STOP` in the working directory
2. **Telegram**: Send `/stop` to your bot
3. **Kill the process**: `Ctrl+C` or `kill` the process

The agent will not place new orders after receiving a stop signal. **Existing open positions remain open** and must be managed manually on polymarket.com if you want to close them immediately.

## Understanding Live vs Paper Differences

| Aspect | Paper | Live |
|--------|-------|------|
| Market data | Real | Real |
| Analysis pipeline | Full | Full |
| Order execution | Virtual | Real (CLOB API) |
| Slippage | Estimated | Actual |
| Fees | Simulated | Real (Polymarket fees) |
| Position closing | Instant at mark price | Market order with slippage |
