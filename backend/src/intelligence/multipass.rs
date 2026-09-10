// ─────────────────────────────────────────────────────────────
// GLYPH — multi-pass NIM analysis + deterministic fallback
//
// The old pipeline truncated the evidence to the newest ~50 commits /
// ~30 PRs and made a single LLM call, so large repositories' decision
// history was barely covered. This module fixes that in two ways:
//
//  1. MULTI-PASS: the commit history is cut into evenly-spaced windows
//     (≤ MAX_ANALYSIS_CHUNKS, default 6) that span the whole history,
//     each with the PRs/issues whose dates fall inside it. Each window
//     gets its own NIM extraction call (pass 1). If enough insights come
//     back, a single consolidation call (pass 2) merges near-duplicates
//     that cross window boundaries and reconciles contradictions.
//
//  2. FALLBACK: if every NIM call fails or returns nothing, a
//     deterministic extractor still produces real insights from commit
//     messages and PR state, so the job completes with honest (if
//     coarser) data instead of failing or inserting zero nodes.
// ─────────────────────────────────────────────────────────────
use std::collections::HashMap;

use chrono::NaiveDateTime;
use serde_json::json;

use crate::errors::AppError;
use crate::ingestion::commits::GitHubCommit;
use crate::ingestion::issues::{Comment as IssueComment, Issue};
use crate::ingestion::pull_requests::{PullRequest, PullRequestComment, PullRequestReview};
use crate::intelligence::client::{
    analyze_events_with_prompt, chat, model_from_env, parse_insights, ExtractedInsight,
};
use crate::intelligence::prompts;

/// Max commits analysed per NIM window — bounds per-call token cost.
const COMMITS_PER_CHUNK: usize = 40;
/// How many extraction windows may be in flight at once.
const MAX_CONCURRENT_WINDOWS: usize = 4;

/// Max NIM windows per job — bounds LLM cost on 10k-commit repos.
/// Override with MAX_ANALYSIS_CHUNKS.
fn max_chunks() -> usize {
    std::env::var("MAX_ANALYSIS_CHUNKS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(6)
}
/// Run the consolidation pass when merged insights exceed this size.
const SYNTHESIS_THRESHOLD: usize = 24;

// ── Chunk builder ────────────────────────────────────────────

fn parse_dt(s: &str) -> Option<NaiveDateTime> {
    chrono::DateTime::parse_from_rfc3339(s).ok().map(|dt| dt.naive_utc())
}

fn in_range(t: Option<NaiveDateTime>, lo: Option<NaiveDateTime>, hi: Option<NaiveDateTime>) -> bool {
    matches!((t, lo, hi), (Some(t), Some(a), Some(b)) if t >= a && t <= b)
}

/// Split the decision evidence into chronologically ordered windows that
/// span the *whole* commit history (not just the newest N commits), each
/// bundled with the PRs/issues that happened inside its time range.
pub fn build_analysis_chunks(
    owner:            &str,
    repo:             &str,
    commits:          &[GitHubCommit],
    prs:              &[PullRequest],
    issues:           &[Issue],
    reviews_by_pr:    &[(i64, Vec<PullRequestReview>)],
    comments_by_pr:   &[(i64, Vec<PullRequestComment>)],
    comments_by_issue: &[(i64, Vec<IssueComment>)],
) -> Vec<String> {
    let rev_map: HashMap<i64, &Vec<PullRequestReview>> = reviews_by_pr.iter().map(|(n, v)| (*n, v)).collect();
    let prc_map: HashMap<i64, &Vec<PullRequestComment>> = comments_by_pr.iter().map(|(n, v)| (*n, v)).collect();
    let ic_map:  HashMap<i64, &Vec<IssueComment>>       = comments_by_issue.iter().map(|(n, v)| (*n, v)).collect();

    let mut ordered: Vec<&GitHubCommit> = commits.iter().collect();
    ordered.sort_by_key(|c| parse_dt(&c.commit.author.date));

    // Evenly-spaced window start indices so coverage reaches the whole history.
    let mut windows: Vec<Vec<&GitHubCommit>> = Vec::new();
    if !ordered.is_empty() {
        let n   = ordered.len();
        let win = COMMITS_PER_CHUNK.max(1);
        let n_windows = ((n + win - 1) / win).min(max_chunks().max(1));
        let last_start = n.saturating_sub(win);
        for i in 0..n_windows {
            let start = if n_windows == 1 { 0 } else { (i * last_start) / (n_windows - 1) };
            let end = (start + win).min(n);
            windows.push(ordered[start..end].to_vec());
        }
    } else if !prs.is_empty() || !issues.is_empty() {
        // No commits at all — still give the model the PR/issue evidence.
        let pr_refs: Vec<&PullRequest> = prs.iter().collect();
        let issue_refs: Vec<&Issue> = issues.iter().collect();
        return vec![build_payload(owner, repo, &[], &pr_refs, &issue_refs, &rev_map, &prc_map, &ic_map, 1, 1)];
    } else {
        return Vec::new();
    }

    let total = windows.len();
    windows
        .iter()
        .enumerate()
        .map(|(idx, win_commits)| {
            let t0 = win_commits.first().and_then(|c| parse_dt(&c.commit.author.date));
            let t1 = win_commits.last().and_then(|c| parse_dt(&c.commit.author.date));
            let window_prs: Vec<&PullRequest> = prs.iter().filter(|p| {
                in_range(parse_dt(&p.created_at), t0, t1)
                    || p.merged_at.as_deref().and_then(parse_dt).map(|m| in_range(Some(m), t0, t1)).unwrap_or(false)
            }).collect();
            let window_issues: Vec<&Issue> = issues.iter()
                .filter(|i| in_range(parse_dt(&i.created_at), t0, t1))
                .collect();
            build_payload(owner, repo, win_commits, &window_prs, &window_issues, &rev_map, &prc_map, &ic_map, idx + 1, total)
        })
        .collect()
}

#[allow(clippy::too_many_arguments)]
fn build_payload(
    owner:             &str,
    repo:              &str,
    win_commits:       &[&GitHubCommit],
    prs:               &[&PullRequest],
    issues:            &[&Issue],
    rev_map:           &HashMap<i64, &Vec<PullRequestReview>>,
    prc_map:           &HashMap<i64, &Vec<PullRequestComment>>,
    ic_map:            &HashMap<i64, &Vec<IssueComment>>,
    chunk_idx:         usize,
    total:             usize,
) -> String {
    let mut reviews: Vec<serde_json::Value> = Vec::new();
    let mut pr_comments: Vec<serde_json::Value> = Vec::new();
    for pr in prs {
        if let Some(revs) = rev_map.get(&pr.number) {
            reviews.extend(revs.iter().take(20).map(|r| json!({
                "pr": pr.number, "state": &r.state, "body": r.body.as_deref().unwrap_or(""),
                "author": r.user.as_ref().map(|u| u.login.as_str()).unwrap_or("unknown"),
                "submitted_at": &r.submitted_at
            })));
        }
        if let Some(cs) = prc_map.get(&pr.number) {
            pr_comments.extend(cs.iter().take(20).map(|c| json!({
                "pr": pr.number, "path": &c.path, "body": &c.body,
                "author": c.user.as_ref().map(|u| u.login.as_str()).unwrap_or("unknown")
            })));
        }
    }
    let mut issue_comments: Vec<serde_json::Value> = Vec::new();
    for i in issues {
        if let Some(cs) = ic_map.get(&i.number) {
            issue_comments.extend(cs.iter().take(20).map(|c| json!({
                "issue": i.number, "body": &c.body,
                "author": c.user.as_ref().map(|u| u.login.as_str()).unwrap_or("unknown")
            })));
        }
    }
    reviews.truncate(60);
    pr_comments.truncate(60);
    issue_comments.truncate(60);

    json!({
        "repository": format!("{}/{}", owner, repo),
        "window": format!("decision-history window {} of {}", chunk_idx, total),
        "commits": win_commits.iter().take(COMMITS_PER_CHUNK).map(|c| json!({
            "sha": &c.sha, "message": &c.commit.message,
            "author": c.author.as_ref().and_then(|a| a.login.as_deref()).unwrap_or("unknown"),
            "date": &c.commit.author.date
        })).collect::<Vec<_>>(),
        "pull_requests": prs.iter().take(20).map(|pr| json!({
            "number": pr.number, "title": &pr.title, "body": pr.body.as_deref().unwrap_or(""),
            "state": &pr.state, "author": pr.user.as_ref().map(|u| u.login.as_str()).unwrap_or("unknown"),
            "created_at": &pr.created_at, "merged_at": &pr.merged_at
        })).collect::<Vec<_>>(),
        "pull_request_reviews": reviews,
        "pull_request_comments": pr_comments,
        "issues": issues.iter().take(20).map(|i| json!({
            "number": i.number, "title": &i.title, "body": i.body.as_deref().unwrap_or(""),
            "state": &i.state, "author": i.user.as_ref().map(|u| u.login.as_str()).unwrap_or("unknown"),
            "created_at": &i.created_at
        })).collect::<Vec<_>>(),
        "issue_comments": issue_comments,
    })
    .to_string()
}

// ── Multi-pass extraction ────────────────────────────────────

/// Pass 1: one NIM extraction call per window (spanning the whole history).
/// Pass 2: a consolidation call that merges cross-window duplicates when the
/// merged set is large enough. Falls back to deterministic dedupe otherwise.
pub async fn analyze_events_multipass(
    client:  &reqwest::Client,
    api_key: &str,
    chunks:  &[String],
) -> Result<Vec<ExtractedInsight>, AppError> {
    if chunks.is_empty() {
        return Err(AppError::NimApiError("no evidence to analyse".into()));
    }

    let total = chunks.len();

    // Windows are independent, so run them concurrently instead of one after
    // another. Current NIM models spend ~100s on a window; serially that made
    // a 6-window analysis take ten minutes, which is unusable interactively.
    // Concurrency is capped so a large repo cannot fan out into a burst that
    // trips NVIDIA's per-account rate limit.
    let permits = std::sync::Arc::new(tokio::sync::Semaphore::new(MAX_CONCURRENT_WINDOWS));
    let mut tasks = Vec::with_capacity(total);

    for (i, chunk) in chunks.iter().enumerate() {
        let prompt = prompts::build_analysis_prompt(&format!(
            "This is {} of the repository's decision history.\n{}",
            i + 1, chunk
        ));
        // reqwest::Client is an Arc internally — cloning shares the pool.
        let client  = client.clone();
        let api_key = api_key.to_string();
        let permits = std::sync::Arc::clone(&permits);

        tasks.push(tokio::spawn(async move {
            let _permit = permits.acquire().await;

            // One retry when the response yields no parseable insights. A
            // reasoning model sometimes spends its whole token budget
            // narrating ("We need to extract: - Decision nodes: …") and never
            // emits the array. That is a sampling accident, not a broken
            // prompt, and a second draw usually succeeds — so rather than lose
            // a sixth of the repository's history to it, ask once more.
            // Transport failures are already retried inside `chat`.
            let mut result = analyze_events_with_prompt(&client, &api_key, &prompt).await;
            if matches!(&result, Err(AppError::NimApiError(m)) if m.contains("Failed to parse")) {
                tracing::warn!(
                    "NIM extraction window {} returned no parseable insights — resampling once",
                    i + 1
                );
                result = analyze_events_with_prompt(&client, &api_key, &prompt).await;
            }
            (i, result)
        }));
    }

    // Collect, then sort by window index so the merged list stays
    // chronological regardless of which window finished first.
    let mut collected: Vec<(usize, Vec<ExtractedInsight>)> = Vec::new();
    for task in tasks {
        match task.await {
            Ok((i, Ok(insights))) => {
                tracing::info!(
                    "NIM extraction window {}/{}: {} insights",
                    i + 1, total, insights.len()
                );
                collected.push((i, insights));
            }
            Ok((i, Err(e))) => {
                tracing::warn!("NIM extraction window {}/{} failed: {}", i + 1, total, e);
            }
            Err(e) => {
                tracing::warn!("NIM extraction window task panicked: {}", e);
            }
        }
    }
    collected.sort_by_key(|(i, _)| *i);

    let merged: Vec<ExtractedInsight> =
        collected.into_iter().flat_map(|(_, v)| v).collect();

    if merged.is_empty() {
        return Err(AppError::NimApiError(
            "all NIM extraction windows returned no insights".into(),
        ));
    }

    // Pass 2 — consolidation for large merged sets.
    if merged.len() >= SYNTHESIS_THRESHOLD {
        match synthesize(client, api_key, &merged).await {
            Ok(clean) if !clean.is_empty() => {
                let clean = inherit_confidence(clean, &merged);
                tracing::info!(
                    "NIM consolidation pass: {} insights -> {} ({} unrated)",
                    merged.len(),
                    clean.len(),
                    clean.iter().filter(|i| i.confidence.is_none()).count()
                );
                return Ok(clean);
            }
            Ok(_) => tracing::warn!("NIM consolidation returned empty — keeping merged insights"),
            Err(e) => tracing::warn!("NIM consolidation failed — keeping merged insights: {}", e),
        }
    }

    let deduped = dedupe(&merged);
    tracing::info!(
        "Multi-pass extraction final: {} insights (deduped from {})",
        deduped.len(),
        merged.len()
    );
    Ok(deduped)
}

/// Pass 2 — one call that merges cross-window near-duplicates and reconciles
/// contradictions toward the higher-confidence version.
async fn synthesize(
    client:    &reqwest::Client,
    api_key:   &str,
    insights:  &[ExtractedInsight],
) -> Result<Vec<ExtractedInsight>, AppError> {
    let input = serde_json::to_string(insights).unwrap_or_default();
    let prompt = format!(
        "You are reconciling structured intelligence extracted in parallel windows from one \
         repository's decision history. Merge duplicate or near-duplicate entries (the same \
         decision, debate, or rejection appearing in more than one window), resolve contradictory \
         summaries toward the higher-confidence version, and keep contributors and source_refs \
         unioned. Drop entries with no clear evidence. \
         Return ONLY a JSON array with fields: node_type, title, summary, reasoning, contributors, \
         source_refs, confidence. Every object MUST carry a numeric confidence \
         between 0 and 1 — carry the input entry's confidence across unless merging \
         changes it.\n\n{}",
        input
    );
    let system = "You are a reconciliation analyst deduplicating intelligence records. Return only a JSON array.";
    let content = chat(client, api_key, &model_from_env(), system, &prompt, 0.1, 8192).await?;
    parse_insights(&content)
        .map_err(|e| AppError::NimApiError(format!("failed to parse consolidation output: {e}")))
}

/// Carry extraction-pass confidence through consolidation.
///
/// The consolidation prompt asks for `confidence` back, but models routinely
/// drop the field while rewriting each record — on one ripgrep run 20 of 23
/// consolidated insights came back unrated even though the extraction pass
/// had scored them. That score is real data the model already produced, so
/// rather than lose it in the merge, match each consolidated insight back to
/// the pre-consolidation set by title and inherit the best score found there.
///
/// Only insights the consolidation pass left unrated are touched; a score it
/// did supply always wins, because it reflects the merged record.
fn inherit_confidence(
    mut clean: Vec<ExtractedInsight>,
    source:    &[ExtractedInsight],
) -> Vec<ExtractedInsight> {
    use std::collections::HashMap;

    let mut by_title: HashMap<String, f32> = HashMap::new();
    for i in source {
        if let Some(c) = i.confidence {
            let key = i.title.trim().to_lowercase();
            if key.is_empty() {
                continue;
            }
            by_title
                .entry(key)
                .and_modify(|best| { if c > *best { *best = c; } })
                .or_insert(c);
        }
    }

    for i in clean.iter_mut() {
        if i.confidence.is_none() {
            i.confidence = by_title.get(&i.title.trim().to_lowercase()).copied();
        }
    }
    clean
}

/// Deterministic dedupe on (node_type, lowercased title) — the safety net
/// that applies when the LLM consolidation pass is unavailable.
fn dedupe(insights: &[ExtractedInsight]) -> Vec<ExtractedInsight> {
    use std::collections::HashSet;
    let mut seen = HashSet::new();
    let mut out  = Vec::new();
    for i in insights {
        // Trimmed as well as lowercased, to agree with inherit_confidence.
        // These two disagreed once, so a title with stray whitespace was the
        // same node to one function and a different node to the other.
        let key = format!("{}::{}", i.node_type, i.title.trim().to_lowercase());
        if seen.insert(key) {
            out.push(i.clone());
        }
    }
    out
}

// ── Deterministic fallback ───────────────────────────────────

/// Produce real insights from commit messages and PR state without any LLM
/// call. Used when the NIM path fails entirely so the job still completes
/// with honest data derived directly from GitHub evidence.
pub fn fallback_insights(
    commits: &[GitHubCommit],
    prs:     &[PullRequest],
) -> Vec<ExtractedInsight> {
    let mut out: Vec<ExtractedInsight> = Vec::new();

    let arch_keywords = [
        "refactor", "migrat", "architect", "restructur", "reorganiz",
        "modular", "adopt", "introduc", "split", "extract",
    ];
    for c in commits.iter().take(30) {
        let title = c.commit.message.lines().next().unwrap_or("UNKNOWN");
        let lower = title.to_lowercase();
        let is_arch = arch_keywords.iter().any(|k| lower.contains(k));
        out.push(ExtractedInsight {
            node_type: if is_arch { "architectural".into() } else { "decision".into() },
            title:     title.chars().take(80).collect(),
            summary:   c.commit.message.chars().take(200).collect(),
            reasoning: "Fallback heuristic: derived directly from the commit message".into(),
            contributors: c.author.as_ref().and_then(|a| a.login.clone()).into_iter().collect(),
            source_refs: vec![c.sha.clone()],
            confidence: Some(if is_arch { 0.70 } else { 0.62 }),
        });
    }

    for pr in prs.iter().take(30) {
        let merged = pr.merged_at.is_some() || pr.state.eq_ignore_ascii_case("merged");
        out.push(ExtractedInsight {
            node_type: if merged { "decision".into() } else { "rejection".into() },
            title:     pr.title.clone(),
            summary:   pr.body.clone().unwrap_or_default(),
            reasoning: format!(
                "Fallback heuristic: PR #{} {}",
                pr.number,
                if merged { "merged" } else { "closed without merge" }
            ),
            contributors: pr.user.as_ref().map(|u| vec![u.login.clone()]).unwrap_or_default(),
            source_refs: vec![format!("PR#{}", pr.number)],
            confidence: Some(if merged { 0.72 } else { 0.60 }),
        });
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ingestion::commits::{AuthorInfo, CommitAuthor, CommitDetail};
    use crate::ingestion::issues::Issue;
    use crate::ingestion::pull_requests::PRUser;

    fn commit(i: usize, msg: &str, date: &str) -> GitHubCommit {
        GitHubCommit {
            sha: format!("sha{:07x}", i),
            commit: CommitDetail {
                message: msg.to_string(),
                author:  CommitAuthor { date: date.to_string() },
            },
            author: Some(AuthorInfo { login: Some("tester".into()) }),
            files:  None,
        }
    }

    fn pr(number: i64, merged: bool) -> PullRequest {
        PullRequest {
            number,
            title: format!("PR title {}", number),
            body:  Some("body".into()),
            state: if merged { "closed".into() } else { "open".into() },
            user:  Some(PRUser { login: "tester".into() }),
            created_at: "2024-03-01T00:00:00Z".into(),
            merged_at:  if merged { Some("2024-03-05T00:00:00Z".into()) } else { None },
        }
    }

    #[test]
    fn chunk_builder_spans_whole_history_within_limit() {
        // 200 commits across ~28 days — many more than a single window.
        let commits: Vec<GitHubCommit> = (0..200)
            .map(|i| commit(i, "chore: work", &format!("2024-01-{:02}T00:00:00Z", i % 28 + 1)))
            .collect();
        let prs: Vec<PullRequest> = (1..=5).map(|n| pr(n, n % 2 == 0)).collect();
        let issues: Vec<Issue> = Vec::new();

        let chunks = build_analysis_chunks("owner", "repo", &commits, &prs, &issues, &[], &[], &[]);

        assert!(!chunks.is_empty(), "expected at least one window");
        assert!(chunks.len() > 1, "expected multiple windows");
        assert!(chunks.len() <= max_chunks(), "window count must respect the cap");

        // First window carries the earliest commit, last window the latest —
        // i.e. the whole history is covered, not just the newest commits.
        assert!(chunks.first().unwrap().contains("sha0000000"));
        assert!(chunks.last().unwrap().contains("2024-01-28"), "last window should reach the newest dates");
    }

    #[test]
    fn chunk_builder_handles_empty_evidence() {
        let chunks = build_analysis_chunks("owner", "repo", &[], &[], &[], &[], &[], &[]);
        assert!(chunks.is_empty());
    }

    #[test]
    fn fallback_derives_node_types_from_evidence() {
        let commits = vec![
            commit(1, "migrate to axum framework", "2024-01-01T00:00:00Z"),
            commit(2, "fix typo in docs", "2024-01-02T00:00:00Z"),
        ];
        let prs = vec![pr(1, true), pr(2, false)];

        let insights = fallback_insights(&commits, &prs);

        assert!(insights.iter().any(|i| i.node_type == "architectural" && i.title.contains("migrate")));
        assert!(insights.iter().any(|i| i.node_type == "decision" && i.title.contains("fix typo")));
        assert!(insights.iter().any(|i| i.node_type == "decision" && i.title.contains("PR title 1")));
        assert!(insights.iter().any(|i| i.node_type == "rejection" && i.title.contains("PR title 2")));
        for i in &insights {
            assert!(matches!(i.confidence, Some(c) if (0.0..=1.0).contains(&c)));
            assert!(!i.source_refs.is_empty());
        }
    }

    /// The consolidation pass rewrites every record and routinely drops the
    /// confidence field. The score the extraction pass produced is real, so
    /// it must survive the merge rather than leaving the insight unrated.
    #[test]
    fn consolidation_inherits_confidence_the_model_dropped() {
        let mk = |title: &str, conf: Option<f32>| ExtractedInsight {
            node_type: "decision".into(), title: title.into(),
            summary: "s".into(), reasoning: "r".into(),
            contributors: vec![], source_refs: vec![], confidence: conf,
        };

        let source = vec![
            mk("Adopt Axum", Some(0.9)),
            mk("Adopt Axum", Some(0.7)),
            mk("Drop Warp", Some(0.4)),
        ];
        let clean = vec![
            mk("adopt axum", None),
            mk("Drop Warp", Some(0.95)),
            mk("Brand new title", None),
        ];

        let out = inherit_confidence(clean, &source);

        // Matched case-insensitively, and takes the best score available.
        assert_eq!(out[0].confidence, Some(0.9));
        // A score the consolidation pass did supply wins over the source.
        assert_eq!(out[1].confidence, Some(0.95));
        // Nothing to inherit stays honestly unrated.
        assert_eq!(out[2].confidence, None);
    }

    /// dedupe and inherit_confidence must normalise a title the same way.
    /// They disagreed once — one trimmed and the other did not — so a padded
    /// title was one node to one function and two to the other.
    #[test]
    fn dedupe_and_inheritance_agree_on_whitespace() {
        let padded = ExtractedInsight {
            node_type: "decision".into(), title: "  Adopt Axum  ".into(),
            summary: String::new(), reasoning: String::new(),
            contributors: vec![], source_refs: vec![], confidence: None,
        };
        let plain = ExtractedInsight {
            node_type: "decision".into(), title: "Adopt Axum".into(),
            summary: String::new(), reasoning: String::new(),
            contributors: vec![], source_refs: vec![], confidence: Some(0.9),
        };

        assert_eq!(dedupe(&[padded.clone(), plain.clone()]).len(), 1);

        let out = inherit_confidence(vec![padded], &[plain]);
        assert_eq!(out[0].confidence, Some(0.9));
    }

    #[test]
    fn dedupe_collapses_case_insensitive_duplicate_titles() {
        let a = ExtractedInsight {
            node_type: "decision".into(), title: "X".into(),
            summary: String::new(), reasoning: String::new(),
            contributors: vec![], source_refs: vec![], confidence: Some(0.5),
        };
        let b = ExtractedInsight {
            node_type: "decision".into(), title: "x".into(),
            summary: String::new(), reasoning: String::new(),
            contributors: vec![], source_refs: vec![], confidence: Some(0.6),
        };
        let out = dedupe(&[a.clone(), b.clone()]);
        assert_eq!(out.len(), 1);
    }
}
