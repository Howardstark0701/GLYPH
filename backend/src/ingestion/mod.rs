pub mod commits;
pub mod issues;
pub mod pull_requests;

use std::time::Duration;

use crate::errors::AppError;

/// Maximum retries on transient GitHub failures (403/429 — secondary rate limits).
const MAX_RETRIES: u32 = 3;

/// Max pages fetched for top-level list endpoints (issues / pulls), configurable
/// via MAX_LIST_PAGES (default 10 → 1000 items). Scales with MAX_COMMITS so
/// large-repo ingestion covers the whole decision history, not just the head.
pub fn max_list_pages() -> u32 {
    std::env::var("MAX_LIST_PAGES")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(10)
}

/// GET a GitHub API URL with the BYOK token attached, retrying with backoff on
/// 403/429 (rate limiting) and surfacing any HTTP or transport failure as a
/// 502 `GitHubApiError` — not a generic 500.
pub async fn gh_get(
    client: &reqwest::Client,
    token:  &str,
    url:    &str,
) -> Result<reqwest::Response, AppError> {
    let mut attempt = 0u32;

    loop {
        let resp = client
            .get(url)
            .header("Authorization", format!("Bearer {}", token))
            .header("User-Agent", "glyph/0.1")
            .header("Accept", "application/vnd.github+json")
            .header("X-GitHub-Api-Version", "2022-11-28")
            .send()
            .await
            .map_err(|e| {
                AppError::GitHubApiError(format!(
                    "GitHub request to {url} failed at transport level: {e}"
                ))
            })?;

        // Transient rate-limit / abuse triggers — honour Retry-After, then back off.
        if resp.status() == reqwest::StatusCode::TOO_MANY_REQUESTS
            || resp.status() == reqwest::StatusCode::FORBIDDEN
        {
            attempt += 1;
            if attempt >= MAX_RETRIES {
                return Err(AppError::GitHubApiError(format!(
                    "GitHub still rejecting request after {MAX_RETRIES} retries ({url})"
                )));
            }

            let retry_after = resp
                .headers()
                .get("retry-after")
                .and_then(|v| v.to_str().ok())
                .and_then(|s| s.parse::<u64>().ok())
                .unwrap_or_else(|| 2u64 * attempt as u64);
            let wait = Duration::from_secs(retry_after);

            tracing::warn!(
                "GitHub {} on {url}, retry {attempt}/{MAX_RETRIES} in {}s",
                resp.status(),
                wait.as_secs()
            );
            tokio::time::sleep(wait).await;
            continue;
        }

        if !resp.status().is_success() {
            return Err(AppError::GitHubApiError(format!(
                "GitHub API returned {} for {url}",
                resp.status()
            )));
        }

        return Ok(resp);
    }
}
