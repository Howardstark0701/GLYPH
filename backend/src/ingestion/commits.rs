use crate::errors::AppError;
use crate::ingestion::gh_get;
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize)]
pub struct GitHubCommit {
    pub sha: String,
    pub commit: CommitDetail,
    pub author: Option<AuthorInfo>,
    pub files: Option<Vec<FileChange>>,
}

#[derive(Debug, Deserialize)]
pub struct CommitDetail {
    pub message: String,
    pub author: CommitAuthor,
}

#[derive(Debug, Deserialize)]
pub struct CommitAuthor {
    pub date: String,
}

#[derive(Debug, Deserialize)]
pub struct AuthorInfo {
    pub login: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct FileChange {
    pub filename: String,
    pub status: String,
    pub additions: Option<i32>,
    pub deletions: Option<i32>,
    pub changes: Option<i32>,
}

const PER_PAGE: u32 = 100;

/// Maximum commits to ingest, configurable via MAX_COMMITS (default 1000).
/// For very large repos set MAX_COMMITS=10000 — this fetches 100 pages and
/// relies on the 403/429 retry/backoff in `gh_get` to stay within rate limits.
fn max_commits() -> u32 {
    std::env::var("MAX_COMMITS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(1000)
}

/// Fetch commits across pages (up to MAX_COMMITS).
pub async fn fetch_all_commits(
    client: &reqwest::Client,
    token:  &str,
    owner:  &str,
    repo:   &str,
) -> Result<Vec<GitHubCommit>, AppError> {
    let max_pages = (max_commits().max(1) + PER_PAGE - 1) / PER_PAGE;
    let mut all   = Vec::new();
    let mut page  = 1u32;

    loop {
        let url = format!(
            "https://api.github.com/repos/{}/{}/commits?per_page={}&page={}",
            owner, repo, PER_PAGE, page
        );

        let resp   = gh_get(client, token, &url).await?;
        let batch: Vec<GitHubCommit> = resp.json().await?;
        let done = batch.len() < PER_PAGE as usize;
        all.extend(batch);

        if done || page >= max_pages { break; }
        page += 1;
    }

    Ok(all)
}
