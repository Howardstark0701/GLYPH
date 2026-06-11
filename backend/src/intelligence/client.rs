use crate::errors::AppError;
use serde::{Deserialize, Serialize};
use serde_json::Value;

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

/// Call NIM with a fully-built prompt string, parse and return insights.
pub async fn analyze_events_with_prompt(
    client:  &reqwest::Client,
    api_key: &str,
    prompt:  &str,
) -> Result<Vec<ExtractedInsight>, AppError> {
    let base_url = std::env::var("NIM_BASE_URL")
        .unwrap_or_else(|_| "https://integrate.api.nvidia.com/v1".to_string());

    let request = NimRequest {
        model: "nvidia/llama-3.1-nemotron-70b-instruct".into(),
        messages: vec![
            Message {
                role:    "system".into(),
                content: crate::intelligence::prompts::SYSTEM_PROMPT.into(),
            },
            Message {
                role:    "user".into(),
                content: prompt.into(),
            },
        ],
        temperature: 0.1,
        max_tokens:  4096,
    };

    let resp = client
        .post(format!("{}/chat/completions", base_url))
        .header("Authorization", format!("Bearer {}", api_key))
        .header("Content-Type", "application/json")
        .json(&request)
        .send()
        .await?;

    if !resp.status().is_success() {
        let status = resp.status();
        let body   = resp.text().await.unwrap_or_default();
        return Err(AppError::NimApiError(format!(
            "NIM API returned {}: {}",
            status, body
        )));
    }

    let nim_resp: NimResponse = resp.json().await?;

    let content = nim_resp
        .choices
        .first()
        .map(|c| c.message.content.as_str())
        .ok_or_else(|| AppError::NimApiError("Empty NIM response".into()))?;

    // NIM sometimes wraps JSON in markdown fences — strip them
    let clean = strip_code_fences(content);

    let insights: Vec<ExtractedInsight> = serde_json::from_str(clean)
        .map_err(|e| AppError::NimApiError(format!(
            "Failed to parse NIM JSON response: {}. Raw: {}",
            e, &clean[..clean.len().min(200)]
        )))?;

    Ok(insights)
}

/// Legacy helper — builds payload inline; prefer analyze_events_with_prompt.
pub async fn analyze_events(
    client:      &reqwest::Client,
    api_key:     &str,
    events_json: Value,
) -> Result<Vec<ExtractedInsight>, AppError> {
    let prompt = crate::intelligence::prompts::build_analysis_prompt(
        &serde_json::to_string_pretty(&events_json).unwrap_or_default()
    );
    analyze_events_with_prompt(client, api_key, &prompt).await
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
