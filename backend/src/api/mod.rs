pub mod analyze;
pub mod repo;

use axum::{routing::{get, post}, Router};
use std::sync::Arc;

use crate::AppState;

pub fn routes(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/analyze", post(analyze::analyze_repo))
        .route("/repo/{id}/status",       get(repo::get_status))
        .route("/repo/{id}/intent",       get(repo::get_intent))
        .route("/repo/{id}/debates",      get(repo::get_debates))
        .route("/repo/{id}/decisions",    get(repo::get_decisions))
        .route("/repo/{id}/rejections",   get(repo::get_rejections))
        .route("/repo/{id}/contributors", get(repo::get_contributors))
        .route("/repo/{id}/graph",        get(repo::get_graph))
        .route("/repo/{id}/summary",      get(repo::get_summary))
        .with_state(state)
}
