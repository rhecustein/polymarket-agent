# Frequently Asked Questions

## General

### Is this a guaranteed money maker?

No. Prediction markets are inherently risky, and no trading system can guarantee profits. The agent uses adversarial AI analysis and Kelly Criterion sizing to find and exploit mispricings, but markets can move against any position. Past performance in paper trading does not guarantee live results. Only trade with money you can afford to lose.

### How much does it cost to run?

Approximately **$3/day in API costs** with default settings (Gemini + Claude). Using Gemini only drops this to about $1/day. There are no subscription fees for the agent itself. If trading live, Polymarket charges standard trading fees on executed orders.

### What categories does it trade?

The agent evaluates markets across five categories:
- **Crypto** - Bitcoin, Ethereum, and other cryptocurrency price/event markets
- **Weather** - Temperature records, hurricane paths, precipitation events
- **Sports** - Game outcomes, player performances, tournament results
- **Politics** - Elections, policy decisions, geopolitical events
- **General** - Entertainment, technology, science, and everything else

Each category has a specialist desk agent tuned to that domain.

### What are the minimum system requirements?

Any system that can compile Rust code. Specifically:
- 512 MB RAM (the agent uses ~50 MB at runtime)
- 100 MB disk space (plus space for SQLite database growth)
- Stable internet connection
- Works on Linux, macOS, and Windows

## Configuration

### Can I use only Gemini without Claude?

Yes. Leave `CLAUDE_API_KEY` empty or unset in your `.env` file. The agent will use Gemini Flash for all AI analysis, including the Bull/Bear/Judge debate. Claude Sonnet is only used as a premium upgrade for the top 3 candidates per cycle. The system works well with Gemini alone.

### How do I change how often the agent scans?

Set `SCAN_INTERVAL_SECS` in your `.env`. Default is 3600 (1 hour). Minimum recommended is 1800 (30 minutes) to avoid unnecessary API costs. For live trading, 3600 is generally optimal.

### Can I exclude certain categories?

Not directly via configuration yet. You can modify the Scout agent's filtering logic in `src/team/scout.rs` to skip categories you want to avoid.

## Operation

### What is paper trading?

Paper trading runs the full analysis pipeline against real market data but executes trades in a virtual portfolio. No real money is involved. It is the recommended way to evaluate the agent before committing funds. See the [Paper Trading Guide](PAPER_TRADING.md) for details.

### How do I stop the agent?

Three ways, from gentlest to most immediate:

1. **STOP file**: Create a file named `STOP` in the working directory. The agent finishes its current cycle and exits cleanly.
2. **Telegram**: Send `/stop` to your configured bot. Same behavior as the STOP file.
3. **Ctrl+C**: Sends SIGINT. The agent catches it and shuts down after the current operation completes.

All three methods are graceful; the agent will not leave data in an inconsistent state.

### Is my data shared?

Only if you explicitly set `KNOWLEDGE_SHARING=true` in your `.env`. When enabled, the agent shares **anonymized** trade outcomes (category, mode, result, return percentage) with the community. It never shares your wallet, balance, specific markets, or any identifying information. See the [Knowledge System](KNOWLEDGE_SYSTEM.md) documentation for full details on what is and is not shared.

### Where are trades stored?

All trade data is stored locally in:
- `data/portfolio.db` - SQLite database with full trade history and portfolio state
- `data/trades.jsonl` - Append-only log file with one JSON record per trade

### How do I view my performance?

Multiple options:
- **Dashboard**: Run `cargo run --release --bin dashboard` and visit `http://localhost:3000`
- **Telegram**: Send `/status` for a quick summary or `/trades` for recent history
- **Logs**: The agent prints a cycle summary after each scan
- **Database**: Query `data/portfolio.db` directly with any SQLite client

## Troubleshooting

### The agent is not finding any markets

This usually means the Polymarket CLOB API is temporarily unavailable. The agent will retry on the next cycle. If the problem persists, check that you have internet connectivity and that polymarket.com is accessible.

### API rate limiting errors

If using the free Gemini tier, reduce `MAX_DEEP_ANALYSIS` to 3 or fewer. The free tier has strict rate limits. Upgrading to Gemini Paid Tier 1 ($0.10/1M input tokens) removes most rate limits.

### The agent skips most markets

This is normal and expected. The agent is conservative by design. A typical cycle scans hundreds of markets but may only find 1-2 worth trading. If it consistently finds zero, try lowering `MIN_EDGE` from 0.07 to 0.05 (but be aware this may reduce win rate).

### Dashboard shows no data

Make sure the agent has completed at least one full cycle. The dashboard reads from `data/portfolio.db`, which is created after the first trade or cycle completion.
