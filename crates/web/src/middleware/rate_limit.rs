//! Per-IP request rate limiting.
//!
//! The architecture document promised a Tower rate-limit layer from the
//! start; this is it. The sensitive bucket covers sign-in, token refresh
//! and QR scanning — the endpoints worth hammering — and the layer is
//! applied globally because every unauthenticated endpoint benefits.
//!
//! ## Design
//!
//! A fixed-window counter per `(bucket, client)` pair, held in memory.
//! Fixed windows can let a caller burst across a boundary (up to 2× the
//! limit in a pathological two-window straddle); that is an acceptable
//! trade for a club-sized deployment, and the numbers are chosen with
//! the slack in mind. The alternative — a sliding log — costs memory
//! proportional to request volume for accuracy nobody here needs.
//!
//! State lives in the process, so a multi-replica deployment limits per
//! replica. `DEPLOYMENT.md` documents that, and the reverse proxy is
//! the right place for a global limit.
//!
//! ## Client identity
//!
//! The client is the peer socket address, or the first hop in
//! `X-Forwarded-For` when the deployment sits behind a proxy. That
//! header is attacker-controlled unless a proxy overwrites it, so
//! trusting it is a deployment decision: `TRUST_FORWARDED_FOR` gates
//! it, defaulting to off.

use std::{
    collections::HashMap,
    net::{IpAddr, SocketAddr},
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

use axum::{
    extract::{ConnectInfo, Request, State},
    middleware::Next,
    response::{IntoResponse, Response},
};

use crate::{error::WebError, state::AppState};

/// Which limit applies to a request.
#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash)]
pub enum Bucket {
    /// Authentication, OTP, and other abuse-sensitive endpoints.
    Sensitive,
    /// Everything else.
    Default,
}

/// A fixed-window counter shared across handlers.
#[derive(Debug, Clone)]
pub struct RateLimiter {
    inner: Arc<Mutex<HashMap<(Bucket, IpAddr), Window>>>,
    window: Duration,
}

#[derive(Debug, Clone, Copy)]
struct Window {
    started: Instant,
    hits: u32,
}

impl Default for RateLimiter {
    fn default() -> Self {
        Self::new(Duration::from_secs(60))
    }
}

impl RateLimiter {
    /// Build a limiter with the given window length.
    #[must_use]
    pub fn new(window: Duration) -> Self {
        Self {
            inner: Arc::new(Mutex::new(HashMap::new())),
            window,
        }
    }

    /// Record a hit and report whether it is within `limit`.
    ///
    /// Returns `true` when the request may proceed.
    pub fn check(&self, bucket: Bucket, client: IpAddr, limit: u32) -> bool {
        let now = Instant::now();
        // A poisoned lock means another thread panicked mid-update. The
        // counter is not worth propagating a panic over — fail open on
        // the inner state rather than take the process down.
        let mut map = match self.inner.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };

        // Opportunistic sweep: drop windows that closed long ago so an
        // attacker cycling source addresses cannot grow the map without
        // bound. Cheap because it only runs when the map is large.
        if map.len() > 4096 {
            let window = self.window;
            map.retain(|_, w| now.duration_since(w.started) < window * 2);
        }

        let entry = map.entry((bucket, client)).or_insert(Window {
            started: now,
            hits: 0,
        });

        if now.duration_since(entry.started) >= self.window {
            entry.started = now;
            entry.hits = 0;
        }

        entry.hits += 1;
        entry.hits <= limit
    }
}

/// Classify a request path into a bucket.
///
/// Kept as a free function so it can be unit-tested without building a
/// router.
#[must_use]
pub fn bucket_for(path: &str) -> Bucket {
    const SENSITIVE: [&str; 4] = [
        "/api/auth/refresh",
        "/api/auth/login",
        "/api/qr/scan",
        "/api/sync/draftbot",
    ];
    if SENSITIVE.iter().any(|p| path.starts_with(p)) {
        Bucket::Sensitive
    } else {
        Bucket::Default
    }
}

/// Resolve the client address for limiting purposes.
fn client_ip(request: &Request, trust_forwarded: bool) -> Option<IpAddr> {
    if trust_forwarded {
        if let Some(forwarded) = request
            .headers()
            .get("x-forwarded-for")
            .and_then(|v| v.to_str().ok())
        {
            if let Some(first) = forwarded.split(',').next() {
                if let Ok(ip) = first.trim().parse::<IpAddr>() {
                    return Some(ip);
                }
            }
        }
    }
    request
        .extensions()
        .get::<ConnectInfo<SocketAddr>>()
        .map(|ConnectInfo(addr)| addr.ip())
}

/// Axum middleware entry point.
///
/// Applied in `router::build`. Requests without a resolvable client
/// address — which in practice means a test harness calling the router
/// directly — are let through rather than blocked.
pub async fn layer(State(state): State<AppState>, request: Request, next: Next) -> Response {
    let cfg = state.config();
    let bucket = bucket_for(request.uri().path());
    let limit = match bucket {
        Bucket::Sensitive => cfg.rate_limit_sensitive_per_min,
        Bucket::Default => cfg.rate_limit_default_per_min,
    };

    if let Some(ip) = client_ip(&request, cfg.trust_forwarded_for) {
        if !state.rate_limiter().check(bucket, ip, limit) {
            tracing::warn!(%ip, ?bucket, limit, "rate limit exceeded");
            return WebError::RateLimited.into_response();
        }
    }

    next.run(request).await
}

#[cfg(test)]
mod tests {
    use std::net::Ipv4Addr;

    use super::*;

    fn ip(last: u8) -> IpAddr {
        IpAddr::V4(Ipv4Addr::new(127, 0, 0, last))
    }

    #[test]
    fn allows_up_to_the_limit_then_blocks() {
        let limiter = RateLimiter::default();
        for i in 1..=3 {
            assert!(
                limiter.check(Bucket::Sensitive, ip(1), 3),
                "request {i} should pass"
            );
        }
        assert!(!limiter.check(Bucket::Sensitive, ip(1), 3), "4th must block");
    }

    #[test]
    fn buckets_are_counted_independently() {
        let limiter = RateLimiter::default();
        assert!(limiter.check(Bucket::Sensitive, ip(1), 1));
        assert!(!limiter.check(Bucket::Sensitive, ip(1), 1));
        // A different bucket for the same client still has its budget.
        assert!(limiter.check(Bucket::Default, ip(1), 1));
    }

    #[test]
    fn clients_are_counted_independently() {
        let limiter = RateLimiter::default();
        assert!(limiter.check(Bucket::Sensitive, ip(1), 1));
        assert!(!limiter.check(Bucket::Sensitive, ip(1), 1));
        assert!(limiter.check(Bucket::Sensitive, ip(2), 1));
    }

    #[test]
    fn window_rollover_restores_the_budget() {
        // A 1ms window so the test does not sleep for a minute.
        let limiter = RateLimiter::new(Duration::from_millis(1));
        assert!(limiter.check(Bucket::Sensitive, ip(1), 1));
        assert!(!limiter.check(Bucket::Sensitive, ip(1), 1));
        std::thread::sleep(Duration::from_millis(3));
        assert!(
            limiter.check(Bucket::Sensitive, ip(1), 1),
            "budget should reset once the window closes"
        );
    }

    #[test]
    fn sensitive_endpoints_land_in_the_sensitive_bucket() {
        assert_eq!(bucket_for("/api/auth/login"), Bucket::Sensitive);
        assert_eq!(bucket_for("/api/auth/refresh"), Bucket::Sensitive);
        assert_eq!(bucket_for("/api/qr/scan"), Bucket::Sensitive);
        assert_eq!(bucket_for("/api/users/leaderboard"), Bucket::Default);
        assert_eq!(bucket_for("/"), Bucket::Default);
    }
}
