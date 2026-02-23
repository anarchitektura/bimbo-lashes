use axum::{
    extract::{ConnectInfo, Request, State},
    http::StatusCode,
    middleware::Next,
    response::{IntoResponse, Response},
    Json,
};
use dashmap::DashMap;
use std::net::{IpAddr, SocketAddr};
use std::sync::Arc;
use std::time::{Duration, Instant};

use crate::models::ApiResponse;

/// Type alias to reduce complexity of the nested DashMap structure.
type TierMap = DashMap<&'static str, (RateLimitConfig, DashMap<IpAddr, Vec<Instant>>)>;

// ── Configuration ──

/// Configuration for a single rate limit tier.
#[derive(Debug, Clone)]
pub struct RateLimitConfig {
    /// Maximum requests allowed within the sliding window.
    pub max_requests: u32,
    /// Duration of the sliding window.
    pub window: Duration,
}

// ── Core Rate Limiter ──

/// In-memory per-IP rate limiter using sliding window counters.
///
/// Each tier (e.g. "public", "booking") has its own config and tracking map.
/// Keys are client IP addresses; values are vectors of request timestamps.
#[derive(Debug, Clone)]
pub struct RateLimiter {
    tiers: Arc<TierMap>,
}

impl RateLimiter {
    /// Create a new empty rate limiter. Call `add_tier()` to configure.
    pub fn new() -> Self {
        Self {
            tiers: Arc::new(DashMap::new()),
        }
    }

    /// Register a named tier with its configuration.
    pub fn add_tier(&self, name: &'static str, config: RateLimitConfig) {
        self.tiers.insert(name, (config, DashMap::new()));
    }

    /// Check if a request from `ip` is allowed under the given tier.
    ///
    /// Returns `Ok(())` if allowed, `Err(retry_after_secs)` if rate limited.
    pub fn check(&self, tier: &'static str, ip: IpAddr) -> Result<(), u64> {
        let tier_entry = self.tiers.get(tier).expect("unknown rate limit tier");
        let (config, ip_map) = tier_entry.value();
        let now = Instant::now();
        let window_start = now - config.window;

        let mut entry = ip_map.entry(ip).or_insert_with(Vec::new);

        // Evict expired timestamps
        entry.retain(|t| *t > window_start);

        if entry.len() >= config.max_requests as usize {
            // Time until the oldest request expires from the window
            let oldest = entry[0];
            let retry_after = (oldest + config.window)
                .saturating_duration_since(now)
                .as_secs()
                .max(1);
            return Err(retry_after);
        }

        entry.push(now);
        Ok(())
    }

    /// Remove stale entries (older than 2× window) from all tiers.
    /// Call periodically from a background task.
    pub fn cleanup(&self) {
        let now = Instant::now();
        for tier_entry in self.tiers.iter() {
            let (config, ip_map) = tier_entry.value();
            let cutoff = config.window * 2;
            ip_map.retain(|_ip, timestamps| {
                timestamps.retain(|t| now.duration_since(*t) < cutoff);
                !timestamps.is_empty()
            });
        }
    }
}

// ── IP Extraction ──

/// Extract client IP from X-Forwarded-For header (Caddy proxy) or ConnectInfo.
pub fn extract_client_ip(req: &Request) -> IpAddr {
    // 1. Check X-Forwarded-For (set by Caddy reverse proxy)
    if let Some(forwarded) = req
        .headers()
        .get("x-forwarded-for")
        .and_then(|v| v.to_str().ok())
    {
        if let Some(first_ip) = forwarded.split(',').next() {
            if let Ok(ip) = first_ip.trim().parse::<IpAddr>() {
                return ip;
            }
        }
    }

    // 2. Fall back to ConnectInfo<SocketAddr>
    req.extensions()
        .get::<ConnectInfo<SocketAddr>>()
        .map(|ci| ci.0.ip())
        .unwrap_or_else(|| "127.0.0.1".parse().unwrap())
}

// ── 429 Response Builder ──

fn too_many_requests(retry_after: u64) -> Response {
    let body = ApiResponse::<()>::error(format!(
        "Too many requests. Try again in {} seconds",
        retry_after
    ));
    (
        StatusCode::TOO_MANY_REQUESTS,
        [("Retry-After", retry_after.to_string())],
        Json(body),
    )
        .into_response()
}

// ── Middleware Functions (one per tier) ──

/// Rate limiter for public read-only endpoints (60 req/min).
pub async fn rate_limit_public(
    State(limiter): State<RateLimiter>,
    req: Request,
    next: Next,
) -> Result<Response, Response> {
    let ip = extract_client_ip(&req);
    limiter
        .check("public", ip)
        .map_err(too_many_requests)?;
    Ok(next.run(req).await)
}

/// Rate limiter for authenticated client endpoints (30 req/min).
pub async fn rate_limit_auth(
    State(limiter): State<RateLimiter>,
    req: Request,
    next: Next,
) -> Result<Response, Response> {
    let ip = extract_client_ip(&req);
    limiter
        .check("auth", ip)
        .map_err(too_many_requests)?;
    Ok(next.run(req).await)
}

/// Rate limiter for booking creation (5 req/5min — strictest).
pub async fn rate_limit_booking(
    State(limiter): State<RateLimiter>,
    req: Request,
    next: Next,
) -> Result<Response, Response> {
    let ip = extract_client_ip(&req);
    limiter
        .check("booking", ip)
        .map_err(too_many_requests)?;
    Ok(next.run(req).await)
}

/// Rate limiter for admin endpoints (120 req/min).
pub async fn rate_limit_admin(
    State(limiter): State<RateLimiter>,
    req: Request,
    next: Next,
) -> Result<Response, Response> {
    let ip = extract_client_ip(&req);
    limiter
        .check("admin", ip)
        .map_err(too_many_requests)?;
    Ok(next.run(req).await)
}

// ── Tests ──

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::Ipv4Addr;
    use std::thread::sleep;

    fn test_ip(last: u8) -> IpAddr {
        IpAddr::V4(Ipv4Addr::new(10, 0, 0, last))
    }

    #[test]
    fn test_allows_requests_under_limit() {
        let limiter = RateLimiter::new();
        limiter.add_tier(
            "test",
            RateLimitConfig {
                max_requests: 3,
                window: Duration::from_secs(60),
            },
        );
        let ip = test_ip(1);
        assert!(limiter.check("test", ip).is_ok());
        assert!(limiter.check("test", ip).is_ok());
        assert!(limiter.check("test", ip).is_ok());
    }

    #[test]
    fn test_rejects_over_limit() {
        let limiter = RateLimiter::new();
        limiter.add_tier(
            "test",
            RateLimitConfig {
                max_requests: 2,
                window: Duration::from_secs(60),
            },
        );
        let ip = test_ip(1);
        assert!(limiter.check("test", ip).is_ok());
        assert!(limiter.check("test", ip).is_ok());
        assert!(limiter.check("test", ip).is_err());
    }

    #[test]
    fn test_returns_retry_after() {
        let limiter = RateLimiter::new();
        limiter.add_tier(
            "test",
            RateLimitConfig {
                max_requests: 1,
                window: Duration::from_secs(60),
            },
        );
        let ip = test_ip(1);
        limiter.check("test", ip).unwrap();
        let retry_after = limiter.check("test", ip).unwrap_err();
        assert!(retry_after >= 1 && retry_after <= 60);
    }

    #[test]
    fn test_different_ips_independent() {
        let limiter = RateLimiter::new();
        limiter.add_tier(
            "test",
            RateLimitConfig {
                max_requests: 1,
                window: Duration::from_secs(60),
            },
        );
        let ip1 = test_ip(1);
        let ip2 = test_ip(2);
        assert!(limiter.check("test", ip1).is_ok());
        assert!(limiter.check("test", ip1).is_err()); // exhausted
        assert!(limiter.check("test", ip2).is_ok()); // different IP — ok
    }

    #[test]
    fn test_different_tiers_independent() {
        let limiter = RateLimiter::new();
        limiter.add_tier(
            "tier_a",
            RateLimitConfig {
                max_requests: 1,
                window: Duration::from_secs(60),
            },
        );
        limiter.add_tier(
            "tier_b",
            RateLimitConfig {
                max_requests: 1,
                window: Duration::from_secs(60),
            },
        );
        let ip = test_ip(1);
        assert!(limiter.check("tier_a", ip).is_ok());
        assert!(limiter.check("tier_a", ip).is_err());
        assert!(limiter.check("tier_b", ip).is_ok()); // different tier — ok
    }

    #[test]
    fn test_window_expiry_allows_again() {
        let limiter = RateLimiter::new();
        limiter.add_tier(
            "test",
            RateLimitConfig {
                max_requests: 1,
                window: Duration::from_millis(100),
            },
        );
        let ip = test_ip(1);
        assert!(limiter.check("test", ip).is_ok());
        assert!(limiter.check("test", ip).is_err());

        sleep(Duration::from_millis(150));

        assert!(limiter.check("test", ip).is_ok()); // window expired
    }

    #[test]
    fn test_cleanup_removes_stale_entries() {
        let limiter = RateLimiter::new();
        limiter.add_tier(
            "test",
            RateLimitConfig {
                max_requests: 10,
                window: Duration::from_millis(50),
            },
        );
        let ip = test_ip(1);
        limiter.check("test", ip).unwrap();

        sleep(Duration::from_millis(120)); // > 2× window

        limiter.cleanup();

        // Entry should be gone; new request creates fresh entry
        assert!(limiter.check("test", ip).is_ok());
    }

    #[test]
    fn test_cleanup_preserves_active_entries() {
        let limiter = RateLimiter::new();
        limiter.add_tier(
            "test",
            RateLimitConfig {
                max_requests: 2,
                window: Duration::from_secs(60),
            },
        );
        let ip = test_ip(1);
        limiter.check("test", ip).unwrap();

        limiter.cleanup(); // should NOT remove active entries

        limiter.check("test", ip).unwrap();
        assert!(limiter.check("test", ip).is_err()); // limit is 2, both still count
    }
}
