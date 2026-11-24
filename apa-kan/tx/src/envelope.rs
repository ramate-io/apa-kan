//! The common signed transaction envelope.

use apa_kan_account::canonical::to_canonical_json;
use apa_kan_account::crypto::{
	generate_keypair, message_id, sign_bytes, verify_bytes, Param, Seed, SignatureBytes,
	VerifyingKeyBytes,
};
use apa_kan_account::{AccountId, RecoveryKind};
use apa_kan_view::{ActionWire, Seat};
use ml_dsa::SigningKey;
use serde::{Deserialize, Serialize};

use crate::TxError;

/// Domain tag in every [`TxBody`]; bump the version for incompatible changes.
pub const TX_DOMAIN: &str = "apa-kan/tx/v1";

/// `prev` of a stream's first transaction (all-zero hash, hex).
pub const GENESIS_PREV: &str = "0000000000000000000000000000000000000000000000000000000000000000";

/// What a transaction does. Tagged by `type`, namespaced by stream kind so
/// `account.*` and `admin.*` payloads can join without touching the envelope.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum TxPayload {
	/// Open `game/<id>` and take `seat`. Must be `seq = 0`. `seed_commit` is
	/// `game::seed_commitment(stream, salt)` for a secret 32-byte salt the
	/// creator reveals after the join.
	#[serde(rename = "game.create")]
	GameCreate { seat: Seat, label: String, seed_commit: String },
	/// Take the open seat. The game then waits for the creator's reveal.
	#[serde(rename = "game.join")]
	GameJoin { label: String },
	/// Creator reveals the committed salt (hex); the deal seed mixes it with
	/// the join transaction's id.
	#[serde(rename = "game.reveal")]
	GameReveal { salt: String },
	/// One engine action by the seat to move.
	#[serde(rename = "game.move")]
	GameMove { action: ActionWire },
	/// Concede; the other seat wins.
	#[serde(rename = "game.resign")]
	GameResign,
	/// Creator withdraws an open game nobody joined, or the joiner abandons a
	/// game whose creator has not revealed.
	#[serde(rename = "game.cancel")]
	GameCancel,

	/// Open `account/<id>` (seq 0), signed by the genesis key `id` derives from.
	#[serde(rename = "account.register")]
	AccountRegister { label: String },
	/// Add `vk` to the account. `proof` is `vk`'s signature over
	/// [`link_proof_id`], so nobody can link a key they don't hold.
	#[serde(rename = "account.link_key")]
	AccountLinkKey { vk: VerifyingKeyBytes, label: String, proof: SignatureBytes },
	/// Revoke a key (never the last active one).
	#[serde(rename = "account.revoke_key")]
	AccountRevokeKey { vk: VerifyingKeyBytes },
	/// Set or clear the recovery email and its switches.
	#[serde(rename = "account.set_email")]
	AccountSetEmail { email: Option<String>, recovery_opt_in: bool, approve_via_email: bool },
	/// Signed by the key to be recovered (not on the account yet).
	/// `email_proof` = [`crate::account::email_proof`] for the email on file.
	#[serde(rename = "account.recovery_request")]
	AccountRecoveryRequest { kind: RecoveryKind, email_proof: String },
	/// Operator-signed: the host mailed a token whose hash this records, and
	/// the cooldown ends at `usable_at_unix`.
	#[serde(rename = "account.recovery_challenge")]
	AccountRecoveryChallenge { token_hash: String, usable_at_unix: i64 },
	/// Pending key proves it received the mailed token.
	#[serde(rename = "account.recovery_confirm")]
	AccountRecoveryConfirm { token: String },
	/// Pending key becomes active once the cooldown passed.
	#[serde(rename = "account.recovery_finish")]
	AccountRecoveryFinish,
	/// An active key stops a recovery in progress.
	#[serde(rename = "account.recovery_cancel")]
	AccountRecoveryCancel,

	/// Give `account` a role (`admin/roles`; operator or an `admin`).
	#[serde(rename = "admin.grant_role")]
	AdminGrantRole { account: AccountId, role: String },
	/// Take a role away.
	#[serde(rename = "admin.revoke_role")]
	AdminRevokeRole { account: AccountId, role: String },
}

/// Domain of the new key's proof in `account.link_key`.
pub const LINK_PROOF_DOMAIN: &str = "apa-kan/link-key/v1";

/// Message the new key signs to be linked to `account` on `chain`.
pub fn link_proof_id(chain: &str, account: &AccountId, vk: &VerifyingKeyBytes) -> [u8; 32] {
	let body = serde_json::json!({
		"domain": LINK_PROOF_DOMAIN,
		"chain": chain,
		"account": account.to_hex(),
	});
	// serde_json::Value always serializes.
	let canonical = to_canonical_json(&body).unwrap_or_default();
	message_id(canonical.as_bytes(), vk)
}

impl TxPayload {
	pub fn kind(&self) -> &'static str {
		match self {
			TxPayload::GameCreate { .. } => "game.create",
			TxPayload::GameJoin { .. } => "game.join",
			TxPayload::GameReveal { .. } => "game.reveal",
			TxPayload::GameMove { .. } => "game.move",
			TxPayload::GameResign => "game.resign",
			TxPayload::GameCancel => "game.cancel",
			TxPayload::AccountRegister { .. } => "account.register",
			TxPayload::AccountLinkKey { .. } => "account.link_key",
			TxPayload::AccountRevokeKey { .. } => "account.revoke_key",
			TxPayload::AccountSetEmail { .. } => "account.set_email",
			TxPayload::AccountRecoveryRequest { .. } => "account.recovery_request",
			TxPayload::AccountRecoveryChallenge { .. } => "account.recovery_challenge",
			TxPayload::AccountRecoveryConfirm { .. } => "account.recovery_confirm",
			TxPayload::AccountRecoveryFinish => "account.recovery_finish",
			TxPayload::AccountRecoveryCancel => "account.recovery_cancel",
			TxPayload::AdminGrantRole { .. } => "admin.grant_role",
			TxPayload::AdminRevokeRole { .. } => "admin.revoke_role",
		}
	}
}

/// Everything a signature covers.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TxBody {
	/// Always [`TX_DOMAIN`].
	pub domain: String,
	/// Deployment / network id (stops replay across hosts or test nets).
	pub chain: String,
	/// Ledger stream, e.g. `game/<id>`.
	pub stream: String,
	/// Position in the stream (0 for the first transaction).
	pub seq: u64,
	/// Hex id of the stream's previous transaction, or [`GENESIS_PREV`].
	pub prev: String,
	/// Account acting.
	pub signer: AccountId,
	/// The account key that signed (one of the account's authorized keys).
	pub signer_vk: VerifyingKeyBytes,
	pub payload: TxPayload,
}

/// A body plus its ML-DSA signature. Same JSON over HTTP, in the ledger, or
/// in a peer datagram.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SignedTx {
	pub body: TxBody,
	pub sig: SignatureBytes,
}

impl TxBody {
	/// Message id, exactly as `apa-kan-account` computes it:
	/// SHA3-256(canonical_json(body) ‖ signer_vk). Also the ledger entry hash.
	pub fn id(&self) -> Result<[u8; 32], TxError> {
		let canonical =
			to_canonical_json(self).map_err(|e| TxError::BadEnvelope { reason: e.to_string() })?;
		Ok(message_id(canonical.as_bytes(), &self.signer_vk))
	}
}

impl SignedTx {
	pub fn id(&self) -> Result<[u8; 32], TxError> {
		self.body.id()
	}

	pub fn id_hex(&self) -> Result<String, TxError> {
		self.id().map(hex::encode)
	}
}

fn is_hash(s: &str) -> bool {
	s.len() == 64 && s.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
}

/// Checks that need nothing but the transaction and the chain id: domain,
/// chain, shape, and the signature against the key in the body. Replaying a
/// committed stream runs only this (key authority was checked at admission).
/// Returns the transaction id.
pub fn verify_self_contained(tx: &SignedTx, chain: &str) -> Result<[u8; 32], TxError> {
	let b = &tx.body;
	if b.domain != TX_DOMAIN {
		return Err(TxError::BadEnvelope { reason: format!("domain must be {TX_DOMAIN}") });
	}
	if b.chain != chain {
		return Err(TxError::WrongChain { expected: chain.into(), got: b.chain.clone() });
	}
	if !is_hash(&b.prev) {
		return Err(TxError::BadEnvelope { reason: "prev must be 64 lowercase hex".into() });
	}
	if (b.seq == 0) != (b.prev == GENESIS_PREV) {
		return Err(TxError::BadEnvelope { reason: "only seq 0 links to the genesis prev".into() });
	}
	let id = b.id()?;
	verify_bytes(&b.signer_vk, &id, &tx.sig).map_err(|_| TxError::BadSignature)?;
	Ok(id)
}

/// What the account directory says about `(signer, signer_vk)`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum KeyStatus {
	/// Registered account; the key is active on it.
	Active,
	/// Registered account; the key was revoked or is still in recovery cooldown.
	Inactive,
	/// Registered account; the key was never linked to it.
	NotOnAccount,
	/// No such account registered. Allowed only for its genesis key.
	UnknownAccount,
}

/// Admission check at a host: [`verify_self_contained`] plus key authority.
pub fn admit(tx: &SignedTx, chain: &str, key: KeyStatus) -> Result<[u8; 32], TxError> {
	let id = verify_self_contained(tx, chain)?;
	match key {
		KeyStatus::Active => Ok(id),
		KeyStatus::UnknownAccount
			if AccountId::from_genesis_vk(&tx.body.signer_vk) == tx.body.signer =>
		{
			Ok(id)
		}
		_ => Err(TxError::KeyNotAuthorized),
	}
}

/// Holds one account key and signs bodies with it.
pub struct TxSigner {
	sk: SigningKey<Param>,
	vk: VerifyingKeyBytes,
	account: AccountId,
}

impl TxSigner {
	/// Key from a 32-byte seed. The account is the one this key is genesis
	/// for; use [`TxSigner::for_account`] for a linked key.
	pub fn from_seed(seed: &Seed) -> Self {
		let (sk, vk) = generate_keypair(seed);
		let account = AccountId::from_genesis_vk(&vk);
		Self { sk, vk, account }
	}

	/// A key linked to an existing `account`.
	pub fn for_account(seed: &Seed, account: AccountId) -> Self {
		let (sk, vk) = generate_keypair(seed);
		Self { sk, vk, account }
	}

	pub fn account(&self) -> AccountId {
		self.account
	}

	pub fn verifying_key(&self) -> &VerifyingKeyBytes {
		&self.vk
	}

	/// This key's proof for `account.link_key` onto `account`.
	pub fn link_proof(&self, chain: &str, account: &AccountId) -> SignatureBytes {
		sign_bytes(&self.sk, &link_proof_id(chain, account, &self.vk))
	}

	pub fn sign(
		&self,
		chain: &str,
		stream: &str,
		seq: u64,
		prev: &str,
		payload: TxPayload,
	) -> Result<SignedTx, TxError> {
		let body = TxBody {
			domain: TX_DOMAIN.into(),
			chain: chain.into(),
			stream: stream.into(),
			seq,
			prev: prev.into(),
			signer: self.account,
			signer_vk: self.vk.clone(),
			payload,
		};
		let id = body.id()?;
		Ok(SignedTx { sig: sign_bytes(&self.sk, &id), body })
	}
}

#[cfg(test)]
mod tests {
	#![allow(clippy::unwrap_used, clippy::expect_used)]

	use super::*;

	fn signer(n: u8) -> TxSigner {
		TxSigner::from_seed(&Seed::from_bytes([n; 32]))
	}

	#[test]
	fn sign_verify_and_tamper() {
		let s = signer(1);
		let tx = s
			.sign(
				"c",
				"game/g",
				0,
				GENESIS_PREV,
				TxPayload::GameCreate { seat: 1, label: "a".into(), seed_commit: "00".repeat(32) },
			)
			.unwrap();
		let id = verify_self_contained(&tx, "c").unwrap();
		assert_eq!(hex::encode(id), tx.id_hex().unwrap());
		assert_eq!(admit(&tx, "c", KeyStatus::UnknownAccount), Ok(id));
		assert_eq!(admit(&tx, "c", KeyStatus::Active), Ok(id));
		assert_eq!(admit(&tx, "c", KeyStatus::Inactive), Err(TxError::KeyNotAuthorized));
		assert_eq!(admit(&tx, "c", KeyStatus::NotOnAccount), Err(TxError::KeyNotAuthorized));
		assert!(matches!(verify_self_contained(&tx, "other"), Err(TxError::WrongChain { .. })));

		let mut t = tx.clone();
		t.body.payload =
			TxPayload::GameCreate { seat: 2, label: "a".into(), seed_commit: "00".repeat(32) };
		assert_eq!(verify_self_contained(&t, "c"), Err(TxError::BadSignature));
		let mut t = tx.clone();
		t.body.domain = "apa-kan/action/v1".into();
		assert!(matches!(verify_self_contained(&t, "c"), Err(TxError::BadEnvelope { .. })));
		let mut t = tx.clone();
		t.body.seq = 1;
		assert!(matches!(verify_self_contained(&t, "c"), Err(TxError::BadEnvelope { .. })));
		// A key claiming someone else's unregistered account is refused.
		let other = signer(2);
		let mut t = tx;
		t.body.signer = other.account();
		let id = t.body.id().unwrap();
		t.sig = sign_bytes(&s.sk, &id);
		assert!(verify_self_contained(&t, "c").is_ok());
		assert_eq!(admit(&t, "c", KeyStatus::UnknownAccount), Err(TxError::KeyNotAuthorized));
		// ...but is fine once the directory says the key is linked there.
		assert!(admit(&t, "c", KeyStatus::Active).is_ok());
	}

	#[test]
	fn wire_shape_and_canonical_id() {
		let s = signer(3);
		let tx = s
			.sign(
				"c",
				"game/g",
				4,
				&"ab".repeat(32),
				TxPayload::GameMove { action: ActionWire::Move { from: 1, to: 2 } },
			)
			.unwrap();
		let json = serde_json::to_value(&tx).unwrap();
		assert_eq!(json["body"]["payload"]["type"], "game.move");
		assert_eq!(json["body"]["payload"]["action"]["kind"], "move");
		assert_eq!(json["body"]["domain"], TX_DOMAIN);
		assert_eq!(json["body"]["signer"], s.account().to_hex());
		// Field order on the wire does not matter: the id is over canonical JSON.
		let back: SignedTx = serde_json::from_str(&serde_json::to_string(&json).unwrap()).unwrap();
		assert_eq!(back.id().unwrap(), tx.id().unwrap());
		assert!(verify_self_contained(&back, "c").is_ok());
	}
}
