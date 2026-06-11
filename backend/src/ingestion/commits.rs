use crate::errors::AppError;
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
    pub name: String,
    pub email: String,
    pub date: String,
}

#[derive(Debug, Deserialize)]
pub struct AuthorInfo {
    pub login: Option<String>,
    pub id: Option<u64>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct FileChange {
    pub filename: String,
    pub status: String,
    pub additions: Option<i32>,
    pub deletions: Option<i32>,
    pub changes: Option<i32>,
}

const MAX_PAGES: u32 = 10; // cap at 1000 commits
const PER_PAGE:  u32 = 100;

/// Fetch all commits across pages (up to MAX_PAGES × PER_PAGE)
pub async fn fetch_all_commits(
    client: &reqwest::Client,
    token:  &str,
    owner:  &str,
    repo:   &str,
) -> Result<Vec<GitHubCommit>, AppError> {
    let mut all    = Vec::new();
    let mut page   = 1u32;

    loop {
        let url = format!(
            "https://api.github.com/repos/{}/{}/commits?per_page={}&page={}",
            owner, repo, PER_PAGE, page
        );

        let resp = client
            .get(&url)
            .header("Authorization", format!("Bearer {}", token))
            .header("User-Agent", "glyph/0.1")
            .send()
            .await?;

        if !resp.status().is_success() {
            return Err(AppError::GitHubApiError(format!(
                "GitHub commits API returned {} on page {}",
                resp.status(), page
            )));
        }

        let batch: Vec<GitHubCommit> = resp.json().await?;
        let done = batch.len() < PER_PAGE as usize;
        all.extend(batch);

        if done || page >= MAX_PAGES { break; }
        page += 1;
    }

    Ok(all)
}

/// Single-page fetch kept for backward compat
pub async fn fetch_commits(
    client: &reqwest::Client,
    token:  &str,
    owner:  &str,
    repo:   &str,
) -> Result<Vec<GitHubCommit>, AppError> {
    fetch_all_commits(client, token, owner, repo).await
}
