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
use crate::paper::Portfolio;
use crate::data::Enricher;
use crate::live::ClobClient;
use crate::data::polymarket::GammaScanner;
use crate::db::StateStore;
use crate::types::Direction;
use futures::future::join_all;
use rust_decimal::Decimal;
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

    // ═══════════════════════════════════════════
    // PHASE 1: INTELLIGENCE (parallel)
    // ═══════════════════════════════════════════
    info!("═══ PHASE 1: SCOUT + RESEARCH ═══");

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
        info!("Scout: no candidates found");
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

    info!(
        "Phase 1 complete: {} data packs, {} research dossiers",
        data_packs.len(),
        research_results.len(),
    );

    // ═══════════════════════════════════════════
    // PHASE 2-4: PARALLEL PER CANDIDATE
    // Specialist Desk -> Bull/Bear -> Judge -> Risk -> Strategist -> Execute
    // ═══════════════════════════════════════════
    info!("═══ PHASE 2-4: PARALLEL ANALYSIS ({} candidates) ═══", candidates.len().min(max_deep_analysis));

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
            )
        })
        .collect();

    let results = join_all(futures).await;

    // Aggregate stats
    for result in results {
        stats.markets_analyzed += result.analyzed;
        stats.markets_approved += result.approved;
        stats.trades_placed += result.traded;
    }

    info!(
        "═══ CYCLE COMPLETE: scanned={} researched={} analyzed={} approved={} traded={} ═══",
        stats.markets_scanned,
        stats.markets_researched,
        stats.markets_analyzed,
        stats.markets_approved,
        stats.trades_placed,
    );

    stats
}

/// Result from analyzing a single candidate
struct CandidateResult {
    analyzed: usize,
    approved: usize,
    traded: usize,
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
) -> CandidateResult {
    let mut result = CandidateResult {
        analyzed: 0,
        approved: 0,
        traded: 0,
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
    // Top 3 candidates (by index) get Sonnet, rest get Gemini
    let use_sonnet = index < 3;
    let verdict = match judge::judge(
        gemini, claude, use_sonnet, candidate, &bull, &bear, &data_pack, &dossier, &desk_report,
    )
    .await
    {
        Ok(v) => {
            info!(
                "  Judge{}: fair={:.2} conf={:.2} -> {}",
                if use_sonnet { "[Sonnet]" } else { "[Gemini]" },
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
    let risk = risk_manager::check(&verdict, portfolio, config, effective_max_pct);
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
    plan.judge_model = Some(if use_sonnet { "sonnet".to_string() } else { "gemini".to_string() });

    // Edge vs SL filter
    if plan.stop_loss_pct > Decimal::ZERO && plan.edge < plan.stop_loss_pct {
        info!(
            "  -> SKIP: edge {:.1}% < SL {:.1}% (unfavorable risk/reward)",
            plan.edge * Decimal::from(100),
            plan.stop_loss_pct * Decimal::from(100),
        );
        return result;
    }

    // ── Executor ──
    if let Some(_trade) = executor::execute(&plan, portfolio, store, telegram).await {
        result.traded = 1;
    }

    result
}
