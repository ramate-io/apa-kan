use std::collections::HashMap;
use std::sync::Mutex;

use apa_kan_account::crypto::VerifyingKeyBytes;
use apa_kan_account::id::AccountId;
use apa_kan_account::link::LinkRequestStatus;
use apa_kan_account::types::AuthorizedKeyStatus;
use async_trait::async_trait;

use crate::error::StoreError;
use crate::model::*;
use crate::repos::*;

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
		tiles: &[(TileKindId, String)],
		granted_at_unix: i64,
	) -> Result<Vec<TileInstanceRow>, StoreError> {
		let mut st = self.inner.lock().unwrap();
		let trade_bind = granted_at_unix + crate::model::TRADE_BIND_SECS;
		let mut out = Vec::new();
		for (kind, origin) in tiles {
			st.next_tile_id += 1;
			let row = TileInstanceRow {
				instance_id: st.next_tile_id,
				account_id: *account_id,
				tile_kind_id: *kind,
				origin: origin.clone(),
				frozen: false,
				trade_bind_until_unix: Some(trade_bind),
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
}
