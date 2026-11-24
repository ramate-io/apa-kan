# Signed transactions, the ledger, and derived views

Apa Kan's hosted state is a small state-machine / chain-style service:

- **Transactions** are the only way to change state. One common, signed
  envelope (`apa-kan-tx::SignedTx`) carries every mutation: game creation,
  seat joins, the deal reveal, moves, resignations, cancellation, account
  keys and recovery, and admin role grants.
- **The ledger** (`apa-kan-ledger`) is an ordered, append-only,
  hash-linked log of accepted transactions, one chain per **stream**.
- **State** is the fold of a stream through a deterministic transition
  function (`GameState::apply` for `game/<id>`, `AccountState::apply` for
  `account/<id>`, `RolesState::apply` for `admin/roles`).
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
directory — the fold of the account's own `account/<id>` stream — about
that pair:

| Account fold says | Result |
|---|---|
| stream exists, key `active` | accepted |
| stream exists, key revoked / pending recovery | `key_not_authorized` |
| stream exists, key never linked | `key_not_authorized` |
| no stream (unregistered) | accepted **only if** `signer == AccountId::from_genesis_vk(signer_vk)` |

So a fresh device key works immediately as a self-certifying guest account,
can be registered later with `account.register` (same id), and additional
devices join with `account.link_key` and then sign for the same account.
Revoking a key stops it from acting on any stream.

## Accounts and admin as transactions

There are no mutating account routes. Everything below is a `SignedTx` on
`POST /v1/tx`; reads are `GET /v1/accounts/{id}` (`AccountView` plus the
store's tiles and rating), `GET /v1/accounts/{id}/keys`, and
`GET /v1/admin/roles`.

| Payload (`account/<id>`) | Signed by | Rule |
|---|---|---|
| `account.register { label }` | genesis key | seq 0; `id == from_genesis_vk(signer_vk)`. Projects starter tiles and rating into the store after commit. |
| `account.link_key { vk, label, proof }` | active key | `proof` = the **new** key's ML-DSA signature over `SHA3(canonical {domain: "apa-kan/link-key/v1", chain, account} ‖ vk)`; nobody links a key they don't hold. |
| `account.revoke_key { vk }` | active key | never the last active key |
| `account.set_email { email?, recovery_opt_in, approve_via_email }` | active key | email trimmed, lowercased, checked for shape; switches need an email |
| `account.recovery_request { kind, email_proof, requested_at_unix }` | the key to recover (not on the account) | account opted in for `kind`; `email_proof = SHA3("apa-kan/recovery-email/v1" ‖ account ‖ pending_vk ‖ email)` matches the email on file; `requested_at_unix` within 10 min of the host clock; **one open recovery per account**; refused (`too_early`) before the retry cooldown ends (see below) |
| `account.recovery_challenge { token_hash, usable_at_unix }` | **operator** | emitted by the host right after a request commits; records `SHA3("apa-kan/recovery-token/v1" ‖ token)` and the cooldown end (72 h full recovery, 24 h email link approval); the token is mailed |
| `account.recovery_confirm { token }` | pending key | token hashes to `token_hash`; key added as `pending_recovery` |
| `account.recovery_finish` | pending key | host clock ≥ `usable_at_unix` (else `too_early`); key becomes `active`. A **full recovery revokes every other active key** (the old devices are presumed lost or compromised); email-link approval only adds the key. |
| `account.recovery_cancel { at_unix }` | any active key | drops the recovery; the pending key is revoked; `at_unix` within 10 min of the host clock starts the retry cooldown |

**Recovery spam and stuck requests.** At most one recovery is open per
account. Times that matter for these rules are signed (`requested_at_unix`,
`at_unix`, the operator's `usable_at_unix`), so replay is deterministic; hosts
only check them against their clock (±`MAX_CLOCK_SKEW_SECS` = 600 s) at
admission.
- After a cancel, new requests wait `recovery_retry_cooldown_secs` (default
  24 h) from `at_unix`.
- A request the operator never challenged (mail or commit failed) **expires**
  after `recovery_challenge_timeout_secs` (default 15 min); a challenged one
  whose token was not confirmed expires at its `usable_at_unix`. An expired
  recovery is a failure: the same pending key may re-request at once (the
  host challenges and mails again), any other key only after the retry
  cooldown counted from the expiry. Confirmed recoveries don't expire; an
  active key cancels them.
- Both timings are chain configuration in `AccountRules`
  (`APAKAN_RECOVERY_COOLDOWN_SECS`, `APAKAN_RECOVERY_CHALLENGE_TIMEOUT_SECS`
  on the server) and must be the same for every host replaying the chain.

The **operator** is a self-certifying account whose key the host holds
(`APAKAN_OPERATOR_SEED`, else a seed file at `APAKAN_OPERATOR_SEED_FILE`,
default `$TMP/apa-kan-operator.seed`). Its id is chain configuration
(`AccountRules`), published in `GET /v1/ledger` as `operator`: replay accepts
challenges only from it, so it must not change for a chain. Email recovery
needs it; without one, requests are refused.

Wall-clock rules (the cooldown) are checked by the host at admission; replay
passes no clock and trusts what was committed. The token itself never touches
the ledger until the pending key reveals it in `recovery_confirm`, after which
it is spent.

**Admin:** admin state lives on per-topic streams `admin/<topic>` (topic
`[a-z][a-z0-9_-]{0,31}`), all submitted through the normal `POST /v1/tx`.
Every `admin.*` payload names its topic (`TxPayload::admin_topic`) and is
refused on any other stream; the host routes by topic and refuses topics it
has no fold for. Authorization is shared (`require_admin`): the operator, or
an account holding `admin` in `admin/roles` (key checked against that
account's fold). Today's topic is `admin/roles` (`admin.grant_role { account,
role }`, `admin.revoke_role`); further admin actions (catalog, character-pass
publishing from #97) add payloads mapped to their topic plus a fold, without
new routes.

**Privacy:** `account.set_email` carries the address, so raw account
transactions are not served (no `/txs` view for accounts); views show only
`email_set`. This is fine while the server is the only host. **Before any
decentralization** (peers replicating account streams) the address must be
encrypted or replaced by a commitment.

## Protecting the account key: passkey and recovery phrase

A browser key in plaintext `localStorage` is lost when the browser is
cleared. Two complementary protections (`apa-kan-account::passkey`,
`::phrase`; client glue in `apa-kan/game/src/keyguard.rs`):

- **Passkey (WebAuthn PRF).** Settings → *Protect with passkey* creates a
  discoverable passkey (`user.id` = account bytes, user verification
  required) and evaluates its PRF extension on the constant input
  `SHA-256("apa-kan/passkey-prf/v1")` (constant so a new device can evaluate
  it before it knows the account). The 32-byte output keys
  HKDF-SHA256 (random 32-byte salt, info = `"apa-kan/passkey-wrap/v1" ‖
  account ‖ credential_id`) → AES-256-GCM (random 12-byte nonce, same bytes as
  AAD), encrypting the 32-byte ML-DSA seed. The wrapped record
  (`PasskeyWrappedSeed {v, credential_id, salt, nonce, ciphertext}`, hex) goes
  to IndexedDB (`apakan`/`keys`, keyed by account hex); the plaintext key is
  removed and `apakan.passkey.v1` records `{account, credential_id}`. With a
  server configured the client registers the account if needed and commits
  `account.set_passkey_backup {wrapped}` (max 8 per account;
  `account.remove_passkey_backup {credential_id}` drops one). The host only
  checks the record's shape; it never sees the PRF output. A finished full
  recovery drops all backups (they wrap the old, now revoked seeds).
- **Fetching the backup is gated.** The record carries `access_hash =
  SHA-256("apa-kan/passkey-access/v1" ‖ access_key)` with `access_key =
  HKDF-SHA256(PRF output, info = "apa-kan/passkey-access/v1" ‖ account ‖
  credential_id)`. `POST /v1/accounts/{id}/passkeys/{credential_id}
  {access_key}` serves the record only when the hash matches (403 otherwise,
  404 if there is none); it is still a view (no ledger change). A signed
  request isn't possible here: a restoring device has no account key yet,
  only the passkey, so proving the passkey is the credential.
- **Locked / unlock.** After a reload a passkey-protected key is locked:
  online play asks for *Unlock with passkey* (PRF via `get()` with the stored
  credential, then unwrap). Local bot games still work.
- **New device / cleared browser.** The passkey syncs through the platform
  password manager. *Restore with passkey* calls `get()` with no
  `allowCredentials`; the chosen passkey returns `userHandle` (the account)
  and its credential id, the client loads the wrapped seed from IndexedDB or
  the gated fetch above, and unwraps it.
- **No PRF.** If the authenticator reports `prf.enabled == false` (or the
  browser has no WebAuthn), nothing changes: the key stays in plaintext
  storage and the recovery phrase is shown instead.
- **Recovery phrase.** 24 BIP39 English words that are exactly the seed's
  256 bits (*Show recovery phrase*, *Restore from recovery phrase*; on
  desktop the phrase prints to the terminal and `APAKAN_RECOVERY_PHRASE`
  restores it). Works offline and with no server. After a phrase restore the
  web client offers passkey protection right away.
- **Prompt.** After a guest's first finished game on the web the lobby
  suggests protecting the key, once (`apakan.passkey.prompted`).
- **Linking.** A restored key is the same account key, so nothing new is
  linked. Adding a different device key still goes through
  `account.link_key`; the passkey backup itself is an account transaction.

Defaults and caveats: Safari may require the WebAuthn call to
come straight from a user gesture, and Bevy handles clicks a frame later, so
Safari may refuse the prompt; some authenticators only return PRF output on
`get()`, so `create()` is followed by one `get()` when needed.

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
game.create (seq 0) ──► open ──game.join──► revealing ──game.reveal──► playing ──game.move*──► finished
                         │                    │                           └──game.resign──────► finished
                         │                    └──game.cancel (joiner) ──► cancelled (no_reveal)
                         └──game.cancel (creator) ──────────────────────► cancelled (creator_withdrew)
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

### Deal seed: commit-reveal

Neither seat may pick the deal, and neither may see it before the other is
committed:

1. `game.create` carries `seed_commit = hex SHA3("apa-kan/deal-commit/v1" ‖
   len(stream) u64 LE ‖ stream ‖ salt)` for a 32-byte secret `salt`. The
   stream is bound in, so a commitment can't be replayed into another game.
2. `game.join` moves the game to `revealing`. The joiner signed the join
   without knowing `salt`.
3. `game.reveal { salt }` — creator only, `revealing` only, must hash to the
   commitment (else `bad_reveal`, HTTP 422). The deal is then
   `seed = SHA3("apa-kan/deal-seed/v2" ‖ salt ‖ join_tx_id)[..8]` (u64 LE) and
   the game is `playing`. The creator fixed `salt` before seeing the join id.

**Non-reveal (default):** while `revealing`, the joiner may `game.cancel` at
any time → `cancelled` with `cancel_reason: "no_reveal"`, and the result view
records it as the **creator's forfeit** (`result.reason =
"no_reveal_forfeit"`, joiner wins). The creator may **not** cancel after a
join (it can already compute the deal, so cancelling would let it re-roll).
There is no clock-based forfeit: `apply` has no trusted time and must replay
identically everywhere; a timeout belongs in a host-side policy or in the
client offering "cancel" after a wait.

**Results (ready for ratings):** starting a game and not finishing it is a
loss. `HostedGameView.result = { winner, loser, reason }` once decided:
`board` (engine win), `draw`, `resign`, or `no_reveal_forfeit`. Leaving a
started hosted game in the client signs `game.resign`, so abandoning through
the UI is a recorded loss. A creator withdrawing an unjoined game has no
result. Silent abandonment (a seat that just stops) needs trusted time; the
plan is a host policy that commits an operator-signed forfeit after a
timeout, not built.

**Client:** the web/desktop client derives `salt = SHA3("apa-kan/deal-salt/v1"
‖ device seed ‖ stream)`, so it can reveal after a reload with nothing extra
stored, and reveals automatically when it sees `revealing` and the commitment
matches its own salt. Any 32 random bytes are valid; the derivation is a
client convenience, not a protocol rule.

Loading a game = read the stream from the ledger, replay. The server keeps an
in-memory cache of folded streams, one lock per stream, refreshed whenever the
ledger head differs (another server instance or a relay committed first).

## Views

| Route | View |
|---|---|
| `GET /v1/ledger` | `{ domain, chain, backend, operator }` — what a client needs to sign |
| `GET /v1/games/{id}` | `HostedGameView`: phase (`open`, `revealing`, `playing`, `finished`, `cancelled`), `ledger` head (`chain`, `stream`, `seq`, `head`), seat claims (account, label, optional `preview_pk`), `seed_commit`, `cancel_reason`, `result` (winner, loser, reason) once decided, and the `GameView` (status, board, legal actions, replayable record) once the deal is revealed |
| `GET /v1/games/{id}/txs?from=n` | committed signed transactions from `n` (audit, resync, peers) |

| `GET /v1/accounts/{id}` | `AccountView` (keys with status, label, `email_set`, recovery switches and stage) + `tiles`, `elo_milli` from the store projection |
| `GET /v1/accounts/{id}/keys` | the account's keys |
| `GET /v1/admin/roles` | `RolesView`: account → roles |

The only write route is `POST /v1/tx`; it routes by stream prefix (`game/`,
`account/`, `admin/`) and replies `{ receipt, view }` with that stream's view.

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

### UDP previews (plan, not built)

An ML-DSA envelope doesn't fit a datagram, and the point of UDP is an early
read, not authority. So:

- **Preview datagram** (unsigned but encrypted, compact binary): `game id`, `seq`, `prev`
  (first bytes), the encoded action, and `tx_hash` — the id of the full
  signed transaction the sender is about to submit (or has submitted) over
  HTTP. Tens of bytes.
- **Encryption (X25519 + ChaCha20-Poly1305; helpers in
  `apa_kan_tx::preview`, transport not built):** each seat puts an ephemeral
  X25519 public key in its signed payload (`game.create.preview_pk`,
  `game.join.preview_pk`, optional, 64 hex), so the key exchange is
  authenticated by ML-DSA and costs no extra round trip. Session keys are
  `HKDF-SHA256(ikm = X25519 shared secret, salt = SHA-256("apa-kan/preview/v1"
  ‖ create_tx_id ‖ join_tx_id))`, one per sending seat (`info = domain ‖
  "/seat" ‖ seat`) so directions never share a nonce. A packet is a 96-bit
  big-endian counter (the nonce) ‖ ChaCha20-Poly1305 ciphertext ‖ 16-byte tag
  (28 bytes overhead), with the stream id as AAD. Receivers keep the highest
  counter accepted and drop anything at or below it (stale, replayed or
  reordered previews are simply lost; they are disposable). Clients derive the
  per-game secret from the device seed (`preview_secret`), like the deal salt.
  This replaces the earlier idea of a code derived from the public tx ids.
  **Future:** hybrid X25519 + ML-KEM-768 if previews need post-quantum
  confidentiality (the payload field grows; the rest is unchanged).
- **Authority:** the receiver applies the preview tentatively (animate the
  move, start thinking) and marks it provisional. The signed transaction —
  via `POST /v1/tx` and the view/`/txs` poll, or relayed peer-to-peer — is
  authoritative. When it arrives, its id must equal the preview's `tx_hash`;
  on mismatch, or if no matching signed tx shows up within a short window,
  the client rolls back to the last committed state and replays.
- Previews never touch the ledger, never move `seq`, and are never relayed.
  A bad preview costs one rollback, nothing more.

## Backends

| URL (`APAKAN_LEDGER_URL`) | Use | Durability |
|---|---|---|
| `memory` | tests | none |
| `sqlite:<path>` | single node (default; falls back to `APA_KAN_DATABASE_URL`, then `$TMP/apa-kan-ledger.db`) | WAL + `synchronous=FULL` |
| `postgres://…` | multi-instance, platform DB (#97); build with `--features postgres` | Postgres commit |

Migrations: `migrations/{sqlite,postgres}/001_ledger.sql`, idempotent, run at
connect.
