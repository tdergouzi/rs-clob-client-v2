# CLAUDE.md

Guidance for Claude Code in this repository.

## Project

`rs-clob-client-v2` — Rust port of Polymarket's `@polymarket/clob-client-v2` (V2 CTF Exchange protocol: 11-field EIP-712 order, `metadata` / `builder` / `timestamp`, `Poly1271` signatures).

Order building delegates to the sibling crate `rs-order-utils` (path reference at `../rs-order-utils` during V2 dev).

The V1 client lives in the sibling repo `rs-clob-client` and is frozen for bug fixes only.

## Workflow

1. Analyze scope and risk.
2. Propose files to touch + key decisions; pause for confirmation on non-trivial changes.
3. Execute; surface issues that fall outside the agreed scope.

Skip propose/confirm for typos, formatting, or explicit unambiguous instructions.

## Build

`cargo check` · `cargo test` · `cargo fmt --all` · `cargo clippy`

Integration tests need a `.env` file with `PK` + API creds.

## Commits

Format: `<type>(<scope>): <subject>`

**Types**: `feat` · `fix` · `docs` · `style` · `refactor` · `test` · `chore` · `perf`

**Rules**:
- **Subject line ≤ 72 characters.** No descriptive body paragraphs.
- Lowercase, imperative mood (`add`, not `added`).
- Scope optional (e.g. `order`, `signing`, `deps`, `types`, `client`).
- `Co-Authored-By:` trailer is allowed (and appropriate when pair-authored).

Examples:
- `feat(order): add v2 builder with metadata + builder fields`
- `fix(signing): correct v2 domain version to "2"`
- `chore(deps): path-ref rs_order_utils for v2 dev`
- `test(eip712): add v2 hash determinism regression`

## Simplicity

- Inline single-use logic; do not create helpers for one-time use.
- Extract a helper only when reused 3+ times or it encapsulates real complexity.
- Do not add fallbacks, validation, or error handling for scenarios that cannot happen.
