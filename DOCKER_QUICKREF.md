# 🚀 Docker Quick Reference

Cheatsheet untuk common commands Docker deployment.

## 📦 Basic Operations

```bash
# Build images
docker compose build

# Build without cache (fresh build)
docker compose build --no-cache

# Start services (detached)
docker compose up -d

# Start services (foreground)
docker compose up

# Stop services
docker compose down

# Stop and remove volumes (⚠️ deletes data!)
docker compose down -v

# Restart all services
docker compose restart

# Restart specific service
docker compose restart dashboard
```

## 📊 Monitoring & Logs

```bash
# View all logs (follow)
docker compose logs -f

# View specific service logs
docker compose logs -f dashboard
docker compose logs -f proxy

# View last 100 lines
docker compose logs --tail=100

# Check service status
docker compose ps

# Check resource usage
docker stats

# Check resource usage (one-time)
docker stats --no-stream
```

## 🔍 Debugging

```bash
# Execute command in running container
docker compose exec dashboard bash
docker compose exec proxy bash

# View container details
docker compose ps --format json

# Inspect service
docker compose config

# Check networks
docker network ls
docker network inspect polymarket-network

# Check volumes
docker volume ls
docker volume inspect polymarket-agent-data
```

## 💾 Data Management

```bash
# Backup data volume
docker run --rm \
  -v polymarket-agent-data:/data \
  -v $(pwd):/backup \
  alpine tar czf /backup/data-backup.tar.gz -C /data .

# Backup configs volume
docker run --rm \
  -v polymarket-agent-configs:/configs \
  -v $(pwd):/backup \
  alpine tar czf /backup/configs-backup.tar.gz -C /configs .

# Restore data volume
docker run --rm \
  -v polymarket-agent-data:/data \
  -v $(pwd):/backup \
  alpine tar xzf /backup/data-backup.tar.gz -C /data

# List volume contents
docker run --rm \
  -v polymarket-agent-data:/data \
  alpine ls -lah /data

# Remove unused volumes (⚠️ careful!)
docker volume prune
```

## 🧹 Cleanup

```bash
# Remove stopped containers
docker container prune

# Remove unused images
docker image prune

# Remove unused volumes (⚠️ loses data!)
docker volume prune

# Remove everything unused (⚠️ nuclear option!)
docker system prune -a

# Remove specific images
docker rmi $(docker images -q polymarket-*)

# Full cleanup (stop all, remove all)
docker compose down -v
docker system prune -a --volumes
```

## 🔧 Troubleshooting

```bash
# Rebuild specific service
docker compose build dashboard
docker compose up -d dashboard

# Recreate containers
docker compose up -d --force-recreate

# Check Docker daemon
systemctl status docker

# Check Docker disk usage
docker system df

# View Docker info
docker info

# Test network connectivity
docker compose exec dashboard ping proxy
docker compose exec proxy ping dashboard
```

## 🌐 Port Checking

```bash
# Check if port is in use (Linux)
netstat -tulpn | grep :3000
netstat -tulpn | grep :3001

# Check if port is in use (macOS)
lsof -i :3000
lsof -i :3001

# Test port access
curl http://localhost:3000
curl http://localhost:3001/api/health

# Test from inside container
docker compose exec dashboard curl http://localhost:3000
docker compose exec proxy curl http://localhost:3001/api/health
```

## 🔐 Environment & Config

```bash
# View environment variables
docker compose exec dashboard env

# View config (rendered)
docker compose config

# Validate compose file
docker compose config --quiet

# Use specific .env file
docker compose --env-file .env.production up -d

# Override compose file
docker compose -f docker-compose.yml -f docker-compose.prod.yml up -d
```

## 📈 Production Commands

```bash
# Pull latest images
docker compose pull

# Update and restart
git pull origin main
docker compose build --no-cache
docker compose up -d --force-recreate

# Scale services (if configured)
docker compose up -d --scale dashboard=2

# View service health
docker compose ps --format "table {{.Service}}\t{{.Status}}\t{{.Health}}"
```

## 🎯 WSL-Specific

```bash
# Fix line endings
find . -type f -name "*.sh" -exec dos2unix {} \;

# Fix permissions
sudo chown -R $USER:$USER .

# Access from Windows
# Use: http://localhost:3000

# Check WSL Docker integration
docker context ls
```

## 🚨 Emergency Commands

```bash
# Stop all containers immediately
docker stop $(docker ps -q)

# Remove all containers
docker rm $(docker ps -a -q)

# Kill stuck container
docker kill <container_id>

# Restart Docker daemon
sudo systemctl restart docker

# Reset to fresh state
docker compose down -v
docker system prune -a --volumes -f
docker compose build --no-cache
docker compose up -d
```

## 📱 One-Liners

```bash
# Quick restart
docker compose down && docker compose up -d

# Quick rebuild
docker compose build && docker compose up -d

# View logs for last 5 minutes
docker compose logs --since 5m

# Check if services are healthy
docker compose ps | grep -i "up"

# Get container IPs
docker compose ps -q | xargs docker inspect -f '{{.Name}} - {{range .NetworkSettings.Networks}}{{.IPAddress}}{{end}}'

# Follow logs for both services
docker compose logs -f dashboard proxy

# Backup both volumes at once
timestamp=$(date +%Y%m%d-%H%M%S) && \
docker run --rm \
  -v polymarket-agent-data:/data \
  -v polymarket-agent-configs:/configs \
  -v $(pwd):/backup \
  alpine tar czf /backup/full-backup-${timestamp}.tar.gz /data /configs
```

## 🎨 Pretty Formatting

```bash
# Formatted status
docker compose ps --format "table {{.Service}}\t{{.Status}}\t{{.Ports}}"

# Formatted logs with timestamps
docker compose logs -f --timestamps

# Formatted resource usage
docker stats --format "table {{.Container}}\t{{.CPUPerc}}\t{{.MemUsage}}\t{{.NetIO}}"

# JSON output for scripting
docker compose ps --format json
docker compose config --format json
```

---

## 🔗 Quick Links

- **Dashboard**: http://localhost:3000
- **Proxy API**: http://localhost:3001
- **Health Check**: http://localhost:3001/api/health

## 📚 More Info

- Full guide: `DOCKER.md`
- Changes: `DOCKER_CHANGES.md`
- Main README: `README.md`
- Setup guide: `SETUP.md`

---

**💡 Tip**: Alias common commands in `.bashrc`:
```bash
alias dcu='docker compose up -d'
alias dcd='docker compose down'
alias dcl='docker compose logs -f'
alias dcp='docker compose ps'
alias dcr='docker compose restart'
```
