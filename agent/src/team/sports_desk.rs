use crate::analyzer::gemini::GeminiClient;
use crate::team::data_analyst;
use crate::team::types::{DataPack, DeskReport, DeskType, MarketCandidate, ResearchDossier};
use anyhow::Result;
use rust_decimal::Decimal;
use serde::Deserialize;
use tracing::info;

const SPORTS_DESK_SYSTEM: &str = r#"You are a SPORTS SPECIALIST on a prediction market trading team. You have deep expertise in sports analytics, team performance, and statistical modeling.

Output ONLY a JSON object:
{"specialist_probability": 0.XX, "key_factors": ["factor1", "factor2", "factor3"], "risk_assessment": "brief risk summary", "data_summary": "what the data tells us", "confidence_in_data": 0.XX}

SPORTS EXPERTISE:
- Team records, win/loss streaks, and head-to-head matchups are strong predictors
- Injuries to key players can shift probabilities 10-20%
- Home/away advantage varies by sport (NBA ~5%, NFL ~3%, soccer ~10%)
- Recent form (last 5-10 games) often beats season averages
- Playoff/tournament dynamics differ from regular season
- Championship markets: look at betting odds, Elo ratings, power rankings
- MVP/awards: check statistical leaders, voting patterns, narrative momentum
- confidence_in_data: how much relevant sports data/stats is available (0.0-1.0)
- Do NOT wrap in markdown code blocks."#;

/// Agent 6: Sports Desk — Sports analytics specialist (Gemini AI)
pub async fn analyze(
    gemini: &GeminiClient,
    candidate: &MarketCandidate,
    data_pack: &DataPack,
    dossier: &ResearchDossier,
) -> Result<DeskReport> {
    let market = &candidate.market;
    let data_text = data_analyst::format_data_pack(data_pack);

    let user_msg = format!(
        "SPORTS ANALYSIS REQUEST:\n\
        Question: {question}\n\
        Description: {desc}\n\
        Current YES price: {yes} ({yes_pct}% implied)\n\
        End Date: {end}\n\
        Volume: ${vol}\n\
        \n\
        AVAILABLE DATA:\n\
        {data}\n\
        \n\
        RESEARCH:\n\
        - News: {news}\n\
        - Facts: {facts}\n\
        - Base rate: {base_rate:.0}%\n\
        - Key factors: {factors}\n\
        \n\
        Provide your specialist sports analysis.",
        question = market.question,
        desc = &market.description[..market.description.len().min(300)],
        yes = market.yes_price,
        yes_pct = (market.yes_price * Decimal::from(100)).round(),
        end = market.end_date,
        vol = market.volume.round(),
        data = data_text,
        news = dossier.news_relevance,
        facts = dossier.fact_check,
        base_rate = dossier.base_rate * 100.0,
        factors = dossier.key_factors.join(", "),
    );

    let (text, cost) = gemini.call(SPORTS_DESK_SYSTEM, &user_msg, 400).await?;
    info!(
        "SportsDesk: {} (${:.4})",
        &market.question[..market.question.len().min(40)],
        cost
    );

    parse_desk_report(&text, &market.id, DeskType::Sports)
}

fn parse_desk_report(text: &str, market_id: &str, desk: DeskType) -> Result<DeskReport> {
    let json_str = extract_json(text);

    #[derive(Deserialize)]
    struct Resp {
        specialist_probability: f64,
        #[serde(default)]
        key_factors: Vec<String>,
        #[serde(default)]
        risk_assessment: String,
        #[serde(default)]
        data_summary: String,
        #[serde(default)]
        confidence_in_data: f64,
    }

    let parsed: Resp = serde_json::from_str(&json_str)
        .map_err(|e| anyhow::anyhow!("SportsDesk JSON parse: {e} | {}", &json_str[..json_str.len().min(200)]))?;

    Ok(DeskReport {
        market_id: market_id.to_string(),
        desk,
        specialist_probability: parsed.specialist_probability.clamp(0.05, 0.95),
        key_factors: parsed.key_factors,
        risk_assessment: parsed.risk_assessment,
        data_summary: parsed.data_summary,
        confidence_in_data: parsed.confidence_in_data.clamp(0.0, 1.0),
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
