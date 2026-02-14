# 🐳 Docker Deployment Guide

Panduan deploy Polymarket Agent menggunakan Docker untuk Linux VPS.

## 📋 Prerequisites

- Docker 20.10+
- Docker Compose 2.0+
- File `.env` dengan konfigurasi lengkap
- Minimal 2GB RAM, 10GB disk space

## 🚀 Quick Start

### 1. Clone & Setup

```bash
git clone https://github.com/rhecustein/polymarket-agent.git
cd polymarket-agent

# Copy dan edit file .env
cp .env.example .env
nano .env
```

### 2. Build & Run

```bash
# Build images (multistage build, ~5-10 menit pertama kali)
docker compose build

# Start services
docker compose up -d

# Check logs
docker compose logs -f

# Check status
docker compose ps
```

### 3. Access Services

- **Dashboard**: http://your-vps-ip:3000
- **Proxy API**: http://your-vps-ip:3001
- **Health Check**: http://your-vps-ip:3001/api/health

## 🔧 Management Commands

```bash
# Start services
docker compose up -d

# Stop services
docker compose down

# Restart services
docker compose restart

# View logs (semua services)
docker compose logs -f

# View logs (specific service)
docker compose logs -f dashboard
docker compose logs -f proxy

# Check resource usage
docker stats

# Remove everything (termasuk volumes)
docker compose down -v
```

## 📊 Data Persistence

Data disimpan di Docker volumes:

```bash
# List volumes
docker volume ls

# Inspect volume
docker volume inspect polymarket-agent-data
docker volume inspect polymarket-agent-configs

# Backup data volume
docker run --rm -v polymarket-agent-data:/data -v $(pwd):/backup \
  alpine tar czf /backup/agent-data-backup.tar.gz -C /data .

# Restore data volume
docker run --rm -v polymarket-agent-data:/data -v $(pwd):/backup \
  alpine tar xzf /backup/agent-data-backup.tar.gz -C /data
```

## 🔄 Update & Rebuild

```bash
# Pull latest code
git pull origin main

# Rebuild images
docker compose build --no-cache

# Restart with new images
docker compose up -d

# Remove old images
docker image prune -f
```

## 🐛 Troubleshooting

### Service Won't Start

```bash
# Check logs untuk error
docker compose logs dashboard
docker compose logs proxy

# Check service health
docker compose ps

# Restart problematic service
docker compose restart dashboard
```

### Port Already in Use

```bash
# Check ports
sudo netstat -tulpn | grep :3000
sudo netstat -tulpn | grep :3001

# Stop conflicting service atau ubah port di docker-compose.yml
```

### Out of Memory

```bash
# Check memory usage
docker stats

# Add memory limits di docker-compose.yml:
services:
  dashboard:
    deploy:
      resources:
        limits:
          memory: 1G
```

### Healthcheck Failing

```bash
# Check proxy health endpoint
curl http://localhost:3001/api/health

# Rebuild proxy jika perlu
docker compose build proxy
docker compose up -d proxy
```

## 🔒 Security Recommendations

### 1. Firewall Setup

```bash
# Allow only necessary ports
sudo ufw allow 22/tcp   # SSH
sudo ufw allow 3000/tcp # Dashboard (atau gunakan reverse proxy)
sudo ufw deny 3001/tcp  # Proxy internal only

# Enable firewall
sudo ufw enable
```

### 2. Reverse Proxy (Nginx)

```nginx
# /etc/nginx/sites-available/polymarket
server {
    listen 80;
    server_name your-domain.com;

    location / {
        proxy_pass http://localhost:3000;
        proxy_http_version 1.1;
        proxy_set_header Upgrade $http_upgrade;
        proxy_set_header Connection 'upgrade';
        proxy_set_header Host $host;
        proxy_cache_bypass $http_upgrade;
    }
}
```

### 3. SSL/TLS (Let's Encrypt)

```bash
# Install certbot
sudo apt install certbot python3-certbot-nginx

# Get certificate
sudo certbot --nginx -d your-domain.com
```

### 4. Environment Variables

```bash
# JANGAN commit .env ke git
# Gunakan secrets manager untuk production

# Contoh: load dari file terpisah
docker compose --env-file .env.production up -d
```

## 📈 Production Optimization

### docker-compose.prod.yml

```yaml
version: '3.8'

services:
  proxy:
    restart: always
    logging:
      driver: "json-file"
      options:
        max-size: "10m"
        max-file: "3"
    deploy:
      resources:
        limits:
          cpus: '0.5'
          memory: 512M

  dashboard:
    restart: always
    logging:
      driver: "json-file"
      options:
        max-size: "10m"
        max-file: "3"
    deploy:
      resources:
        limits:
          cpus: '1.0'
          memory: 1G
```

Run dengan:
```bash
docker compose -f docker-compose.yml -f docker-compose.prod.yml up -d
```

## 🔄 Auto-restart on Reboot

```bash
# Install docker systemd service
sudo systemctl enable docker

# Services akan auto-start karena restart: unless-stopped
```

## 📊 Monitoring

### Basic Monitoring

```bash
# CPU/Memory usage
docker stats

# Disk usage
docker system df

# Container health
docker compose ps
```

### Advanced: Prometheus + Grafana

```yaml
# Tambahkan di docker-compose.yml
  prometheus:
    image: prom/prometheus
    volumes:
      - ./prometheus.yml:/etc/prometheus/prometheus.yml
    ports:
      - "9090:9090"

  grafana:
    image: grafana/grafana
    ports:
      - "3001:3000"
    environment:
      - GF_SECURITY_ADMIN_PASSWORD=admin
```

## 🧪 Testing di WSL

### Setup WSL2 + Docker Desktop

1. Install Docker Desktop untuk Windows
2. Enable WSL2 integration
3. Test di WSL:

```bash
# Di WSL terminal
cd /mnt/c/Users/YourName/path/to/polymarket-agent

# Build & run
docker compose up -d

# Access dari Windows browser
http://localhost:3000
```

### WSL-Specific Issues

```bash
# Fix permission issues
sudo chown -R $USER:$USER .

# Fix line endings (if needed)
find . -type f -name "*.sh" -exec dos2unix {} \;
```

## 📝 Environment Variables Reference

Required di `.env`:

```env
# AI API Keys
CLAUDE_API_KEY=sk-ant-api03-xxxxx
GEMINI_API_KEY=AIzaSy...

# Trading Parameters
INITIAL_BALANCE=20.00
PAPER_TRADING=true

# Database (auto-created in volumes)
DB_PATH=/app/data/agent.db

# Telegram (optional)
TELEGRAM_BOT_TOKEN=
TELEGRAM_CHAT_ID=

# Email (optional)
SMTP_HOST=smtp.gmail.com
SMTP_PORT=587
SMTP_USER=
SMTP_PASS=
```

## 🎯 Best Practices

1. **Always start with paper trading** (`PAPER_TRADING=true`)
2. **Backup volumes regularly** (lihat section Data Persistence)
3. **Monitor logs** untuk errors dan anomalies
4. **Set resource limits** untuk prevent OOM
5. **Use reverse proxy** untuk production
6. **Enable SSL/TLS** jika expose ke internet
7. **Update regularly** untuk security patches

## 🆘 Support

- Issues: [GitHub Issues](https://github.com/rhecustein/polymarket-agent/issues)
- Logs location: `/var/lib/docker/volumes/`
- Config location: Check `docker volume inspect`

---

**Built with ❤️ and Docker**
