use crate::analyzer::gemini::GeminiClient;
use crate::team::data_analyst;
use crate::team::types::{BullCase, DataPack, DeskReport, MarketCandidate, ResearchDossier};
use anyhow::Result;
use rust_decimal::Decimal;
use serde::Deserialize;
use tracing::info;

const BULL_SYSTEM: &str = r#"You are the BULL ANALYST on a prediction market trading team. Your job is to build the STRONGEST possible case for YES.

You must argue for YES regardless of your personal belief. Be like a lawyer making the best case for their client.

Output ONLY a JSON object:
{"probability_yes": 0.XX, "case_strength": "WEAK|MODERATE|STRONG|OVERWHELMING", "arguments": ["arg1", "arg2", "arg3"], "evidence": ["evidence1", "evidence2"], "reasoning": "2 sentences max"}

RULES:
- probability_yes = the highest defensible YES probability you can argue (0.05 to 0.95)
- case_strength = honestly rate how strong your own case is:
  * WEAK: only circumstantial evidence, mostly speculation
  * MODERATE: some concrete evidence but gaps exist
  * STRONG: solid evidence and logical chain
  * OVERWHELMING: near-certain based on data
- arguments: 2-4 strongest arguments for YES
- evidence: cite SPECIFIC data points (prices, dates, numbers) from the data provided
- Be honest about case_strength — inflating it helps nobody
- Do NOT wrap in markdown code blocks."#;

/// Agent 8: Bull Analyst — Builds the strongest YES case (Gemini AI)
pub async fn analyze(
    gemini: &GeminiClient,
    candidate: &MarketCandidate,
    data_pack: &DataPack,
    dossier: &ResearchDossier,
    desk_report: &DeskReport,
) -> Result<BullCase> {
    let market = &candidate.market;
    let data_text = data_analyst::format_data_pack(data_pack);

    let desk_section = format!(
        "SPECIALIST DESK ({desk}) SAYS:\n\
        - Probability: {prob:.0}%\n\
        - Key factors: {factors}\n\
        - Risk: {risk}\n\
        - Data confidence: {conf:.0}%",
        desk = desk_report.desk,
        prob = desk_report.specialist_probability * 100.0,
        factors = desk_report.key_factors.join(", "),
        risk = desk_report.risk_assessment,
        conf = desk_report.confidence_in_data * 100.0,
    );

    let user_msg = format!(
        "Build the strongest YES case for this market:\n\
        \n\
        MARKET: {question}\n\
        Description: {desc}\n\
        Current YES price: {yes} ({yes_pct}% implied)\n\
        End Date: {end}\n\
        Volume: ${vol}\n\
        \n\
        {desk}\n\
        \n\
        QUANTITATIVE DATA:\n\
        {data}\n\
        \n\
        RESEARCH:\n\
        - News: {news}\n\
        - Facts: {facts}\n\
        - Base rate: {base_rate:.0}%\n\
        - Key factors: {factors}\n\
        \n\
        Build the STRONGEST possible case that YES will happen.",
        question = market.question,
        desc = &market.description[..market.description.len().min(300)],
        yes = market.yes_price,
        yes_pct = (market.yes_price * Decimal::from(100)).round(),
        end = market.end_date,
        vol = market.volume.round(),
        desk = desk_section,
        data = data_text,
        news = dossier.news_relevance,
        facts = dossier.fact_check,
        base_rate = dossier.base_rate * 100.0,
        factors = dossier.key_factors.join(", "),
    );

    let (text, cost) = gemini.call(BULL_SYSTEM, &user_msg, 400).await?;

    info!(
        "Bull: {} (${:.4})",
        &market.question[..market.question.len().min(40)],
        cost
    );

    parse_bull_case(&text, &market.id)
}

fn parse_bull_case(text: &str, market_id: &str) -> Result<BullCase> {
    let json_str = extract_json(text);

    #[derive(Deserialize)]
    struct Resp {
        probability_yes: f64,
        case_strength: String,
        #[serde(default)]
        arguments: Vec<String>,
        #[serde(default)]
        evidence: Vec<String>,
        reasoning: String,
    }

    let parsed: Resp = serde_json::from_str(&json_str)
        .map_err(|e| anyhow::anyhow!("Bull JSON parse: {e} | {}", &json_str[..json_str.len().min(200)]))?;

    Ok(BullCase {
        market_id: market_id.to_string(),
        probability_yes: parsed.probability_yes.clamp(0.05, 0.95),
        case_strength: parsed.case_strength,
        arguments: parsed.arguments,
        evidence: parsed.evidence,
        reasoning: parsed.reasoning,
    })
}

fn extract_json(text: &str) -> String {
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
    if let Some(s) = text.find("```json") {
        let after = &text[s + 7..];
        if let Some(e) = after.find("```") {
            return after[..e].trim().to_string();
        }
    }
    text.to_string()
}
