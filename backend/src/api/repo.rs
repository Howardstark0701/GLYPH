use axum::{
    extract::{Path, State},
    http::HeaderMap,
    Json,
};
use serde::Serialize;
use serde_json::{json, Value};
use std::sync::Arc;
use uuid::Uuid;

use crate::errors::AppError;
use crate::AppState;

// ── Shared row types ──────────────────────────────────────

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct RepoStatus {
    pub id:          String,
    pub status:      Option<String>,
    pub analyzed_at: Option<String>,
}

#[derive(Debug, sqlx::FromRow)]
struct RepoRow {
    owner: Option<String>,
    name:  Option<String>,
}

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

// ── GET /repo/:id/status ──────────────────────────────────

pub async fn get_status(
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> Result<Json<RepoStatus>, AppError> {
    let row = sqlx::query_as_unchecked!(
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

pub async fn get_intent(
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> Result<Json<Value>, AppError> {
    ensure_repo_exists(&state, id).await?;
    let rows = fetch_intent_rows(&state, id, None).await?;
    Ok(Json(json!({ "repo_id": id.to_string(), "nodes": rows })))
}

// ── GET /repo/:id/debates ─────────────────────────────────

pub async fn get_debates(
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> Result<Json<Value>, AppError> {
    ensure_repo_exists(&state, id).await?;
    let rows = fetch_intent_rows(&state, id, Some("debate")).await?;

    let total         = rows.len() as f64;
    let resolved      = rows.iter().filter(|r| {
        r.summary.as_deref().map(|s| s.to_lowercase().contains("resolved")).unwrap_or(false)
    }).count() as f64;
    let contention    = if total > 0.0 { ((total - resolved) / total * 100.0).round() } else { 0.0 };
    let agreement     = if total > 0.0 { (resolved / total * 100.0).round() } else { 0.0 };
    let conf_avg      = if total > 0.0 {
        rows.iter().filter_map(|r| r.confidence).sum::<f64>() / total * 100.0
    } else { 0.0 };

    Ok(Json(json!({
        "repo_id": id.to_string(),
        "debates": rows,
        "metrics": {
            "total":          total as u64,
            "agreement_pct":  agreement,
            "contention_pct": contention,
            "confidence_avg": conf_avg.round()
        }
    })))
}

// ── GET /repo/:id/decisions ───────────────────────────────

pub async fn get_decisions(
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> Result<Json<Value>, AppError> {
    ensure_repo_exists(&state, id).await?;
    let rows = fetch_intent_rows(&state, id, Some("decision")).await?;
    let total = rows.len();
    Ok(Json(json!({ "repo_id": id.to_string(), "decisions": rows, "total": total })))
}

// ── GET /repo/:id/rejections ──────────────────────────────

pub async fn get_rejections(
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> Result<Json<Value>, AppError> {
    ensure_repo_exists(&state, id).await?;
    let rows = fetch_intent_rows(&state, id, Some("rejection")).await?;
    let total = rows.len();
    Ok(Json(json!({ "repo_id": id.to_string(), "rejections": rows, "total": total })))
}

// ── GET /repo/:id/contributors ────────────────────────────

pub async fn get_contributors(
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> Result<Json<Value>, AppError> {
    ensure_repo_exists(&state, id).await?;
    let node_rows = fetch_intent_rows(&state, id, None).await?;

    use std::collections::HashMap;
    #[derive(Default)]
    struct Stats { decisions: u32, debates: u32, rejections: u32, total: u32, conf_sum: f64 }

    let mut map: HashMap<String, Stats> = HashMap::new();
    for node in &node_rows {
        if let Some(Value::Array(contribs)) = &node.contributors {
            for cv in contribs {
                if let Value::String(handle) = cv {
                    let e = map.entry(handle.clone()).or_default();
                    e.total    += 1;
                    e.conf_sum += node.confidence.unwrap_or(0.0);
                    match node.node_type.as_deref() {
                        Some("decision")  => e.decisions  += 1,
                        Some("debate")    => e.debates    += 1,
                        Some("rejection") => e.rejections += 1,
                        _ => {}
                    }
                }
            }
        }
    }

    let contributors: Vec<Value> = map.into_iter().map(|(handle, s)| {
        let avg = if s.total > 0 { s.conf_sum / s.total as f64 * 100.0 } else { 0.0 };
        json!({
            "handle":         handle,
            "decisions":      s.decisions,
            "debates":        s.debates,
            "rejections":     s.rejections,
            "total_nodes":    s.total,
            "avg_confidence": avg.round()
        })
    }).collect();

    Ok(Json(json!({ "repo_id": id.to_string(), "contributors": contributors })))
}

// ── GET /repo/:id/graph ───────────────────────────────────

pub async fn get_graph(
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> Result<Json<Value>, AppError> {
    ensure_repo_exists(&state, id).await?;
    let rows = fetch_intent_rows(&state, id, None).await?;

    let nodes: Vec<Value> = rows.iter().map(|r| json!({
        "id":         r.id,
        "label":      r.title.as_deref().unwrap_or("UNKNOWN"),
        "type":       r.node_type.as_deref().unwrap_or("unknown"),
        "summary":    r.summary,
        "confidence": r.confidence,
        "sha": r.source_refs
            .as_ref()
            .and_then(|v| v.as_array())
            .and_then(|a| a.first())
            .and_then(|v| v.as_str())
            .map(|s| format!("0x{}", &s[..s.len().min(7)]))
            .unwrap_or_else(|| "0x0000000".into())
    })).collect();

    let mut links: Vec<Value> = Vec::new();
    let decision_ids: Vec<&str> = rows.iter()
        .filter(|r| r.node_type.as_deref() == Some("decision"))
        .map(|r| r.id.as_str())
        .collect();

    for w in decision_ids.windows(2) {
        links.push(json!({ "source": w[0], "target": w[1], "type": "decision" }));
    }
    for r in rows.iter().filter(|r| r.node_type.as_deref() == Some("rejection")) {
        if let Some(d) = decision_ids.first() {
            links.push(json!({ "source": d, "target": r.id, "type": "rejected" }));
        }
    }
    for r in rows.iter().filter(|r| r.node_type.as_deref() == Some("debate")) {
        if let Some(d) = decision_ids.first() {
            links.push(json!({ "source": d, "target": r.id, "type": "debate" }));
        }
    }

    let nn = nodes.len();
    let nl = links.len();
    let density = if nn > 1 { (nl as f64 / (nn * (nn - 1) / 2) as f64 * 100.0).round() as u64 } else { 0 };

    Ok(Json(json!({
        "repo_id": id.to_string(),
        "nodes":   nodes,
        "links":   links,
        "meta":    { "total_nodes": nn, "total_links": nl, "density_pct": density }
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

    let rows = fetch_intent_rows(&state, id, None).await?;
    if rows.is_empty() {
        return Err(AppError::NotFound("No intent nodes found — run analysis first".into()));
    }

    // Fetch repo owner/name
    let repo = sqlx::query_as_unchecked!(
        RepoRow,
        "SELECT owner, name FROM repos WHERE id = $1",
        id
    )
    .fetch_one(&state.db)
    .await?;

    let context = json!({
        "repository": format!("{}/{}", repo.owner.as_deref().unwrap_or("unknown"), repo.name.as_deref().unwrap_or("unknown")),
        "intent_nodes": rows.iter().map(|r| json!({
            "type": r.node_type, "title": r.title, "summary": r.summary
        })).collect::<Vec<_>>()
    });

    let prompt = format!(
        "Based on the following extracted intelligence nodes from a GitHub repository, \
         write a concise narrative (3-5 paragraphs) describing the project's decision history, \
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
    let narrative = nim_resp.choices.first().map(|c| c.message.content.clone()).unwrap_or_default();

    Ok(Json(json!({ "repo_id": id.to_string(), "narrative": narrative, "node_count": rows.len() })))
}

// ── Helpers ───────────────────────────────────────────────

async fn ensure_repo_exists(state: &Arc<AppState>, id: Uuid) -> Result<(), AppError> {
    let exists: bool = sqlx::query_scalar_unchecked!(
        "SELECT EXISTS(SELECT 1 FROM repos WHERE id = $1)",
        id
    )
    .fetch_one(&state.db)
    .await?
    .unwrap_or(false);

    if !exists {
        return Err(AppError::NotFound(format!("Repository {} not found", id)));
    }
    Ok(())
}

async fn fetch_intent_rows(
    state:     &Arc<AppState>,
    id:        Uuid,
    node_type: Option<&str>,
) -> Result<Vec<IntentRow>, AppError> {
    let rows = if let Some(nt) = node_type {
        sqlx::query_as_unchecked!(
            IntentRow,
            "SELECT id::text, node_type, title, summary, reasoning,
                    contributors, source_refs, timestamp::text, confidence
             FROM intent_nodes
             WHERE repo_id = $1 AND node_type = $2
             ORDER BY timestamp ASC NULLS LAST",
            id, nt
        )
        .fetch_all(&state.db)
        .await?
    } else {
        sqlx::query_as_unchecked!(
            IntentRow,
            "SELECT id::text, node_type, title, summary, reasoning,
                    contributors, source_refs, timestamp::text, confidence
             FROM intent_nodes
             WHERE repo_id = $1
             ORDER BY timestamp ASC NULLS LAST",
            id
        )
        .fetch_all(&state.db)
        .await?
    };
    Ok(rows)
}
