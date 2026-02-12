use crate::analyzer::gemini::GeminiClient;
use crate::team::types::{MarketCandidate, ResearchDossier};
use anyhow::Result;
use rust_decimal::Decimal;
use serde::Deserialize;
use tracing::{info, warn};

const RESEARCH_SYSTEM: &str = r#"You are a prediction market researcher. For each market, provide factual research to help estimate the true probability.

Output ONLY a JSON object with these exact keys:
{"news_relevance": "summary of relevant recent news", "fact_check": "key facts that affect the outcome", "base_rate": 0.XX, "counter_arguments": "main arguments against the current market price", "key_factors": ["factor1", "factor2", "factor3"]}

RULES:
- base_rate = your estimate of the historical base rate for similar events (0.0 to 1.0)
- news_relevance = 1-2 sentences about the most relevant recent news
- fact_check = concrete verifiable facts (prices, dates, historical data)
- counter_arguments = what could prove the market wrong
- key_factors = 2-4 most important factors for this market's resolution
- Be specific and factual. Cite numbers when possible.
- Do NOT wrap in markdown code blocks."#;

/// Agent 2: Researcher — Gemini AI research per candidate
/// Gathers news relevance, fact checks, base rates, and counter-arguments.
pub async fn research(
    gemini: &GeminiClient,
    candidates: &[MarketCandidate],
) -> Vec<(String, Result<ResearchDossier>)> {
    let mut results = Vec::with_capacity(candidates.len());

    for candidate in candidates {
        let market = &candidate.market;
        let user_msg = format!(
            "Research this prediction market:\n\
            Question: {}\n\
            Description: {}\n\
            Category: {}\n\
            End Date: {}\n\
            Current YES price: {} ({}% implied)\n\
            Volume: ${}\n\
            \n\
            Provide factual research to help estimate the true probability.",
            market.question,
            &market.description[..market.description.len().min(400)],
            market.category,
            market.end_date,
            market.yes_price,
            (market.yes_price * Decimal::from(100)).round(),
            market.volume.round(),
        );

        match gemini.call(RESEARCH_SYSTEM, &user_msg, 400).await {
            Ok((text, cost)) => {
                info!(
                    "Researcher: {} (${:.4})",
                    &market.question[..market.question.len().min(40)],
                    cost
                );
                match parse_research(&text, &market.id) {
                    Ok(dossier) => results.push((market.id.clone(), Ok(dossier))),
                    Err(e) => {
                        warn!("Research parse failed for {}: {e}", &market.id[..8.min(market.id.len())]);
                        results.push((market.id.clone(), Err(e)));
                    }
                }
            }
            Err(e) => {
                warn!("Research call failed: {e}");
                results.push((market.id.clone(), Err(e)));
            }
        }
    }

    results
}

fn parse_research(text: &str, market_id: &str) -> Result<ResearchDossier> {
    let json_str = extract_json(text);

    #[derive(Deserialize)]
    struct Resp {
        news_relevance: String,
        fact_check: String,
        base_rate: f64,
        counter_arguments: String,
        #[serde(default)]
        key_factors: Vec<String>,
    }

    let parsed: Resp = serde_json::from_str(&json_str)
        .map_err(|e| anyhow::anyhow!("Research JSON parse: {e} | text: {}", &json_str[..json_str.len().min(200)]))?;

    Ok(ResearchDossier {
        market_id: market_id.to_string(),
        news_relevance: parsed.news_relevance,
        fact_check: parsed.fact_check,
        base_rate: parsed.base_rate.clamp(0.01, 0.99),
        counter_arguments: parsed.counter_arguments,
        key_factors: parsed.key_factors,
    })
}

fn extract_json(text: &str) -> String {
    // Find JSON object
    if let Some(start) = text.find('{') {
        let mut depth = 0;
        for (i, ch) in text[start..].char_indices() {
            match ch {
                '{' => depth += 1,
                '}' => {
                    depth -= 1;
                    if depth == 0 {
                        return text[start..=start + i].to_string();
                    }
                }
                _ => {}
            }
        }
    }
    // Try code block
    if let Some(s) = text.find("```json") {
        let after = &text[s + 7..];
        if let Some(e) = after.find("```") {
            return after[..e].trim().to_string();
        }
    }
    text.to_string()
}
