# Signed transactions, the ledger, and derived views

Apa Kan's hosted state is a small state-machine / chain-style service:

- **Transactions** are the only way to change state. One common, signed
  envelope (`apa-kan-tx::SignedTx`) carries every mutation: game creation,
  seat joins, moves, resignations, cancellation, and later account and admin
  actions.
- **The ledger** (`apa-kan-ledger`) is an ordered, append-only,
  hash-linked log of accepted transactions, one chain per **stream**.
- **State** is the fold of a stream through a deterministic transition
  function (`apa-kan-tx::game::GameState::apply`).
- **Views** (`apa-kan-view`) are read models derived from state. Views never
  mutate; transactions never read except through the state they fold into.

HTTP is one transport (`POST /v1/tx` to write; `GET` views to read). The same
bytes can travel peer-to-peer over UDP later.

## Envelope

```jsonc
{
  "body": {
    "domain": "apa-kan/tx/v1",        // domain separation + version
    "chain": "apa-kan-local",          // deployment / network id (APAKAN_CHAIN_ID)
    "stream": "game/3f9c0a…",          // which chain of state
    "seq": 7,                          // position in the stream (0 = first)
    "prev": "<64 hex>",                // id of entry seq-1; all zeros for seq 0
    "signer": "<64 hex AccountId>",    // the account acting
    "signer_vk": "<hex ML-DSA-65 vk>", // the account key that signed
    "payload": { "type": "game.move", "action": { "kind": "move", "from": 3, "to": 9 } }
  },
  "sig": "<hex ML-DSA-65 signature>"
}
```

Payload types (tagged by `type`, namespaced by stream kind):

| `type` | Fields | Allowed when |
|---|---|---|
| `game.create` | `seat` (1/2), `label` | `seq = 0` on a new `game/<id>` |
| `game.join` | `label` | phase `open`, signer not seated |
| `game.move` | `action` (`ActionWire`) | phase `playing`, signer holds the seat to move, engine accepts |
| `game.resign` | — | phase `playing`, signer seated |
| `game.cancel` | — | phase `open`, signer is the creator |

Future `account.*` (link key, revoke key, set email…) and `admin.*` payloads
use the same envelope on `account/<id>` / `admin/<scope>` streams; adding one
is a new enum variant plus a transition function for that stream kind.

### Signing and ids — mirrors `apa-kan-account`

Exactly the account crate's scheme, not a new one:

1. `canonical = to_canonical_json(body)` (sorted keys).
2. `id = SHA3-256(canonical ‖ signer_vk)` (`apa_kan_account::crypto::message_id`).
3. `sig = ML-DSA-65.sign(id)` (`sign_bytes`), verified with `verify_bytes`.

`id` is also the ledger entry hash, so the next transaction's `prev` commits
to the whole previous body *and* its signer key. Field order on the wire does
not matter.

### Account indirection

Seats belong to **accounts**, not keys. A transaction names its account
(`signer`) and the key that signed (`signer_vk`). Admission asks the account
directory (`authorized_keys` in `apa-kan-store`) about that pair:

| Directory says | Result |
|---|---|
| account registered, key `active` | accepted |
| account registered, key revoked / pending recovery | `key_not_authorized` |
| account registered, key never linked | `key_not_authorized` |
| account not registered | accepted **only if** `signer == AccountId::from_genesis_vk(signer_vk)` |

So a fresh device key works immediately as a self-certifying guest account,
can be registered later with `POST /v1/accounts` (same id), and additional
devices join through the existing link-request flow and then sign for the
same account. Revoking a key stops it from acting on any stream.

## Ordering, replay and fork protection

- **Total order per stream:** `seq` counts from 0; a transaction is valid only
  at exactly `seq = stream length`.
- **Hash link:** `prev` must equal the current head id. A transaction signed
  against a stale or alternative history can't be applied, even at the right
  `seq`.
- **Domain + chain:** a signature for another protocol version, another
  deployment, or another stream can't be replayed here (all are in the signed
  body).
- **Ledger CAS:** `append` lands only if `(seq, prev)` match the stored head
  (`UPDATE ledger_streams … WHERE next_seq = $seq AND head_hash = $prev`, or
  inserting the stream row for `seq = 0`), and inserts the entry in the same DB
  transaction. `tx_hash` is unique across the ledger. Of N concurrent submits
  at one `seq`, exactly one commits; the rest get `409 stale_seq`.
- **Commit before acknowledge:** the host applies the transaction to a clone of
  the state, awaits the ledger commit, and only then swaps state and returns
  `200`. A storage error returns `503` and nothing is acknowledged; a retry of
  the same signed bytes either lands or gets `stale_seq`.

No separate per-key nonce is used for game streams: `(chain, stream, seq,
prev)` already makes every signed body single-use.

## State machine

```
game.create (seq 0) ──► open ──game.join──► playing ──game.move*──► finished
                         │                    └──game.resign──────► finished
                         └──game.cancel (creator)─────────────────► cancelled
```

`GameState::apply(state, tx, id)` is pure: same state and transaction give the
same next state on every host or peer. It checks seq, prev, payload vs. phase,
the signer's seat, and the engine (`apa-kan-core`, deterministic). It does
**not** verify signatures — callers do that first:

- **Admission** (`admit`): domain, chain, shape, signature, key authority
  (directory lookup). Run once, when a transaction first arrives.
- **Replay** (`game::replay`): domain, chain, shape, and signature for every
  stored entry, then `apply`. Key authority is not re-checked on replay (it
  was valid when committed; revoking a key later must not rewrite history).

The deal is fixed at join: `seed = SHA3("apa-kan/deal-seed/v1" ‖ create_id ‖
join_id)[..8]`, so the creator alone can't choose it.

Loading a game = read the stream from the ledger, replay. The server keeps an
in-memory cache of folded streams, one lock per stream, refreshed whenever the
ledger head differs (another server instance or a relay committed first).

## Views

| Route | View |
|---|---|
| `GET /v1/ledger` | `{ domain, chain, backend }` — what a client needs to sign |
| `GET /v1/games/{id}` | `HostedGameView`: phase, `ledger` head (`chain`, `stream`, `seq`, `head`), seat claims (account + label), and the `GameView` (status, board, legal actions, replayable record) once both seats are in |
| `GET /v1/games/{id}/txs?from=n` | committed signed transactions from `n` (audit, resync, peers) |

The only game write route is `POST /v1/tx`.

## Peer-to-peer (not built)

- Peers exchange `SignedTx` directly. Each verifies (`verify_self_contained`),
  applies to its local fold, and shows the move at once.
- The ledger host stays the **reconciliation point**: either peer relays each
  transaction to `POST /v1/tx`; first commit at a `seq` wins via the CAS;
  duplicate relays get `stale_seq` and are harmless.
- In a two-seat turn game only the seat to move can produce a valid
  transaction at `seq`, so honest peers never fork. Two different signed
  transactions by one account for the same `(stream, seq, prev)` are
  equivocation, and the pair is self-contained proof of it.
- A peer that diverges (missed datagrams, crash) fetches
  `/v1/games/{id}/txs?from=k` and replays.
- Size: an ML-DSA-65 envelope is ~10 KB as hex JSON (vk 1952 B, sig 3309 B).
  For UDP, a compact binary encoding plus referencing the signer key by hash
  (keys learned once per session) would bring it near one datagram; that is a
  transport encoding change, not an envelope change.

## Backends

| URL (`APAKAN_LEDGER_URL`) | Use | Durability |
|---|---|---|
| `memory` | tests | none |
| `sqlite:<path>` | single node (default; falls back to `APA_KAN_DATABASE_URL`, then `$TMP/apa-kan-ledger.db`) | WAL + `synchronous=FULL` |
| `postgres://…` | multi-instance, platform DB (#97); build with `--features postgres` | Postgres commit |

Migrations: `migrations/{sqlite,postgres}/001_ledger.sql`, idempotent, run at
connect.
