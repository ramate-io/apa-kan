# RFC 002-001-003: Storage traits, SQL, and migrations

**Milestone:** M1 (SQLite local), M3+ (Postgres online)  
**Status:** accepted for platform bring-up (trait surface grows with milestones)  
**Tracks:** [#56](https://github.com/ramate-io/apa-kan/issues/56), [#57](https://github.com/ramate-io/apa-kan/issues/57)

## Problem

Accounts, keys, inventories, ratings, lobbies, and game transcripts need durable storage with testable abstractions. The server must not embed SQL details; offline SQLite should mirror online Postgres schema.

## Proposed solution

### Crate layout

`apa-kan-store`:

| Feature | Backend |
|---|---|
| `memory` (default) | In-process `HashMap` repos for unit tests |
| `sqlite` | `sqlx` + SQLite file or `:memory:` |
| `postgres` (stub) | Same queries, bound later for M3 |

Repositories (v1 implementation subset):

- `AccountRepo`, `KeyRepo`, `InventoryRepo`, `RatingRepo`, `RecoveryRepo`
- Later: seasons, lobbies, wagers, game transcript repos per [#56](https://github.com/ramate-io/apa-kan/issues/56)

### Ratings and divisions (**Decision #58-Q1 / Q2**)

- **One Elo value** per account in `ratings` (single ladder for now).
- Wagered rated games **update** that Elo when `apa-kan-rating` lands (Q2).
- **`rating_division_slots`** reserves rows keyed by `(account_id, opponent_class human|bot, tile_option, time_control)` for concurrent division enrollment without duplicating the headline Elo column.

### Inventory and grants

- `tile_instances.trade_bind_until_unix` — **Decision #58-Q7:** every **new grant** (starter kit, future drops) is trade-bound for **48 hours** from grant time.
- **Decision #58-Q5:** ~**1/30** tile drop on beating a bot is **not** implemented in M1; `grant_tiles(..., granted_at_unix)` is the hook for drops + trade-bind.

### Hybrid persistence

State tables for accounts, keys, inventory, ratings; append-only game events deferred until M3.

### v1 schema (SQLite)

See `apa-kan/store/migrations/001_platform_v1.sql` — includes `trade_bind_until_unix` and empty `rating_division_slots`.

`tile_kind_id` remains opaque until `apa-kan-core` exports tile ids.

## Implementation

- `async_trait` on repos; `MemoryStore` + `SqliteStore`.
- Errors: `StoreError` with conflict, not found, expired, bad nonce variants.

## Alternatives considered

- **Full event sourcing for inventory:** rejected for v1.
- **Per-mode Elo columns now:** rejected; one `ratings` row + division slot table for growth.

## Open questions

| # | Topic | Status |
|---|---|---|
| 19 | `ml-dsa` (indirect) | **OPEN-Q** in workspace only |