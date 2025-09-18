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
use crate::datasource::quote::{BestQuote, Venue, Instrument, QuoteUpdate};

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

    pub async fn from_pool(
        rpc_url: &str,
        ws_url: &str,
        pool_pubkey_str: &str,
        venue_name: &str,
    ) -> Result<Self> {

        let program_id = raydium_clmm::ID;
        let pool_pk = Pubkey::from_str(pool_pubkey_str)?;

        let client = Client::new_with_options(
            Cluster::Custom(rpc_url.to_string(), rpc_url.to_string()),
            Rc::new(Keypair::new()),
            CommitmentConfig::processed(),
        );

        let program = client.program(program_id)?;

        let pool_state: PoolState = program.account(pool_pk).await?;
        let cfg: AmmConfig = program.account(pool_state.amm_config).await?;

        let fee_rate = (cfg.trade_fee_rate as f64) / 1_000_000.0;

        println!("Raydium pool's fee rate is {}", fee_rate);

        Ok(Self {
            ws_url: ws_url.to_string(),
            pool_pubkey: pool_pk,
            venue: Venue { name: venue_name.to_string() },
            fee_rate,
            half_spread_bps: 2.0, // or configurable
        })
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
        let half_spread_bps = self.half_spread_bps;
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

                // derive mid price from sqrt_price_x64
                let r = (pool_state.sqrt_price_x64 as f64) / (2f64.powi(64));
                let p_raw = r * r;
                let scale = 10f64.powi(pool_state.mint_decimals_0 as i32 - pool_state.mint_decimals_1 as i32);
                let mid = p_raw * scale;

                let spread_frac = half_spread_bps / 10_000.0;
                let bid = mid * (1.0 - spread_frac);
                let ask = mid * (1.0 + spread_frac);

                let now = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH).unwrap();
                let ts_ms = (now.as_secs() as i64) * 1000 + (now.subsec_millis() as i64);

                yield QuoteUpdate {
                    ts: ts_ms,
                    venue: venue.clone(),
                    instrument: instrument_clone.clone(),
                    best_quote: BestQuote {
                        bid_price: bid,
                        bid_size: 0.0,
                        ask_price: ask,
                        ask_size: 0.0,
                    },
                    fee_rate,
                };
            }
        };

        Ok(Box::pin(s))
    }
}