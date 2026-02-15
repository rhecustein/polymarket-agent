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

const DEVILS_SYSTEM: &str = r#"You are the JUDGE — aggressive profit-seeking arbiter on a prediction market trading team. You receive arguments from both a Bull (YES) analyst and a Bear (NO) analyst, plus raw data and a specialist desk report.

Your job: find profitable edges and approve trades that can make money.

Output ONLY a JSON object:
{"fair_value_yes": 0.XX, "confidence": 0.XX, "direction": "YES"|"NO"|"SKIP", "reasoning": "2-3 sentences", "bull_flaws": "flaws in YES case", "bear_flaws": "flaws in NO case"}

AGGRESSIVE RULES:
1. PROFIT FOCUS: Accept trades with positive expected value and reasonable edge (3%+).
2. EVIDENCE MATTERS: Prefer evidence-backed arguments, but speculation with edge is acceptable.
3. BASE RATE: Use as starting point, but adjust quickly based on current data.
4. SPECIALIST WEIGHT: Desk specialist important but not decisive — trust market opportunities.
5. CALIBRATION: Fair value can deviate 25% from market price for strong opportunities.
   * Markets can be inefficient — don't be afraid to take contrarian positions.
6. CONFIDENCE: Use 0.45-0.55 for uncertain trades, 0.60+ when you see clear edge.
7. SKIP RULES (output direction="SKIP" only when):
   * Both cases completely contradictory with zero edge
   * Your fair_value is within 2% of market price → edge too small
   * Confidence is below 0.40 → too uncertain
8. DIRECTION must be consistent with fair_value:
   * If fair_value_yes > market_yes_price + 0.03 → "YES"
   * If fair_value_yes < market_yes_price - 0.03 → "NO"
   * Otherwise → "SKIP"
9. Maximum edge can be 30%+. Large edges are opportunities, not errors.

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

    // AGGRESSIVE MODE: Removed pre-checks for weak cases - let judge decide
    // Even weak cases can have profitable edges

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

    // AGGRESSIVE MODE: Wider calibration bounds (±30%)
    let market_yes = market.yes_price.to_f64().unwrap_or(0.5);
    let max_fair = (market_yes + 0.30).min(0.98);
    let min_fair = (market_yes - 0.30).max(0.02);
    if verdict.fair_value_yes > max_fair || verdict.fair_value_yes < min_fair {
        warn!(
            "Judge calibration clamp: {:.2} -> [{:.2}, {:.2}] (market={})",
            verdict.fair_value_yes, min_fair, max_fair, market_yes
        );
        verdict.fair_value_yes = verdict.fair_value_yes.clamp(min_fair, max_fair);
    }

    // Enforce direction consistency (min edge 0.03 for direction)
    let edge = verdict.fair_value_yes - market_yes;
    let dir = verdict.direction.to_uppercase();
    if dir == "YES" && edge < 0.03 {
        verdict.direction = "SKIP".to_string();
    } else if dir == "NO" && edge > -0.03 {
        verdict.direction = "SKIP".to_string();
    }

    // Allow larger edges (0.35 max — big edges are opportunities)
    if edge.abs() > 0.35 {
        warn!("Judge edge very large: {:.2} — clamping to 0.35", edge);
        verdict.fair_value_yes = if edge > 0.0 {
            market_yes + 0.35
        } else {
            market_yes - 0.35
        };
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

const CLAUDE_FINAL_SYSTEM: &str = r#"Anda adalah VALIDATOR AGRESIF untuk sistem trading prediction market.

PERAN ANDA: Maksimalkan profit dengan mengambil peluang yang menguntungkan.

ATURAN BARU (AGRESIF):
1. SETUJUI jika probabilitas menang ≥ 42% DAN ada edge positif
2. TERIMA trade dengan risiko medium-high jika potensi profit jelas
3. GAMBLING DIPERBOLEHKAN jika expected value positif
4. Fokus: cepat profit, scaling dari modal kecil ke besar

Output HANYA JSON:
{
  "approved": true/false,
  "win_probability": 0.0-1.0,
  "confidence": 0.0-1.0,
  "reasoning": "penjelasan singkat dalam bahasa Indonesia",
  "risk_level": "LOW|MEDIUM|HIGH|GAMBLING"
}

Bersikap AGRESIF. Jika ada edge, SETUJUI.
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
