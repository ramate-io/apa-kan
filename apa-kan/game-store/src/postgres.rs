//! Postgres backend, matching the platform Postgres in #97. The append's
//! `UPDATE … WHERE next_seq = $2` takes the game row lock, so a racing append
//! for the same seq waits, re-checks, and misses.

use std::str::FromStr;

use sqlx::postgres::{PgConnectOptions, PgPool, PgPoolOptions};

use crate::sql::{is_unique_violation, sql_game_store, storage};
use crate::Result;

const MIGRATION_001: &str = include_str!("../migrations/postgres/001_hosted_games.sql");

pub struct PostgresGameStore {
	pool: PgPool,
}

impl PostgresGameStore {
	pub async fn connect(url: &str) -> Result<Self> {
		let options = PgConnectOptions::from_str(url).map_err(storage)?;
		let pool = PgPoolOptions::new()
			.max_connections(8)
			.connect_with(options)
			.await
			.map_err(storage)?;
		sqlx::raw_sql(MIGRATION_001).execute(&pool).await.map_err(storage)?;
		Ok(Self { pool })
	}
}

sql_game_store!(PostgresGameStore, sqlx::Postgres, "postgres", is_unique_violation);

#[cfg(test)]
mod tests {
	use std::sync::Arc;

	use super::*;
	use crate::{conformance, GameStore};

	/// Runs when `APAKAN_TEST_POSTGRES_URL` points at a scratch database
	/// (CI sets it with a Postgres service); skipped otherwise.
	#[tokio::test]
	async fn postgres_conformance() {
		let Ok(url) = std::env::var("APAKAN_TEST_POSTGRES_URL") else {
			eprintln!("APAKAN_TEST_POSTGRES_URL unset; skipping Postgres conformance");
			return;
		};
		let store: Arc<dyn GameStore> =
			Arc::new(PostgresGameStore::connect(&url).await.unwrap_or_else(|e| panic!("{e}")));
		let prefix = format!("pg{}", std::process::id());
		conformance::run(store.clone(), &prefix).await;
		conformance::race(store, &prefix).await;
	}
}
