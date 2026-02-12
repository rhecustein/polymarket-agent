# Architecture: The 14-Agent Virtual Trading Company

Polymarket Agent operates as a **virtual trading company** with 14 specialized AI agents organized into 4 divisions. Each agent has a single responsibility, and they communicate through a structured pipeline to produce high-quality trading decisions.

## Overview

```
                         POLYMARKET AGENT
                    ========================

  INTELLIGENCE DIVISION          SPECIALIST DESK
  =====================          ===============
  [1] Scout                      [4] Crypto Desk
  [2] Researcher                 [5] Weather Desk
  [3] Data Analyst               [6] Sports Desk
                                 [7] General Desk

  DEBATE ROOM                    C-SUITE
  ===========                    =======
  [8]  Bull Analyst              [11] Risk Manager
  [9]  Bear Analyst              [12] Strategist
  [10] Judge (Devil's Advocate)  [13] Executor
                                 [14] Auditor
```

## Division 1: Intelligence Division

The Intelligence Division discovers and enriches market opportunities.

### Agent 1: Scout
- **Role**: Market discovery and filtering
- **AI**: None (rule-based)
- **Process**: Scans ~700 active Polymarket markets via the CLOB API. Filters by liquidity, time to resolution, and category. Produces a ranked list of up to `MAX_CANDIDATES` (default 10) markets.
- **Output**: `Vec<ScoutReport>` with market metadata, current prices, volume, and category tags.

### Agent 2: Researcher
- **Role**: Deep market research
- **AI**: Gemini Flash 2.0
- **Process**: For each candidate market, queries Gemini with the market question, current odds, and available context. Produces a research brief with key factors, recent developments, and probability assessment.
- **Output**: `ResearchReport` with factors, sentiment, and preliminary probability estimate.

### Agent 3: Data Analyst
- **Role**: Quantitative data enrichment
- **AI**: None (data pipeline)
- **Process**: Pulls CoinGecko price/volume data for crypto markets, weather API data for weather markets, and CLOB order book depth for all markets. Computes technical indicators (RSI, momentum, volume trends).
- **Output**: `DataReport` with price history, order book analysis, and computed indicators.

## Division 2: Specialist Desk

Category-specific analysts that understand the nuances of their domain.

### Agent 4: Crypto Desk
- **Role**: Cryptocurrency market specialist
- **AI**: Gemini Flash 2.0
- **Process**: Analyzes crypto markets using on-chain data, CoinGecko metrics, correlation analysis, and market sentiment. Understands BTC dominance cycles, altcoin seasonality, and exchange flows.
- **Output**: `DeskReport` with category-specific probability, confidence, and key factors.

### Agent 5: Weather Desk
- **Role**: Weather event specialist
- **AI**: Gemini Flash 2.0
- **Process**: Analyzes weather markets using forecast data, historical patterns, model consensus (GFS, ECMWF), and seasonal trends.
- **Output**: `DeskReport` with weather-specific analysis.

### Agent 6: Sports Desk
- **Role**: Sports outcome specialist
- **AI**: Gemini Flash 2.0
- **Process**: Analyzes sports markets using team stats, injury reports, historical matchups, and betting line movements.
- **Output**: `DeskReport` with sports-specific analysis.

### Agent 7: General Desk
- **Role**: Politics, entertainment, and general events
- **AI**: Gemini Flash 2.0
- **Process**: Analyzes general markets using news sentiment, polling data, historical precedent, and expert opinions.
- **Output**: `DeskReport` with general analysis.

## Division 3: Debate Room

The adversarial analysis engine that forces consideration of both sides.

### Agent 8: Bull Analyst
- **Role**: Build the strongest possible YES case
- **AI**: Claude Sonnet (top 3 candidates) / Gemini Flash (rest)
- **Process**: Given the research and desk reports, constructs the most compelling argument for YES. Identifies catalysts, underpriced scenarios, and positive momentum signals.
- **Output**: `BullCase` with conviction score (0-100), key arguments, and risk factors.

### Agent 9: Bear Analyst
- **Role**: Build the strongest possible NO case
- **AI**: Claude Sonnet (top 3 candidates) / Gemini Flash (rest)
- **Process**: Constructs the most compelling argument for NO. Identifies headwinds, overpriced scenarios, and negative signals. Actively tries to counter the Bull's arguments.
- **Output**: `BearCase` with conviction score (0-100), key arguments, and risk factors.

### Agent 10: Judge (Devil's Advocate)
- **Role**: Impartial verdict on Bull vs Bear debate
- **AI**: Claude Sonnet (top 3 candidates) / Gemini Flash (rest)
- **Process**: Reviews both cases side by side. Applies calibration rules:
  - Maximum 30% deviation from current market price
  - If both sides are WEAK, verdict is SKIP
  - Must justify which side was more convincing and why
- **Output**: `DevilsVerdict` with final probability, confidence, recommended side (YES/NO/SKIP), and reasoning.

## Division 4: C-Suite

Executive decision-making and execution.

### Agent 11: Risk Manager
- **Role**: Position sizing and portfolio risk control
- **AI**: None (mathematical)
- **Process**: Applies Kelly Criterion with fractional sizing. Checks portfolio concentration limits, correlation exposure, and drawdown thresholds. Enforces survival mode when balance drops below kill threshold.
- **Output**: `RiskAssessment` with recommended position size, risk/reward ratio, and portfolio impact.

### Agent 12: Strategist
- **Role**: Trade mode assignment and exit planning
- **AI**: None (rule-based)
- **Process**: Classifies each trade into one of three modes:
  - **SCALP**: Tight take-profit/stop-loss, 4-hour maximum hold, for quick edge capture
  - **SWING**: 50% edge capture target, 48-hour maximum hold, for medium-term positions
  - **CONVICTION**: Hold to resolution, for high-confidence bets with clear catalysts
- **Output**: `TradePlan` with mode, entry price, take-profit, stop-loss, and time limit.

### Agent 13: Executor
- **Role**: Trade execution
- **AI**: None (execution engine)
- **Process**: Places orders via the Polymarket CLOB API (live mode) or records virtual trades (paper mode). Handles order book slippage estimation and retry logic.
- **Output**: `TradeResult` with fill price, fees, and execution status.

### Agent 14: Auditor
- **Role**: Performance review and learning
- **AI**: Gemini Flash 2.0
- **Process**: Runs every 10 completed trades. Reviews win/loss patterns, identifies systematic errors, and generates actionable insights. Saves learnings to `knowledge.json` for future cycles.
- **Output**: `AuditReport` with performance stats, pattern observations, and parameter adjustment recommendations.

## Pipeline Flow

```
Phase 1: INTELLIGENCE (parallel)
  Scout ──> [10 candidates]
              |
              ├── Researcher (parallel per candidate)
              └── Data Analyst (parallel per candidate)

Phase 2: SPECIALIST DESK (parallel per candidate)
  [Research + Data] ──> Route to correct Desk
                         ├── Crypto Desk
                         ├── Weather Desk
                         ├── Sports Desk
                         └── General Desk

Phase 3: DEBATE (parallel per candidate)
  [Desk Report] ──> Bull Analyst ──┐
                     Bear Analyst ──┤──> Judge ──> Verdict
                                    │
                     (Claude for top 3, Gemini for rest)

Phase 4: EXECUTE (sequential per approved trade)
  [Verdict: YES/NO] ──> Risk Manager ──> Strategist ──> Executor

Phase 5: LEARN (periodic, every 10 trades)
  [Completed Trades] ──> Auditor ──> knowledge.json
```

## API Cost Breakdown

| Agent | Model | Calls/Cycle | Cost/Call | Cost/Cycle |
|-------|-------|-------------|-----------|------------|
| Researcher | Gemini Flash | 5 | $0.002 | $0.010 |
| Crypto/Weather/Sports/General Desk | Gemini Flash | 5 | $0.002 | $0.010 |
| Bull Analyst (top 3) | Claude Sonnet | 3 | $0.008 | $0.024 |
| Bull Analyst (rest) | Gemini Flash | 2 | $0.002 | $0.004 |
| Bear Analyst (top 3) | Claude Sonnet | 3 | $0.008 | $0.024 |
| Bear Analyst (rest) | Gemini Flash | 2 | $0.002 | $0.004 |
| Judge (top 3) | Claude Sonnet | 3 | $0.008 | $0.024 |
| Judge (rest) | Gemini Flash | 2 | $0.002 | $0.004 |
| Auditor | Gemini Flash | 0.1 | $0.002 | $0.000 |
| **Total** | | | | **~$0.06** |

- **Per day** (48 cycles at 30-minute intervals): ~$2.88/day
- **Per month**: ~$86/month
- **Gemini-only mode** (no Claude): ~$0.02/cycle, ~$0.96/day

Note: Costs are estimates based on typical prompt sizes. Actual costs vary with market complexity and number of candidates that pass filtering.
