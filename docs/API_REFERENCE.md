# Proxy API Reference

The knowledge-sharing proxy exposes the following REST API endpoints. All requests and responses use JSON.

Base URL: `http://localhost:8080` (local) or `https://proxy.polymarket-agent.org` (community)

---

## POST /api/v1/register

Register a new agent with the proxy. Called automatically on first run.

### Request

```json
{
  "agent_hash": "a1b2c3d4e5f6...",
  "version": "1.0.0"
}
```

### Response (200 OK)

```json
{
  "status": "registered",
  "agent_hash": "a1b2c3d4e5f6...",
  "registered_at": "2026-02-12T10:30:00Z"
}
```

### Response (409 Conflict)

```json
{
  "status": "already_registered",
  "agent_hash": "a1b2c3d4e5f6..."
}
```

---

## POST /api/v1/contribute

Submit anonymized trade reports. Requires HMAC signature.

### Headers

```
X-Agent-Hash: a1b2c3d4e5f6...
X-Signature: hmac_sha256_hex_string
Content-Type: application/json
```

### Request

```json
{
  "agent_hash": "a1b2c3d4e5f6...",
  "trades": [
    {
      "category": "crypto",
      "trade_mode": "swing",
      "ai_model": "gemini",
      "side": "yes",
      "edge": 0.12,
      "confidence": 0.75,
      "result": "win",
      "return_pct": 0.35,
      "exit_reason": "take_profit",
      "duration_hours": 6.5
    },
    {
      "category": "weather",
      "trade_mode": "conviction",
      "ai_model": "claude",
      "side": "no",
      "edge": 0.20,
      "confidence": 0.82,
      "result": "win",
      "return_pct": 0.80,
      "exit_reason": "resolved",
      "duration_hours": 72.0
    }
  ],
  "signature": "hmac_sha256_hex_string"
}
```

### Response (200 OK)

```json
{
  "status": "accepted",
  "trades_recorded": 2
}
```

### Response (401 Unauthorized)

```json
{
  "error": "invalid_signature"
}
```

---

## GET /api/v1/insights

Retrieve aggregated community insights. No authentication required.

### Response (200 OK)

```json
{
  "computed_at": "2026-02-12T10:00:00Z",
  "total_trades": 1547,
  "total_agents": 23,
  "overall_win_rate": 0.61,
  "category_stats": {
    "crypto": {
      "trades": 620,
      "win_rate": 0.65,
      "avg_return": 0.18,
      "avg_edge": 0.11,
      "avg_confidence": 0.72
    },
    "weather": {
      "trades": 310,
      "win_rate": 0.58,
      "avg_return": 0.22,
      "avg_edge": 0.14,
      "avg_confidence": 0.68
    },
    "sports": {
      "trades": 412,
      "win_rate": 0.54,
      "avg_return": 0.12,
      "avg_edge": 0.09,
      "avg_confidence": 0.65
    },
    "general": {
      "trades": 205,
      "win_rate": 0.60,
      "avg_return": 0.15,
      "avg_edge": 0.10,
      "avg_confidence": 0.70
    }
  },
  "mode_stats": {
    "scalp": {
      "trades": 680,
      "win_rate": 0.63,
      "avg_return": 0.08,
      "avg_duration_hours": 2.1
    },
    "swing": {
      "trades": 590,
      "win_rate": 0.59,
      "avg_return": 0.21,
      "avg_duration_hours": 18.5
    },
    "conviction": {
      "trades": 277,
      "win_rate": 0.58,
      "avg_return": 0.45,
      "avg_duration_hours": 96.0
    }
  },
  "model_stats": {
    "gemini": {
      "trades": 1100,
      "win_rate": 0.59
    },
    "claude": {
      "trades": 447,
      "win_rate": 0.66
    }
  },
  "golden_rules": [
    "Crypto scalps with >0.70 confidence have 73% win rate",
    "Avoid sports markets with <0.08 edge (47% win rate)",
    "Conviction trades on weather markets outperform all other combos",
    "Swing trades held beyond 36 hours see diminishing returns"
  ]
}
```

---

## GET /api/v1/parameters

Get recommended trading parameters based on community performance data.

### Response (200 OK)

```json
{
  "min_edge": 0.07,
  "min_confidence": 0.50,
  "kelly_fraction": 0.40,
  "max_position_pct": 0.10,
  "scan_interval_secs": 3600,
  "category_adjustments": {
    "crypto": { "min_edge": 0.06, "kelly_boost": 1.1 },
    "weather": { "min_edge": 0.08, "kelly_boost": 1.0 },
    "sports": { "min_edge": 0.10, "kelly_boost": 0.8 },
    "general": { "min_edge": 0.07, "kelly_boost": 1.0 }
  },
  "updated_at": "2026-02-12T10:00:00Z"
}
```

---

## GET /api/v1/health

Health check endpoint.

### Response (200 OK)

```json
{
  "status": "healthy",
  "version": "1.0.0",
  "uptime_secs": 86400,
  "total_agents": 23,
  "total_trades": 1547
}
```

---

## Error Responses

All endpoints may return the following errors:

### 400 Bad Request

```json
{
  "error": "invalid_request",
  "message": "Missing required field: agent_hash"
}
```

### 429 Too Many Requests

```json
{
  "error": "rate_limited",
  "retry_after_secs": 60
}
```

### 500 Internal Server Error

```json
{
  "error": "internal_error",
  "message": "Database connection failed"
}
```
