use crate::errors::AppError;
use crate::ingestion::gh_get;
use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct Issue {
    pub number: i64,
    pub title: String,
    pub body: Option<String>,
    pub state: String,
    pub user: Option<IssueUser>,
    pub created_at: String,
}

#[derive(Debug, Deserialize)]
pub struct IssueUser {
    pub login: String,
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
/// Bounded sweep over a single issue's comment thread.
const THREAD_PAGES: u32 = 3;

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

        let resp   = gh_get(client, token, &url).await?;
        let batch: Vec<Issue> = resp.json().await?;
        let done = batch.len() < PER_PAGE as usize;
        all.extend(batch);

        if done || page >= MAX_PAGES { break; }
        page += 1;
    }

    Ok(all)
}

/// Fetch comments for a single issue (paginated, bounded)
pub async fn fetch_issue_comments(
    client:       &reqwest::Client,
    token:        &str,
    owner:        &str,
    repo:         &str,
    issue_number: i64,
) -> Result<Vec<Comment>, AppError> {
    let mut all  = Vec::new();
    let mut page = 1u32;

    loop {
        let url = format!(
            "https://api.github.com/repos/{}/{}/issues/{}/comments?per_page={}&page={}",
            owner, repo, issue_number, PER_PAGE, page
        );

        let resp   = gh_get(client, token, &url).await?;
        let batch: Vec<Comment> = resp.json().await?;
        let done = batch.len() < PER_PAGE as usize;
        all.extend(batch);

        if done || page >= THREAD_PAGES { break; }
        page += 1;
    }

    Ok(all)
}
