use sqlx::Row;
use uuid::Uuid;

use crate::db::models::{Commit, IntentNode, Repo};
use crate::errors::AppError;
use sqlx::PgPool;

pub async fn insert_repo(pool: &PgPool, github_url: &str, owner: &str, name: &str) -> Result<Repo, AppError> {
    let row = sqlx::query(
        "INSERT INTO repos (github_url, owner, name, status)
         VALUES ($1, $2, $3, 'pending')
         RETURNING id, github_url, owner, name, analyzed_at, status",
    )
    .bind(github_url).bind(owner).bind(name)
    .fetch_one(pool).await?;

    Ok(Repo {
        id:          row.try_get("id")?,
        github_url:  row.try_get("github_url")?,
        owner:       row.try_get("owner").ok(),
        name:        row.try_get("name").ok(),
        analyzed_at: row.try_get("analyzed_at").ok(),
        status:      row.try_get("status").ok(),
    })
}

pub async fn get_repo_by_id(pool: &PgPool, repo_id: Uuid) -> Result<Repo, AppError> {
    let row = sqlx::query(
        "SELECT id, github_url, owner, name, analyzed_at, status FROM repos WHERE id = $1"
    )
    .bind(repo_id)
    .fetch_optional(pool).await?
    .ok_or_else(|| AppError::NotFound("Repository not found".into()))?;

    Ok(Repo {
        id:          row.try_get("id")?,
        github_url:  row.try_get("github_url")?,
        owner:       row.try_get("owner").ok(),
        name:        row.try_get("name").ok(),
        analyzed_at: row.try_get("analyzed_at").ok(),
        status:      row.try_get("status").ok(),
    })
}

pub async fn insert_commit(pool: &PgPool, commit: &Commit) -> Result<(), AppError> {
    sqlx::query(
        "INSERT INTO commits (id, repo_id, sha, message, author, timestamp, files_changed)
         VALUES ($1, $2, $3, $4, $5, $6, $7)",
    )
    .bind(commit.id).bind(commit.repo_id).bind(&commit.sha)
    .bind(&commit.message).bind(&commit.author)
    .bind(commit.timestamp).bind(&commit.files_changed)
    .execute(pool).await?;
    Ok(())
}

pub async fn insert_intent_node(pool: &PgPool, node: &IntentNode) -> Result<(), AppError> {
    sqlx::query(
        "INSERT INTO intent_nodes
           (id, repo_id, node_type, title, summary, reasoning,
            contributors, source_refs, timestamp, confidence)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)",
    )
    .bind(node.id).bind(node.repo_id).bind(&node.node_type)
    .bind(&node.title).bind(&node.summary).bind(&node.reasoning)
    .bind(&node.contributors).bind(&node.source_refs)
    .bind(node.timestamp).bind(node.confidence)
    .execute(pool).await?;
    Ok(())
}
