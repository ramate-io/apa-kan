//! Account store projection: what registration grants (starter tiles,
//! default rating) and the profile read. Keys and recovery live in the
//! `account/<id>` ledger streams ([`crate::accounts`]), not here.

use std::sync::Arc;

use apa_kan_account::crypto::VerifyingKeyBytes;
use apa_kan_account::id::AccountId;
use apa_kan_account::types::AuthorizedKeyStatus;
use apa_kan_store::model::*;
use apa_kan_store::repos::*;
use apa_kan_store::Clock;
use apa_kan_store::StoreError;

use crate::starter::starter_grants;

pub const DEFAULT_ELO_MILLI: i32 = 1_500_000;

pub struct AccountService<S> {
	store: Arc<S>,
	clock: Arc<dyn Clock>,
}

impl<S> AccountService<S>
where
	S: Store,
{
	pub fn new(store: Arc<S>, clock: Arc<dyn Clock>) -> Self {
		Self { store, clock }
	}

	fn now(&self) -> i64 {
		self.clock.now_unix()
	}

	pub async fn create_account(
		&self,
		genesis_vk: VerifyingKeyBytes,
	) -> Result<AccountRow, StoreError> {
		let account_id = AccountId::from_genesis_vk(&genesis_vk);
		let now = self.now();
		let row = self
			.store
			.insert_account(NewAccount {
				account_id,
				genesis_vk: genesis_vk.clone(),
				created_at_unix: now,
			})
			.await?;
		self.store
			.insert_genesis_key(AuthorizedKeyRow {
				account_id,
				vk: genesis_vk,
				label: "genesis".into(),
				status: AuthorizedKeyStatus::Active,
				next_nonce: 0,
				added_at_unix: now,
				revoked_at_unix: None,
				usable_at_unix: None,
			})
			.await?;
		self.store.grant_tiles(&account_id, &starter_grants(), now).await?;
		self.store.upsert_default_rating(&account_id, DEFAULT_ELO_MILLI).await?;
		Ok(row)
	}

	pub async fn get_profile(
		&self,
		id: &AccountId,
	) -> Result<(AccountRow, Vec<TileInstanceRow>, RatingRow), StoreError> {
		let account = self.store.get_account(id).await?.ok_or(StoreError::NotFound)?;
		let tiles = self.store.list_tiles(id).await?;
		let rating = self.store.get_rating(id).await?.ok_or(StoreError::NotFound)?;
		Ok((account, tiles, rating))
	}
}

/// Registration projects into the store: account row, genesis key, starter
/// tiles, default rating. Re-running for an existing account is a no-op.
#[async_trait::async_trait]
impl<S> crate::accounts::Projection for AccountService<S>
where
	S: Store + Send + Sync + 'static,
{
	async fn registered(&self, id: AccountId, genesis_vk: VerifyingKeyBytes) -> Result<(), String> {
		if self.store.get_account(&id).await.map_err(|e| e.to_string())?.is_some() {
			return Ok(());
		}
		match self.create_account(genesis_vk).await {
			Ok(_) | Err(StoreError::Conflict(_)) => Ok(()),
			Err(e) => Err(e.to_string()),
		}
	}
}

#[cfg(test)]
mod tests {
	use std::sync::Arc;

	use apa_kan_account::crypto::{generate_keypair, Seed};
	use apa_kan_store::memory::MemoryStore;
	use apa_kan_store::FakeClock;

	use super::*;
	use crate::starter::STARTER_TILE_COUNT;

	#[tokio::test]
	async fn create_grants_starter_tiles() {
		let store = Arc::new(MemoryStore::new());
		let clock = Arc::new(FakeClock::new(100));
		let svc = AccountService::new(store, clock);
		let (_sk, vk) = generate_keypair(&Seed::random());
		let row = svc.create_account(vk).await.unwrap();
		let (_a, tiles, rating) = svc.get_profile(&row.account_id).await.unwrap();
		assert_eq!(tiles.len(), STARTER_TILE_COUNT);
		assert_eq!(rating.elo_milli, 1_500_000);
		let kinds: std::collections::HashSet<_> = tiles.iter().map(|t| t.tile_kind_id).collect();
		assert_eq!(kinds.len(), STARTER_TILE_COUNT);
	}
}
