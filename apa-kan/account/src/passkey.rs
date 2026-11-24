//! Passkey protection for the account seed.
//!
//! The browser evaluates the WebAuthn PRF extension on a passkey with the
//! fixed input [`prf_input`]. The 32-byte PRF output never leaves the
//! device; [`wrap_seed`] stretches it with HKDF-SHA256 (random salt, info
//! bound to the account and credential) into an AES-256-GCM key and encrypts
//! the seed. The resulting [`PasskeyWrappedSeed`] is safe to store in
//! IndexedDB and on the server: without the passkey (and its user
//! verification) it is 256-bit-key ciphertext.
//!
//! Because passkeys sync through the platform password manager, a new device
//! (or a cleared browser) evaluates the same PRF, fetches the wrapped seed,
//! and unwraps the same account key.
//!
//! Fetching is gated too: the wrapped record carries
//! `access_hash = SHA-256(ACCESS_DOMAIN ‖ access_key)` where [`access_key`] is
//! another HKDF output of the same PRF. A device that has no account key yet
//! (that's why it is restoring) proves it holds the passkey by presenting
//! `access_key`; the host compares hashes and only then serves the record.

use aes_gcm::aead::{Aead, KeyInit, Payload};
use aes_gcm::{Aes256Gcm, Key, Nonce};
use hkdf::Hkdf;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::crypto::Seed;
use crate::id::AccountId;

/// Format version of [`PasskeyWrappedSeed`].
pub const PASSKEY_WRAP_VERSION: u32 = 1;
/// Domain of the PRF input every client evaluates.
pub const PRF_INPUT_DOMAIN: &[u8] = b"apa-kan/passkey-prf/v1";
/// HKDF info / AEAD associated-data domain.
pub const WRAP_DOMAIN: &[u8] = b"apa-kan/passkey-wrap/v1";
/// HKDF info domain for the fetch access key, and its hash domain.
pub const ACCESS_DOMAIN: &[u8] = b"apa-kan/passkey-access/v1";
/// Longest credential id accepted (WebAuthn allows up to 1023 bytes).
pub const MAX_CREDENTIAL_ID_LEN: usize = 1023;

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum PasskeyError {
	#[error("wrapped seed is malformed: {0}")]
	Malformed(&'static str),
	#[error("passkey did not unlock this seed")]
	WrongKey,
}

/// PRF input (`eval.first`): the same for every account, so a new device can
/// evaluate it before it knows which account the passkey belongs to.
pub fn prf_input() -> [u8; 32] {
	Sha256::digest(PRF_INPUT_DOMAIN).into()
}

/// A seed encrypted under a passkey's PRF output. Hex fields.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PasskeyWrappedSeed {
	pub v: u32,
	/// Hex WebAuthn credential id (raw id bytes).
	pub credential_id: String,
	/// HKDF salt, 32 bytes.
	pub salt: String,
	/// AES-GCM nonce, 12 bytes.
	pub nonce: String,
	/// AES-256-GCM ciphertext + tag of the 32-byte seed.
	pub ciphertext: String,
	/// Hex `SHA-256(ACCESS_DOMAIN ‖ access_key)`: hosts serve the record only
	/// to a caller presenting the matching [`access_key`].
	pub access_hash: String,
}

/// Key a device presents to fetch its wrapped seed; derived from the PRF
/// output, so only the passkey holder has it.
pub fn access_key(prf: &[u8; 32], account: &AccountId, credential_id: &[u8]) -> [u8; 32] {
	let mut info = ACCESS_DOMAIN.to_vec();
	info.extend_from_slice(account.as_bytes());
	info.extend_from_slice(credential_id);
	let mut key = [0u8; 32];
	// 32 bytes is far below HKDF-SHA256's output limit.
	let _ = Hkdf::<Sha256>::new(None, prf).expand(&info, &mut key);
	key
}

/// Hex hash stored for an access key.
pub fn access_hash(access_key: &[u8; 32]) -> String {
	let mut h = Sha256::new();
	h.update(ACCESS_DOMAIN);
	h.update(access_key);
	hex::encode(h.finalize())
}

fn info(account: &AccountId, credential_id: &[u8]) -> Vec<u8> {
	let mut v = Vec::with_capacity(WRAP_DOMAIN.len() + 32 + credential_id.len());
	v.extend_from_slice(WRAP_DOMAIN);
	v.extend_from_slice(account.as_bytes());
	v.extend_from_slice(credential_id);
	v
}

fn cipher(prf: &[u8; 32], salt: &[u8; 32], info: &[u8]) -> Aes256Gcm {
	let mut key = [0u8; 32];
	// 32 bytes is far below HKDF-SHA256's output limit.
	let _ = Hkdf::<Sha256>::new(Some(salt), prf).expand(info, &mut key);
	Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(&key))
}

/// Encrypt `seed` for `account` under the PRF output of `credential_id`.
/// `salt` and `nonce` must be fresh random bytes.
pub fn wrap_seed(
	seed: &Seed,
	prf: &[u8; 32],
	account: &AccountId,
	credential_id: &[u8],
	salt: [u8; 32],
	nonce: [u8; 12],
) -> PasskeyWrappedSeed {
	let info = info(account, credential_id);
	let ct = cipher(prf, &salt, &info)
		.encrypt(Nonce::from_slice(&nonce), Payload { msg: &seed.0, aad: &info })
		// AES-GCM encryption of 32 bytes does not fail.
		.unwrap_or_default();
	PasskeyWrappedSeed {
		v: PASSKEY_WRAP_VERSION,
		credential_id: hex::encode(credential_id),
		salt: hex::encode(salt),
		nonce: hex::encode(nonce),
		ciphertext: hex::encode(ct),
		access_hash: access_hash(&access_key(prf, account, credential_id)),
	}
}

/// Decoded `(credential_id, salt, nonce, ciphertext)`.
type WrapParts = (Vec<u8>, [u8; 32], [u8; 12], Vec<u8>);

impl PasskeyWrappedSeed {
	/// Shape checks (lengths, hex) without the key; hosts run this before
	/// storing a wrapped seed.
	pub fn validate(&self) -> Result<(), PasskeyError> {
		self.parts().map(|_| ())
	}

	fn parts(&self) -> Result<WrapParts, PasskeyError> {
		if self.v != PASSKEY_WRAP_VERSION {
			return Err(PasskeyError::Malformed("unknown version"));
		}
		let cred = hex::decode(&self.credential_id)
			.map_err(|_| PasskeyError::Malformed("credential_id"))?;
		if cred.is_empty() || cred.len() > MAX_CREDENTIAL_ID_LEN {
			return Err(PasskeyError::Malformed("credential_id length"));
		}
		let salt: [u8; 32] = hex::decode(&self.salt)
			.ok()
			.and_then(|b| b.try_into().ok())
			.ok_or(PasskeyError::Malformed("salt"))?;
		let nonce: [u8; 12] = hex::decode(&self.nonce)
			.ok()
			.and_then(|b| b.try_into().ok())
			.ok_or(PasskeyError::Malformed("nonce"))?;
		let ct =
			hex::decode(&self.ciphertext).map_err(|_| PasskeyError::Malformed("ciphertext"))?;
		if ct.len() != 32 + 16 {
			return Err(PasskeyError::Malformed("ciphertext length"));
		}
		let hash_ok = self.access_hash.len() == 64
			&& self.access_hash.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'));
		if !hash_ok {
			return Err(PasskeyError::Malformed("access_hash"));
		}
		Ok((cred, salt, nonce, ct))
	}

	/// Does `access_key` open this record for fetching?
	pub fn grants(&self, access_key: &[u8; 32]) -> bool {
		access_hash(access_key) == self.access_hash
	}

	/// Decrypt with the passkey's PRF output.
	pub fn unwrap_seed(&self, prf: &[u8; 32], account: &AccountId) -> Result<Seed, PasskeyError> {
		let (cred, salt, nonce, ct) = self.parts()?;
		let info = info(account, &cred);
		let pt = cipher(prf, &salt, &info)
			.decrypt(Nonce::from_slice(&nonce), Payload { msg: &ct, aad: &info })
			.map_err(|_| PasskeyError::WrongKey)?;
		let bytes: [u8; 32] = pt.try_into().map_err(|_| PasskeyError::WrongKey)?;
		Ok(Seed::from_bytes(bytes))
	}
}

#[cfg(test)]
mod tests {
	#![allow(clippy::unwrap_used)]

	use super::*;

	#[test]
	fn wrap_unwrap_and_binding() {
		let seed = Seed::from_bytes([9; 32]);
		let account = AccountId::from_bytes([1; 32]);
		let prf = [3u8; 32];
		let w = wrap_seed(&seed, &prf, &account, b"cred-1", [4; 32], [5; 12]);
		w.validate().unwrap();
		assert_eq!(w.unwrap_seed(&prf, &account).unwrap().0, seed.0);
		// Wrong passkey output, wrong account, or a swapped credential id fail.
		assert_eq!(w.unwrap_seed(&[0; 32], &account).err(), Some(PasskeyError::WrongKey));
		assert_eq!(
			w.unwrap_seed(&prf, &AccountId::from_bytes([2; 32])).err(),
			Some(PasskeyError::WrongKey)
		);
		let mut moved = w.clone();
		moved.credential_id = hex::encode(b"cred-2");
		assert_eq!(moved.unwrap_seed(&prf, &account).err(), Some(PasskeyError::WrongKey));
		let mut bad = w.clone();
		bad.nonce = "00".into();
		assert!(matches!(bad.validate(), Err(PasskeyError::Malformed(_))));
		// Ciphertext doesn't contain the seed.
		assert!(!w.ciphertext.contains(&hex::encode(seed.0)));
		assert_eq!(prf_input(), prf_input());
		// Only the PRF holder can fetch it.
		assert!(w.grants(&access_key(&prf, &account, b"cred-1")));
		assert!(!w.grants(&access_key(&[0; 32], &account, b"cred-1")));
		assert!(!w.grants(&access_key(&prf, &account, b"cred-2")));
		assert_ne!(access_key(&prf, &account, b"cred-1").as_slice(), &prf);
	}
}
