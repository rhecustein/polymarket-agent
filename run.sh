#!/bin/bash
# ══════════════════════════════════════════════
# Polymarket AI Agent — Run Script
# Dashboard (localhost:3000) - Gemini Only Mode
# Agents are managed from the dashboard UI
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

# ── Build (suppress warnings) ──
echo "══════════════════════════════════════════════"
echo "  Polymarket AI Agent (Gemini-Only)"
echo "══════════════════════════════════════════════"
echo ""
echo "[*] Building dashboard..."
RUSTFLAGS="-A warnings" cargo build --release --manifest-path "$ROOT_DIR/agent/Cargo.toml" 2>&1

# ── Start Dashboard ──
echo ""
echo "[*] Starting dashboard..."
echo ""

DASH_BIN="$ROOT_DIR/target/release/dashboard"
if [ ! -f "$DASH_BIN" ]; then
    DASH_BIN="$ROOT_DIR/agent/target/release/dashboard"
fi
"$DASH_BIN" &
DASH_PID=$!

echo ""
echo "══════════════════════════════════════════════"
echo "  Dashboard : http://localhost:3000"
echo ""
echo "  Gemini-Only Mode (No Proxy Required)"
echo "  Agents are managed from the dashboard UI."
echo "  Open http://localhost:3000, configure, and"
echo "  click START to launch agents."
echo "══════════════════════════════════════════════"
echo ""
echo "Press Ctrl+C to stop"
echo ""

# ── Wait ──
wait
