// ─────────────────────────────────────────────────────────────
// GLYPH — POST /api/compare
//
// Diff the decision histories of two already-analyzed repositories.
// Looks both repos up by URL in the `repos` table; if either has never
// been analyzed (or is mid-flight), the response reports per-side
// status and `comparison.ready=false` so the frontend can kick off the
// missing analyses and retry. When both are complete it returns an
// honest side-by-side: raw counts, intent-type distribution, mean
// confidence, and the contributor overlap between the two histories.
// ─────────────────────────────────────────────────────────────
use axum::{extract::State, Json};
use serde::Deserialize;
use serde_json::{json, Value};
use sqlx::Row;
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;
use uuid::Uuid;

use crate::api::analyze::parse_github_url;
use crate::errors::AppError;
use crate::AppState;

#[derive(Debug, Deserialize)]
pub struct CompareRequest {
    pub repo_a_url: String,
    pub repo_b_url: String,
}

#[derive(Debug)]
struct RepoRef {
    id:          Option<Uuid>,
    owner:       String,
    name:        String,
    status:      String,
    analyzed_at: Option<String>,
}

/// Most recent analysis of a repo URL, or a `never` ref if none exists.
async fn resolve_repo(state: &Arc<AppState>, url: &str) -> Result<RepoRef, AppError> {
    let (owner, name) = parse_github_url(url)
        .ok_or_else(|| AppError::BadRequest("Invalid GitHub URL — expected https://github.com/owner/repo".into()))?;

    let row = sqlx::query(
        "SELECT id, status, analyzed_at::text
         FROM repos WHERE owner = $1 AND name = $2
         ORDER BY analyzed_at DESC NULLS LAST LIMIT 1",
    )
    .bind(&owner)
    .bind(&name)
    .fetch_optional(&state.db)
    .await?;

    Ok(match row {
        Some(r) => RepoRef {
            id:          r.try_get("id").ok(),
            owner,
            name,
            status:      r.try_get("status").unwrap_or_else(|_| "unknown".into()),
            analyzed_at: r.try_get("analyzed_at").ok(),
        },
        None => RepoRef { id: None, owner, name, status: "never".into(), analyzed_at: None },
    })
}

/// Raw GitHub-side scale counts for a completed repo.
async fn repo_counts(state: &Arc<AppState>, id: Uuid) -> Result<Value, AppError> {
    let commits: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM commits WHERE repo_id = $1")
        .bind(id)
        .fetch_one(&state.db)
        .await?;
    let pull_requests: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM pull_requests WHERE repo_id = $1")
        .bind(id)
        .fetch_one(&state.db)
        .await?;
    let issues: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM issues WHERE repo_id = $1")
        .bind(id)
        .fetch_one(&state.db)
        .await?;
    Ok(json!({ "commits": commits, "pull_requests": pull_requests, "issues": issues }))
}

struct Profile {
    total:             u64,
    type_distribution: BTreeMap<String, u64>,
    contributors:      BTreeSet<String>,
    avg_confidence:    f64,
}

/// Intent-node profile: type mix, participant set, mean confidence.
async fn intent_profile(state: &Arc<AppState>, id: Uuid) -> Result<Profile, AppError> {
    let rows = sqlx::query(
        "SELECT node_type, contributors, confidence FROM intent_nodes WHERE repo_id = $1",
    )
    .bind(id)
    .fetch_all(&state.db)
    .await?;

    let mut dist = BTreeMap::new();
    let mut contribs = BTreeSet::new();
    let mut total = 0u64;
    let mut conf_sum = 0.0f64;
    let mut conf_rated = 0u64;

    for r in &rows {
        if let Ok(nt) = r.try_get::<String, _>("node_type") {
            *dist.entry(nt).or_default() += 1;
        }
        if let Ok(Value::Array(arr)) = r.try_get::<Value, _>("contributors") {
            for v in arr {
                if let Value::String(s) = v {
                    contribs.insert(s);
                }
            }
        }
        // NULL confidence means the model never scored that node. Counting
        // it as zero would make a repository look less certain than it is.
        if let Ok(Some(c)) = r.try_get::<Option<f64>, _>("confidence") {
            conf_sum   += c;
            conf_rated += 1;
        }
        total += 1;
    }

    Ok(Profile {
        total,
        type_distribution: dist,
        contributors: contribs,
        avg_confidence: if conf_rated > 0 { conf_sum / conf_rated as f64 * 100.0 } else { 0.0 },
    })
}

pub async fn compare_repos(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<CompareRequest>,
) -> Result<Json<Value>, AppError> {
    if payload.repo_a_url.trim().len() > 2048 || payload.repo_b_url.trim().len() > 2048 {
        return Err(AppError::BadRequest("repo URL is too long".into()));
    }

    let a = resolve_repo(&state, &payload.repo_a_url).await?;
    let b = resolve_repo(&state, &payload.repo_b_url).await?;

    let ready = a.status == "complete" && b.status == "complete";

    let comparison = if ready {
        let a_id = a.id.ok_or(AppError::Internal("complete repo missing id".into()))?;
        let b_id = b.id.ok_or(AppError::Internal("complete repo missing id".into()))?;

        let counts_a = repo_counts(&state, a_id).await?;
        let counts_b = repo_counts(&state, b_id).await?;
        let prof_a   = intent_profile(&state, a_id).await?;
        let prof_b   = intent_profile(&state, b_id).await?;

        let shared: Vec<String> = prof_a.contributors.intersection(&prof_b.contributors).cloned().collect();
        let a_only: Vec<String> = prof_a.contributors.difference(&prof_b.contributors).cloned().collect();
        let b_only: Vec<String> = prof_b.contributors.difference(&prof_a.contributors).cloned().collect();

        // Deterministic, honest one-line comparisons from the metrics above.
        let da = prof_a.type_distribution.get("decision").copied().unwrap_or(0);
        let db = prof_b.type_distribution.get("decision").copied().unwrap_or(0);
        let mut notes: Vec<String> = Vec::new();

        if da == 0 && db == 0 {
            notes.push("Neither repository has extracted decision nodes yet.".into());
        } else if da >= db && db > 0 {
            notes.push(format!(
                "Repository A has {:.1}× the decision volume of repository B ({} vs {}).",
                da as f64 / db as f64, da, db
            ));
        } else if da > 0 {
            notes.push(format!(
                "Repository B has {:.1}× the decision volume of repository A ({} vs {}).",
                db as f64 / da as f64, db, da
            ));
        } else {
            notes.push(format!("Only repository B has extracted decision nodes ({}).", db));
        }

        let conf_delta = prof_a.avg_confidence - prof_b.avg_confidence;
        if conf_delta.abs() < 0.5 {
            notes.push("Mean decision confidence is essentially equal between the two repositories.".into());
        } else if conf_delta > 0.0 {
            notes.push(format!(
                "Mean decision confidence runs {:.0} points higher in repository A ({:.0} vs {:.0}).",
                conf_delta, prof_a.avg_confidence, prof_b.avg_confidence
            ));
        } else {
            notes.push(format!(
                "Mean decision confidence runs {:.0} points higher in repository B ({:.0} vs {:.0}).",
                -conf_delta, prof_b.avg_confidence, prof_a.avg_confidence
            ));
        }

        if shared.is_empty() {
            notes.push("No overlapping contributors — the two decision histories are disjoint.".into());
        } else {
            let pool = shared.len() + a_only.len() + b_only.len();
            notes.push(format!(
                "{} of {} contributors appear in both repositories ({} shared).",
                shared.len(), pool, shared.len()
            ));
        }

        json!({
            "ready": true,
            "a": {
                "counts": counts_a,
                "total_nodes": prof_a.total,
                "type_distribution": prof_a.type_distribution,
                "avg_confidence": prof_a.avg_confidence.round(),
                "contributors": prof_a.contributors.iter().collect::<Vec<_>>(),
            },
            "b": {
                "counts": counts_b,
                "total_nodes": prof_b.total,
                "type_distribution": prof_b.type_distribution,
                "avg_confidence": prof_b.avg_confidence.round(),
                "contributors": prof_b.contributors.iter().collect::<Vec<_>>(),
            },
            "overlap": { "shared": shared, "a_only": a_only, "b_only": b_only },
            "notes": notes,
        })
    } else {
        json!({ "ready": false })
    };

    Ok(Json(json!({
        "a": {
            "owner": a.owner, "name": a.name, "status": a.status,
            "repo_id": a.id.map(|i| i.to_string()), "analyzed_at": a.analyzed_at,
        },
        "b": {
            "owner": b.owner, "name": b.name, "status": b.status,
            "repo_id": b.id.map(|i| i.to_string()), "analyzed_at": b.analyzed_at,
        },
        "comparison": comparison,
    })))
}
