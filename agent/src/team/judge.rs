use crate::analyzer::claude::ClaudeClient;
use crate::analyzer::gemini::GeminiClient;
use crate::team::data_analyst;
use crate::team::types::{
    BearCase, BullCase, CaseStrength, DataPack, DeskReport, DevilsVerdict, MarketCandidate,
    ResearchDossier,
};
use anyhow::Result;
use rust_decimal::Decimal;
use rust_decimal::prelude::*;
use serde::Deserialize;
use tracing::{info, warn};

const DEVILS_SYSTEM: &str = r#"You are the JUDGE — the impartial arbiter on a prediction market trading team. You receive arguments from both a Bull (YES) analyst and a Bear (NO) analyst, plus raw data and a specialist desk report.

Your job: find flaws in BOTH arguments, then render an impartial final verdict.

Output ONLY a JSON object:
{"fair_value_yes": 0.XX, "confidence": 0.XX, "direction": "YES"|"NO"|"SKIP", "reasoning": "2-3 sentences", "bull_flaws": "flaws in YES case", "bear_flaws": "flaws in NO case"}

JUDGMENT RULES:
1. SKEPTICISM: Assume both analysts are biased. Find the weakest link in each argument.
2. EVIDENCE > SPECULATION: Only count arguments backed by concrete data. Speculation = 0 weight.
3. BASE RATE ANCHOR: Start from the historical base rate, then adjust based on evidence strength.
4. SPECIALIST WEIGHT: The desk specialist report carries significant weight — they have domain expertise.
5. CALIBRATION: Your fair_value_yes must be within 20% of the current market price. Markets are usually efficient.
   * If market says 60% YES, your fair value should be between 40% and 80%.
   * Deviations beyond this range are almost always calibration errors.
6. CONFIDENCE: Use 0.60-0.68 for normal markets. Only use 0.72+ when evidence is STRONG on one side.
7. SKIP RULES (output direction="SKIP" when):
   * Both cases are WEAK strength → not enough evidence to trade
   * Bull and Bear probabilities are within 5% of each other → too close to call
   * Your fair_value is within 7% of market price → no meaningful edge
   * Confidence is below 0.55 → too uncertain
8. DIRECTION must be consistent with fair_value:
   * If fair_value_yes > market_yes_price + 0.08 → "YES"
   * If fair_value_yes < market_yes_price - 0.08 → "NO"
   * Otherwise → "SKIP"
9. Maximum edge should be 15-20%. Edges >20% are almost always errors.

Do NOT wrap in markdown code blocks."#;

/// Agent 10: Judge — Judges both sides, renders final verdict
/// Uses Claude Sonnet for top candidates (use_sonnet=true), Gemini for rest
pub async fn judge(
    gemini: &GeminiClient,
    claude: &ClaudeClient,
    use_sonnet: bool,
    candidate: &MarketCandidate,
    bull: &BullCase,
    bear: &BearCase,
    data_pack: &DataPack,
    dossier: &ResearchDossier,
    desk_report: &DeskReport,
) -> Result<DevilsVerdict> {
    let market = &candidate.market;

    // Pre-check: if both cases are WEAK, skip without AI call
    if bull.strength() == CaseStrength::Weak && bear.strength() == CaseStrength::Weak {
        info!(
            "Judge: SKIP {} (both cases WEAK)",
            &market.question[..market.question.len().min(40)]
        );
        return Ok(DevilsVerdict {
            market_id: market.id.clone(),
            fair_value_yes: market.yes_price.to_f64().unwrap_or(0.5),
            confidence: 0.0,
            direction: "SKIP".to_string(),
            reasoning: "Both Bull and Bear cases are WEAK — insufficient evidence to trade."
                .to_string(),
            bull_flaws: "Case too weak to evaluate".to_string(),
            bear_flaws: "Case too weak to evaluate".to_string(),
        });
    }

    // Pre-check: if probabilities within 5%, skip
    let prob_diff = (bull.probability_yes - (1.0 - bear.probability_no)).abs();
    if prob_diff < 0.05 {
        info!(
            "Judge: SKIP {} (bull/bear within 5%: {:.2} vs {:.2})",
            &market.question[..market.question.len().min(40)],
            bull.probability_yes,
            1.0 - bear.probability_no,
        );
        return Ok(DevilsVerdict {
            market_id: market.id.clone(),
            fair_value_yes: market.yes_price.to_f64().unwrap_or(0.5),
            confidence: 0.0,
            direction: "SKIP".to_string(),
            reasoning: format!(
                "Bull ({:.0}% YES) and Bear ({:.0}% NO) are too close — no clear edge.",
                bull.probability_yes * 100.0,
                bear.probability_no * 100.0,
            ),
            bull_flaws: "N/A".to_string(),
            bear_flaws: "N/A".to_string(),
        });
    }

    let data_text = data_analyst::format_data_pack(data_pack);

    let user_msg = format!(
        "JUDGE THIS MARKET:\n\
        Question: {question}\n\
        Current YES price: {yes} ({yes_pct}% implied)\n\
        End Date: {end}\n\
        Base Rate: {base_rate:.0}%\n\
        \n\
        === SPECIALIST DESK ({desk}) ===\n\
        Probability: {desk_prob:.0}%\n\
        Key factors: {desk_factors}\n\
        Risk: {desk_risk}\n\
        Data confidence: {desk_conf:.0}%\n\
        \n\
        === BULL CASE (YES) ===\n\
        Probability YES: {bull_prob:.0}%\n\
        Case Strength: {bull_strength}\n\
        Arguments: {bull_args}\n\
        Evidence: {bull_evidence}\n\
        Reasoning: {bull_reason}\n\
        \n\
        === BEAR CASE (NO) ===\n\
        Probability NO: {bear_prob:.0}%\n\
        Case Strength: {bear_strength}\n\
        Arguments: {bear_args}\n\
        Evidence: {bear_evidence}\n\
        Reasoning: {bear_reason}\n\
        \n\
        === RAW DATA ===\n\
        {data}\n\
        \n\
        Find flaws in both arguments and render your final verdict.",
        question = market.question,
        yes = market.yes_price,
        yes_pct = (market.yes_price * Decimal::from(100)).round(),
        end = market.end_date,
        base_rate = dossier.base_rate * 100.0,
        desk = desk_report.desk,
        desk_prob = desk_report.specialist_probability * 100.0,
        desk_factors = desk_report.key_factors.join(", "),
        desk_risk = desk_report.risk_assessment,
        desk_conf = desk_report.confidence_in_data * 100.0,
        bull_prob = bull.probability_yes * 100.0,
        bull_strength = bull.case_strength,
        bull_args = bull.arguments.join("; "),
        bull_evidence = bull.evidence.join("; "),
        bull_reason = bull.reasoning,
        bear_prob = bear.probability_no * 100.0,
        bear_strength = bear.case_strength,
        bear_args = bear.arguments.join("; "),
        bear_evidence = bear.evidence.join("; "),
        bear_reason = bear.reasoning,
        data = data_text,
    );

    // Route to Sonnet (top 3) or Gemini (rest)
    let (text, cost, model_label) = if use_sonnet && claude.is_configured() {
        let (t, c) = claude.call(DEVILS_SYSTEM, &user_msg, 500).await?;
        (t, c, "sonnet")
    } else {
        let (t, c) = gemini.call(DEVILS_SYSTEM, &user_msg, 500).await?;
        (t, c, "gemini")
    };

    info!(
        "Judge[{}]: {} (${:.4})",
        model_label,
        &market.question[..market.question.len().min(40)],
        cost
    );

    let mut verdict = parse_verdict(&text, &market.id)?;

    // Post-processing: enforce calibration bounds (±20%)
    let market_yes = market.yes_price.to_f64().unwrap_or(0.5);
    let max_fair = (market_yes + 0.20).min(0.98);
    let min_fair = (market_yes - 0.20).max(0.02);
    if verdict.fair_value_yes > max_fair || verdict.fair_value_yes < min_fair {
        warn!(
            "Judge calibration clamp: {:.2} -> [{:.2}, {:.2}] (market={})",
            verdict.fair_value_yes, min_fair, max_fair, market_yes
        );
        verdict.fair_value_yes = verdict.fair_value_yes.clamp(min_fair, max_fair);
    }

    // Enforce direction consistency (min edge 0.08 for direction)
    let edge = verdict.fair_value_yes - market_yes;
    let dir = verdict.direction.to_uppercase();
    if dir == "YES" && edge < 0.08 {
        verdict.direction = "SKIP".to_string();
    } else if dir == "NO" && edge > -0.08 {
        verdict.direction = "SKIP".to_string();
    }

    // Enforce max edge (0.20 — edges beyond this are almost always errors)
    if edge.abs() > 0.20 {
        warn!("Judge edge too large: {:.2} — forcing SKIP", edge);
        verdict.direction = "SKIP".to_string();
    }

    Ok(verdict)
}

fn parse_verdict(text: &str, market_id: &str) -> Result<DevilsVerdict> {
    let json_str = extract_json(text);

    #[derive(Deserialize)]
    struct Resp {
        fair_value_yes: f64,
        confidence: f64,
        direction: String,
        reasoning: String,
        #[serde(default)]
        bull_flaws: String,
        #[serde(default)]
        bear_flaws: String,
    }

    let parsed: Resp = serde_json::from_str(&json_str)
        .map_err(|e| anyhow::anyhow!("Judge JSON parse: {e} | {}", &json_str[..json_str.len().min(200)]))?;

    Ok(DevilsVerdict {
        market_id: market_id.to_string(),
        fair_value_yes: parsed.fair_value_yes.clamp(0.02, 0.98),
        confidence: parsed.confidence.clamp(0.0, 1.0),
        direction: parsed.direction,
        reasoning: parsed.reasoning,
        bull_flaws: parsed.bull_flaws,
        bear_flaws: parsed.bear_flaws,
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
