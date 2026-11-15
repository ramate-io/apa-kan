use ml_dsa::signature::{Signer, Verifier};
use ml_dsa::{EncodedVerifyingKey, MlDsa44, Signature, SigningKey, VerifyingKey, B32};
use rand::RngCore;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use sha3::{Digest, Sha3_256};
use thiserror::Error;
use zeroize::{Zeroize, ZeroizeOnDrop};

pub type Param = MlDsa44;

#[derive(Clone, Zeroize, ZeroizeOnDrop)]
pub struct Seed(pub [u8; 32]);

impl Seed {
	pub fn random() -> Self {
		let mut bytes = [0u8; 32];
		rand::thread_rng().fill_bytes(&mut bytes);
		Self(bytes)
	}

	pub fn from_bytes(bytes: [u8; 32]) -> Self {
		Self(bytes)
	}
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VerifyingKeyBytes(pub Vec<u8>);

impl VerifyingKeyBytes {
	pub fn as_slice(&self) -> &[u8] {
		&self.0
	}

	pub fn decode(&self) -> Result<VerifyingKey<Param>, CryptoError> {
		let enc = EncodedVerifyingKey::<Param>::try_from(self.0.as_slice())
			.map_err(|_| CryptoError::BadVerifyingKey)?;
		Ok(VerifyingKey::decode(&enc))
	}
}

impl Serialize for VerifyingKeyBytes {
	fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
		hex::encode(&self.0).serialize(serializer)
	}
}

impl<'de> Deserialize<'de> for VerifyingKeyBytes {
	fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
		let hex: String = String::deserialize(deserializer)?;
		let bytes = hex::decode(hex.trim()).map_err(serde::de::Error::custom)?;
		Ok(Self(bytes))
	}
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SignatureBytes(pub Vec<u8>);

impl Serialize for SignatureBytes {
	fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
		hex::encode(&self.0).serialize(serializer)
	}
}

impl<'de> Deserialize<'de> for SignatureBytes {
	fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
		let hex: String = String::deserialize(deserializer)?;
		let bytes = hex::decode(hex.trim()).map_err(serde::de::Error::custom)?;
		Ok(Self(bytes))
	}
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum CryptoError {
	#[error("invalid verifying key encoding")]
	BadVerifyingKey,
	#[error("invalid signature")]
	BadSignature,
	#[error("signature verification failed")]
	VerifyFailed,
}

pub fn generate_keypair(seed: &Seed) -> (SigningKey<Param>, VerifyingKeyBytes) {
	let b32 = B32::from_iter(seed.0);
	let sk = SigningKey::<Param>::from_seed(&b32);
	let vk = sk.verifying_key();
	let encoded = vk.encode();
	(sk, VerifyingKeyBytes(encoded.as_slice().to_vec()))
}

/// Message id per gwrdfa pattern: SHA3-256(canonical_payload ‖ signer_vk_bytes).
pub fn message_id(canonical_payload: &[u8], signer_vk: &VerifyingKeyBytes) -> [u8; 32] {
	let mut hasher = Sha3_256::new();
	hasher.update(canonical_payload);
	hasher.update(signer_vk.as_slice());
	hasher.finalize().into()
}

pub fn sign_bytes(sk: &SigningKey<Param>, message_id: &[u8; 32]) -> SignatureBytes {
	let sig = sk.sign(message_id);
	let enc = sig.encode();
	SignatureBytes(enc.as_slice().to_vec())
}

pub fn verify_bytes(
	vk: &VerifyingKeyBytes,
	message_id: &[u8; 32],
	sig: &SignatureBytes,
) -> Result<(), CryptoError> {
	let vk = vk.decode()?;
	let signature =
		Signature::<Param>::try_from(sig.0.as_slice()).map_err(|_| CryptoError::BadSignature)?;
	vk.verify(message_id, &signature).map_err(|_| CryptoError::VerifyFailed)
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn sign_verify_round_trip() {
		let seed = Seed::from_bytes([3u8; 32]);
		let (sk, vk) = generate_keypair(&seed);
		let payload = br#"{"domain":"test"}"#;
		let id = message_id(payload, &vk);
		let sig = sign_bytes(&sk, &id);
		verify_bytes(&vk, &id, &sig).expect("verify");
	}

	#[test]
	fn wrong_key_fails() {
		let seed_a = Seed::from_bytes([1u8; 32]);
		let seed_b = Seed::from_bytes([2u8; 32]);
		let (sk_a, vk_a) = generate_keypair(&seed_a);
		let (_sk_b, vk_b) = generate_keypair(&seed_b);
		let id = message_id(b"payload", &vk_a);
		let sig = sign_bytes(&sk_a, &id);
		assert!(verify_bytes(&vk_b, &id, &sig).is_err());
	}
}
