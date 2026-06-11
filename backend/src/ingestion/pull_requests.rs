use crate::errors::AppError;
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
    pub closed_at: Option<String>,
    pub html_url: String,
}

#[derive(Debug, Deserialize)]
pub struct PRUser {
    pub login: String,
    pub id: u64,
}

const MAX_PAGES: u32 = 10;
const PER_PAGE:  u32 = 100;

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

        let resp = client
            .get(&url)
            .header("Authorization", format!("Bearer {}", token))
            .header("User-Agent", "glyph/0.1")
            .send()
            .await?;

        if !resp.status().is_success() {
            return Err(AppError::GitHubApiError(format!(
                "GitHub PRs API returned {} on page {}",
                resp.status(), page
            )));
        }

        let batch: Vec<PullRequest> = resp.json().await?;
        let done = batch.len() < PER_PAGE as usize;
        all.extend(batch);

        if done || page >= MAX_PAGES { break; }
        page += 1;
    }

    Ok(all)
}

/// Single-page fetch kept for backward compat
pub async fn fetch_pull_requests(
    client: &reqwest::Client,
    token:  &str,
    owner:  &str,
    repo:   &str,
) -> Result<Vec<PullRequest>, AppError> {
    fetch_all_pull_requests(client, token, owner, repo).await
}
