## Arbitrage opportunity checker between on-chain and centralised exchange

The program that monitors a market in Solana network and the same market in any CEX in search for arbitrage opportunities across the 2 venues. Connectivity should be done via websockets to both places to get live updates and constantly assess the potential for arbitrage.


### Current Solution

#### Pair: SOL-USDC

#### On-chain venue: 
Raydium SOL-USDC concentrated liquidity pool
https://raydium.io/liquidity-pools
Pool-id: 8EzbUfvcRT1Q6RL462ekGkgqbxsPmwC5FMLQZhSPMjJ3

#### CEX: Binance

The current implementation connects to both Raydium CLMM on Solana and Binance via WebSocket, subscribes to live price updates, and monitors for potential arbitrage opportunities.****__
 
- **RaydiumClmmSource** subscribes to a CLMM pool account, fetches AMM config to get fee data, decodes the pool state, and derives a synthetic bid/ask from the on-chain price. 
- **BinanceSource** connects to Binance’s `bookTicker` stream to get the latest best bid/ask prices.
- Both streams are merged, the most recent quotes are cached, and an `ArbitrageChecker` compares the two exchanges to detect price spreads exceeding a configurable threshold.

- Each detected opportunity is logged with timestamp, spread, and quote staleness.  


Output example:
```
[Arb] ts=1758105620456 staleness=11960ms | Buy RAYDIUM_CLMM @ 234.5436, Sell BINANCE @ 234.5600, Spread 0.0164, Fees 0.1255
```

### Pre-requisites

Currently, you need rust and cargo to run the program (main.rs), version used:
rustc 1.89.0
cargo 1.89.0

### How to run

```
cargo run
```

### Project folders

- *resources*: IDL (interface definition language) fetched for Raydium Pool using fetch_idl.sh.
- *src*: program code
 

### Improvements Required

1. **RaydiumClmmSource**  
   Current implementation fetches pool state manually and relies on hard-coded schemas and response parsing.  
   Improvement: rewrite RaydiumClmmSource using the Anchor framework to simplify pool state fetching and avoid manual schema definitions.

2. **BinanceSource**  
   Current implementation only fetches the latest tick (`bookTicker`).  
   Improvement: integrate L2 order book stream instead, as this would be more appropriate for identifying real opportunities to buy/sell an asset and for evaluating available depth.

3. **Raydium**  
   Current version derives bid/ask prices manually from the mid price (sqrt_price_x64) and does not analyze depth or slippage.  
   Improvement: enhance the implementation to read tick arrays and compute realistic liquidity depth and slippage for trades.
