//! Shared SQL for the SQLite and Postgres ledgers. Both accept `$N`
//! placeholders, so one macro body serves both pool types.

macro_rules! sql_ledger {
	($store:ty, $name:literal) => {
		impl $store {
			async fn conflict_or_missing(&self, e: &$crate::LedgerEntry) -> $crate::LedgerError {
				match <$store as $crate::LedgerStore>::head(self, &e.stream).await {
					Ok(h) => $crate::LedgerError::Conflict { current: h.next_seq, got: e.seq },
					Err(err) => err,
				}
			}
		}

		#[async_trait::async_trait]
		impl $crate::LedgerStore for $store {
			async fn append(&self, e: &$crate::LedgerEntry) -> $crate::Result<()> {
				use $crate::sql::{is_unique_violation, storage};
				$crate::validate_entry(e)?;
				let mut db = self.pool.begin().await.map_err(storage)?;
				if e.seq == 0 {
					// Genesis: creating the stream row is the CAS.
					let res = sqlx::query(
										"INSERT INTO ledger_streams (stream_id, next_seq, head_hash, created_at_unix) \
						 VALUES ($1, 1, $2, $3)",
									)
									.bind(&e.stream)
									.bind(&e.hash)
									.bind(e.committed_at_unix)
									.execute(&mut *db)
									.await;
					match res {
						Ok(_) => {}
						Err(err) if is_unique_violation(&err) => {
							drop(db);
							return Err(self.conflict_or_missing(e).await);
						}
						Err(err) => return Err(storage(err)),
					}
				} else {
					// Compare-and-swap seq *and* head hash. Postgres takes the
					// stream row lock, so a racing append blocks, re-checks,
					// and misses; SQLite serializes writers.
					let bumped = sqlx::query(
						"UPDATE ledger_streams SET next_seq = next_seq + 1, head_hash = $3 \
						 WHERE stream_id = $1 AND next_seq = $2 AND head_hash = $4",
					)
					.bind(&e.stream)
					.bind(e.seq as i64)
					.bind(&e.hash)
					.bind(&e.prev)
					.execute(&mut *db)
					.await
					.map_err(storage)?
					.rows_affected();
					if bumped != 1 {
						drop(db);
						return Err(self.conflict_or_missing(e).await);
					}
				}
				let res = sqlx::query(
									"INSERT INTO ledger_entries (stream_id, seq, tx_hash, prev_hash, signer, kind, \
					 tx_json, committed_at_unix) VALUES ($1, $2, $3, $4, $5, $6, $7, $8)",
								)
								.bind(&e.stream)
								.bind(e.seq as i64)
								.bind(&e.hash)
								.bind(&e.prev)
								.bind(&e.signer)
								.bind(&e.kind)
								.bind(&e.tx_json)
								.bind(e.committed_at_unix)
								.execute(&mut *db)
								.await;
				match res {
					Ok(_) => {}
					Err(err) if is_unique_violation(&err) => {
						drop(db);
						return Err(self.conflict_or_missing(e).await);
					}
					Err(err) => return Err(storage(err)),
				}
				db.commit().await.map_err(storage)?;
				Ok(())
			}

			async fn read(
				&self,
				stream: &str,
				from: u64,
			) -> $crate::Result<Vec<$crate::LedgerEntry>> {
				use sqlx::Row;
				use $crate::sql::storage;
				let head = <$store as $crate::LedgerStore>::head(self, stream).await?;
				let rows = sqlx::query(
					"SELECT seq, tx_hash, prev_hash, signer, kind, tx_json, committed_at_unix \
					 FROM ledger_entries WHERE stream_id = $1 AND seq >= $2 ORDER BY seq",
				)
				.bind(stream)
				.bind(from.min(i64::MAX as u64) as i64)
				.fetch_all(&self.pool)
				.await
				.map_err(storage)?;
				let mut out = Vec::with_capacity(rows.len());
				for r in rows {
					let seq: i64 = r.try_get(0).map_err(storage)?;
					out.push($crate::LedgerEntry {
						stream: stream.to_string(),
						seq: seq as u64,
						hash: r.try_get(1).map_err(storage)?,
						prev: r.try_get(2).map_err(storage)?,
						signer: r.try_get(3).map_err(storage)?,
						kind: r.try_get(4).map_err(storage)?,
						tx_json: r.try_get(5).map_err(storage)?,
						committed_at_unix: r.try_get(6).map_err(storage)?,
					});
				}
				$crate::check_chain(stream, from, &out)?;
				let expected = head.next_seq.saturating_sub(from);
				if from < head.next_seq && out.len() as u64 != expected {
					return Err($crate::LedgerError::Corrupt(format!(
						"{stream}: head says {} entries, read {}",
						head.next_seq,
						out.len()
					)));
				}
				Ok(out)
			}

			async fn head(&self, stream: &str) -> $crate::Result<$crate::StreamHead> {
				use sqlx::Row;
				use $crate::sql::storage;
				let row = sqlx::query(
					"SELECT next_seq, head_hash FROM ledger_streams WHERE stream_id = $1",
				)
				.bind(stream)
				.fetch_optional(&self.pool)
				.await
				.map_err(storage)?
				.ok_or($crate::LedgerError::NotFound)?;
				let n: i64 = row.try_get(0).map_err(storage)?;
				Ok($crate::StreamHead {
					next_seq: n as u64,
					head: row.try_get(1).map_err(storage)?,
				})
			}

			async fn count_streams(&self, prefix: &str) -> $crate::Result<u64> {
				use sqlx::Row;
				use $crate::sql::storage;
				// Prefix match without LIKE escaping pitfalls.
				let row = sqlx::query(
					"SELECT COUNT(*) FROM ledger_streams WHERE substr(stream_id, 1, $2) = $1",
				)
				.bind(prefix)
				.bind(prefix.chars().count() as i32)
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

pub(crate) use sql_ledger;

pub(crate) fn storage(e: sqlx::Error) -> crate::LedgerError {
	crate::LedgerError::Storage(e.to_string())
}

pub(crate) fn is_unique_violation(e: &sqlx::Error) -> bool {
	matches!(e, sqlx::Error::Database(d) if d.is_unique_violation())
}
