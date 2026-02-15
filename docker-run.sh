#!/bin/bash
# ══════════════════════════════════════════════
# Polymarket Agent - Docker Runner
# Simplified Gemini-Only Deployment
# ══════════════════════════════════════════════

set -e

echo "══════════════════════════════════════════════"
echo "  Polymarket AI Agent - Docker"
echo "  Gemini-Only Mode"
echo "══════════════════════════════════════════════"
echo ""

# Check if .env exists
if [ ! -f ".env" ]; then
    echo "❌ .env file not found!"
    echo "   Copy .env.example to .env and configure"
    exit 1
fi

# Check Docker
if ! command -v docker &> /dev/null; then
    echo "❌ Docker not installed"
    echo "   Install from: https://www.docker.com/"
    exit 1
fi

# Check Docker Compose
if ! docker compose version &> /dev/null; then
    echo "❌ Docker Compose not installed"
    exit 1
fi

echo "[1/3] Building Docker image..."
docker compose build

echo ""
echo "[2/3] Starting container..."
docker compose up -d

echo ""
echo "[3/3] Checking status..."
sleep 3
docker compose ps

echo ""
echo "══════════════════════════════════════════════"
echo "  ✅ Dashboard Running"
echo "══════════════════════════════════════════════"
echo ""
echo "  🌐 Dashboard: http://localhost:3000"
echo ""
echo "  📊 View logs:  docker compose logs -f"
echo "  🛑 Stop:       docker compose down"
echo "  🔄 Restart:    docker compose restart"
echo ""
echo "══════════════════════════════════════════════"
