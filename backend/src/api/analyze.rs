use axum::{extract::State, http::HeaderMap, Json};
use chrono::NaiveDateTime;
use serde::{Deserialize, Serialize};
use serde_json::json;
use sqlx::PgPool;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use uuid::Uuid;

use crate::errors::AppError;
use crate::ingestion::{commits, issues, pull_requests};
use crate::intelligence::{client as nim, prompts};
use crate::processing::linker;
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

    // Defense-in-depth: cap URL length before any further processing.
    if payload.repo_url.trim().len() > 2048 {
        return Err(AppError::BadRequest("repo_url is too long".into()));
    }

    let (owner, name) = parse_github_url(&payload.repo_url).ok_or_else(|| {
        AppError::BadRequest(
            "Invalid GitHub URL — expected https://github.com/owner/repo".into(),
        )
    })?;

    // Insert repo record, get back the UUID
    let repo_id: Uuid = sqlx::query_scalar(
        "INSERT INTO repos (github_url, owner, name, status, stage)
         VALUES ($1, $2, $3, 'processing', 'queued')
         RETURNING id",
    )
    .bind(&payload.repo_url)
    .bind(&owner)
    .bind(&name)
    .fetch_one(&state.db)
    .await?;

    // Register a cancellation flag the terminate endpoint can flip.
    let cancel = Arc::new(AtomicBool::new(false));
    state
        .cancellations
        .lock()
        .unwrap()
        .insert(repo_id, Arc::clone(&cancel));

    // Spawn background analysis
    let state2   = Arc::clone(&state);
    let owner2   = owner.clone();
    let name2    = name.clone();
    let token2   = github_token.clone();
    let nim_key2 = nim_api_key.clone();

    tokio::spawn(async move {
        let outcome = run_analysis(&state2, repo_id, &owner2, &name2, &token2, &nim_key2, cancel)
            .await;

        let (status, error) = match outcome {
            Ok(()) => ("complete", None),
            Err(AppError::Cancelled) => {
                ("terminated", Some(AppError::Cancelled.to_string()))
            }
            Err(e) => ("failed", Some(e.to_string())),
        };

        let _ = sqlx::query(
            "UPDATE repos SET status = $1, error_message = $2, analyzed_at = NOW(), stage = 'idle'
             WHERE id = $3",
        )
        .bind(status)
        .bind(&error)
        .bind(repo_id)
        .execute(&state2.db)
        .await;

        // Release the cancellation handle.
        state2.cancellations.lock().unwrap().remove(&repo_id);
    });

    Ok(Json(AnalyzeResponse {
        job_id:  repo_id.to_string(),
        status:  "processing".into(),
        message: format!("Analysis started for {}/{}", owner, name),
    }))
}

#[allow(clippy::too_many_arguments)]
async fn run_analysis(
    state:        &Arc<AppState>,
    repo_id:      Uuid,
    owner:        &str,
    repo:         &str,
    github_token: &str,
    nim_api_key:  &str,
    cancel:       Arc<AtomicBool>,
) -> Result<(), AppError> {
    let db = &state.db;

    // ── commits ────────────────────────────────────────────────
    cancelled(&cancel)?;
    set_stage(db, repo_id, "ingesting_commits").await;
    let all_commits = commits::fetch_all_commits(&state.http_client, github_token, owner, repo)
        .await?;
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
        .execute(db)
        .await?;
    }

    // ── pull requests ──────────────────────────────────────────
    cancelled(&cancel)?;
    set_stage(db, repo_id, "ingesting_pull_requests").await;
    let all_prs =
        pull_requests::fetch_all_pull_requests(&state.http_client, github_token, owner, repo)
            .await?;
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
        .execute(db)
        .await?;
    }

    // ── issues ─────────────────────────────────────────────────
    cancelled(&cancel)?;
    set_stage(db, repo_id, "ingesting_issues").await;
    let all_issues =
        issues::fetch_all_issues(&state.http_client, github_token, owner, repo).await?;
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
        .execute(db)
        .await?;
    }

    // ── review threads + comments (the debate evidence) ────────
    cancelled(&cancel)?;
    set_stage(db, repo_id, "ingesting_review_threads").await;
    let (reviews_by_pr, comments_by_pr, comments_by_issue) =
        fetch_review_threads(&state.http_client, github_token, owner, repo, &all_prs, &all_issues)
            .await;

    for (pr, reviews) in &reviews_by_pr {
        for rv in reviews {
            sqlx::query(
                "INSERT INTO pull_request_reviews (repo_id, pr_number, github_id, \"user\", state, body, submitted_at)
                 VALUES ($1, $2, $3, $4, $5, $6, $7)
                 ON CONFLICT DO NOTHING",
            )
            .bind(repo_id)
            .bind(*pr as i32)
            .bind(rv.id as i64)
            .bind(rv.user.as_ref().map(|u| u.login.as_str()))
            .bind(&rv.state)
            .bind(&rv.body)
            .bind(rv.submitted_at.as_deref().and_then(parse_github_datetime))
            .execute(db)
            .await?;
        }
    }
    for (pr, comments) in &comments_by_pr {
        for c in comments {
            sqlx::query(
                "INSERT INTO pull_request_comments (repo_id, pr_number, github_id, \"user\", body, path, created_at)
                 VALUES ($1, $2, $3, $4, $5, $6, $7)
                 ON CONFLICT DO NOTHING",
            )
            .bind(repo_id)
            .bind(*pr as i32)
            .bind(c.id as i64)
            .bind(c.user.as_ref().map(|u| u.login.as_str()))
            .bind(&c.body)
            .bind(&c.path)
            .bind(parse_github_datetime(&c.created_at))
            .execute(db)
            .await?;
        }
    }
    for (issue, comments) in &comments_by_issue {
        for c in comments {
            sqlx::query(
                "INSERT INTO issue_comments (repo_id, issue_number, github_id, \"user\", body, created_at)
                 VALUES ($1, $2, $3, $4, $5, $6)
                 ON CONFLICT DO NOTHING",
            )
            .bind(repo_id)
            .bind(*issue as i32)
            .bind(c.id as i64)
            .bind(c.user.as_ref().map(|u| u.login.as_str()))
            .bind(&c.body)
            .bind(parse_github_datetime(&c.created_at))
            .execute(db)
            .await?;
        }
    }

    // ── NIM intent extraction ──────────────────────────────────
    cancelled(&cancel)?;
    set_stage(db, repo_id, "extracting_intent").await;

    let timeline = linker::build_timeline(repo_id, &all_commits, &all_prs, &all_issues).to_payload();

    let events_payload = json!({
        "repository": format!("{}/{}", owner, repo),
        "timeline": timeline,
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
        "pull_request_reviews": reviews_by_pr.iter().flat_map(|(pr, revs)| revs.iter().map(move |r| json!({
            "pr": pr, "state": &r.state, "body": r.body.as_deref().unwrap_or(""),
            "author": r.user.as_ref().map(|u| u.login.as_str()).unwrap_or("unknown"),
            "submitted_at": &r.submitted_at
        }))).take(120).collect::<Vec<_>>(),
        "pull_request_comments": comments_by_pr.iter().flat_map(|(pr, cs)| cs.iter().map(move |c| json!({
            "pr": pr, "path": &c.path, "body": &c.body,
            "author": c.user.as_ref().map(|u| u.login.as_str()).unwrap_or("unknown")
        }))).take(120).collect::<Vec<_>>(),
        "issues": all_issues.iter().take(30).map(|i| json!({
            "number": i.number, "title": &i.title,
            "body": i.body.as_deref().unwrap_or(""),
            "state": &i.state,
            "author": i.user.as_ref().map(|u| u.login.as_str()).unwrap_or("unknown"),
            "created_at": &i.created_at
        })).collect::<Vec<_>>(),
        "issue_comments": comments_by_issue.iter().flat_map(|(num, cs)| cs.iter().map(move |c| json!({
            "issue": num, "body": &c.body,
            "author": c.user.as_ref().map(|u| u.login.as_str()).unwrap_or("unknown")
        }))).take(120).collect::<Vec<_>>()
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
        .execute(db)
        .await?;
    }

    Ok(())
}

/// Fetch review threads for the PRs/issues that actually reach the NIM prompt.
/// A failure on one thread is logged and skipped, never fatal to the whole job.
async fn fetch_review_threads(
    client:     &reqwest::Client,
    token:      &str,
    owner:      &str,
    repo:       &str,
    all_prs:    &[pull_requests::PullRequest],
    all_issues: &[issues::Issue],
) -> (
    Vec<(i64, Vec<pull_requests::PullRequestReview>)>,
    Vec<(i64, Vec<pull_requests::PullRequestComment>)>,
    Vec<(i64, Vec<issues::Comment>)>,
) {
    let pr_numbers: Vec<i64> = all_prs.iter().take(30).map(|p| p.number).collect();
    let issue_numbers: Vec<i64> = all_issues.iter().take(30).map(|i| i.number).collect();

    let mut reviews = Vec::new();
    let mut pr_comments = Vec::new();
    let mut issue_comments = Vec::new();

    for num in &pr_numbers {
        match pull_requests::fetch_pr_reviews(client, token, owner, repo, *num).await {
            Ok(revs) => reviews.push((*num, revs)),
            Err(e) => tracing::warn!("PR {num} reviews skipped: {e}"),
        }
        match pull_requests::fetch_pr_comments(client, token, owner, repo, *num).await {
            Ok(cs) => pr_comments.push((*num, cs)),
            Err(e) => tracing::warn!("PR {num} comments skipped: {e}"),
        }
    }
    for num in &issue_numbers {
        match issues::fetch_issue_comments(client, token, owner, repo, *num).await {
            Ok(cs) => issue_comments.push((*num, cs)),
            Err(e) => tracing::warn!("issue {num} comments skipped: {e}"),
        }
    }

    (reviews, pr_comments, issue_comments)
}

async fn set_stage(db: &PgPool, repo_id: Uuid, stage: &str) {
    let _ = sqlx::query("UPDATE repos SET stage = $1 WHERE id = $2")
        .bind(stage)
        .bind(repo_id)
        .execute(db)
        .await;
}

fn cancelled(flag: &AtomicBool) -> Result<(), AppError> {
    if flag.load(Ordering::Relaxed) {
        Err(AppError::Cancelled)
    } else {
        Ok(())
    }
}

/// Strictly github.com URLs, exactly two path segments. Accepts http(s) and a
/// bare `github.com/...` prefix, plus optional `.git` / trailing slash.
pub fn parse_github_url(url: &str) -> Option<(String, String)> {
    let trimmed = url.trim();
    if trimmed.is_empty() {
        return None;
    }
    let stripped = trimmed
        .strip_suffix(".git")
        .unwrap_or(trimmed)
        .trim_end_matches('/');

    let rest = stripped
        .strip_prefix("https://github.com/")
        .or_else(|| stripped.strip_prefix("http://github.com/"))
        .or_else(|| stripped.strip_prefix("github.com/"))?;

    let mut parts = rest.split('/').filter(|s| !s.is_empty());
    let owner = parts.next()?;
    let name  = parts.next()?;
    // Reject extra path segments (e.g. /tree/main, /issues/1) and subdomains.
    if parts.next().is_some() || owner.contains('.') {
        return None;
    }
    Some((owner.to_string(), name.to_string()))
}

fn parse_github_datetime(s: &str) -> Option<NaiveDateTime> {
    chrono::DateTime::parse_from_rfc3339(s)
        .ok()
        .map(|dt| dt.naive_utc())
}

#[cfg(test)]
mod tests {
    use super::parse_github_url;

    #[test]
    fn parses_valid_https_url() {
        assert_eq!(
            parse_github_url("https://github.com/owner/repo"),
            Some(("owner".into(), "repo".into()))
        );
    }

    #[test]
    fn parses_git_suffix_and_trailing_slash() {
        assert_eq!(
            parse_github_url("https://github.com/owner/repo.git"),
            Some(("owner".into(), "repo".into()))
        );
        assert_eq!(
            parse_github_url("https://github.com/owner/repo/"),
            Some(("owner".into(), "repo".into()))
        );
    }

    #[test]
    fn parses_http_and_bare_prefix() {
        assert_eq!(
            parse_github_url("http://github.com/owner/repo"),
            Some(("owner".into(), "repo".into()))
        );
        assert_eq!(
            parse_github_url("github.com/owner/repo"),
            Some(("owner".into(), "repo".into()))
        );
    }

    #[test]
    fn rejects_foreign_hosts() {
        assert_eq!(parse_github_url("https://gitlab.com/owner/repo"), None);
        assert_eq!(parse_github_url("https://example.github.com/owner/repo"), None);
        assert_eq!(parse_github_url("https://github.com.evil.com/owner/repo"), None);
    }

    #[test]
    fn rejects_missing_or_extra_segments() {
        assert_eq!(parse_github_url("https://github.com/owner"), None);
        assert_eq!(parse_github_url("https://github.com/owner/repo/tree/main"), None);
        assert_eq!(parse_github_url("https://github.com/owner/repo/issues/1"), None);
        assert_eq!(parse_github_url(""), None);
        assert_eq!(parse_github_url("   "), None);
    }
}
