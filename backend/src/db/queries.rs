use uuid::Uuid;

use crate::db::models::{Commit, IntentNode, Repo};
use crate::errors::AppError;
use sqlx::PgPool;

pub async fn insert_repo(pool: &PgPool, github_url: &str, owner: &str, name: &str) -> Result<Repo, AppError> {
    let repo = sqlx::query_as_unchecked!(
        Repo,
        "INSERT INTO repos (github_url, owner, name, status)
         VALUES ($1, $2, $3, 'pending')
         RETURNING *",
        github_url, owner, name
    )
    .fetch_one(pool)
    .await?;
    Ok(repo)
}

pub async fn get_repo_by_id(pool: &PgPool, repo_id: Uuid) -> Result<Repo, AppError> {
    let repo = sqlx::query_as_unchecked!(
        Repo,
        "SELECT * FROM repos WHERE id = $1",
        repo_id
    )
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| AppError::NotFound("Repository not found".into()))?;
    Ok(repo)
}

pub async fn insert_commit(pool: &PgPool, commit: &Commit) -> Result<(), AppError> {
    sqlx::query_unchecked!(
        "INSERT INTO commits (id, repo_id, sha, message, author, timestamp, files_changed)
         VALUES ($1, $2, $3, $4, $5, $6, $7)",
        commit.id, commit.repo_id, commit.sha, commit.message,
        commit.author, commit.timestamp, commit.files_changed
    )
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn insert_intent_node(pool: &PgPool, node: &IntentNode) -> Result<(), AppError> {
    sqlx::query_unchecked!(
        "INSERT INTO intent_nodes
           (id, repo_id, node_type, title, summary, reasoning,
            contributors, source_refs, timestamp, confidence)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)",
        node.id, node.repo_id, node.node_type, node.title,
        node.summary, node.reasoning, node.contributors,
        node.source_refs, node.timestamp, node.confidence
    )
    .execute(pool)
    .await?;
    Ok(())
}
