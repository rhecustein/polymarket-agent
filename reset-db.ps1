Write-Host "🛑 Stopping all agent processes..." -ForegroundColor Yellow
Stop-Process -Name "polyagent", "dashboard" -Force -ErrorAction SilentlyContinue
Start-Sleep -Seconds 1

Write-Host "🧹 Cleaning up data directory..." -ForegroundColor Yellow
Remove-Item -Path "data\*.db" -Force -ErrorAction SilentlyContinue
Remove-Item -Path "data\*.db-shm" -Force -ErrorAction SilentlyContinue
Remove-Item -Path "data\*.db-wal" -Force -ErrorAction SilentlyContinue
Remove-Item -Path "data\*.jsonl" -Force -ErrorAction SilentlyContinue

Write-Host "🗑️  Cleaning up config files..." -ForegroundColor Yellow
Remove-Item -Path "configs\*.env" -Force -ErrorAction SilentlyContinue

Write-Host "✅ Database reset complete!" -ForegroundColor Green
Write-Host "   - Deleted all agent databases (data/*.db)" -ForegroundColor Cyan
Write-Host "   - Deleted all agent configs (configs/*.env)" -ForegroundColor Cyan
Write-Host ""
Write-Host "💡 Tip: Start dashboard to create new agents with fresh configs" -ForegroundColor Blue
