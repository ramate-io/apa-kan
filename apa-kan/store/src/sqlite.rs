use apa_kan_account::crypto::VerifyingKeyBytes;
use apa_kan_account::id::AccountId;
use apa_kan_account::link::{LinkRequest, LinkRequestPayload, LinkRequestStatus};
use apa_kan_account::recovery::RecoveryKind;
use apa_kan_account::types::AuthorizedKeyStatus;
use async_trait::async_trait;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePool, SqlitePoolOptions};
use sqlx::Row;
use std::str::FromStr;

use crate::error::StoreError;
use crate::model::*;
use crate::repos::*;

const MIGRATION_001: &str = include_str!("../migrations/001_platform_v1.sql");

pub struct SqliteStore {
	pool: SqlitePool,
}

impl SqliteStore {
	pub async fn connect_in_memory() -> Result<Self, StoreError> {
		let options = SqliteConnectOptions::from_str("sqlite::memory:")
			.map_err(|e| StoreError::Storage(e.to_string()))?
			.create_if_missing(true);
		let pool = SqlitePoolOptions::new()
			.max_connections(1)
			.connect_with(options)
			.await
			.map_err(|e| StoreError::Storage(e.to_string()))?;
		sqlx::raw_sql(MIGRATION_001)
			.execute(&pool)
			.await
			.map_err(|e| StoreError::Storage(e.to_string()))?;
		Ok(Self { pool })
	}

	pub async fn connect_file(path: &str) -> Result<Self, StoreError> {
		let options = SqliteConnectOptions::from_str(&format!("sqlite:{path}"))
			.map_err(|e| StoreError::Storage(e.to_string()))?
			.create_if_missing(true);
		let pool = SqlitePoolOptions::new()
			.connect_with(options)
			.await
			.map_err(|e| StoreError::Storage(e.to_string()))?;
		sqlx::raw_sql(MIGRATION_001)
			.execute(&pool)
			.await
			.map_err(|e| StoreError::Storage(e.to_string()))?;
		Ok(Self { pool })
	}
}

fn status_str(s: &AuthorizedKeyStatus) -> &'static str {
	match s {
		AuthorizedKeyStatus::PendingRecovery => "pending_recovery",
		AuthorizedKeyStatus::Active => "active",
		AuthorizedKeyStatus::Revoked => "revoked",
	}
}

fn status_from_str(s: &str) -> Result<AuthorizedKeyStatus, StoreError> {
	match s {
		"pending_recovery" => Ok(AuthorizedKeyStatus::PendingRecovery),
		"active" => Ok(AuthorizedKeyStatus::Active),
		"revoked" => Ok(AuthorizedKeyStatus::Revoked),
		_ => Err(StoreError::Storage(format!("bad key status {s}"))),
	}
}

fn link_status_str(s: &LinkRequestStatus) -> &'static str {
	match s {
		LinkRequestStatus::Open => "open",
		LinkRequestStatus::Approved => "approved",
		LinkRequestStatus::Rejected => "rejected",
		LinkRequestStatus::Expired => "expired",
		LinkRequestStatus::Cancelled => "cancelled",
	}
}

fn link_status_from(s: &str) -> Result<LinkRequestStatus, StoreError> {
	match s {
		"open" => Ok(LinkRequestStatus::Open),
		"approved" => Ok(LinkRequestStatus::Approved),
		"rejected" => Ok(LinkRequestStatus::Rejected),
		"expired" => Ok(LinkRequestStatus::Expired),
		"cancelled" => Ok(LinkRequestStatus::Cancelled),
		_ => Err(StoreError::Storage(format!("bad link status {s}"))),
	}
}

fn blob32(row: &[u8]) -> Result<[u8; 32], StoreError> {
	if row.len() != 32 {
		return Err(StoreError::Storage("expected 32-byte blob".into()));
	}
	let mut out = [0u8; 32];
	out.copy_from_slice(row);
	Ok(out)
}

fn decode_link_request(
	request_id: [u8; 32],
	payload_json: &str,
	new_sig: Vec<u8>,
) -> Result<LinkRequest, StoreError> {
	let payload: LinkRequestPayload =
		serde_json::from_str(payload_json).map_err(|e| StoreError::Storage(e.to_string()))?;
	Ok(LinkRequest {
		request_id,
		payload,
		new_key_sig: apa_kan_account::crypto::SignatureBytes(new_sig),
	})
}

#[async_trait]
impl AccountRepo for SqliteStore {
	async fn insert_account(&self, account: NewAccount) -> Result<AccountRow, StoreError> {
		let result = sqlx::query(
			r#"INSERT INTO accounts (account_id, genesis_vk, created_at_unix)
               VALUES (?, ?, ?)"#,
		)
		.bind(account.account_id.as_bytes().as_slice())
		.bind(account.genesis_vk.as_slice())
		.bind(account.created_at_unix)
		.execute(&self.pool)
		.await;

		match result {
			Ok(_) => Ok(AccountRow {
				account_id: account.account_id,
				genesis_vk: account.genesis_vk,
				email: None,
				email_verified: false,
				recovery_opt_in: false,
				approve_via_email: false,
				created_at_unix: account.created_at_unix,
			}),
			Err(e) => {
				if let sqlx::Error::Database(db) = &e {
					if db.is_unique_violation() {
						return Err(StoreError::Conflict("account exists".into()));
					}
				}
				Err(StoreError::Storage(e.to_string()))
			}
		}
	}

	async fn get_account(&self, id: &AccountId) -> Result<Option<AccountRow>, StoreError> {
		let row = sqlx::query(
			r#"SELECT genesis_vk, email, email_verified, recovery_opt_in, approve_via_email, created_at_unix
               FROM accounts WHERE account_id = ?"#,
		)
		.bind(id.as_bytes().as_slice())
		.fetch_optional(&self.pool)
		.await
		.map_err(|e| StoreError::Storage(e.to_string()))?;

		let Some(row) = row else { return Ok(None) };
		Ok(Some(AccountRow {
			account_id: *id,
			genesis_vk: VerifyingKeyBytes(row.get::<Vec<u8>, _>(0)),
			email: row.get(1),
			email_verified: row.get::<i64, _>(2) != 0,
			recovery_opt_in: row.get::<i64, _>(3) != 0,
			approve_via_email: row.get::<i64, _>(4) != 0,
			created_at_unix: row.get(5),
		}))
	}

	async fn set_email(
		&self,
		id: &AccountId,
		email: Option<String>,
		recovery_opt_in: bool,
		approve_via_email: bool,
	) -> Result<(), StoreError> {
		let verified = email.is_some();
		let updated = sqlx::query(
			r#"UPDATE accounts SET email = ?, email_verified = ?, recovery_opt_in = ?, approve_via_email = ?
               WHERE account_id = ?"#,
		)
		.bind(&email)
		.bind(verified as i64)
		.bind(recovery_opt_in as i64)
		.bind(approve_via_email as i64)
		.bind(id.as_bytes().as_slice())
		.execute(&self.pool)
		.await
		.map_err(|e| StoreError::Storage(e.to_string()))?;
		if updated.rows_affected() == 0 {
			return Err(StoreError::NotFound);
		}
		Ok(())
	}
}

#[async_trait]
impl KeyRepo for SqliteStore {
	async fn insert_genesis_key(&self, key: AuthorizedKeyRow) -> Result<(), StoreError> {
		sqlx::query(
			r#"INSERT INTO authorized_keys
               (account_id, vk, label, status, next_nonce, added_at_unix, revoked_at_unix, usable_at_unix)
               VALUES (?, ?, ?, ?, ?, ?, ?, ?)"#,
		)
		.bind(key.account_id.as_bytes().as_slice())
		.bind(key.vk.as_slice())
		.bind(&key.label)
		.bind(status_str(&key.status))
		.bind(key.next_nonce as i64)
		.bind(key.added_at_unix)
		.bind(key.revoked_at_unix)
		.bind(key.usable_at_unix)
		.execute(&self.pool)
		.await
		.map_err(|e| StoreError::Storage(e.to_string()))?;
		Ok(())
	}

	async fn list_keys(&self, account_id: &AccountId) -> Result<Vec<AuthorizedKeyRow>, StoreError> {
		let rows = sqlx::query(
			r#"SELECT vk, label, status, next_nonce, added_at_unix, revoked_at_unix, usable_at_unix
               FROM authorized_keys WHERE account_id = ?"#,
		)
		.bind(account_id.as_bytes().as_slice())
		.fetch_all(&self.pool)
		.await
		.map_err(|e| StoreError::Storage(e.to_string()))?;

		rows.into_iter()
			.map(|row| {
				Ok(AuthorizedKeyRow {
					account_id: *account_id,
					vk: VerifyingKeyBytes(row.get(0)),
					label: row.get(1),
					status: status_from_str(&row.get::<String, _>(2))?,
					next_nonce: row.get::<i64, _>(3) as u64,
					added_at_unix: row.get(4),
					revoked_at_unix: row.get(5),
					usable_at_unix: row.get(6),
				})
			})
			.collect()
	}

	async fn is_active_key(
		&self,
		account_id: &AccountId,
		vk: &VerifyingKeyBytes,
	) -> Result<bool, StoreError> {
		let count: (i64,) = sqlx::query_as(
			r#"SELECT COUNT(*) FROM authorized_keys
               WHERE account_id = ? AND vk = ? AND status = 'active'"#,
		)
		.bind(account_id.as_bytes().as_slice())
		.bind(vk.as_slice())
		.fetch_one(&self.pool)
		.await
		.map_err(|e| StoreError::Storage(e.to_string()))?;
		Ok(count.0 > 0)
	}

	async fn bump_nonce(
		&self,
		account_id: &AccountId,
		vk: &VerifyingKeyBytes,
		expected: u64,
	) -> Result<(), StoreError> {
		let updated = sqlx::query(
			r#"UPDATE authorized_keys SET next_nonce = ?
               WHERE account_id = ? AND vk = ? AND next_nonce = ? AND status = 'active'"#,
		)
		.bind((expected + 1) as i64)
		.bind(account_id.as_bytes().as_slice())
		.bind(vk.as_slice())
		.bind(expected as i64)
		.execute(&self.pool)
		.await
		.map_err(|e| StoreError::Storage(e.to_string()))?;
		if updated.rows_affected() == 0 {
			return Err(StoreError::BadNonce);
		}
		Ok(())
	}

	async fn insert_authorized_key(&self, key: AuthorizedKeyRow) -> Result<(), StoreError> {
		sqlx::query(
			r#"INSERT INTO authorized_keys
               (account_id, vk, label, status, next_nonce, added_at_unix, revoked_at_unix, usable_at_unix)
               VALUES (?, ?, ?, ?, ?, ?, ?, ?)"#,
		)
		.bind(key.account_id.as_bytes().as_slice())
		.bind(key.vk.as_slice())
		.bind(&key.label)
		.bind(status_str(&key.status))
		.bind(key.next_nonce as i64)
		.bind(key.added_at_unix)
		.bind(key.revoked_at_unix)
		.bind(key.usable_at_unix)
		.execute(&self.pool)
		.await
		.map_err(|e| StoreError::Storage(e.to_string()))?;
		Ok(())
	}

	async fn insert_link_request(&self, stored: StoredLinkRequest) -> Result<(), StoreError> {
		let payload_json = serde_json::to_string(&stored.request.payload)
			.map_err(|e| StoreError::Storage(e.to_string()))?;
		sqlx::query(
			r#"INSERT INTO link_requests
               (request_id, account_id, payload_json, new_sig, status, created_at_unix, expires_at_unix)
               VALUES (?, ?, ?, ?, ?, ?, ?)"#,
		)
		.bind(&stored.request.request_id[..])
		.bind(stored.request.payload.account_id.as_bytes().as_slice())
		.bind(payload_json)
		.bind(stored.request.new_key_sig.0.as_slice())
		.bind(link_status_str(&stored.status))
		.bind(stored.created_at_unix)
		.bind(stored.request.payload.expires_at_unix)
		.execute(&self.pool)
		.await
		.map_err(|e| StoreError::Storage(e.to_string()))?;
		Ok(())
	}

	async fn get_link_request(
		&self,
		request_id: &[u8; 32],
	) -> Result<Option<StoredLinkRequest>, StoreError> {
		let row = sqlx::query(
			r#"SELECT payload_json, new_sig, status, created_at_unix FROM link_requests WHERE request_id = ?"#,
		)
		.bind(&request_id[..])
		.fetch_optional(&self.pool)
		.await
		.map_err(|e| StoreError::Storage(e.to_string()))?;

		let Some(row) = row else { return Ok(None) };
		let request =
			decode_link_request(*request_id, row.get::<String, _>(0).as_str(), row.get(1))?;
		Ok(Some(StoredLinkRequest {
			request,
			status: link_status_from(&row.get::<String, _>(2))?,
			created_at_unix: row.get(3),
		}))
	}

	async fn update_link_request_status(
		&self,
		request_id: &[u8; 32],
		status: LinkRequestStatus,
	) -> Result<(), StoreError> {
		let updated = sqlx::query(r#"UPDATE link_requests SET status = ? WHERE request_id = ?"#)
			.bind(link_status_str(&status))
			.bind(&request_id[..])
			.execute(&self.pool)
			.await
			.map_err(|e| StoreError::Storage(e.to_string()))?;
		if updated.rows_affected() == 0 {
			return Err(StoreError::NotFound);
		}
		Ok(())
	}

	async fn insert_link_approval(&self, _stored: StoredLinkApproval) -> Result<(), StoreError> {
		Ok(())
	}
}

#[async_trait]
impl InventoryRepo for SqliteStore {
	async fn grant_tiles(
		&self,
		account_id: &AccountId,
		tiles: &[(TileKindId, String)],
	) -> Result<Vec<TileInstanceRow>, StoreError> {
		let mut out = Vec::new();
		for (kind, origin) in tiles {
			let row = sqlx::query(
				r#"INSERT INTO tile_instances (account_id, tile_kind_id, origin, frozen)
                   VALUES (?, ?, ?, 0)
                   RETURNING instance_id"#,
			)
			.bind(account_id.as_bytes().as_slice())
			.bind(*kind as i64)
			.bind(origin)
			.fetch_one(&self.pool)
			.await
			.map_err(|e| StoreError::Storage(e.to_string()))?;
			let instance_id: i64 = row.get(0);
			out.push(TileInstanceRow {
				instance_id: instance_id as u64,
				account_id: *account_id,
				tile_kind_id: *kind,
				origin: origin.clone(),
				frozen: false,
			});
		}
		Ok(out)
	}

	async fn list_tiles(&self, account_id: &AccountId) -> Result<Vec<TileInstanceRow>, StoreError> {
		let rows = sqlx::query(
			r#"SELECT instance_id, tile_kind_id, origin, frozen FROM tile_instances WHERE account_id = ?"#,
		)
		.bind(account_id.as_bytes().as_slice())
		.fetch_all(&self.pool)
		.await
		.map_err(|e| StoreError::Storage(e.to_string()))?;

		Ok(rows
			.into_iter()
			.map(|row| TileInstanceRow {
				instance_id: row.get::<i64, _>(0) as u64,
				account_id: *account_id,
				tile_kind_id: row.get::<i64, _>(1) as u32,
				origin: row.get(2),
				frozen: row.get::<i64, _>(3) != 0,
			})
			.collect())
	}
}

#[async_trait]
impl RatingRepo for SqliteStore {
	async fn upsert_default_rating(
		&self,
		account_id: &AccountId,
		elo_milli: i32,
	) -> Result<RatingRow, StoreError> {
		sqlx::query(
			r#"INSERT INTO ratings (account_id, elo_milli, games) VALUES (?, ?, 0)
               ON CONFLICT(account_id) DO UPDATE SET elo_milli = excluded.elo_milli"#,
		)
		.bind(account_id.as_bytes().as_slice())
		.bind(elo_milli)
		.execute(&self.pool)
		.await
		.map_err(|e| StoreError::Storage(e.to_string()))?;
		Ok(RatingRow { account_id: *account_id, elo_milli, games: 0 })
	}

	async fn get_rating(&self, account_id: &AccountId) -> Result<Option<RatingRow>, StoreError> {
		let row = sqlx::query(r#"SELECT elo_milli, games FROM ratings WHERE account_id = ?"#)
			.bind(account_id.as_bytes().as_slice())
			.fetch_optional(&self.pool)
			.await
			.map_err(|e| StoreError::Storage(e.to_string()))?;
		Ok(row.map(|row| RatingRow {
			account_id: *account_id,
			elo_milli: row.get(0),
			games: row.get::<i64, _>(1) as u32,
		}))
	}
}

#[async_trait]
impl RecoveryRepo for SqliteStore {
	async fn insert_recovery(&self, row: RecoveryRow) -> Result<(), StoreError> {
		sqlx::query(
			r#"INSERT INTO recovery_pending
               (account_id, pending_vk, token_hash, kind, started_at_unix, usable_at_unix, email)
               VALUES (?, ?, ?, ?, ?, ?, ?)"#,
		)
		.bind(row.account_id.as_bytes().as_slice())
		.bind(row.pending_vk.as_slice())
		.bind(&row.token_hash[..])
		.bind(match row.kind {
			RecoveryKind::FullRecovery => "full_recovery",
			RecoveryKind::EmailLinkApproval => "email_link_approval",
		})
		.bind(row.started_at_unix)
		.bind(row.usable_at_unix)
		.bind(&row.email)
		.execute(&self.pool)
		.await
		.map_err(|e| StoreError::Storage(e.to_string()))?;
		Ok(())
	}

	async fn get_active_recovery(
		&self,
		account_id: &AccountId,
	) -> Result<Option<RecoveryRow>, StoreError> {
		let row = sqlx::query(
			r#"SELECT pending_vk, token_hash, kind, started_at_unix, usable_at_unix, cancelled_at_unix, email
               FROM recovery_pending WHERE account_id = ? AND cancelled_at_unix IS NULL
               ORDER BY id DESC LIMIT 1"#,
		)
		.bind(account_id.as_bytes().as_slice())
		.fetch_optional(&self.pool)
		.await
		.map_err(|e| StoreError::Storage(e.to_string()))?;
		let Some(row) = row else { return Ok(None) };
		let kind = match row.get::<String, _>(2).as_str() {
			"full_recovery" => RecoveryKind::FullRecovery,
			_ => RecoveryKind::EmailLinkApproval,
		};
		Ok(Some(RecoveryRow {
			account_id: *account_id,
			pending_vk: VerifyingKeyBytes(row.get(0)),
			token_hash: blob32(row.get::<Vec<u8>, _>(1).as_slice())?,
			kind,
			started_at_unix: row.get(3),
			usable_at_unix: row.get(4),
			cancelled_at_unix: row.get(5),
			email: row.get(6),
		}))
	}

	async fn cancel_recovery(
		&self,
		account_id: &AccountId,
		now_unix: i64,
	) -> Result<(), StoreError> {
		let updated = sqlx::query(
			r#"UPDATE recovery_pending SET cancelled_at_unix = ?
               WHERE account_id = ? AND cancelled_at_unix IS NULL"#,
		)
		.bind(now_unix)
		.bind(account_id.as_bytes().as_slice())
		.execute(&self.pool)
		.await
		.map_err(|e| StoreError::Storage(e.to_string()))?;
		if updated.rows_affected() == 0 {
			return Err(StoreError::NotFound);
		}
		Ok(())
	}

	async fn finish_recovery(
		&self,
		account_id: &AccountId,
		now_unix: i64,
	) -> Result<(), StoreError> {
		let row = self.get_active_recovery(account_id).await?.ok_or(StoreError::NotFound)?;
		if now_unix < row.usable_at_unix {
			return Err(StoreError::Expired);
		}
		self.insert_authorized_key(AuthorizedKeyRow {
			account_id: *account_id,
			vk: row.pending_vk,
			label: "recovery".into(),
			status: AuthorizedKeyStatus::Active,
			next_nonce: 0,
			added_at_unix: now_unix,
			revoked_at_unix: None,
			usable_at_unix: None,
		})
		.await?;
		self.cancel_recovery(account_id, now_unix).await?;
		Ok(())
	}
}

#[async_trait]
impl Store for SqliteStore {}

#[cfg(test)]
mod tests {
	use super::*;
	use apa_kan_account::id::AccountId;

	#[tokio::test]
	async fn sqlite_migrations_and_account() {
		let store = SqliteStore::connect_in_memory().await.unwrap();
		let id = AccountId::from_bytes([8u8; 32]);
		let vk = VerifyingKeyBytes(vec![1, 2, 3]);
		store
			.insert_account(NewAccount { account_id: id, genesis_vk: vk, created_at_unix: 1 })
			.await
			.unwrap();
		assert!(store.get_account(&id).await.unwrap().is_some());
	}
}
