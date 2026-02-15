use crate::analyzer::claude::ClaudeClient;
use crate::analyzer::gemini::GeminiClient;
use crate::team::data_analyst;
use crate::team::types::{
    BearCase, BullCase, CaseStrength, DataPack, DeskReport, DevilsVerdict, MarketCandidate,
    ResearchDossier, TradePlan,
};
use anyhow::Result;
use rust_decimal::Decimal;
use rust_decimal::prelude::*;
use serde::{Deserialize, Serialize};
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
    _claude: &ClaudeClient,
    _use_sonnet: bool,
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

    // Force Gemini-only (Sonnet disabled for cost optimization)
    let (text, cost) = gemini.call(DEVILS_SYSTEM, &user_msg, 500).await?;

    info!(
        "Judge[Gemini]: {} (${:.4})",
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

// ═══════════════════════════════════════════════════════════════════════════
// CLAUDE FINAL VALIDATOR — Hakim Akhir dengan Threshold 60%
// ═══════════════════════════════════════════════════════════════════════════

#[derive(Debug, Serialize, Deserialize)]
pub struct ClaudeFinalVerdict {
    pub approved: bool,
    pub win_probability: f64,
    pub confidence: f64,
    pub reasoning: String,
    pub risk_level: String, // "LOW", "MEDIUM", "HIGH", "GAMBLING"
}

const CLAUDE_FINAL_SYSTEM: &str = r#"Anda adalah HAKIM AKHIR untuk sistem trading prediction market.

PERAN ANDA: Melindungi modal dengan menolak trade yang berisiko atau gambling.

ATURAN KETAT:
1. HANYA setujui jika probabilitas menang ≥ 60%
2. TOLAK jika terdeteksi perilaku gambling (tebakan acak, tidak ada edge jelas)
3. TOLAK jika confidence rendah atau reasoning lemah
4. Evaluasi: kualitas data, logika, magnitude edge

Output HANYA JSON:
{
  "approved": true/false,
  "win_probability": 0.0-1.0,
  "confidence": 0.0-1.0,
  "reasoning": "penjelasan singkat dalam bahasa Indonesia",
  "risk_level": "LOW|MEDIUM|HIGH|GAMBLING"
}

Bersikap KONSERVATIF. Jika ragu, TOLAK.
Jangan pakai markdown code blocks, langsung JSON."#;

/// Claude Final Validator: Validasi akhir sebelum execute trade
/// Threshold: 60% win rate minimum
pub async fn claude_final_validator(
    claude: &ClaudeClient,
    plan: &TradePlan,
) -> Result<ClaudeFinalVerdict> {
    let user_msg = format!(
        r#"Evaluasi Trade Plan Ini:

Market: {}
Arah: {}
Harga Saat Ini: {}
Fair Value: {}
Edge: {:.2}%
Confidence: {:.2}

Mode Trading: {}
Ukuran Posisi: ${}
Take Profit: {:?}%
Stop Loss: {:?}%

Ringkasan Analisis:
- Specialist: {}
- Bull Probability: {:?}%
- Bear Probability: {:?}%
- Reasoning: {}

TUGAS ANDA: Validasi trade ini. Apakah layak dieksekusi?
INGAT: Hanya setujui jika win rate ≥ 60% DAN ada edge jelas.

Berikan penilaian dalam JSON format."#,
        plan.market.question,
        plan.direction,
        plan.entry_price,
        plan.fair_value_yes,
        plan.edge * Decimal::from(100),
        plan.confidence,
        plan.mode,
        plan.bet_size,
        plan.take_profit_pct.to_f64().map(|v| v * 100.0),
        plan.stop_loss_pct.to_f64().map(|v| v * 100.0),
        plan.specialist_desk.as_deref().unwrap_or("GENERAL"),
        plan.bull_probability.map(|v| v * 100.0),
        plan.bear_probability.map(|v| v * 100.0),
        plan.reasoning,
    );

    let (response, cost) = claude.call(CLAUDE_FINAL_SYSTEM, &user_msg, 600).await?;

    // Parse JSON response
    let json_str = extract_json(&response);
    let verdict: ClaudeFinalVerdict = serde_json::from_str(&json_str)
        .map_err(|e| {
            anyhow::anyhow!(
                "Failed to parse Claude final verdict: {} | Response: {}",
                e,
                &json_str[..json_str.len().min(300)]
            )
        })?;

    // Log decision
    if verdict.approved {
        info!(
            "✅ CLAUDE APPROVED: {:.0}% win | {} risk | ${:.4} cost | {}",
            verdict.win_probability * 100.0,
            verdict.risk_level,
            cost,
            &verdict.reasoning[..verdict.reasoning.len().min(60)]
        );
    } else {
        warn!(
            "❌ CLAUDE REJECTED: {} | {} | ${:.4} cost",
            verdict.risk_level,
            verdict.reasoning,
            cost
        );
    }

    Ok(verdict)
}
