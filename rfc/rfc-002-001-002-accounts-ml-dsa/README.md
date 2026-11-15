# RFC 002-001-002: Accounts, ML-DSA keys, linking, and recovery

**Milestone:** M1 (local accounts), M9 (email recovery)  
**Status:** accepted for platform bring-up (implementation PR follows this RFC)  
**Tracks:** [#49](https://github.com/ramate-io/apa-kan/issues/49), [#58](https://github.com/ramate-io/apa-kan/issues/58)

## Problem

Apa Kan needs stable player identities that hold tiles and Elo, work offline with local keys, and can authorize online actions without Bevy or game rendering. Keys must rotate without changing identity. Recovery must be optional, slow, and cancellable.

## Proposed solution

### Identity and crypto

- **Parameter set:** **`MlDsa65`** (`AccountMlDsaParams` in `apa-kan/account/src/param.rs`). **Decision #58-Q20:** intentionally **not** gwrdfa’s MlDsa44.
- **Crate pin:** `ml-dsa = "0.1.0-rc.7"`. **OPEN-Q #58-19:** version may joint-bump with gwrdfa later (`OPEN-Q` in workspace `Cargo.toml`).
- **Hash:** SHA3-256 (`sha3 = "0.10.8"`).
- **Signing:** domain-separated canonical JSON → message id `SHA3-256(payload ‖ signer_pubkey_bytes)` → `Signer::sign(id)`.
- **Account id:** `SHA3-256(b"apa-kan/account-id/v1" ‖ genesis_verifying_key_encode)` — **Decision #58-Q9:** lowercase **raw hex** in URLs and APIs (no `akan1` prefix).

An account is not a pubkey; authorized keys are rows keyed by `account_id`.

### Key storage (client)

**Decision #58-Q10:**

| Platform | Storage |
|---|---|
| Native (desktop / mobile) | OS keychain / Android Keystore / iOS Keychain (`KeyStore` impl per shell) |
| Browser (Dioxus WASM, #58-Q11 context) | Passphrase-wrapped seed blob in IndexedDB; WebCrypto encryption via `PassphraseKeystoreBackend` |

`apa-kan-account::KeyStore` + `MemoryKeyStore` (tests). Browser backend is **not** implemented in M1; traits and `WrappedSeedBlob` are stable for the shell.

### Key linking

1. **Link request** — new key signs `apa-kan/link-request/v1` (proves possession).
2. **Link approval** — active key signs `apa-kan/link-approval/v1` with monotonic `next_nonce` (CAS on store).
3. Server or local store verifies signatures, expiry, and nonce before inserting `AuthorizedKey` as `active`.

### Email recovery and link-by-email (opt-in)

- Double opt-in email while an active key is available.
- Recovery inserts `pending_recovery` with `usable_at = now + 72h`.
- Email approval of an existing link request uses a shorter cooldown when already documented in [#49](https://github.com/ramate-io/apa-kan/issues/49).
- **Decision #58-Q8:** v1 uses **72h cooldown only** (no email 2FA / passkey).

### Signed game actions (join point with core)

`apa-kan-action/v1` envelopes wrap core actions; validation is account signature + core replay. Out of scope for the first implementation PR except type stubs.

### Ratings context (#58-Q1 / Q2)

- **One Elo ladder** per account in storage for now (`ratings.elo_milli`).
- **Decision #58-Q2:** wagered games **do** write Elo when ranked (enforced in `apa-kan-rating` later).
- Concurrent **human vs bot divisions** per mode will use `rating_division_slots` (see store RFC); not populated in M1.

## Implementation

Crate: `apa-kan/account` (no tokio, no sqlx, no Bevy).

| Module | Responsibility |
|---|---|
| `param` | `AccountMlDsaParams` (= `MlDsa65`) |
| `id` | `AccountId`, genesis derivation, raw hex |
| `crypto` | Seed, vk bytes, sign/verify, message id (`Param` alias) |
| `keystore` | `KeyStore`, passphrase-wrap traits, `MemoryKeyStore` |
| `canonical` | Stable JSON for hashing |
| `link` | Request, approval, verify |
| `recovery` | Pending recovery, cooldown constants |

The server never sees seeds.

## Alternatives considered

- **MlDsa44 (gwrdfa):** rejected per **Q20** (stay on 65 unless both repos align later).
- **Email as standing signer:** rejected; ML-DSA only for game/economy actions.
- **Account id = pubkey:** rejected; Aptos-style stable address.

## Open questions

| # | Topic | Status |
|---|---|---|
| 19 | `ml-dsa` crate version | **OPEN-Q** — pin `0.1.0-rc.7` until joint bump |