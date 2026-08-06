use crate::errors::AppError;
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Default NIM model — override with the `NIM_MODEL` env var.
pub const DEFAULT_MODEL: &str = "nvidia/llama-3.1-nemotron-70b-instruct";

pub fn model_from_env() -> String {
    std::env::var("NIM_MODEL").unwrap_or_else(|_| DEFAULT_MODEL.to_string())
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

#[derive(Debug, Serialize, Deserialize)]
pub struct ExtractedInsight {
    pub node_type:    String,
    pub title:        String,
    pub summary:      String,
    pub reasoning:    String,
    pub contributors: Vec<String>,
    pub source_refs:  Vec<String>,
    pub confidence:   f32,
}

/// One consolidated chat call — the only NIM HTTP path in the app. Both the
/// analysis pipeline and the on-demand narrative summary go through here, so
/// the model, base URL, and error mapping stay in one place.
pub async fn chat(
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

    let resp = client
        .post(format!("{}/chat/completions", base_url))
        .header("Authorization", format!("Bearer {}", api_key))
        .header("Content-Type", "application/json")
        .json(&request)
        .send()
        .await
        .map_err(|e| AppError::NimApiError(format!("NIM request failed: {e}")))?;

    if !resp.status().is_success() {
        let status = resp.status();
        let body   = resp.text().await.unwrap_or_default();
        return Err(AppError::NimApiError(format!(
            "NIM API returned {}: {}",
            status, body
        )));
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
    let content = chat(client, api_key, &model_from_env(), system, prompt, 0.1, 4096).await?;

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
    let trimmed = content.trim();

    // 1. Bare array — the happy path.
    if trimmed.starts_with('[') {
        return serde_json::from_str(trimmed)
            .map_err(|e| format!("array parse failed: {e}"));
    }

    // 2. Strip markdown fences, then retry as a bare array.
    let unfenced = strip_code_fences(content);
    if unfenced.starts_with('[') {
        return serde_json::from_str(unfenced)
            .map_err(|e| format!("fence-stripped array parse failed: {e}"));
    }

    // 3. Scan for the first [ … last ] span — rescues prose-wrapped output.
    if let (Some(start), Some(end)) = (unfenced.find('['), unfenced.rfind(']')) {
        if end > start {
            let candidate = &unfenced[start..=end];
            if let Ok(parsed) = serde_json::from_str::<Vec<ExtractedInsight>>(candidate) {
                return Ok(parsed);
            }
        }
    }

    // 4. Object wrapper: { "insights": [...], "data": [...], … }.
    if let Ok(value) = serde_json::from_str::<Value>(unfenced) {
        if let Some(obj) = value.as_object() {
            for key in ["insights", "data", "nodes", "result", "results", "decisions"] {
                if let Some(arr) = obj.get(key).and_then(|v| v.as_array()) {
                    if let Ok(parsed) =
                        serde_json::from_value::<Vec<ExtractedInsight>>(Value::Array(arr.clone()))
                    {
                        return Ok(parsed);
                    }
                }
            }
        }
    }

    Err("no JSON array of insights found in response".into())
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
