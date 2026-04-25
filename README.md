# rs-clob-client-v2

🦀 Rust client for Polymarket's CLOB (Central Limit Order Book) **v2 protocol**, with full EIP-712 signing and byte-level cross-language parity against the official TypeScript SDK.

Port of [`@polymarket/clob-client-v2`](https://github.com/Polymarket/clob-client-v2). V2 introduces an 11-field EIP-712 order struct (adds `metadata`, `builder`, `timestamp`; drops `taker`, `nonce`, `expiration`, `feeRateBps` from the signed payload), bumps the domain version to `"2"`, and adds `Poly1271` for EIP-1271 smart-contract wallets.

For the V1 client (frozen, bug-fix only), see [`rs-clob-client`](https://github.com/tdergouzi/rs-clob-client).

## Installation

```toml
[dependencies]
rs-clob-client-v2 = "0.1.1"
alloy-primitives = "0.8"
alloy-signer-local = "0.8"
tokio = { version = "1.0", features = ["full"] }
```

## Quick Start

```rust
use alloy_signer_local::PrivateKeySigner;
use rs_clob_client_v2::{ClobClient, Chain, OrderType, Side, UserLimitOrder};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let wallet = "0xYOUR_PRIVATE_KEY".parse::<PrivateKeySigner>()?;
    let host = "https://clob.polymarket.com".to_string();
    let gamma = "https://gamma-api.polymarket.com".to_string();

    // Bootstrap: derive an API key if one exists, otherwise create a new one.
    let bootstrap = ClobClient::new(
        host.clone(), gamma.clone(), Chain::Polygon,
        Some(wallet.clone()), None, None, None, None, false, None, None,
    )?;
    let creds = bootstrap.create_or_derive_api_key(None).await?;

    // Main client with L1 + L2 auth.
    let client = ClobClient::new(
        host, gamma, Chain::Polygon,
        Some(wallet), Some(creds),
        Some(0),    // 0=EOA, 1=PolyProxy, 2=PolyGnosisSafe, 3=Poly1271
        None, None, false, None, None,
    )?;

    // V2 limit order. metadata/builder/timestamp default to zero.
    // expiration is transmitted but NOT covered by the EIP-712 signature.
    let order = UserLimitOrder {
        token_id: "your_token_id".to_string(),
        price: 0.52,
        size: 10.0,
        side: Side::Buy,
        expiration: None,
        timestamp: None,
        metadata: None,
        builder: None,
    };
    let response = client.create_and_post_limit_order(&order, None, OrderType::Gtc).await?;
    println!("{response:?}");
    Ok(())
}
```

## Signature Types

| Value | Variant | Use case |
|-------|---------|----------|
| `0` | `Eoa` | Standard wallet (MetaMask, raw `PrivateKeySigner`) |
| `1` | `PolyProxy` | Polymarket proxy contract (email / Magic login) |
| `2` | `PolyGnosisSafe` | Gnosis Safe wallets |
| `3` | **`Poly1271`** *(V2 new)* | EIP-1271 smart-contract wallets / vaults |

## V2 Contracts (Polygon, chain 137)

| Name | Address |
|------|---------|
| CTF Exchange | `0xE111180000d2663C0091e4f400237545B87B996B` |
| Neg Risk CTF Exchange | `0xe2222d279d744050d28e00520010520000310F59` |
| Neg Risk Adapter | `0xd91E80cF2E7be2e162c6513ceD06f1dD0dA35296` |
| Conditional Tokens (CTF) | `0x4D97DCd97eC945f40cF65F87097ACe5EA0476045` |
| Collateral (pUSD) | `0xC011a7E12a19f7B1f670d46F03B03f3342E82DFB` |
| CollateralOnramp | `0x93070a847efEf7F70739046A929D47a521F5B8ee` |

V2 migrates collateral from USDC.e to **pUSD** (standard ERC-20 on Polygon, backed by USDC). API-only users wrap USDC.e → pUSD via `CollateralOnramp.wrap()`. Full contract list: [Polymarket docs](https://docs.polymarket.com/resources/contracts).

### Testing Endpoint

Before the V2 production cutover, point at `https://clob-v2.polymarket.com` (paired with `https://gamma-api.polymarket.com`). After cutover, `https://clob.polymarket.com` routes to V2 automatically.

## Market Data (public, no auth)

```rust
use rs_clob_client_v2::{ClobClient, Chain, PriceParams, Side};

let client = ClobClient::new(
    "https://clob.polymarket.com".into(),
    "https://gamma-api.polymarket.com".into(),
    Chain::Polygon,
    None, None, None, None, None, false, None, None,
)?;

let book   = client.get_orderbook(&"token_id".to_string()).await?;
let price  = client.get_price(PriceParams { token_id: "…".into(), side: Side::Buy }).await?;
let spread = client.get_spread("token_id").await?;
```

V2-specific market helpers:

- `get_market_by_token(token_id)` — condition-id resolution
- `get_clob_market(condition_id)` — CLOB market detail (`MarketDetails` with tokens, tick size, fee breakdown)
- `get_market_trades_events(condition_id)` — live trade feed
- `get_builder_fees(builder_code)` — maker/taker rates for a builder
- `get_current_rewards()` / `get_raw_rewards_for_market(condition_id)` — rewards catalogue

## Trading & Account

```rust
// Cancel a single order.
client.cancel_order("0xorder_id".to_string()).await?;

// Cancel all orders for a market.
client.cancel_market_orders(CancelMarketOrdersParams { asset_id: "token_id".into() }).await?;

// Cancel everything.
client.cancel_all().await?;

// List open orders.
let open = client.get_open_orders(None).await?;

// Pre-migration (V1-protocol) orders still owned by the account.
let legacy = client.get_pre_migration_orders().await?;

// Read-only API keys (V2 new): scope-limited creds for integrations.
let ro_resp = client.create_readonly_api_key().await?;
let keys    = client.get_readonly_api_keys().await?;
client.delete_readonly_api_key("key").await?;
```

## Configuration

### Chain

V2 is deployed on Polygon mainnet (`Chain::Polygon`, chain id 137) only.

### Proxy Support
```rust
let client = ClobClient::new(
    host, gamma, chain,
    Some(wallet), Some(creds), Some(0), None, None, false, None,
    Some("http://127.0.0.1:7890".to_string()),
)?;
```
Formats: `http://`, `https://`, `socks5://` (with optional `user:pass@`).

### Builder API (optional)
```rust
use rs_builder_signing_sdk::BuilderConfig;

let builder_cfg = BuilderConfig::new("builder_key".into(), "builder_secret".into());
let client = ClobClient::new(
    host, gamma, chain,
    Some(wallet), Some(creds), Some(0), None, None, false,
    Some(builder_cfg),
    None,
)?;

let builder_trades = client.get_builder_trades(None, None).await?;
let my_keys        = client.get_builder_api_keys().await?;
```

## Security

- 🔒 Never hardcode private keys; use env vars or a secret manager.
- 🔐 V2 signatures are bound to the chain id, exchange contract, and the protocol domain version `"2"`. V1 signatures do not verify against V2 backends.
- ⚠️ `expiration` is transmitted but NOT covered by the EIP-712 signature (V2 design). Expiry is enforced protocol-side.
- ✅ V2 signature output is cross-validated byte-for-byte against the TS V2 SDK for 7 canonical input scenarios (see `rs-order-utils/tests/v2_cross_language_vectors.rs`).

## Testing

```bash
cargo test --lib         # unit tests, no network
cargo test               # integration tests; requires .env with PK + API creds
cargo fmt --all          # format
cargo clippy             # lint
```

## Notice

⚠️ AI-assisted port. Audit thoroughly; verify order submission end-to-end against the V2 preview endpoint (`https://clob-v2.polymarket.com`) before switching production traffic to `https://clob.polymarket.com`.

## License

MIT

## Acknowledgments

- Upstream: [`Polymarket/clob-client-v2`](https://github.com/Polymarket/clob-client-v2)
- Built on [Alloy](https://github.com/alloy-rs/alloy) and [`rs-order-utils`](https://github.com/tdergouzi/rs-order-utils) v0.3 (V2 `v2` module)
