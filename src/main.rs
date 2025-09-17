mod datasource;
mod arbitrage;

use anyhow::Result;
use futures::{StreamExt, stream::select};
use crate::datasource::quote::{Exchange, Instrument, QuoteUpdate};
use crate::datasource::raydium_clmm::RaydiumClmmSource;
use crate::datasource::datasource::DataSource;
use crate::datasource::binance::BinanceSource;
use crate::arbitrage::arbitrage_checker::ArbitrageChecker;

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
            println!("[Quote] [RAYDIUM] {:?}", update);
            last_raydium = Some(update.clone());
        } else if update.exchange.name == "BINANCE" {
            //println!("[Quote] [BINANCE] {:?}", update);
            last_binance = Some(update.clone());
        }

        let checker = ArbitrageChecker::new(0.01,
                                            datasource_raydium.fee_rate,
                                            datasource::binance::BINANCE_FEE);

        if let (Some(raydium_quote_update), Some(binance_quote_update)) = (&last_raydium, &last_binance) {
            checker.check(
                datasource_raydium.exchange(),
                raydium_quote_update,
                datasource_binance.exchange(),
                binance_quote_update,
            );
        }
    }

    Ok(())
}