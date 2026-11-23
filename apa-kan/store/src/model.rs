use apa_kan_account::crypto::VerifyingKeyBytes;
use apa_kan_account::id::AccountId;
use apa_kan_account::link::{LinkApproval, LinkRequest, LinkRequestStatus};
use apa_kan_account::recovery::RecoveryKind;
use apa_kan_account::types::AuthorizedKeyStatus;
use serde::{Deserialize, Serialize};

/// Inventory kind id: `apa_kan::game::Tile::kind_index` plus capture mark
/// (see `apa_kan_server::starter::inventory_kind_id`). Switch to
/// `apa-kan-core` tile identity once that crate lands.
pub type TileKindId = u32;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AccountRow {
	pub account_id: AccountId,
	pub genesis_vk: VerifyingKeyBytes,
	pub email: Option<String>,
	pub email_verified: bool,
	pub recovery_opt_in: bool,
	pub approve_via_email: bool,
	pub created_at_unix: i64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct NewAccount {
	pub account_id: AccountId,
	pub genesis_vk: VerifyingKeyBytes,
	pub created_at_unix: i64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuthorizedKeyRow {
	pub account_id: AccountId,
	pub vk: VerifyingKeyBytes,
	pub label: String,
	pub status: AuthorizedKeyStatus,
	pub next_nonce: u64,
	pub added_at_unix: i64,
	pub revoked_at_unix: Option<i64>,
	pub usable_at_unix: Option<i64>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct StoredLinkRequest {
	pub request: LinkRequest,
	pub status: LinkRequestStatus,
	pub created_at_unix: i64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct StoredLinkApproval {
	pub request_id: [u8; 32],
	pub approval: LinkApproval,
	pub created_at_unix: i64,
}

/// Decision #58-Q7: new grants are trade-bound for [`crate::TRADE_BIND_SECS`].
pub const TRADE_BIND_SECS: i64 = 48 * 3600;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TileInstanceRow {
	pub instance_id: u64,
	pub account_id: AccountId,
	pub tile_kind_id: TileKindId,
	pub origin: String,
	pub frozen: bool,
	pub trade_bind_until_unix: Option<i64>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RatingRow {
	pub account_id: AccountId,
	pub elo_milli: i32,
	pub games: u32,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecoveryRow {
	pub account_id: AccountId,
	pub pending_vk: VerifyingKeyBytes,
	pub token_hash: [u8; 32],
	pub kind: RecoveryKind,
	pub started_at_unix: i64,
	pub usable_at_unix: i64,
	pub cancelled_at_unix: Option<i64>,
	pub email: String,
}
