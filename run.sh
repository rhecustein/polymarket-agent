#!/bin/bash
# ══════════════════════════════════════════════
# Polymarket AI Agent — Run Script
# Dashboard (localhost:3000) + Proxy API (localhost:3001)
# Agents are managed from the dashboard UI
# ══════════════════════════════════════════════

set -e

ROOT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROXY_PID=""
DASH_PID=""

cleanup() {
    echo ""
    echo "[*] Shutting down..."
    [ -n "$DASH_PID" ] && kill "$DASH_PID" 2>/dev/null && echo "[*] Dashboard stopped"
    [ -n "$PROXY_PID" ] && kill "$PROXY_PID" 2>/dev/null && echo "[*] Proxy stopped"
    exit 0
}

trap cleanup SIGINT SIGTERM

# ── Create directories ──
mkdir -p "$ROOT_DIR/agent/data"
mkdir -p "$ROOT_DIR/agent/configs"

# ── Build (suppress warnings) ──
echo "══════════════════════════════════════════════"
echo "  Polymarket AI Agent"
echo "══════════════════════════════════════════════"
echo ""
echo "[1/2] Building proxy..."
RUSTFLAGS="-A warnings" cargo build --release --manifest-path "$ROOT_DIR/proxy/Cargo.toml" 2>&1

echo "[2/2] Building agent + dashboard..."
RUSTFLAGS="-A warnings" cargo build --release --manifest-path "$ROOT_DIR/agent/Cargo.toml" 2>&1

# ── Start services ──
echo ""
echo "[*] Starting services..."
echo ""

# Proxy API on port 3001
cd "$ROOT_DIR/proxy"
# Check workspace root target first, then per-crate target
PROXY_BIN="$ROOT_DIR/target/release/polyproxy"
if [ ! -f "$PROXY_BIN" ] && [ ! -f "${PROXY_BIN}.exe" ]; then
    PROXY_BIN="$ROOT_DIR/proxy/target/release/polyproxy"
fi
if [ -f "${PROXY_BIN}.exe" ]; then
    PROXY_BIN="${PROXY_BIN}.exe"
fi
"$PROXY_BIN" &
PROXY_PID=$!
cd "$ROOT_DIR"

sleep 1

# Dashboard UI on port 3000 (manages agents)
cd "$ROOT_DIR/agent"
# Check workspace root target first, then per-crate target
DASH_BIN="$ROOT_DIR/target/release/dashboard"
if [ ! -f "$DASH_BIN" ] && [ ! -f "${DASH_BIN}.exe" ]; then
    DASH_BIN="$ROOT_DIR/agent/target/release/dashboard"
fi
if [ -f "${DASH_BIN}.exe" ]; then
    DASH_BIN="${DASH_BIN}.exe"
fi
"$DASH_BIN" &
DASH_PID=$!
cd "$ROOT_DIR"

echo ""
echo "══════════════════════════════════════════════"
echo "  Dashboard : http://localhost:3000"
echo "  Proxy API : http://localhost:3001"
echo "  Health    : http://localhost:3001/api/health"
echo ""
echo "  Agents are managed from the dashboard UI."
echo "  Open http://localhost:3000, configure, and"
echo "  click START to launch agents."
echo "══════════════════════════════════════════════"
echo ""
echo "Press Ctrl+C to stop all services"
echo ""

# ── Wait ──
wait
