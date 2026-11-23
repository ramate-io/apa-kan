use std::collections::HashMap;
use std::sync::Mutex;

use apa_kan_account::crypto::VerifyingKeyBytes;
use apa_kan_account::id::AccountId;
use apa_kan_account::link::LinkRequestStatus;
use apa_kan_account::types::AuthorizedKeyStatus;
use async_trait::async_trait;

use crate::ann::AnnHit;
use crate::catalog::*;
use crate::error::StoreError;
use crate::ids::validate_character_id;
use crate::model::*;
use crate::pass::{PassBook, PassPointer, ROLE_ADMIN};
use crate::repos::*;
use apa_kan_core::character_pass::{CoreKey, SnapshotCharacterTables, SparseCharacterRow};
use std::collections::BTreeMap;

#[derive(Default)]
pub struct MemoryStore {
	inner: Mutex<MemoryState>,
}

#[derive(Default)]
struct MemoryState {
	accounts: HashMap<[u8; 32], AccountRow>,
	keys: Vec<AuthorizedKeyRow>,
	link_requests: HashMap<[u8; 32], StoredLinkRequest>,
	tiles: Vec<TileInstanceRow>,
	next_tile_id: u64,
	ratings: HashMap<[u8; 32], RatingRow>,
	recoveries: Vec<RecoveryRow>,
	characters: HashMap<String, CharacterRecord>,
	roles: Vec<(AccountId, String, i64)>,
	pass: PassBook,
	core_tiles: HashMap<String, CoreTileRecord>,
	tile_types: HashMap<String, TileTypeRecord>,
}

impl MemoryStore {
	pub fn new() -> Self {
		Self::default()
	}
}

#[async_trait]
impl AccountRepo for MemoryStore {
	async fn insert_account(&self, account: NewAccount) -> Result<AccountRow, StoreError> {
		let mut st = self.inner.lock().unwrap();
		let id = *account.account_id.as_bytes();
		if st.accounts.contains_key(&id) {
			return Err(StoreError::Conflict("account exists".into()));
		}
		let row = AccountRow {
			account_id: account.account_id,
			genesis_vk: account.genesis_vk,
			email: None,
			email_verified: false,
			recovery_opt_in: false,
			approve_via_email: false,
			created_at_unix: account.created_at_unix,
		};
		st.accounts.insert(id, row.clone());
		Ok(row)
	}

	async fn get_account(&self, id: &AccountId) -> Result<Option<AccountRow>, StoreError> {
		let st = self.inner.lock().unwrap();
		Ok(st.accounts.get(id.as_bytes()).cloned())
	}

	async fn set_email(
		&self,
		id: &AccountId,
		email: Option<String>,
		recovery_opt_in: bool,
		approve_via_email: bool,
	) -> Result<(), StoreError> {
		let mut st = self.inner.lock().unwrap();
		let acc = st.accounts.get_mut(id.as_bytes()).ok_or(StoreError::NotFound)?;
		let verified = email.is_some();
		acc.email = email;
		acc.recovery_opt_in = recovery_opt_in;
		acc.approve_via_email = approve_via_email;
		acc.email_verified = verified;
		Ok(())
	}
}

#[async_trait]
impl KeyRepo for MemoryStore {
	async fn insert_genesis_key(&self, key: AuthorizedKeyRow) -> Result<(), StoreError> {
		let mut st = self.inner.lock().unwrap();
		st.keys.push(key);
		Ok(())
	}

	async fn list_keys(&self, account_id: &AccountId) -> Result<Vec<AuthorizedKeyRow>, StoreError> {
		let st = self.inner.lock().unwrap();
		Ok(st.keys.iter().filter(|k| k.account_id == *account_id).cloned().collect())
	}

	async fn is_active_key(
		&self,
		account_id: &AccountId,
		vk: &VerifyingKeyBytes,
	) -> Result<bool, StoreError> {
		let st = self.inner.lock().unwrap();
		Ok(st.keys.iter().any(|k| {
			k.account_id == *account_id && k.vk == *vk && k.status == AuthorizedKeyStatus::Active
		}))
	}

	async fn bump_nonce(
		&self,
		account_id: &AccountId,
		vk: &VerifyingKeyBytes,
		expected: u64,
	) -> Result<(), StoreError> {
		let mut st = self.inner.lock().unwrap();
		let key = st
			.keys
			.iter_mut()
			.find(|k| k.account_id == *account_id && k.vk == *vk)
			.ok_or(StoreError::NotFound)?;
		if key.next_nonce != expected {
			return Err(StoreError::BadNonce);
		}
		key.next_nonce = expected.saturating_add(1);
		Ok(())
	}

	async fn insert_authorized_key(&self, key: AuthorizedKeyRow) -> Result<(), StoreError> {
		let mut st = self.inner.lock().unwrap();
		st.keys.push(key);
		Ok(())
	}

	async fn insert_link_request(&self, stored: StoredLinkRequest) -> Result<(), StoreError> {
		let mut st = self.inner.lock().unwrap();
		if st.link_requests.contains_key(&stored.request.request_id) {
			return Err(StoreError::Conflict("link request exists".into()));
		}
		st.link_requests.insert(stored.request.request_id, stored);
		Ok(())
	}

	async fn get_link_request(
		&self,
		request_id: &[u8; 32],
	) -> Result<Option<StoredLinkRequest>, StoreError> {
		let st = self.inner.lock().unwrap();
		Ok(st.link_requests.get(request_id).cloned())
	}

	async fn update_link_request_status(
		&self,
		request_id: &[u8; 32],
		status: LinkRequestStatus,
	) -> Result<(), StoreError> {
		let mut st = self.inner.lock().unwrap();
		let req = st.link_requests.get_mut(request_id).ok_or(StoreError::NotFound)?;
		req.status = status;
		Ok(())
	}

	async fn insert_link_approval(&self, stored: StoredLinkApproval) -> Result<(), StoreError> {
		let _ = stored;
		Ok(())
	}
}

#[async_trait]
impl InventoryRepo for MemoryStore {
	async fn grant_tiles(
		&self,
		account_id: &AccountId,
		tiles: &[TileGrant],
		granted_at_unix: i64,
	) -> Result<Vec<TileInstanceRow>, StoreError> {
		let mut st = self.inner.lock().unwrap();
		let trade_bind = granted_at_unix + crate::model::TRADE_BIND_SECS;
		let mut out = Vec::new();
		for grant in tiles {
			st.next_tile_id += 1;
			let row = TileInstanceRow {
				instance_id: st.next_tile_id,
				account_id: *account_id,
				tile_kind_id: grant.tile_kind_id,
				origin: grant.origin.clone(),
				frozen: false,
				trade_bind_until_unix: Some(trade_bind),
				generation_id: grant.generation_id,
				core_tile_id: grant.core_tile_id.clone(),
				tile_type_id: grant.tile_type_id.clone(),
				character_id: grant.character_id.clone(),
				pass_version: grant.pass_version,
			};
			st.tiles.push(row.clone());
			out.push(row);
		}
		Ok(out)
	}

	async fn list_tiles(&self, account_id: &AccountId) -> Result<Vec<TileInstanceRow>, StoreError> {
		let st = self.inner.lock().unwrap();
		Ok(st.tiles.iter().filter(|t| t.account_id == *account_id).cloned().collect())
	}
}

#[async_trait]
impl RatingRepo for MemoryStore {
	async fn upsert_default_rating(
		&self,
		account_id: &AccountId,
		elo_milli: i32,
	) -> Result<RatingRow, StoreError> {
		let mut st = self.inner.lock().unwrap();
		let row = RatingRow { account_id: *account_id, elo_milli, games: 0 };
		st.ratings.insert(*account_id.as_bytes(), row.clone());
		Ok(row)
	}

	async fn get_rating(&self, account_id: &AccountId) -> Result<Option<RatingRow>, StoreError> {
		let st = self.inner.lock().unwrap();
		Ok(st.ratings.get(account_id.as_bytes()).cloned())
	}
}

#[async_trait]
impl RecoveryRepo for MemoryStore {
	async fn insert_recovery(&self, row: RecoveryRow) -> Result<(), StoreError> {
		let mut st = self.inner.lock().unwrap();
		if st
			.recoveries
			.iter()
			.any(|r| r.account_id == row.account_id && r.cancelled_at_unix.is_none())
		{
			return Err(StoreError::Conflict("recovery in flight".into()));
		}
		st.recoveries.push(row);
		Ok(())
	}

	async fn get_active_recovery(
		&self,
		account_id: &AccountId,
	) -> Result<Option<RecoveryRow>, StoreError> {
		let st = self.inner.lock().unwrap();
		Ok(st
			.recoveries
			.iter()
			.rev()
			.find(|r| r.account_id == *account_id && r.cancelled_at_unix.is_none())
			.cloned())
	}

	async fn cancel_recovery(
		&self,
		account_id: &AccountId,
		now_unix: i64,
	) -> Result<(), StoreError> {
		let mut st = self.inner.lock().unwrap();
		let row = st
			.recoveries
			.iter_mut()
			.rev()
			.find(|r| r.account_id == *account_id && r.cancelled_at_unix.is_none())
			.ok_or(StoreError::NotFound)?;
		row.cancelled_at_unix = Some(now_unix);
		Ok(())
	}

	async fn finish_recovery(
		&self,
		account_id: &AccountId,
		now_unix: i64,
	) -> Result<(), StoreError> {
		let mut st = self.inner.lock().unwrap();
		let row = st
			.recoveries
			.iter()
			.rev()
			.find(|r| r.account_id == *account_id && r.cancelled_at_unix.is_none())
			.ok_or(StoreError::NotFound)?;
		if now_unix < row.usable_at_unix {
			return Err(StoreError::Expired);
		}
		let pending_vk = row.pending_vk.clone();
		let kind = row.kind;
		let _ = kind;
		st.keys.push(AuthorizedKeyRow {
			account_id: *account_id,
			vk: pending_vk,
			label: "recovery".into(),
			status: AuthorizedKeyStatus::Active,
			next_nonce: 0,
			added_at_unix: now_unix,
			revoked_at_unix: None,
			usable_at_unix: None,
		});
		if let Some(r) = st
			.recoveries
			.iter_mut()
			.rev()
			.find(|r| r.account_id == *account_id && r.cancelled_at_unix.is_none())
		{
			r.cancelled_at_unix = Some(now_unix);
		}
		Ok(())
	}
}

#[async_trait]
impl CatalogRepo for MemoryStore {
	async fn upsert_character(&self, row: CharacterRecord) -> Result<(), StoreError> {
		validate_character_id(&row.character_id)?;
		if row.display_name.trim().is_empty() {
			return Err(StoreError::InvalidInput("display_name is empty".into()));
		}
		let mut st = self.inner.lock().unwrap();
		st.characters.insert(row.character_id.clone(), row);
		Ok(())
	}

	async fn get_character(&self, id: &str) -> Result<Option<CharacterRecord>, StoreError> {
		let st = self.inner.lock().unwrap();
		Ok(st.characters.get(id).cloned())
	}

	async fn list_characters(&self) -> Result<Vec<CharacterRecord>, StoreError> {
		let st = self.inner.lock().unwrap();
		let mut rows: Vec<_> = st.characters.values().cloned().collect();
		rows.sort_by(|a, b| a.character_id.cmp(&b.character_id));
		Ok(rows)
	}

	async fn set_character_image(&self, id: &str, png: Vec<u8>) -> Result<(), StoreError> {
		let mut st = self.inner.lock().unwrap();
		let rec = st.characters.get_mut(id).ok_or(StoreError::NotFound)?;
		rec.image_png = Some(png);
		Ok(())
	}
}

#[async_trait]
impl PassRepo for MemoryStore {
	async fn pass_pointer(&self) -> Result<PassPointer, StoreError> {
		Ok(self.inner.lock().unwrap().pass.pointer.clone())
	}

	async fn load_tables(&self, version: u16) -> Result<SnapshotCharacterTables, StoreError> {
		self.inner.lock().unwrap().pass.snapshot(version)
	}

	async fn upsert_draft_embedding(&self, row: SparseCharacterRow) -> Result<u16, StoreError> {
		self.inner.lock().unwrap().pass.upsert_embedding(row)
	}

	async fn set_draft_fallback(&self, ids: Vec<String>) -> Result<u16, StoreError> {
		self.inner.lock().unwrap().pass.set_fallback(ids)
	}

	async fn set_draft_specific(
		&self,
		maps: BTreeMap<CoreKey, Vec<String>>,
	) -> Result<u16, StoreError> {
		self.inner.lock().unwrap().pass.set_specific(maps)
	}

	async fn set_draft_config(&self, tau: f32, k: Option<usize>) -> Result<u16, StoreError> {
		self.inner.lock().unwrap().pass.set_config(tau, k)
	}

	async fn publish_draft(&self) -> Result<u16, StoreError> {
		self.inner.lock().unwrap().pass.publish()
	}

	async fn seed_snapshot(&self, tables: SnapshotCharacterTables) -> Result<(), StoreError> {
		let mut st = self.inner.lock().unwrap();
		if !st.pass.embeddings.is_empty() {
			return Ok(());
		}
		st.pass.seed_from_tables(&tables)
	}
}

#[async_trait]
impl AnnLookup for MemoryStore {
	async fn nearest(
		&self,
		version: u16,
		query: &[f32],
		tau: f32,
		k: Option<usize>,
	) -> Result<Vec<AnnHit>, StoreError> {
		Ok(self.inner.lock().unwrap().pass.nearest(version, query, tau, k))
	}
}

#[async_trait]
impl RoleRepo for MemoryStore {
	async fn grant_role(
		&self,
		account_id: &AccountId,
		role: &str,
		now_unix: i64,
	) -> Result<(), StoreError> {
		let mut st = self.inner.lock().unwrap();
		if st.roles.iter().any(|(id, r, _)| id == account_id && r == role) {
			return Ok(());
		}
		st.roles.push((*account_id, role.to_string(), now_unix));
		Ok(())
	}

	async fn has_role(&self, account_id: &AccountId, role: &str) -> Result<bool, StoreError> {
		let st = self.inner.lock().unwrap();
		Ok(st.roles.iter().any(|(id, r, _)| id == account_id && r == role))
	}

	async fn admin_count(&self) -> Result<u64, StoreError> {
		let st = self.inner.lock().unwrap();
		Ok(st.roles.iter().filter(|(_, r, _)| r == ROLE_ADMIN).count() as u64)
	}
}

#[async_trait]
impl IssuedTileRepo for MemoryStore {
	async fn list_issued(&self, filter: TileFilter) -> Result<Vec<TileInstanceRow>, StoreError> {
		let st = self.inner.lock().unwrap();
		let mut rows: Vec<_> = st
			.tiles
			.iter()
			.filter(|t| {
				filter.character_id.as_ref().map(|c| t.character_id == *c).unwrap_or(true)
					&& filter.core_tile_id.as_ref().map(|c| t.core_tile_id == *c).unwrap_or(true)
					&& filter.tile_type_id.as_ref().map(|c| t.tile_type_id == *c).unwrap_or(true)
					&& filter.account_id.as_ref().map(|a| t.account_id == *a).unwrap_or(true)
			})
			.cloned()
			.collect();
		rows.sort_by_key(|t| t.instance_id);
		let offset = filter.offset.unwrap_or(0) as usize;
		if offset > rows.len() {
			return Ok(Vec::new());
		}
		rows = rows.split_off(offset);
		if let Some(limit) = filter.limit {
			rows.truncate(limit as usize);
		}
		Ok(rows)
	}

	async fn upsert_core_tile(&self, rec: CoreTileRecord) -> Result<(), StoreError> {
		let mut st = self.inner.lock().unwrap();
		st.core_tiles.insert(rec.core_tile_id.clone(), rec);
		Ok(())
	}

	async fn upsert_tile_type(&self, rec: TileTypeRecord) -> Result<(), StoreError> {
		let mut st = self.inner.lock().unwrap();
		st.tile_types.insert(rec.tile_type_id.clone(), rec);
		Ok(())
	}

	async fn get_core_tile(&self, id: &str) -> Result<Option<CoreTileRecord>, StoreError> {
		Ok(self.inner.lock().unwrap().core_tiles.get(id).cloned())
	}

	async fn get_tile_type(&self, id: &str) -> Result<Option<TileTypeRecord>, StoreError> {
		Ok(self.inner.lock().unwrap().tile_types.get(id).cloned())
	}
}

#[async_trait]
impl Store for MemoryStore {}

#[cfg(test)]
mod tests {
	use super::*;
	use apa_kan_account::id::AccountId;

	#[tokio::test]
	async fn memory_insert_account() {
		let store = MemoryStore::new();
		let id = AccountId::from_bytes([1u8; 32]);
		let vk = VerifyingKeyBytes(vec![2, 3]);
		let row = store
			.insert_account(NewAccount { account_id: id, genesis_vk: vk, created_at_unix: 0 })
			.await
			.unwrap();
		assert_eq!(row.account_id, id);
	}

	#[tokio::test]
	async fn draft_edit_does_not_change_published_until_publish() {
		use crate::seed::seed_bundled_v1;
		use apa_kan_core::character_pass::{
			assign_character, JsonCharacterTables, SparseCharacterRow,
		};
		use apa_kan_core::core_pass::{CorePass, TileId};
		use apa_kan_core::game::TileColor;
		use std::collections::BTreeMap;

		let store = MemoryStore::new();
		seed_bundled_v1(&store, 0).await.unwrap();
		let json = JsonCharacterTables::load_v1().unwrap();
		let published = store.load_tables(1).await.unwrap();
		let cfg = CorePass::base();
		let tile = cfg.run(TileId(3));
		let before = assign_character(&published, &tile, 3);
		assert_eq!(before, assign_character(&json, &tile, 3));

		store
			.upsert_draft_embedding(SparseCharacterRow {
				id: "westerlit".into(),
				color: BTreeMap::from([(TileColor::Maroon, 4.0)]),
				ring_type: BTreeMap::new(),
				emblem: BTreeMap::new(),
				n_rings: 0.0,
				capture: BTreeMap::new(),
				dots: 0.0,
			})
			.await
			.unwrap();
		let pointer = store.pass_pointer().await.unwrap();
		assert_eq!(pointer.published_version, Some(1));
		assert_eq!(pointer.draft_version, 2);
		let still = store.load_tables(1).await.unwrap();
		assert_eq!(assign_character(&still, &tile, 3), before);
		store.publish_draft().await.unwrap();
		assert_eq!(store.pass_pointer().await.unwrap().published_version, Some(2));
	}
}
