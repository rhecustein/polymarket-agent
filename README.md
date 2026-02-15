# Polymarket AI Agent (Gemini-Only)

Autonomous trading bot for Polymarket using Google Gemini Flash 2.0.

## 🚀 Quick Start

### Windows (WSL2)

```bash
# 1. Open WSL terminal
wsl

# 2. Navigate to project
cd /mnt/c/Users/Bintang\ Wijaya/Herd/polymarket-agent

# 3. Setup
chmod +x wsl-setup.sh
./wsl-setup.sh

# 4. Run
chmod +x run.sh
./run.sh
```

### Linux

```bash
# 1. Setup
chmod +x wsl-setup.sh
./wsl-setup.sh

# 2. Run
./run.sh
```

Dashboard: **http://localhost:3000**

## 📋 Configuration

Edit `.env` file:

```bash
# Gemini API (REQUIRED)
GEMINI_API_KEY=your-key-here

# Trading
INITIAL_BALANCE=20.00
PAPER_TRADING=true

# Mode
GEMINI_ONLY=true
```

Get Gemini API key: https://ai.google.dev/

## ✅ Features

- ✅ **Gemini Flash 2.0** - Fast & cheap AI ($0.10-0.50/day)
- ✅ **Paper Trading** - Test with virtual money
- ✅ **Multi-Desk Strategy** - Crypto, Weather, Sports, General
- ✅ **Risk Management** - Kelly criterion, dynamic TP/SL
- ✅ **Local SQLite** - No cloud dependencies
- ✅ **Telegram Alerts** - Optional notifications

## 📊 Architecture

```
agent/
├── src/
│   ├── main.rs          # Entry point
│   ├── analyzer/        # Gemini AI client
│   ├── strategy/        # Trading strategies
│   ├── paper/           # Paper trading
│   ├── live/            # Live trading (CLOB)
│   └── db.rs            # SQLite storage
├── data/                # Databases
└── configs/             # Agent configs
```

## 🎯 Port Usage

- **3000** - Dashboard UI
- **3001** - ~~Proxy~~ (Removed - not needed)

## 💰 Cost

- **Gemini-only**: ~$10-15/month
- **With Claude**: ~$150-300/month (disabled)

## 🔒 Live Trading

**⚠️ Use paper trading first!**

1. Set `PAPER_TRADING=false` in `.env`
2. Add `WALLET_PRIVATE_KEY`
3. Fund wallet with USDC
4. Monitor carefully

## 📚 Resources

- **Gemini API**: https://ai.google.dev/
- **Polymarket**: https://polymarket.com
- **Issues**: https://github.com/rhecustein/polymarket-agent/issues

## 📖 Documentation

- `SETUP-WSL.md` - WSL setup guide
- `DEPLOY_LINUX.md` - Linux deployment

## 📄 License

MIT
