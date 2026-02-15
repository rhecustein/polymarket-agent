pub mod auditor;
pub mod bear_analyst;
pub mod bull_analyst;
pub mod crypto_desk;
pub mod data_analyst;
pub mod judge;
pub mod executor;
pub mod general_desk;
pub mod researcher;
pub mod risk_manager;
pub mod scout;
pub mod sports_desk;
pub mod strategist;
pub mod types;
pub mod weather_desk;

use crate::telegram::TelegramAlert;
use crate::analyzer::claude::ClaudeClient;
use crate::analyzer::gemini::GeminiClient;
use crate::config::Config;
use crate::paper::{Portfolio, SimConfig};
use crate::data::Enricher;
use crate::live::ClobClient;
use crate::data::polymarket::GammaScanner;
use crate::db::StateStore;
use crate::types::Direction;
use futures::future::join_all;
use rust_decimal::Decimal;
use std::str::FromStr;
use tracing::{error, info, warn};
use types::{detect_desk, DeskType, TeamCycleStats};

/// Run one full v2.0 team cycle with parallel analysis
/// 14-Agent Company: Scout -> Data Analyst + Researcher -> Specialist Desk -> Bull/Bear -> Judge -> Risk -> Strategist -> Execute
pub async fn run_cycle(
    config: &Config,
    gemini: &GeminiClient,
    claude: &ClaudeClient,
    enricher: &Enricher,
    scanner: &GammaScanner,
    clob: &ClobClient,
    portfolio: &Portfolio,
    store: &StateStore,
    telegram: &TelegramAlert,
    effective_max_pct: Decimal,
    max_candidates: usize,
    max_deep_analysis: usize,
) -> TeamCycleStats {
    let mut stats = TeamCycleStats::default();
    let sim = SimConfig::from_config(config);

    // ═══════════════════════════════════════════
    // PHASE 1: INTELLIGENCE (parallel)
    // ═══════════════════════════════════════════
    // ═══════════════════════════════════════════
    // PHASE 1: INTELLIGENCE (parallel)
    // ═══════════════════════════════════════════
    info!("═══ PHASE 1: SCOUT + RESEARCH ═══");
    store.update_status("scanning", "Scanning markets for opportunities...").ok();

    // Scout: scan + filter + score (all categories)
    let scout_report = match scout::scan(scanner, config, max_candidates).await {
        Ok(report) => report,
        Err(e) => {
            error!("Scout failed: {e}");
            return stats;
        }
    };
    stats.markets_scanned = scout_report.total_scanned;
    stats.markets_passed_quality = scout_report.total_passed_quality;

    if scout_report.candidates.is_empty() {
        warn!("Scout: no candidates found (scanned={}, passed_quality={}) — check min_edge/min_confidence thresholds",
            scout_report.total_scanned, scout_report.total_passed_quality);
        return stats;
    }

    info!(
        "Scout: {} candidates from {} scanned ({} passed quality)",
        scout_report.candidates.len(),
        scout_report.total_scanned,
        scout_report.total_passed_quality,
    );

    // Filter out recently analyzed markets
    let candidates: Vec<_> = scout_report
        .candidates
        .into_iter()
        .filter(|c| !store.was_recently_analyzed(&c.market.id, 4))
        .collect();

    if candidates.is_empty() {
        info!("All candidates recently analyzed, skipping");
        return stats;
    }

    // Data Analyst + Researcher (parallel)
    let (data_packs, research_results) = tokio::join!(
        data_analyst::analyze(enricher, clob, &candidates),
        researcher::research(gemini, &candidates),
    );

    stats.markets_researched = research_results.len();

    // Estimate Researcher API cost: ~$0.00015 per market researched
    let research_cost = Decimal::from_str("0.00015").unwrap() * Decimal::from(research_results.len());
    stats.api_cost += research_cost;

    info!(
        "Phase 1 complete: {} data packs, {} research dossiers (API cost: ${:.4})",
        data_packs.len(),
        research_results.len(),
        research_cost
    );

    // ═══════════════════════════════════════════
    // PHASE 2-4: PARALLEL PER CANDIDATE
    // Specialist Desk -> Bull/Bear -> Judge -> Risk -> Strategist -> Execute
    // ═══════════════════════════════════════════
    // ═══════════════════════════════════════════
    // PHASE 2-4: PARALLEL PER CANDIDATE
    // Specialist Desk -> Bull/Bear -> Judge -> Risk -> Strategist -> Execute
    // ═══════════════════════════════════════════
    info!("═══ PHASE 2-4: PARALLEL ANALYSIS ({} candidates) ═══", candidates.len().min(max_deep_analysis));
    store.update_status("analyzing", &format!("Analyzing {} candidates...", candidates.len())).ok();

    let analysis_limit = max_deep_analysis.min(candidates.len());

    // Build futures for parallel candidate analysis
    let futures: Vec<_> = candidates
        .iter()
        .take(analysis_limit)
        .enumerate()
        .map(|(i, candidate)| {
            let market_id = candidate.market.id.clone();
            let data_pack = data_packs.iter().find(|p| p.market_id == market_id).cloned();
            let dossier = research_results
                .iter()
                .find(|(id, _)| *id == market_id)
                .and_then(|(_, r)| r.as_ref().ok())
                .cloned();

            let sim_clone = sim.clone();
            analyze_candidate(
                i,
                analysis_limit,
                candidate,
                data_pack,
                dossier,
                gemini,
                claude,
                portfolio,
                config,
                store,
                telegram,
                effective_max_pct,
                sim_clone,
            )
        })
        .collect();

    let results = join_all(futures).await;

    // Aggregate stats
    for result in results {
        stats.markets_analyzed += result.analyzed;
        stats.markets_approved += result.approved;
        stats.trades_placed += result.traded;
        stats.api_cost += result.api_cost;
    }

    store.update_status("idle", "Cycle complete. Waiting for next run...").ok();

    info!(
        "═══ CYCLE COMPLETE: scanned={} researched={} analyzed={} approved={} traded={} (API cost: ${:.4}) ═══",
        stats.markets_scanned,
        stats.markets_researched,
        stats.markets_analyzed,
        stats.markets_approved,
        stats.trades_placed,
        stats.api_cost,
    );

    stats
}

/// Result from analyzing a single candidate
struct CandidateResult {
    analyzed: usize,
    approved: usize,
    traded: usize,
    api_cost: Decimal,
}

/// Analyze a single candidate through the full pipeline:
/// Specialist Desk -> Bull + Bear -> Judge -> Risk -> Strategist -> Execute
async fn analyze_candidate(
    index: usize,
    total: usize,
    candidate: &types::MarketCandidate,
    data_pack: Option<types::DataPack>,
    dossier: Option<types::ResearchDossier>,
    gemini: &GeminiClient,
    claude: &ClaudeClient,
    portfolio: &Portfolio,
    config: &Config,
    store: &StateStore,
    telegram: &TelegramAlert,
    effective_max_pct: Decimal,
    sim: SimConfig,
) -> CandidateResult {
    let mut result = CandidateResult {
        analyzed: 0,
        approved: 0,
        traded: 0,
        api_cost: Decimal::ZERO,
    };

    let market_id = &candidate.market.id;

    let data_pack = match data_pack {
        Some(p) => p,
        None => {
            warn!("No data pack for {}", market_id);
            return result;
        }
    };

    let dossier = match dossier {
        Some(d) => d,
        None => {
            warn!("No research dossier for {}", market_id);
            return result;
        }
    };

    info!(
        "── Market {}/{}: {} ──",
        index + 1,
        total,
        &candidate.market.question[..candidate.market.question.len().min(50)]
    );

    // ── Specialist Desk ──
    let desk_type = detect_desk(&candidate.market.question, &candidate.market.category);
    let desk_report = match desk_type {
        DeskType::Crypto => crypto_desk::analyze(gemini, candidate, &data_pack, &dossier).await,
        DeskType::Weather => weather_desk::analyze(gemini, candidate, &data_pack, &dossier).await,
        DeskType::Sports => sports_desk::analyze(gemini, candidate, &data_pack, &dossier).await,
        DeskType::General => general_desk::analyze(gemini, candidate, &data_pack, &dossier).await,
    };

    let desk_report = match desk_report {
        Ok(r) => {
            // Estimate Gemini API cost: ~500 input + ~300 output tokens
            // Tier 1: $0.10/1M input, $0.40/1M output
            let est_cost = Decimal::from_str("0.00017").unwrap(); // ~$0.00017 per desk call
            result.api_cost += est_cost;

            info!(
                "  Desk[{}]: prob={:.0}% conf={:.0}%",
                r.desk,
                r.specialist_probability * 100.0,
                r.confidence_in_data * 100.0,
            );
            r
        }
        Err(e) => {
            warn!("Desk analysis failed: {e}");
            return result;
        }
    };

    // ── Bull + Bear (parallel) ──
    let (bull_result, bear_result) = tokio::join!(
        bull_analyst::analyze(gemini, candidate, &data_pack, &dossier, &desk_report),
        bear_analyst::analyze(gemini, candidate, &data_pack, &dossier, &desk_report),
    );

    let bull = match bull_result {
        Ok(b) => {
            // Estimate cost for Bull analyst
            result.api_cost += Decimal::from_str("0.00015").unwrap();
            info!(
                "  Bull: {:.0}% YES ({})",
                b.probability_yes * 100.0,
                b.case_strength
            );
            b
        }
        Err(e) => {
            warn!("Bull analysis failed: {e}");
            return result;
        }
    };

    let bear = match bear_result {
        Ok(b) => {
            // Estimate cost for Bear analyst
            result.api_cost += Decimal::from_str("0.00015").unwrap();
            info!(
                "  Bear: {:.0}% NO ({})",
                b.probability_no * 100.0,
                b.case_strength
            );
            b
        }
        Err(e) => {
            warn!("Bear analysis failed: {e}");
            return result;
        }
    };

    // ── Judge ──
    // Force Gemini-only (cost optimization)
    let verdict = match judge::judge(
        gemini, claude, false, candidate, &bull, &bear, &data_pack, &dossier, &desk_report,
    )
    .await
    {
        Ok(v) => {
            // Estimate cost: Gemini ~$0.10/1M in + $0.40/1M out
            let judge_cost = Decimal::from_str("0.0002").unwrap(); // Gemini
            result.api_cost += judge_cost;

            info!(
                "  Judge[Gemini]: fair={:.2} conf={:.2} -> {}",
                v.fair_value_yes,
                v.confidence,
                v.direction
            );
            v
        }
        Err(e) => {
            warn!("Judge failed: {e}");
            return result;
        }
    };

    result.analyzed = 1;

    if verdict.direction_enum() == Direction::Skip {
        info!("  -> SKIP: {}", verdict.reasoning);
        return result;
    }

    // ── Risk Manager ──
    let risk = risk_manager::check(&verdict, portfolio, config, effective_max_pct, candidate.market.yes_price);
    if !risk.approved {
        info!("  -> REJECTED by Risk Manager: {}", risk.reason);
        return result;
    }

    // ── Strategist ──
    let mut plan = strategist::plan(&verdict, &risk, &candidate.market);
    result.approved = 1;

    // Enrich TradePlan with agent trail for paper trading
    plan.specialist_desk = Some(format!("{}", desk_type));
    plan.bull_probability = Some(bull.probability_yes);
    plan.bear_probability = Some(bear.probability_no);
    plan.judge_model = Some("gemini".to_string());

    // Edge vs SL filter
    if plan.stop_loss_pct > Decimal::ZERO && plan.edge < plan.stop_loss_pct {
        info!(
            "  -> SKIP: edge {:.1}% < SL {:.1}% (unfavorable risk/reward)",
            plan.edge * Decimal::from(100),
            plan.stop_loss_pct * Decimal::from(100),
        );
        return result;
    }

    // ══ CLAUDE FINAL VALIDATOR (Hakim Akhir - Threshold 60%) ══
    if claude.is_configured() {
        info!("  ⚖️  Validasi Hakim Akhir (Claude Sonnet)...");
        match judge::claude_final_validator(claude, &plan).await {
            Ok(claude_verdict) => {
                // Tambahkan API cost Claude (Sonnet: $3/1M in + $15/1M out)
                // Estimasi: ~500 input + ~400 output tokens = ~$0.007
                result.api_cost += Decimal::from_str("0.007").unwrap();

                if !claude_verdict.approved {
                    warn!(
                        "  ❌ CLAUDE REJECTED: {} | Win: {:.0}% | {}",
                        claude_verdict.risk_level,
                        claude_verdict.win_probability * 100.0,
                        claude_verdict.reasoning
                    );
                    return result;
                }

                info!(
                    "  ✅ CLAUDE APPROVED: Win {:.0}% | Conf {:.0}% | {}",
                    claude_verdict.win_probability * 100.0,
                    claude_verdict.confidence * 100.0,
                    claude_verdict.risk_level
                );
            }
            Err(e) => {
                error!("  ⚠️  Claude validation failed: {e} — proceeding without validation");
            }
        }
    } else {
        warn!("  ⚠️  Claude Final Validator DISABLED (no API key)");
    }

    // ── Executor ──
    store.update_status("trading", &format!("Executing {} trade...", verdict.direction)).ok();
    if let Some(_trade) = executor::execute(&plan, portfolio, store, telegram, &sim).await {
        result.traded = 1;
    }

    result
}
