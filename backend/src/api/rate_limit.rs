// ── Per-client rate limiting (BYOK abuse protection) ──────
//
// Lightweight fixed-window limiter built on axum middleware — no extra
// dependencies. Keyed by the effective client identifier (first X-Forwarded-For
// hop, falling back to the socket peer IP) so Render's proxy still yields a
// per-user identity. On exceed we return 429 + Retry-After.
//
// The limiter is deliberately in-memory and per-process: Render's free tier
// runs a single instance, which is exactly the surface this guards.

use axum::{
    body::Body,
    http::{Request, Response, StatusCode},
    middleware::Next,
    response::Response as AxumResponse,
    Extension,
};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

#[derive(Clone, Default)]
pub struct RateLimiter {
    windows: Arc<Mutex<HashMap<String, Window>>>,
}

struct Window {
    start: Instant,
    count: u32,
}

impl RateLimiter {
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns Ok(()) if within budget, Err(retry_after_secs) if over.
    fn check(&self, key: &str, limit: u32, window: Duration) -> Result<(), u64> {
        let mut guard = self.windows.lock().expect("rate-limit lock poisoned");
        let now = Instant::now();
        let entry = guard
            .entry(key.to_string())
            .or_insert_with(|| Window { start: now, count: 0 });

        // Fixed window: reset when the window has elapsed.
        if now.duration_since(entry.start) > window {
            *entry = Window { start: now, count: 0 };
        }
        if entry.count >= limit {
            let remaining = window
                .saturating_sub(now.duration_since(entry.start))
                .as_secs();
            return Err(remaining.max(1));
        }
        entry.count += 1;
        Ok(())
    }

    /// Best-effort eviction so the map can't grow without bound from
    /// attackers cycling spoofed X-Forwarded-For headers.
    pub fn prune(&self, window: Duration) {
        let mut guard = self.windows.lock().expect("rate-limit lock poisoned");
        let now = Instant::now();
        guard.retain(|_, w| now.duration_since(w.start) <= window);
    }
}

pub const LIMIT_PER_MINUTE: u32 = 120;
pub const WINDOW: Duration = Duration::from_secs(60);

fn effective_client(req: &Request<Body>) -> String {
    if let Some(fwd) = req.headers().get("x-forwarded-for").and_then(|v| v.to_str().ok()) {
        if let Some(first) = fwd.split(',').next() {
            let trimmed = first.trim();
            if !trimmed.is_empty() {
                return trimmed.to_string();
            }
        }
    }
    req.extensions()
        .get::<axum::extract::ConnectInfo<std::net::SocketAddr>>()
        .map(|ci| ci.0.ip().to_string())
        .unwrap_or_else(|| "unknown".to_string())
}

pub async fn rate_limit_middleware(
    Extension(limiter): Extension<RateLimiter>,
    req: Request<Body>,
    next: Next,
) -> AxumResponse {
    let key = effective_client(&req);

    match limiter.check(&key, LIMIT_PER_MINUTE, WINDOW) {
        Ok(()) => next.run(req).await,
        Err(retry_after) => {
            tracing::warn!("rate limit hit for client {key}; retry in {retry_after}s");
            let mut res = Response::new(Body::from(
                "Rate limit exceeded. Slow down and try again shortly.",
            ));
            *res.status_mut() = StatusCode::TOO_MANY_REQUESTS;
            if let Ok(v) = retry_after.to_string().parse() {
                res.headers_mut().insert("retry-after", v);
            }
            res
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{http::Request, routing::get, Router};
    use tower::ServiceExt;

    /// Regression test for the 500 "Missing request extension" bug: the
    /// Extension(rate_limiter) layer must be applied AFTER (outside) the
    /// middleware so the middleware can extract it. If the order is wrong,
    /// even the first request 500s instead of reaching the handler.
    #[tokio::test]
    async fn middleware_resolves_extension_when_layered_last() {
        let limiter = RateLimiter::new();
        let app = Router::new()
            .route("/", get(|| async { "ok" }))
            .layer(axum::middleware::from_fn(rate_limit_middleware))
            .layer(Extension(limiter));

        // First request must succeed (not 500 with "Missing request extension").
        let res = app
            .clone()
            .oneshot(Request::builder().uri("/").body(Body::from("")).unwrap())
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::OK, "first request must pass the middleware");

        // Burst to the limit → the next request returns 429. The first request
        // above already consumed 1 of the budget, so LIMIT_PER_MINUTE-1 more
        // stay within budget and the one after that is rejected.
        let mut last: StatusCode = StatusCode::OK;
        for _ in 1..LIMIT_PER_MINUTE {
            last = app
                .clone()
                .oneshot(Request::builder().uri("/").body(Body::from("")).unwrap())
                .await
                .unwrap()
                .status();
        }
        assert_eq!(last, StatusCode::OK, "requests up to the limit stay within budget");
        let over = app
            .clone()
            .oneshot(Request::builder().uri("/").body(Body::from("")).unwrap())
            .await
            .unwrap();
        assert_eq!(over.status(), StatusCode::TOO_MANY_REQUESTS, "request past the limit is 429");
    }
}
