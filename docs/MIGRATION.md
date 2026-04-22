# V1 → V2 Migration Guide

`rs-clob-client-v2` is a breaking change from `rs-clob-client`. Both crates can coexist in the same `Cargo.toml` during a phased rollout, but signed orders from one protocol cannot be submitted to the other's exchange.

## TL;DR

1. Replace `rs-clob-client = "0.1"` with `rs-clob-client-v2 = "0.1.0-alpha.1"`.
2. Drop `fee_rate_bps`, `nonce`, `taker` from `UserLimitOrder` / `UserMarketOrder`.
3. Optionally set `metadata`, `builder`, `timestamp` (all default to zero / now).
4. For EIP-1271 smart-contract wallets, pass `signature_type = Some(3)` (`Poly1271`).
5. Nothing else needs to change on the client-call side.

## Order struct changes

The V2 EIP-712 signed struct drops **4 fields** and adds **3 fields**. `expiration` remains in the API payload but is **NOT** covered by the signature.

| Field | V1 signed | V2 signed | V2 payload only |
|-------|-----------|-----------|-----------------|
| `salt` | ✅ | ✅ | — |
| `maker` | ✅ | ✅ | — |
| `signer` | ✅ | ✅ | — |
| `taker` | ✅ | ❌ | — |
| `tokenId` | ✅ | ✅ | — |
| `makerAmount` | ✅ | ✅ | — |
| `takerAmount` | ✅ | ✅ | — |
| `expiration` | ✅ (signed) | ❌ | ✅ (un-signed) |
| `nonce` | ✅ | ❌ | — |
| `feeRateBps` | ✅ | ❌ | — |
| `side` | ✅ | ✅ | — |
| `signatureType` | ✅ | ✅ | — |
| `timestamp` | ❌ | ✅ **(new)** | — |
| `metadata` | ❌ | ✅ **(new, bytes32)** | — |
| `builder` | ❌ | ✅ **(new, bytes32)** | — |

## Signature type

```rust
// V1: 3 variants
enum SignatureType { Eoa = 0, PolyProxy = 1, PolyGnosisSafe = 2 }

// V2: adds Poly1271
enum SignatureType { Eoa = 0, PolyProxy = 1, PolyGnosisSafe = 2, Poly1271 = 3 }
```

`Poly1271` uses EIP-1271 `isValidSignature(bytes32,bytes)` — the submitting contract (vault, Safe v2+, MPC wallet) verifies the signature itself. Pass `signature_type: Some(3)` to `ClobClient::new` to opt in.

## Domain

```
V1: { name: "Polymarket CTF Exchange", version: "1", chainId, verifyingContract }
V2: { name: "Polymarket CTF Exchange", version: "2", chainId, verifyingContract }
```

Only the `version` string changed. V1 and V2 signatures are NOT interchangeable because the domain separator differs.

## User type diff

### UserLimitOrder
```diff
  pub struct UserLimitOrder {
      pub token_id: String,
      pub price: f64,
      pub size: f64,
      pub side: Side,
-     pub fee_rate_bps: Option<u32>,
-     pub nonce: Option<u64>,
      pub expiration: Option<u64>,
-     pub taker: Option<Address>,
+     pub timestamp: Option<u64>,    // Unix ms, defaults to Date.now() equivalent
+     pub metadata: Option<B256>,    // bytes32, defaults to zero
+     pub builder: Option<B256>,     // bytes32, defaults to zero
  }
```

### UserMarketOrder
```diff
  pub struct UserMarketOrder {
      pub token_id: String,
      pub price: Option<f64>,
      pub amount: f64,
      pub side: Side,
      pub order_type: Option<OrderType>,
-     pub fee_rate_bps: Option<u32>,
-     pub nonce: Option<u64>,
-     pub taker: Option<Address>,
+     pub timestamp: Option<u64>,
+     pub metadata: Option<B256>,
+     pub builder: Option<B256>,
  }
```

### Minimal call-site diff

```diff
  let order = UserLimitOrder {
      token_id: "123…".to_string(),
      price: 0.52,
      size: 10.0,
      side: Side::Buy,
-     fee_rate_bps: None,
-     nonce: None,
      expiration: None,
-     taker: None,
+     timestamp: None,
+     metadata: None,
+     builder: None,
  };
```

## Client API surface

### Removed
| Symbol | Reason |
|--------|--------|
| `ClobClient::_resolve_fee_rate_bps` | V2 doesn't sign `feeRateBps`; no client-side reconciliation needed |
| `ClobError::InvalidFeeRate` | Unreachable after the above |
| `user_order.side → "0"/"1"` conversion in `signed_order_to_json` | V2 serializer emits `"BUY"`/`"SELL"` natively |

### Added
| Method | Auth | Endpoint |
|--------|------|----------|
| `post_heartbeat(id)` | L2 | `POST /v1/heartbeats` |
| `create_readonly_api_key()` | L2 | `POST /auth/readonly-api-key` |
| `get_readonly_api_keys()` | L2 | `GET /auth/readonly-api-keys` |
| `delete_readonly_api_key(key)` | L2 | `DELETE /auth/readonly-api-key` |
| `get_pre_migration_orders()` | L2 | `GET /data/pre-migration-orders` (paginated) |
| `get_builder_fees(code)` | — | `GET /fees/builder-fees/{code}` |
| `get_market_by_token(id)` | — | `GET /markets-by-token/{id}` |
| `get_clob_market(cid)` | — | `GET /clob-markets/{cid}` |
| `get_market_trades_events(cid)` | — | `GET /markets/live-activity/{cid}` |
| `get_spread(id)` | — | `GET /spread?token_id={id}` |
| `get_current_rewards()` | — | `GET /rewards/markets/current` (paginated) |
| `get_raw_rewards_for_market(cid)` | — | `GET /rewards/markets/{cid}` (paginated) |

### Unchanged public API
- `ClobClient::new(...)` signature and all 11 positional args stay the same; `signature_type: Option<u8>` now accepts `3` for `Poly1271`.
- All L1 auth (`create_api_key` / `derive_api_key` / `create_or_derive_api_key`), order posting, cancellation, public market data, notifications, balance/allowance, rewards-user-day methods.
- L1 `ClobAuthDomain` (API-key signing) is unchanged — domain version stays `"1"`.

## What you must test before mainnet

1. **Signature compat**: the 11-field struct + domain-version bump means V1 signatures silently fail against V2 backends. Verify with testnet orders first.
2. **Expiration semantics**: server behavior around `expiration` is protocol-enforced in V2 (not signature-bound). Confirm your expected expiry is honored.
3. **`Poly1271` integration**: if you use smart-contract wallets, end-to-end test against the specific wallet contract's `isValidSignature` implementation.
4. **Builder referral** (optional): set `builder` (`B256`) on orders and confirm the builder code is attributed server-side.

## Cross-language validation

V2 signatures from `rs-clob-client-v2` are verified **byte-for-byte** against the TS V2 SDK for 7 canonical input configurations (EOA buy/sell, PolyGnosisSafe buy, Poly1271 sell, with `metadata`, with `builder`, with non-zero `expiration`). See `rs-order-utils/tests/v2_cross_language_vectors.rs` and `test_vectors_v2.json`. Regeneration instructions are in the Rust test file's header.
