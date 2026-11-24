//! State machine for the `admin/roles` stream: which accounts hold which
//! operator-granted roles. Signed by the chain's operator account or by an
//! account that holds the `admin` role; the host checks that the signing key
//! is active on that account (via the account fold) before [`RolesState::apply`].
//!
//! Further admin actions (catalog, pass publishing, …) join as more
//! `admin.*` payloads on `admin/<topic>` streams under the same rule.

use std::collections::{BTreeMap, BTreeSet};

use apa_kan_account::AccountId;
use apa_kan_view::{LedgerHead, RolesView, VIEW_SCHEMA};

use crate::account::AccountRules;
use crate::envelope::{verify_self_contained, SignedTx, TxPayload, GENESIS_PREV};
use crate::{TxError, TxReceipt};

/// The one admin stream so far.
pub const ROLES_STREAM: &str = "admin/roles";
/// Role that may grant and revoke roles.
pub const ADMIN_ROLE: &str = "admin";
/// Role name cap.
pub const MAX_ROLE_LEN: usize = 32;

fn valid_role(role: &str) -> bool {
	let mut chars = role.chars();
	role.len() <= MAX_ROLE_LEN
		&& chars.next().is_some_and(|c| c.is_ascii_lowercase())
		&& chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || c == '_')
}

/// Fold of `admin/roles`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RolesState {
	pub chain: String,
	/// Hex account id → roles.
	pub roles: BTreeMap<String, BTreeSet<String>>,
	pub next_seq: u64,
	pub head: String,
}

impl RolesState {
	pub fn has_role(&self, account: &AccountId, role: &str) -> bool {
		self.roles.get(&account.to_hex()).is_some_and(|r| r.contains(role))
	}

	/// May `signer` administer roles? (Operator, or holds `admin`.)
	pub fn is_admin(rules: &AccountRules, state: Option<&RolesState>, signer: &AccountId) -> bool {
		rules.operator.as_ref() == Some(signer)
			|| state.is_some_and(|s| s.has_role(signer, ADMIN_ROLE))
	}

	/// Apply an already verified and key-checked `tx`.
	pub fn apply(
		rules: &AccountRules,
		state: Option<&RolesState>,
		tx: &SignedTx,
		tx_id: &[u8; 32],
	) -> Result<(RolesState, TxReceipt), TxError> {
		let b = &tx.body;
		if b.stream != ROLES_STREAM {
			return Err(TxError::UnsupportedStream { stream: b.stream.clone() });
		}
		let mut next = match state {
			None => {
				if b.seq != 0 {
					return Err(TxError::NotFound);
				}
				if b.prev != GENESIS_PREV {
					return Err(TxError::PrevMismatch { expected: GENESIS_PREV.into() });
				}
				RolesState {
					chain: b.chain.clone(),
					roles: BTreeMap::new(),
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
				s.clone()
			}
		};
		if !Self::is_admin(rules, state, &b.signer) {
			return Err(TxError::Unauthorized);
		}
		match &b.payload {
			TxPayload::AdminGrantRole { account, role } => {
				if !valid_role(role) {
					return Err(TxError::BadEnvelope { reason: format!("bad role name {role:?}") });
				}
				if !next.roles.entry(account.to_hex()).or_default().insert(role.clone()) {
					return Err(TxError::Rejected { reason: "role already held".into() });
				}
			}
			TxPayload::AdminRevokeRole { account, role } => {
				let key = account.to_hex();
				let held = next.roles.get_mut(&key).is_some_and(|r| r.remove(role));
				if !held {
					return Err(TxError::Rejected { reason: "role not held".into() });
				}
				if next.roles.get(&key).is_some_and(BTreeSet::is_empty) {
					next.roles.remove(&key);
				}
			}
			other => {
				return Err(TxError::BadEnvelope {
					reason: format!("{} is not an admin transaction", other.kind()),
				})
			}
		}
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

	pub fn view(state: Option<&RolesState>) -> RolesView {
		RolesView {
			v: VIEW_SCHEMA,
			ledger: state.map(|s| LedgerHead {
				chain: s.chain.clone(),
				stream: ROLES_STREAM.into(),
				seq: s.next_seq,
				head: s.head.clone(),
			}),
			roles: state
				.map(|s| {
					s.roles.iter().map(|(a, r)| (a.clone(), r.iter().cloned().collect())).collect()
				})
				.unwrap_or_default(),
		}
	}
}

/// Fold committed `admin/roles` transactions, re-verifying signatures.
pub fn replay(
	rules: &AccountRules,
	chain: &str,
	txs: &[SignedTx],
) -> Result<Option<RolesState>, TxError> {
	let mut state: Option<RolesState> = None;
	for tx in txs {
		let id = verify_self_contained(tx, chain)?;
		let (next, _) = RolesState::apply(rules, state.as_ref(), tx, &id)?;
		state = Some(next);
	}
	Ok(state)
}

#[cfg(test)]
mod tests {
	#![allow(clippy::unwrap_used)]

	use super::*;
	use crate::envelope::TxSigner;
	use apa_kan_account::Seed;

	#[test]
	fn operator_and_admins_manage_roles() {
		let op = TxSigner::from_seed(&Seed::from_bytes([200; 32]));
		let alice = TxSigner::from_seed(&Seed::from_bytes([1; 32]));
		let bob = TxSigner::from_seed(&Seed::from_bytes([2; 32]));
		let rules = AccountRules { operator: Some(op.account()) };
		let mut state: Option<RolesState> = None;
		let mut log = Vec::new();
		let mut send = |who: &TxSigner, payload: TxPayload| {
			let (seq, prev) = state
				.as_ref()
				.map(|s| (s.next_seq, s.head.clone()))
				.unwrap_or((0, GENESIS_PREV.into()));
			let tx = who.sign("c", ROLES_STREAM, seq, &prev, payload).unwrap();
			let id = verify_self_contained(&tx, "c").unwrap();
			let r = RolesState::apply(&rules, state.as_ref(), &tx, &id);
			if let Ok((n, _)) = &r {
				state = Some(n.clone());
				log.push(tx);
			}
			r.map(|_| ())
		};
		let grant = |a: &TxSigner, role: &str| TxPayload::AdminGrantRole {
			account: a.account(),
			role: role.into(),
		};
		assert_eq!(send(&alice, grant(&alice, "admin")), Err(TxError::Unauthorized));
		send(&op, grant(&alice, "admin")).unwrap();
		send(&alice, grant(&bob, "moderator")).unwrap();
		assert!(matches!(send(&alice, grant(&bob, "Bad Role")), Err(TxError::BadEnvelope { .. })));
		assert_eq!(send(&bob, grant(&bob, "admin")), Err(TxError::Unauthorized));
		send(&op, TxPayload::AdminRevokeRole { account: alice.account(), role: "admin".into() })
			.unwrap();
		assert_eq!(send(&alice, grant(&bob, "x")), Err(TxError::Unauthorized));
		let s = state.clone().unwrap();
		assert!(s.has_role(&bob.account(), "moderator"));
		assert!(!s.roles.contains_key(&alice.account().to_hex()));
		assert_eq!(replay(&rules, "c", &log).unwrap().unwrap(), s);
		let v = RolesState::view(Some(&s));
		assert_eq!(v.roles[&bob.account().to_hex()], vec!["moderator".to_string()]);
	}
}
