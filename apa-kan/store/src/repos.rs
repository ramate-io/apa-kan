use apa_kan_account::id::AccountId;
use async_trait::async_trait;

use crate::error::StoreError;
use crate::model::*;

#[async_trait]
pub trait AccountRepo: Send + Sync {
	async fn insert_account(&self, account: NewAccount) -> Result<AccountRow, StoreError>;
	async fn get_account(&self, id: &AccountId) -> Result<Option<AccountRow>, StoreError>;
	async fn set_email(
		&self,
		id: &AccountId,
		email: Option<String>,
		recovery_opt_in: bool,
		approve_via_email: bool,
	) -> Result<(), StoreError>;
}

#[async_trait]
pub trait KeyRepo: Send + Sync {
	async fn insert_genesis_key(&self, key: AuthorizedKeyRow) -> Result<(), StoreError>;
	async fn list_keys(&self, account_id: &AccountId) -> Result<Vec<AuthorizedKeyRow>, StoreError>;
	async fn is_active_key(
		&self,
		account_id: &AccountId,
		vk: &apa_kan_account::crypto::VerifyingKeyBytes,
	) -> Result<bool, StoreError>;
	async fn bump_nonce(
		&self,
		account_id: &AccountId,
		vk: &apa_kan_account::crypto::VerifyingKeyBytes,
		expected: u64,
	) -> Result<(), StoreError>;
	async fn insert_authorized_key(&self, key: AuthorizedKeyRow) -> Result<(), StoreError>;
	async fn insert_link_request(&self, stored: StoredLinkRequest) -> Result<(), StoreError>;
	async fn get_link_request(
		&self,
		request_id: &[u8; 32],
	) -> Result<Option<StoredLinkRequest>, StoreError>;
	async fn update_link_request_status(
		&self,
		request_id: &[u8; 32],
		status: apa_kan_account::link::LinkRequestStatus,
	) -> Result<(), StoreError>;
	async fn insert_link_approval(&self, stored: StoredLinkApproval) -> Result<(), StoreError>;
}

#[async_trait]
pub trait InventoryRepo: Send + Sync {
	async fn grant_tiles(
		&self,
		account_id: &AccountId,
		tiles: &[(TileKindId, String)],
		granted_at_unix: i64,
	) -> Result<Vec<TileInstanceRow>, StoreError>;
	async fn list_tiles(&self, account_id: &AccountId) -> Result<Vec<TileInstanceRow>, StoreError>;
}

#[async_trait]
pub trait RatingRepo: Send + Sync {
	async fn upsert_default_rating(
		&self,
		account_id: &AccountId,
		elo_milli: i32,
	) -> Result<RatingRow, StoreError>;
	async fn get_rating(&self, account_id: &AccountId) -> Result<Option<RatingRow>, StoreError>;
}

#[async_trait]
pub trait RecoveryRepo: Send + Sync {
	async fn insert_recovery(&self, row: RecoveryRow) -> Result<(), StoreError>;
	async fn get_active_recovery(
		&self,
		account_id: &AccountId,
	) -> Result<Option<RecoveryRow>, StoreError>;
	async fn cancel_recovery(
		&self,
		account_id: &AccountId,
		now_unix: i64,
	) -> Result<(), StoreError>;
	async fn finish_recovery(
		&self,
		account_id: &AccountId,
		now_unix: i64,
	) -> Result<(), StoreError>;
}

#[async_trait]
pub trait Store:
	AccountRepo + KeyRepo + InventoryRepo + RatingRepo + RecoveryRepo + Send + Sync
{
}
