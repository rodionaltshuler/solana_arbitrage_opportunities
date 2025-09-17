mod datasource;

use anyhow::Result;
use futures::StreamExt;
use crate::datasource::quote::Instrument;
use crate::datasource::raydium_clmm::RaydiumClmmSource;
use crate::datasource::datasource::DataSource;

#[tokio::main]
async fn main() -> Result<()> {

    let ws_url = "wss://api.mainnet-beta.solana.com";
    let rpc_url = "https://api.mainnet-beta.solana.com";

    let pool = "3ucNos4NbumPLZNWztqGHNFFgkHeRMBQAVemeeomsUxv"; // SOL/USDC CLMM pool state

    let input_on_chain = RaydiumClmmSource::from_pool(
        rpc_url,
        ws_url,
        pool,
        "RAYDIUM_CLMM",
    ).await?;

    let instrument = Instrument::new("SOL", "USDC");
    let mut stream = input_on_chain.subscribe_best_quotes(instrument).await?;

    while let Some(update) = stream.next().await {
        println!("{:?}", update);
    }

    Ok(())
}
