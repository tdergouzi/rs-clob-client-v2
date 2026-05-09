# Changelog

All notable changes to `rs-clob-client-v2` are documented in this file.

The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and the project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.2.1] — 2026-05-09

### Added

- **End-to-end V2 `Poly1271` (deposit-wallet) order signing.** `create_limit_order` / `create_market_order` now set the order's `signer` field to the deposit-wallet contract (= `maker`) when `signature_type == Poly1271`, while the EOA continues to produce the inner ECDSA. Combined with the `rs_order_utils 0.4.2` bump below, this yields blobs that the deposit-wallet's `isValidSignature` (ERC-1271) accepts on-chain. The EOA / PolyProxy / PolyGnosisSafe paths are unchanged.

### Changed

- Bumped `rs_order_utils` from `0.4.1` to `0.4.2` for the ERC-7739 nested `TypedDataSign` Poly1271 signing path.

### Notes

- Cross-language parity (`rs-order-utils/tests/v2_cross_language_vectors.rs`) currently covers `Eoa` / `PolyProxy` / `PolyGnosisSafe` only. `Poly1271` is excluded pending the upstream TS `@polymarket/clob-client-v2` SDK adopting the same ERC-7739 wrapping.
