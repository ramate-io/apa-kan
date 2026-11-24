//! State machine for `account/<id>` streams: the account's keys, recovery
//! email, and email recovery, all as signed transactions.
//!
//! ```text
//! account.register (seq 0, genesis key)
//!   account.link_key      active key; the new key co-signs a proof
//!   account.revoke_key    active key; never the last active key
//!   account.set_email     active key
//!   account.recovery_request    pending key (not on the account), proves the email
//!   account.recovery_challenge  operator: token hash + cooldown end; token mailed
//!   account.recovery_confirm    pending key shows the token → key pending_recovery
//!   account.recovery_finish     pending key, after the cooldown → key active
//!   account.recovery_cancel     any active key, any time before finish
//! ```
//!
//! The fold is the authority on an account's keys: game hosts look keys up
//! here. [`AccountState::apply`] checks the signer key itself (it is the key
//! directory), so hosts only run [`crate::verify_self_contained`] first.
//! Wall-clock rules (the recovery cooldown) take `now` from the host at
//! admission; replay passes `None` and trusts what was committed.

use apa_kan_account::crypto::{verify_bytes, VerifyingKeyBytes};
use apa_kan_account::{AccountId, AuthorizedKeyStatus, RecoveryKind};
use apa_kan_view::{
	AccountKeyView, AccountView, KeyState, LedgerHead, RecoveryStage, RecoveryView, VIEW_SCHEMA,
};
use sha3::{Digest, Sha3_256};

use crate::envelope::{link_proof_id, verify_self_contained, SignedTx, TxPayload, GENESIS_PREV};
use crate::{TxError, TxReceipt};

/// Key / account label cap.
pub const MAX_KEY_LABEL_LEN: usize = 40;
/// Longest accepted email address.
pub const MAX_EMAIL_LEN: usize = 254;
/// Most keys (any status) an account keeps.
pub const MAX_KEYS: usize = 64;
/// Domain of `account.recovery_request`'s email proof.
pub const EMAIL_PROOF_DOMAIN: &[u8] = b"apa-kan/recovery-email/v1";
/// Domain of the recovery token hash.
pub const RECOVERY_TOKEN_DOMAIN: &[u8] = b"apa-kan/recovery-token/v1";
/// How far a signed `requested_at_unix` / `at_unix` may be from the host
/// clock at admission.
pub const MAX_CLOCK_SKEW_SECS: i64 = 600;
/// Default [`AccountRules::recovery_retry_cooldown_secs`]: 24 h.
pub const DEFAULT_RECOVERY_RETRY_COOLDOWN_SECS: i64 = 24 * 3600;
/// Default [`AccountRules::recovery_challenge_timeout_secs`]: 15 min.
pub const DEFAULT_RECOVERY_CHALLENGE_TIMEOUT_SECS: i64 = 15 * 60;

/// `account/<hex id>`.
pub fn account_stream(id: &AccountId) -> String {
	format!("account/{}", id.to_hex())
}

/// The account a stream belongs to, if it is a well-formed `account/<hex>`.
pub fn account_id_of(stream: &str) -> Option<AccountId> {
	let hex_id = stream.strip_prefix("account/")?;
	if hex_id.len() != 64 || !hex_id.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f')) {
		return None;
	}
	AccountId::from_hex(hex_id).ok()
}

fn sha3_hex(parts: &[&[u8]]) -> String {
	let mut h = Sha3_256::new();
	for p in parts {
		h.update(p);
	}
	hex::encode(h.finalize())
}

/// Proof that the requester of a recovery knows the email on file, bound to
/// the account and the key being recovered (so it can't be replayed for
/// another key). Emails compare trimmed and lowercased.
pub fn email_proof(account: &AccountId, pending_vk: &VerifyingKeyBytes, email: &str) -> String {
	let email = normalize_email(email);
	sha3_hex(&[EMAIL_PROOF_DOMAIN, account.as_bytes(), pending_vk.as_slice(), email.as_bytes()])
}

/// Hash of a mailed recovery token, as recorded by the operator.
pub fn recovery_token_hash(token: &str) -> String {
	sha3_hex(&[RECOVERY_TOKEN_DOMAIN, token.as_bytes()])
}

fn normalize_email(email: &str) -> String {
	email.trim().to_lowercase()
}

/// Chain-wide rules every host and peer folds account streams with.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AccountRules {
	/// Operator account (self-certifying genesis key) allowed to sign
	/// `account.recovery_challenge`. `None`: email recovery is off.
	pub operator: Option<AccountId>,
	/// After a cancelled or failed (expired) recovery, other keys wait this
	/// long before requesting again.
	pub recovery_retry_cooldown_secs: i64,
	/// A request the operator never challenged expires after this long; the
	/// same key may then retry at once, others after the cooldown.
	pub recovery_challenge_timeout_secs: i64,
}

impl Default for AccountRules {
	fn default() -> Self {
		Self {
			operator: None,
			recovery_retry_cooldown_secs: DEFAULT_RECOVERY_RETRY_COOLDOWN_SECS,
			recovery_challenge_timeout_secs: DEFAULT_RECOVERY_CHALLENGE_TIMEOUT_SECS,
		}
	}
}

impl AccountRules {
	/// Rules with `operator` and default timings.
	pub fn with_operator(operator: Option<AccountId>) -> Self {
		Self { operator, ..Self::default() }
	}
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KeyEntry {
	pub vk: VerifyingKeyBytes,
	pub label: String,
	pub status: AuthorizedKeyStatus,
	pub added_seq: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Stage {
	Requested,
	Challenged { token_hash: String, usable_at_unix: i64 },
	Confirmed { usable_at_unix: i64 },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Recovery {
	pub pending_vk: VerifyingKeyBytes,
	pub kind: RecoveryKind,
	pub stage: Stage,
	/// Signed `requested_at_unix` of the request.
	pub requested_at_unix: i64,
}

impl Recovery {
	/// When this recovery counts as failed: an unchallenged request after
	/// the challenge timeout, a challenged one whose token was not confirmed
	/// by the end of its waiting period. Confirmed recoveries don't expire.
	pub fn expires_at(&self, rules: &AccountRules) -> Option<i64> {
		match self.stage {
			Stage::Requested => {
				Some(self.requested_at_unix.saturating_add(rules.recovery_challenge_timeout_secs))
			}
			Stage::Challenged { usable_at_unix, .. } => Some(usable_at_unix),
			Stage::Confirmed { .. } => None,
		}
	}
}

fn check_skew(now: Option<i64>, at: i64) -> Result<(), TxError> {
	match now {
		Some(now) if (now - at).abs() > MAX_CLOCK_SKEW_SECS => Err(reject(format!(
			"signed time {at} is more than {MAX_CLOCK_SKEW_SECS}s from the host clock"
		))),
		_ => Ok(()),
	}
}

/// Fold of one `account/<id>` stream.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AccountState {
	pub id: AccountId,
	pub chain: String,
	pub label: String,
	pub genesis_vk: VerifyingKeyBytes,
	pub keys: Vec<KeyEntry>,
	/// On file for recovery mail; never in the view.
	pub email: Option<String>,
	pub recovery_opt_in: bool,
	pub approve_via_email: bool,
	pub recovery: Option<Recovery>,
	/// New recovery requests are refused before this (signed) time.
	pub recovery_cooldown_until: i64,
	pub next_seq: u64,
	pub head: String,
}

fn reject(reason: impl Into<String>) -> TxError {
	TxError::Rejected { reason: reason.into() }
}

fn clean_label(label: &str, fallback: &str) -> String {
	let l: String = label
		.trim()
		.chars()
		.filter(|c| !c.is_control())
		.take(MAX_KEY_LABEL_LEN)
		.collect();
	if l.is_empty() {
		fallback.into()
	} else {
		l
	}
}

fn kind_name(kind: RecoveryKind) -> &'static str {
	match kind {
		RecoveryKind::FullRecovery => "full_recovery",
		RecoveryKind::EmailLinkApproval => "email_link_approval",
	}
}

impl AccountState {
	/// Status of `vk` on this account (`None` if it was never added).
	pub fn key_status(&self, vk: &VerifyingKeyBytes) -> Option<AuthorizedKeyStatus> {
		self.keys.iter().find(|k| &k.vk == vk).map(|k| k.status)
	}

	fn active_count(&self) -> usize {
		self.keys.iter().filter(|k| k.status == AuthorizedKeyStatus::Active).count()
	}

	fn require_active(&self, vk: &VerifyingKeyBytes) -> Result<(), TxError> {
		match self.key_status(vk) {
			Some(AuthorizedKeyStatus::Active) => Ok(()),
			_ => Err(TxError::KeyNotAuthorized),
		}
	}

	fn require_pending_signer(&self, vk: &VerifyingKeyBytes) -> Result<&Recovery, TxError> {
		let r = self.recovery.as_ref().ok_or_else(|| reject("no recovery in progress"))?;
		if &r.pending_vk != vk {
			return Err(TxError::KeyNotAuthorized);
		}
		Ok(r)
	}

	/// Apply `tx` (signature already verified, id `tx_id`) to `state` (`None`
	/// for an empty stream). `now` is the host's clock at admission, `None`
	/// on replay. On error nothing changes.
	pub fn apply(
		rules: &AccountRules,
		state: Option<&AccountState>,
		tx: &SignedTx,
		tx_id: &[u8; 32],
		now: Option<i64>,
	) -> Result<(AccountState, TxReceipt), TxError> {
		let b = &tx.body;
		let id = account_id_of(&b.stream)
			.ok_or_else(|| TxError::UnsupportedStream { stream: b.stream.clone() })?;
		let vk = &b.signer_vk;
		let mut next = match state {
			None => {
				if b.seq != 0 {
					return Err(TxError::NotFound);
				}
				if b.prev != GENESIS_PREV {
					return Err(TxError::PrevMismatch { expected: GENESIS_PREV.into() });
				}
				let TxPayload::AccountRegister { label } = &b.payload else {
					return Err(TxError::NotFound);
				};
				// Only the genesis key can open its account's stream.
				if AccountId::from_genesis_vk(vk) != id || b.signer != id {
					return Err(TxError::KeyNotAuthorized);
				}
				AccountState {
					id,
					chain: b.chain.clone(),
					label: clean_label(label, ""),
					genesis_vk: vk.clone(),
					keys: vec![KeyEntry {
						vk: vk.clone(),
						label: "genesis".into(),
						status: AuthorizedKeyStatus::Active,
						added_seq: 0,
					}],
					email: None,
					recovery_opt_in: false,
					approve_via_email: false,
					recovery: None,
					recovery_cooldown_until: 0,
					next_seq: 0,
					head: GENESIS_PREV.into(),
				}
			}
			Some(s) => {
				if b.chain != s.chain {
					return Err(TxError::WrongChain {
						expected: s.chain.clone(),
						got: b.chain.clone(),
					});
				}
				if b.seq != s.next_seq {
					return Err(TxError::StaleSeq { current: s.next_seq, got: b.seq });
				}
				if b.prev != s.head {
					return Err(TxError::PrevMismatch { expected: s.head.clone() });
				}
				// Every account tx except the operator's names this account.
				let operator_tx = matches!(b.payload, TxPayload::AccountRecoveryChallenge { .. });
				if !operator_tx && b.signer != id {
					return Err(TxError::KeyNotAuthorized);
				}
				let mut n = s.clone();
				match &b.payload {
					TxPayload::AccountRegister { .. } => return Err(reject("already registered")),
					TxPayload::AccountLinkKey { vk: new_vk, label, proof } => {
						s.require_active(vk)?;
						if s.key_status(new_vk).is_some() {
							return Err(reject("key was already added to this account"));
						}
						if s.recovery.as_ref().is_some_and(|r| &r.pending_vk == new_vk) {
							return Err(reject("key is being recovered; finish or cancel first"));
						}
						if s.keys.len() >= MAX_KEYS {
							return Err(reject("too many keys"));
						}
						new_vk.decode().map_err(|_| TxError::BadEnvelope {
							reason: "vk is not an ML-DSA-65 key".into(),
						})?;
						verify_bytes(new_vk, &link_proof_id(&b.chain, &id, new_vk), proof)
							.map_err(|_| TxError::BadSignature)?;
						n.keys.push(KeyEntry {
							vk: new_vk.clone(),
							label: clean_label(label, "device"),
							status: AuthorizedKeyStatus::Active,
							added_seq: b.seq,
						});
					}
					TxPayload::AccountRevokeKey { vk: target } => {
						s.require_active(vk)?;
						let status =
							s.key_status(target).ok_or_else(|| reject("key not on account"))?;
						match status {
							AuthorizedKeyStatus::Revoked => {
								return Err(reject("key already revoked"))
							}
							AuthorizedKeyStatus::Active if s.active_count() <= 1 => {
								return Err(reject("can't revoke the last active key"))
							}
							_ => {}
						}
						if let Some(k) = n.keys.iter_mut().find(|k| &k.vk == target) {
							k.status = AuthorizedKeyStatus::Revoked;
						}
						if n.recovery.as_ref().is_some_and(|r| &r.pending_vk == target) {
							n.recovery = None;
						}
					}
					TxPayload::AccountSetEmail { email, recovery_opt_in, approve_via_email } => {
						s.require_active(vk)?;
						let email = email.as_deref().map(normalize_email).filter(|e| !e.is_empty());
						if let Some(e) = &email {
							let ok = e.len() <= MAX_EMAIL_LEN
								&& e.split_once('@')
									.is_some_and(|(l, d)| !l.is_empty() && !d.is_empty())
								&& !e.chars().any(|c| c.is_whitespace() || c.is_control());
							if !ok {
								return Err(reject("not an email address"));
							}
						}
						if email.is_none() && (*recovery_opt_in || *approve_via_email) {
							return Err(reject("recovery switches need an email"));
						}
						n.email = email;
						n.recovery_opt_in = *recovery_opt_in;
						n.approve_via_email = *approve_via_email;
					}
					TxPayload::AccountRecoveryRequest {
						kind,
						email_proof: proof,
						requested_at_unix: at,
					} => {
						if rules.operator.is_none() {
							return Err(reject("email recovery is not enabled on this chain"));
						}
						check_skew(now, *at)?;
						// One open recovery per account. A failed one (expired)
						// may be replaced: by the same key at once (retry), by
						// another key after the cooldown.
						if let Some(r) = &s.recovery {
							match r.expires_at(rules) {
								Some(exp) if *at >= exp => {
									let until =
										exp.saturating_add(rules.recovery_retry_cooldown_secs);
									if &r.pending_vk != vk && *at < until {
										return Err(TxError::TooEarly { usable_at_unix: until });
									}
								}
								_ => return Err(reject("a recovery is already in progress")),
							}
						} else if *at < s.recovery_cooldown_until {
							return Err(TxError::TooEarly {
								usable_at_unix: s.recovery_cooldown_until,
							});
						}
						if s.key_status(vk).is_some() {
							return Err(reject("key was already added to this account"));
						}
						let allowed = match kind {
							RecoveryKind::FullRecovery => s.recovery_opt_in,
							RecoveryKind::EmailLinkApproval => s.approve_via_email,
						};
						let email = s.email.as_deref().filter(|_| allowed);
						let Some(email) = email else {
							return Err(reject("account has not opted in to this recovery"));
						};
						if *proof != email_proof(&id, vk, email) {
							return Err(TxError::Unauthorized);
						}
						n.recovery = Some(Recovery {
							pending_vk: vk.clone(),
							kind: *kind,
							stage: Stage::Requested,
							requested_at_unix: *at,
						});
					}
					TxPayload::AccountRecoveryChallenge { token_hash, usable_at_unix } => {
						let op = rules.operator.ok_or(TxError::KeyNotAuthorized)?;
						if b.signer != op || AccountId::from_genesis_vk(vk) != op {
							return Err(TxError::KeyNotAuthorized);
						}
						let r =
							n.recovery.as_mut().ok_or_else(|| reject("no recovery in progress"))?;
						if r.stage != Stage::Requested {
							return Err(reject("recovery already challenged"));
						}
						if token_hash.len() != 64 {
							return Err(TxError::BadEnvelope {
								reason: "token_hash must be hex64".into(),
							});
						}
						r.stage = Stage::Challenged {
							token_hash: token_hash.clone(),
							usable_at_unix: *usable_at_unix,
						};
					}
					TxPayload::AccountRecoveryConfirm { token } => {
						let r = s.require_pending_signer(vk)?;
						let Stage::Challenged { token_hash, usable_at_unix } = &r.stage else {
							return Err(reject("recovery is not waiting for a token"));
						};
						if recovery_token_hash(token) != *token_hash {
							return Err(TxError::Unauthorized);
						}
						let usable_at_unix = *usable_at_unix;
						if s.keys.len() >= MAX_KEYS {
							return Err(reject("too many keys"));
						}
						n.keys.push(KeyEntry {
							vk: vk.clone(),
							label: "recovery".into(),
							status: AuthorizedKeyStatus::PendingRecovery,
							added_seq: b.seq,
						});
						if let Some(r) = n.recovery.as_mut() {
							r.stage = Stage::Confirmed { usable_at_unix };
						}
					}
					TxPayload::AccountRecoveryFinish => {
						let r = s.require_pending_signer(vk)?;
						let Stage::Confirmed { usable_at_unix } = r.stage else {
							return Err(reject("recovery token not confirmed yet"));
						};
						if now.is_some_and(|t| t < usable_at_unix) {
							return Err(TxError::TooEarly { usable_at_unix });
						}
						let full = r.kind == RecoveryKind::FullRecovery;
						for k in &mut n.keys {
							if &k.vk == vk {
								k.status = AuthorizedKeyStatus::Active;
							} else if full && k.status == AuthorizedKeyStatus::Active {
								// Full recovery means the old devices are lost or
								// compromised: only the recovered key stays.
								k.status = AuthorizedKeyStatus::Revoked;
							}
						}
						n.recovery = None;
					}
					TxPayload::AccountRecoveryCancel { at_unix } => {
						s.require_active(vk)?;
						check_skew(now, *at_unix)?;
						n.recovery_cooldown_until = n
							.recovery_cooldown_until
							.max(at_unix.saturating_add(rules.recovery_retry_cooldown_secs));
						let r =
							s.recovery.as_ref().ok_or_else(|| reject("no recovery in progress"))?;
						let pending = r.pending_vk.clone();
						if let Some(k) = n.keys.iter_mut().find(|k| k.vk == pending) {
							k.status = AuthorizedKeyStatus::Revoked;
						}
						n.recovery = None;
					}
					other => {
						return Err(TxError::BadEnvelope {
							reason: format!("{} is not an account transaction", other.kind()),
						})
					}
				}
				n
			}
		};
		let seq = next.next_seq;
		next.next_seq += 1;
		next.head = hex::encode(tx_id);
		let receipt = TxReceipt {
			tx_id: next.head.clone(),
			stream: b.stream.clone(),
			seq,
			kind: b.payload.kind().into(),
			phase: None,
			log_line: None,
		};
		Ok((next, receipt))
	}

	pub fn view(&self) -> AccountView {
		AccountView {
			v: VIEW_SCHEMA,
			id: self.id.to_hex(),
			ledger: LedgerHead {
				chain: self.chain.clone(),
				stream: account_stream(&self.id),
				seq: self.next_seq,
				head: self.head.clone(),
			},
			label: self.label.clone(),
			keys: self
				.keys
				.iter()
				.map(|k| AccountKeyView {
					vk: hex::encode(k.vk.as_slice()),
					label: k.label.clone(),
					status: match k.status {
						AuthorizedKeyStatus::Active => KeyState::Active,
						AuthorizedKeyStatus::Revoked => KeyState::Revoked,
						AuthorizedKeyStatus::PendingRecovery => KeyState::PendingRecovery,
					},
					added_seq: k.added_seq,
				})
				.collect(),
			email_set: self.email.is_some(),
			recovery_opt_in: self.recovery_opt_in,
			approve_via_email: self.approve_via_email,
			recovery: self.recovery.as_ref().map(|r| RecoveryView {
				pending_vk: hex::encode(r.pending_vk.as_slice()),
				kind: kind_name(r.kind).into(),
				stage: match r.stage {
					Stage::Requested => RecoveryStage::Requested,
					Stage::Challenged { .. } => RecoveryStage::Challenged,
					Stage::Confirmed { .. } => RecoveryStage::Confirmed,
				},
				usable_at_unix: match r.stage {
					Stage::Requested => None,
					Stage::Challenged { usable_at_unix, .. }
					| Stage::Confirmed { usable_at_unix } => Some(usable_at_unix),
				},
			}),
		}
	}
}

/// Fold committed account transactions, re-verifying every signature.
pub fn replay(
	rules: &AccountRules,
	chain: &str,
	txs: &[SignedTx],
) -> Result<Option<AccountState>, TxError> {
	let mut state: Option<AccountState> = None;
	for tx in txs {
		let id = verify_self_contained(tx, chain)?;
		let (next, _) = AccountState::apply(rules, state.as_ref(), tx, &id, None)?;
		state = Some(next);
	}
	Ok(state)
}

#[cfg(test)]
mod tests {
	#![allow(clippy::unwrap_used, clippy::expect_used)]

	use super::*;
	use crate::envelope::TxSigner;
	use apa_kan_account::Seed;

	const CHAIN: &str = "test";

	fn key(n: u8) -> Seed {
		Seed::from_bytes([n; 32])
	}

	struct Acct {
		rules: AccountRules,
		owner: TxSigner,
		state: Option<AccountState>,
		log: Vec<SignedTx>,
		now: i64,
	}

	impl Acct {
		fn new() -> Self {
			let op = TxSigner::from_seed(&key(200));
			Self {
				rules: AccountRules::with_operator(Some(op.account())),
				owner: TxSigner::from_seed(&key(1)),
				state: None,
				log: Vec::new(),
				now: 1_000,
			}
		}

		fn stream(&self) -> String {
			account_stream(&self.owner.account())
		}

		fn send(&mut self, who: &TxSigner, payload: TxPayload) -> Result<TxReceipt, TxError> {
			let (seq, prev) = match &self.state {
				Some(s) => (s.next_seq, s.head.clone()),
				None => (0, GENESIS_PREV.to_string()),
			};
			let tx = who.sign(CHAIN, &self.stream(), seq, &prev, payload).unwrap();
			let id = verify_self_contained(&tx, CHAIN)?;
			let (next, r) =
				AccountState::apply(&self.rules, self.state.as_ref(), &tx, &id, Some(self.now))?;
			self.state = Some(next);
			self.log.push(tx);
			Ok(r)
		}

		fn s(&self) -> &AccountState {
			self.state.as_ref().unwrap()
		}

		fn linked(&self, n: u8) -> TxSigner {
			TxSigner::for_account(&key(n), self.owner.account())
		}
	}

	#[test]
	fn register_link_revoke() {
		let mut a = Acct::new();
		let owner = TxSigner::from_seed(&key(1));
		// Only the genesis key opens the stream.
		let stranger = a.linked(9);
		assert_eq!(
			a.send(&stranger, TxPayload::AccountRegister { label: "x".into() }),
			Err(TxError::KeyNotAuthorized)
		);
		a.send(&owner, TxPayload::AccountRegister { label: "Ana".into() }).unwrap();
		assert_eq!(
			a.send(&owner, TxPayload::AccountRegister { label: "x".into() }).unwrap_err(),
			reject("already registered")
		);

		let phone = a.linked(2);
		let proof = phone.link_proof(CHAIN, &owner.account());
		// The proof must come from the key being linked.
		let wrong = a.linked(3).link_proof(CHAIN, &owner.account());
		let link = |proof| TxPayload::AccountLinkKey {
			vk: phone.verifying_key().clone(),
			label: "phone".into(),
			proof,
		};
		assert_eq!(a.send(&owner, link(wrong)), Err(TxError::BadSignature));
		// A key not on the account can't link anything.
		assert_eq!(a.send(&phone, link(proof.clone())), Err(TxError::KeyNotAuthorized));
		a.send(&owner, link(proof.clone())).unwrap();
		assert_eq!(a.s().key_status(phone.verifying_key()), Some(AuthorizedKeyStatus::Active));
		assert!(a.send(&owner, link(proof)).is_err());

		// The linked key acts for the account; it revokes the genesis key.
		a.send(&phone, TxPayload::AccountRevokeKey { vk: owner.verifying_key().clone() })
			.unwrap();
		assert_eq!(
			a.send(&owner, TxPayload::AccountRevokeKey { vk: phone.verifying_key().clone() }),
			Err(TxError::KeyNotAuthorized)
		);
		assert_eq!(
			a.send(&phone, TxPayload::AccountRevokeKey { vk: phone.verifying_key().clone() })
				.unwrap_err(),
			reject("can't revoke the last active key")
		);
		let v = a.s().view();
		assert_eq!(v.keys.len(), 2);
		assert_eq!(v.keys[0].status, KeyState::Revoked);
		assert_eq!(v.label, "Ana");

		let rebuilt = replay(&a.rules, CHAIN, &a.log).unwrap().unwrap();
		assert_eq!(&rebuilt, a.s());
	}

	#[test]
	fn email_recovery_with_operator_challenge_and_cooldown() {
		let mut a = Acct::new();
		let owner = TxSigner::from_seed(&key(1));
		let op = TxSigner::from_seed(&key(200));
		a.send(&owner, TxPayload::AccountRegister { label: String::new() }).unwrap();
		let lost = a.linked(5);
		let req = |a: &Acct, email: &str| TxPayload::AccountRecoveryRequest {
			kind: RecoveryKind::FullRecovery,
			email_proof: email_proof(&a.owner.account(), lost.verifying_key(), email),
			requested_at_unix: a.now,
		};
		// Another device that full recovery will revoke.
		let phone = a.linked(2);
		let link = TxPayload::AccountLinkKey {
			vk: phone.verifying_key().clone(),
			label: "phone".into(),
			proof: phone.link_proof(CHAIN, &owner.account()),
		};
		a.send(&owner, link).unwrap();
		// No email on file yet.
		assert!(matches!(a.send(&lost, req(&a, "a@b.c")), Err(TxError::Rejected { .. })));
		assert!(a
			.send(
				&owner,
				TxPayload::AccountSetEmail {
					email: Some("not-an-email".into()),
					recovery_opt_in: true,
					approve_via_email: false
				}
			)
			.is_err());
		a.send(
			&owner,
			TxPayload::AccountSetEmail {
				email: Some(" Ana@Example.com ".into()),
				recovery_opt_in: true,
				approve_via_email: false,
			},
		)
		.unwrap();
		assert!(a.s().view().email_set);
		// Wrong email: refused. Case and whitespace don't matter.
		assert_eq!(a.send(&lost, req(&a, "eve@example.com")), Err(TxError::Unauthorized));
		// The signed request time must be near the host clock.
		let mut skewed = req(&a, "ana@example.com");
		if let TxPayload::AccountRecoveryRequest { requested_at_unix, .. } = &mut skewed {
			*requested_at_unix += MAX_CLOCK_SKEW_SECS + 1;
		}
		assert!(matches!(a.send(&lost, skewed), Err(TxError::Rejected { .. })));
		a.send(&lost, req(&a, "ana@example.com")).unwrap();
		assert_eq!(a.s().view().recovery.unwrap().stage, RecoveryStage::Requested);
		// Only the operator challenges; the pending key can't skip ahead.
		let challenge = TxPayload::AccountRecoveryChallenge {
			token_hash: recovery_token_hash("tok"),
			usable_at_unix: 1_000 + 72 * 3600,
		};
		assert_eq!(a.send(&owner, challenge.clone()), Err(TxError::KeyNotAuthorized));
		assert!(a
			.send(&lost, TxPayload::AccountRecoveryConfirm { token: "tok".into() })
			.is_err());
		a.send(&op, challenge).unwrap();
		assert_eq!(
			a.send(&lost, TxPayload::AccountRecoveryConfirm { token: "nope".into() }),
			Err(TxError::Unauthorized)
		);
		a.send(&lost, TxPayload::AccountRecoveryConfirm { token: "tok".into() })
			.unwrap();
		assert_eq!(
			a.s().key_status(lost.verifying_key()),
			Some(AuthorizedKeyStatus::PendingRecovery)
		);
		// Pending keys can't act yet; the cooldown holds.
		assert_eq!(
			a.send(&lost, TxPayload::AccountRecoveryFinish),
			Err(TxError::TooEarly { usable_at_unix: 1_000 + 72 * 3600 })
		);
		a.now += 72 * 3600;
		a.send(&lost, TxPayload::AccountRecoveryFinish).unwrap();
		assert_eq!(a.s().key_status(lost.verifying_key()), Some(AuthorizedKeyStatus::Active));
		// A finished full recovery revokes every other key.
		assert_eq!(a.s().key_status(owner.verifying_key()), Some(AuthorizedKeyStatus::Revoked));
		assert_eq!(a.s().key_status(phone.verifying_key()), Some(AuthorizedKeyStatus::Revoked));
		assert!(a.s().recovery.is_none());
		// Replay ignores the clock and reproduces the fold.
		assert_eq!(&replay(&a.rules, CHAIN, &a.log).unwrap().unwrap(), a.s());
		// Without the operator in the rules the challenge doesn't replay.
		assert_eq!(
			replay(&AccountRules::default(), CHAIN, &a.log).unwrap_err(),
			reject("email recovery is not enabled on this chain")
		);
	}

	#[test]
	fn active_key_cancels_recovery() {
		let mut a = Acct::new();
		let owner = TxSigner::from_seed(&key(1));
		a.send(&owner, TxPayload::AccountRegister { label: String::new() }).unwrap();
		a.send(
			&owner,
			TxPayload::AccountSetEmail {
				email: Some("a@b.co".into()),
				recovery_opt_in: false,
				approve_via_email: true,
			},
		)
		.unwrap();
		let thief = a.linked(6);
		let proof = email_proof(&owner.account(), thief.verifying_key(), "a@b.co");
		// Full recovery not opted in; email link approval is.
		assert!(a
			.send(
				&thief,
				TxPayload::AccountRecoveryRequest {
					kind: RecoveryKind::FullRecovery,
					email_proof: proof.clone(),
					requested_at_unix: a.now,
				}
			)
			.is_err());
		a.send(
			&thief,
			TxPayload::AccountRecoveryRequest {
				kind: RecoveryKind::EmailLinkApproval,
				email_proof: proof.clone(),
				requested_at_unix: a.now,
			},
		)
		.unwrap();
		let cancel = |a: &Acct| TxPayload::AccountRecoveryCancel { at_unix: a.now };
		assert_eq!(a.send(&thief, cancel(&a)), Err(TxError::KeyNotAuthorized));
		a.send(&owner, cancel(&a)).unwrap();
		assert!(a.s().view().recovery.is_none());

		// After a cancel, requests wait out the retry cooldown.
		let request = |a: &Acct, who: &TxSigner| TxPayload::AccountRecoveryRequest {
			kind: RecoveryKind::EmailLinkApproval,
			email_proof: email_proof(&owner.account(), who.verifying_key(), "a@b.co"),
			requested_at_unix: a.now,
		};
		let cooled = a.now + DEFAULT_RECOVERY_RETRY_COOLDOWN_SECS;
		assert_eq!(
			a.send(&thief, request(&a, &thief)),
			Err(TxError::TooEarly { usable_at_unix: cooled })
		);
		a.now = cooled;
		a.send(&thief, request(&a, &thief)).unwrap();
		// One open request per account, even unchallenged…
		let other_key = a.linked(8);
		a.now += 60;
		assert_eq!(
			a.send(&other_key, request(&a, &other_key)).unwrap_err(),
			reject("a recovery is already in progress")
		);
		// …until it expires unchallenged (a failure): the same key may retry at
		// once, another key only after the cooldown.
		let expired = cooled + DEFAULT_RECOVERY_CHALLENGE_TIMEOUT_SECS;
		a.now = expired;
		assert_eq!(
			a.send(&other_key, request(&a, &other_key)),
			Err(TxError::TooEarly {
				usable_at_unix: expired + DEFAULT_RECOVERY_RETRY_COOLDOWN_SECS
			})
		);
		a.send(&thief, request(&a, &thief)).unwrap();
		assert_eq!(a.s().recovery.as_ref().unwrap().requested_at_unix, expired);
		assert_eq!(&replay(&a.rules, CHAIN, &a.log).unwrap().unwrap(), a.s());
		// Account txs must name this account (a key signing for another
		// account id is refused).
		let other = TxSigner::from_seed(&key(7));
		assert_eq!(
			a.send(&other, TxPayload::AccountRecoveryCancel { at_unix: a.now }),
			Err(TxError::KeyNotAuthorized)
		);
	}

	#[test]
	fn stream_ids() {
		let id = TxSigner::from_seed(&key(1)).account();
		assert_eq!(account_id_of(&account_stream(&id)), Some(id));
		assert_eq!(account_id_of("account/xyz"), None);
		assert_eq!(account_id_of(&format!("account/{}", "A".repeat(64))), None);
	}
}
