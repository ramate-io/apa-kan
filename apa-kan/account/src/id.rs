use crate::crypto::VerifyingKeyBytes;
use serde::{Deserialize, Serialize};
use sha3::{Digest, Sha3_256};
use std::fmt;

const ACCOUNT_ID_DOMAIN: &[u8] = b"apa-kan/account-id/v1";

/// Stable 32-byte account identifier (foreign key for tiles, Elo, listings).
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct AccountId(pub [u8; 32]);

impl Serialize for AccountId {
	fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
		hex::encode(self.0).serialize(serializer)
	}
}

impl<'de> Deserialize<'de> for AccountId {
	fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
		let s = String::deserialize(deserializer)?;
		Self::from_hex(&s).map_err(serde::de::Error::custom)
	}
}

impl AccountId {
	pub fn from_genesis_vk(vk: &VerifyingKeyBytes) -> Self {
		let mut hasher = Sha3_256::new();
		hasher.update(ACCOUNT_ID_DOMAIN);
		hasher.update(vk.as_slice());
		Self(hasher.finalize().into())
	}

	pub fn from_bytes(bytes: [u8; 32]) -> Self {
		Self(bytes)
	}

	pub fn as_bytes(&self) -> &[u8; 32] {
		&self.0
	}

	/// Canonical account id: raw 32-byte value as lowercase hex (Decision #58-Q9).
	pub fn to_hex(&self) -> String {
		hex::encode(self.0)
	}

	pub fn from_hex(s: &str) -> Result<Self, hex::FromHexError> {
		let bytes = hex::decode(s)?;
		if bytes.len() != 32 {
			return Err(hex::FromHexError::InvalidStringLength);
		}
		let mut arr = [0u8; 32];
		arr.copy_from_slice(&bytes);
		Ok(Self(arr))
	}
}

impl fmt::Debug for AccountId {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		write!(f, "AccountId({})", self.to_hex())
	}
}

impl fmt::Display for AccountId {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		write!(f, "{}", self.to_hex())
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::crypto::{generate_keypair, Seed};

	#[test]
	fn account_id_stable_from_genesis() {
		let seed = Seed::from_bytes([7u8; 32]);
		let (_sk, vk) = generate_keypair(&seed);
		let id1 = AccountId::from_genesis_vk(&vk);
		let id2 = AccountId::from_genesis_vk(&vk);
		assert_eq!(id1, id2);
		assert_ne!(id1.to_hex().len(), 0);
	}
}
