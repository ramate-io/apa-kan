use apa_kan_account::id::AccountId;
use apa_kan_core::character_pass::{SnapshotCharacterTables, SparseCharacterRow};
use async_trait::async_trait;
use std::collections::BTreeMap;

use crate::ann::AnnHit;
use crate::catalog::*;
use crate::error::StoreError;
use crate::model::*;
use crate::pass::PassPointer;
use apa_kan_core::character_pass::CoreKey;

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
		tiles: &[TileGrant],
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
pub trait CatalogRepo: Send + Sync {
	async fn upsert_character(&self, row: CharacterRecord) -> Result<(), StoreError>;
	async fn get_character(&self, id: &str) -> Result<Option<CharacterRecord>, StoreError>;
	async fn list_characters(&self) -> Result<Vec<CharacterRecord>, StoreError>;
	async fn set_character_image(&self, id: &str, png: Vec<u8>) -> Result<(), StoreError>;
}

#[async_trait]
pub trait PassRepo: Send + Sync {
	async fn pass_pointer(&self) -> Result<PassPointer, StoreError>;
	async fn load_tables(&self, version: u16) -> Result<SnapshotCharacterTables, StoreError>;
	async fn upsert_draft_embedding(&self, row: SparseCharacterRow) -> Result<u16, StoreError>;
	async fn set_draft_fallback(&self, ids: Vec<String>) -> Result<u16, StoreError>;
	async fn set_draft_specific(
		&self,
		maps: BTreeMap<CoreKey, Vec<String>>,
	) -> Result<u16, StoreError>;
	async fn set_draft_config(&self, tau: f32, k: Option<usize>) -> Result<u16, StoreError>;
	async fn publish_draft(&self) -> Result<u16, StoreError>;
	/// Load bundled v1 (or any snapshot) when the pass book is empty.
	async fn seed_snapshot(&self, tables: SnapshotCharacterTables) -> Result<(), StoreError>;
}

#[async_trait]
pub trait AnnLookup: Send + Sync {
	async fn nearest(
		&self,
		version: u16,
		query: &[f32],
		tau: f32,
		k: Option<usize>,
	) -> Result<Vec<AnnHit>, StoreError>;
}

#[async_trait]
pub trait RoleRepo: Send + Sync {
	async fn grant_role(
		&self,
		account_id: &AccountId,
		role: &str,
		now_unix: i64,
	) -> Result<(), StoreError>;
	async fn has_role(&self, account_id: &AccountId, role: &str) -> Result<bool, StoreError>;
	async fn admin_count(&self) -> Result<u64, StoreError>;
}

#[async_trait]
pub trait IssuedTileRepo: Send + Sync {
	async fn list_issued(&self, filter: TileFilter) -> Result<Vec<TileInstanceRow>, StoreError>;
	async fn upsert_core_tile(&self, rec: CoreTileRecord) -> Result<(), StoreError>;
	async fn upsert_tile_type(&self, rec: TileTypeRecord) -> Result<(), StoreError>;
	async fn get_core_tile(&self, id: &str) -> Result<Option<CoreTileRecord>, StoreError>;
	async fn get_tile_type(&self, id: &str) -> Result<Option<TileTypeRecord>, StoreError>;
}

#[async_trait]
pub trait Store:
	AccountRepo
	+ KeyRepo
	+ InventoryRepo
	+ RatingRepo
	+ RecoveryRepo
	+ CatalogRepo
	+ PassRepo
	+ AnnLookup
	+ RoleRepo
	+ IssuedTileRepo
	+ Send
	+ Sync
{
}
