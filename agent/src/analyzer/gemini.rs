use anyhow::{Context, Result};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::str::FromStr;
use tracing::debug;

/// Gemini Flash 2.0 API client (Paid Tier 1: $0.10/1M input, $0.40/1M output)
pub struct GeminiClient {
    api_key: String,
    client: reqwest::Client,
}

#[derive(Serialize)]
struct GeminiRequest {
    system_instruction: GeminiContent,
    contents: Vec<GeminiMessage>,
    #[serde(rename = "generationConfig")]
    generation_config: GenerationConfig,
}

#[derive(Serialize)]
struct GeminiContent {
    parts: Vec<GeminiPart>,
}

#[derive(Serialize)]
struct GeminiMessage {
    role: String,
    parts: Vec<GeminiPart>,
}

#[derive(Serialize, Deserialize)]
struct GeminiPart {
    text: String,
}

#[derive(Serialize)]
struct GenerationConfig {
    temperature: f32,
    #[serde(rename = "maxOutputTokens")]
    max_output_tokens: u32,
}

#[derive(Deserialize)]
struct GeminiResponse {
    candidates: Option<Vec<Candidate>>,
    #[serde(rename = "usageMetadata")]
    usage_metadata: Option<UsageMetadata>,
}

#[derive(Deserialize)]
struct Candidate {
    content: Option<CandidateContent>,
}

#[derive(Deserialize)]
struct CandidateContent {
    parts: Option<Vec<GeminiPart>>,
}

#[derive(Deserialize)]
struct UsageMetadata {
    #[serde(rename = "promptTokenCount", default)]
    prompt_token_count: u32,
    #[serde(rename = "candidatesTokenCount", default)]
    candidates_token_count: u32,
}

impl GeminiClient {
    pub fn new(api_key: &str) -> Self {
        Self {
            api_key: api_key.to_string(),
            client: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(60))
                .build()
                .expect("HTTP client"),
        }
    }

    #[allow(dead_code)]
    pub fn is_configured(&self) -> bool {
        !self.api_key.is_empty()
    }

    /// Call Gemini Flash 2.0 API
    /// Returns (response_text, cost) — Paid Tier 1 pricing
    pub async fn call(
        &self,
        system: &str,
        user_msg: &str,
        max_tokens: u32,
    ) -> Result<(String, Decimal)> {
        let url = format!(
            "https://generativelanguage.googleapis.com/v1beta/models/gemini-2.0-flash:generateContent?key={}",
            self.api_key
        );

        let req = GeminiRequest {
            system_instruction: GeminiContent {
                parts: vec![GeminiPart { text: system.to_string() }],
            },
            contents: vec![GeminiMessage {
                role: "user".to_string(),
                parts: vec![GeminiPart { text: user_msg.to_string() }],
            }],
            generation_config: GenerationConfig {
                temperature: 0.3,
                max_output_tokens: max_tokens,
            },
        };

        let resp = self
            .client
            .post(&url)
            .header("Content-Type", "application/json")
            .json(&req)
            .send()
            .await
            .context("Gemini API request")?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            anyhow::bail!("Gemini API {status}: {}", &body[..body.len().min(300)]);
        }

        let data: GeminiResponse = resp.json().await.context("Parse Gemini response")?;

        let text = data
            .candidates
            .and_then(|c| c.into_iter().next())
            .and_then(|c| c.content)
            .and_then(|c| c.parts)
            .map(|parts| parts.into_iter().map(|p| p.text).collect::<Vec<_>>().join(""))
            .unwrap_or_default();

        if text.is_empty() {
            anyhow::bail!("Gemini returned empty response");
        }

        let usage = data.usage_metadata.unwrap_or(UsageMetadata {
            prompt_token_count: 0,
            candidates_token_count: 0,
        });

        // Gemini Flash 2.0 Paid Tier 1: $0.10/1M input, $0.40/1M output
        let input_cost = Decimal::from(usage.prompt_token_count) * Decimal::from_str("0.0000001").unwrap();
        let output_cost = Decimal::from(usage.candidates_token_count) * Decimal::from_str("0.0000004").unwrap();
        let cost = input_cost + output_cost;

        debug!(
            "Gemini: {} tokens in, {} tokens out, ${cost}",
            usage.prompt_token_count, usage.candidates_token_count
        );

        Ok((text, cost))
    }
}
