use std::fmt;

#[derive(Debug)]
pub struct ArbitrageOpportunity {
    pub ts: i64,
    pub staleness_ms: i64,
    pub buy_venue: String,
    pub sell_venue: String,
    pub buy_price: f64,
    pub sell_price: f64,
    pub spread: f64,
    pub total_fee: f64,
    pub trade_size: f64,     // max executable size in base (e.g. SOL)
}

impl fmt::Display for ArbitrageOpportunity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "[Arb] ts={} staleness={}ms | Buy {} @ {:.4}, Sell {} @ {:.4}, Spread {:.4}, Fees {:.4}, Size {:.4}",
            self.ts,
            self.staleness_ms,
            self.buy_venue,
            self.buy_price,
            self.sell_venue,
            self.sell_price,
            self.spread,
            self.total_fee,
            self.trade_size,
        )
    }
}