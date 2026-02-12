use chrono::{DateTime, Utc};
use dashmap::DashMap;
use std::sync::Arc;

/// Tracks per-agent daily trade submissions and per-IP request rates.
#[derive(Clone)]
pub struct RateLimiter {
    /// agent_hash -> (count, window_start_date as YYYY-MM-DD)
    agent_daily: Arc<DashMap<String, (u32, String)>>,
    /// IP address -> (count, window_start as DateTime<Utc>)
    ip_minute: Arc<DashMap<String, (u32, DateTime<Utc>)>>,
    pub max_trades_per_agent_per_day: u32,
    pub max_requests_per_ip_per_minute: u32,
}

impl RateLimiter {
    pub fn new(max_trades_per_agent_per_day: u32, max_requests_per_ip_per_minute: u32) -> Self {
        Self {
            agent_daily: Arc::new(DashMap::new()),
            ip_minute: Arc::new(DashMap::new()),
            max_trades_per_agent_per_day,
            max_requests_per_ip_per_minute,
        }
    }

    /// Check if an agent has exceeded their daily trade submission limit.
    /// Returns Ok(current_count) or Err(message) if over limit.
    pub fn check_agent_limit(&self, agent_hash: &str) -> Result<u32, String> {
        let today = Utc::now().format("%Y-%m-%d").to_string();

        let mut entry = self
            .agent_daily
            .entry(agent_hash.to_string())
            .or_insert((0, today.clone()));

        // Reset counter if it is a new day
        if entry.1 != today {
            entry.0 = 0;
            entry.1 = today;
        }

        if entry.0 >= self.max_trades_per_agent_per_day {
            return Err(format!(
                "Agent has exceeded daily trade limit of {}",
                self.max_trades_per_agent_per_day
            ));
        }

        entry.0 += 1;
        Ok(entry.0)
    }

    /// Check if an IP address has exceeded the per-minute request limit.
    /// Returns Ok(current_count) or Err(message) if over limit.
    pub fn check_ip_limit(&self, ip: &str) -> Result<u32, String> {
        let now = Utc::now();

        let mut entry = self
            .ip_minute
            .entry(ip.to_string())
            .or_insert((0, now));

        // Reset window if more than 60 seconds have passed
        let elapsed = (now - entry.1).num_seconds();
        if elapsed >= 60 {
            entry.0 = 0;
            entry.1 = now;
        }

        if entry.0 >= self.max_requests_per_ip_per_minute {
            return Err(format!(
                "IP has exceeded rate limit of {} requests/minute",
                self.max_requests_per_ip_per_minute
            ));
        }

        entry.0 += 1;
        Ok(entry.0)
    }

    /// Periodically clean up stale entries to prevent memory growth.
    /// Call this from a background task.
    pub fn cleanup_stale_entries(&self) {
        let today = Utc::now().format("%Y-%m-%d").to_string();
        let now = Utc::now();

        // Remove agent entries from previous days
        self.agent_daily.retain(|_, (_, date)| *date == today);

        // Remove IP entries older than 2 minutes
        self.ip_minute
            .retain(|_, (_, window_start)| (now - *window_start).num_seconds() < 120);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_agent_limit() {
        let limiter = RateLimiter::new(3, 60);

        assert!(limiter.check_agent_limit("agent1").is_ok());
        assert!(limiter.check_agent_limit("agent1").is_ok());
        assert!(limiter.check_agent_limit("agent1").is_ok());
        // Fourth should fail
        assert!(limiter.check_agent_limit("agent1").is_err());

        // Different agent is fine
        assert!(limiter.check_agent_limit("agent2").is_ok());
    }

    #[test]
    fn test_ip_limit() {
        let limiter = RateLimiter::new(50, 3);

        assert!(limiter.check_ip_limit("1.2.3.4").is_ok());
        assert!(limiter.check_ip_limit("1.2.3.4").is_ok());
        assert!(limiter.check_ip_limit("1.2.3.4").is_ok());
        // Fourth should fail
        assert!(limiter.check_ip_limit("1.2.3.4").is_err());

        // Different IP is fine
        assert!(limiter.check_ip_limit("5.6.7.8").is_ok());
    }
}
