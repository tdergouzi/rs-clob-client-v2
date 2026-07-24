# Changelog

All notable changes to `rs-clob-client-v2` are documented in this file.

The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and the project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.2.3] — 2026-07-24

### Fixed

- **`post_order` / `post_orders` keep returning settlement transaction hashes.** As of the 2026-07-24 CLOB maintenance the exchange executes matches through an async pipeline: `POST /order` and `POST /orders` no longer carry `transactionsHashes` for FAK/FOK matches and return `tradeIDs` instead. Both methods now backfill the hashes transparently by polling `GET /data/trades` for the returned trade IDs, writing them back onto the response under both `transactionsHashes` and `transactionHashes`. Best-effort: 250 ms poll interval, a 30 s ceiling shared across a `post_orders` batch, and never turns a successful placement into an error. Trades whose status is `FAILED` contribute no hash. No caller changes are required.
- **`Trade` deserialization tolerates trades that have not been broadcast yet.** `transaction_hash` and `maker_orders` are now optional on the wire — the async pipeline creates the trade before its transaction exists.

### Notes

- A matched FOK/FAK `post_order` now blocks until its hashes resolve (typically a few hundred ms, 30 s worst case). With `use_server_time = true` each poll costs an extra `/time` request.

## [0.2.2] — 2026-06-23

### Fixed

- **`cancel_orders` request body now matches the Polymarket API.** The `DELETE /orders` endpoint expects a bare JSON array of order IDs (`["0x…", …]`), but the client was sending — and signing — a wrapped object (`{"order_ids": ["0x…", …]}`). This produced an HMAC over the wrong payload and a body the backend rejected. Both the signed body and the transmitted body are now the array itself.

## [0.2.1] — 2026-05-09

### Added

- **End-to-end V2 `Poly1271` (deposit-wallet) order signing.** `create_limit_order` / `create_market_order` now set the order's `signer` field to the deposit-wallet contract (= `maker`) when `signature_type == Poly1271`, while the EOA continues to produce the inner ECDSA. Combined with the `rs_order_utils 0.4.2` bump below, this yields blobs that the deposit-wallet's `isValidSignature` (ERC-1271) accepts on-chain. The EOA / PolyProxy / PolyGnosisSafe paths are unchanged.

### Changed

- Bumped `rs_order_utils` from `0.4.1` to `0.4.2` for the ERC-7739 nested `TypedDataSign` Poly1271 signing path.

### Notes

- Cross-language parity (`rs-order-utils/tests/v2_cross_language_vectors.rs`) currently covers `Eoa` / `PolyProxy` / `PolyGnosisSafe` only. `Poly1271` is excluded pending the upstream TS `@polymarket/clob-client-v2` SDK adopting the same ERC-7739 wrapping.
