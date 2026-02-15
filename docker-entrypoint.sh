#!/bin/bash
# ══════════════════════════════════════════════
# Polymarket AI Agent — Docker Entrypoint
# Starts proxy + dashboard (like run.sh)
# ══════════════════════════════════════════════

set -e

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

echo "══════════════════════════════════════════════"
echo "  Polymarket AI Agent"
echo "══════════════════════════════════════════════"
echo ""
echo "[*] Starting services..."
echo ""

# Start proxy on port 3001
/app/polyproxy &
PROXY_PID=$!
echo "  → Proxy started (PID: $PROXY_PID)"

sleep 2

# Start dashboard on port 3000
cd /app
/app/dashboard &
DASH_PID=$!
echo "  → Dashboard started (PID: $DASH_PID)"

echo ""
echo "══════════════════════════════════════════════"
echo "  Dashboard : http://localhost:3000"
echo "  Proxy API : http://localhost:3001"
echo "  Health    : http://localhost:3001/api/health"
echo "══════════════════════════════════════════════"
echo ""

# Wait for both processes
wait
