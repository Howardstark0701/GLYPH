mod api;
mod errors;
mod ingestion;
mod intelligence;

use axum::{http::HeaderValue, routing::get, Router};
use std::collections::HashMap;
use std::sync::{Arc, atomic::AtomicBool};
use tower_http::cors::{Any, CorsLayer};
use tower_http::limit::RequestBodyLimitLayer;
use uuid::Uuid;

use axum::extract::Extension;
use axum::http::Method;

pub struct AppState {
    pub db:           PgPool,
    pub http_client:  reqwest::Client,
    /// Cancel flags for in-flight analyses, keyed by repo id. The terminate
    /// endpoint flips the flag; run_analysis checks it between phases.
    pub cancellations: std::sync::Mutex<HashMap<Uuid, Arc<AtomicBool>>>,
}

use sqlx::PgPool;

#[tokio::main]
async fn main() {
    // Initialise tracing
    tracing_subscriber::fmt()
        .with_env_filter(
            std::env::var("RUST_LOG").unwrap_or_else(|_| "glyph=info,tower_http=info".into())
        )
        .init();

    // Database ─────────────────────────────────────────────
    let database_url = std::env::var("DATABASE_URL")
        .expect("DATABASE_URL must be set");

    let pool = PgPool::connect(&database_url)
        .await
        .expect("Failed to connect to PostgreSQL");

    // Run migrations on startup
    sqlx::migrate!()
        .run(&pool)
        .await
        .expect("Failed to run database migrations");

    tracing::info!("Database migrations complete");

    // ── Stale in-flight job recovery ────────────────────────────
    // Any repo still 'processing' when the server comes up lost its
    // background task to the previous process. Mark it failed with an
    // honest error so it never sits in 'processing' forever; the user
    // can re-run it (the job is idempotent — ON CONFLICT DO NOTHING).
    let stale = sqlx::query(
        "UPDATE repos
            SET status = 'failed',
                error_message = 'Analysis interrupted by server restart — please re-run the job.',
                stage = 'idle',
                analyzed_at = NOW()
          WHERE status = 'processing'",
    )
        .execute(&pool)
        .await
        .expect("Failed to mark stale processing jobs");
    if stale.rows_affected() > 0 {
        tracing::warn!(
            "Marked {} stale in-flight job(s) as failed after restart",
            stale.rows_affected()
        );
    }

    // App state ────────────────────────────────────────────
    let state = Arc::new(AppState {
        db: pool,
        http_client: reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(60))
            .build()
            .expect("Failed to build HTTP client"),
        cancellations: std::sync::Mutex::new(HashMap::new()),
    });

    // CORS ─────────────────────────────────────────────────
    // Always allow localhost dev; read the production frontend origin(s) from
    // env. Accepts a comma-separated list (FRONTEND_URLS) and the legacy
    // single-origin FRONTEND_URL. If neither is configured, ANY origin is
    // allowed — safe for GLYPH because it has no cookies/sessions: BYOK keys
    // travel per-request as headers and are never stored server-side, so a
    // cross-origin caller gains nothing it couldn't do with its own keys.
    let mut origins: Vec<HeaderValue> = vec![
        "http://localhost:4321".parse::<HeaderValue>().unwrap(),
        "http://localhost:3000".parse::<HeaderValue>().unwrap(),
    ];
    let mut configured_origins = 0;
    for key in ["FRONTEND_URLS", "FRONTEND_URL"] {
        if let Ok(val) = std::env::var(key) {
            for entry in val.split(',') {
                let entry = entry.trim();
                if entry.is_empty() {
                    continue;
                }
                if let Ok(v) = entry.parse::<HeaderValue>() {
                    origins.push(v);
                    configured_origins += 1;
                    tracing::info!("CORS origin added: {}", entry);
                }
            }
        }
    }

    let cors = if configured_origins == 0 {
        tracing::warn!("No FRONTEND_URL(S) configured — allowing all origins (BYOK, no cookies)");
        CorsLayer::new()
            .allow_origin(Any)
            .allow_methods([Method::GET, Method::POST, Method::OPTIONS])
            .allow_headers(Any)
    } else {
        CorsLayer::new()
            .allow_origin(origins)
            .allow_methods([Method::GET, Method::POST, Method::OPTIONS])
            .allow_headers(Any)
    };

    // Router ───────────────────────────────────────────────
    // Health probe + the whole API surface (all routes live in api::routes).
    //
    // Middleware stack on /api (BYOK abuse protection + body hygiene):
    //   1. RequestBodyLimitLayer  — reject oversized bodies (64 KB cap)
    //   2. rate_limit_middleware  — fixed-window per-client rate limit → 429
    //   3. Extension(rate_limiter) — injects the limiter into the middleware
    // The Extension must sit inside the middleware so it can read it.
    let rate_limiter = api::rate_limit::RateLimiter::new();

    // Periodic eviction of stale windows so the map can't grow unbounded.
    {
        let limiter = Arc::new(rate_limiter.clone());
        tokio::spawn(async move {
            loop {
                tokio::time::sleep(std::time::Duration::from_secs(300)).await;
                limiter.prune(api::rate_limit::WINDOW);
            }
        });
    }

    let api_router = api::routes(Arc::clone(&state))
        .layer(Extension(rate_limiter))
        .layer(axum::middleware::from_fn(api::rate_limit::rate_limit_middleware))
        .layer(RequestBodyLimitLayer::new(1024 * 64));

    let router = Router::new()
        .route("/health", get(|| async { "OK" }))
        .nest("/api", api_router)
        .layer(cors);

    // Bind ─────────────────────────────────────────────────
    let port = std::env::var("PORT").unwrap_or_else(|_| "8000".into());
    let addr = format!("0.0.0.0:{}", port);
    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .expect("Failed to bind port");

    tracing::info!("GLYPH backend listening on {}", addr);
    axum::serve(
        listener,
        router.into_make_service_with_connect_info::<std::net::SocketAddr>(),
    )
        .await
        .expect("Server error");
}
