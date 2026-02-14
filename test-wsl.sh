#!/bin/bash
# ══════════════════════════════════════════════
# Polymarket Agent - WSL2 Test Script
# Quick testing di WSL sebelum deploy ke VPS
# ══════════════════════════════════════════════

set -e

# Colors
GREEN='\033[0;32m'
RED='\033[0;31m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

echo -e "${BLUE}══════════════════════════════════════════════${NC}"
echo -e "${BLUE}  Polymarket Agent - WSL2 Testing${NC}"
echo -e "${BLUE}══════════════════════════════════════════════${NC}"
echo ""

# Step 1: Check Docker
echo -e "${YELLOW}[1/7]${NC} Checking Docker..."
if ! command -v docker &> /dev/null; then
    echo -e "${RED}✗ Docker not found${NC}"
    echo "Install Docker Desktop for Windows and enable WSL2 integration"
    exit 1
fi
echo -e "${GREEN}✓ Docker found: $(docker --version)${NC}"

if ! command -v docker compose &> /dev/null; then
    echo -e "${RED}✗ Docker Compose not found${NC}"
    exit 1
fi
echo -e "${GREEN}✓ Docker Compose found: $(docker compose version)${NC}"

# Step 2: Check .env
echo ""
echo -e "${YELLOW}[2/7]${NC} Checking .env file..."
if [ ! -f ".env" ]; then
    echo -e "${YELLOW}⚠ .env not found, creating from example${NC}"
    if [ -f ".env.example" ]; then
        cp .env.example .env
        echo -e "${GREEN}✓ Created .env${NC}"
        echo -e "${RED}⚠ IMPORTANT: Edit .env and add your API keys!${NC}"
        read -p "Press Enter after editing .env..."
    else
        echo -e "${RED}✗ .env.example not found${NC}"
        exit 1
    fi
else
    echo -e "${GREEN}✓ .env exists${NC}"
fi

# Check for API keys
source .env
if [ -z "$CLAUDE_API_KEY" ] && [ -z "$GEMINI_API_KEY" ]; then
    echo -e "${RED}✗ No API keys found in .env${NC}"
    echo "Add CLAUDE_API_KEY or GEMINI_API_KEY"
    exit 1
fi
echo -e "${GREEN}✓ API keys configured${NC}"

# Step 3: Fix line endings (WSL issue)
echo ""
echo -e "${YELLOW}[3/7]${NC} Fixing line endings (WSL compatibility)..."
if command -v dos2unix &> /dev/null; then
    find . -type f -name "*.sh" -exec dos2unix {} \; 2>/dev/null || true
    echo -e "${GREEN}✓ Line endings fixed${NC}"
else
    echo -e "${YELLOW}⚠ dos2unix not found, skipping (might be OK)${NC}"
fi

# Step 4: Clean previous containers
echo ""
echo -e "${YELLOW}[4/7]${NC} Cleaning previous containers..."
docker compose down -v 2>/dev/null || true
echo -e "${GREEN}✓ Cleaned${NC}"

# Step 5: Build images
echo ""
echo -e "${YELLOW}[5/7]${NC} Building Docker images (this takes 5-10 minutes first time)..."
echo "Building..."
docker compose build --no-cache

if [ $? -eq 0 ]; then
    echo -e "${GREEN}✓ Build successful${NC}"
else
    echo -e "${RED}✗ Build failed${NC}"
    exit 1
fi

# Step 6: Start services
echo ""
echo -e "${YELLOW}[6/7]${NC} Starting services..."
docker compose up -d

if [ $? -eq 0 ]; then
    echo -e "${GREEN}✓ Services started${NC}"
else
    echo -e "${RED}✗ Failed to start services${NC}"
    exit 1
fi

# Wait for services to be ready
echo ""
echo "Waiting for services to start..."
sleep 5

# Step 7: Test services
echo ""
echo -e "${YELLOW}[7/7]${NC} Testing services..."

# Check if containers are running
DASHBOARD_STATUS=$(docker compose ps dashboard --format json 2>/dev/null | grep -o '"State":"[^"]*"' | cut -d'"' -f4)
PROXY_STATUS=$(docker compose ps proxy --format json 2>/dev/null | grep -o '"State":"[^"]*"' | cut -d'"' -f4)

if [ "$DASHBOARD_STATUS" = "running" ]; then
    echo -e "${GREEN}✓ Dashboard container running${NC}"
else
    echo -e "${RED}✗ Dashboard not running (status: $DASHBOARD_STATUS)${NC}"
fi

if [ "$PROXY_STATUS" = "running" ]; then
    echo -e "${GREEN}✓ Proxy container running${NC}"
else
    echo -e "${RED}✗ Proxy not running (status: $PROXY_STATUS)${NC}"
fi

# Test proxy endpoint (wait up to 30 seconds)
echo ""
echo "Testing proxy endpoint..."
for i in {1..30}; do
    if curl -s -f http://localhost:3001/api/health > /dev/null 2>&1; then
        echo -e "${GREEN}✓ Proxy API responding on port 3001${NC}"
        break
    elif [ $i -eq 30 ]; then
        echo -e "${YELLOW}⚠ Proxy API not responding (might still be starting)${NC}"
    else
        sleep 1
    fi
done

# Test dashboard endpoint
echo "Testing dashboard endpoint..."
for i in {1..30}; do
    if curl -s -f http://localhost:3000 > /dev/null 2>&1; then
        echo -e "${GREEN}✓ Dashboard responding on port 3000${NC}"
        break
    elif [ $i -eq 30 ]; then
        echo -e "${YELLOW}⚠ Dashboard not responding (might still be starting)${NC}"
    else
        sleep 1
    fi
done

# Show resource usage
echo ""
echo "Resource usage:"
docker stats --no-stream --format "table {{.Name}}\t{{.CPUPerc}}\t{{.MemUsage}}"

# Final summary
echo ""
echo -e "${BLUE}══════════════════════════════════════════════${NC}"
echo -e "${BLUE}  Testing Complete!${NC}"
echo -e "${BLUE}══════════════════════════════════════════════${NC}"
echo ""
echo -e "${GREEN}Services:${NC}"
echo "  Dashboard : ${GREEN}http://localhost:3000${NC}"
echo "  Proxy API : ${GREEN}http://localhost:3001${NC}"
echo ""
echo -e "${YELLOW}Next Steps:${NC}"
echo "  1. Open http://localhost:3000 di browser Windows"
echo "  2. Check dashboard UI berfungsi"
echo "  3. Test start agent dengan preset strategy"
echo "  4. View logs: ${BLUE}docker compose logs -f${NC}"
echo "  5. Stop: ${BLUE}docker compose down${NC}"
echo ""
echo -e "${GREEN}Jika semua OK, siap deploy ke VPS!${NC}"
echo ""
