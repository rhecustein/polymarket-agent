# 🚀 Coolify Deployment Guide

## ✅ Pre-Deployment Checklist

### 1. Files Ready
- ✅ `Dockerfile` - Updated to use `rust:latest` (supports edition2024)
- ✅ `docker-compose.prod.yml` - Standalone production config
- ✅ Both services build successfully

### 2. Required Environment Variables

#### Proxy Service (REQUIRED):
```bash
SUPABASE_URL=https://your-project.supabase.co
SUPABASE_SERVICE_KEY=your-service-role-key-here
HMAC_SECRET=generate-random-secret-min-32-chars
```

#### Optional Variables (with defaults):
```bash
PORT=3001
RUST_LOG=info
MAX_TRADES_PER_AGENT_PER_DAY=50
MAX_REQUESTS_PER_IP_PER_MINUTE=60
AGGREGATE_INTERVAL_SECS=3600
```

### 3. How to Generate HMAC_SECRET
```bash
# Option 1: Using openssl
openssl rand -base64 32

# Option 2: Using pwgen (if installed)
pwgen -s 64 1

# Option 3: Online
# Visit: https://randomkeygen.com/
```

## 🎯 Coolify Configuration

### Step 1: Create New Resource
1. Go to Coolify Dashboard
2. Click "New Resource"
3. Choose "Docker Compose"

### Step 2: Repository Settings
- Repository: Your GitHub repo URL
- Branch: `main`
- Docker Compose File: `docker-compose.prod.yml`
- Build Pack: Leave empty (using Dockerfile)

### Step 3: Environment Variables
Add these in Coolify's Environment Variables section:

```env
# REQUIRED - Get from Supabase Project Settings
SUPABASE_URL=https://xxx.supabase.co
SUPABASE_SERVICE_KEY=eyJhbGc...your-service-key

# REQUIRED - Generate new secret
HMAC_SECRET=your-random-secret-here

# OPTIONAL - Customize if needed
RUST_LOG=info
PORT=3001
MAX_TRADES_PER_AGENT_PER_DAY=50
MAX_REQUESTS_PER_IP_PER_MINUTE=60
AGGREGATE_INTERVAL_SECS=3600
```

### Step 4: Port Mapping
Coolify should auto-detect from docker-compose:
- Dashboard: Port 3000
- Proxy: Port 3001

Make sure to expose these ports publicly or configure reverse proxy.

### Step 5: Deploy
Click "Deploy" and monitor the build logs.

## 🔍 Troubleshooting

### Build Fails with "edition2024" Error
- ✅ Already fixed! Dockerfile now uses `rust:latest`

### Proxy Container Keeps Restarting
- ❌ Check environment variables are set correctly
- ❌ Verify SUPABASE_URL and SUPABASE_SERVICE_KEY are valid
- ❌ Ensure HMAC_SECRET is at least 32 characters

### Can't Connect to Dashboard
- Check if port 3000 is exposed
- Verify proxy is healthy (dashboard depends on it)
- Check Coolify's reverse proxy configuration

### Services Build But Don't Start
- Check Coolify logs: `Build Logs` tab
- Check container logs: `Container Logs` tab
- Verify all required environment variables are set

## 📊 Expected Results

After successful deployment:
- ✅ Proxy running on port 3001
- ✅ Dashboard running on port 3000
- ✅ Both containers healthy
- ✅ Can access dashboard UI
- ✅ Can make API calls to proxy

## 🔐 Security Notes

1. **Never commit secrets to Git**
   - Add `.env` to `.gitignore` (already done)
   - Use Coolify's environment variable manager

2. **Generate strong HMAC_SECRET**
   - Minimum 32 characters
   - Use cryptographically secure random generator

3. **Protect Supabase Service Key**
   - This is a privileged key with admin access
   - Only use in server-side code (proxy)
   - Never expose to frontend

## 🎯 Next Steps After Deployment

1. Test the API endpoints
2. Configure domain/SSL in Coolify
3. Set up monitoring and alerts
4. Configure backup strategy for volumes:
   - `polymarket-agent-data` (agent databases)
   - `polymarket-agent-configs` (agent configs)

## 📝 Local Testing (Optional)

If you want to test locally first:

1. Create `.env` file with all required variables
2. Run: `docker compose -f docker-compose.prod.yml up -d`
3. Check logs: `docker compose -f docker-compose.prod.yml logs -f`
4. Access dashboard: http://localhost:3000
5. Test proxy: http://localhost:3001/health

---

**Deploy with confidence!** All Docker build issues have been resolved.
Just configure the environment variables in Coolify and you're good to go! 🚀
