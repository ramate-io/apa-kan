//! Client-side seed storage (Decision #58-Q10). Server never implements this.
//!
//! - **Native shells:** OS keychain / Keystore backends (implemented per platform later).
//! - **Browser shell (Dioxus/WASM):** passphrase-wrapped seed in IndexedDB; use
//!   [`PassphraseWrappedSeed`] + a `IndexedDbKeystore` adapter (WebCrypto) when the shell lands.

use crate::crypto::Seed;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum KeyStoreError {
	#[error("not found")]
	NotFound,
	#[error("wrong passphrase")]
	BadPassphrase,
	#[error("backend: {0}")]
	Backend(String),
}

/// Persist and load the 32-byte ML-DSA seed for a local key slot.
pub trait KeyStore: Send + Sync {
	/// Stable slot id (e.g. `"default"` or account genesis vk fingerprint).
	fn load_seed(&self, slot_id: &str) -> Result<Seed, KeyStoreError>;
	fn store_seed(&self, slot_id: &str, seed: &Seed) -> Result<(), KeyStoreError>;
	fn delete_seed(&self, slot_id: &str) -> Result<(), KeyStoreError>;
}

/// In-memory keystore for unit tests and headless CLI smoke.
#[derive(Default)]
pub struct MemoryKeyStore(std::sync::Mutex<std::collections::HashMap<String, [u8; 32]>>);

impl MemoryKeyStore {
	pub fn new() -> Self {
		Self::default()
	}
}

impl KeyStore for MemoryKeyStore {
	fn load_seed(&self, slot_id: &str) -> Result<Seed, KeyStoreError> {
		let map = self.0.lock().map_err(|e| KeyStoreError::Backend(e.to_string()))?;
		let bytes = map.get(slot_id).copied().ok_or(KeyStoreError::NotFound)?;
		Ok(Seed::from_bytes(bytes))
	}

	fn store_seed(&self, slot_id: &str, seed: &Seed) -> Result<(), KeyStoreError> {
		let mut map = self.0.lock().map_err(|e| KeyStoreError::Backend(e.to_string()))?;
		map.insert(slot_id.to_string(), seed.0);
		Ok(())
	}

	fn delete_seed(&self, slot_id: &str) -> Result<(), KeyStoreError> {
		let mut map = self.0.lock().map_err(|e| KeyStoreError::Backend(e.to_string()))?;
		map.remove(slot_id);
		Ok(())
	}
}

/// Opaque ciphertext + salt for browser storage. Platform shell encrypts/decrypts with WebCrypto.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct WrappedSeedBlob {
	pub salt: [u8; 16],
	pub nonce: [u8; 12],
	pub ciphertext: Vec<u8>,
}

/// Passphrase-wrapped seed format for IndexedDB (encryption is shell-provided).
#[derive(Clone)]
pub struct PassphraseWrappedSeed {
	pub slot_id: String,
	pub blob: WrappedSeedBlob,
}

/// Browser and native backends implement this to map slots ↔ wrapped blobs.
pub trait PassphraseKeystoreBackend: Send + Sync {
	fn unwrap_seed(&self, passphrase: &str, blob: &WrappedSeedBlob) -> Result<Seed, KeyStoreError>;
	fn wrap_seed(&self, passphrase: &str, seed: &Seed) -> Result<WrappedSeedBlob, KeyStoreError>;
	fn load_blob(&self, slot_id: &str) -> Result<WrappedSeedBlob, KeyStoreError>;
	fn store_blob(&self, slot_id: &str, blob: &WrappedSeedBlob) -> Result<(), KeyStoreError>;
}

/// KeyStore adapter over a passphrase backend (browser path).
pub struct PassphraseKeyStore<B: PassphraseKeystoreBackend> {
	backend: B,
	passphrase: String,
}

impl<B: PassphraseKeystoreBackend> PassphraseKeyStore<B> {
	pub fn new(backend: B, passphrase: impl Into<String>) -> Self {
		Self { backend, passphrase: passphrase.into() }
	}
}

impl<B: PassphraseKeystoreBackend> KeyStore for PassphraseKeyStore<B> {
	fn load_seed(&self, slot_id: &str) -> Result<Seed, KeyStoreError> {
		let blob = self.backend.load_blob(slot_id)?;
		self.backend.unwrap_seed(&self.passphrase, &blob)
	}

	fn store_seed(&self, slot_id: &str, seed: &Seed) -> Result<(), KeyStoreError> {
		let blob = self.backend.wrap_seed(&self.passphrase, seed)?;
		self.backend.store_blob(slot_id, &blob)
	}

	fn delete_seed(&self, slot_id: &str) -> Result<(), KeyStoreError> {
		self.backend
			.store_blob(
				slot_id,
				&WrappedSeedBlob { salt: [0u8; 16], nonce: [0u8; 12], ciphertext: Vec::new() },
			)
			.map_err(|e| e)
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn memory_keystore_round_trip() {
		let ks = MemoryKeyStore::new();
		let seed = Seed::from_bytes([2u8; 32]);
		ks.store_seed("default", &seed).unwrap();
		let loaded = ks.load_seed("default").unwrap();
		assert_eq!(loaded.0, seed.0);
	}
}
