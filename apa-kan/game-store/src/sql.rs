//! Shared SQL for the SQLite and Postgres backends. Both accept `$N`
//! placeholders, so one macro body serves both pool types.

/// Implement [`crate::GameStore`] for `$store` wrapping a `$pool` field.
macro_rules! sql_game_store {
	($store:ty, $db:ty, $name:literal, $is_unique:path) => {
		#[async_trait::async_trait]
		impl $crate::GameStore for $store {
			async fn create(&self, game: &$crate::StoredGame) -> $crate::Result<()> {
				$crate::validate_new(game)?;
				let opening = serde_json::to_string(&game.opening)
					.map_err(|e| $crate::GameStoreError::Invalid(e.to_string()))?;
				let res = sqlx::query(
					"INSERT INTO hosted_games (game_id, schema_v, opening_json, seat1_secret, \
					 seat2_secret, seat1_label, seat2_label, next_seq, created_at_unix) \
					 VALUES ($1, $2, $3, $4, $5, $6, $7, 0, $8)",
				)
				.bind(&game.game_id)
				.bind(game.opening.v as i32)
				.bind(opening)
				.bind(&game.seat_secrets[0])
				.bind(&game.seat_secrets[1])
				.bind(&game.labels[0])
				.bind(&game.labels[1])
				.bind(game.created_at_unix)
				.execute(&self.pool)
				.await;
				match res {
					Ok(_) => Ok(()),
					Err(e) if $is_unique(&e) => Err($crate::GameStoreError::Exists),
					Err(e) => Err(storage(e)),
				}
			}

			async fn append(&self, tx: &$crate::LoggedTx) -> $crate::Result<()> {
				$crate::validate_tx(tx)?;
				let action = serde_json::to_string(&tx.action)
					.map_err(|e| $crate::GameStoreError::Invalid(e.to_string()))?;
				let mut db = self.pool.begin().await.map_err(storage)?;
				// Compare-and-swap the cursor; the row lock (Postgres) or the
				// write lock (SQLite) makes a racing append re-check and miss.
				let bumped = sqlx::query(
					"UPDATE hosted_games SET next_seq = next_seq + 1 \
					 WHERE game_id = $1 AND next_seq = $2",
				)
				.bind(&tx.game_id)
				.bind(tx.seq as i64)
				.execute(&mut *db)
				.await
				.map_err(storage)?
				.rows_affected();
				if bumped != 1 {
					drop(db);
					return match self.head_seq(&tx.game_id).await {
						Ok(current) => {
							Err($crate::GameStoreError::Conflict { current, got: tx.seq })
						}
						Err(e) => Err(e),
					};
				}
				let res = sqlx::query(
					"INSERT INTO hosted_game_txs (game_id, seq, seat, action_json, auth_kind, \
					 auth_json, committed_at_unix) VALUES ($1, $2, $3, $4, $5, $6, $7)",
				)
				.bind(&tx.game_id)
				.bind(tx.seq as i64)
				.bind(i16::from(tx.seat))
				.bind(action)
				.bind(tx.auth.kind())
				.bind(tx.auth.proof_json())
				.bind(tx.committed_at_unix)
				.execute(&mut *db)
				.await;
				if let Err(e) = res {
					drop(db);
					if $is_unique(&e) {
						let current = self.head_seq(&tx.game_id).await?;
						return Err($crate::GameStoreError::Conflict { current, got: tx.seq });
					}
					return Err(storage(e));
				}
				db.commit().await.map_err(storage)?;
				Ok(())
			}

			async fn load(&self, game_id: &str) -> $crate::Result<$crate::LoadedGame> {
				use sqlx::Row;
				let row = sqlx::query(
					"SELECT opening_json, seat1_secret, seat2_secret, seat1_label, seat2_label, \
					 created_at_unix, next_seq FROM hosted_games WHERE game_id = $1",
				)
				.bind(game_id)
				.fetch_optional(&self.pool)
				.await
				.map_err(storage)?
				.ok_or($crate::GameStoreError::NotFound)?;
				let opening_json: String = row.try_get(0).map_err(storage)?;
				let opening: apa_kan_view::GameRecord = serde_json::from_str(&opening_json)
					.map_err(|e| $crate::GameStoreError::Corrupt(e.to_string()))?;
				let game = $crate::StoredGame {
					game_id: game_id.to_string(),
					opening,
					seat_secrets: [
						row.try_get(1).map_err(storage)?,
						row.try_get(2).map_err(storage)?,
					],
					labels: [row.try_get(3).map_err(storage)?, row.try_get(4).map_err(storage)?],
					created_at_unix: row.try_get(5).map_err(storage)?,
				};
				let next_seq: i64 = row.try_get(6).map_err(storage)?;
				let rows = sqlx::query(
					"SELECT seq, seat, action_json, auth_kind, auth_json, committed_at_unix \
					 FROM hosted_game_txs WHERE game_id = $1 ORDER BY seq",
				)
				.bind(game_id)
				.fetch_all(&self.pool)
				.await
				.map_err(storage)?;
				let mut log = Vec::with_capacity(rows.len());
				for r in rows {
					let seq: i64 = r.try_get(0).map_err(storage)?;
					let seat: i16 = r.try_get(1).map_err(storage)?;
					let action_json: String = r.try_get(2).map_err(storage)?;
					let kind: String = r.try_get(3).map_err(storage)?;
					let proof: Option<String> = r.try_get(4).map_err(storage)?;
					log.push($crate::LoggedTx {
						game_id: game_id.to_string(),
						seq: seq as u64,
						seat: u8::try_from(seat)
							.map_err(|e| $crate::GameStoreError::Corrupt(e.to_string()))?,
						action: serde_json::from_str(&action_json)
							.map_err(|e| $crate::GameStoreError::Corrupt(e.to_string()))?,
						auth: $crate::LoggedAuth::from_parts(&kind, proof.as_deref())?,
						committed_at_unix: r.try_get(5).map_err(storage)?,
					});
				}
				$crate::check_log(game_id, &log)?;
				if log.len() as i64 != next_seq {
					return Err($crate::GameStoreError::Corrupt(format!(
						"{game_id}: cursor {next_seq} but {} log rows",
						log.len()
					)));
				}
				Ok($crate::LoadedGame { game, log })
			}

			async fn head_seq(&self, game_id: &str) -> $crate::Result<u64> {
				use sqlx::Row;
				let row = sqlx::query("SELECT next_seq FROM hosted_games WHERE game_id = $1")
					.bind(game_id)
					.fetch_optional(&self.pool)
					.await
					.map_err(storage)?
					.ok_or($crate::GameStoreError::NotFound)?;
				let n: i64 = row.try_get(0).map_err(storage)?;
				Ok(n as u64)
			}

			async fn count(&self) -> $crate::Result<u64> {
				use sqlx::Row;
				let row = sqlx::query("SELECT COUNT(*) FROM hosted_games")
					.fetch_one(&self.pool)
					.await
					.map_err(storage)?;
				let n: i64 = row.try_get(0).map_err(storage)?;
				Ok(n as u64)
			}

			fn backend(&self) -> &'static str {
				$name
			}
		}
	};
}

pub(crate) use sql_game_store;

pub(crate) fn storage(e: sqlx::Error) -> crate::GameStoreError {
	crate::GameStoreError::Storage(e.to_string())
}

pub(crate) fn is_unique_violation(e: &sqlx::Error) -> bool {
	matches!(e, sqlx::Error::Database(d) if d.is_unique_violation())
}
