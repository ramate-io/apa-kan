//! Challenge-response sign-in. No passwords or SSO.
//!
//! The client signs a server-issued nonce under `apa-kan/auth-challenge/v1`
//! with an active ML-DSA key. The account is unchanged; admin is a role on
//! that account (see the store).

use crate::canonical::to_canonical_json;
use crate::crypto::{message_id, verify_bytes, CryptoError, SignatureBytes, VerifyingKeyBytes};
use crate::envelope::DOMAIN_AUTH_CHALLENGE;
use crate::id::AccountId;
use serde::{Deserialize, Serialize};
use thiserror::Error;

const CHALLENGE_TTL_SECS: i64 = 5 * 60;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum AuthError {
	#[error("challenge expired")]
	Expired,
	#[error("challenge mismatch")]
	Mismatch,
	#[error("bad domain")]
	BadDomain,
	#[error(transparent)]
	Crypto(#[from] CryptoError),
	#[error("serde: {0}")]
	Serde(String),
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuthChallengePayload {
	pub domain: String,
	pub account_id: AccountId,
	pub vk: VerifyingKeyBytes,
	pub challenge_id: String,
	#[serde(with = "hex_bytes32")]
	pub nonce: [u8; 32],
	pub expires_at_unix: i64,
}

impl AuthChallengePayload {
	pub fn new(
		account_id: AccountId,
		vk: VerifyingKeyBytes,
		challenge_id: impl Into<String>,
		nonce: [u8; 32],
		now_unix: i64,
	) -> Self {
		Self {
			domain: DOMAIN_AUTH_CHALLENGE.to_string(),
			account_id,
			vk,
			challenge_id: challenge_id.into(),
			nonce,
			expires_at_unix: now_unix + CHALLENGE_TTL_SECS,
		}
	}

	pub fn message_id(&self) -> Result<[u8; 32], AuthError> {
		let canonical = to_canonical_json(self).map_err(|e| AuthError::Serde(e.to_string()))?;
		Ok(message_id(canonical.as_bytes(), &self.vk))
	}
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuthChallengeResponse {
	pub payload: AuthChallengePayload,
	pub signature: SignatureBytes,
}

pub fn verify_auth_challenge(
	issued: &AuthChallengePayload,
	response: &AuthChallengeResponse,
	now_unix: i64,
) -> Result<(), AuthError> {
	if issued.domain != DOMAIN_AUTH_CHALLENGE || response.payload.domain != DOMAIN_AUTH_CHALLENGE {
		return Err(AuthError::BadDomain);
	}
	if now_unix > issued.expires_at_unix {
		return Err(AuthError::Expired);
	}
	if issued != &response.payload {
		return Err(AuthError::Mismatch);
	}
	let id = issued.message_id()?;
	verify_bytes(&issued.vk, &id, &response.signature)?;
	Ok(())
}

mod hex_bytes32 {
	use serde::{Deserialize, Deserializer, Serialize, Serializer};

	pub fn serialize<S: Serializer>(bytes: &[u8; 32], serializer: S) -> Result<S::Ok, S::Error> {
		hex::encode(bytes).serialize(serializer)
	}

	pub fn deserialize<'de, D: Deserializer<'de>>(deserializer: D) -> Result<[u8; 32], D::Error> {
		let s = String::deserialize(deserializer)?;
		let bytes = hex::decode(s.trim()).map_err(serde::de::Error::custom)?;
		if bytes.len() != 32 {
			return Err(serde::de::Error::custom("expected 32 bytes"));
		}
		let mut out = [0u8; 32];
		out.copy_from_slice(&bytes);
		Ok(out)
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::crypto::{generate_keypair, sign_bytes, Seed};

	#[test]
	fn sign_in_round_trip() {
		let (sk, vk) = generate_keypair(&Seed::from_bytes([9u8; 32]));
		let account = AccountId::from_genesis_vk(&vk);
		let issued = AuthChallengePayload::new(account, vk.clone(), "ch1", [3u8; 32], 1000);
		let id = issued.message_id().unwrap();
		let response =
			AuthChallengeResponse { payload: issued.clone(), signature: sign_bytes(&sk, &id) };
		verify_auth_challenge(&issued, &response, 1000).unwrap();
		assert!(verify_auth_challenge(&issued, &response, 1000 + CHALLENGE_TTL_SECS + 1).is_err());
	}

	#[test]
	fn wrong_key_fails() {
		let (sk_a, vk_a) = generate_keypair(&Seed::from_bytes([1u8; 32]));
		let (_sk_b, vk_b) = generate_keypair(&Seed::from_bytes([2u8; 32]));
		let account = AccountId::from_genesis_vk(&vk_a);
		let issued = AuthChallengePayload::new(account, vk_a, "ch1", [3u8; 32], 1000);
		let mut forged = issued.clone();
		forged.vk = vk_b;
		let id = issued.message_id().unwrap();
		let response = AuthChallengeResponse { payload: forged, signature: sign_bytes(&sk_a, &id) };
		assert!(verify_auth_challenge(&issued, &response, 1000).is_err());
	}
}
