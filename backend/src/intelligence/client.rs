use crate::errors::AppError;
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Ordered NIM model candidates, best first.
///
/// NVIDIA retires hosted models on a rolling basis: a model can still be
/// listed by `GET /v1/models` while `POST /chat/completions` answers 404
/// ("Function ...: Not found for account") or 410 ("reached its end of life").
/// `llama-3.1-nemotron-70b-instruct` — the original single hardcoded default —
/// did exactly that, and because extraction failures fall back to the
/// deterministic path, every analysis silently degraded to scraping commit
/// titles with no visible error. Walking a chain keeps the LLM layer alive
/// when the head of the list disappears.
pub const DEFAULT_MODELS: &[&str] = &[
    "nvidia/nemotron-3-super-120b-a12b",
    "nvidia/nemotron-3.5-lightning-30b-a3b",
    "openai/gpt-oss-20b",
    "nvidia/llama-3.1-nemotron-70b-instruct",
];

pub const DEFAULT_MODEL: &str = DEFAULT_MODELS[0];

/// The candidate chain for this process. `NIM_MODEL` pins a single model
/// (explicit operator choice wins over the fallback chain).
pub fn model_candidates() -> Vec<String> {
    match std::env::var("NIM_MODEL") {
        Ok(m) if !m.trim().is_empty() => vec![m.trim().to_string()],
        _ => DEFAULT_MODELS.iter().map(|s| s.to_string()).collect(),
    }
}

pub fn model_from_env() -> String {
    model_candidates().into_iter().next().unwrap_or_else(|| DEFAULT_MODEL.to_string())
}

/// First model observed to work in this process. Once one answers, every
/// later call goes straight to it instead of re-probing dead models — a
/// 6-window analysis would otherwise pay the 404 round-trips six times over.
static RESOLVED_MODEL: std::sync::OnceLock<std::sync::Mutex<Option<String>>> =
    std::sync::OnceLock::new();

fn resolved_model() -> Option<String> {
    RESOLVED_MODEL
        .get_or_init(|| std::sync::Mutex::new(None))
        .lock()
        .ok()
        .and_then(|g| g.clone())
}

fn set_resolved_model(model: &str) {
    if let Ok(mut g) = RESOLVED_MODEL
        .get_or_init(|| std::sync::Mutex::new(None))
        .lock()
    {
        *g = Some(model.to_string());
    }
}

/// A status that means "this model is gone", as opposed to a request problem
/// (bad key, rate limit, malformed body) that would fail identically on every
/// other model and so must not trigger a pointless walk down the chain.
fn model_unavailable(status: reqwest::StatusCode) -> bool {
    status == reqwest::StatusCode::NOT_FOUND || status == reqwest::StatusCode::GONE
}

#[derive(Debug, Serialize)]
pub struct NimRequest {
    pub model: String,
    pub messages: Vec<Message>,
    pub temperature: f32,
    pub max_tokens: u32,
}

#[derive(Debug, Serialize)]
pub struct Message {
    pub role: String,
    pub content: String,
}

#[derive(Debug, Deserialize)]
pub struct NimResponse {
    pub choices: Vec<Choice>,
}

#[derive(Debug, Deserialize)]
pub struct Choice {
    pub message: ResponseMessage,
}

#[derive(Debug, Deserialize)]
pub struct ResponseMessage {
    pub content: String,
}

/// One extracted insight.
///
/// Every field is `#[serde(default)]` and the loose fields are tolerant on
/// purpose. LLMs drop keys they consider obvious, return `confidence` as the
/// string "0.9" or as 90, and collapse a one-element `contributors` array to a
/// bare string. Under strict deserialization any one of those made serde
/// reject the *entire* array, so a single sloppy object discarded a whole
/// extraction window and the job silently fell back to commit-title scraping.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ExtractedInsight {
    #[serde(default = "default_node_type")]
    pub node_type:    String,
    #[serde(default)]
    pub title:        String,
    #[serde(default)]
    pub summary:      String,
    #[serde(default)]
    pub reasoning:    String,
    #[serde(default, deserialize_with = "de_string_list")]
    pub contributors: Vec<String>,
    #[serde(default, deserialize_with = "de_string_list")]
    pub source_refs:  Vec<String>,
    #[serde(default, deserialize_with = "de_confidence")]
    pub confidence:   f32,
}

fn default_node_type() -> String {
    "decision".to_string()
}

/// Accept `["a","b"]`, `"a"`, `null`, or a list of objects carrying a name.
fn de_string_list<'de, D>(d: D) -> Result<Vec<String>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let value = Value::deserialize(d)?;
    Ok(match value {
        Value::Null => Vec::new(),
        Value::String(s) => {
            if s.trim().is_empty() { Vec::new() } else { vec![s] }
        }
        Value::Array(items) => items
            .into_iter()
            .filter_map(|v| match v {
                Value::String(s) => Some(s),
                Value::Number(n) => Some(n.to_string()),
                // e.g. {"login": "octocat"} or {"name": "octocat"}
                Value::Object(o) => o
                    .get("login")
                    .or_else(|| o.get("name"))
                    .or_else(|| o.get("handle"))
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string()),
                _ => None,
            })
            .filter(|s| !s.trim().is_empty())
            .collect(),
        _ => Vec::new(),
    })
}

/// Accept `0.85`, `"0.85"`, `85`, or `"85%"`, normalised to 0.0..=1.0.
fn de_confidence<'de, D>(d: D) -> Result<f32, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let value = Value::deserialize(d)?;
    let raw = match value {
        Value::Number(n) => n.as_f64().unwrap_or(0.0) as f32,
        Value::String(s) => s
            .trim()
            .trim_end_matches('%')
            .parse::<f32>()
            .unwrap_or(0.0),
        _ => 0.0,
    };
    // A model asked for a 0-1 score often answers on a 0-100 scale.
    let normalised = if raw > 1.0 { raw / 100.0 } else { raw };
    Ok(normalised.clamp(0.0, 1.0))
}

/// One consolidated chat call — the only NIM HTTP path in the app. Both the
/// analysis pipeline and the on-demand narrative summary go through here, so
/// the model, base URL, and error mapping stay in one place.
///
/// `model` is the preferred model. If it is unavailable on the caller's
/// account (404/410), the remaining candidates are tried in order before
/// giving up, and the first one that answers is remembered for the rest of
/// the process. Any other failure (bad key, rate limit, bad request) is
/// returned immediately — retrying it against another model just wastes time.
pub async fn chat(
    client:      &reqwest::Client,
    api_key:     &str,
    model:       &str,
    system:      &str,
    user:        &str,
    temperature: f32,
    max_tokens:  u32,
) -> Result<String, AppError> {
    // Preferred model first, then anything already proven to work, then the
    // rest of the chain — de-duplicated, order preserved.
    let mut chain: Vec<String> = vec![model.to_string()];
    if let Some(known) = resolved_model() {
        chain.push(known);
    }
    chain.extend(model_candidates());
    chain.dedup();
    let mut seen = std::collections::HashSet::new();
    chain.retain(|m| seen.insert(m.clone()));

    let mut last_err = None;
    for candidate in &chain {
        match chat_once(client, api_key, candidate, system, user, temperature, max_tokens).await {
            Ok(content) => {
                set_resolved_model(candidate);
                return Ok(content);
            }
            Err(AppError::NimModelUnavailable(msg)) => {
                tracing::warn!("NIM model '{candidate}' unavailable, trying next: {msg}");
                last_err = Some(AppError::NimApiError(msg));
            }
            Err(e) => return Err(e),
        }
    }

    Err(last_err.unwrap_or_else(|| {
        AppError::NimApiError("No NIM model available for this account".into())
    }))
}

/// A single chat request against one specific model.
#[allow(clippy::too_many_arguments)]
async fn chat_once(
    client:      &reqwest::Client,
    api_key:     &str,
    model:       &str,
    system:      &str,
    user:        &str,
    temperature: f32,
    max_tokens:  u32,
) -> Result<String, AppError> {
    let base_url = std::env::var("NIM_BASE_URL")
        .unwrap_or_else(|_| "https://integrate.api.nvidia.com/v1".to_string());

    let request = NimRequest {
        model: model.to_string(),
        messages: vec![
            Message { role: "system".into(), content: system.into() },
            Message { role: "user".into(),   content: user.into() },
        ],
        temperature,
        max_tokens,
    };

    // The shared AppState client carries a 60s timeout, which suits GitHub's
    // fast REST calls but is shorter than a single NIM completion: current
    // reasoning models spend ~100s on an extraction window, so every LLM call
    // timed out and the pipeline fell back to deterministic extraction with
    // only a generic "error sending request" in the log. Give NIM its own
    // budget rather than loosening the timeout for GitHub too.
    let nim_timeout = std::env::var("NIM_TIMEOUT_SECS")
        .ok()
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(300);

    let resp = client
        .post(format!("{}/chat/completions", base_url))
        .header("Authorization", format!("Bearer {}", api_key))
        .header("Content-Type", "application/json")
        .timeout(std::time::Duration::from_secs(nim_timeout))
        .json(&request)
        .send()
        .await
        .map_err(|e| AppError::NimApiError(format!("NIM request failed: {e}")))?;

    if !resp.status().is_success() {
        let status = resp.status();
        let body   = resp.text().await.unwrap_or_default();
        let msg = format!("NIM API returned {} for model '{}': {}", status, model, body);
        // Distinguish "this model is gone" from "this request is bad" so the
        // caller knows whether trying another model could possibly help.
        return Err(if model_unavailable(status) {
            AppError::NimModelUnavailable(msg)
        } else {
            AppError::NimApiError(msg)
        });
    }

    let nim_resp: NimResponse = resp
        .json()
        .await
        .map_err(|e| AppError::NimApiError(format!("Invalid NIM response envelope: {e}")))?;

    nim_resp
        .choices
        .into_iter()
        .next()
        .map(|c| c.message.content)
        .ok_or_else(|| AppError::NimApiError("Empty NIM response".into()))
}

/// Call NIM with a fully-built prompt string, parse and return insights.
pub async fn analyze_events_with_prompt(
    client:  &reqwest::Client,
    api_key: &str,
    prompt:  &str,
) -> Result<Vec<ExtractedInsight>, AppError> {
    let system = crate::intelligence::prompts::SYSTEM_PROMPT;
    // 8192, not 4096: reasoning-tuned models spend thousands of tokens before
    // the JSON, and a window truncated mid-array parses as nothing at all.
    let content = chat(client, api_key, &model_from_env(), system, prompt, 0.1, 8192).await?;

    parse_insights(&content).map_err(|e| {
        AppError::NimApiError(format!(
            "Failed to parse NIM JSON response: {}. Raw: {}",
            e,
            &content[..content.len().min(200)]
        ))
    })
}

/// Robust extraction of a `Vec<ExtractedInsight>` from NIM output. NIM models
/// are not guaranteed to emit bare JSON: they wrap it in markdown fences, tuck
/// it inside a "Here is the result:" narrative, or nest it under an object key.
/// This walks every reasonable shape before giving up.
pub fn parse_insights(content: &str) -> Result<Vec<ExtractedInsight>, String> {
    // Reasoning models (nemotron-3-*, gpt-oss) narrate before answering and
    // may wrap the narration in <think> tags. Drop that first — the prose is
    // full of brackets and braces that wreck naive span scanning.
    let content = strip_think_blocks(content);
    let trimmed = content.trim();

    // 1. Bare array — the happy path.
    if trimmed.starts_with('[') {
        if let Ok(v) = serde_json::from_str::<Vec<ExtractedInsight>>(trimmed) {
            return Ok(v);
        }
    }

    // 2. Strip markdown fences, then retry as a bare array.
    let unfenced = strip_code_fences(&content);
    if unfenced.starts_with('[') {
        if let Ok(v) = serde_json::from_str::<Vec<ExtractedInsight>>(unfenced) {
            return Ok(v);
        }
    }

    // 3. Every fenced block in the response, in order. A model that thinks
    //    out loud often puts the real answer in the *last* ```json block.
    for block in fenced_blocks(&content).iter().rev() {
        if let Some(v) = insights_from_json_text(block) {
            return Ok(v);
        }
    }

    // 4. Every balanced [ … ] and { … } span in the whole response, longest
    //    first. The previous version took first '[' to last ']', which spans
    //    straight across any bracket that appears in the surrounding prose
    //    and therefore never parses.
    let mut spans = balanced_spans(&content, '[', ']');
    spans.extend(balanced_spans(&content, '{', '}'));
    spans.sort_by_key(|s| std::cmp::Reverse(s.len()));
    for span in spans {
        if let Some(v) = insights_from_json_text(&span) {
            return Ok(v);
        }
    }

    Err("no JSON array of insights found in response".into())
}

/// Parse one chunk of candidate JSON as either a bare insight array or an
/// object wrapping one under a common key.
fn insights_from_json_text(text: &str) -> Option<Vec<ExtractedInsight>> {
    let text = text.trim();
    if text.is_empty() {
        return None;
    }
    if let Ok(v) = serde_json::from_str::<Vec<ExtractedInsight>>(text) {
        if !v.is_empty() {
            return Some(v);
        }
    }
    let value: Value = serde_json::from_str(text).ok()?;
    let obj = value.as_object()?;
    for key in [
        "insights", "data", "nodes", "result", "results",
        "decisions", "items", "intent_nodes", "output",
    ] {
        if let Some(arr) = obj.get(key).and_then(|v| v.as_array()) {
            if let Ok(v) =
                serde_json::from_value::<Vec<ExtractedInsight>>(Value::Array(arr.clone()))
            {
                if !v.is_empty() {
                    return Some(v);
                }
            }
        }
    }
    None
}

/// Remove `<think> … </think>` / `<reasoning> … </reasoning>` sections.
fn strip_think_blocks(content: &str) -> String {
    let mut out = content.to_string();
    for (open, close) in [("<think>", "</think>"), ("<reasoning>", "</reasoning>")] {
        loop {
            let Some(a) = out.find(open) else { break };
            match out[a..].find(close) {
                Some(rel) => {
                    let b = a + rel + close.len();
                    out.replace_range(a..b, "");
                }
                // Unclosed tag: everything after it is reasoning, drop it.
                None => {
                    out.truncate(a);
                    break;
                }
            }
        }
    }
    out
}

/// Contents of every ``` fenced block, in document order.
fn fenced_blocks(content: &str) -> Vec<String> {
    let mut blocks = Vec::new();
    let mut rest = content;
    while let Some(a) = rest.find("```") {
        let after = &rest[a + 3..];
        // Skip an optional language tag on the opening line.
        let body_start = after.find('\n').map(|i| i + 1).unwrap_or(0);
        let body = &after[body_start..];
        match body.find("```") {
            Some(b) => {
                blocks.push(body[..b].to_string());
                rest = &body[b + 3..];
            }
            None => {
                blocks.push(body.to_string());
                break;
            }
        }
    }
    blocks
}

/// Every balanced `open … close` span in `content`, skipping brackets that
/// appear inside JSON string literals (and their escapes).
fn balanced_spans(content: &str, open: char, close: char) -> Vec<String> {
    let chars: Vec<char> = content.chars().collect();
    let mut spans = Vec::new();
    let mut i = 0usize;

    while i < chars.len() {
        if chars[i] != open {
            i += 1;
            continue;
        }
        let mut depth = 0i32;
        let mut in_str = false;
        let mut escaped = false;
        let mut j = i;
        while j < chars.len() {
            let c = chars[j];
            if in_str {
                if escaped {
                    escaped = false;
                } else if c == '\\' {
                    escaped = true;
                } else if c == '"' {
                    in_str = false;
                }
            } else if c == '"' {
                in_str = true;
            } else if c == open {
                depth += 1;
            } else if c == close {
                depth -= 1;
                if depth == 0 {
                    spans.push(chars[i..=j].iter().collect());
                    break;
                }
            }
            j += 1;
        }
        i += 1;
        // Cap the work on pathological output.
        if spans.len() >= 64 {
            break;
        }
    }
    spans
}

fn strip_code_fences(s: &str) -> &str {
    let s = s.trim();
    if let Some(inner) = s.strip_prefix("```json") {
        inner.strip_suffix("```").unwrap_or(inner).trim()
    } else if let Some(inner) = s.strip_prefix("```") {
        inner.strip_suffix("```").unwrap_or(inner).trim()
    } else {
        s
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_bare_array() {
        let json = r#"[{"node_type":"decision","title":"t","summary":"s","reasoning":"r","contributors":["a"],"source_refs":["b"],"confidence":0.9}]"#;
        let insights = parse_insights(json).unwrap();
        assert_eq!(insights.len(), 1);
        assert_eq!(insights[0].node_type, "decision");
        assert_eq!(insights[0].confidence, 0.9);
    }

    #[test]
    fn parses_markdown_fenced_array() {
        let json = "```json\n[{\"node_type\":\"debate\",\"title\":\"t\",\"summary\":\"s\",\"reasoning\":\"r\",\"contributors\":[],\"source_refs\":[],\"confidence\":0.5}]\n```";
        let insights = parse_insights(json).unwrap();
        assert_eq!(insights.len(), 1);
        assert_eq!(insights[0].node_type, "debate");
    }

    #[test]
    fn parses_prose_wrapped_array() {
        let json = "Here is the result of the analysis:\n\n[{\"node_type\":\"rejection\",\"title\":\"t\",\"summary\":\"s\",\"reasoning\":\"r\",\"contributors\":[],\"source_refs\":[],\"confidence\":0.3}]\n\nHope that helps!";
        let insights = parse_insights(json).unwrap();
        assert_eq!(insights.len(), 1);
        assert_eq!(insights[0].node_type, "rejection");
    }

    #[test]
    fn parses_object_wrapped_array() {
        let json = "{\"insights\": [{\"node_type\":\"decision\",\"title\":\"t\",\"summary\":\"s\",\"reasoning\":\"r\",\"contributors\":[],\"source_refs\":[],\"confidence\":0.9}]}";
        let insights = parse_insights(json).unwrap();
        assert_eq!(insights.len(), 1);
    }

    /// Reasoning-tuned models narrate before answering and the narration
    /// routinely contains brackets. The old "first [ to last ]" span therefore
    /// ran from a bracket in the prose to another in the prose and never
    /// parsed — in production that dropped whole extraction windows and fell
    /// back to commit-title scraping with no visible error.
    #[test]
    fn parses_array_after_bracket_heavy_reasoning() {
        let raw = format!(
            "We are given a list of commits [see above] and pull requests.
             Step 1: identify decisions [decision nodes].
             Step 2: emit the array [as requested].

[{}]",
            r#"{"node_type":"decision","title":"t","summary":"s","reasoning":"r","contributors":[],"source_refs":[],"confidence":0.7}"#
        );
        let insights = parse_insights(&raw).unwrap();
        assert_eq!(insights.len(), 1);
        assert_eq!(insights[0].node_type, "decision");
    }

    /// A `<think>` block must be discarded, not mined for JSON.
    #[test]
    fn ignores_think_block_and_uses_the_real_answer() {
        let raw = format!(
            "<think>I could emit [{}] but let me reconsider.</think>[{}]",
            r#"{"node_type":"debate","title":"t","summary":"s","reasoning":"r","contributors":[],"source_refs":[],"confidence":0.7}"#,
            r#"{"node_type":"rejection","title":"t","summary":"s","reasoning":"r","contributors":[],"source_refs":[],"confidence":0.7}"#
        );
        let insights = parse_insights(&raw).unwrap();
        assert_eq!(insights.len(), 1);
        assert_eq!(
            insights[0].node_type, "rejection",
            "must take the answer, not the discarded reasoning"
        );
    }

    /// The answer often lands in the last fenced block, after the model has
    /// shown working in an earlier one.
    #[test]
    fn prefers_the_last_fenced_block() {
        let raw = format!(
            "Draft:
```json
[{}]
```
On reflection:
```json
[{}]
```",
            r#"{"node_type":"debate","title":"t","summary":"s","reasoning":"r","contributors":[],"source_refs":[],"confidence":0.7}"#,
            r#"{"node_type":"rejection","title":"t","summary":"s","reasoning":"r","contributors":[],"source_refs":[],"confidence":0.7}"#
        );
        let insights = parse_insights(&raw).unwrap();
        assert_eq!(insights.len(), 1);
        assert_eq!(insights[0].node_type, "rejection");
    }

    /// A window truncated at max_tokens leaves an unbalanced array. That is a
    /// real failure and must be reported, not silently half-parsed.
    #[test]
    fn rejects_truncated_array() {
        let truncated = format!(r#"[{}, {{"node_type":"decision","title":"unfinis"#, r#"{"node_type":"decision","title":"t","summary":"s","reasoning":"r","contributors":[],"source_refs":[],"confidence":0.7}"#);
        assert!(parse_insights(&truncated).is_err());
    }

    /// Real NIM output is sloppy: keys go missing, confidence arrives as a
    /// percentage string, and a single contributor collapses to a bare string.
    /// None of that may discard the array.
    #[test]
    fn tolerates_missing_keys_and_loose_types() {
        let raw = r#"[
          {"node_type":"decision","title":"Adopt Axum","confidence":"85%"},
          {"title":"No node_type given","contributors":"solo_dev","confidence":0.4},
          {"node_type":"rejection","title":"Scaled 0-100","confidence":72,
           "contributors":[{"login":"octocat"}],"source_refs":"abc123"}
        ]"#;
        let insights = parse_insights(raw).unwrap();
        assert_eq!(insights.len(), 3);

        // "85%" -> 0.85
        assert!((insights[0].confidence - 0.85).abs() < 1e-4);
        assert_eq!(insights[0].summary, "", "missing keys default, not fail");

        // node_type defaults rather than dropping the object
        assert_eq!(insights[1].node_type, "decision");
        assert_eq!(insights[1].contributors, vec!["solo_dev".to_string()]);

        // 72 on a 0-100 scale -> 0.72; {"login": ...} -> the handle
        assert!((insights[2].confidence - 0.72).abs() < 1e-4);
        assert_eq!(insights[2].contributors, vec!["octocat".to_string()]);
        assert_eq!(insights[2].source_refs, vec!["abc123".to_string()]);
    }

    #[test]
    fn rejects_garbage() {
        assert!(parse_insights("no json here at all").is_err());
        assert!(parse_insights("").is_err());
    }

    #[test]
    fn strips_fences() {
        assert_eq!(strip_code_fences("```json\n[1,2]\n```"), "[1,2]");
        assert_eq!(strip_code_fences("```\n[1,2]\n```"), "[1,2]");
        assert_eq!(strip_code_fences("[1,2]"), "[1,2]");
    }
}
