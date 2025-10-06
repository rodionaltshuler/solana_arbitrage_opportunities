mod datasource;
mod arbitrage;

use anyhow::Result;
use futures::{StreamExt, stream::select};
use crate::datasource::domain::{Instrument, QuoteUpdate};
use crate::datasource::raydium_clmm::RaydiumClmmSource;
use crate::datasource::datasource::DataSource;
use crate::datasource::binance::{BinanceSource, BINANCE_FEE};
use crate::arbitrage::arbitrage_checker::ArbitrageChecker;

#[tokio::main]
async fn main() -> Result<()> {

    let instrument = Instrument::new("SOL", "USDC");

    let datasource_raydium = RaydiumClmmSource::new(
        "RAYDIUM_CLMM",
    ).await?;

    let datasource_binance = BinanceSource::new("BINANCE");

    let binance_stream = datasource_binance
        .subscribe_best_quotes(instrument.clone()).await?;

    let raydium_stream = datasource_raydium
        .subscribe_best_quotes(instrument.clone()).await?;

    let mut combined = select(raydium_stream, binance_stream);

    let mut last_raydium: Option<QuoteUpdate> = None;
    let mut last_binance: Option<QuoteUpdate> = None;

    let arbitrage = ArbitrageChecker::new(0.01,
                                          datasource_raydium.fee_rate,
                                          BINANCE_FEE);

    while let Some(update) = combined.next().await {
        if update.venue.name == "RAYDIUM_CLMM" {
            //println!("[Quote] [RAYDIUM] {:?}", update);
            last_raydium = Some(update.clone());
        } else if update.venue.name == "BINANCE" {
            //println!("[Quote] [BINANCE] {:?}", update);
            last_binance = Some(update.clone());
        }

        if let (Some(raydium_quote_update), Some(binance_quote_update)) = (&last_raydium, &last_binance) {
            let opportunity = arbitrage.check(
                datasource_raydium.venue(),
                raydium_quote_update,
                datasource_binance.venue(),
                binance_quote_update,
            );
            if let Some(opportunity) = opportunity {
                println!("{}", opportunity);
            }
        }
    }

    Ok(())
}