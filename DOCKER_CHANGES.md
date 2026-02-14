# 🔄 Docker Configuration Changes

## ✅ Yang Sudah Diperbaiki

### 1. **Dockerfile Baru (Root Level)**
- ✅ **Multistage build** dengan 3 stages:
  - Stage 1: Builder - compile semua binaries dari workspace
  - Stage 2: Dashboard runtime - minimal image untuk dashboard
  - Stage 3: Proxy runtime - minimal image untuk proxy
- ✅ **Dependency caching** - build lebih cepat untuk rebuild
- ✅ **Workspace-aware** - build dari root, bukan per-crate
- ✅ **Optimized size** - menggunakan debian:bookworm-slim

### 2. **docker-compose.yml Updated**
- ✅ **2 services** sesuai dengan `run.sh`:
  - `proxy` - Port 3001 (Polymarket Proxy API)
  - `dashboard` - Port 3000 (Multi-agent Management UI)
- ✅ **Proper volumes**:
  - `agent-data` - untuk SQLite databases
  - `agent-configs` - untuk agent configurations
- ✅ **Healthcheck** untuk service dependencies
- ✅ **Network isolation** dengan custom bridge network
- ✅ **Environment variables** dari file `.env`

### 3. **Files Baru**
- ✅ `.dockerignore` - exclude unnecessary files dari build
- ✅ `DOCKER.md` - comprehensive deployment guide
- ✅ `deploy.sh` - interactive deployment script
- ✅ `DOCKER_CHANGES.md` - this file

## 📊 Perbandingan: Sebelum vs Sesudah

### Sebelum (Outdated)
```yaml
services:
  agent:
    build: ./agent          # ❌ Build per-crate, bukan workspace
    env_file: ./agent/.env  # ❌ Wrong path

  proxy:
    build: ./proxy          # ❌ Build per-crate
    env_file: ./proxy/.env  # ❌ Wrong path
    ports:
      - "8080:8080"         # ❌ Wrong port
```

**Problems:**
- ❌ Build dari subdirectory, bukan workspace
- ❌ Port tidak sesuai dengan `run.sh`
- ❌ Tidak ada dashboard service
- ❌ Tidak ada multistage build di agent
- ❌ Volume configuration kurang lengkap

### Sesudah (Fixed)
```yaml
services:
  proxy:
    build:
      context: .               # ✅ Build dari root (workspace)
      dockerfile: Dockerfile
      target: proxy            # ✅ Multistage target
    ports:
      - "3001:3001"           # ✅ Port yang benar
    env_file: - .env          # ✅ Root .env file
    healthcheck: ...          # ✅ Health monitoring

  dashboard:
    build:
      target: dashboard       # ✅ Separate stage
    ports:
      - "3000:3000"          # ✅ Port yang benar
    volumes:
      - agent-data:/app/data      # ✅ Persistent data
      - agent-configs:/app/configs # ✅ Agent configs
```

**Improvements:**
- ✅ Single Dockerfile dengan multistage build
- ✅ Workspace-aware build process
- ✅ Correct ports (3000, 3001) sesuai `run.sh`
- ✅ Proper volume mounting untuk persistence
- ✅ Healthcheck untuk reliability
- ✅ Network isolation
- ✅ Production-ready configuration

## 🏗️ Build Architecture

```
┌─────────────────────────────────────────┐
│ Dockerfile (Root)                       │
│                                         │
│ Stage 1: Builder                        │
│ ┌─────────────────────────────────┐    │
│ │ rust:1.83-slim-bookworm         │    │
│ │ - Build workspace               │    │
│ │ - Cache dependencies            │    │
│ │ - Build: dashboard + polyproxy  │    │
│ └─────────────────────────────────┘    │
│           │                             │
│           ├──> Stage 2: Dashboard       │
│           │    ┌─────────────────┐     │
│           │    │ debian:slim     │     │
│           │    │ + dashboard bin │     │
│           │    │ + ca-certs      │     │
│           │    └─────────────────┘     │
│           │                             │
│           └──> Stage 3: Proxy          │
│                ┌─────────────────┐     │
│                │ debian:slim     │     │
│                │ + polyproxy bin │     │
│                │ + ca-certs+bash │     │
│                └─────────────────┘     │
└─────────────────────────────────────────┘
```

## 🚀 Cara Testing

### Option 1: Linux VPS (Production)

```bash
# 1. Upload files ke VPS
scp -r polymarket-agent user@your-vps:~/

# 2. SSH ke VPS
ssh user@your-vps

# 3. Masuk ke directory
cd ~/polymarket-agent

# 4. Setup .env
cp .env.example .env
nano .env  # Edit dengan API keys

# 5. Deploy dengan script
chmod +x deploy.sh
./deploy.sh

# Pilih option 1: Build & Start
```

### Option 2: WSL2 (Local Testing)

```bash
# 1. Buka WSL2 terminal
wsl

# 2. Navigate ke project
cd /mnt/c/Users/Bintang\ Wijaya/Herd/polymarket-agent

# 3. Fix line endings (jika perlu)
find . -type f -name "*.sh" -exec dos2unix {} \;

# 4. Setup .env
cp .env.example .env
# Edit .env dengan text editor

# 5. Build & Run
docker compose build
docker compose up -d

# 6. Check logs
docker compose logs -f

# 7. Access dari Windows browser
# http://localhost:3000 - Dashboard
# http://localhost:3001 - Proxy
```

### Option 3: Manual Docker Commands

```bash
# Build images
docker compose build --no-cache

# Start services
docker compose up -d

# Check status
docker compose ps

# View logs
docker compose logs -f dashboard
docker compose logs -f proxy

# Stop services
docker compose down
```

## ✅ Testing Checklist

Setelah deploy, test hal berikut:

- [ ] **Build Success**
  ```bash
  docker compose build
  # Should complete without errors
  ```

- [ ] **Services Running**
  ```bash
  docker compose ps
  # Both dashboard & proxy should be "Up"
  ```

- [ ] **Port Accessible**
  ```bash
  curl http://localhost:3001/api/health  # Proxy health
  curl http://localhost:3000             # Dashboard
  ```

- [ ] **Dashboard UI**
  - Open http://localhost:3000 di browser
  - Harus tampil UI dashboard
  - Test start agent dengan preset strategy

- [ ] **Data Persistence**
  ```bash
  # Stop services
  docker compose down

  # Start again
  docker compose up -d

  # Data harus masih ada (check di dashboard)
  ```

- [ ] **Logs Working**
  ```bash
  docker compose logs -f
  # Should show real-time logs
  ```

- [ ] **Resource Usage**
  ```bash
  docker stats
  # Memory usage < 1GB per service
  ```

## 🔧 Troubleshooting

### Build Gagal

```bash
# Check Docker version
docker --version  # Should be 20.10+

# Clean build cache
docker builder prune -a

# Rebuild
docker compose build --no-cache
```

### Port Already in Use

```bash
# Check what's using the port
netstat -tulpn | grep :3000
netstat -tulpn | grep :3001

# Option 1: Stop conflicting service
# Option 2: Change port di docker-compose.yml
```

### Services Won't Start

```bash
# Check logs
docker compose logs dashboard
docker compose logs proxy

# Common issues:
# - Missing .env file
# - Invalid API keys
# - Port conflicts
```

### Permission Issues (WSL)

```bash
# Fix ownership
sudo chown -R $USER:$USER .

# Fix line endings
dos2unix *.sh
```

### Volumes Not Persisting

```bash
# Check volumes exist
docker volume ls | grep polymarket

# Inspect volume
docker volume inspect polymarket-agent-data

# If needed, recreate
docker compose down -v
docker compose up -d
```

## 📈 Next Steps

1. ✅ Test di WSL2
2. ✅ Fix any issues yang muncul
3. ✅ Test deploy ke VPS
4. 🔄 Production optimization (jika perlu):
   - Add resource limits
   - Setup reverse proxy (Nginx)
   - Add SSL/TLS
   - Setup monitoring
   - Configure backups

## 📝 Notes

- **Multistage build** akan build sekitar 5-10 menit pertama kali
- **Rebuild** lebih cepat karena dependency caching
- **Volumes** persist data bahkan setelah container dihapus
- **Healthcheck** memastikan proxy ready sebelum dashboard start
- **.dockerignore** mempercepat build dengan exclude unnecessary files

## 🎯 Production Considerations

Untuk production deployment, pertimbangkan:

1. **Reverse Proxy** (Nginx/Caddy)
   - SSL/TLS termination
   - Rate limiting
   - Load balancing

2. **Monitoring**
   - Prometheus + Grafana
   - Log aggregation (ELK stack)
   - Alerting (PagerDuty, etc)

3. **Backups**
   - Automated volume backups
   - Off-site storage
   - Backup rotation

4. **Security**
   - Firewall rules
   - Network segmentation
   - Secrets management
   - Regular updates

5. **Scaling**
   - Resource limits
   - Horizontal scaling (if needed)
   - Database optimization

---

**Questions or Issues?**
Check `DOCKER.md` untuk detailed guide atau buat issue di GitHub.
