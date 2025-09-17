use crate::datasource::quote::{Exchange, QuoteUpdate};

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
        first_exchange: &Exchange,
        first: &QuoteUpdate,
        second_exchange: &Exchange,
        second: &QuoteUpdate,
    ) {
        let diff_buy_first_sell_second = second.best_quote.bid_price - first.best_quote.ask_price;
        let diff_buy_second_sell_first = first.best_quote.bid_price - second.best_quote.ask_price;

        let opp_ts = first.ts.min(second.ts);
        let now_ms = chrono::Utc::now().timestamp_millis();
        let staleness = now_ms - opp_ts;

        let total_fee = self.fee_first + self.fee_second;

        if diff_buy_first_sell_second > self.min_spread {
            println!(
                "[Arb] ts={} staleness={}ms | Buy {} @ {:.4}, Sell {} @ {:.4}, Spread {:.4}, Fees {:.4}",
                opp_ts,
                staleness,
                first_exchange.name,
                first.best_quote.ask_price,
                second_exchange.name,
                second.best_quote.bid_price,
                diff_buy_first_sell_second,
                total_fee * first.best_quote.ask_price,
            );
        }

        if diff_buy_second_sell_first > self.min_spread {
            println!(
                "[Arb] ts={} staleness={}ms | Buy {} @ {:.4}, Sell {} @ {:.4}, Spread {:.4}, Fees {:.4}",
                opp_ts,
                staleness,
                second_exchange.name,
                second.best_quote.ask_price,
                first_exchange.name,
                first.best_quote.bid_price,
                diff_buy_second_sell_first,
                total_fee * second.best_quote.ask_price,
            );
        }
    }
}
