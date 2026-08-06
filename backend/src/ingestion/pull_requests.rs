use crate::errors::AppError;
use crate::ingestion::gh_get;
use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct PullRequest {
    pub number: i64,
    pub title: String,
    pub body: Option<String>,
    pub state: String,
    pub user: Option<PRUser>,
    pub created_at: String,
    pub merged_at: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct PRUser {
    pub login: String,
}

/// A formal PR review (approval / request-changes / comment thread).
#[derive(Debug, Deserialize)]
pub struct PullRequestReview {
    pub id: u64,
    pub state: String,
    pub body: Option<String>,
    pub user: Option<PRUser>,
    pub submitted_at: Option<String>,
}

/// An inline comment on a PR diff.
#[derive(Debug, Deserialize)]
pub struct PullRequestComment {
    pub id: u64,
    pub body: String,
    pub path: Option<String>,
    pub user: Option<PRUser>,
    pub created_at: String,
}

const MAX_PAGES: u32 = 10;
const PER_PAGE:  u32 = 100;
/// Bounded sweep over a single PR/issue's review thread — enough to capture the
/// real debate without hammering the rate limit on huge PRs.
const THREAD_PAGES: u32 = 3;

/// Fetch all pull requests across pages
pub async fn fetch_all_pull_requests(
    client: &reqwest::Client,
    token:  &str,
    owner:  &str,
    repo:   &str,
) -> Result<Vec<PullRequest>, AppError> {
    let mut all  = Vec::new();
    let mut page = 1u32;

    loop {
        let url = format!(
            "https://api.github.com/repos/{}/{}/pulls?state=all&per_page={}&page={}",
            owner, repo, PER_PAGE, page
        );

        let resp   = gh_get(client, token, &url).await?;
        let batch: Vec<PullRequest> = resp.json().await?;
        let done = batch.len() < PER_PAGE as usize;
        all.extend(batch);

        if done || page >= MAX_PAGES { break; }
        page += 1;
    }

    Ok(all)
}

/// Fetch formal reviews for a single PR.
pub async fn fetch_pr_reviews(
    client:   &reqwest::Client,
    token:    &str,
    owner:    &str,
    repo:     &str,
    pr_number: i64,
) -> Result<Vec<PullRequestReview>, AppError> {
    let mut all  = Vec::new();
    let mut page = 1u32;

    loop {
        let url = format!(
            "https://api.github.com/repos/{}/{}/pulls/{}/reviews?per_page={}&page={}",
            owner, repo, pr_number, PER_PAGE, page
        );

        let resp   = gh_get(client, token, &url).await?;
        let batch: Vec<PullRequestReview> = resp.json().await?;
        let done = batch.len() < PER_PAGE as usize;
        all.extend(batch);

        if done || page >= THREAD_PAGES { break; }
        page += 1;
    }

    Ok(all)
}

/// Fetch inline review comments for a single PR.
pub async fn fetch_pr_comments(
    client:    &reqwest::Client,
    token:     &str,
    owner:     &str,
    repo:      &str,
    pr_number: i64,
) -> Result<Vec<PullRequestComment>, AppError> {
    let mut all  = Vec::new();
    let mut page = 1u32;

    loop {
        let url = format!(
            "https://api.github.com/repos/{}/{}/pulls/{}/comments?per_page={}&page={}",
            owner, repo, pr_number, PER_PAGE, page
        );

        let resp   = gh_get(client, token, &url).await?;
        let batch: Vec<PullRequestComment> = resp.json().await?;
        let done = batch.len() < PER_PAGE as usize;
        all.extend(batch);

        if done || page >= THREAD_PAGES { break; }
        page += 1;
    }

    Ok(all)
}
