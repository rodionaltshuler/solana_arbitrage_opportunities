use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Venue {
    pub name: String
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Instrument {
    pub base: String,
    pub quote: String
}

impl Instrument {
    pub fn new(base: &str, quote: &str) -> Self {
        Self { base: base.to_string(), quote: quote.to_string() }
    }
    pub fn symbol(&self) -> String {
        format!("{}-{}", self.base, self.quote)
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct BestQuote {
    pub bid_price: f64,
    pub bid_size: f64,
    pub ask_price: f64,
    pub ask_size: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuoteUpdate {
    pub ts: i64,
    pub venue: Venue,
    pub instrument: Instrument,
    pub best_quote: BestQuote,
    pub fee_rate: f64
}