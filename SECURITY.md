# Security Policy

## Reporting a Vulnerability

If you discover a security vulnerability, please report it responsibly:

1. **Do NOT open a public issue**
2. Email: bintangworks (at) gmail.com
3. Or use GitHub's [private security advisory](https://github.com/bintangworks/polymarket-agent/security/advisories/new)

We will respond within 48 hours and work with you to address the issue.

## Security Architecture

### Agent Side
- **No Supabase credentials** in agent code -- all database access goes through the proxy server
- **HMAC-SHA256 signing** for all trade contributions to prevent spoofing
- **Machine-ID-based hashing** for agent identification -- no PII collected
- **Wallet private keys** are only used locally and never transmitted
- `.env` files are excluded from version control via `.gitignore`

### Proxy Side
- **Row Level Security (RLS)** enabled on all Supabase tables
- **Rate limiting** per agent (daily) and per IP (per minute) to prevent abuse
- **Input validation** on all submitted trade data
- **HMAC verification** before accepting any contributions
- **No agent-identifying data** stored beyond the anonymous hash

### Data Privacy
- **Shared**: win/loss result, category, trade mode, AI confidence, exit reason
- **Never shared**: wallet address, account balance, identity, specific market URLs

## Best Practices for Users

1. Never commit your `.env` file
2. Use a separate wallet for live trading with limited funds
3. Keep your `WALLET_PRIVATE_KEY` secure
4. Rotate API keys periodically
5. Monitor your agent's activity via Telegram or email alerts
