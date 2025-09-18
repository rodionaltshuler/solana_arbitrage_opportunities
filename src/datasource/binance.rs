use anyhow::Result;
use async_trait::async_trait;
use futures::{StreamExt, stream::BoxStream};
use serde::Deserialize;
use tokio_tungstenite::connect_async;

use crate::datasource::datasource::DataSource;
use crate::datasource::domain::{BestQuote, Venue, Instrument, QuoteUpdate};

pub(crate) const BINANCE_FEE: f64 = 0.00013500;

/// Binance L2 orderbook update (depth5)
#[derive(Deserialize, Debug)]
struct BinanceDepth {
    #[serde(rename = "lastUpdateId")]
    pub last_update_id: u64,
    pub bids: Vec<[String; 2]>,
    pub asks: Vec<[String; 2]>,
}

pub struct BinanceSource {
    pub venue: Venue,
}

impl BinanceSource {
    pub fn new(venue_name: &str) -> Self {
        Self {
            venue: Venue { name: venue_name.to_string() },
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
        // Binance wants lowercase symbols
        let symbol = format!("{}{}", instrument.base.to_lowercase(), instrument.quote.to_lowercase());
        let url = format!("wss://stream.binance.com:9443/ws/{}@depth5@100ms", symbol);

        let (ws_stream, _) = connect_async(url).await?;
        let (_write, read) = ws_stream.split();

        let venue = self.venue.clone();
        let instrument_owned = instrument.clone();

        let stream = read.filter_map(move |msg| {
            let venue = venue.clone();
            let instrument = instrument_owned.clone();
            async move {
                let msg = msg.ok()?;
                let txt = msg.into_text().ok()?;

                //println!("BINANCE MSG: {}", txt);
                if !txt.trim_start().starts_with('{') {
                    return None;
                }
                
                let parsed: BinanceDepth = match serde_json::from_str(&txt) {
                    Ok(p) => p,
                    Err(e) => {
                        eprintln!("binance parse error: {e} | raw: {txt}");
                        return None;
                    }
                };

                let best_bid = parsed.bids.get(0)?;
                let bid_price: f64 = best_bid[0].parse().ok()?;
                let bid_size: f64 = best_bid[1].parse().ok()?; // in base (SOL)

                let best_ask = parsed.asks.get(0)?;
                let ask_price: f64 = best_ask[0].parse().ok()?;
                let ask_size: f64 = best_ask[1].parse().ok()?; // in base (SOL)

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
                    fee_rate: BINANCE_FEE,
                })
            }
        });

        Ok(Box::pin(stream))
    }
}
