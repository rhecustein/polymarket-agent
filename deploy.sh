#!/bin/bash
# ══════════════════════════════════════════════
# Polymarket AI Agent — Docker Deployment Script
# Quick deploy untuk Linux VPS
# ══════════════════════════════════════════════

set -e

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Functions
log_info() {
    echo -e "${GREEN}[INFO]${NC} $1"
}

log_warn() {
    echo -e "${YELLOW}[WARN]${NC} $1"
}

log_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

check_docker() {
    if ! command -v docker &> /dev/null; then
        log_error "Docker not found. Please install Docker first:"
        echo "  curl -fsSL https://get.docker.com -o get-docker.sh"
        echo "  sudo sh get-docker.sh"
        exit 1
    fi

    if ! command -v docker compose &> /dev/null; then
        log_error "Docker Compose not found. Please install Docker Compose v2"
        exit 1
    fi

    log_info "Docker $(docker --version)"
    log_info "Docker Compose $(docker compose version)"
}

check_env() {
    if [ ! -f ".env" ]; then
        log_warn ".env file not found"
        if [ -f ".env.example" ]; then
            log_info "Copying .env.example to .env"
            cp .env.example .env
            log_warn "Please edit .env and add your API keys!"
            echo ""
            echo "Required variables:"
            echo "  - CLAUDE_API_KEY or GEMINI_API_KEY"
            echo "  - INITIAL_BALANCE"
            echo "  - PAPER_TRADING=true (recommended for first run)"
            echo ""
            read -p "Press Enter after editing .env..."
        else
            log_error ".env.example not found. Cannot create .env"
            exit 1
        fi
    fi

    # Check for required env vars
    source .env

    if [ -z "$CLAUDE_API_KEY" ] && [ -z "$GEMINI_API_KEY" ]; then
        log_error "No AI API key found in .env"
        log_error "Set CLAUDE_API_KEY or GEMINI_API_KEY"
        exit 1
    fi

    log_info "Environment configuration OK"
}

show_menu() {
    echo ""
    echo "══════════════════════════════════════════════"
    echo "  Polymarket AI Agent - Docker Deploy"
    echo "══════════════════════════════════════════════"
    echo ""
    echo "1) Build & Start (first time / rebuild)"
    echo "2) Start (existing images)"
    echo "3) Stop"
    echo "4) Restart"
    echo "5) View Logs"
    echo "6) Status"
    echo "7) Backup Data"
    echo "8) Restore Data"
    echo "9) Clean (remove all containers & volumes)"
    echo "0) Exit"
    echo ""
}

build_and_start() {
    log_info "Building Docker images (this may take 5-10 minutes)..."
    docker compose build --no-cache

    log_info "Starting services..."
    docker compose up -d

    echo ""
    log_info "Services started successfully!"
    echo ""
    echo "  Dashboard : http://localhost:3000"
    echo "  Proxy API : http://localhost:3001"
    echo ""
    log_info "View logs with: docker compose logs -f"
}

start_services() {
    log_info "Starting services..."
    docker compose up -d

    echo ""
    log_info "Services started!"
    docker compose ps
}

stop_services() {
    log_info "Stopping services..."
    docker compose down
    log_info "Services stopped"
}

restart_services() {
    log_info "Restarting services..."
    docker compose restart
    log_info "Services restarted"
}

view_logs() {
    echo ""
    echo "Choose service:"
    echo "1) All"
    echo "2) Dashboard"
    echo "3) Proxy"
    read -p "Choice [1-3]: " choice

    case $choice in
        1) docker compose logs -f ;;
        2) docker compose logs -f dashboard ;;
        3) docker compose logs -f proxy ;;
        *) log_error "Invalid choice" ;;
    esac
}

show_status() {
    log_info "Services status:"
    docker compose ps

    echo ""
    log_info "Resource usage:"
    docker stats --no-stream

    echo ""
    log_info "Volumes:"
    docker volume ls | grep polymarket
}

backup_data() {
    BACKUP_FILE="backup-$(date +%Y%m%d-%H%M%S).tar.gz"

    log_info "Creating backup: $BACKUP_FILE"
    docker run --rm \
        -v polymarket-agent-data:/data \
        -v polymarket-agent-configs:/configs \
        -v $(pwd):/backup \
        alpine tar czf /backup/$BACKUP_FILE -C / data configs

    log_info "Backup created: $BACKUP_FILE"
}

restore_data() {
    read -p "Enter backup file name: " backup_file

    if [ ! -f "$backup_file" ]; then
        log_error "Backup file not found: $backup_file"
        return 1
    fi

    log_warn "This will overwrite existing data!"
    read -p "Are you sure? [y/N]: " confirm

    if [ "$confirm" != "y" ]; then
        log_info "Restore cancelled"
        return 0
    fi

    log_info "Restoring from: $backup_file"
    docker run --rm \
        -v polymarket-agent-data:/data \
        -v polymarket-agent-configs:/configs \
        -v $(pwd):/backup \
        alpine tar xzf /backup/$backup_file -C /

    log_info "Restore completed"
}

clean_all() {
    log_warn "This will remove all containers, volumes, and data!"
    read -p "Are you sure? Type 'yes' to confirm: " confirm

    if [ "$confirm" != "yes" ]; then
        log_info "Clean cancelled"
        return 0
    fi

    log_info "Removing all containers and volumes..."
    docker compose down -v

    log_info "Removing images..."
    docker images | grep polymarket | awk '{print $3}' | xargs -r docker rmi

    log_info "Cleanup completed"
}

# Main
main() {
    check_docker
    check_env

    while true; do
        show_menu
        read -p "Choose action [0-9]: " choice

        case $choice in
            1) build_and_start ;;
            2) start_services ;;
            3) stop_services ;;
            4) restart_services ;;
            5) view_logs ;;
            6) show_status ;;
            7) backup_data ;;
            8) restore_data ;;
            9) clean_all ;;
            0)
                log_info "Goodbye!"
                exit 0
                ;;
            *)
                log_error "Invalid choice"
                ;;
        esac

        echo ""
        read -p "Press Enter to continue..."
    done
}

main
