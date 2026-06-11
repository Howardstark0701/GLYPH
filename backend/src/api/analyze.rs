use axum::{
    extract::State,
    http::HeaderMap,
    Json,
};
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
    // ── Extract BYOK credentials ──────────────────────────
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

    let (owner, name) = parse_github_url(&payload.repo_url)
        .ok_or_else(|| AppError::BadRequest("Invalid GitHub URL — expected https://github.com/owner/repo".into()))?;

    // ── Create repo record (pending) ──────────────────────
    let repo_id: Uuid = sqlx::query_scalar!(
        "INSERT INTO repos (github_url, owner, name, status)
         VALUES ($1, $2, $3, 'processing')
         RETURNING id",
        payload.repo_url,
        owner,
        name
    )
    .fetch_one(&state.db)
    .await?;

    // ── Spawn async analysis task — return job_id immediately ──
    let state_clone    = Arc::clone(&state);
    let owner_clone    = owner.clone();
    let name_clone     = name.clone();
    let token_clone    = github_token.clone();
    let nim_key_clone  = nim_api_key.clone();

    tokio::spawn(async move {
        let result = run_analysis(
            &state_clone,
            repo_id,
            &owner_clone,
            &name_clone,
            &token_clone,
            &nim_key_clone,
        )
        .await;

        let final_status = match result {
            Ok(_)  => "complete",
            Err(_) => "failed",
        };

        let _ = sqlx::query!(
            "UPDATE repos SET status = $1, analyzed_at = NOW() WHERE id = $2",
            final_status,
            repo_id
        )
        .execute(&state_clone.db)
        .await;
    });

    Ok(Json(AnalyzeResponse {
        job_id:  repo_id.to_string(),
        status:  "processing".into(),
        message: format!("Analysis started for {}/{}", owner, name),
    }))
}

// ── Core analysis pipeline ────────────────────────────────
async fn run_analysis(
    state:       &Arc<AppState>,
    repo_id:     Uuid,
    owner:       &str,
    repo:        &str,
    github_token: &str,
    nim_api_key:  &str,
) -> Result<(), AppError> {
    // 1. Ingest all pages of commits, PRs, issues ─────────
    let all_commits = commits::fetch_all_commits(&state.http_client, github_token, owner, repo).await?;
    let all_prs     = pull_requests::fetch_all_pull_requests(&state.http_client, github_token, owner, repo).await?;
    let all_issues  = issues::fetch_all_issues(&state.http_client, github_token, owner, repo).await?;

    // 2. Persist raw commits ──────────────────────────────
    for c in &all_commits {
        let ts = parse_github_datetime(&c.commit.author.date);
        let files_json = c.files.as_ref().map(|f| json!(f));
        sqlx::query!(
            "INSERT INTO commits (repo_id, sha, message, author, timestamp, files_changed)
             VALUES ($1, $2, $3, $4, $5, $6)
             ON CONFLICT DO NOTHING",
            repo_id,
            c.sha,
            c.commit.message,
            c.author.as_ref().and_then(|a| a.login.clone()),
            ts,
            files_json
        )
        .execute(&state.db)
        .await?;
    }

    // 3. Persist raw pull requests ────────────────────────
    for pr in &all_prs {
        let created = parse_github_datetime(&pr.created_at);
        let merged  = pr.merged_at.as_deref().and_then(parse_github_datetime);
        sqlx::query!(
            "INSERT INTO pull_requests (repo_id, number, title, body, state, author, created_at, merged_at)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
             ON CONFLICT DO NOTHING",
            repo_id,
            pr.number as i32,
            pr.title,
            pr.body,
            pr.state,
            pr.user.as_ref().map(|u| u.login.as_str()),
            created,
            merged
        )
        .execute(&state.db)
        .await?;
    }

    // 4. Persist raw issues ───────────────────────────────
    for issue in &all_issues {
        let created = parse_github_datetime(&issue.created_at);
        sqlx::query!(
            "INSERT INTO issues (repo_id, number, title, body, state, author, created_at)
             VALUES ($1, $2, $3, $4, $5, $6, $7)
             ON CONFLICT DO NOTHING",
            repo_id,
            issue.number as i32,
            issue.title,
            issue.body,
            issue.state,
            issue.user.as_ref().map(|u| u.login.as_str()),
            created
        )
        .execute(&state.db)
        .await?;
    }

    // 5. Build event payload for NIM ──────────────────────
    // Send batches of ≤30 events to stay within token limits
    let events_payload = json!({
        "repository": format!("{}/{}", owner, repo),
        "commits": all_commits.iter().take(50).map(|c| json!({
            "sha": &c.sha,
            "message": &c.commit.message,
            "author": c.author.as_ref().and_then(|a| a.login.as_deref()).unwrap_or("unknown"),
            "date": &c.commit.author.date
        })).collect::<Vec<_>>(),
        "pull_requests": all_prs.iter().take(30).map(|pr| json!({
            "number": pr.number,
            "title": &pr.title,
            "body": pr.body.as_deref().unwrap_or(""),
            "state": &pr.state,
            "author": pr.user.as_ref().map(|u| u.login.as_str()).unwrap_or("unknown"),
            "created_at": &pr.created_at,
            "merged_at": &pr.merged_at
        })).collect::<Vec<_>>(),
        "issues": all_issues.iter().take(30).map(|i| json!({
            "number": i.number,
            "title": &i.title,
            "body": i.body.as_deref().unwrap_or(""),
            "state": &i.state,
            "author": i.user.as_ref().map(|u| u.login.as_str()).unwrap_or("unknown"),
            "created_at": &i.created_at
        })).collect::<Vec<_>>()
    });

    // 6. Call NIM and extract insights ────────────────────
    let prompt  = prompts::build_analysis_prompt(&events_payload.to_string());
    let insights = nim::analyze_events_with_prompt(&state.http_client, nim_api_key, &prompt).await?;

    // 7. Persist intent nodes ─────────────────────────────
    for insight in &insights {
        sqlx::query!(
            "INSERT INTO intent_nodes
               (repo_id, node_type, title, summary, reasoning, contributors, source_refs, confidence)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8)",
            repo_id,
            insight.node_type,
            insight.title,
            insight.summary,
            insight.reasoning,
            json!(insight.contributors),
            json!(insight.source_refs),
            insight.confidence as f64
        )
        .execute(&state.db)
        .await?;
    }

    Ok(())
}

// ── Helpers ───────────────────────────────────────────────
fn parse_github_url(url: &str) -> Option<(String, String)> {
    let url = url.trim_end_matches(".git").trim_end_matches('/');
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
