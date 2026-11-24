//! Durable storage for hosted games.
//!
//! A hosted game is its opening [`GameRecord`] (rules, seed, completed deal)
//! plus an append-only log of accepted transactions, keyed by
//! `(game_id, seq)`. Core is deterministic, so [`LoadedGame::record`] +
//! `GameRecord::replay` rebuilds the exact position after a restart.
//!
//! [`GameStore::append`] is a compare-and-swap on the game's next sequence
//! number: it succeeds only when `seq` equals the number of entries already
//! logged, and the cursor bump and the log insert commit together. Two
//! concurrent appends for the same move number therefore cannot both land; the
//! loser gets [`GameStoreError::Conflict`]. Hosts commit before acknowledging
//! a move.
//!
//! Bearer seat tokens are never stored, only their hashes (`seat_secrets`).
//!
//! Backends: [`memory::MemoryGameStore`] (tests, `memory` URL),
//! `sqlite::SqliteGameStore` (feature `sqlite`, single-node default), and
//! `postgres::PostgresGameStore` (feature `postgres`, matches the platform
//! Postgres in #97). [`connect`] picks one from a URL.

use std::sync::Arc;

use apa_kan_view::{ActionWire, GameRecord, Seat};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

pub mod memory;
#[cfg(feature = "postgres")]
pub mod postgres;
#[cfg(any(feature = "sqlite", feature = "postgres"))]
mod sql;
#[cfg(feature = "sqlite")]
pub mod sqlite;

#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum GameStoreError {
	#[error("game not found")]
	NotFound,
	#[error("game already exists")]
	Exists,
	/// The append named a seq that is not the game's next one (someone else
	/// committed that move first, or the caller is behind).
	#[error("seq conflict: store is at {current}, append was for {got}")]
	Conflict { current: u64, got: u64 },
	#[error("invalid input: {0}")]
	Invalid(String),
	#[error("corrupt stored game: {0}")]
	Corrupt(String),
	#[error("storage: {0}")]
	Storage(String),
}

pub type Result<T> = std::result::Result<T, GameStoreError>;

/// Everything about a game except its moves.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct StoredGame {
	pub game_id: String,
	/// Opening record; `actions` must be empty.
	pub opening: GameRecord,
	/// Hex SHA3 of each seat's bearer token (`apa_kan_tx::SeatSecret`).
	pub seat_secrets: [String; 2],
	pub labels: [String; 2],
	pub created_at_unix: i64,
}

/// How a logged transaction was authorized. Never holds a bearer token.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum LoggedAuth {
	/// Checked against the seat secret hash at commit time.
	SeatToken,
}

impl LoggedAuth {
	pub fn kind(&self) -> &'static str {
		match self {
			LoggedAuth::SeatToken => "seat_token",
		}
	}

	/// Public proof to keep alongside the row (signatures, later).
	pub fn proof_json(&self) -> Option<String> {
		match self {
			LoggedAuth::SeatToken => None,
		}
	}

	pub fn from_parts(kind: &str, _proof: Option<&str>) -> Result<Self> {
		match kind {
			"seat_token" => Ok(LoggedAuth::SeatToken),
			other => Err(GameStoreError::Corrupt(format!("unknown auth kind {other}"))),
		}
	}
}

/// One accepted transaction, as committed.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LoggedTx {
	pub game_id: String,
	/// Move number: the record's action count before this action.
	pub seq: u64,
	pub seat: Seat,
	pub action: ActionWire,
	pub auth: LoggedAuth,
	pub committed_at_unix: i64,
}

/// A game read back from storage.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LoadedGame {
	pub game: StoredGame,
	/// Log entries in seq order, `log[i].seq == i`.
	pub log: Vec<LoggedTx>,
}

impl LoadedGame {
	/// Opening record with every logged action appended.
	pub fn record(&self) -> GameRecord {
		let mut rec = self.game.opening.clone();
		rec.actions = self.log.iter().map(|t| t.action).collect();
		rec
	}

	pub fn seq(&self) -> u64 {
		self.log.len() as u64
	}
}

#[async_trait]
pub trait GameStore: Send + Sync {
	/// Persist a new game with an empty log.
	async fn create(&self, game: &StoredGame) -> Result<()>;
	/// Atomically append `tx` iff `tx.seq` is the game's next seq.
	async fn append(&self, tx: &LoggedTx) -> Result<()>;
	/// Read a game and its whole log.
	async fn load(&self, game_id: &str) -> Result<LoadedGame>;
	/// The game's next seq (number of logged transactions).
	async fn head_seq(&self, game_id: &str) -> Result<u64>;
	/// Number of stored games.
	async fn count(&self) -> Result<u64>;
	/// Short backend name for logs.
	fn backend(&self) -> &'static str;
}

/// Checks shared by every backend before writing a new game.
pub fn validate_new(game: &StoredGame) -> Result<()> {
	if game.game_id.is_empty() || game.game_id.len() > 128 {
		return Err(GameStoreError::Invalid("game_id must be 1..=128 bytes".into()));
	}
	if game.opening.id != game.game_id {
		return Err(GameStoreError::Invalid("opening record id differs from game_id".into()));
	}
	if !game.opening.actions.is_empty() {
		return Err(GameStoreError::Invalid("opening record must have no actions".into()));
	}
	Ok(())
}

/// Checks shared by every backend before an append.
pub fn validate_tx(tx: &LoggedTx) -> Result<()> {
	if !(1..=2).contains(&tx.seat) {
		return Err(GameStoreError::Invalid(format!("seat {} out of range", tx.seat)));
	}
	if tx.seq > i64::MAX as u64 {
		return Err(GameStoreError::Invalid("seq too large".into()));
	}
	Ok(())
}

/// Check a loaded log is dense and in order.
pub fn check_log(game_id: &str, log: &[LoggedTx]) -> Result<()> {
	for (i, t) in log.iter().enumerate() {
		if t.seq != i as u64 || t.game_id != game_id {
			return Err(GameStoreError::Corrupt(format!(
				"log gap or mismatch at index {i} (seq {}) for {game_id}",
				t.seq
			)));
		}
	}
	Ok(())
}

/// Open a store from a URL:
/// - `memory` / `memory:` — process memory (tests, throwaway dev)
/// - `sqlite:<path>` or `sqlite::memory:` — feature `sqlite`
/// - `postgres://…` / `postgresql://…` — feature `postgres`
pub async fn connect(url: &str) -> Result<Arc<dyn GameStore>> {
	let url = url.trim();
	if url == "memory" || url == "memory:" {
		return Ok(Arc::new(memory::MemoryGameStore::default()));
	}
	if url.starts_with("postgres://") || url.starts_with("postgresql://") {
		#[cfg(feature = "postgres")]
		{
			return Ok(Arc::new(postgres::PostgresGameStore::connect(url).await?));
		}
		#[cfg(not(feature = "postgres"))]
		{
			return Err(GameStoreError::Invalid(
				"postgres URL given but the `postgres` feature is off".into(),
			));
		}
	}
	if let Some(rest) = url.strip_prefix("sqlite:") {
		#[cfg(feature = "sqlite")]
		{
			return Ok(Arc::new(sqlite::SqliteGameStore::connect(rest).await?));
		}
		#[cfg(not(feature = "sqlite"))]
		{
			let _ = rest;
			return Err(GameStoreError::Invalid(
				"sqlite URL given but the `sqlite` feature is off".into(),
			));
		}
	}
	Err(GameStoreError::Invalid(format!("unsupported game store URL: {url}")))
}

/// Backend-independent conformance checks. Each backend's tests call this.
#[doc(hidden)]
pub mod conformance {
	use super::*;
	use apa_kan_core::{Game, RuleSet};

	pub fn sample_game(id: &str, seed: u64) -> (StoredGame, Game) {
		let live = Game::new(RuleSet::default(), seed);
		let opening = GameRecord::begin(id, &live).unwrap_or_else(|e| panic!("{e}"));
		(
			StoredGame {
				game_id: id.into(),
				opening,
				seat_secrets: ["aa".repeat(32), "bb".repeat(32)],
				labels: ["ana".into(), "bo".into()],
				created_at_unix: 1_700_000_000,
			},
			live,
		)
	}

	fn logged(id: &str, seq: u64, live: &Game, action: ActionWire) -> LoggedTx {
		LoggedTx {
			game_id: id.into(),
			seq,
			seat: apa_kan_view::seat_of(live.current_player()),
			action,
			auth: LoggedAuth::SeatToken,
			committed_at_unix: 1_700_000_100 + seq as i64,
		}
	}

	/// Round trip, CAS conflicts, unknown games, and exact replay.
	pub async fn run(store: Arc<dyn GameStore>, prefix: &str) {
		let id = format!("{prefix}-g1");
		let (stored, mut live) = sample_game(&id, 31);
		store.create(&stored).await.unwrap_or_else(|e| panic!("create: {e}"));
		assert_eq!(store.create(&stored).await, Err(GameStoreError::Exists));
		assert_eq!(store.head_seq(&id).await, Ok(0));

		for seq in 0..10u64 {
			let a = live.legal_actions()[0];
			let entry = logged(&id, seq, &live, a.into());
			store.append(&entry).await.unwrap_or_else(|e| panic!("append {seq}: {e}"));
			live.apply(a).unwrap_or_else(|e| panic!("{e}"));
			// Same seq again loses.
			assert_eq!(
				store.append(&entry).await,
				Err(GameStoreError::Conflict { current: seq + 1, got: seq })
			);
		}
		// Skipping ahead is a conflict too.
		let ahead = logged(&id, 12, &live, ActionWire::Pass);
		assert_eq!(
			store.append(&ahead).await,
			Err(GameStoreError::Conflict { current: 10, got: 12 })
		);
		assert_eq!(store.head_seq(&id).await, Ok(10));

		let loaded = store.load(&id).await.unwrap_or_else(|e| panic!("load: {e}"));
		assert_eq!(loaded.game, stored);
		assert_eq!(loaded.seq(), 10);
		let replayed = loaded.record().replay().unwrap_or_else(|e| panic!("replay: {e}"));
		apa_kan_core::bench_log::assert_same_terminal(&live, &replayed)
			.unwrap_or_else(|e| panic!("replay mismatch: {e}"));

		let missing = format!("{prefix}-missing");
		assert!(matches!(store.load(&missing).await, Err(GameStoreError::NotFound)));
		assert_eq!(store.head_seq(&missing).await, Err(GameStoreError::NotFound));
		assert_eq!(
			store.append(&logged(&missing, 0, &live, ActionWire::Pass)).await,
			Err(GameStoreError::NotFound)
		);
		let mut bad = logged(&id, 10, &live, ActionWire::Pass);
		bad.seat = 3;
		assert!(matches!(store.append(&bad).await, Err(GameStoreError::Invalid(_))));
		assert!(store.count().await.unwrap_or(0) >= 1);
	}

	/// Many concurrent appends for the same seq: exactly one lands.
	pub async fn race(store: Arc<dyn GameStore>, prefix: &str) {
		let id = format!("{prefix}-race");
		let (stored, live) = sample_game(&id, 32);
		store.create(&stored).await.unwrap_or_else(|e| panic!("create: {e}"));
		let action: ActionWire = live.legal_actions()[0].into();
		let mut handles = Vec::new();
		for _ in 0..16 {
			let store = store.clone();
			let entry = logged(&id, 0, &live, action);
			handles.push(tokio::spawn(async move { store.append(&entry).await }));
		}
		let mut ok = 0;
		let mut conflicts = 0;
		for h in handles {
			match h.await {
				Ok(Ok(())) => ok += 1,
				Ok(Err(GameStoreError::Conflict { .. })) => conflicts += 1,
				other => panic!("unexpected race result: {other:?}"),
			}
		}
		assert_eq!(ok, 1);
		assert_eq!(conflicts, 15);
		assert_eq!(store.head_seq(&id).await, Ok(1));
		assert_eq!(store.load(&id).await.map(|g| g.log.len()), Ok(1));
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[tokio::test]
	async fn memory_conformance() {
		let store: Arc<dyn GameStore> = Arc::new(memory::MemoryGameStore::default());
		conformance::run(store.clone(), "mem").await;
		conformance::race(store, "mem").await;
	}

	#[tokio::test]
	async fn connect_parses_urls() {
		assert_eq!(connect("memory").await.map(|s| s.backend()).ok(), Some("memory"));
		assert!(matches!(connect("redis://x").await, Err(GameStoreError::Invalid(_))));
	}

	#[test]
	fn validation() {
		let (mut g, _) = conformance::sample_game("v", 1);
		assert_eq!(validate_new(&g), Ok(()));
		g.opening.id = "other".into();
		assert!(validate_new(&g).is_err());
		assert!(check_log("x", &[]).is_ok());
		assert_eq!(LoggedAuth::from_parts("seat_token", None), Ok(LoggedAuth::SeatToken));
		assert!(LoggedAuth::from_parts("bearer", None).is_err());
	}
}
