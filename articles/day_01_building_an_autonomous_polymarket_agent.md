# Building an Autonomous Polymarket Agent with Rust (Day 1)

**"Can we build a trading firm that fits on a USB stick?"**

That's the question I asked myself when starting this project. The goal is simple but ambitious: create a fleet of autonomous AI agents that trade on Polymarket, analyze real-world events, and manage their own risk—all running from a single binary.

Today marks **Day 1** of development. Here's what has been built so far, the tech stack choices, and the interesting challenges encountered along the way.

## 🏗️ The Architecture

We aren't just building a script; we're building a system. The architecture consists of three main components:

1.  **The Proxy (`polyproxy`)**: A high-performance data aggregator. It fetches market data from Polymarket's CLOB (Central Limit Order Book) API, computes technical indicators, and stores insights in a Supabase database.
2.  **The Agent (`polyagent`)**: The brain. It's an autonomous CLI tool that makes trading decisions based on the data provided by the proxy. It supports both **Paper Trading** (for testing strategies without risk) and **Live Trading** (using real USDC on Polygon).
3.  **The Dashboard**: A unified control center. Instead of running multiple terminal windows, I built a web dashboard (embedded directly into a Rust binary using `axum`) that can spawn, monitor, and stop agent processes.

### 🦀 Why Rust?

Rust was the obvious choice for this project for a few reasons:
*   **Performance**: High-frequency data processing needs speed.
*   **Reliability**: The type system prevents entire classes of bugs (like null pointer exceptions).
*   **Single Binary Deployment**: The entire dashboard + agent manager compiles down to a single executable. No `npm install`, no Python venv hell.

## 🚧 Challenges & Fixes (Day 1)

Development is never smooth sailing. Here are a few improved "real-world" bugs we squashed today:

### 1. The "Windows vs. Everyone Else" Problem 🪟

Most of my development happens on macOS/Linux, but today I was testing on Windows. The deployment script (`run.sh`) immediately failed.

**The Issue:**
On Unix systems, binaries are just filenames (e.g., `polyagent`). On Windows, they need the `.exe` extension (e.g., `polyagent.exe`).

**The Fix:**
I updated the dashboard's spawner logic to be platform-aware. It now dynamically checks for the `.exe` extension if the standard path isn't found. This ensures the dashboard works seamlessly across OSes without code changes.

```rust
// Simplified logic
let binary_name = if cfg!(target_os = "windows") { "polyagent.exe" } else { "polyagent" };
```

### 2. The Case of the Invalid Timestamp ⏳

We use Supabase (PostgreSQL) to store market insights. Suddenly, the proxy started crashing with:
`invalid input syntax for type timestamp with time zone`

**The Investigation:**
I was generating timestamps using `chrono::Utc::now().to_rfc3339()`. This produces strings like `2023-10-27T10:00:00+00:00`.
The problem? When passed as a **URL query parameter** to Supabase's API, the `+` character (indicating the timezone) was being interpreted as a **space**. PostgreSQL received `... 00 00` instead of `...+00:00` and rejected it.

**The Fix:**
A classic URL encoding issue. I manually replaced `+` with `%2B` in the query string generation logic.

```rust
// Before: invalid timestamp 
let since = now.to_rfc3339(); 

// After: robust URL encoding
let since = now.to_rfc3339().replace("+", "%2B");
```

### 3. The "Silent Failure" of Child Processes 👻

The dashboard spawns agents as child processes. Initially, when a user clicked "Start", nothing happened if the agent crashed immediately (e.g., due to a missing API key). The UI just stayed silent.

**The Fix:**
I overhauled the spawning logic in `dashboard.rs` to capture `stderr` (standard error) output. Now, if an agent fails to launch, the error is captured and piped directly to the dashboard logs, making debugging trivial.

## 🚀 What's Next?

We have a running dashboard, a functioning proxy, and agents that can be spawned on command.

**Day 2 Plan:** 
*   Refining the **AI Strategy Engine**: Connecting the agents to an LLM (Claude/OpenAI) to analyze news sentiment before placing bets.
*   Implementing **Risk Management**: Adding "Circuit Breakers" that kill an agent if it loses more than X% of its portfolio.

Stay tuned! The code is open-source (soon), and we are just getting started. 🚀

---
*Follow me for daily updates on building the Autonomous Polymarket Agent.*
