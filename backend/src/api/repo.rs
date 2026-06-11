use axum::{
    extract::{Path, State},
    http::HeaderMap,
    Json,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::sync::Arc;
use uuid::Uuid;

use crate::errors::AppError;
use crate::intelligence::{client as nim, prompts};
use crate::AppState;

// ── Shared types ──────────────────────────────────────────

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct RepoStatus {
    pub id:          String,
    pub status:      Option<String>,
    pub analyzed_at: Option<String>,
}

// ── GET /repo/:id/status ──────────────────────────────────

pub async fn get_status(
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> Result<Json<RepoStatus>, AppError> {
    let row = sqlx::query_as!(
        RepoStatus,
        "SELECT id::text, status, analyzed_at::text FROM repos WHERE id = $1",
        id
    )
    .fetch_optional(&state.db)
    .await?
    .ok_or_else(|| AppError::NotFound(format!("Repository {} not found", id)))?;

    Ok(Json(row))
}

// ── GET /repo/:id/intent ──────────────────────────────────

#[derive(Debug, Serialize, sqlx::FromRow)]
struct IntentRow {
    id:           String,
    node_type:    Option<String>,
    title:        Option<String>,
    summary:      Option<String>,
    reasoning:    Option<String>,
    contributors: Option<Value>,
    source_refs:  Option<Value>,
    timestamp:    Option<String>,
    confidence:   Option<f64>,
}

pub async fn get_intent(
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> Result<Json<Value>, AppError> {
    ensure_repo_exists(&state, id).await?;

    let rows = sqlx::query_as!(
        IntentRow,
        r#"SELECT
             id::text,
             node_type,
             title,
             summary,
             reasoning,
             contributors,
             source_refs,
             timestamp::text,
             confidence
           FROM intent_nodes
           WHERE repo_id = $1
           ORDER BY timestamp ASC NULLS LAST"#,
        id
    )
    .fetch_all(&state.db)
    .await?;

    Ok(Json(json!({ "repo_id": id, "nodes": rows })))
}

// ── GET /repo/:id/debates ─────────────────────────────────

pub async fn get_debates(
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> Result<Json<Value>, AppError> {
    ensure_repo_exists(&state, id).await?;

    let rows = sqlx::query_as!(
        IntentRow,
        r#"SELECT
             id::text,
             node_type,
             title,
             summary,
             reasoning,
             contributors,
             source_refs,
             timestamp::text,
             confidence
           FROM intent_nodes
           WHERE repo_id = $1
             AND node_type = 'debate'
           ORDER BY timestamp ASC NULLS LAST"#,
        id
    )
    .fetch_all(&state.db)
    .await?;

    // Build sentiment metrics from the data
    let total      = rows.len() as f64;
    let high_conf  = rows.iter().filter(|r| r.confidence.unwrap_or(0.0) > 0.7).count() as f64;
    let resolved   = rows.iter().filter(|r| {
        r.summary.as_deref().map(|s| s.to_lowercase().contains("resolved")).unwrap_or(false)
    }).count() as f64;

    let contention_pct = if total > 0.0 { ((total - resolved) / total * 100.0).round() } else { 0.0 };
    let agreement_pct  = if total > 0.0 { (resolved / total * 100.0).round() } else { 0.0 };
    let confidence_avg = if total > 0.0 {
        rows.iter().filter_map(|r| r.confidence).sum::<f64>() / total * 100.0
    } else { 0.0 };

    Ok(Json(json!({
        "repo_id": id,
        "debates": rows,
        "metrics": {
            "total":          total as u64,
            "agreement_pct":  agreement_pct,
            "contention_pct": contention_pct,
            "confidence_avg": confidence_avg.round()
        }
    })))
}

// ── GET /repo/:id/decisions ───────────────────────────────

pub async fn get_decisions(
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> Result<Json<Value>, AppError> {
    ensure_repo_exists(&state, id).await?;

    let rows = sqlx::query_as!(
        IntentRow,
        r#"SELECT
             id::text,
             node_type,
             title,
             summary,
             reasoning,
             contributors,
             source_refs,
             timestamp::text,
             confidence
           FROM intent_nodes
           WHERE repo_id = $1
             AND node_type = 'decision'
           ORDER BY timestamp ASC NULLS LAST"#,
        id
    )
    .fetch_all(&state.db)
    .await?;

    Ok(Json(json!({ "repo_id": id, "decisions": rows, "total": rows.len() })))
}

// ── GET /repo/:id/rejections ──────────────────────────────

pub async fn get_rejections(
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> Result<Json<Value>, AppError> {
    ensure_repo_exists(&state, id).await?;

    let rows = sqlx::query_as!(
        IntentRow,
        r#"SELECT
             id::text,
             node_type,
             title,
             summary,
             reasoning,
             contributors,
             source_refs,
             timestamp::text,
             confidence
           FROM intent_nodes
           WHERE repo_id = $1
             AND node_type = 'rejection'
           ORDER BY timestamp DESC NULLS LAST"#,
        id
    )
    .fetch_all(&state.db)
    .await?;

    Ok(Json(json!({ "repo_id": id, "rejections": rows, "total": rows.len() })))
}

// ── GET /repo/:id/contributors ────────────────────────────

pub async fn get_contributors(
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> Result<Json<Value>, AppError> {
    ensure_repo_exists(&state, id).await?;

    // Aggregate contributor handles from all intent_nodes
    // contributors column is JSONB array of strings e.g. ["alice", "bob"]
    let rows: Vec<(Option<Value>,)> = sqlx::query_as(
        "SELECT contributors FROM intent_nodes WHERE repo_id = $1"
    )
    .bind(id)
    .fetch_all(&state.db)
    .await?;

    // Count decisions, debates, rejections per contributor
    use std::collections::HashMap;
    #[derive(Debug, Default)]
    struct ContribStats {
        decisions:   u32,
        debates:     u32,
        rejections:  u32,
        total_nodes: u32,
        confidence_sum: f64,
    }

    let node_rows = sqlx::query_as!(
        IntentRow,
        r#"SELECT
             id::text, node_type, title, summary, reasoning,
             contributors, source_refs, timestamp::text, confidence
           FROM intent_nodes WHERE repo_id = $1"#,
        id
    )
    .fetch_all(&state.db)
    .await?;

    let mut stats: HashMap<String, ContribStats> = HashMap::new();

    for node in &node_rows {
        if let Some(Value::Array(contribs)) = &node.contributors {
            for cv in contribs {
                if let Value::String(handle) = cv {
                    let entry = stats.entry(handle.clone()).or_default();
                    entry.total_nodes += 1;
                    entry.confidence_sum += node.confidence.unwrap_or(0.0);
                    match node.node_type.as_deref() {
                        Some("decision")  => entry.decisions  += 1,
                        Some("debate")    => entry.debates    += 1,
                        Some("rejection") => entry.rejections += 1,
                        _ => {}
                    }
                }
            }
        }
    }

    let contributors: Vec<Value> = stats.into_iter().map(|(handle, s)| {
        let avg_conf = if s.total_nodes > 0 {
            s.confidence_sum / s.total_nodes as f64
        } else { 0.0 };
        json!({
            "handle":      handle,
            "decisions":   s.decisions,
            "debates":     s.debates,
            "rejections":  s.rejections,
            "total_nodes": s.total_nodes,
            "avg_confidence": (avg_conf * 100.0).round()
        })
    }).collect();

    Ok(Json(json!({ "repo_id": id, "contributors": contributors })))
}

// ── GET /repo/:id/graph ───────────────────────────────────

pub async fn get_graph(
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> Result<Json<Value>, AppError> {
    ensure_repo_exists(&state, id).await?;

    let rows = sqlx::query_as!(
        IntentRow,
        r#"SELECT
             id::text, node_type, title, summary, reasoning,
             contributors, source_refs, timestamp::text, confidence
           FROM intent_nodes WHERE repo_id = $1
           ORDER BY timestamp ASC NULLS LAST"#,
        id
    )
    .fetch_all(&state.db)
    .await?;

    // Build D3-compatible node/link graph
    let nodes: Vec<Value> = rows.iter().map(|r| json!({
        "id":        r.id,
        "label":     r.title.as_deref().unwrap_or("UNKNOWN"),
        "type":      r.node_type.as_deref().unwrap_or("unknown"),
        "summary":   r.summary,
        "confidence": r.confidence,
        // Use first source_ref as a pseudo-sha for display
        "sha": r.source_refs
            .as_ref()
            .and_then(|v| v.as_array())
            .and_then(|a| a.first())
            .and_then(|v| v.as_str())
            .map(|s| format!("0x{}", &s[..s.len().min(7)]))
            .unwrap_or_else(|| "0x0000000".into())
    })).collect();

    // Link consecutive decisions and flag debates/rejections
    let mut links: Vec<Value> = Vec::new();
    let decision_ids: Vec<&str> = rows.iter()
        .filter(|r| r.node_type.as_deref() == Some("decision"))
        .map(|r| r.id.as_str())
        .collect();

    for window in decision_ids.windows(2) {
        links.push(json!({
            "source": window[0],
            "target": window[1],
            "type":   "decision"
        }));
    }

    // Link rejection nodes back to nearest decision
    for rej in rows.iter().filter(|r| r.node_type.as_deref() == Some("rejection")) {
        if let Some(dec_id) = decision_ids.first() {
            links.push(json!({
                "source": dec_id,
                "target": rej.id,
                "type":   "rejected"
            }));
        }
    }

    // Link debate nodes as debate edges
    for deb in rows.iter().filter(|r| r.node_type.as_deref() == Some("debate")) {
        if let Some(dec_id) = decision_ids.first() {
            links.push(json!({
                "source": dec_id,
                "target": deb.id,
                "type":   "debate"
            }));
        }
    }

    Ok(Json(json!({
        "repo_id": id,
        "nodes":   nodes,
        "links":   links,
        "meta": {
            "total_nodes": nodes.len(),
            "total_links": links.len(),
            "density_pct": if nodes.len() > 1 {
                (links.len() as f64 / (nodes.len() * (nodes.len() - 1) / 2) as f64 * 100.0).round() as u64
            } else { 0 }
        }
    })))
}

// ── GET /repo/:id/summary ─────────────────────────────────

pub async fn get_summary(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<Json<Value>, AppError> {
    ensure_repo_exists(&state, id).await?;

    let nim_api_key = headers
        .get("X-Nim-Api-Key")
        .and_then(|v| v.to_str().ok())
        .ok_or(AppError::MissingCredentials)?;

    // Gather all intent nodes for summary context
    let rows = sqlx::query_as!(
        IntentRow,
        r#"SELECT
             id::text, node_type, title, summary, reasoning,
             contributors, source_refs, timestamp::text, confidence
           FROM intent_nodes WHERE repo_id = $1
           ORDER BY timestamp ASC NULLS LAST"#,
        id
    )
    .fetch_all(&state.db)
    .await?;

    if rows.is_empty() {
        return Err(AppError::NotFound(
            "No intent nodes found — run analysis first".into()
        ));
    }

    let repo = sqlx::query!(
        "SELECT owner, name FROM repos WHERE id = $1", id
    )
    .fetch_one(&state.db)
    .await?;

    let context = json!({
        "repository": format!("{}/{}",
            repo.owner.as_deref().unwrap_or("unknown"),
            repo.name.as_deref().unwrap_or("unknown")),
        "intent_nodes": rows.iter().map(|r| json!({
            "type":    r.node_type,
            "title":   r.title,
            "summary": r.summary
        })).collect::<Vec<_>>()
    });

    let prompt = format!(
        "Based on the following extracted intelligence nodes from a GitHub repository, \
         write a concise narrative (3–5 paragraphs) describing the project's decision history, \
         architectural evolution, key debates, and what was rejected and why. \
         Write it as a factual intelligence report, not a marketing document.\n\n{}",
        context
    );

    let base_url = std::env::var("NIM_BASE_URL")
        .unwrap_or_else(|_| "https://integrate.api.nvidia.com/v1".to_string());

    let request = crate::intelligence::client::NimRequest {
        model: "nvidia/llama-3.1-nemotron-70b-instruct".into(),
        messages: vec![
            crate::intelligence::client::Message {
                role:    "system".into(),
                content: "You are a codebase intelligence analyst writing structured repository intelligence reports.".into(),
            },
            crate::intelligence::client::Message {
                role:    "user".into(),
                content: prompt,
            },
        ],
        temperature: 0.3,
        max_tokens:  2048,
    };

    let resp = state.http_client
        .post(format!("{}/chat/completions", base_url))
        .header("Authorization", format!("Bearer {}", nim_api_key))
        .header("Content-Type", "application/json")
        .json(&request)
        .send()
        .await?;

    if !resp.status().is_success() {
        let status = resp.status();
        let body   = resp.text().await.unwrap_or_default();
        return Err(AppError::NimApiError(format!("NIM returned {}: {}", status, body)));
    }

    let nim_resp: crate::intelligence::client::NimResponse = resp.json().await?;
    let narrative = nim_resp
        .choices
        .first()
        .map(|c| c.message.content.clone())
        .unwrap_or_default();

    Ok(Json(json!({
        "repo_id":   id,
        "narrative": narrative,
        "node_count": rows.len()
    })))
}

// ── Internal helper ───────────────────────────────────────

async fn ensure_repo_exists(state: &Arc<AppState>, id: Uuid) -> Result<(), AppError> {
    let exists: bool = sqlx::query_scalar!(
        "SELECT EXISTS(SELECT 1 FROM repos WHERE id = $1)", id
    )
    .fetch_one(&state.db)
    .await?
    .unwrap_or(false);

    if !exists {
        return Err(AppError::NotFound(format!("Repository {} not found", id)));
    }
    Ok(())
}
