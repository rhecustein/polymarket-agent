use crate::data::Enricher;
use crate::live::ClobClient;
use crate::team::types::{DataPack, MarketCandidate};
use tracing::{debug, warn};

/// Agent 3: Data Analyst — Quantitative data collection (no AI)
/// Wraps Enricher (CoinGecko, news) + ClobClient (order book).
pub async fn analyze(
    enricher: &Enricher,
    clob: &ClobClient,
    candidates: &[MarketCandidate],
) -> Vec<DataPack> {
    let mut packs = Vec::with_capacity(candidates.len());

    for candidate in candidates {
        let market = &candidate.market;

        // Fetch enrichment data (crypto prices, news)
        let enrichment = enricher.enrich(market).await;

        // Extract price trend from crypto signals
        let price_trend_24h = enrichment
            .crypto_signals
            .as_ref()
            .map(|s| s.price_24h_change_pct);
        let volume_trend = enrichment
            .crypto_signals
            .as_ref()
            .map(|s| s.volume_24h_change_pct);

        // Fetch order book data if tokens available
        let (spread, bid_depth, ask_depth) = if !market.tokens.is_empty() {
            match clob.get_order_book(&market.tokens[0].token_id).await {
                Ok(book) => {
                    debug!(
                        "CLOB book for {}: spread={} bid_depth={} ask_depth={}",
                        &market.question[..market.question.len().min(30)],
                        book.spread,
                        book.bid_depth,
                        book.ask_depth,
                    );
                    (Some(book.spread), Some(book.bid_depth), Some(book.ask_depth))
                }
                Err(e) => {
                    warn!("Order book fetch failed: {e}");
                    (None, None, None)
                }
            }
        } else {
            (None, None, None)
        };

        packs.push(DataPack {
            market_id: market.id.clone(),
            enrichment,
            price_trend_24h,
            volume_trend,
            order_book_spread: spread,
            order_book_bid_depth: bid_depth,
            order_book_ask_depth: ask_depth,
        });
    }

    packs
}

/// Format DataPack into a text summary for AI prompts
pub fn format_data_pack(pack: &DataPack) -> String {
    let mut parts = Vec::new();

    if let Some(ref crypto) = pack.enrichment.crypto_signals {
        let mut s = format!(
            "CRYPTO: {} | ${:.2} | 24h: {:.2}% | 7d: {:.2}%",
            crypto.asset, crypto.current_price, crypto.price_24h_change_pct, crypto.price_7d_change_pct,
        );
        if let Some(rsi) = crypto.rsi_14 {
            let signal = if rsi > 70.0 { " (OVERBOUGHT)" }
                else if rsi < 30.0 { " (OVERSOLD)" }
                else { "" };
            s.push_str(&format!(" | RSI-14: {rsi:.1}{signal}"));
        }
        if let Some(idx) = crypto.fear_greed_index {
            let signal = if idx <= 20 { " (EXTREME FEAR)" }
                else if idx <= 35 { " (FEAR)" }
                else if idx >= 80 { " (EXTREME GREED)" }
                else if idx >= 65 { " (GREED)" }
                else { "" };
            s.push_str(&format!(" | Fear/Greed: {idx}/100{signal}"));
        }
        if let Some(dom) = crypto.btc_dominance {
            s.push_str(&format!(" | BTC Dom: {dom:.1}%"));
        }
        parts.push(s);
    }

    if let Some(spread) = pack.order_book_spread {
        let mut s = format!("ORDER BOOK: spread={}", spread);
        if let Some(bid) = pack.order_book_bid_depth {
            s.push_str(&format!(" | bid_depth=${}", bid));
        }
        if let Some(ask) = pack.order_book_ask_depth {
            s.push_str(&format!(" | ask_depth=${}", ask));
        }
        parts.push(s);
    }

    if !pack.enrichment.news_headlines.is_empty() {
        parts.push(format!(
            "NEWS:\n  {}",
            pack.enrichment
                .news_headlines
                .iter()
                .enumerate()
                .map(|(i, h)| format!("{}. {}", i + 1, h))
                .collect::<Vec<_>>()
                .join("\n  ")
        ));
    }

    if parts.is_empty() {
        "No external data available.".to_string()
    } else {
        parts.join("\n")
    }
}
