#!/bin/bash

echo "🛑 Stopping all agent processes..."
# Kill all polyagent and dashboard processes
pkill -f polyagent || true
pkill -f dashboard || true
pkill -f polyproxy || true
sleep 1

echo "🧹 Cleaning up data directory..."
# Remove all files in data/ but keep the directory
rm -f data/*.db
rm -f data/*.db-shm
rm -f data/*.db-wal
rm -f data/*.jsonl

echo "🗑️  Cleaning up config files..."
# Remove all config files
rm -f configs/*.env

echo "✅ Database reset complete!"
echo "   - Deleted all agent databases (data/*.db)"
echo "   - Deleted all agent configs (configs/*.env)"
echo ""
echo "💡 Tip: Start dashboard to create new agents with fresh configs"
