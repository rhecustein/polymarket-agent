# 🐧 Linux Deployment Guide - Polymarket AI Agent

Comprehensive guide for deploying Polymarket AI Agent on Linux systems (Ubuntu, Debian, VPS, etc.)

## 📋 Prerequisites

### System Requirements
- **OS**: Ubuntu 20.04+, Debian 11+, or any modern Linux distro
- **RAM**: Minimum 2GB (4GB+ recommended)
- **Storage**: 10GB free space
- **Network**: Port 3000 and 3001 available

### Required Software

**Option 1: Docker (Recommended)**
```bash
# Install Docker
curl -fsSL https://get.docker.com -o get-docker.sh
sudo sh get-docker.sh
sudo usermod -aG docker $USER
```

**Option 2: Native Rust Build**
```bash
# Install Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source $HOME/.cargo/env
```

---

## 🚀 Deployment Methods

### Method 1: Docker Deployment (Recommended)

**Why Docker?**
- ✅ Clean, isolated environment
- ✅ No Rust installation needed
- ✅ Easy updates and rollbacks
- ✅ Production-ready
- ✅ Portable across Linux distros

#### Quick Start

```bash
# 1. Clone repository
git clone <your-repo-url>
cd polymarket-agent

# 2. Configure environment
cp .env.example .env
nano .env  # Edit configuration

# 3. Run with Docker
chmod +x run-docker.sh
./run-docker.sh
```

#### Manual Docker Commands

```bash
# Build images
docker compose build

# Start services (detached)
docker compose up -d

# View logs
docker compose logs -f

# Check status
docker compose ps

# Stop services
docker compose down
```

#### Docker Compose Services

The `docker-compose.yml` defines two services:

- **proxy** - API proxy service (port 3001)
- **dashboard** - Web dashboard (port 3000)

Both services:
- Auto-restart on failure
- Share persistent volumes for data
- Run health checks automatically
- Use optimized multi-stage builds

---

### Method 2: Native Binary Deployment

For maximum performance or when Docker isn't available.

#### Build from Source

```bash
# 1. Install Rust (if not already installed)
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source $HOME/.cargo/env

# 2. Clone repository
git clone <your-repo-url>
cd polymarket-agent

# 3. Configure environment
cp .env.example .env
nano .env  # Edit configuration

# 4. Build and run
chmod +x run.sh
./run.sh
```

#### What `run.sh` Does

1. **Creates directories**
   - `agent/data/` - SQLite databases
   - `agent/configs/` - Agent configurations

2. **Builds binaries** (release mode)
   - `polyproxy` - Proxy API server
   - `dashboard` - Dashboard web UI
   - `polyagent` - Trading agent binary

3. **Starts services**
   - Proxy on port 3001 (starts first)
   - Dashboard on port 3000 (1 second delay)

4. **Process management**
   - Handles Ctrl+C gracefully
   - Cleans up child processes

---

## 🔧 Configuration

### Environment Variables (.env)

**Required for Proxy:**
```bash
# Proxy API Configuration
SUPABASE_URL=https://placeholder.supabase.co
SUPABASE_SERVICE_KEY=placeholder-key
HMAC_SECRET=your-secret-key-here
PORT=3001
```

**Required for Agent:**
```bash
# Claude API
CLAUDE_API_KEY=sk-ant-api03-xxxxx
CLAUDE_MODEL_HAIKU=claude-haiku-4-5-20251001
CLAUDE_MODEL_SONNET=claude-sonnet-4-5-20250929

# Trading Parameters
INITIAL_BALANCE=20.00
MAX_POSITION_PCT=0.04
MIN_EDGE_THRESHOLD=0.10
KILL_THRESHOLD=3.00
KELLY_FRACTION=0.333

# Polymarket API
GAMMA_API=https://gamma-api.polymarket.com
POLYMARKET_CLOB_API=https://clob.polymarket.com
POLYMARKET_HOST=https://polymarket.com

# Mode
PAPER_TRADING=true
RUST_LOG=info
```

### File Structure

```
polymarket-agent/
├── agent/
│   ├── src/           # Agent source code
│   ├── data/          # SQLite databases (auto-created)
│   └── configs/       # Agent .env files (auto-created)
├── proxy/
│   ├── src/           # Proxy source code
│   └── data/          # Proxy database (auto-created)
├── run.sh             # Native binary launcher
├── run-docker.sh      # Docker launcher
├── reset-db.sh        # Database reset utility
├── docker-compose.yml # Docker services definition
├── Dockerfile         # Multi-stage build
└── .env               # Configuration
```

---

## 🐳 Docker Commands Reference

### Basic Operations

```bash
# Start services
docker compose up -d

# Stop services
docker compose down

# Restart services
docker compose restart

# View logs (all services)
docker compose logs -f

# View logs (specific service)
docker compose logs -f proxy
docker compose logs -f dashboard

# Check service status
docker compose ps

# Rebuild after code changes
docker compose build
docker compose up -d
```

### Maintenance

```bash
# View resource usage
docker stats

# Clean up old images
docker system prune

# Update images
docker compose pull
docker compose up -d

# Backup data volumes
docker run --rm -v polymarket-agent-data:/data -v $(pwd):/backup \
  alpine tar czf /backup/backup-$(date +%Y%m%d).tar.gz /data

# Restore data volumes
docker run --rm -v polymarket-agent-data:/data -v $(pwd):/backup \
  alpine tar xzf /backup/backup-20260215.tar.gz -C /
```

### Debugging

```bash
# Enter container shell
docker compose exec dashboard /bin/bash
docker compose exec proxy /bin/bash

# View container details
docker inspect polymarket-dashboard
docker inspect polymarket-proxy

# Check health status
docker compose ps --format json | jq

# Follow logs in real-time
docker compose logs -f --tail=100
```

---

## 🔒 Production Deployment (VPS)

### Security Hardening

```bash
# 1. Use firewall (ufw)
sudo ufw allow 22/tcp    # SSH
sudo ufw allow 80/tcp    # HTTP (if using reverse proxy)
sudo ufw allow 443/tcp   # HTTPS (if using reverse proxy)
sudo ufw enable

# 2. Use reverse proxy (nginx)
sudo apt install nginx
# Configure nginx to proxy to localhost:3000 and :3001

# 3. Enable SSL with Let's Encrypt
sudo apt install certbot python3-certbot-nginx
sudo certbot --nginx -d yourdomain.com

# 4. Run Docker in rootless mode
dockerd-rootless-setuptool.sh install
```

### Systemd Service (Native Binary)

Create `/etc/systemd/system/polymarket-agent.service`:

```ini
[Unit]
Description=Polymarket AI Agent
After=network.target

[Service]
Type=simple
User=youruser
WorkingDirectory=/home/youruser/polymarket-agent
ExecStart=/home/youruser/polymarket-agent/run.sh
Restart=always
RestartSec=10

[Install]
WantedBy=multi-user.target
```

Enable and start:
```bash
sudo systemctl daemon-reload
sudo systemctl enable polymarket-agent
sudo systemctl start polymarket-agent
sudo systemctl status polymarket-agent
```

### Docker Compose Service

For Docker deployments, use `restart: unless-stopped` (already configured).

```bash
# Start on boot
docker compose up -d

# Docker daemon starts automatically on system boot
# Services will auto-start with it
```

---

## 📊 Monitoring

### Health Checks

```bash
# Check proxy health
curl http://localhost:3001/api/health

# Check dashboard (HTTP status)
curl -I http://localhost:3000

# Docker health status
docker compose ps
```

### Logs

```bash
# Real-time logs (Docker)
docker compose logs -f

# Native binary logs
journalctl -u polymarket-agent -f

# Log rotation (for native)
sudo nano /etc/logrotate.d/polymarket-agent
```

### Resource Monitoring

```bash
# Docker stats
docker stats polymarket-proxy polymarket-dashboard

# System resources
htop
iotop
nethogs
```

---

## 🛠️ Troubleshooting

### Common Issues

**1. Port Already in Use**
```bash
# Find process using port 3000
sudo lsof -i :3000
sudo lsof -i :3001

# Kill process
kill -9 <PID>
```

**2. Permission Denied**
```bash
# Fix script permissions
chmod +x run.sh run-docker.sh reset-db.sh

# Fix Docker permissions
sudo usermod -aG docker $USER
newgrp docker
```

**3. Docker Containers Exit Immediately**
```bash
# Check logs for errors
docker compose logs proxy
docker compose logs dashboard

# Verify .env file has all required variables
cat .env | grep -E "SUPABASE_URL|HMAC_SECRET|CLAUDE_API_KEY"

# Rebuild from scratch
docker compose down -v
docker compose build --no-cache
docker compose up -d
```

**4. Can't Connect to Dashboard**
```bash
# Check if services are running
docker compose ps

# Check firewall
sudo ufw status

# Check listening ports
ss -tlnp | grep -E "3000|3001"
```

**5. Database Locked Errors**
```bash
# Stop all services
docker compose down

# Remove lock files
rm -f agent/data/*.db-wal
rm -f agent/data/*.db-shm

# Restart
docker compose up -d
```

### Reset Everything

```bash
# Docker deployment
docker compose down -v
rm -rf agent/data/* agent/configs/*
docker compose up -d

# Native deployment
./reset-db.sh
./run.sh
```

---

## 🚦 Performance Tuning

### Docker Resource Limits

Edit `docker-compose.yml`:

```yaml
services:
  proxy:
    deploy:
      resources:
        limits:
          cpus: '1.0'
          memory: 1G
        reservations:
          memory: 512M
```

### Rust Binary Optimization

Already enabled in `run.sh`:
- Release mode builds (`--release`)
- Optimized for speed

For even more optimization:
```bash
# Edit Cargo.toml profiles
[profile.release]
lto = true              # Link-time optimization
codegen-units = 1       # Better optimization
opt-level = 3           # Maximum optimization
strip = true            # Strip symbols (smaller binary)
```

---

## 📦 Updates & Maintenance

### Update Code

**Docker:**
```bash
git pull
docker compose build
docker compose up -d
```

**Native:**
```bash
git pull
./run.sh  # Rebuilds automatically
```

### Backup Strategy

**Backup databases:**
```bash
tar czf backup-$(date +%Y%m%d).tar.gz agent/data agent/configs
```

**Restore:**
```bash
tar xzf backup-20260215.tar.gz
```

---

## 🎯 Next Steps

1. **Access Dashboard**: http://localhost:3000
2. **Create Agents**: Configure trading strategies
3. **Monitor**: Check logs and performance
4. **Scale**: Add more agents as needed

For issues, check:
- GitHub Issues
- Project documentation
- Community forums

---

**Project**: Polymarket AI Agent
**License**: MIT
**Platform**: Linux (Ubuntu, Debian, RHEL, etc.)
**Architecture**: x86_64, ARM64 (via Docker)
