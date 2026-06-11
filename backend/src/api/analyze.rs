use axum::{extract::State, http::HeaderMap, Json};
use chrono::NaiveDateTime;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::sync::Arc;
use uuid::Uuid;

use crate::errors::AppError;
use crate::ingestion::{commits, issues, pull_requests};
use crate::intelligence::{client as nim, prompts};
use crate::AppState;

#[derive(Debug, Deserialize)]
pub struct AnalyzeRequest {
    pub repo_url: String,
}

#[derive(Debug, Serialize)]
pub struct AnalyzeResponse {
    pub job_id: String,
    pub status: String,
    pub message: String,
}

pub async fn analyze_repo(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(payload): Json<AnalyzeRequest>,
) -> Result<Json<AnalyzeResponse>, AppError> {
    let github_token = headers
        .get("X-Github-Token")
        .and_then(|v| v.to_str().ok())
        .ok_or(AppError::MissingCredentials)?
        .to_string();

    let nim_api_key = headers
        .get("X-Nim-Api-Key")
        .and_then(|v| v.to_str().ok())
        .ok_or(AppError::MissingCredentials)?
        .to_string();

    let (owner, name) = parse_github_url(&payload.repo_url).ok_or_else(|| {
        AppError::BadRequest("Invalid GitHub URL — expected https://github.com/owner/repo".into())
    })?;

    // Insert repo record, get back the UUID
    let repo_id: Uuid = sqlx::query_scalar(
        "INSERT INTO repos (github_url, owner, name, status)
         VALUES ($1, $2, $3, 'processing')
         RETURNING id",
    )
    .bind(&payload.repo_url)
    .bind(&owner)
    .bind(&name)
    .fetch_one(&state.db)
    .await?;

    // Spawn background analysis
    let state2     = Arc::clone(&state);
    let owner2     = owner.clone();
    let name2      = name.clone();
    let token2     = github_token.clone();
    let nim_key2   = nim_api_key.clone();

    tokio::spawn(async move {
        let ok = run_analysis(&state2, repo_id, &owner2, &name2, &token2, &nim_key2)
            .await
            .is_ok();
        let status = if ok { "complete" } else { "failed" };
        let _ = sqlx::query("UPDATE repos SET status = $1, analyzed_at = NOW() WHERE id = $2")
            .bind(status)
            .bind(repo_id)
            .execute(&state2.db)
            .await;
    });

    Ok(Json(AnalyzeResponse {
        job_id:  repo_id.to_string(),
        status:  "processing".into(),
        message: format!("Analysis started for {}/{}", owner, name),
    }))
}

async fn run_analysis(
    state:        &Arc<AppState>,
    repo_id:      Uuid,
    owner:        &str,
    repo:         &str,
    github_token: &str,
    nim_api_key:  &str,
) -> Result<(), AppError> {
    let all_commits = commits::fetch_all_commits(&state.http_client, github_token, owner, repo).await?;
    let all_prs     = pull_requests::fetch_all_pull_requests(&state.http_client, github_token, owner, repo).await?;
    let all_issues  = issues::fetch_all_issues(&state.http_client, github_token, owner, repo).await?;

    for c in &all_commits {
        let ts         = parse_github_datetime(&c.commit.author.date);
        let files_json = c.files.as_ref().map(|f| json!(f));
        sqlx::query(
            "INSERT INTO commits (repo_id, sha, message, author, timestamp, files_changed)
             VALUES ($1, $2, $3, $4, $5, $6)
             ON CONFLICT DO NOTHING",
        )
        .bind(repo_id)
        .bind(&c.sha)
        .bind(&c.commit.message)
        .bind(c.author.as_ref().and_then(|a| a.login.as_deref()))
        .bind(ts)
        .bind(files_json)
        .execute(&state.db)
        .await?;
    }

    for pr in &all_prs {
        let created = parse_github_datetime(&pr.created_at);
        let merged  = pr.merged_at.as_deref().and_then(parse_github_datetime);
        sqlx::query(
            "INSERT INTO pull_requests (repo_id, number, title, body, state, author, created_at, merged_at)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
             ON CONFLICT DO NOTHING",
        )
        .bind(repo_id)
        .bind(pr.number as i32)
        .bind(&pr.title)
        .bind(&pr.body)
        .bind(&pr.state)
        .bind(pr.user.as_ref().map(|u| u.login.as_str()))
        .bind(created)
        .bind(merged)
        .execute(&state.db)
        .await?;
    }

    for issue in &all_issues {
        let created = parse_github_datetime(&issue.created_at);
        sqlx::query(
            "INSERT INTO issues (repo_id, number, title, body, state, author, created_at)
             VALUES ($1, $2, $3, $4, $5, $6, $7)
             ON CONFLICT DO NOTHING",
        )
        .bind(repo_id)
        .bind(issue.number as i32)
        .bind(&issue.title)
        .bind(&issue.body)
        .bind(&issue.state)
        .bind(issue.user.as_ref().map(|u| u.login.as_str()))
        .bind(created)
        .execute(&state.db)
        .await?;
    }

    let events_payload = json!({
        "repository": format!("{}/{}", owner, repo),
        "commits": all_commits.iter().take(50).map(|c| json!({
            "sha": &c.sha, "message": &c.commit.message,
            "author": c.author.as_ref().and_then(|a| a.login.as_deref()).unwrap_or("unknown"),
            "date": &c.commit.author.date
        })).collect::<Vec<_>>(),
        "pull_requests": all_prs.iter().take(30).map(|pr| json!({
            "number": pr.number, "title": &pr.title,
            "body": pr.body.as_deref().unwrap_or(""),
            "state": &pr.state,
            "author": pr.user.as_ref().map(|u| u.login.as_str()).unwrap_or("unknown"),
            "created_at": &pr.created_at, "merged_at": &pr.merged_at
        })).collect::<Vec<_>>(),
        "issues": all_issues.iter().take(30).map(|i| json!({
            "number": i.number, "title": &i.title,
            "body": i.body.as_deref().unwrap_or(""),
            "state": &i.state,
            "author": i.user.as_ref().map(|u| u.login.as_str()).unwrap_or("unknown"),
            "created_at": &i.created_at
        })).collect::<Vec<_>>()
    });

    let prompt   = prompts::build_analysis_prompt(&events_payload.to_string());
    let insights = nim::analyze_events_with_prompt(&state.http_client, nim_api_key, &prompt).await?;

    for insight in &insights {
        sqlx::query(
            "INSERT INTO intent_nodes
               (repo_id, node_type, title, summary, reasoning, contributors, source_refs, confidence)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8)",
        )
        .bind(repo_id)
        .bind(&insight.node_type)
        .bind(&insight.title)
        .bind(&insight.summary)
        .bind(&insight.reasoning)
        .bind(json!(&insight.contributors))
        .bind(json!(&insight.source_refs))
        .bind(insight.confidence as f64)
        .execute(&state.db)
        .await?;
    }

    Ok(())
}

fn parse_github_url(url: &str) -> Option<(String, String)> {
    let url   = url.trim_end_matches(".git").trim_end_matches('/');
    let parts: Vec<&str> = url.split('/').collect();
    if parts.len() >= 2 {
        Some((parts[parts.len() - 2].to_string(), parts[parts.len() - 1].to_string()))
    } else {
        None
    }
}

fn parse_github_datetime(s: &str) -> Option<NaiveDateTime> {
    chrono::DateTime::parse_from_rfc3339(s)
        .ok()
        .map(|dt| dt.naive_utc())
}
