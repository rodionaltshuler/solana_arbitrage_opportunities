## Arbitrage opportunity monitor between on-chain and centralised exchange

The program that monitors a market in Solana network and the same market in any CEX in search for arbitrage opportunities across the 2 venues. Connectivity should be done via websockets to both places to get live updates and constantly assess the potential for arbitrage.


### Current Solution

#### Pair: SOL-USDC

#### On-chain venue: 
Raydium SOL-USDC concentrated liquidity pool
https://raydium.io/liquidity-pools
Pool-id: 8EzbUfvcRT1Q6RL462ekGkgqbxsPmwC5FMLQZhSPMjJ3

#### CEX: Binance

The current implementation connects to both Raydium CLMM on Solana and Binance via WebSocket, subscribes to live price updates, and monitors for potential arbitrage opportunities.****__
 
- **RaydiumClmmSource** subscribes to a CLMM pool account, fetches AMM config to get fee data, and derives a bid/ask price and liquidity at these prices within current tick. 
- **BinanceSource** connects to Binance’s `depth5@100ms` stream to get best available bid/ask prices and corresponding liquidity (possible trade sizes)
- Both streams are merged, the most recent quotes are cached, and an `ArbitrageChecker` compares the two exchanges to detect price spreads exceeding a configurable threshold.

- Each detected opportunity is logged with timestamp, spread, quote staleness, fee (per unit of base currency) and possible trade size.  


Output example:
```
[Arb] ts=1758219382799 staleness=5604ms | Buy RAYDIUM_CLMM @ 250.9866, Sell BINANCE @ 251.1200, Spread 0.1334, Fees 0.1343, Size 13.8726
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

- *idls*: interface definition language) fetched for Raydium Pool using fetch_idl.sh.
- *src*: program code