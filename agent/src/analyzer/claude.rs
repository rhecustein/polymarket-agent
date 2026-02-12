use anyhow::{Context, Result};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::str::FromStr;
use tracing::debug;

/// Claude Sonnet API client for high-stakes decisions (Judge top 3)
/// Cost: $3/1M input + $15/1M output tokens
pub struct ClaudeClient {
    api_key: String,
    client: reqwest::Client,
}

#[derive(Serialize)]
struct ClaudeRequest {
    model: String,
    max_tokens: u32,
    system: String,
    messages: Vec<Message>,
}

#[derive(Serialize)]
struct Message {
    role: String,
    content: String,
}

#[derive(Deserialize)]
struct ClaudeResponse {
    content: Vec<ContentBlock>,
    usage: Option<Usage>,
}

#[derive(Deserialize)]
struct ContentBlock {
    #[serde(rename = "type")]
    _type: String,
    text: Option<String>,
}

#[derive(Deserialize)]
struct Usage {
    input_tokens: u32,
    output_tokens: u32,
}

impl ClaudeClient {
    pub fn new(api_key: &str) -> Self {
        Self {
            api_key: api_key.to_string(),
            client: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(90))
                .build()
                .expect("HTTP client"),
        }
    }

    pub fn is_configured(&self) -> bool {
        !self.api_key.is_empty()
    }

    /// Call Claude Sonnet API
    /// Returns (response_text, cost) — Sonnet pricing: $3/1M input, $15/1M output
    pub async fn call(
        &self,
        system: &str,
        user_msg: &str,
        max_tokens: u32,
    ) -> Result<(String, Decimal)> {
        let req = ClaudeRequest {
            model: "claude-sonnet-4-5-20250929".to_string(),
            max_tokens,
            system: system.to_string(),
            messages: vec![Message {
                role: "user".to_string(),
                content: user_msg.to_string(),
            }],
        };

        let resp = self
            .client
            .post("https://api.anthropic.com/v1/messages")
            .header("Content-Type", "application/json")
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", "2023-06-01")
            .json(&req)
            .send()
            .await
            .context("Claude API request")?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            anyhow::bail!("Claude API {status}: {}", &body[..body.len().min(300)]);
        }

        let data: ClaudeResponse = resp.json().await.context("Parse Claude response")?;

        let text = data
            .content
            .into_iter()
            .filter_map(|b| b.text)
            .collect::<Vec<_>>()
            .join("");

        if text.is_empty() {
            anyhow::bail!("Claude returned empty response");
        }

        let usage = data.usage.unwrap_or(Usage {
            input_tokens: 0,
            output_tokens: 0,
        });

        // Sonnet: $3/M input, $15/M output
        let input_cost =
            Decimal::from(usage.input_tokens) * Decimal::from_str("0.000003").unwrap();
        let output_cost =
            Decimal::from(usage.output_tokens) * Decimal::from_str("0.000015").unwrap();
        let cost = input_cost + output_cost;

        debug!(
            "Claude Sonnet: {} tokens in, {} tokens out, ${cost}",
            usage.input_tokens, usage.output_tokens
        );

        Ok((text, cost))
    }
}
