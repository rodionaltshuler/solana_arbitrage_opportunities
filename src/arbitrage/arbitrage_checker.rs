use crate::datasource::quote::{Exchange, QuoteUpdate};

pub struct ArbitrageChecker {
    pub min_spread: f64, // threshold in quote currency
}

impl ArbitrageChecker {
    pub fn new(min_spread: f64) -> Self {
        Self { min_spread }
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

        if diff_buy_first_sell_second > self.min_spread {
            println!(
                "[Arb] ts={} staleness={}ms | Buy {} @ {:.4}, Sell {} @ {:.4}, Spread {:.4}",
                opp_ts,
                staleness,
                first_exchange.name,
                first.best_quote.ask_price,
                second_exchange.name,
                second.best_quote.bid_price,
                diff_buy_first_sell_second
            );
        }

        if diff_buy_second_sell_first > self.min_spread {
            println!(
                "[Arb] ts={} staleness={}ms | Buy {} @ {:.4}, Sell {} @ {:.4}, Spread {:.4}",
                opp_ts,
                staleness,
                second_exchange.name,
                second.best_quote.ask_price,
                first_exchange.name,
                first.best_quote.bid_price,
                diff_buy_second_sell_first
            );
        }
    }
}
