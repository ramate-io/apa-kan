# Hosted game persistence and transaction transport

How hosted games are stored, and how the same transactions can later travel
peer-to-peer (e.g. over UDP) without changing their format.

## Model

A hosted game is:

- an **opening record** (`apa_kan_view::GameRecord` with no actions): rules,
  seed, the completed deal, budgets. Written once at create time.
- an **append-only log** of accepted transactions, keyed by
  `(game_id, seq)`. `seq` is the move number: the number of actions already
  applied. Row `seq = n` is the action that moved the game from `n` to `n+1`.

Core is deterministic, so `opening + log[0..n]` replayed through
`GameRecord::replay` *is* the position at move `n`. Nothing else is stored:
no board snapshots, no derived state. Snapshots can be added later as a cache
keyed by `(game_id, seq)`, never as the source of truth.

Seat bearer tokens are never stored; only their SHA3 hashes
(`apa_kan_tx::SeatSecret`). The log records *how* a move was authorized
(`auth_kind`) and, for future signed transactions, the public proof.

## Ordering by move number

`seq` gives a total order per game. A transaction for `seq = n` is valid only
against the position after exactly `n` actions; it carries that number, so a
receiver never has to guess what it applies to.

`GameStore::append` is a compare-and-swap:

```sql
BEGIN;
UPDATE hosted_games SET next_seq = next_seq + 1
 WHERE game_id = $1 AND next_seq = $2;   -- 0 rows => Conflict / NotFound
INSERT INTO hosted_game_txs (game_id, seq, …) VALUES ($1, $2, …);
COMMIT;
```

The cursor bump and the log row commit together; `PRIMARY KEY (game_id, seq)`
is a second guard. In Postgres the `UPDATE` takes the game's row lock, so a
racing append for the same `seq` blocks, re-evaluates `next_seq = $2`, and
matches zero rows. In SQLite the write transaction serializes writers. Either
way, of N concurrent submits for one move number exactly one commits and the
rest get `Conflict` → HTTP 409 `stale_seq` with the current number. The
conformance suite (`conformance::race`) runs 16 concurrent appends against
each backend and asserts exactly one lands.

## Commit before acknowledge

`GameHost::submit` (server):

1. lock the game's cache slot; refresh from the store if its head moved;
2. `authorize` the seat;
3. `apply_transaction` on **clones** of the record and engine (rejects stale
   seq, wrong turn, illegal action, game over);
4. `store.append(…)` and await the commit;
5. only then swap the new position into the cache and return 200 + receipt.

A storage error at step 4 returns 503 and leaves the cached position
untouched; the client retries the same transaction (same `seq`) and either it
lands or it gets `stale_seq` if it actually landed. Appends are idempotent in
effect: the same `(game_id, seq)` can never be applied twice.

## Validating against the deterministic engine

The store is dumb on purpose: it enforces ordering and atomicity, not rules.
Every writer validates a transaction by applying it to the engine at
`seq = n` before committing it. Every reader can re-validate the whole log by
replaying it (`LoadedGame::record().replay()`); a log that does not replay is
`Corrupt`, not silently accepted. Because the engine is deterministic, any two
parties holding the same opening and the same log prefix compute the same
position, legal moves, and result, with no trust in each other's state.

## Transport-agnostic transactions

`apa_kan_tx::Transaction { game_id, seq, seat, action, auth }` is plain serde
(JSON today; any self-describing encoding works) and is self-contained: it
names the game, the move number, the seat, the action, and its authorization.
Nothing in it depends on HTTP.

`Transaction::signing_payload()` is the canonical bytes that a signature
covers: domain `apa-kan/tx/v1`, game id, seq, seat, action, and *not* `auth`.
Today `auth` is a seat bearer token (fine over TLS to a host, wrong for
peers, since forwarding it hands out the seat). For p2p the planned variant is
`TxAuth::Signature { key, sig }` over `signing_payload()`, with each seat's
public key fixed in the opening record. A signed transaction can be relayed by
anyone, verified by everyone, and stored as-is (`auth_kind = 'signature'`,
`auth_json` holding key and sig).

## How p2p reconciles with the store

Not built here; this is the intended shape.

- Peers exchange transactions directly (UDP datagrams fit: a transaction is a
  few hundred bytes). Each peer validates by applying to its local engine at
  `seq`, then shows the move immediately.
- The store stays the **reconciliation point**. Either peer (or both) relays
  each transaction to the host. The host validates exactly as above and
  appends; first commit at a given `seq` wins via the CAS. Duplicate relays of
  the same transaction get `stale_seq` and are harmless.
- In a two-seat turn game only the seat to move can produce a valid
  transaction at `seq`, so honest peers never fork. A fork means one side
  signed two different actions for the same `seq` (equivocation): the host
  keeps whichever committed first, and the conflicting signed pair is proof
  of cheating.
- A peer that diverges (missed datagrams, crash, equivocation) resyncs by
  fetching `GET /v1/games/{id}` (or the log from `seq` k onward) and replaying.
  UDP loss is handled by the same rule: a peer that sees `seq = n+2` while at
  `n` asks a peer or the store for `n` and `n+1`.
- Results (ratings, ladders) are computed only from the store's log, never
  from a peer's claim.

## Backends

| Backend | Use | Durability |
|---|---|---|
| `memory` | tests, throwaway dev | none |
| `sqlite:<path>` | single node (default) | WAL + `synchronous=FULL` |
| `postgres://…` | multi-instance / platform DB (#97) | Postgres commit |

The server picks one from `APAKAN_GAME_STORE_URL`, falling back to
`APA_KAN_DATABASE_URL`, then a SQLite file in the temp dir. Migrations are
plain SQL under `migrations/{sqlite,postgres}` and run idempotently at
connect, matching `apa-kan-store`.
