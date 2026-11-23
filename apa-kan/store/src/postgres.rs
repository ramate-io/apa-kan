//! Postgres + pgvector backend. ANN uses HNSW cosine; generation still loads
//! an exact snapshot so picks match Memory/SQLite for the same version.

use std::collections::BTreeMap;
use std::str::FromStr;

use apa_kan_account::crypto::VerifyingKeyBytes;
use apa_kan_account::id::AccountId;
use apa_kan_account::link::{LinkRequest, LinkRequestPayload, LinkRequestStatus};
use apa_kan_account::recovery::RecoveryKind;
use apa_kan_account::types::AuthorizedKeyStatus;
use apa_kan_core::character_pass::{
	encode_character, CharacterTables, CoreKey, SnapshotCharacterTables, SparseCharacterRow,
};
use async_trait::async_trait;
use pgvector::Vector;
use sqlx::postgres::{PgConnectOptions, PgPool, PgPoolOptions, PgRow};
use sqlx::Row;

use crate::ann::AnnHit;
use crate::catalog::*;
use crate::error::StoreError;
use crate::ids::validate_character_id;
use crate::model::*;
use crate::pass::{is_live, PassPointer, ROLE_ADMIN};
use crate::repos::*;

const MIGRATION: &str = include_str!("../migrations/postgres/001_platform.sql");

pub struct PostgresStore {
	pool: PgPool,
}

impl PostgresStore {
	pub async fn connect(url: &str) -> Result<Self, StoreError> {
		let options =
			PgConnectOptions::from_str(url).map_err(|e| StoreError::Storage(e.to_string()))?;
		let pool = PgPoolOptions::new()
			.max_connections(8)
			.connect_with(options)
			.await
			.map_err(|e| StoreError::Storage(e.to_string()))?;
		sqlx::raw_sql(MIGRATION)
			.execute(&pool)
			.await
			.map_err(|e| StoreError::Storage(e.to_string()))?;
		Ok(Self { pool })
	}

	async fn load_pointer(&self) -> Result<PassPointer, StoreError> {
		let row =
			sqlx::query("SELECT draft_version, published_version FROM pass_pointer WHERE id = 1")
				.fetch_optional(&self.pool)
				.await
				.map_err(|e| StoreError::Storage(e.to_string()))?
				.ok_or(StoreError::NotFound)?;
		Ok(PassPointer {
			draft_version: row.get::<i32, _>(0) as u16,
			published_version: row.get::<Option<i32>, _>(1).map(|v| v as u16),
		})
	}

	async fn save_pointer(&self, pointer: &PassPointer) -> Result<(), StoreError> {
		sqlx::query(
			"UPDATE pass_pointer SET draft_version = $1, published_version = $2 WHERE id = 1",
		)
		.bind(pointer.draft_version as i32)
		.bind(pointer.published_version.map(|v| v as i32))
		.execute(&self.pool)
		.await
		.map_err(|e| StoreError::Storage(e.to_string()))?;
		Ok(())
	}

	async fn ensure_draft(&self) -> Result<u16, StoreError> {
		let mut pointer = self.load_pointer().await?;
		pointer.ensure_draft();
		self.save_pointer(&pointer).await?;
		Ok(pointer.draft_version)
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

fn unique_or_storage(e: sqlx::Error, conflict: &str) -> StoreError {
	if let sqlx::Error::Database(db) = &e {
		if db.is_unique_violation() {
			return StoreError::Conflict(conflict.into());
		}
	}
	StoreError::Storage(e.to_string())
}

#[async_trait]
impl AccountRepo for PostgresStore {
	async fn insert_account(&self, account: NewAccount) -> Result<AccountRow, StoreError> {
		sqlx::query(
			"INSERT INTO accounts (account_id, genesis_vk, created_at_unix) VALUES ($1, $2, $3)",
		)
		.bind(account.account_id.as_bytes().as_slice())
		.bind(account.genesis_vk.as_slice())
		.bind(account.created_at_unix)
		.execute(&self.pool)
		.await
		.map_err(|e| unique_or_storage(e, "account exists"))?;
		Ok(AccountRow {
			account_id: account.account_id,
			genesis_vk: account.genesis_vk,
			email: None,
			email_verified: false,
			recovery_opt_in: false,
			approve_via_email: false,
			created_at_unix: account.created_at_unix,
		})
	}

	async fn get_account(&self, id: &AccountId) -> Result<Option<AccountRow>, StoreError> {
		let row = sqlx::query(
			r#"SELECT genesis_vk, email, email_verified, recovery_opt_in, approve_via_email, created_at_unix
               FROM accounts WHERE account_id = $1"#,
		)
		.bind(id.as_bytes().as_slice())
		.fetch_optional(&self.pool)
		.await
		.map_err(|e| StoreError::Storage(e.to_string()))?;
		let Some(row) = row else { return Ok(None) };
		Ok(Some(AccountRow {
			account_id: *id,
			genesis_vk: VerifyingKeyBytes(row.get(0)),
			email: row.get(1),
			email_verified: row.get(2),
			recovery_opt_in: row.get(3),
			approve_via_email: row.get(4),
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
			r#"UPDATE accounts SET email = $1, email_verified = $2, recovery_opt_in = $3, approve_via_email = $4
               WHERE account_id = $5"#,
		)
		.bind(&email)
		.bind(verified)
		.bind(recovery_opt_in)
		.bind(approve_via_email)
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
impl KeyRepo for PostgresStore {
	async fn insert_genesis_key(&self, key: AuthorizedKeyRow) -> Result<(), StoreError> {
		self.insert_authorized_key(key).await
	}

	async fn list_keys(&self, account_id: &AccountId) -> Result<Vec<AuthorizedKeyRow>, StoreError> {
		let rows = sqlx::query(
			r#"SELECT vk, label, status, next_nonce, added_at_unix, revoked_at_unix, usable_at_unix
               FROM authorized_keys WHERE account_id = $1"#,
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
			"SELECT COUNT(*) FROM authorized_keys WHERE account_id = $1 AND vk = $2 AND status = 'active'",
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
			r#"UPDATE authorized_keys SET next_nonce = $1
               WHERE account_id = $2 AND vk = $3 AND next_nonce = $4 AND status = 'active'"#,
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
               VALUES ($1, $2, $3, $4, $5, $6, $7, $8)"#,
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
               VALUES ($1, $2, $3, $4, $5, $6, $7)"#,
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
		.map_err(|e| unique_or_storage(e, "link request exists"))?;
		Ok(())
	}

	async fn get_link_request(
		&self,
		request_id: &[u8; 32],
	) -> Result<Option<StoredLinkRequest>, StoreError> {
		let row = sqlx::query(
			"SELECT payload_json, new_sig, status, created_at_unix FROM link_requests WHERE request_id = $1",
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
		let updated = sqlx::query("UPDATE link_requests SET status = $1 WHERE request_id = $2")
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
impl InventoryRepo for PostgresStore {
	async fn grant_tiles(
		&self,
		account_id: &AccountId,
		tiles: &[TileGrant],
		granted_at_unix: i64,
	) -> Result<Vec<TileInstanceRow>, StoreError> {
		let mut out = Vec::new();
		let trade_bind = granted_at_unix + crate::model::TRADE_BIND_SECS;
		for grant in tiles {
			let row = sqlx::query(
				r#"INSERT INTO tile_instances (
                    account_id, tile_kind_id, origin, frozen, trade_bind_until_unix,
                    generation_id, core_tile_id, tile_type_id, character_id, pass_version
                   ) VALUES ($1, $2, $3, FALSE, $4, $5, $6, $7, $8, $9)
                   RETURNING instance_id"#,
			)
			.bind(account_id.as_bytes().as_slice())
			.bind(grant.tile_kind_id as i64)
			.bind(&grant.origin)
			.bind(trade_bind)
			.bind(grant.generation_id as i64)
			.bind(&grant.core_tile_id)
			.bind(&grant.tile_type_id)
			.bind(&grant.character_id)
			.bind(grant.pass_version as i32)
			.fetch_one(&self.pool)
			.await
			.map_err(|e| StoreError::Storage(e.to_string()))?;
			out.push(TileInstanceRow {
				instance_id: row.get::<i64, _>(0) as u64,
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
			});
		}
		Ok(out)
	}

	async fn list_tiles(&self, account_id: &AccountId) -> Result<Vec<TileInstanceRow>, StoreError> {
		self.list_issued(TileFilter { account_id: Some(*account_id), ..TileFilter::default() })
			.await
	}
}

#[async_trait]
impl RatingRepo for PostgresStore {
	async fn upsert_default_rating(
		&self,
		account_id: &AccountId,
		elo_milli: i32,
	) -> Result<RatingRow, StoreError> {
		sqlx::query(
			r#"INSERT INTO ratings (account_id, elo_milli, games) VALUES ($1, $2, 0)
               ON CONFLICT (account_id) DO UPDATE SET elo_milli = excluded.elo_milli"#,
		)
		.bind(account_id.as_bytes().as_slice())
		.bind(elo_milli)
		.execute(&self.pool)
		.await
		.map_err(|e| StoreError::Storage(e.to_string()))?;
		Ok(RatingRow { account_id: *account_id, elo_milli, games: 0 })
	}

	async fn get_rating(&self, account_id: &AccountId) -> Result<Option<RatingRow>, StoreError> {
		let row = sqlx::query("SELECT elo_milli, games FROM ratings WHERE account_id = $1")
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
impl RecoveryRepo for PostgresStore {
	async fn insert_recovery(&self, row: RecoveryRow) -> Result<(), StoreError> {
		sqlx::query(
			r#"INSERT INTO recovery_pending
               (account_id, pending_vk, token_hash, kind, started_at_unix, usable_at_unix, email)
               VALUES ($1, $2, $3, $4, $5, $6, $7)"#,
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
               FROM recovery_pending WHERE account_id = $1 AND cancelled_at_unix IS NULL
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
			"UPDATE recovery_pending SET cancelled_at_unix = $1 WHERE account_id = $2 AND cancelled_at_unix IS NULL",
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
impl CatalogRepo for PostgresStore {
	async fn upsert_character(&self, row: CharacterRecord) -> Result<(), StoreError> {
		validate_character_id(&row.character_id)?;
		if row.display_name.trim().is_empty() {
			return Err(StoreError::InvalidInput("display_name is empty".into()));
		}
		sqlx::query(
			r#"INSERT INTO character_catalog (character_id, display_name, image_png, created_at_unix)
               VALUES ($1, $2, $3, $4)
               ON CONFLICT (character_id) DO UPDATE SET
                 display_name = excluded.display_name,
                 image_png = COALESCE(excluded.image_png, character_catalog.image_png)"#,
		)
		.bind(&row.character_id)
		.bind(&row.display_name)
		.bind(&row.image_png)
		.bind(row.created_at_unix)
		.execute(&self.pool)
		.await
		.map_err(|e| StoreError::Storage(e.to_string()))?;
		Ok(())
	}

	async fn get_character(&self, id: &str) -> Result<Option<CharacterRecord>, StoreError> {
		let row = sqlx::query(
			"SELECT character_id, display_name, image_png, created_at_unix FROM character_catalog WHERE character_id = $1",
		)
		.bind(id)
		.fetch_optional(&self.pool)
		.await
		.map_err(|e| StoreError::Storage(e.to_string()))?;
		Ok(row.map(|row| CharacterRecord {
			character_id: row.get(0),
			display_name: row.get(1),
			image_png: row.get(2),
			created_at_unix: row.get(3),
		}))
	}

	async fn list_characters(&self) -> Result<Vec<CharacterRecord>, StoreError> {
		let rows = sqlx::query(
			"SELECT character_id, display_name, image_png, created_at_unix FROM character_catalog ORDER BY character_id",
		)
		.fetch_all(&self.pool)
		.await
		.map_err(|e| StoreError::Storage(e.to_string()))?;
		Ok(rows
			.into_iter()
			.map(|row| CharacterRecord {
				character_id: row.get(0),
				display_name: row.get(1),
				image_png: row.get(2),
				created_at_unix: row.get(3),
			})
			.collect())
	}

	async fn set_character_image(&self, id: &str, png: Vec<u8>) -> Result<(), StoreError> {
		let updated =
			sqlx::query("UPDATE character_catalog SET image_png = $1 WHERE character_id = $2")
				.bind(&png)
				.bind(id)
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
impl PassRepo for PostgresStore {
	async fn pass_pointer(&self) -> Result<PassPointer, StoreError> {
		self.load_pointer().await
	}

	async fn load_tables(&self, version: u16) -> Result<SnapshotCharacterTables, StoreError> {
		let cfg_rows = sqlx::query("SELECT valid_from, valid_to, tau, k FROM pass_config_revs")
			.fetch_all(&self.pool)
			.await
			.map_err(|e| StoreError::Storage(e.to_string()))?;
		let cfg = cfg_rows
			.into_iter()
			.find(|row| {
				is_live(
					row.get::<i32, _>(0) as u16,
					row.get::<Option<i32>, _>(1).map(|v| v as u16),
					version,
				)
			})
			.ok_or(StoreError::NotFound)?;
		let tau: f64 = cfg.get(2);
		let k = cfg.get::<Option<i32>, _>(3).map(|v| v as usize);

		let emb_rows = sqlx::query(
			"SELECT character_id, valid_from, valid_to, sparse_json FROM embedding_revs",
		)
		.fetch_all(&self.pool)
		.await
		.map_err(|e| StoreError::Storage(e.to_string()))?;
		let mut rows = Vec::new();
		for row in emb_rows {
			if is_live(
				row.get::<i32, _>(1) as u16,
				row.get::<Option<i32>, _>(2).map(|v| v as u16),
				version,
			) {
				let json: String = row.get(3);
				let value: serde_json::Value =
					serde_json::from_str(&json).map_err(|e| StoreError::Storage(e.to_string()))?;
				rows.push(
					SparseCharacterRow::from_json_value(&value)
						.map_err(StoreError::InvalidInput)?,
				);
			}
		}

		let fb_rows = sqlx::query("SELECT valid_from, valid_to, ids_json FROM fallback_revs")
			.fetch_all(&self.pool)
			.await
			.map_err(|e| StoreError::Storage(e.to_string()))?;
		let fallback = fb_rows
			.into_iter()
			.find(|row| {
				is_live(
					row.get::<i32, _>(0) as u16,
					row.get::<Option<i32>, _>(1).map(|v| v as u16),
					version,
				)
			})
			.ok_or(StoreError::NotFound)?;
		let fallback: Vec<String> = serde_json::from_str(fallback.get::<String, _>(2).as_str())
			.map_err(|e| StoreError::Storage(e.to_string()))?;

		let spec_rows = sqlx::query("SELECT valid_from, valid_to, maps_json FROM specific_revs")
			.fetch_all(&self.pool)
			.await
			.map_err(|e| StoreError::Storage(e.to_string()))?;
		let mut maps = BTreeMap::new();
		if let Some(spec) = spec_rows.into_iter().find(|row| {
			is_live(
				row.get::<i32, _>(0) as u16,
				row.get::<Option<i32>, _>(1).map(|v| v as u16),
				version,
			)
		}) {
			let arr: Vec<serde_json::Value> =
				serde_json::from_str(spec.get::<String, _>(2).as_str())
					.map_err(|e| StoreError::Storage(e.to_string()))?;
			for v in arr {
				let (key, chars) = CoreKey::from_map_json(&v).map_err(StoreError::InvalidInput)?;
				maps.insert(key, chars);
			}
		}
		SnapshotCharacterTables::new(version, tau as f32, k, maps, rows, fallback)
			.map_err(StoreError::InvalidInput)
	}

	async fn upsert_draft_embedding(&self, row: SparseCharacterRow) -> Result<u16, StoreError> {
		row.validate().map_err(StoreError::InvalidInput)?;
		let draft = self.ensure_draft().await?;
		sqlx::query(
			"UPDATE embedding_revs SET valid_to = $1 WHERE character_id = $2 AND valid_to IS NULL",
		)
		.bind(draft as i32)
		.bind(&row.id)
		.execute(&self.pool)
		.await
		.map_err(|e| StoreError::Storage(e.to_string()))?;
		let json = serde_json::to_string(&row.to_json_value())
			.map_err(|e| StoreError::Storage(e.to_string()))?;
		let vector = Vector::from(encode_character(&row));
		sqlx::query(
			"INSERT INTO embedding_revs (character_id, valid_from, valid_to, sparse_json, vector) VALUES ($1, $2, NULL, $3, $4)",
		)
		.bind(&row.id)
		.bind(draft as i32)
		.bind(json)
		.bind(vector)
		.execute(&self.pool)
		.await
		.map_err(|e| StoreError::Storage(e.to_string()))?;
		Ok(draft)
	}

	async fn set_draft_fallback(&self, ids: Vec<String>) -> Result<u16, StoreError> {
		if ids.is_empty() {
			return Err(StoreError::InvalidInput("fallback set is empty".into()));
		}
		let draft = self.ensure_draft().await?;
		sqlx::query("UPDATE fallback_revs SET valid_to = $1 WHERE valid_to IS NULL")
			.bind(draft as i32)
			.execute(&self.pool)
			.await
			.map_err(|e| StoreError::Storage(e.to_string()))?;
		let json = serde_json::to_string(&ids).map_err(|e| StoreError::Storage(e.to_string()))?;
		sqlx::query(
			"INSERT INTO fallback_revs (valid_from, valid_to, ids_json) VALUES ($1, NULL, $2)",
		)
		.bind(draft as i32)
		.bind(json)
		.execute(&self.pool)
		.await
		.map_err(|e| StoreError::Storage(e.to_string()))?;
		Ok(draft)
	}

	async fn set_draft_specific(
		&self,
		maps: BTreeMap<CoreKey, Vec<String>>,
	) -> Result<u16, StoreError> {
		let draft = self.ensure_draft().await?;
		sqlx::query("UPDATE specific_revs SET valid_to = $1 WHERE valid_to IS NULL")
			.bind(draft as i32)
			.execute(&self.pool)
			.await
			.map_err(|e| StoreError::Storage(e.to_string()))?;
		let arr: Vec<serde_json::Value> = maps.iter().map(|(k, v)| k.to_map_json(v)).collect();
		let json = serde_json::to_string(&arr).map_err(|e| StoreError::Storage(e.to_string()))?;
		sqlx::query(
			"INSERT INTO specific_revs (valid_from, valid_to, maps_json) VALUES ($1, NULL, $2)",
		)
		.bind(draft as i32)
		.bind(json)
		.execute(&self.pool)
		.await
		.map_err(|e| StoreError::Storage(e.to_string()))?;
		Ok(draft)
	}

	async fn set_draft_config(&self, tau: f32, k: Option<usize>) -> Result<u16, StoreError> {
		if !(0.0..=1.0).contains(&tau) {
			return Err(StoreError::InvalidInput(format!("tau {tau} is outside [0, 1]")));
		}
		if let Some(0) = k {
			return Err(StoreError::InvalidInput("k must be >= 1 when set".into()));
		}
		let draft = self.ensure_draft().await?;
		sqlx::query("UPDATE pass_config_revs SET valid_to = $1 WHERE valid_to IS NULL")
			.bind(draft as i32)
			.execute(&self.pool)
			.await
			.map_err(|e| StoreError::Storage(e.to_string()))?;
		sqlx::query(
			"INSERT INTO pass_config_revs (valid_from, valid_to, tau, k) VALUES ($1, NULL, $2, $3)",
		)
		.bind(draft as i32)
		.bind(tau as f64)
		.bind(k.map(|v| v as i32))
		.execute(&self.pool)
		.await
		.map_err(|e| StoreError::Storage(e.to_string()))?;
		Ok(draft)
	}

	async fn publish_draft(&self) -> Result<u16, StoreError> {
		let mut pointer = self.load_pointer().await?;
		let version = pointer.publish()?;
		let _ = self.load_tables(version).await?;
		self.save_pointer(&pointer).await?;
		Ok(version)
	}

	async fn seed_snapshot(&self, tables: SnapshotCharacterTables) -> Result<(), StoreError> {
		let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM embedding_revs")
			.fetch_one(&self.pool)
			.await
			.map_err(|e| StoreError::Storage(e.to_string()))?;
		if count.0 > 0 {
			return Ok(());
		}
		let version = tables.version();
		self.save_pointer(&PassPointer {
			draft_version: version,
			published_version: Some(version),
		})
		.await?;
		sqlx::query(
			"INSERT INTO pass_config_revs (valid_from, valid_to, tau, k) VALUES ($1, NULL, $2, $3)",
		)
		.bind(version as i32)
		.bind(tables.tau() as f64)
		.bind(tables.k().map(|v| v as i32))
		.execute(&self.pool)
		.await
		.map_err(|e| StoreError::Storage(e.to_string()))?;
		for row in tables.embedding_rows() {
			let json = serde_json::to_string(&row.to_json_value())
				.map_err(|e| StoreError::Storage(e.to_string()))?;
			let vector = Vector::from(encode_character(row));
			sqlx::query(
				"INSERT INTO embedding_revs (character_id, valid_from, valid_to, sparse_json, vector) VALUES ($1, $2, NULL, $3, $4)",
			)
			.bind(&row.id)
			.bind(version as i32)
			.bind(json)
			.bind(vector)
			.execute(&self.pool)
			.await
			.map_err(|e| StoreError::Storage(e.to_string()))?;
		}
		let fb = serde_json::to_string(tables.fallback_ids())
			.map_err(|e| StoreError::Storage(e.to_string()))?;
		sqlx::query(
			"INSERT INTO fallback_revs (valid_from, valid_to, ids_json) VALUES ($1, NULL, $2)",
		)
		.bind(version as i32)
		.bind(fb)
		.execute(&self.pool)
		.await
		.map_err(|e| StoreError::Storage(e.to_string()))?;
		let arr: Vec<serde_json::Value> =
			tables.specific_all().iter().map(|(k, v)| k.to_map_json(v)).collect();
		let spec = serde_json::to_string(&arr).map_err(|e| StoreError::Storage(e.to_string()))?;
		sqlx::query(
			"INSERT INTO specific_revs (valid_from, valid_to, maps_json) VALUES ($1, NULL, $2)",
		)
		.bind(version as i32)
		.bind(spec)
		.execute(&self.pool)
		.await
		.map_err(|e| StoreError::Storage(e.to_string()))?;
		Ok(())
	}
}

#[async_trait]
impl AnnLookup for PostgresStore {
	async fn nearest(
		&self,
		version: u16,
		query: &[f32],
		tau: f32,
		k: Option<usize>,
	) -> Result<Vec<AnnHit>, StoreError> {
		let vector = Vector::from(query.to_vec());
		let limit = k.unwrap_or(64).max(1) as i64;
		let rows = sqlx::query(
			r#"SELECT character_id, 1.0 - (vector <=> $1) AS cosine
               FROM embedding_revs
               WHERE valid_from <= $2 AND (valid_to IS NULL OR valid_to > $2)
                 AND (1.0 - (vector <=> $1)) >= $3
               ORDER BY vector <=> $1, character_id
               LIMIT $4"#,
		)
		.bind(vector)
		.bind(version as i32)
		.bind(tau as f64)
		.bind(limit)
		.fetch_all(&self.pool)
		.await
		.map_err(|e| StoreError::Storage(e.to_string()))?;
		Ok(rows
			.into_iter()
			.map(|row| AnnHit { character_id: row.get(0), cosine: row.get::<f64, _>(1) as f32 })
			.collect())
	}
}

#[async_trait]
impl RoleRepo for PostgresStore {
	async fn grant_role(
		&self,
		account_id: &AccountId,
		role: &str,
		now_unix: i64,
	) -> Result<(), StoreError> {
		sqlx::query(
			"INSERT INTO account_roles (account_id, role, granted_at_unix) VALUES ($1, $2, $3) ON CONFLICT DO NOTHING",
		)
		.bind(account_id.as_bytes().as_slice())
		.bind(role)
		.bind(now_unix)
		.execute(&self.pool)
		.await
		.map_err(|e| StoreError::Storage(e.to_string()))?;
		Ok(())
	}

	async fn has_role(&self, account_id: &AccountId, role: &str) -> Result<bool, StoreError> {
		let count: (i64,) = sqlx::query_as(
			"SELECT COUNT(*) FROM account_roles WHERE account_id = $1 AND role = $2",
		)
		.bind(account_id.as_bytes().as_slice())
		.bind(role)
		.fetch_one(&self.pool)
		.await
		.map_err(|e| StoreError::Storage(e.to_string()))?;
		Ok(count.0 > 0)
	}

	async fn admin_count(&self) -> Result<u64, StoreError> {
		let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM account_roles WHERE role = $1")
			.bind(ROLE_ADMIN)
			.fetch_one(&self.pool)
			.await
			.map_err(|e| StoreError::Storage(e.to_string()))?;
		Ok(count.0 as u64)
	}
}

#[async_trait]
impl IssuedTileRepo for PostgresStore {
	async fn list_issued(&self, filter: TileFilter) -> Result<Vec<TileInstanceRow>, StoreError> {
		let rows = sqlx::query(
			r#"SELECT instance_id, account_id, tile_kind_id, origin, frozen, trade_bind_until_unix,
                      generation_id, core_tile_id, tile_type_id, character_id, pass_version
               FROM tile_instances ORDER BY instance_id"#,
		)
		.fetch_all(&self.pool)
		.await
		.map_err(|e| StoreError::Storage(e.to_string()))?;
		let mut out = Vec::new();
		for row in rows {
			let rec = decode_tile_row(&row)?;
			if filter.character_id.as_ref().map(|c| rec.character_id == *c).unwrap_or(true)
				&& filter.core_tile_id.as_ref().map(|c| rec.core_tile_id == *c).unwrap_or(true)
				&& filter.tile_type_id.as_ref().map(|c| rec.tile_type_id == *c).unwrap_or(true)
				&& filter.account_id.as_ref().map(|a| rec.account_id == *a).unwrap_or(true)
			{
				out.push(rec);
			}
		}
		let offset = filter.offset.unwrap_or(0) as usize;
		if offset > out.len() {
			return Ok(Vec::new());
		}
		out = out.split_off(offset);
		if let Some(limit) = filter.limit {
			out.truncate(limit as usize);
		}
		Ok(out)
	}

	async fn upsert_core_tile(&self, rec: CoreTileRecord) -> Result<(), StoreError> {
		let rings =
			serde_json::to_string(&rec.rings).map_err(|e| StoreError::Storage(e.to_string()))?;
		sqlx::query(
			r#"INSERT INTO core_tiles (core_tile_id, rings_json, dots, capture, emblem)
               VALUES ($1, $2, $3, $4, $5) ON CONFLICT (core_tile_id) DO NOTHING"#,
		)
		.bind(&rec.core_tile_id)
		.bind(rings)
		.bind(rec.dots as i32)
		.bind(&rec.capture)
		.bind(&rec.emblem)
		.execute(&self.pool)
		.await
		.map_err(|e| StoreError::Storage(e.to_string()))?;
		Ok(())
	}

	async fn upsert_tile_type(&self, rec: TileTypeRecord) -> Result<(), StoreError> {
		let rings =
			serde_json::to_string(&rec.rings).map_err(|e| StoreError::Storage(e.to_string()))?;
		sqlx::query(
			r#"INSERT INTO tile_types (tile_type_id, rings_json, dots, capture, emblem)
               VALUES ($1, $2, $3, $4, $5) ON CONFLICT (tile_type_id) DO NOTHING"#,
		)
		.bind(&rec.tile_type_id)
		.bind(rings)
		.bind(rec.dots as i32)
		.bind(&rec.capture)
		.bind(&rec.emblem)
		.execute(&self.pool)
		.await
		.map_err(|e| StoreError::Storage(e.to_string()))?;
		Ok(())
	}

	async fn get_core_tile(&self, id: &str) -> Result<Option<CoreTileRecord>, StoreError> {
		let row = sqlx::query(
			"SELECT core_tile_id, rings_json, dots, capture, emblem FROM core_tiles WHERE core_tile_id = $1",
		)
		.bind(id)
		.fetch_optional(&self.pool)
		.await
		.map_err(|e| StoreError::Storage(e.to_string()))?;
		row.map(decode_core_tile).transpose()
	}

	async fn get_tile_type(&self, id: &str) -> Result<Option<TileTypeRecord>, StoreError> {
		let row = sqlx::query(
			"SELECT tile_type_id, rings_json, dots, capture, emblem FROM tile_types WHERE tile_type_id = $1",
		)
		.bind(id)
		.fetch_optional(&self.pool)
		.await
		.map_err(|e| StoreError::Storage(e.to_string()))?;
		row.map(decode_tile_type).transpose()
	}
}

fn decode_tile_row(row: &PgRow) -> Result<TileInstanceRow, StoreError> {
	Ok(TileInstanceRow {
		instance_id: row.get::<i64, _>(0) as u64,
		account_id: AccountId::from_bytes(blob32(row.get::<Vec<u8>, _>(1).as_slice())?),
		tile_kind_id: row.get::<i64, _>(2) as u32,
		origin: row.get(3),
		frozen: row.get(4),
		trade_bind_until_unix: row.get(5),
		generation_id: row.get::<i64, _>(6) as u32,
		core_tile_id: row.get(7),
		tile_type_id: row.get(8),
		character_id: row.get(9),
		pass_version: row.get::<i32, _>(10) as u16,
	})
}

fn decode_core_tile(row: PgRow) -> Result<CoreTileRecord, StoreError> {
	let rings: Vec<(String, String)> = serde_json::from_str(row.get::<String, _>(1).as_str())
		.map_err(|e| StoreError::Storage(e.to_string()))?;
	Ok(CoreTileRecord {
		core_tile_id: row.get(0),
		rings,
		dots: row.get::<i32, _>(2) as u8,
		capture: row.get(3),
		emblem: row.get(4),
	})
}

fn decode_tile_type(row: PgRow) -> Result<TileTypeRecord, StoreError> {
	let rings: Vec<(String, String)> = serde_json::from_str(row.get::<String, _>(1).as_str())
		.map_err(|e| StoreError::Storage(e.to_string()))?;
	Ok(TileTypeRecord {
		tile_type_id: row.get(0),
		rings,
		dots: row.get::<i32, _>(2) as u8,
		capture: row.get(3),
		emblem: row.get(4),
	})
}

#[async_trait]
impl Store for PostgresStore {}

#[cfg(test)]
mod tests {
	use super::*;
	use apa_kan_core::character_pass::{assign_character, encode_character, JsonCharacterTables};
	use apa_kan_core::core_pass::{CorePass, TileId};

	async fn maybe_store() -> Option<PostgresStore> {
		let url = std::env::var("APA_KAN_POSTGRES_URL").ok()?;
		Some(PostgresStore::connect(&url).await.expect("postgres"))
	}

	#[tokio::test]
	async fn postgres_generation_matches_json_when_available() {
		let Some(store) = maybe_store().await else {
			return;
		};
		crate::seed::seed_bundled_v1(&store, 0).await.unwrap();
		let tables = store.load_tables(1).await.unwrap();
		let json = JsonCharacterTables::load_v1().unwrap();
		let cfg = CorePass::base();
		for id in 0..48u32 {
			let tile = cfg.run(TileId(id));
			assert_eq!(assign_character(&tables, &tile, id), assign_character(&json, &tile, id));
		}
		let row = json.embedding_rows().iter().find(|r| r.id == "westerlit").unwrap();
		let query = encode_character(row);
		let hits = store.nearest(1, &query, 0.0, Some(5)).await.unwrap();
		assert!(!hits.is_empty());
		assert_eq!(hits[0].character_id, "westerlit");
	}
}
