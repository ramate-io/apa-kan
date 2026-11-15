# RFC 002-001-002: Accounts, ML-DSA keys, linking, and recovery

**Milestone:** M1 (local accounts), M9 (email recovery)  
**Status:** accepted for platform bring-up (implementation PR follows this RFC)  
**Tracks:** [#49](https://github.com/ramate-io/apa-kan/issues/49), [#58](https://github.com/ramate-io/apa-kan/issues/58)

## Problem

Apa Kan needs stable player identities that hold tiles and Elo, work offline with local keys, and can authorize online actions without Bevy or game rendering. Keys must rotate without changing identity. Recovery must be optional, slow, and cancellable.

## Proposed solution

### Identity and crypto

- **Parameter set:** `MlDsa44`, matching [`ramate-io/gwrdfa`](https://github.com/ramate-io/gwrdfa).
- **Crate pin:** `ml-dsa = "0.1.0-rc.7"` (joint bump with gwrdfa later).
- **Hash:** SHA3-256 (`sha3 = "0.10.8"`).
- **Signing:** domain-separated canonical JSON → message id `SHA3-256(payload ‖ signer_pubkey_bytes)` → `Signer::sign(id)`.
- **Account id:** `SHA3-256(b"apa-kan/account-id/v1" ‖ genesis_verifying_key_encode)` — 32 bytes, hex in APIs (human `akan1` prefix is **open**; see below).

An account is not a pubkey; authorized keys are rows keyed by `account_id`.

### Key linking

1. **Link request** — new key signs `apa-kan/link-request/v1` (proves possession).
2. **Link approval** — active key signs `apa-kan/link-approval/v1` with monotonic `next_nonce` (CAS on store).
3. Server or local store verifies signatures, expiry, and nonce before inserting `AuthorizedKey` as `active`.

### Email recovery and link-by-email (opt-in)

- Double opt-in email while an active key is available.
- Recovery inserts `pending_recovery` with `usable_at = now + 72h`.
- Email approval of an existing link request uses the same pending row with **24h** cooldown (shorter because a human already started the link).
- Any active key may **cancel** during cooldown.
- **OPEN-Q #8:** v1 uses cooldown only (no email 2FA / passkey).

### Signed game actions (join point with core)

`apa-kan-action/v1` envelopes wrap core actions; validation is account signature + core replay. Out of scope for the first implementation PR except type stubs.

## Implementation

Crate: `apa-kan/account` (no tokio, no sqlx, no Bevy).

| Module | Responsibility |
|---|---|
| `id` | `AccountId`, genesis derivation |
| `crypto` | Seed, vk bytes, sign/verify, message id |
| `canonical` | Stable JSON for hashing |
| `link` | Request, approval, verify |
| `recovery` | Pending recovery, cooldown constants |
| `envelope` | Domain strings and shared hashing |

Platform shells persist seeds (**OPEN-Q #10** keystore); the server never sees seeds.

## Alternatives considered

- **MlDsa65:** larger signatures; rejected for v1 to match gwrdfa.
- **Email as standing signer:** rejected; ML-DSA only for game/economy actions.
- **Account id = pubkey:** rejected; Aptos-style stable address.

## Open questions

| # | Topic | This RFC |
|---|---|---|
| 8 | Email 2FA / passkey | Cooldown only (`OPEN-Q #8` in recovery paths) |
| 9 | `akan1` prefix | Hex account id in APIs; display helper TBD (`OPEN-Q #9`) |
| 10 | Keystore | Client-only; not in `apa-kan-account` (`OPEN-Q #10`) |
| 19 | `ml-dsa` version | `0.1.0-rc.7` per Decision |
| 20 | 44 vs 65 | `MlDsa44` per Decision |