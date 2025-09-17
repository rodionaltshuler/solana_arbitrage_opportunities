mod datasource;

use anyhow::Result;
use futures::{StreamExt, stream::select};
use crate::datasource::quote::{Exchange, Instrument, QuoteUpdate};
use crate::datasource::raydium_clmm::RaydiumClmmSource;
use crate::datasource::datasource::DataSource;
use crate::datasource::binance::BinanceSource;


#[tokio::main]
async fn main() -> Result<()> {

    let instrument = Instrument::new("SOL", "USDC");

    let ws_url = "wss://api.mainnet-beta.solana.com";
    let rpc_url = "https://api.mainnet-beta.solana.com";

    let pool = "3ucNos4NbumPLZNWztqGHNFFgkHeRMBQAVemeeomsUxv"; // SOL/USDC CLMM pool state

    let datasource_raydium = RaydiumClmmSource::from_pool(
        rpc_url,
        ws_url,
        pool,
        "RAYDIUM_CLMM",
    ).await?;

    let datasource_binance = BinanceSource::new();

    let binance_stream = datasource_binance
        .subscribe_best_quotes(instrument.clone()).await?;

    let raydium_stream = datasource_raydium
        .subscribe_best_quotes(instrument.clone()).await?;

    let mut combined = select(raydium_stream, binance_stream);

    let mut last_raydium: Option<QuoteUpdate> = None;
    let mut last_binance: Option<QuoteUpdate> = None;

    while let Some(update) = combined.next().await {
        if update.exchange.name == "RAYDIUM_CLMM" {
            last_raydium = Some(update.clone());
        } else if update.exchange.name == "BINANCE" {
            last_binance = Some(update.clone());
        }

        if let (Some(raydium_quote_update), Some(binance_quote_update)) = (&last_raydium, &last_binance) {
            check_arbitrage(
                datasource_raydium.exchange(),
                raydium_quote_update,
                datasource_binance.exchange(),
                binance_quote_update);
        }
    }

    Ok(())
}

fn check_arbitrage(first_exchange: &Exchange,
                   first: &QuoteUpdate,
                   second_exchange: &Exchange,
                   second: &QuoteUpdate) {
    let diff_buy_first_sell_second = second.best_quote.bid_price - first.best_quote.ask_price;
    let diff_buy_second_sell_first = first.best_quote.bid_price - second.best_quote.ask_price;

    if diff_buy_first_sell_second > 0.1 {
        println!("Arb: buy {} @ {:.4}, sell {} @ {:.4}, spread {:.4}",
            first_exchange.name,
            first.best_quote.ask_price,
            second_exchange.name,
            second.best_quote.bid_price,
            diff_buy_first_sell_second);
    }
    if diff_buy_second_sell_first > 0.1 {
        println!("Arb: buy {} @ {:.4}, sell {} @ {:.4}, spread {:.4}",
                 second_exchange.name,
                 second.best_quote.ask_price,
                 first_exchange.name,
                 first.best_quote.bid_price,
                 diff_buy_second_sell_first);
    }
}