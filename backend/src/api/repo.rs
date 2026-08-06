use axum::{extract::{Path, State}, http::HeaderMap, Json};
use serde::Serialize;
use serde_json::{json, Value};
use sqlx::Row;
use std::sync::Arc;
use uuid::Uuid;

use crate::errors::AppError;
use crate::AppState;

// ── Row structs ───────────────────────────────────────────

#[derive(Debug, Serialize)]
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
) -> Result<Json<Value>, AppError> {
    match sqlx::query(
        "SELECT id::text, status, stage, error_message, analyzed_at::text
         FROM repos WHERE id = $1"
    )
    .bind(id)
    .fetch_optional(&state.db)
    .await {
        Ok(Some(row)) => Ok(Json(json!({
            "id": row.try_get::<String,_>("id").unwrap_or_default(),
            "status": row.try_get::<String,_>("status").ok(),
            "stage": row.try_get::<String,_>("stage").ok(),
            "error_message": row.try_get::<String,_>("error_message").ok(),
            "analyzed_at": row.try_get::<String,_>("analyzed_at").ok(),
        }))),
        Ok(None) => Err(AppError::NotFound("repo not found".into())),
        Err(e) => Err(AppError::DatabaseError(e.to_string())),
    }
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

    let total      = rows.len() as f64;
    let resolved   = rows.iter().filter(|r| {
        r.summary.as_deref().map(|s| s.to_lowercase().contains("resolved")).unwrap_or(false)
    }).count() as f64;
    let contention = if total > 0.0 { ((total - resolved) / total * 100.0).round() } else { 0.0 };
    let agreement  = if total > 0.0 { (resolved / total * 100.0).round() } else { 0.0 };
    let conf_avg   = if total > 0.0 {
        rows.iter().filter_map(|r| r.confidence).sum::<f64>() / total * 100.0
    } else { 0.0 };

    Ok(Json(json!({
        "repo_id": id.to_string(), "debates": rows,
        "metrics": { "total": total as u64, "agreement_pct": agreement,
                     "contention_pct": contention, "confidence_avg": conf_avg.round() }
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
        json!({ "handle": handle, "decisions": s.decisions, "debates": s.debates,
                "rejections": s.rejections, "total_nodes": s.total, "avg_confidence": avg.round() })
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
        "id": r.id, "label": r.title.as_deref().unwrap_or("UNKNOWN"),
        "type": r.node_type.as_deref().unwrap_or("unknown"),
        "summary": r.summary, "confidence": r.confidence,
        "sha": r.source_refs.as_ref()
            .and_then(|v| v.as_array()).and_then(|a| a.first())
            .and_then(|v| v.as_str())
            .map(|s| format!("0x{}", &s[..s.len().min(7)]))
            .unwrap_or_else(|| "N/A".into())
    })).collect();

    let mut links: Vec<Value> = Vec::new();
    let decision_ids: Vec<&str> = rows.iter()
        .filter(|r| r.node_type.as_deref() == Some("decision"))
        .map(|r| r.id.as_str()).collect();

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
        "repo_id": id.to_string(), "nodes": nodes, "links": links,
        "meta": { "total_nodes": nn, "total_links": nl, "density_pct": density }
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

    // Get repo owner/name with proper error handling
    let (owner, name) = match sqlx::query("SELECT owner, name FROM repos WHERE id = $1")
        .bind(id)
        .fetch_optional(&state.db)
        .await {
        Ok(Some(row)) => {
            let owner: Option<String> = row.try_get("owner").ok();
            let name: Option<String> = row.try_get("name").ok();
            (owner.unwrap_or_else(|| "unknown".into()), name.unwrap_or_else(|| "unknown".into()))
        },
        Ok(None) => ("unknown".into(), "unknown".into()),
        Err(e) => {
            tracing::error!("get_summary: failed to fetch repo: {:?}", e);
            return Err(AppError::DatabaseError(e.to_string()));
        }
    };

    let context = json!({
        "repository": format!("{}/{}", owner, name),
        "intent_nodes": rows.iter().map(|r| json!({ "type": r.node_type, "title": r.title, "summary": r.summary })).collect::<Vec<_>>()
    });

    let prompt = format!(
        "Based on the following extracted intelligence nodes from a GitHub repository, \
         write a concise narrative (3-5 paragraphs) describing the project's decision history, \
         architectural evolution, key debates, and what was rejected and why. \
         Write it as a factual intelligence report, not a marketing document.\n\n{}",
        context
    );

    let system = "You are a codebase intelligence analyst writing structured repository intelligence reports.";
    let narrative = crate::intelligence::client::chat(
        &state.http_client,
        nim_api_key,
        &crate::intelligence::client::model_from_env(),
        system,
        &prompt,
        0.3,
        2048,
    )
    .await?;

    Ok(Json(json!({ "repo_id": id.to_string(), "narrative": narrative, "node_count": rows.len() })))
}

// ── POST /repo/:id/override ───────────────────────────────

pub async fn override_repo(
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> Result<Json<Value>, AppError> {
    match sqlx::query("UPDATE repos SET status = 'override' WHERE id = $1")
        .bind(id)
        .execute(&state.db)
        .await {
        Ok(_) => {
            let ts = chrono::Utc::now().to_rfc3339();
            Ok(Json(json!({
                "repo_id": id.to_string(),
                "status": "override",
                "message": "Manual override initialized. Repository analysis queued for reprocessing.",
                "timestamp": ts
            })))
        },
        Err(e) => Err(AppError::DatabaseError(e.to_string())),
    }
}

// ── POST /repo/:id/terminate ──────────────────────────────

/// Request cancellation of an in-flight analysis. Flips the in-memory cancel
/// flag (checked between phases); run_analysis records status "terminated".
/// Idempotent — a repo with no running job is just marked terminated.
pub async fn terminate_repo(
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> Result<Json<Value>, AppError> {
    {
        let mut guards = state.cancellations.lock().unwrap();
        if let Some(flag) = guards.remove(&id) {
            flag.store(true, std::sync::atomic::Ordering::Relaxed);
        }
    } // guard dropped — never held across await

    let updated = sqlx::query("UPDATE repos SET status = 'terminated' WHERE id = $1")
        .bind(id)
        .execute(&state.db)
        .await
        .map_err(|e| AppError::DatabaseError(e.to_string()))?;

    if updated.rows_affected() == 0 {
        return Err(AppError::NotFound(format!("Repository {} not found", id)));
    }

    Ok(Json(json!({
        "repo_id": id.to_string(),
        "status": "terminated",
        "message": "Analysis termination requested. In-flight work will stop at the next checkpoint.",
    })))
}

// ── Helpers ───────────────────────────────────────────────

async fn ensure_repo_exists(state: &Arc<AppState>, id: Uuid) -> Result<(), AppError> {
    match sqlx::query("SELECT 1 FROM repos WHERE id = $1")
        .bind(id)
        .fetch_optional(&state.db)
        .await {
        Ok(Some(_)) => Ok(()),
        Ok(None) => Err(AppError::NotFound(format!("Repository {} not found", id))),
        Err(e) => Err(AppError::DatabaseError(e.to_string())),
    }
}

async fn fetch_intent_rows(
    state:     &Arc<AppState>,
    id:        Uuid,
    node_type: Option<&str>,
) -> Result<Vec<IntentRow>, AppError> {
    let sql_rows = if let Some(nt) = node_type {
        match sqlx::query(
            "SELECT id::text, node_type, title, summary, reasoning,
                    contributors, source_refs, timestamp::text, confidence
             FROM intent_nodes WHERE repo_id = $1 AND node_type = $2
             ORDER BY timestamp ASC NULLS LAST",
        )
        .bind(id).bind(nt)
        .fetch_all(&state.db).await {
            Ok(rows) => rows,
            Err(e) => return Err(AppError::DatabaseError(e.to_string())),
        }
    } else {
        match sqlx::query(
            "SELECT id::text, node_type, title, summary, reasoning,
                    contributors, source_refs, timestamp::text, confidence
             FROM intent_nodes WHERE repo_id = $1
             ORDER BY timestamp ASC NULLS LAST",
        )
        .bind(id)
        .fetch_all(&state.db).await {
            Ok(rows) => rows,
            Err(e) => return Err(AppError::DatabaseError(e.to_string())),
        }
    };

    let rows = sql_rows.iter().map(|r| IntentRow {
        id:           r.try_get("id").unwrap_or_default(),
        node_type:    r.try_get("node_type").ok(),
        title:        r.try_get("title").ok(),
        summary:      r.try_get("summary").ok(),
        reasoning:    r.try_get("reasoning").ok(),
        contributors: r.try_get("contributors").ok(),
        source_refs:  r.try_get("source_refs").ok(),
        timestamp:    r.try_get("timestamp").ok(),
        confidence:   r.try_get("confidence").ok(),
    }).collect();

    Ok(rows)
}
