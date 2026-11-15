use crate::crypto::VerifyingKeyBytes;
use crate::id::AccountId;
use serde::{Deserialize, Serialize};

/// Full account recovery via email token (Decision #58-Q8: 72h cooldown only; no 2FA/passkey v1).
pub const COOLDOWN_RECOVERY_SECS: i64 = 72 * 3600;

/// Email approves an in-flight link request (laptop offline).
pub const COOLDOWN_EMAIL_LINK_SECS: i64 = 24 * 3600;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RecoveryKind {
	FullRecovery,
	EmailLinkApproval,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecoveryPending {
	pub account_id: AccountId,
	pub pending_vk: VerifyingKeyBytes,
	pub kind: RecoveryKind,
	pub usable_at_unix: i64,
	pub cancelled_at_unix: Option<i64>,
}

impl RecoveryPending {
	pub fn is_active_at(&self, now_unix: i64) -> bool {
		self.cancelled_at_unix.is_none() && now_unix >= self.usable_at_unix
	}

	pub fn in_cooldown_at(&self, now_unix: i64) -> bool {
		self.cancelled_at_unix.is_none() && now_unix < self.usable_at_unix
	}
}
