use crate::analyzer::gemini::GeminiClient;
use crate::team::types::AuditInsight;
use crate::types::{Trade, TradeStatus};
use anyhow::Result;
use serde::Deserialize;
use std::collections::HashMap;
use tracing::info;

const AUDIT_SYSTEM: &str = r#"You are the AUDITOR on a prediction market trading team. Analyze closed trades to find patterns and improve future performance.

Output ONLY a JSON object:
{"avg_calibration_error": 0.XX, "insights": ["insight1", "insight2", "insight3"], "bull_accuracy": 0.XX, "bear_accuracy": 0.XX, "desk_accuracy": {"CRYPTO": 0.XX, "WEATHER": 0.XX, "SPORTS": 0.XX, "GENERAL": 0.XX}}

RULES:
- avg_calibration_error = average difference between predicted fair value and actual outcome
- insights = 2-4 actionable lessons from this batch of trades
- bull_accuracy = fraction of trades where Bull direction was correct (0.0 to 1.0)
- bear_accuracy = fraction of trades where Bear direction was correct (0.0 to 1.0)
- desk_accuracy = per-desk win rates (estimate from trade categories; omit desks with no trades)
- Focus on patterns: which categories/desks perform best, what edge sizes are reliable, etc.
- Be specific and actionable."#;

/// Agent 10: Auditor — Post-trade learning (Gemini AI, periodic)
/// Analyzes closed trades to find patterns and generate insights.
pub async fn audit(
    gemini: &GeminiClient,
    closed_trades: &[Trade],
) -> Result<AuditInsight> {
    if closed_trades.is_empty() {
        return Ok(AuditInsight {
            timestamp: chrono::Utc::now().to_rfc3339(),
            trade_count: 0,
            win_rate: 0.0,
            avg_calibration_error: 0.0,
            insights: vec!["No trades to audit.".to_string()],
            bull_accuracy: 0.0,
            bear_accuracy: 0.0,
            desk_accuracy: HashMap::new(),
        });
    }

    let win_count = closed_trades.iter().filter(|t| t.status == TradeStatus::Won).count();
    let total = closed_trades.len();
    let win_rate = win_count as f64 / total as f64;

    // Build trade summary for AI analysis
    let mut trade_summaries = Vec::new();
    for (i, trade) in closed_trades.iter().enumerate().take(20) {
        let result = match trade.status {
            TradeStatus::Won => "WON",
            TradeStatus::Lost => "LOST",
            _ => "OPEN",
        };
        trade_summaries.push(format!(
            "#{}: {} {} | entry={} fair={} edge={} | PnL=${} | {}",
            i + 1,
            trade.direction,
            &trade.question[..trade.question.len().min(50)],
            trade.entry_price,
            trade.fair_value,
            trade.edge,
            trade.pnl,
            result,
        ));
    }

    let user_msg = format!(
        "Audit these {} closed trades (win rate: {:.0}%):\n\n{}\n\n\
        Find patterns: what worked, what failed, what should we adjust?",
        total,
        win_rate * 100.0,
        trade_summaries.join("\n"),
    );

    let (text, cost) = gemini.call(AUDIT_SYSTEM, &user_msg, 500).await?;
    info!("Auditor: analyzed {} trades (${:.4})", total, cost);

    let insight = parse_audit(&text, total, win_rate)?;
    Ok(insight)
}

/// Save audit insights to knowledge.json for injection into future prompts
pub fn save_insights(insights: &AuditInsight) -> Result<()> {
    if insights.insights.is_empty() {
        return Ok(());
    }

    #[derive(serde::Serialize, Deserialize)]
    struct Knowledge {
        #[serde(default)]
        insights: Vec<String>,
    }

    // Read existing knowledge
    let mut knowledge = match std::fs::read_to_string("knowledge.json") {
        Ok(content) => serde_json::from_str::<Knowledge>(&content).unwrap_or(Knowledge {
            insights: Vec::new(),
        }),
        Err(_) => Knowledge {
            insights: Vec::new(),
        },
    };

    // Append new insights (keep last 20)
    for insight in &insights.insights {
        if !knowledge.insights.contains(insight) {
            knowledge.insights.push(insight.clone());
        }
    }
    if knowledge.insights.len() > 20 {
        knowledge.insights = knowledge.insights.split_off(knowledge.insights.len() - 20);
    }

    std::fs::write("knowledge.json", serde_json::to_string_pretty(&knowledge)?)?;
    info!("Auditor: saved {} insights to knowledge.json", knowledge.insights.len());

    Ok(())
}

fn parse_audit(text: &str, trade_count: usize, win_rate: f64) -> Result<AuditInsight> {
    let json_str = extract_json(text);

    #[derive(Deserialize)]
    struct Resp {
        avg_calibration_error: f64,
        #[serde(default)]
        insights: Vec<String>,
        #[serde(default)]
        bull_accuracy: f64,
        #[serde(default)]
        bear_accuracy: f64,
        #[serde(default)]
        desk_accuracy: HashMap<String, f64>,
    }

    let parsed: Resp = serde_json::from_str(&json_str)
        .map_err(|e| anyhow::anyhow!("Audit JSON parse: {e}"))?;

    Ok(AuditInsight {
        timestamp: chrono::Utc::now().to_rfc3339(),
        trade_count,
        win_rate,
        avg_calibration_error: parsed.avg_calibration_error,
        insights: parsed.insights,
        bull_accuracy: parsed.bull_accuracy,
        bear_accuracy: parsed.bear_accuracy,
        desk_accuracy: parsed.desk_accuracy,
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
