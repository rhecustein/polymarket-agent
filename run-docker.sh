#!/bin/bash
# ══════════════════════════════════════════════
# Polymarket AI Agent — Docker Deployment Script
# Optimized for Linux systems (VPS, Ubuntu, Debian, etc.)
# ══════════════════════════════════════════════

set -e

ROOT_DIR="$(cd "$(dirname "$0")" && pwd)"

echo "══════════════════════════════════════════════"
echo "  Polymarket AI Agent - Docker Deployment"
echo "══════════════════════════════════════════════"
echo ""

# ── Check Docker ──
if ! command -v docker &> /dev/null; then
    echo "❌ Error: Docker is not installed"
    echo "   Install Docker: https://docs.docker.com/engine/install/"
    exit 1
fi

if ! docker compose version &> /dev/null; then
    echo "❌ Error: Docker Compose is not installed"
    echo "   Install Docker Compose: https://docs.docker.com/compose/install/"
    exit 1
fi

# ── Check .env file ──
if [ ! -f "$ROOT_DIR/.env" ]; then
    echo "❌ Error: .env file not found"
    echo "   Copy .env.example to .env and configure it"
    exit 1
fi

# ── Build and Start ──
echo "[1/3] Building Docker images..."
docker compose build

echo ""
echo "[2/3] Starting services..."
docker compose up -d

echo ""
echo "[3/3] Waiting for services to be ready..."
sleep 3

# ── Check Status ──
echo ""
echo "══════════════════════════════════════════════"
echo "  Service Status"
echo "══════════════════════════════════════════════"
docker compose ps

echo ""
echo "══════════════════════════════════════════════"
echo "  ✅ Polymarket AI Agent is running!"
echo "══════════════════════════════════════════════"
echo ""
echo "  🌐 Dashboard : http://localhost:3000"
echo "  🔌 Proxy API : http://localhost:3001"
echo "  ❤️  Health   : http://localhost:3001/api/health"
echo ""
echo "══════════════════════════════════════════════"
echo ""
echo "Commands:"
echo "  📊 View logs     : docker compose logs -f"
echo "  🛑 Stop services : docker compose down"
echo "  🔄 Restart       : docker compose restart"
echo "  📈 View stats    : docker stats"
echo ""
