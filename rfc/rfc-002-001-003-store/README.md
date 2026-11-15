# RFC 002-001-003: Storage traits, SQL, and migrations

**Milestone:** M1 (SQLite local), M3+ (Postgres online)  
**Status:** accepted for platform bring-up (trait surface grows with milestones)  
**Tracks:** [#56](https://github.com/ramate-io/apa-kan/issues/56), [#57](https://github.com/ramate-io/apa-kan/issues/57)

## Problem

Accounts, keys, inventories, ratings, lobbies, and game transcripts need durable storage with testable abstractions. The server must not embed SQL details; offline SQLite should mirror online Postgres schema.

## Proposed solution

### Crate layout

`apa-kan/store`:

| Feature | Backend |
|---|---|
| `memory` (default) | In-process `HashMap` repos for unit tests |
| `sqlite` | `sqlx` + SQLite file or `:memory:` |
| `postgres` (stub) | Same queries, bound later for M3 |

Repositories (v1 implementation subset):

- `AccountRepo` — account row, email flags
- `KeyRepo` — authorized keys, nonces, link requests/approvals
- `InventoryRepo` — tile instance rows (opaque tile kind id until core split lands)
- `RatingRepo` — per-account Elo snapshot (single default ladder until M4 splits by tile/time)
- `RecoveryRepo` — pending recovery, email tokens, cancel

Full sketch from [#56](https://github.com/ramate-io/apa-kan/issues/56) adds `SeasonRepo`, `LobbyRepo`, `WagerRepo`, `ListingRepo`, `GameRepo` — **not implemented in v1 PR**; traits are documented here for direction.

### Hybrid persistence

- **State tables** for accounts, keys, inventory, ratings (current row + status enum).
- **Append-only `game_events`** deferred until M3 (`apa-kan-server` match hosting).

Transactions: freeze, link approval + nonce bump, recovery cancel must be atomic (`Store::transaction`).

### Migrations

Versioned SQL under `apa-kan/store/migrations/`. SQLite runs migrations on open; Postgres uses the same files with driver-specific types where needed later.

### v1 schema (SQLite)

```
accounts(account_id BLOB PK, genesis_vk BLOB, email, recovery_opt_in, approve_via_email, created_at)
authorized_keys(account_id, vk BLOB, label, status, next_nonce, added_at, revoked_at, usable_at)
link_requests(request_id BLOB PK, account_id, new_vk, payload_json, new_sig BLOB, status, expires_at, created_at)
link_approvals(request_id PK, approver_vk, approver_sig BLOB, approver_nonce, created_at)
tile_instances(instance_id INTEGER PK, account_id, tile_kind_id INTEGER, origin TEXT, frozen INTEGER)
ratings(account_id PK, elo_milli INTEGER, games INTEGER)
recovery_pending(account_id, pending_vk, token_hash BLOB, kind, usable_at, cancelled_at)
schema_version(singleton version INTEGER)
```

`tile_kind_id` is an opaque `u32` until `apa-kan-core` exports tile ids.

## Implementation

- `async_trait` on repos; `Store` aggregator + `MemoryStore` + `SqliteStore`.
- Errors: `StoreError` with conflict, not found, expired, bad nonce variants.

## Alternatives considered

- **Full event sourcing for inventory:** rejected for v1; row + transaction is enough.
- **Single `Store` impl without traits:** rejected; tests need memory backend.

## Open questions

None blocking v1 schema. Season and lobby tables wait on [#51](https://github.com/ramate-io/apa-kan/issues/51) and [#50](https://github.com/ramate-io/apa-kan/issues/50) decisions.