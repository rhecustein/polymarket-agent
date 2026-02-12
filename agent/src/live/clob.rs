use anyhow::{Context, Result};
use rust_decimal::Decimal;
use serde::Deserialize;
use std::str::FromStr;
use tracing::{debug, warn};

/// CLOB (Central Limit Order Book) client for Polymarket
pub struct ClobClient {
    base_url: String,
    client: reqwest::Client,
}

#[derive(Debug, Clone)]
pub struct OrderBookSummary {
    pub best_bid: Decimal,
    pub best_ask: Decimal,
    pub spread: Decimal,
    pub bid_depth: Decimal,
    pub ask_depth: Decimal,
}

#[derive(Debug, Deserialize)]
struct ClobOrderBook {
    bids: Option<Vec<ClobLevel>>,
    asks: Option<Vec<ClobLevel>>,
}

#[derive(Debug, Deserialize)]
struct ClobLevel {
    price: String,
    size: String,
}

#[derive(Debug, Deserialize)]
struct ClobPriceResponse {
    price: Option<String>,
}

impl ClobClient {
    pub fn new(base_url: &str) -> Self {
        Self {
            base_url: base_url.to_string(),
            client: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(15))
                .build()
                .expect("HTTP client"),
        }
    }

    /// Fetch order book for a token
    pub async fn get_order_book(&self, token_id: &str) -> Result<OrderBookSummary> {
        let url = format!("{}/book?token_id={}", self.base_url, token_id);

        let resp: ClobOrderBook = self
            .client
            .get(&url)
            .header("Accept", "application/json")
            .send()
            .await
            .context("CLOB order book request")?
            .json()
            .await
            .context("Parse CLOB order book")?;

        let bids = resp.bids.unwrap_or_default();
        let asks = resp.asks.unwrap_or_default();

        let best_bid = bids
            .first()
            .and_then(|l| Decimal::from_str(&l.price).ok())
            .unwrap_or(Decimal::ZERO);

        let best_ask = asks
            .first()
            .and_then(|l| Decimal::from_str(&l.price).ok())
            .unwrap_or(Decimal::ONE);

        let bid_depth: Decimal = bids
            .iter()
            .filter_map(|l| Decimal::from_str(&l.size).ok())
            .sum();

        let ask_depth: Decimal = asks
            .iter()
            .filter_map(|l| Decimal::from_str(&l.size).ok())
            .sum();

        let spread = best_ask - best_bid;

        debug!(
            "CLOB book: bid={best_bid} ask={best_ask} spread={spread} depth=({bid_depth}/{ask_depth})"
        );

        Ok(OrderBookSummary {
            best_bid,
            best_ask,
            spread,
            bid_depth,
            ask_depth,
        })
    }

    /// Get mid-price for a token
    pub async fn get_price(&self, token_id: &str) -> Result<Decimal> {
        let url = format!("{}/price?token_id={}", self.base_url, token_id);

        let resp = self
            .client
            .get(&url)
            .header("Accept", "application/json")
            .send()
            .await;

        match resp {
            Ok(r) if r.status().is_success() => {
                let data: ClobPriceResponse = r.json().await.context("Parse CLOB price")?;
                data.price
                    .and_then(|p| Decimal::from_str(&p).ok())
                    .ok_or_else(|| anyhow::anyhow!("No price in CLOB response"))
            }
            Ok(r) => {
                warn!("CLOB price request returned {}", r.status());
                anyhow::bail!("CLOB price request failed")
            }
            Err(e) => {
                warn!("CLOB price request error: {e}");
                anyhow::bail!("CLOB price request failed: {e}")
            }
        }
    }

    /// Place a limit order (returns order ID)
    pub async fn place_order(
        &self,
        token_id: &str,
        side: &str,
        price: Decimal,
        size: Decimal,
        _api_key: &str,
    ) -> Result<String> {
        let url = format!("{}/order", self.base_url);

        let body = serde_json::json!({
            "tokenID": token_id,
            "side": side,
            "price": price.to_string(),
            "size": size.to_string(),
            "type": "GTC",
        });

        let resp = self
            .client
            .post(&url)
            .header("Accept", "application/json")
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await
            .context("CLOB order placement")?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            anyhow::bail!("CLOB order failed {status}: {}", &body[..body.len().min(200)]);
        }

        #[derive(Deserialize)]
        struct OrderResp {
            #[serde(rename = "orderID")]
            order_id: Option<String>,
        }

        let data: OrderResp = resp.json().await.context("Parse CLOB order response")?;
        data.order_id
            .ok_or_else(|| anyhow::anyhow!("No order ID in CLOB response"))
    }

    /// Check order status
    pub async fn get_order_status(&self, order_id: &str) -> Result<String> {
        let url = format!("{}/order/{}", self.base_url, order_id);

        let resp = self
            .client
            .get(&url)
            .header("Accept", "application/json")
            .send()
            .await
            .context("CLOB order status")?;

        if !resp.status().is_success() {
            anyhow::bail!("CLOB order status failed: {}", resp.status());
        }

        #[derive(Deserialize)]
        struct StatusResp {
            status: Option<String>,
        }

        let data: StatusResp = resp.json().await.context("Parse order status")?;
        Ok(data.status.unwrap_or_else(|| "unknown".into()))
    }

    /// Cancel an order
    pub async fn cancel_order(&self, order_id: &str) -> Result<()> {
        let url = format!("{}/order/{}", self.base_url, order_id);

        let resp = self
            .client
            .delete(&url)
            .header("Accept", "application/json")
            .send()
            .await
            .context("CLOB cancel order")?;

        if !resp.status().is_success() {
            warn!("CLOB cancel failed for {order_id}: {}", resp.status());
        }

        Ok(())
    }
}
