use anyhow::Result;
use async_trait::async_trait;
use futures::{StreamExt, stream::BoxStream};
use serde::Deserialize;
use tokio_tungstenite::connect_async;

use crate::datasource::datasource::DataSource;
use crate::datasource::quote::{BestQuote, Venue, Instrument, QuoteUpdate};

pub(crate) const BINANCE_FEE: f64 = 0.00013500;

#[derive(Deserialize)]
struct BinanceBookTicker {
    #[serde(rename = "u")]
    pub update_id: u64,
    #[serde(rename = "s")]
    pub symbol: String,
    #[serde(rename = "b")]
    pub bid_price: String,
    #[serde(rename = "B")]
    pub bid_qty: String,
    #[serde(rename = "a")]
    pub ask_price: String,
    #[serde(rename = "A")]
    pub ask_qty: String,
}

pub struct BinanceSource {
    pub venue: Venue,
}

impl BinanceSource {

    pub fn new() -> Self {
        Self {
            venue: Venue { name: "BINANCE".to_string() },
        }
    }
}

#[async_trait]
impl DataSource for BinanceSource {
    fn venue(&self) -> &Venue {
        &self.venue
    }

    async fn subscribe_best_quotes(
        &self,
        instrument: Instrument,
    ) -> Result<BoxStream<'static, QuoteUpdate>> {
        let symbol = format!("{}{}", instrument.base.to_lowercase(), instrument.quote.to_lowercase());
        let url = format!("wss://stream.binance.com:9443/ws/{}@bookTicker", symbol);

        let (ws_stream, _) = connect_async(url.to_string()).await?;
        let (_write, read) = ws_stream.split();

        let venue = self.venue.clone();
        let instrument_owned = instrument.clone();

        let stream = read.filter_map(move |msg| {
            let venue = venue.clone();
            let instrument = instrument_owned.clone();
            async move {
                let msg = msg.ok()?;
                let txt = msg.into_text().ok()?;
                let parsed: BinanceBookTicker = serde_json::from_str(&txt).ok()?;

                let bid_price: f64 = parsed.bid_price.parse().ok()?;
                let bid_size: f64 = parsed.bid_qty.parse().ok()?;
                let ask_price: f64 = parsed.ask_price.parse().ok()?;
                let ask_size: f64 = parsed.ask_qty.parse().ok()?;

                let ts_ms = chrono::Utc::now().timestamp_millis();

                Some(QuoteUpdate {
                    ts: ts_ms,
                    venue: venue.clone(),
                    instrument: instrument.clone(),
                    best_quote: BestQuote {
                        bid_price,
                        bid_size,
                        ask_price,
                        ask_size,
                    },
                    fee_rate: BINANCE_FEE
                })
            }
        });

        Ok(Box::pin(stream))
    }
}
