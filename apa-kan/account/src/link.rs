use crate::canonical::to_canonical_json;
use crate::crypto::{message_id, verify_bytes, SignatureBytes, VerifyingKeyBytes};
use crate::envelope::{DOMAIN_LINK_APPROVAL, DOMAIN_LINK_REQUEST};
use crate::id::AccountId;
use serde::{Deserialize, Serialize};
use sha3::{Digest, Sha3_256};
use thiserror::Error;

const LINK_REQUEST_TTL_SECS: i64 = 15 * 60;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LinkRequestStatus {
	Open,
	Approved,
	Rejected,
	Expired,
	Cancelled,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LinkRequestPayload {
	pub domain: String,
	pub account_id: AccountId,
	pub new_vk: VerifyingKeyBytes,
	#[serde(with = "hex_nonce")]
	pub nonce: [u8; 32],
	pub expires_at_unix: i64,
	pub label: String,
}

impl LinkRequestPayload {
	pub fn new(
		account_id: AccountId,
		new_vk: VerifyingKeyBytes,
		nonce: [u8; 32],
		now_unix: i64,
		label: impl Into<String>,
	) -> Self {
		Self {
			domain: DOMAIN_LINK_REQUEST.to_string(),
			account_id,
			new_vk,
			nonce,
			expires_at_unix: now_unix + LINK_REQUEST_TTL_SECS,
			label: label.into(),
		}
	}
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LinkRequest {
	pub request_id: [u8; 32],
	pub payload: LinkRequestPayload,
	pub new_key_sig: SignatureBytes,
}

impl LinkRequest {
	pub fn compute_request_id(payload: &LinkRequestPayload) -> Result<[u8; 32], serde_json::Error> {
		let canonical = to_canonical_json(payload)?;
		let mut hasher = Sha3_256::new();
		hasher.update(canonical.as_bytes());
		hasher.update(payload.new_vk.as_slice());
		Ok(hasher.finalize().into())
	}

	pub fn verify_new_key_signature(&self) -> Result<(), LinkError> {
		let id = Self::compute_request_id(&self.payload).map_err(LinkError::Serde)?;
		if id != self.request_id {
			return Err(LinkError::RequestIdMismatch);
		}
		verify_bytes(&self.payload.new_vk, &id, &self.new_key_sig).map_err(LinkError::Crypto)?;
		Ok(())
	}

	pub fn is_open_at(&self, status: LinkRequestStatus, now_unix: i64) -> bool {
		status == LinkRequestStatus::Open && now_unix <= self.payload.expires_at_unix
	}
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LinkApprovalPayload {
	pub domain: String,
	pub request_id: [u8; 32],
	pub account_id: AccountId,
	pub approver_vk: VerifyingKeyBytes,
	pub nonce: u64,
	pub expires_at_unix: i64,
}

impl LinkApprovalPayload {
	pub fn new(
		request_id: [u8; 32],
		account_id: AccountId,
		approver_vk: VerifyingKeyBytes,
		nonce: u64,
		now_unix: i64,
	) -> Self {
		Self {
			domain: DOMAIN_LINK_APPROVAL.to_string(),
			request_id,
			account_id,
			approver_vk,
			nonce,
			expires_at_unix: now_unix + LINK_REQUEST_TTL_SECS,
		}
	}
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LinkApproval {
	pub payload: LinkApprovalPayload,
	pub approver_sig: SignatureBytes,
}

impl LinkApproval {
	pub fn message_id(&self) -> Result<[u8; 32], serde_json::Error> {
		let canonical = to_canonical_json(&self.payload)?;
		Ok(message_id(canonical.as_bytes(), &self.payload.approver_vk))
	}

	pub fn verify_approver_signature(&self) -> Result<[u8; 32], LinkError> {
		let id = self.message_id().map_err(LinkError::Serde)?;
		verify_bytes(&self.payload.approver_vk, &id, &self.approver_sig)
			.map_err(LinkError::Crypto)?;
		Ok(id)
	}
}

mod hex_nonce {
	use serde::{Deserialize, Deserializer, Serialize, Serializer};

	pub fn serialize<S: Serializer>(bytes: &[u8; 32], serializer: S) -> Result<S::Ok, S::Error> {
		hex::encode(bytes).serialize(serializer)
	}

	pub fn deserialize<'de, D: Deserializer<'de>>(deserializer: D) -> Result<[u8; 32], D::Error> {
		let s = String::deserialize(deserializer)?;
		let v = hex::decode(s.trim()).map_err(serde::de::Error::custom)?;
		if v.len() != 32 {
			return Err(serde::de::Error::custom("nonce must be 32 bytes"));
		}
		let mut out = [0u8; 32];
		out.copy_from_slice(&v);
		Ok(out)
	}
}

#[derive(Debug, Error)]
pub enum LinkError {
	#[error("request id does not match payload")]
	RequestIdMismatch,
	#[error("request expired")]
	Expired,
	#[error("request not open")]
	NotOpen,
	#[error("approver not active on account")]
	ApproverNotActive,
	#[error("bad nonce")]
	BadNonce,
	#[error("account mismatch")]
	AccountMismatch,
	#[error("crypto error: {0}")]
	Crypto(#[from] crate::crypto::CryptoError),
	#[error("serde error: {0}")]
	Serde(#[from] serde_json::Error),
}

/// Verify a link approval against an already-validated open request.
pub fn verify_link_approval(
	request: &LinkRequest,
	request_status: LinkRequestStatus,
	approval: &LinkApproval,
	approver_is_active: bool,
	expected_nonce: u64,
	now_unix: i64,
) -> Result<(), LinkError> {
	request.verify_new_key_signature()?;
	if !request.is_open_at(request_status, now_unix) {
		if now_unix > request.payload.expires_at_unix {
			return Err(LinkError::Expired);
		}
		return Err(LinkError::NotOpen);
	}
	if request.payload.account_id != approval.payload.account_id {
		return Err(LinkError::AccountMismatch);
	}
	if request.request_id != approval.payload.request_id {
		return Err(LinkError::RequestIdMismatch);
	}
	if !approver_is_active {
		return Err(LinkError::ApproverNotActive);
	}
	if approval.payload.nonce != expected_nonce {
		return Err(LinkError::BadNonce);
	}
	if now_unix > approval.payload.expires_at_unix {
		return Err(LinkError::Expired);
	}
	approval.verify_approver_signature()?;
	Ok(())
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::crypto::{generate_keypair, sign_bytes, Seed};

	fn unix_now() -> i64 {
		1_700_000_000
	}

	#[test]
	fn link_request_round_trip() {
		let seed = Seed::from_bytes([5u8; 32]);
		let (sk, vk) = generate_keypair(&seed);
		let account_id = AccountId::from_genesis_vk(&vk);
		let (new_sk, new_vk) = generate_keypair(&Seed::from_bytes([6u8; 32]));
		let payload =
			LinkRequestPayload::new(account_id, new_vk.clone(), [9u8; 32], unix_now(), "phone");
		let request_id = LinkRequest::compute_request_id(&payload).unwrap();
		let sig = sign_bytes(&new_sk, &request_id);
		let req = LinkRequest { request_id, payload, new_key_sig: sig };
		req.verify_new_key_signature().unwrap();

		let approval_payload =
			LinkApprovalPayload::new(request_id, account_id, vk.clone(), 0, unix_now());
		let canonical = to_canonical_json(&approval_payload).unwrap();
		let msg_id = message_id(canonical.as_bytes(), &vk);
		let approver_sig = sign_bytes(&sk, &msg_id);
		let approval = LinkApproval { payload: approval_payload, approver_sig };
		verify_link_approval(&req, LinkRequestStatus::Open, &approval, true, 0, unix_now())
			.unwrap();
		let _ = new_sk;
	}
}
