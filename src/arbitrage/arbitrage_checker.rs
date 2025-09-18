use crate::arbitrage::arbitrage_opportunity::ArbitrageOpportunity;
use crate::datasource::domain::{Venue, QuoteUpdate};

pub struct ArbitrageChecker {
    pub min_spread: f64,     // minimum spread in quote currency
    pub fee_first: f64,      // fee rate (fraction) for first exchange
    pub fee_second: f64,     // fee rate (fraction) for second exchange
}

impl ArbitrageChecker {
    pub fn new(min_spread: f64, fee_first: f64, fee_second: f64) -> Self {
        Self {
            min_spread,
            fee_first,
            fee_second,
        }
    }

    pub fn check(
        &self,
        first_venue: &Venue,
        first: &QuoteUpdate,
        second_venue: &Venue,
        second: &QuoteUpdate,
    ) -> Vec<ArbitrageOpportunity> {
        let mut opportunities = Vec::new();

        let opp_ts = first.ts.min(second.ts);
        let now_ms = chrono::Utc::now().timestamp_millis();
        let staleness = now_ms - opp_ts;
        let total_fee = self.fee_first + self.fee_second;

        // --- Case 1: Buy first, Sell second ---
        let spread1 = second.best_quote.bid_price - first.best_quote.ask_price;
        if spread1 > self.min_spread {
            let trade_size = first.best_quote.ask_size.min(second.best_quote.bid_size);

            opportunities.push(ArbitrageOpportunity {
                ts: opp_ts,
                staleness_ms: staleness,
                buy_venue: first_venue.name.clone(),
                sell_venue: second_venue.name.clone(),
                buy_price: first.best_quote.ask_price,
                sell_price: second.best_quote.bid_price,
                spread: spread1,
                total_fee: total_fee * first.best_quote.ask_price,
                trade_size,
            });
        }

        // --- Case 2: Buy second, Sell first ---
        let spread2 = first.best_quote.bid_price - second.best_quote.ask_price;
        if spread2 > self.min_spread {
            let trade_size = second.best_quote.ask_size.min(first.best_quote.bid_size);

            opportunities.push(ArbitrageOpportunity {
                ts: opp_ts,
                staleness_ms: staleness,
                buy_venue: second_venue.name.clone(),
                sell_venue: first_venue.name.clone(),
                buy_price: second.best_quote.ask_price,
                sell_price: first.best_quote.bid_price,
                spread: spread2,
                total_fee: total_fee * second.best_quote.ask_price,
                trade_size,
            });
        }
        opportunities
    }
}
