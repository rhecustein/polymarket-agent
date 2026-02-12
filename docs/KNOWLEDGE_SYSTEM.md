# Knowledge Sharing System

The knowledge system allows agents to anonymously share trade outcomes and collectively improve. Participation is optional and privacy-preserving.

## How It Works

When `KNOWLEDGE_SHARING=true` in your `.env`, the agent periodically submits anonymized trade reports to the community proxy. In return, it receives aggregated insights from all participating agents.

```
Your Agent ──> Proxy Server ──> Supabase Database
                   │
                   └──> Aggregated Insights ──> All Agents
```

## What Is Shared

Each trade report contains **only** the following fields:

| Field | Example | Purpose |
|-------|---------|---------|
| `category` | `"crypto"` | Track performance by market category |
| `trade_mode` | `"swing"` | Track performance by trade mode |
| `ai_model` | `"gemini"` | Track performance by AI model |
| `side` | `"yes"` | Which side was taken |
| `edge` | `0.12` | Estimated edge at entry |
| `confidence` | `0.75` | Judge confidence at entry |
| `result` | `"win"` | Win, loss, or break-even |
| `return_pct` | `0.35` | Percentage return on the trade |
| `exit_reason` | `"take_profit"` | Why the position was closed |
| `duration_hours` | `6.5` | How long the position was held |

## What Is NEVER Shared

The following data never leaves your machine:

- Wallet address or private key
- Account balance or portfolio size
- Position sizes in dollar terms
- Specific market questions or IDs
- IP address (proxy does not log IPs)
- Any personally identifiable information

## Agent Identity

Each agent is identified by a **machine-ID-based hash**:

```
agent_hash = HMAC-SHA256(machine_id, "polymarket-agent-v1")
```

This produces a consistent but anonymous identifier. It allows the proxy to deduplicate reports from the same agent without knowing who the agent is.

## HMAC Signing

Every submission is signed to prevent tampering:

```
signature = HMAC-SHA256(agent_secret, request_body)
```

The `agent_secret` is generated locally during first run and stored in your `data/` directory. It is never transmitted.

## Community Insights

In return for contributing, your agent receives:

### Category Stats
Win rates and average returns broken down by market category (crypto, weather, sports, etc.).

### Mode Stats
Performance comparison of Scalp vs Swing vs Conviction trades across all agents.

### Model Stats
How Gemini-only agents compare to Gemini+Claude agents.

### Golden Rules
Patterns discovered across the community, such as:
- "Crypto scalps have 71% win rate vs 45% for sports scalps"
- "Conviction trades with >0.80 confidence win 82% of the time"
- "Swing trades held >36 hours underperform by 15%"

## GitHub Fallback

If the proxy server is unavailable, the agent falls back to reading static insights from the `knowledge/` directory in the repository. These files are updated periodically via GitHub Actions as the community grows.

- `knowledge/insights.json` - Latest aggregated community insights
- `knowledge/parameters.json` - Recommended parameter values based on community data

## Configuration

```bash
# Enable/disable knowledge sharing
KNOWLEDGE_SHARING=true

# Proxy server URL (default community server)
KNOWLEDGE_PROXY_URL=https://proxy.polymarket-agent.org

# Or self-host the proxy
KNOWLEDGE_PROXY_URL=http://localhost:8080
```

## Self-Hosting the Proxy

If you want to run your own knowledge proxy (for a private group or personal use):

```bash
cd proxy
cp .env.example .env
# Edit .env with your Supabase credentials
cargo run --release
```

The proxy runs on port 8080 by default. Point your agents to it via `KNOWLEDGE_PROXY_URL`.
