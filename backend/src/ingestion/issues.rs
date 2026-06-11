use crate::errors::AppError;
use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct Issue {
    pub number: i64,
    pub title: String,
    pub body: Option<String>,
    pub state: String,
    pub user: Option<IssueUser>,
    pub labels: Vec<Label>,
    pub comments: i64,
    pub created_at: String,
    pub closed_at: Option<String>,
    pub html_url: String,
}

#[derive(Debug, Deserialize)]
pub struct IssueUser {
    pub login: String,
    pub id: u64,
}

#[derive(Debug, Deserialize)]
pub struct Label {
    pub name: String,
    pub color: String,
}

#[derive(Debug, Deserialize)]
pub struct Comment {
    pub id: u64,
    pub body: String,
    pub user: Option<IssueUser>,
    pub created_at: String,
}

const MAX_PAGES: u32 = 10;
const PER_PAGE:  u32 = 100;

/// Fetch all issues across pages (excludes PRs via filter)
pub async fn fetch_all_issues(
    client: &reqwest::Client,
    token:  &str,
    owner:  &str,
    repo:   &str,
) -> Result<Vec<Issue>, AppError> {
    let mut all  = Vec::new();
    let mut page = 1u32;

    loop {
        // filter=issues excludes pull requests from the issues endpoint
        let url = format!(
            "https://api.github.com/repos/{}/{}/issues?state=all&filter=all&per_page={}&page={}",
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
                "GitHub issues API returned {} on page {}",
                resp.status(), page
            )));
        }

        let batch: Vec<Issue> = resp.json().await?;
        let done = batch.len() < PER_PAGE as usize;
        all.extend(batch);

        if done || page >= MAX_PAGES { break; }
        page += 1;
    }

    Ok(all)
}

/// Fetch comments for a single issue
pub async fn fetch_issue_comments(
    client:       &reqwest::Client,
    token:        &str,
    owner:        &str,
    repo:         &str,
    issue_number: i64,
) -> Result<Vec<Comment>, AppError> {
    let url = format!(
        "https://api.github.com/repos/{}/{}/issues/{}/comments",
        owner, repo, issue_number
    );

    let resp = client
        .get(&url)
        .header("Authorization", format!("Bearer {}", token))
        .header("User-Agent", "glyph/0.1")
        .send()
        .await?;

    if !resp.status().is_success() {
        return Err(AppError::GitHubApiError(format!(
            "GitHub issue comments API returned {}",
            resp.status()
        )));
    }

    Ok(resp.json().await?)
}

/// Single-page fetch kept for backward compat
pub async fn fetch_issues(
    client: &reqwest::Client,
    token:  &str,
    owner:  &str,
    repo:   &str,
) -> Result<Vec<Issue>, AppError> {
    fetch_all_issues(client, token, owner, repo).await
}
