//! This device's account key.
//!
//! A 32-byte ML-DSA seed. Unprotected, it lives in `localStorage`
//! (`apakan.key.v1`) on web or in `APAKAN_KEY_FILE` on desktop (memory only
//! when unset); the account is the one the key is genesis for, so it works
//! as a guest and becomes a full account once it signs `account.register`.
//!
//! Protected by a passkey (web, see [`crate::keyguard`]), the plaintext is
//! removed: the seed is kept AES-GCM-wrapped under the passkey's PRF output
//! in IndexedDB (and as an `account.set_passkey_backup` on the server), and
//! `localStorage` keeps only a marker (`apakan.passkey.v1`: account +
//! credential id). Until the passkey unlocks it, the identity is **locked**:
//! the account is known (so views still show your seat) but nothing can be
//! signed.
//!
//! The 24-word recovery phrase ([`apa_kan_account::seed_to_phrase`]) is the
//! fallback everywhere; on desktop `APAKAN_RECOVERY_PHRASE` restores a key
//! at startup.

use std::sync::{Arc, OnceLock, RwLock};

use apa_kan_account::{AccountId, Seed};
use apa_kan_tx::TxSigner;
use serde::{Deserialize, Serialize};

/// Plaintext seed (hex) when the key is not passkey-protected.
pub const PLAIN_KEY: &str = "apakan.key.v1";
/// Marker for a passkey-protected key (JSON [`PasskeyMarker`]).
pub const PASSKEY_MARKER: &str = "apakan.passkey.v1";

/// How the key is stored on this device.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Protection {
	/// Plaintext seed in local storage / the key file.
	Plain,
	/// Wrapped under a passkey (hex credential id).
	Passkey { credential_id: String },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PasskeyMarker {
	/// Hex account id.
	pub account: String,
	/// Hex credential id.
	pub credential_id: String,
}

struct Inner {
	account: AccountId,
	seed: Option<[u8; 32]>,
	signer: Option<Arc<TxSigner>>,
	protection: Protection,
}

fn parse_seed(hex_seed: &str) -> Option<Seed> {
	let bytes: [u8; 32] = hex::decode(hex_seed.trim()).ok()?.try_into().ok()?;
	Some(Seed::from_bytes(bytes))
}

fn unlocked(seed: Seed, account: Option<AccountId>, protection: Protection) -> Inner {
	let signer = match account {
		Some(a) => TxSigner::for_account(&seed, a),
		None => TxSigner::from_seed(&seed),
	};
	Inner {
		account: signer.account(),
		seed: Some(seed.0),
		signer: Some(Arc::new(signer)),
		protection,
	}
}

#[cfg(target_arch = "wasm32")]
fn storage() -> Option<web_sys::Storage> {
	web_sys::window().and_then(|w| w.local_storage().ok().flatten())
}

/// Read a local-storage item (web) — `None` elsewhere.
pub fn local_get(key: &str) -> Option<String> {
	#[cfg(target_arch = "wasm32")]
	{
		storage().and_then(|s| s.get_item(key).ok().flatten())
	}
	#[cfg(not(target_arch = "wasm32"))]
	{
		let _ = key;
		None
	}
}

/// Write (`Some`) or remove (`None`) a local-storage item (web only).
pub fn local_set(key: &str, value: Option<&str>) {
	#[cfg(target_arch = "wasm32")]
	if let Some(s) = storage() {
		let _ = match value {
			Some(v) => s.set_item(key, v),
			None => s.remove_item(key),
		};
	}
	#[cfg(not(target_arch = "wasm32"))]
	{
		let _ = (key, value);
	}
}

fn load() -> Inner {
	#[cfg(target_arch = "wasm32")]
	{
		if let Some(seed) = local_get(PLAIN_KEY).and_then(|v| parse_seed(&v)) {
			return unlocked(seed, None, Protection::Plain);
		}
		let marker = local_get(PASSKEY_MARKER)
			.and_then(|m| serde_json::from_str::<PasskeyMarker>(&m).ok())
			.and_then(|m| AccountId::from_hex(&m.account).ok().map(|a| (a, m.credential_id)));
		if let Some((account, credential_id)) = marker {
			return Inner {
				account,
				seed: None,
				signer: None,
				protection: Protection::Passkey { credential_id },
			};
		}
		let seed = Seed::random();
		local_set(PLAIN_KEY, Some(&hex::encode(seed.0)));
		unlocked(seed, None, Protection::Plain)
	}
	#[cfg(not(target_arch = "wasm32"))]
	{
		let path = std::env::var_os("APAKAN_KEY_FILE").map(std::path::PathBuf::from);
		let from_phrase = std::env::var("APAKAN_RECOVERY_PHRASE")
			.ok()
			.filter(|p| !p.trim().is_empty())
			.and_then(|p| match apa_kan_account::phrase_to_seed(&p) {
				Ok(s) => Some(s),
				Err(e) => {
					eprintln!("APAKAN_RECOVERY_PHRASE ignored: {e}");
					None
				}
			});
		let seed = from_phrase
			.or_else(|| {
				path.as_ref()
					.and_then(|p| std::fs::read_to_string(p).ok())
					.and_then(|v| parse_seed(&v))
			})
			.unwrap_or_else(Seed::random);
		if let Some(p) = path {
			let _ = std::fs::write(p, hex::encode(seed.0));
		}
		unlocked(seed, None, Protection::Plain)
	}
}

fn state() -> &'static RwLock<Inner> {
	static S: OnceLock<RwLock<Inner>> = OnceLock::new();
	S.get_or_init(|| RwLock::new(load()))
}

fn read<T>(f: impl FnOnce(&Inner) -> T) -> T {
	match state().read() {
		Ok(g) => f(&g),
		Err(p) => f(&p.into_inner()),
	}
}

/// The signer for this device, or `None` while a passkey-protected key is
/// locked.
pub fn signer() -> Option<Arc<TxSigner>> {
	read(|i| i.signer.clone())
}

/// This device's account (known even while locked).
pub fn account() -> AccountId {
	read(|i| i.account)
}

/// Hex of [`account`].
pub fn account_hex() -> String {
	account().to_hex()
}

/// The seed, while unlocked (for the recovery phrase and passkey wrapping).
pub fn seed() -> Option<Seed> {
	read(|i| i.seed.map(Seed::from_bytes))
}

pub fn protection() -> Protection {
	read(|i| i.protection.clone())
}

pub fn is_locked() -> bool {
	read(|i| i.signer.is_none())
}

/// This device's deal salt for `stream` (see `apa_kan_tx::deal_salt`), while
/// unlocked.
pub fn deal_salt(stream: &str) -> Option<[u8; 32]> {
	read(|i| i.seed.map(|s| apa_kan_tx::deal_salt(&s, stream)))
}

/// Replace the identity with `seed` (for `account`, default: the account the
/// seed is genesis for) and persist how it is protected: `Plain` writes the
/// seed to local storage / the key file; `Passkey` removes any plaintext and
/// writes the marker (the caller stores the wrapped seed).
pub fn install(seed: Seed, account: Option<AccountId>, protection: Protection) {
	let inner = unlocked(seed, account, protection.clone());
	match &protection {
		Protection::Plain => {
			local_set(PLAIN_KEY, Some(&hex::encode(inner.seed.unwrap_or_default())));
			local_set(PASSKEY_MARKER, None);
			#[cfg(not(target_arch = "wasm32"))]
			if let Some(p) = std::env::var_os("APAKAN_KEY_FILE") {
				let _ = std::fs::write(p, hex::encode(inner.seed.unwrap_or_default()));
			}
		}
		Protection::Passkey { credential_id } => {
			let marker = PasskeyMarker {
				account: inner.account.to_hex(),
				credential_id: credential_id.clone(),
			};
			if let Ok(json) = serde_json::to_string(&marker) {
				local_set(PASSKEY_MARKER, Some(&json));
			}
			local_set(PLAIN_KEY, None);
		}
	}
	match state().write() {
		Ok(mut g) => *g = inner,
		Err(p) => *p.into_inner() = inner,
	}
}

/// Unlock a passkey-protected identity in memory (nothing is persisted).
pub fn unlock(seed: Seed) -> Result<(), String> {
	let (account, protection) = read(|i| (i.account, i.protection.clone()));
	let inner = unlocked(seed, Some(account), protection);
	if inner.signer.as_ref().map(|s| s.account()) != Some(account) {
		return Err("passkey unlocked a key for another account".into());
	}
	match state().write() {
		Ok(mut g) => *g = inner,
		Err(p) => *p.into_inner() = inner,
	}
	Ok(())
}

#[cfg(test)]
mod tests {
	#![allow(clippy::unwrap_used)]

	use super::*;

	// The global identity is shared by every test in this binary, so these
	// exercise the pure parts only.
	#[test]
	fn unlocked_binds_signer_to_account() {
		let g = unlocked(Seed::from_bytes([42; 32]), None, Protection::Plain);
		assert_eq!(g.signer.as_ref().unwrap().account(), g.account);
		assert_eq!(g.account, TxSigner::from_seed(&Seed::from_bytes([42; 32])).account());
		// A linked key signs for its account, not its own genesis id.
		let other = AccountId::from_bytes([9; 32]);
		let l = unlocked(Seed::from_bytes([43; 32]), Some(other), Protection::Plain);
		assert_eq!(l.account, other);
		assert_eq!(l.signer.unwrap().account(), other);
		assert_eq!(parse_seed(&"07".repeat(32)).unwrap().0, [7; 32]);
		assert!(parse_seed("zz").is_none());
	}
}
