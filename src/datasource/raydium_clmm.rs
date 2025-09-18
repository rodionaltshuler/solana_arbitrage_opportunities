use anchor_client::{
    solana_sdk::{
        commitment_config::CommitmentConfig, signature::Keypair
    },
    Client, Cluster,
};
use anchor_lang::prelude::*;
use std::rc::Rc;

use async_stream::stream;
use futures::StreamExt;
use base64::Engine;
use base64::engine::general_purpose::STANDARD as BASE64;
use crate::datasource::domain::{BestQuote, Venue, Instrument, QuoteUpdate};

use anyhow::Result;
use std::{str::FromStr};
use anchor_client::solana_account_decoder::{UiAccountData, UiAccountEncoding};
use anchor_client::solana_client::nonblocking::pubsub_client::PubsubClient;
use anchor_client::solana_client::rpc_config::RpcAccountInfoConfig;
use futures::stream::BoxStream;
use crate::datasource::datasource::DataSource;

declare_program!(raydium_clmm);

use raydium_clmm::{accounts::{AmmConfig, PoolState}};

pub struct RaydiumClmmSource {
    ws_url: String,
    pool_pubkey: Pubkey,
    half_spread_bps: f64,
    venue: Venue,

    pub(crate) fee_rate: f64,  // as fraction, e.g. 0.0001 = 1 bps
}

impl RaydiumClmmSource {

    const RPC_URL: &'static str = "https://api.mainnet-beta.solana.com";
    const WS_URL: &'static str  = "wss://api.mainnet-beta.solana.com";
    const POOL_PUBKEY: &'static str = "3ucNos4NbumPLZNWztqGHNFFgkHeRMBQAVemeeomsUxv"; // SOL/USDC

    pub async fn new(
        venue_name: &str,
    ) -> Result<Self> {

        let program_id = raydium_clmm::ID;
        let pool_pk = Pubkey::from_str(Self::POOL_PUBKEY)?;

        let client = Client::new_with_options(
            Cluster::Custom(Self::RPC_URL.to_string(), Self::RPC_URL.to_string()),
            Rc::new(Keypair::new()),
            CommitmentConfig::processed(),
        );

        let program = client.program(program_id)?;

        let pool_state: PoolState = program.account(pool_pk).await?;
        let cfg: AmmConfig = program.account(pool_state.amm_config).await?;

        let fee_rate = (cfg.trade_fee_rate as f64) / 1_000_000.0;

        println!("Raydium pool's fee rate is {}", fee_rate);

        Ok(Self {
            ws_url: Self::WS_URL.to_string(),
            pool_pubkey: pool_pk,
            venue: Venue { name: venue_name.to_string() },
            fee_rate,
            half_spread_bps: 2.0, // or configurable
        })
    }

    fn clmm_tick_quote(
        sqrt_price_x64: u128,
        liquidity: u128,
        tick_current: i32,
        tick_spacing: u16,
        base_dec: u8,
        quote_dec: u8,
    ) -> (f64, f64, f64, f64) {
        
        //println!("sqrt_price_x64: {}, liquidity: {}, tick_current: {}, tick_spacing: {}, base_dec: {}, quote_dec: {}",
        //    sqrt_price_x64, liquidity, tick_current, tick_spacing, base_dec, quote_dec);

        let s = (sqrt_price_x64 as f64) / (2f64.powi(64));
        let l = liquidity as f64;

        let exp = base_dec as i32 - quote_dec as i32;
        let scale = 10f64.powi(exp);

        let tick_lower = (tick_current / tick_spacing as i32) * tick_spacing as i32;
        let tick_upper = tick_lower + tick_spacing as i32;

        let sqrt_lower = 1.0001f64.powf(tick_lower as f64 / 2.0);
        let sqrt_upper = 1.0001f64.powf(tick_upper as f64 / 2.0);

        // ASK (buy base with quote, price up until sqrt_upper)
        let max_quote_in = l * (sqrt_upper - s);
        let base_out = l * (1.0/s - 1.0/sqrt_upper);
        let ask_price = (max_quote_in / base_out) * scale;

        // BID (sell base for quote, price down until sqrt_lower)
        let max_base_in = l * (1.0/sqrt_lower - 1.0/s);
        let quote_out = l * (s - sqrt_lower);
        let bid_price = (quote_out / max_base_in) * scale;

        let bid_size = max_base_in / 10f64.powi(base_dec as i32);
        let ask_size = base_out   / 10f64.powi(base_dec as i32);

        (bid_price, bid_size, ask_price, ask_size)
    }

}

#[async_trait::async_trait]
impl DataSource for RaydiumClmmSource {
    fn venue(&self) -> &Venue {
        &self.venue
    }

    async fn subscribe_best_quotes(
        &self,
        instrument: Instrument,
    ) -> Result<BoxStream<'static, QuoteUpdate>> {
        let ws_url = self.ws_url.clone();
        let pool_pk = self.pool_pubkey;
        let venue = self.venue.clone();
        let instrument_clone = instrument.clone();
        let fee_rate = self.fee_rate;

        let s = stream! {
            let client = match PubsubClient::new(&ws_url).await {
                Ok(c) => c,
                Err(e) => {
                    eprintln!("PubSub connect failed: {e}");
                    return;
                }
            };

            let (mut sub, _unsub) = match client.account_subscribe(
                &pool_pk,
                Some(RpcAccountInfoConfig {
                    encoding: Some(UiAccountEncoding::Base64),
                    commitment: None,
                    data_slice: None,
                    min_context_slot: None,
                }),
            ).await {
                Ok(s) => s,
                Err(e) => {
                    eprintln!("account_subscribe failed: {e}");
                    return;
                }
            };

            while let Some(update) = sub.next().await {
                // decode using Anchor
                let raw_bytes: Vec<u8> = match &update.value.data {
                    UiAccountData::Binary(b64, _) => {
                        match BASE64.decode(b64) {
                            Ok(bytes) => bytes,
                            Err(e) => {
                                eprintln!("base64 decode error: {e}");
                                continue;
                            }
                        }
                    }
                    _ => continue,
                };

                let mut data_slice: &[u8] = &raw_bytes;
                let pool_state: PoolState = match PoolState::try_deserialize(&mut data_slice) {
                    Ok(state) => state,
                    Err(e) => {
                        eprintln!("anchor deserialize error: {e}");
                        continue;
                    }
                };

                let now = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH).unwrap();
                let ts_ms = (now.as_secs() as i64) * 1000 + (now.subsec_millis() as i64);

                let (bid_price, bid_size, ask_price, ask_size) = RaydiumClmmSource::clmm_tick_quote(
                    pool_state.sqrt_price_x64,
                    pool_state.liquidity,
                    pool_state.tick_current,
                    pool_state.tick_spacing,
                    pool_state.mint_decimals_0,
                    pool_state.mint_decimals_1,
                );

                yield QuoteUpdate {
                    ts: ts_ms,
                    venue: venue.clone(),
                    instrument: instrument_clone.clone(),
                    best_quote: BestQuote {
                        bid_price,
                        bid_size,
                        ask_price,
                        ask_size,
                    },
                    fee_rate,
                };
            }
        };

        Ok(Box::pin(s))
    }
}