//! Game host behind `/v1/games`.
//!
//! Wraps `apa-kan-view` (records and views) and `apa-kan-tx` (transactions)
//! with ids, seat secrets, and a [`GameStore`]. Every accepted transaction is
//! appended to the store (compare-and-swap on its move number) before the
//! response goes out, so an acknowledged move survives a restart; loading a
//! game replays its opening record plus the log through core. An in-process
//! cache of live positions avoids replaying on every request and is
//! refreshed whenever the store's head moves (another instance, or a peer
//! relay, committed first). Hosted games are human-vs-human, unclocked,
//! default rules, with the deal auto-picked by core's default setup.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use apa_kan_core::{Game, RuleSet};
use apa_kan_game_store::{GameStore, GameStoreError, LoadedGame, LoggedAuth, LoggedTx, StoredGame};
use apa_kan_tx::{
	apply_transaction, authorize, SeatSecret, Transaction, TxAuth, TxError, TxReceipt,
};
use apa_kan_view::{GameRecord, GameView, Seat};
use rand::RngCore;
use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;

/// Refuse new games past this many, so an open endpoint can't exhaust memory.
pub const DEFAULT_MAX_GAMES: usize = 10_000;
/// Seat label cap.
pub const MAX_LABEL_LEN: usize = 40;

#[derive(Clone, Debug, Default, Deserialize)]
pub struct NewGame {
	/// Deal seed. Random when omitted.
	#[serde(default)]
	pub seed: Option<u64>,
	/// Display names for seats 1 and 2.
	#[serde(default)]
	pub labels: Option<[String; 2]>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SeatInvite {
	pub seat: Seat,
	/// Bearer secret for this seat. Shown once; the host keeps only a hash.
	pub token: String,
	/// Client route that claims this seat: `/game/<id>?seat=N&token=…`.
	pub path: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CreatedGame {
	pub game_id: String,
	pub seats: [SeatInvite; 2],
	pub view: GameView,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TxResponse {
	pub receipt: TxReceipt,
	pub view: GameView,
}

/// Where hosted games are stored, from the environment:
/// `APAKAN_GAME_STORE_URL` (`memory`, `sqlite:<path>`, `postgres://…`), else
/// `APA_KAN_DATABASE_URL` (share the account database), else a SQLite file
/// in the temp dir.
pub fn game_store_url_from_env() -> String {
	let nonempty = |k: &str| std::env::var(k).ok().filter(|v| !v.trim().is_empty());
	nonempty("APAKAN_GAME_STORE_URL")
		.or_else(|| {
			// main.rs accepts a bare path for the account DB; mirror that.
			nonempty("APA_KAN_DATABASE_URL").map(|u| {
				if u.contains(':') && !u.starts_with('/') {
					u
				} else {
					format!("sqlite:{u}")
				}
			})
		})
		.unwrap_or_else(|| {
			format!("sqlite:{}", std::env::temp_dir().join("apa-kan-games.db").display())
		})
}

struct HostedGame {
	record: GameRecord,
	game: Game,
	secrets: [SeatSecret; 2],
	labels: [String; 2],
}

impl HostedGame {
	fn view(&self) -> GameView {
		GameView::new(&self.record, &self.game, [&self.labels[0], &self.labels[1]])
	}

	fn seq(&self) -> u64 {
		self.record.seq()
	}

	/// Rebuild from storage by replaying the log through core.
	fn from_loaded(loaded: &LoadedGame) -> Result<Self, TxError> {
		let record = loaded.record();
		let game = record.replay().map_err(|e| host_err(format!("stored game replay: {e}")))?;
		let secret = |i: usize| {
			SeatSecret::from_hex(&loaded.game.seat_secrets[i])
				.ok_or_else(|| host_err("stored seat secret is not valid hex"))
		};
		Ok(Self {
			record,
			game,
			secrets: [secret(0)?, secret(1)?],
			labels: loaded.game.labels.clone(),
		})
	}
}

type Slot = Arc<Mutex<HostedGame>>;

pub struct GameHost {
	store: Arc<dyn GameStore>,
	/// Live positions; each game has its own lock so games don't serialize
	/// on one another. Dropped wholesale past `max_cached` (it's only a cache).
	cache: Mutex<HashMap<String, Slot>>,
	max_games: usize,
	max_cached: usize,
}

impl Default for GameHost {
	/// Memory-backed host (tests, `AppState::new`).
	fn default() -> Self {
		Self::new(
			Arc::new(apa_kan_game_store::memory::MemoryGameStore::default()),
			DEFAULT_MAX_GAMES,
		)
	}
}

fn random_hex(bytes: usize) -> String {
	let mut buf = vec![0u8; bytes];
	rand::rngs::OsRng.fill_bytes(&mut buf);
	hex::encode(buf)
}

fn clean_label(s: &str, fallback: &str) -> String {
	let t: String = s.trim().chars().filter(|c| !c.is_control()).take(MAX_LABEL_LEN).collect();
	if t.is_empty() {
		fallback.to_string()
	} else {
		t
	}
}

fn host_err(reason: impl Into<String>) -> TxError {
	TxError::Host { reason: reason.into() }
}

fn store_err(e: GameStoreError) -> TxError {
	match e {
		GameStoreError::NotFound => TxError::NotFound,
		GameStoreError::Conflict { current, got } => TxError::StaleSeq { current, got },
		other => host_err(other.to_string()),
	}
}

fn now_unix() -> i64 {
	SystemTime::now()
		.duration_since(UNIX_EPOCH)
		.map(|d| d.as_secs() as i64)
		.unwrap_or(0)
}

fn logged_auth(auth: &TxAuth) -> LoggedAuth {
	match auth {
		TxAuth::SeatToken { .. } => LoggedAuth::SeatToken,
	}
}

impl GameHost {
	pub fn new(store: Arc<dyn GameStore>, max_games: usize) -> Self {
		Self { store, cache: Mutex::new(HashMap::new()), max_games, max_cached: 4_096 }
	}

	pub fn store_backend(&self) -> &'static str {
		self.store.backend()
	}

	pub async fn game_count(&self) -> Result<u64, TxError> {
		self.store.count().await.map_err(store_err)
	}

	/// Deal and host a new game. The deal runs on a blocking thread; the game
	/// is in the store before its invites are returned.
	pub async fn create(&self, req: NewGame) -> Result<CreatedGame, TxError> {
		if self.game_count().await? >= self.max_games as u64 {
			return Err(host_err("game capacity reached"));
		}
		let seed = req.seed.unwrap_or_else(|| rand::rngs::OsRng.next_u64());
		let id = random_hex(12);
		let deal_id = id.clone();
		let (record, game) = tokio::task::spawn_blocking(move || {
			let game = Game::new(RuleSet::default(), seed);
			GameRecord::begin(deal_id, &game).map(|r| (r, game))
		})
		.await
		.map_err(|e| host_err(e.to_string()))?
		.map_err(host_err)?;
		let tokens = [random_hex(24), random_hex(24)];
		let labels = match req.labels {
			Some([a, b]) => [clean_label(&a, "P1"), clean_label(&b, "P2")],
			None => ["P1".into(), "P2".into()],
		};
		let secrets = [SeatSecret::from_token(&tokens[0]), SeatSecret::from_token(&tokens[1])];
		let stored = StoredGame {
			game_id: id.clone(),
			opening: record.clone(),
			seat_secrets: [secrets[0].to_hex(), secrets[1].to_hex()],
			labels: labels.clone(),
			created_at_unix: now_unix(),
		};
		self.store.create(&stored).await.map_err(store_err)?;
		let hosted = HostedGame { record, game, secrets, labels };
		let view = hosted.view();
		self.insert_cache(id.clone(), hosted).await;
		let invite = |seat: Seat, token: &str| SeatInvite {
			seat,
			token: token.to_string(),
			path: format!("/game/{id}?seat={seat}&token={token}"),
		};
		Ok(CreatedGame {
			game_id: id.clone(),
			seats: [invite(1, &tokens[0]), invite(2, &tokens[1])],
			view,
		})
	}

	async fn insert_cache(&self, id: String, hosted: HostedGame) -> Slot {
		let mut cache = self.cache.lock().await;
		if cache.len() >= self.max_cached {
			cache.clear();
		}
		let slot = Arc::new(Mutex::new(hosted));
		cache.insert(id, slot.clone());
		slot
	}

	async fn load_from_store(&self, id: &str) -> Result<HostedGame, TxError> {
		let loaded = self.store.load(id).await.map_err(store_err)?;
		tokio::task::spawn_blocking(move || HostedGame::from_loaded(&loaded))
			.await
			.map_err(|e| host_err(e.to_string()))?
	}

	/// Cached slot for `id`, loading it from the store on a miss.
	async fn slot(&self, id: &str) -> Result<Slot, TxError> {
		if let Some(slot) = self.cache.lock().await.get(id).cloned() {
			return Ok(slot);
		}
		let hosted = self.load_from_store(id).await?;
		Ok(self.insert_cache(id.to_string(), hosted).await)
	}

	/// Bring a locked cached game up to the store's head if it fell behind.
	async fn refresh(&self, id: &str, hosted: &mut HostedGame) -> Result<(), TxError> {
		let head = self.store.head_seq(id).await.map_err(store_err)?;
		if head != hosted.seq() {
			*hosted = self.load_from_store(id).await?;
		}
		Ok(())
	}

	pub async fn view(&self, id: &str) -> Result<GameView, TxError> {
		let slot = self.slot(id).await?;
		let mut hosted = slot.lock().await;
		self.refresh(id, &mut hosted).await?;
		Ok(hosted.view())
	}

	/// Validate `tx` against the engine, commit it to the store, then advance
	/// the cached position. Nothing is acknowledged before the commit.
	pub async fn submit(&self, id: &str, tx: &Transaction) -> Result<TxResponse, TxError> {
		let slot = self.slot(id).await?;
		let mut hosted = slot.lock().await;
		self.refresh(id, &mut hosted).await?;
		authorize(tx, &hosted.secrets)?;
		let mut record = hosted.record.clone();
		let mut game = hosted.game.clone();
		let receipt = apply_transaction(&mut record, &mut game, tx)?;
		let entry = LoggedTx {
			game_id: id.to_string(),
			seq: tx.seq,
			seat: tx.seat,
			action: tx.action,
			auth: logged_auth(&tx.auth),
			committed_at_unix: now_unix(),
		};
		match self.store.append(&entry).await {
			Ok(()) => {}
			Err(GameStoreError::Conflict { current, got }) => {
				// Someone else committed this move number first; resync.
				if let Ok(fresh) = self.load_from_store(id).await {
					*hosted = fresh;
				}
				return Err(TxError::StaleSeq { current, got });
			}
			Err(e) => return Err(store_err(e)),
		}
		hosted.record = record;
		hosted.game = game;
		Ok(TxResponse { receipt, view: hosted.view() })
	}

	/// Forget cached positions (tests: simulate a restart on the same store).
	pub async fn clear_cache(&self) {
		self.cache.lock().await.clear();
	}
}

#[cfg(test)]
mod tests {
	#![allow(clippy::unwrap_used, clippy::expect_used)]

	use super::*;
	use apa_kan_game_store::memory::MemoryGameStore;
	use apa_kan_tx::TxAuth;
	use apa_kan_view::ActionWire;

	fn tx_for(view: &GameView, token: &str, action: ActionWire) -> Transaction {
		Transaction {
			game_id: view.id.clone(),
			seq: view.seq,
			seat: view.to_move,
			action,
			auth: TxAuth::SeatToken { token: token.into() },
		}
	}

	#[tokio::test]
	async fn create_view_submit_round_trip() {
		let host = GameHost::default();
		let created = host
			.create(NewGame { seed: Some(3), labels: Some(["ana".into(), " ".into()]) })
			.await
			.unwrap();
		assert_eq!(created.view.seats[0].label, "ana");
		assert_eq!(created.view.seats[1].label, "P2");
		assert!(created.seats[0]
			.path
			.starts_with(&format!("/game/{}?seat=1&token=", created.game_id)));
		let mut view = host.view(&created.game_id).await.unwrap();
		for _ in 0..6 {
			let token = &created.seats[usize::from(view.to_move - 1)].token;
			let res = host.submit(&view.id, &tx_for(&view, token, view.legal[0])).await.unwrap();
			assert_eq!(res.view.seq, view.seq + 1);
			view = res.view;
		}
		let replayed = view.replay().unwrap();
		assert_eq!(replayed.turn_count(), view.turn);
	}

	#[tokio::test]
	async fn wrong_token_and_unknown_game() {
		let host = GameHost::default();
		let created = host.create(NewGame { seed: Some(4), labels: None }).await.unwrap();
		let view = created.view.clone();
		let other_seat_token = &created.seats[1].token;
		assert_eq!(
			host.submit(&view.id, &tx_for(&view, other_seat_token, view.legal[0]))
				.await
				.unwrap_err(),
			TxError::Unauthorized
		);
		assert_eq!(host.view("missing").await.unwrap_err(), TxError::NotFound);
		assert_eq!(host.view(&view.id).await.unwrap().seq, 0);
	}

	#[tokio::test]
	async fn capacity_is_enforced() {
		let host = GameHost::new(Arc::new(MemoryGameStore::default()), 1);
		host.create(NewGame { seed: Some(1), labels: None }).await.unwrap();
		assert!(matches!(host.create(NewGame::default()).await, Err(TxError::Host { .. })));
	}

	/// Acknowledged moves survive dropping every cached position, and a second
	/// host on the same store sees them and rejects a replayed move number.
	#[tokio::test]
	async fn moves_are_durable_and_shared_through_the_store() {
		let store: Arc<dyn GameStore> = Arc::new(MemoryGameStore::default());
		let a = GameHost::new(store.clone(), 10);
		let b = GameHost::new(store.clone(), 10);
		let created = a.create(NewGame { seed: Some(9), labels: None }).await.unwrap();
		let mut view = created.view.clone();
		for _ in 0..4 {
			let token = &created.seats[usize::from(view.to_move - 1)].token;
			view = a.submit(&view.id, &tx_for(&view, token, view.legal[0])).await.unwrap().view;
		}
		assert_eq!(store.head_seq(&view.id).await, Ok(4));
		a.clear_cache().await;
		let reloaded = a.view(&view.id).await.unwrap();
		assert_eq!(serde_json::to_value(&reloaded).unwrap(), serde_json::to_value(&view).unwrap());

		// Host b moves; host a's stale cache refreshes from the store.
		let token = &created.seats[usize::from(view.to_move - 1)].token;
		let stale = tx_for(&view, token, view.legal[0]);
		let next = b.submit(&view.id, &stale).await.unwrap().view;
		assert_eq!(a.view(&view.id).await.unwrap().seq, next.seq);
		assert_eq!(
			a.submit(&view.id, &stale).await.unwrap_err(),
			TxError::StaleSeq { current: 5, got: 4 }
		);
	}

	/// Concurrent submits for one move number: exactly one is acknowledged.
	#[tokio::test]
	async fn concurrent_same_seq_submits_commit_once() {
		let store: Arc<dyn GameStore> = Arc::new(MemoryGameStore::default());
		let hosts: Vec<Arc<GameHost>> =
			(0..4).map(|_| Arc::new(GameHost::new(store.clone(), 10))).collect();
		let created = hosts[0].create(NewGame { seed: Some(11), labels: None }).await.unwrap();
		let view = created.view.clone();
		let token = created.seats[usize::from(view.to_move - 1)].token.clone();
		let mut joins = Vec::new();
		for i in 0..12 {
			let host = hosts[i % hosts.len()].clone();
			let action = view.legal[i % view.legal.len()];
			let tx = tx_for(&view, &token, action);
			let id = view.id.clone();
			joins.push(tokio::spawn(async move { host.submit(&id, &tx).await }));
		}
		let mut ok = 0;
		for j in joins {
			match j.await.unwrap() {
				Ok(_) => ok += 1,
				Err(TxError::StaleSeq { current: 1, got: 0 }) => {}
				Err(e) => panic!("unexpected: {e:?}"),
			}
		}
		assert_eq!(ok, 1);
		assert_eq!(store.head_seq(&view.id).await, Ok(1));
	}
}
