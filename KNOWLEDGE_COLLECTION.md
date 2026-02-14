# Knowledge Collection System

Sistem pengumpulan data lokal untuk optimasi strategi trading berdasarkan performance paper trading.

## 🎯 Fitur

Sistem ini secara otomatis mengumpulkan 4 jenis knowledge:

### 1. **Win Rate per Kategori Market**
- Tracking performance berdasarkan kategori (crypto, sports, weather, politics, dll)
- Win rate, total PnL, average edge per kategori
- Identifikasi kategori mana yang paling profitable

### 2. **Fee & Slippage Impact Analysis**
- Total fees (gas + platform + maker/taker) per trade
- Total slippage (entry + exit) per trade
- Persentase impact terhadap profit
- Data untuk optimize position sizing

### 3. **Entry Timing & Confidence Correlation**
- Jam entry optimal (UTC timezone)
- Korelasi antara AI confidence level dan hasil trade
- Duration optimal untuk hold position
- Exit reason analysis

### 4. **Strategy Parameter Optimization**
- Record semua parameter strategy di setiap generation
- Sharpe ratio, max drawdown, win rate per parameter set
- Data untuk genetic algorithm optimization

## 📊 Data Storage

Semua data disimpan **100% lokal** di SQLite database:
- `data/agent.db` (default)
- `data/{agent-id}.db` (multi-agent mode)

**Privacy guarantee:** Tidak ada data yang di-share ke external server.

## 🚀 Usage

### Menjalankan Agent dengan Knowledge Collection

```bash
# Knowledge collection AKTIF secara otomatis saat paper trading
cargo run --bin polyagent

# Multi-agent mode (setiap agent punya knowledge DB sendiri)
cargo run --bin polyagent -- --agent-id alpha
cargo run --bin polyagent -- --agent-id beta
```

### Generate Knowledge Report

```bash
# Generate report dari default database
cargo run --bin polyagent -- --knowledge-report

# Generate report dari specific agent
cargo run --bin polyagent -- --agent-id alpha --knowledge-report
```

Output example:
```
═══ KNOWLEDGE SUMMARY ═══

📊 Category Performance:
  crypto (all): 68.2% win rate, 22 trades, $45.30 PnL, 11.2% avg edge
  weather (swing): 55.0% win rate, 20 trades, $12.50 PnL, 9.5% avg edge
  sports (scalp): 42.1% win rate, 19 trades, -$8.20 PnL, 7.8% avg edge

💰 Cost Impact:
  Avg Fees: 2.15% of bet size
  Avg Slippage: 0.85% of bet size
  Avg Total Impact: 4.2%

⏰ Best Entry Times (UTC):
  14:00-14:59: 72.5% win rate, 8 trades, 12.3% avg PnL
  09:00-09:59: 65.0% win rate, 10 trades, 8.5% avg PnL
  18:00-18:59: 60.0% win rate, 5 trades, 7.2% avg PnL

🎯 ACTIONABLE RECOMMENDATIONS:

✅ FOCUS ON: crypto markets (68.2% win rate, 22 trades)

⚠️  AVOID THESE CATEGORIES:
   • sports (42.1% win rate, 19 trades)

⏰ OPTIMAL TRADING HOURS (UTC):
   1. 14:00-14:59 (72.5% win rate)
   2. 09:00-09:59 (65.0% win rate)
   3. 18:00-18:59 (60.0% win rate)
```

## 🔍 Manual Data Analysis

Query database langsung dengan SQLite:

```bash
# Best performing categories
sqlite3 data/agent.db "
  SELECT category, trade_mode, win_rate, total_trades, total_pnl
  FROM knowledge_category_stats
  ORDER BY win_rate DESC
"

# Cost impact analysis
sqlite3 data/agent.db "
  SELECT
    category,
    AVG(fee_pct_of_size) as avg_fee_pct,
    AVG(slippage_pct_of_size) as avg_slip_pct,
    AVG(cost_impact_pct) as avg_cost_impact
  FROM knowledge_cost_impact
  GROUP BY category
"

# Timing patterns
sqlite3 data/agent.db "
  SELECT
    entry_hour,
    COUNT(*) as trades,
    AVG(pnl_pct) as avg_pnl,
    SUM(CASE WHEN result = 'win' THEN 1 ELSE 0 END) * 100.0 / COUNT(*) as win_rate
  FROM knowledge_timing_analysis
  GROUP BY entry_hour
  HAVING trades >= 3
  ORDER BY win_rate DESC
"

# Strategy parameter comparison
sqlite3 data/agent.db "
  SELECT
    generation,
    min_confidence,
    min_edge,
    category_filter,
    win_rate,
    total_pnl,
    sharpe_ratio
  FROM knowledge_strategy_params
  ORDER BY win_rate DESC
"
```

## 🧬 Genetic Algorithm Optimization

Data knowledge digunakan untuk optimize parameter strategy:

1. **Generation 1**: Random parameter variations
2. **Collect**: Run paper trading, collect knowledge
3. **Analyze**: Generate report, identify best performers
4. **Generation 2**: Mutate parameters based on top performers
5. **Repeat**: Iterate until convergence

Parameter yang di-optimize:
- `MIN_CONFIDENCE` - minimum AI confidence threshold
- `MIN_EDGE_THRESHOLD` - minimum edge untuk trade
- `MAX_POSITION_PCT` - max % of balance per trade
- `KELLY_FRACTION` - Kelly criterion multiplier
- `EXIT_TP_PCT` - take profit threshold
- `EXIT_SL_PCT` - stop loss threshold
- `CATEGORY_FILTER` - focus on specific categories

## 🔐 Privacy & Security

- ✅ **100% local storage** - tidak ada data keluar dari mesin Anda
- ✅ **No network calls** - knowledge collection purely offline
- ✅ **No PII** - tidak ada wallet address, IP, atau identitas
- ✅ **Anonymized** - hanya agregat statistics, bukan individual trades

## 📈 Integration with Dashboard

Dashboard akan show knowledge insights (coming soon):
- Category performance chart
- Cost impact trends
- Optimal timing heatmap
- Parameter optimization progress

## 🛠 Technical Details

### Database Schema

**knowledge_category_stats** - Category performance
- category, trade_mode, total_trades, wins, losses
- win_rate, avg_edge, avg_confidence, avg_hold_hours
- total_pnl, last_updated

**knowledge_cost_impact** - Fee & slippage tracking
- trade_id, bet_size, total_fees, total_slippage
- fee_pct_of_size, slippage_pct_of_size
- pnl_before_costs, pnl_after_costs, cost_impact_pct

**knowledge_timing_analysis** - Timing patterns
- trade_id, entry_timestamp, entry_hour, entry_day_of_week
- judge_confidence, judge_fair_value, edge_at_entry
- hold_duration_hours, exit_reason, result, pnl_pct

**knowledge_strategy_params** - Parameter tracking
- generation, agent_id, parameter values
- total_trades, win_rate, total_pnl, sharpe_ratio, max_drawdown

### Auto-Collection Trigger

Knowledge collection terjadi otomatis pada:
1. **Trade close** - setiap kali position di-close (TP/SL/manual)
2. **Graceful shutdown** - saat agent stop dengan Ctrl+C
3. **Periodic** - setiap resolve cycle (~90 seconds default)

### Cost

Knowledge collection:
- **Compute**: Near-zero (simple DB inserts)
- **Storage**: ~1KB per trade (negligible)
- **Network**: Zero (100% offline)
- **AI API**: Zero (no additional calls)

## 🎓 Best Practices

1. **Run minimum 50 trades** sebelum analyze patterns
2. **Compare multiple agents** dengan parameter berbeda
3. **Focus on win rate per category** untuk filter market
4. **Optimize position size** based on cost impact data
5. **Trade at optimal hours** based on timing analysis
6. **Iterate parameters** dengan genetic algorithm

## ⚡ Quick Start

```bash
# 1. Start paper trading (knowledge auto-collected)
cargo run --bin polyagent

# 2. Let it run for 24-48 hours (or 50+ trades)
# Press Ctrl+C to stop

# 3. Generate knowledge report
cargo run --bin polyagent -- --knowledge-report

# 4. Adjust parameters based on insights
# Edit configs/.env or agent .env file

# 5. Repeat with new generation
GENERATION=2 cargo run --bin polyagent
```

---

**Made with ❤️ for Polymarket Agent Paper Trading Optimization**
