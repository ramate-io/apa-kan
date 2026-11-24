//! SQLite backend: single-node durable default. WAL + `synchronous=FULL`, so
//! a committed append survives a crash before the move is acknowledged.

use std::str::FromStr;
use std::time::Duration;

use sqlx::sqlite::{
	SqliteConnectOptions, SqliteJournalMode, SqlitePool, SqlitePoolOptions, SqliteSynchronous,
};

use crate::sql::{is_unique_violation, sql_game_store, storage};
use crate::Result;

const MIGRATION_001: &str = include_str!("../migrations/sqlite/001_hosted_games.sql");

pub struct SqliteGameStore {
	pool: SqlitePool,
}

impl SqliteGameStore {
	/// `path` is a file path, or `:memory:` for a private in-memory database.
	pub async fn connect(path: &str) -> Result<Self> {
		let memory = path == ":memory:" || path.is_empty();
		let url = if memory { "sqlite::memory:".to_string() } else { format!("sqlite:{path}") };
		let mut options = SqliteConnectOptions::from_str(&url)
			.map_err(storage)?
			.create_if_missing(true)
			.foreign_keys(true)
			.busy_timeout(Duration::from_secs(10))
			.synchronous(SqliteSynchronous::Full);
		if !memory {
			options = options.journal_mode(SqliteJournalMode::Wal);
		}
		let pool = SqlitePoolOptions::new()
			.max_connections(if memory { 1 } else { 4 })
			.connect_with(options)
			.await
			.map_err(storage)?;
		sqlx::raw_sql(MIGRATION_001).execute(&pool).await.map_err(storage)?;
		Ok(Self { pool })
	}
}

sql_game_store!(SqliteGameStore, sqlx::Sqlite, "sqlite", is_unique_violation);

#[cfg(test)]
mod tests {
	use std::sync::Arc;

	use super::*;
	use crate::{conformance, GameStore};

	#[tokio::test]
	async fn sqlite_memory_conformance() {
		let store: Arc<dyn GameStore> =
			Arc::new(SqliteGameStore::connect(":memory:").await.unwrap_or_else(|e| panic!("{e}")));
		conformance::run(store.clone(), "sqlm").await;
		conformance::race(store, "sqlm").await;
	}

	#[tokio::test]
	async fn sqlite_file_survives_reopen() {
		let dir = std::env::temp_dir().join(format!("apakan-gs-{}", std::process::id()));
		std::fs::create_dir_all(&dir).unwrap_or_else(|e| panic!("{e}"));
		let path = dir.join("games.db");
		let path = path.to_string_lossy().to_string();
		{
			let store: Arc<dyn GameStore> =
				Arc::new(SqliteGameStore::connect(&path).await.unwrap_or_else(|e| panic!("{e}")));
			conformance::run(store.clone(), "sqlf").await;
			conformance::race(store, "sqlf").await;
		}
		let reopened = SqliteGameStore::connect(&path).await.unwrap_or_else(|e| panic!("{e}"));
		assert_eq!(reopened.head_seq("sqlf-g1").await, Ok(10));
		let g = reopened.load("sqlf-g1").await.unwrap_or_else(|e| panic!("{e}"));
		assert!(g.record().replay().is_ok());
		let _ = std::fs::remove_dir_all(dir);
	}
}
