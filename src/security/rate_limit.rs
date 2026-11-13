//! Per-client request limiting and failed-unlock backoff.
#![allow(dead_code, unused_imports)]

use crate::security::policy;
use dioxus::server::axum::{
    extract::{ConnectInfo, Request},
    http::StatusCode,
    middleware::Next,
    response::{IntoResponse, Response},
    Json,
};
use std::collections::HashMap;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::sync::{LazyLock, Mutex};
use std::time::{Duration, Instant};

struct Bucket {
    tokens: f64,
    last: Instant,
}

static BUCKETS: LazyLock<Mutex<HashMap<IpAddr, Bucket>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

static UNLOCK: LazyLock<Mutex<UnlockState>> = LazyLock::new(|| Mutex::new(UnlockState::default()));

#[derive(Default)]
struct UnlockState {
    failures: u32,
    blocked_until: Option<Instant>,
}

/// General API rate limiter. Health is exempt so container probes never trip
/// the limiter.
#[cfg(all(feature = "server", not(any(feature = "desktop", feature = "mobile"))))]
pub async fn layer(request: Request, next: Next) -> Response {
    let path = request.uri().path();
    if !path.contains("/api/")
        || path.ends_with("/api/health")
        || request.method() == dioxus::server::axum::http::Method::OPTIONS
    {
        return next.run(request).await;
    }
    let settings = policy::current();
    let ip = request
        .extensions()
        .get::<ConnectInfo<SocketAddr>>()
        .map(|connect| connect.0.ip())
        .unwrap_or(IpAddr::V4(Ipv4Addr::LOCALHOST));
    let capacity = settings.api_rate_burst.max(1) as f64;
    let refill_per_second = settings.api_requests_per_minute.max(1) as f64 / 60.0;
    if !allow(ip, capacity, refill_per_second) {
        let body = Json(serde_json::json!({
            "message": "too many requests; retry shortly",
            "code": StatusCode::TOO_MANY_REQUESTS.as_u16(),
            "data": null,
        }));
        return (
            StatusCode::TOO_MANY_REQUESTS,
            [(dioxus::server::axum::http::header::RETRY_AFTER, "1")],
            body,
        )
            .into_response();
    }
    next.run(request).await
}

/// Reject an unlock attempt while the global failure backoff is active.
pub fn unlock_allowed() -> Result<(), u64> {
    let mut state = match UNLOCK.lock() {
        Ok(state) => state,
        Err(poisoned) => poisoned.into_inner(),
    };
    let now = Instant::now();
    if let Some(until) = state.blocked_until {
        if until > now {
            return Err(until.duration_since(now).as_secs().max(1));
        }
        state.blocked_until = None;
        state.failures = 0;
    }
    Ok(())
}

/// Record a failed password attempt and start exponential backoff once the
/// configured threshold is reached.
pub fn record_unlock_failure() {
    let settings = policy::current();
    let mut state = match UNLOCK.lock() {
        Ok(state) => state,
        Err(poisoned) => poisoned.into_inner(),
    };
    state.failures = state.failures.saturating_add(1);
    if state.failures < settings.unlock_failure_threshold {
        return;
    }
    let extra = state
        .failures
        .saturating_sub(settings.unlock_failure_threshold)
        .min(6);
    let factor = 1u64 << extra;
    let delay = settings.unlock_backoff_secs.saturating_mul(factor).min(300);
    state.blocked_until = Instant::now().checked_add(Duration::from_secs(delay));
}

/// Clear the failure counter after a successful unlock.
pub fn record_unlock_success() {
    let mut state = match UNLOCK.lock() {
        Ok(state) => state,
        Err(poisoned) => poisoned.into_inner(),
    };
    state.failures = 0;
    state.blocked_until = None;
}

fn allow(ip: IpAddr, capacity: f64, refill_per_second: f64) -> bool {
    let now = Instant::now();
    let mut buckets = match BUCKETS.lock() {
        Ok(buckets) => buckets,
        Err(poisoned) => poisoned.into_inner(),
    };
    if buckets.len() > 4096 {
        buckets.retain(|_, bucket| now.duration_since(bucket.last) < Duration::from_secs(600));
    }
    let bucket = buckets.entry(ip).or_insert(Bucket {
        tokens: capacity,
        last: now,
    });
    let elapsed = now.duration_since(bucket.last).as_secs_f64();
    bucket.tokens = (bucket.tokens + elapsed * refill_per_second).min(capacity);
    bucket.last = now;
    if bucket.tokens < 1.0 {
        return false;
    }
    bucket.tokens -= 1.0;
    true
}
