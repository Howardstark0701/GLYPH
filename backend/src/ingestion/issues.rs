use crate::errors::AppError;
use crate::ingestion::{gh_get, max_list_pages};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct Issue {
    pub number: i64,
    pub title: String,
    pub body: Option<String>,
    pub state: String,
    pub user: Option<IssueUser>,
    pub created_at: String,
    /// Present only when GitHub is describing a pull request.
    ///
    /// The repository issues endpoint returns pull requests alongside issues
    /// by design, and no query parameter on that endpoint turns it off. This
    /// marker is the documented way to tell them apart; without it every PR
    /// was ingested twice — once properly, and once as a fake issue.
    #[serde(default)]
    pub pull_request: Option<serde_json::Value>,
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

const PER_PAGE:  u32 = 100;
/// Bounded sweep over a single issue's comment thread.
const THREAD_PAGES: u32 = 3;

/// Fetch all issues across pages, excluding pull requests.
pub async fn fetch_all_issues(
    client: &reqwest::Client,
    token:  &str,
    owner:  &str,
    repo:   &str,
) -> Result<Vec<Issue>, AppError> {
    let mut all  = Vec::new();
    let mut page = 1u32;

    let mut skipped = 0usize;

    loop {
        // No `filter` param: it belongs to the *user* issues endpoint, does
        // nothing here, and its presence disguised the fact that pull
        // requests were being ingested as issues.
        let url = format!(
            "https://api.github.com/repos/{}/{}/issues?state=all&per_page={}&page={}",
            owner, repo, PER_PAGE, page
        );

        let resp   = gh_get(client, token, &url).await?;
        let batch: Vec<Issue> = resp.json().await?;
        let done = batch.len() < PER_PAGE as usize;

        let before = batch.len();
        let issues_only: Vec<Issue> =
            batch.into_iter().filter(|i| i.pull_request.is_none()).collect();
        skipped += before - issues_only.len();
        all.extend(issues_only);

        if done || page >= max_list_pages() { break; }
        page += 1;
    }

    if skipped > 0 {
        tracing::info!(
            "skipped {} pull request(s) returned by the issues endpoint",
            skipped
        );
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
