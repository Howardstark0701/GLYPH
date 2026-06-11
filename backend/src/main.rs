mod api;
mod db;
mod errors;
mod ingestion;
mod intelligence;
mod processing;

use axum::{
    http::{HeaderValue, Method},
    Router,
    routing::get,
};
use sqlx::PgPool;
use std::sync::Arc;
use tower_http::cors::{Any, CorsLayer};

pub struct AppState {
    pub db:          PgPool,
    pub http_client: reqwest::Client,
}

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

    // App state ────────────────────────────────────────────
    let state = Arc::new(AppState {
        db: pool,
        http_client: reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .expect("Failed to build HTTP client"),
    });

    // CORS ─────────────────────────────────────────────────
    // Always allow localhost dev; read FRONTEND_URL from env for production
    let mut origins: Vec<HeaderValue> = vec![
        "http://localhost:4321".parse::<HeaderValue>().unwrap(),
        "http://localhost:3000".parse::<HeaderValue>().unwrap(),
    ];
    if let Ok(frontend_url) = std::env::var("FRONTEND_URL") {
        if let Ok(v) = frontend_url.parse::<HeaderValue>() {
            origins.push(v);
            tracing::info!("CORS origin added: {}", frontend_url);
        }
    }

    let cors = CorsLayer::new()
        .allow_origin(origins)
        .allow_methods([Method::GET, Method::POST, Method::OPTIONS])
        .allow_headers(Any);

    // Router ───────────────────────────────────────────────
    let router = Router::new()
        .route("/health", get(|| async { "OK" }))
        .nest("/api", api::routes(state))
        .layer(cors);

    // Bind ─────────────────────────────────────────────────
    let port = std::env::var("PORT").unwrap_or_else(|_| "8000".into());
    let addr = format!("0.0.0.0:{}", port);
    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .expect("Failed to bind port");

    tracing::info!("GLYPH backend listening on {}", addr);
    axum::serve(listener, router)
        .await
        .expect("Server error");
}
