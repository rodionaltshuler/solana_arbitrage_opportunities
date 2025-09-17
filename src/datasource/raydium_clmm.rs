use std::{str::FromStr, time::{SystemTime, UNIX_EPOCH}};

use anyhow::{anyhow, Result};
use async_stream::stream;
use base64::Engine;
use base64::engine::general_purpose::STANDARD as BASE64;
use futures::stream::BoxStream;
use solana_account_decoder::{UiAccount, UiAccountData, UiAccountEncoding};
use solana_client::nonblocking::rpc_client::RpcClient;

use crate::datasource::datasource::DataSource;
use crate::datasource::quote::{BestQuote, Exchange, Instrument, QuoteUpdate};

use borsh::BorshDeserialize;
use solana_sdk::pubkey::Pubkey;

// anchor idl fetch CAMMCzo5YL8w4VFF8KVHrK22GGUsp5VTaW7grrKgrWqK --provider.cluster mainnet > raydium_clmm.json
#[derive(Debug, BorshDeserialize)]
pub struct PoolStateHead {
    pub discriminator: [u8; 8],
    pub bump: [u8; 1],
    pub amm_config: Pubkey,
    pub owner: Pubkey,
    pub token_mint_0: Pubkey,
    pub token_mint_1: Pubkey,
    pub token_vault_0: Pubkey,
    pub token_vault_1: Pubkey,
    pub observation_key: Pubkey,
    pub mint_decimals_0: u8,
    pub mint_decimals_1: u8,
    pub tick_spacing: u16,
    pub liquidity: u128,
    pub sqrt_price_x64: u128,
    pub tick_current: i32,
}

#[derive(Debug, BorshDeserialize)]
pub struct AmmConfig {
    pub discriminator: [u8; 8],

    // IDL: kind=struct, fields in *this* order:
    pub bump: u8,
    pub index: u16,
    pub owner: Pubkey,

    pub protocol_fee_rate: u32,
    pub trade_fee_rate: u32,
    pub tick_spacing: u16,
    pub fund_fee_rate: u32,
    pub padding_u32: u32,       

    pub fund_owner: Pubkey,     
    pub padding: [u64; 3],
}

pub struct RaydiumClmmSource {
    ws_url: String,
    pool_pubkey: Pubkey,
    base_dec: u8,
    quote_dec: u8,
    half_spread_bps: f64,
    exchange: Exchange,
    owner_program_str: Option<String>,

    // cached static config
    pub(crate) fee_rate: f64,  // as fraction, e.g. 0.0001 = 1 bps
}

impl RaydiumClmmSource {

    const OWNER_PROGRAM: &'static str = "CAMMCzo5YL8w4VFF8KVHrK22GGUsp5VTaW7grrKgrWqK";

    #[allow(clippy::too_many_arguments)]
    pub async fn from_pool(
        rpc_url: &str,
        ws_url: &str,
        pool_pubkey_str: &str,
        exchange_name: &str,
    ) -> Result<Self> {
        let rpc_client = RpcClient::new(rpc_url.to_string());
        let pool_pk = Pubkey::from_str(pool_pubkey_str)?;

        println!("Fetching Raydium pool config to get fees info");
        let acc = rpc_client.get_account(&pool_pk).await?;
        let mut data: &[u8] = &acc.data;
        let pool_state: PoolStateHead = PoolStateHead::deserialize(&mut data)?;

        let acc_cfg = rpc_client.get_account(&pool_state.amm_config).await?;
        let mut data_cfg: &[u8] = &acc_cfg.data;
        let cfg: AmmConfig = AmmConfig::deserialize(&mut data_cfg)?;

        let fee_rate = (cfg.trade_fee_rate as f64) / 1_000_000.0;
        println!("Raydium pool's fee rate is {}", fee_rate);

        Ok(Self {
            ws_url: ws_url.to_string(),
            pool_pubkey: pool_pk,
            exchange: Exchange { name: exchange_name.to_string() },
            base_dec: pool_state.mint_decimals_0,
            quote_dec: pool_state.mint_decimals_1,
            fee_rate,
            half_spread_bps: 2.0, // or configurable
            owner_program_str: Some(Self::OWNER_PROGRAM.to_string()),
        })
    }

    fn unix_ms_now() -> i64 {
        let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap();
        (now.as_secs() as i64) * 1000 + (now.subsec_millis() as i64)
    }

    fn price_from_sqrtpx(&self, sqrt_price_x64: u128) -> f64 {
        let r = (sqrt_price_x64 as f64) / (2f64.powi(64));
        let p_raw = r * r;

        let exp: i32 = (self.base_dec as i32) - (self.quote_dec as i32);
        let scale = 10f64.powi(exp);

        p_raw * scale
    }


    fn read_le_u128(buf: &[u8], offset: usize) -> Result<u128> {
        let end = offset + 16;
        if end > buf.len() {
            return Err(anyhow!("buffer too small: need {} bytes, have {}", end, buf.len()));
        }
        let mut arr = [0u8; 16];
        arr.copy_from_slice(&buf[offset..end]);
        Ok(u128::from_le_bytes(arr))
    }

    fn parse_account_bytes(&self, ui_acc: &UiAccount) -> Result<Vec<u8>> {

        if let Some(expected_owner) = &self.owner_program_str {
            if &ui_acc.owner != expected_owner {
                return Err(anyhow!(
                "unexpected owner program {}, expected {}",
                ui_acc.owner,
                expected_owner
            ));
            }
        }

        match &ui_acc.data {
            UiAccountData::Binary(b64, enc) => {
                let raw = BASE64.decode(b64.as_bytes())?;
                match enc {
                    UiAccountEncoding::Base64 => Ok(raw),
                    UiAccountEncoding::Base64Zstd => {
                        let decompressed = zstd::decode_all(&*raw)?;
                        Ok(decompressed)
                    }
                    other => Err(anyhow!("unsupported encoding: {:?}", other)),
                }
            }
            UiAccountData::Json(_) => Err(anyhow!("expected Base64/Base64Zstd account data, got Json")),
            UiAccountData::LegacyBinary(_) => Err(anyhow!("legacy binary encoding not supported")),
        }
    }
}

#[async_trait::async_trait]
impl DataSource for RaydiumClmmSource {
    fn exchange(&self) -> &Exchange {
        &self.exchange
    }

    async fn subscribe_best_quotes(
        &self,
        instrument: Instrument,
    ) -> Result<BoxStream<'static, QuoteUpdate>> {
        use async_stream::stream;
        use futures::StreamExt;
        use solana_account_decoder::{UiAccountData, UiAccountEncoding};
        use solana_client::rpc_config::RpcAccountInfoConfig;
        use solana_client::nonblocking::pubsub_client::PubsubClient;

        let ws_url            = self.ws_url.clone();
        let pool_pk           = self.pool_pubkey;
        let half_spread_bps   = self.half_spread_bps;
        let exchange          = self.exchange.clone();
        let instrument_clone  = instrument.clone();
        let fee_rate_clone = self.fee_rate.clone();

        let s = stream! {
            let client = match PubsubClient::new(&ws_url).await {
                Ok(c) => c,
                Err(e) => { eprintln!("PubSub connect failed: {e}"); return; }
            };

            let (mut sub, _unsub) = match client
                .account_subscribe(
                    &pool_pk,
                    Some(RpcAccountInfoConfig {
                        encoding: Some(UiAccountEncoding::Base64),
                        commitment: None,
                        data_slice: None,
                        min_context_slot: None,
                    }),
                )
                .await
            {
                Ok(s) => s,
                Err(e) => { eprintln!("account_subscribe failed: {e}"); return; }
            };

            while let Some(update) = sub.next().await {
                let raw_bytes = match &update.value.data {
                    UiAccountData::Binary(b64, _) => {
                        match base64::engine::general_purpose::STANDARD.decode(b64.as_bytes()) {
                            Ok(bytes) => bytes,
                            Err(e) => { eprintln!("base64 decode error: {e}"); continue; }
                        }
                    }
                    _ => continue,
                };

                let mut data_slice: &[u8] = &raw_bytes;
                let pool_state: PoolStateHead = match PoolStateHead::deserialize(&mut data_slice) {
                    Ok(s) => s,
                    Err(e) => { eprintln!("borsh deserialize error: {e}"); continue; }
                };


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
                    exchange: exchange.clone(),
                    instrument: instrument_clone.clone(),
                    best_quote: BestQuote {
                        bid_price: bid,
                        bid_size: 0.0,
                        ask_price: ask,
                        ask_size: 0.0,
                    },
                    fee_rate: fee_rate_clone.clone(),
                };
            }
        };

        Ok(Box::pin(s))
    }
}