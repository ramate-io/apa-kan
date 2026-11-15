use std::sync::Arc;

use apa_kan_account::crypto::VerifyingKeyBytes;
use apa_kan_account::id::AccountId;
use apa_kan_account::link::{verify_link_approval, LinkApproval, LinkRequest, LinkRequestStatus};
use apa_kan_account::recovery::{RecoveryKind, COOLDOWN_RECOVERY_SECS};
use apa_kan_account::types::AuthorizedKeyStatus;
use apa_kan_store::model::*;
use apa_kan_store::repos::*;
use apa_kan_store::Clock;
use apa_kan_store::StoreError;
use sha3::{Digest, Sha3_256};

use crate::mailer::Mailer;

pub const STARTER_TILE_COUNT: usize = 15;
pub const DEFAULT_ELO_MILLI: i32 = 1_500_000;

pub struct AccountService<S> {
	store: Arc<S>,
	clock: Arc<dyn Clock>,
	mailer: Arc<dyn Mailer>,
}

impl<S> AccountService<S>
where
	S: Store,
{
	pub fn new(store: Arc<S>, clock: Arc<dyn Clock>, mailer: Arc<dyn Mailer>) -> Self {
		Self { store, clock, mailer }
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
		let tiles: Vec<_> =
			(1..=STARTER_TILE_COUNT as u32).map(|id| (id, "starter".to_string())).collect();
		self.store.grant_tiles(&account_id, &tiles, now).await?;
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

	pub async fn submit_link_request(&self, request: LinkRequest) -> Result<[u8; 32], StoreError> {
		request.verify_new_key_signature().map_err(StoreError::Account)?;
		let now = self.now();
		if now > request.payload.expires_at_unix {
			return Err(StoreError::Expired);
		}
		self.store
			.get_account(&request.payload.account_id)
			.await?
			.ok_or(StoreError::NotFound)?;
		self.store
			.insert_link_request(StoredLinkRequest {
				request: request.clone(),
				status: LinkRequestStatus::Open,
				created_at_unix: now,
			})
			.await?;
		Ok(request.request_id)
	}

	pub async fn approve_link(&self, approval: LinkApproval) -> Result<(), StoreError> {
		let stored = self
			.store
			.get_link_request(&approval.payload.request_id)
			.await?
			.ok_or(StoreError::NotFound)?;
		let active = self
			.store
			.is_active_key(&approval.payload.account_id, &approval.payload.approver_vk)
			.await?;
		let approver_nonce = self
			.store
			.list_keys(&approval.payload.account_id)
			.await?
			.into_iter()
			.find(|k| k.vk == approval.payload.approver_vk)
			.map(|k| k.next_nonce)
			.ok_or(StoreError::NotFound)?;
		verify_link_approval(
			&stored.request,
			stored.status,
			&approval,
			active,
			approver_nonce,
			self.now(),
		)
		.map_err(StoreError::Account)?;
		self.store
			.bump_nonce(&approval.payload.account_id, &approval.payload.approver_vk, approver_nonce)
			.await?;
		self.store
			.insert_authorized_key(AuthorizedKeyRow {
				account_id: approval.payload.account_id,
				vk: stored.request.payload.new_vk.clone(),
				label: stored.request.payload.label.clone(),
				status: AuthorizedKeyStatus::Active,
				next_nonce: 0,
				added_at_unix: self.now(),
				revoked_at_unix: None,
				usable_at_unix: None,
			})
			.await?;
		self.store
			.update_link_request_status(&approval.payload.request_id, LinkRequestStatus::Approved)
			.await?;
		self.store
			.insert_link_approval(StoredLinkApproval {
				request_id: approval.payload.request_id,
				approval,
				created_at_unix: self.now(),
			})
			.await?;
		Ok(())
	}

	pub async fn start_recovery(
		&self,
		account_id: &AccountId,
		pending_vk: VerifyingKeyBytes,
		email: &str,
	) -> Result<String, StoreError> {
		let account = self.store.get_account(account_id).await?.ok_or(StoreError::NotFound)?;
		if !account.recovery_opt_in {
			return Err(StoreError::InvalidInput("recovery not opted in".into()));
		}
		if account.email.as_deref() != Some(email) {
			return Err(StoreError::InvalidInput("email mismatch".into()));
		}
		let token = format!("tok_{}", hex::encode(rand::random::<[u8; 16]>()));
		let token_hash: [u8; 32] = Sha3_256::digest(token.as_bytes()).into();
		let now = self.now();
		let row = RecoveryRow {
			account_id: *account_id,
			pending_vk,
			token_hash,
			kind: RecoveryKind::FullRecovery,
			started_at_unix: now,
			usable_at_unix: now + COOLDOWN_RECOVERY_SECS,
			cancelled_at_unix: None,
			email: email.to_string(),
		};
		self.store.insert_recovery(row).await?;
		self.mailer.send_recovery_token(email, &account_id.to_hex(), &token);
		Ok(token)
	}

	pub async fn confirm_recovery_token(
		&self,
		account_id: &AccountId,
		token: &str,
	) -> Result<(), StoreError> {
		let row = self.store.get_active_recovery(account_id).await?.ok_or(StoreError::NotFound)?;
		let hash: [u8; 32] = Sha3_256::digest(token.as_bytes()).into();
		if hash != row.token_hash {
			return Err(StoreError::InvalidInput("bad token".into()));
		}
		Ok(())
	}

	pub async fn cancel_recovery(&self, account_id: &AccountId) -> Result<(), StoreError> {
		self.store.cancel_recovery(account_id, self.now()).await
	}

	pub async fn finish_recovery(&self, account_id: &AccountId) -> Result<(), StoreError> {
		self.store.finish_recovery(account_id, self.now()).await
	}

	pub async fn set_recovery_email(
		&self,
		account_id: &AccountId,
		email: String,
		recovery_opt_in: bool,
		approve_via_email: bool,
	) -> Result<(), StoreError> {
		self.store
			.set_email(account_id, Some(email), recovery_opt_in, approve_via_email)
			.await
	}

	pub async fn list_keys(
		&self,
		account_id: &AccountId,
	) -> Result<Vec<AuthorizedKeyRow>, StoreError> {
		self.store.list_keys(account_id).await
	}
}

#[cfg(test)]
mod tests {
	use std::sync::Arc;

	use apa_kan_account::crypto::{generate_keypair, Seed};
	use apa_kan_store::memory::MemoryStore;
	use apa_kan_store::FakeClock;

	use super::*;
	use crate::mailer::NoopMailer;

	#[tokio::test]
	async fn create_grants_starter_tiles() {
		let store = Arc::new(MemoryStore::new());
		let clock = Arc::new(FakeClock::new(100));
		let svc = AccountService::new(store, clock, Arc::new(NoopMailer));
		let (_sk, vk) = generate_keypair(&Seed::random());
		let row = svc.create_account(vk).await.unwrap();
		let (_a, tiles, rating) = svc.get_profile(&row.account_id).await.unwrap();
		assert_eq!(tiles.len(), 15);
		assert_eq!(rating.elo_milli, 1_500_000);
	}
}
