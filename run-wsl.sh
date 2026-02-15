#!/bin/bash
# ══════════════════════════════════════════════
# Polymarket AI Agent - WSL Runner
# Gemini-Only, No Proxy Required
# ══════════════════════════════════════════════

set -e

ROOT_DIR="$(cd "$(dirname "$0")" && pwd)"
DASH_PID=""

cleanup() {
    echo ""
    echo "[*] Shutting down..."
    [ -n "$DASH_PID" ] && kill "$DASH_PID" 2>/dev/null && echo "[*] Dashboard stopped"
    exit 0
}

trap cleanup SIGINT SIGTERM

# ── Create directories ──
mkdir -p "$ROOT_DIR/agent/data"
mkdir -p "$ROOT_DIR/agent/configs"

# ── Build ──
echo "══════════════════════════════════════════════"
echo "  Polymarket AI Agent (Gemini-Only)"
echo "══════════════════════════════════════════════"
echo ""
echo "[1/2] Building dashboard..."
cd "$ROOT_DIR/agent"
RUSTFLAGS="-A warnings" cargo build --release
cd "$ROOT_DIR"

# ── Start Dashboard ──
echo ""
echo "[2/2] Starting dashboard..."
echo ""

DASH_BIN="$ROOT_DIR/agent/target/release/dashboard"
if [ ! -f "$DASH_BIN" ]; then
    echo "❌ Dashboard binary not found at: $DASH_BIN"
    exit 1
fi

"$DASH_BIN" &
DASH_PID=$!

echo ""
echo "══════════════════════════════════════════════"
echo "  Dashboard: http://localhost:3000"
echo ""
echo "  Agents are managed from the dashboard UI."
echo "  Open http://localhost:3000, configure, and"
echo "  click START to launch agents."
echo "══════════════════════════════════════════════"
echo ""
echo "Press Ctrl+C to stop"
echo ""

# ── Wait ──
wait
