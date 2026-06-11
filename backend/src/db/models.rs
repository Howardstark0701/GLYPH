use chrono::NaiveDateTime;
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

#[derive(Debug, FromRow, Serialize, Deserialize)]
pub struct Repo {
    pub id: Uuid,
    pub github_url: String,
    pub owner: Option<String>,
    pub name: Option<String>,
    pub analyzed_at: Option<NaiveDateTime>,
    pub status: Option<String>,
}

#[derive(Debug, FromRow, Serialize, Deserialize)]
pub struct Commit {
    pub id: Uuid,
    pub repo_id: Uuid,
    pub sha: String,
    pub message: String,
    pub author: Option<String>,
    pub timestamp: Option<NaiveDateTime>,
    pub files_changed: Option<serde_json::Value>,
}

#[derive(Debug, FromRow, Serialize, Deserialize)]
pub struct PullRequest {
    pub id: Uuid,
    pub repo_id: Uuid,
    pub number: i32,
    pub title: String,
    pub body: Option<String>,
    pub state: Option<String>,
    pub author: Option<String>,
    pub created_at: Option<NaiveDateTime>,
    pub merged_at: Option<NaiveDateTime>,
}

#[derive(Debug, FromRow, Serialize, Deserialize)]
pub struct Issue {
    pub id: Uuid,
    pub repo_id: Uuid,
    pub number: i32,
    pub title: String,
    pub body: Option<String>,
    pub state: Option<String>,
    pub author: Option<String>,
    pub created_at: Option<NaiveDateTime>,
    pub comments: Option<serde_json::Value>,
}

#[derive(Debug, FromRow, Serialize, Deserialize)]
pub struct IntentNode {
    pub id: Uuid,
    pub repo_id: Uuid,
    pub node_type: Option<String>,
    pub title: Option<String>,
    pub summary: Option<String>,
    pub reasoning: Option<String>,
    pub contributors: Option<serde_json::Value>,
    pub source_refs: Option<serde_json::Value>,
    pub timestamp: Option<NaiveDateTime>,
    pub confidence: Option<f64>,
}
