#!/bin/bash
# ══════════════════════════════════════════════
# Polymarket AI Agent — Linux Setup Script
# Checks dependencies and prepares environment
# ══════════════════════════════════════════════

set -e

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

echo -e "${BLUE}══════════════════════════════════════════════${NC}"
echo -e "${BLUE}  Polymarket AI Agent - Linux Setup${NC}"
echo -e "${BLUE}══════════════════════════════════════════════${NC}"
echo ""

# ── Check Linux Distribution ──
echo -e "${BLUE}[*] Checking Linux distribution...${NC}"
if [ -f /etc/os-release ]; then
    . /etc/os-release
    echo -e "${GREEN}✓${NC} OS: $NAME $VERSION"
else
    echo -e "${YELLOW}⚠${NC}  Could not detect OS (continuing anyway)"
fi

# ── Check Docker ──
echo ""
echo -e "${BLUE}[*] Checking Docker installation...${NC}"
if command -v docker &> /dev/null; then
    DOCKER_VERSION=$(docker --version | cut -d ' ' -f3 | tr -d ',')
    echo -e "${GREEN}✓${NC} Docker installed: $DOCKER_VERSION"

    # Check if user can run docker without sudo
    if docker ps &> /dev/null; then
        echo -e "${GREEN}✓${NC} Docker permissions OK"
    else
        echo -e "${YELLOW}⚠${NC}  Docker requires sudo. Fix with:"
        echo "   sudo usermod -aG docker $USER"
        echo "   newgrp docker"
    fi
else
    echo -e "${RED}✗${NC} Docker not installed"
    echo ""
    echo "Install Docker:"
    echo "  curl -fsSL https://get.docker.com -o get-docker.sh"
    echo "  sudo sh get-docker.sh"
    echo "  sudo usermod -aG docker $USER"
fi

# ── Check Docker Compose ──
echo ""
echo -e "${BLUE}[*] Checking Docker Compose...${NC}"
if docker compose version &> /dev/null; then
    COMPOSE_VERSION=$(docker compose version --short)
    echo -e "${GREEN}✓${NC} Docker Compose installed: $COMPOSE_VERSION"
else
    echo -e "${RED}✗${NC} Docker Compose not installed"
    echo ""
    echo "Docker Compose v2 should come with Docker Desktop."
    echo "For manual installation:"
    echo "  https://docs.docker.com/compose/install/"
fi

# ── Check Rust (optional for native builds) ──
echo ""
echo -e "${BLUE}[*] Checking Rust installation (optional)...${NC}"
if command -v cargo &> /dev/null; then
    RUST_VERSION=$(rustc --version | cut -d ' ' -f2)
    echo -e "${GREEN}✓${NC} Rust installed: $RUST_VERSION"
else
    echo -e "${YELLOW}⚠${NC}  Rust not installed (OK if using Docker)"
    echo ""
    echo "To install Rust for native builds:"
    echo "  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh"
fi

# ── Check Git ──
echo ""
echo -e "${BLUE}[*] Checking Git...${NC}"
if command -v git &> /dev/null; then
    GIT_VERSION=$(git --version | cut -d ' ' -f3)
    echo -e "${GREEN}✓${NC} Git installed: $GIT_VERSION"
else
    echo -e "${YELLOW}⚠${NC}  Git not installed"
    echo "   sudo apt install git  # Debian/Ubuntu"
    echo "   sudo yum install git  # RHEL/CentOS"
fi

# ── Check curl ──
echo ""
echo -e "${BLUE}[*] Checking curl...${NC}"
if command -v curl &> /dev/null; then
    echo -e "${GREEN}✓${NC} curl installed"
else
    echo -e "${YELLOW}⚠${NC}  curl not installed"
    echo "   sudo apt install curl  # Debian/Ubuntu"
fi

# ── Check .env file ──
echo ""
echo -e "${BLUE}[*] Checking configuration...${NC}"
if [ -f ".env" ]; then
    echo -e "${GREEN}✓${NC} .env file exists"

    # Check required variables
    MISSING_VARS=""

    if ! grep -q "CLAUDE_API_KEY=" .env || grep -q "CLAUDE_API_KEY=sk-ant-api03-xxxxx" .env; then
        MISSING_VARS="${MISSING_VARS}\n  - CLAUDE_API_KEY"
    fi

    if ! grep -q "SUPABASE_URL=" .env || grep -q "SUPABASE_URL=https://placeholder.supabase.co" .env; then
        MISSING_VARS="${MISSING_VARS}\n  - SUPABASE_URL"
    fi

    if ! grep -q "HMAC_SECRET=" .env || grep -q "HMAC_SECRET=your-" .env; then
        MISSING_VARS="${MISSING_VARS}\n  - HMAC_SECRET"
    fi

    if [ -n "$MISSING_VARS" ]; then
        echo -e "${YELLOW}⚠${NC}  Configuration incomplete. Please set:${MISSING_VARS}"
    else
        echo -e "${GREEN}✓${NC} Configuration looks good"
    fi
else
    echo -e "${RED}✗${NC} .env file not found"
    echo ""
    echo "Create .env file:"
    echo "  cp .env.example .env"
    echo "  nano .env  # Edit configuration"
fi

# ── Check ports ──
echo ""
echo -e "${BLUE}[*] Checking required ports...${NC}"
PORT_3000_USED=false
PORT_3001_USED=false

if command -v ss &> /dev/null; then
    if ss -tlnp 2>/dev/null | grep -q ":3000"; then
        echo -e "${YELLOW}⚠${NC}  Port 3000 is in use"
        PORT_3000_USED=true
    else
        echo -e "${GREEN}✓${NC} Port 3000 available"
    fi

    if ss -tlnp 2>/dev/null | grep -q ":3001"; then
        echo -e "${YELLOW}⚠${NC}  Port 3001 is in use"
        PORT_3001_USED=true
    else
        echo -e "${GREEN}✓${NC} Port 3001 available"
    fi
else
    echo -e "${YELLOW}⚠${NC}  Could not check ports (ss command not found)"
fi

if [ "$PORT_3000_USED" = true ] || [ "$PORT_3001_USED" = true ]; then
    echo ""
    echo "To find process using ports:"
    echo "  sudo lsof -i :3000"
    echo "  sudo lsof -i :3001"
fi

# ── Check disk space ──
echo ""
echo -e "${BLUE}[*] Checking disk space...${NC}"
AVAILABLE_SPACE=$(df -BG . | tail -1 | awk '{print $4}' | sed 's/G//')
if [ "$AVAILABLE_SPACE" -lt 5 ]; then
    echo -e "${YELLOW}⚠${NC}  Low disk space: ${AVAILABLE_SPACE}GB available (10GB recommended)"
else
    echo -e "${GREEN}✓${NC} Disk space OK: ${AVAILABLE_SPACE}GB available"
fi

# ── Check memory ──
echo ""
echo -e "${BLUE}[*] Checking memory...${NC}"
TOTAL_MEM=$(free -g | awk '/^Mem:/{print $2}')
if [ "$TOTAL_MEM" -lt 2 ]; then
    echo -e "${YELLOW}⚠${NC}  Low memory: ${TOTAL_MEM}GB (2GB+ recommended)"
else
    echo -e "${GREEN}✓${NC} Memory OK: ${TOTAL_MEM}GB"
fi

# ── Make scripts executable ──
echo ""
echo -e "${BLUE}[*] Setting script permissions...${NC}"
chmod +x run.sh run-docker.sh reset-db.sh deploy.sh 2>/dev/null || true
echo -e "${GREEN}✓${NC} Scripts are executable"

# ── Summary ──
echo ""
echo -e "${BLUE}══════════════════════════════════════════════${NC}"
echo -e "${BLUE}  Setup Complete!${NC}"
echo -e "${BLUE}══════════════════════════════════════════════${NC}"
echo ""
echo "Next steps:"
echo ""
echo "1. Configure environment:"
echo "   ${GREEN}nano .env${NC}"
echo ""
echo "2. Choose deployment method:"
echo ""
echo "   ${YELLOW}A) Docker (Recommended):${NC}"
echo "      ${GREEN}./run-docker.sh${NC}"
echo ""
echo "   ${YELLOW}B) Native binary:${NC}"
echo "      ${GREEN}./run.sh${NC}"
echo ""
echo "3. Access dashboard:"
echo "   ${GREEN}http://localhost:3000${NC}"
echo ""
echo "For detailed instructions, see:"
echo "   ${GREEN}cat DEPLOY_LINUX.md${NC}"
echo ""
