//! In-memory game host behind `/v1/games`.
//!
//! Wraps `apa-kan-view` (records and views) and `apa-kan-tx` (transactions)
//! with ids, seat secrets, and a lock. Games live in process memory only;
//! durable transcripts are a later store repo (#56). Hosted games are
//! human-vs-human, unclocked, default rules, with the deal auto-picked by
//! core's default setup.

use std::collections::HashMap;
use std::sync::Mutex;

use apa_kan_core::{Game, RuleSet};
use apa_kan_tx::{apply_transaction, authorize, SeatSecret, Transaction, TxError, TxReceipt};
use apa_kan_view::{GameRecord, GameView, Seat};
use rand::RngCore;
use serde::{Deserialize, Serialize};

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
}

pub struct GameHost {
	games: Mutex<HashMap<String, HostedGame>>,
	max_games: usize,
}

impl Default for GameHost {
	fn default() -> Self {
		Self::new(DEFAULT_MAX_GAMES)
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

impl GameHost {
	pub fn new(max_games: usize) -> Self {
		Self { games: Mutex::new(HashMap::new()), max_games }
	}

	pub fn len(&self) -> usize {
		self.games.lock().map(|g| g.len()).unwrap_or(0)
	}

	pub fn is_empty(&self) -> bool {
		self.len() == 0
	}

	/// Deal and host a new game. CPU-bound (deck picks): call from a blocking
	/// task in async handlers.
	pub fn create(&self, req: NewGame) -> Result<CreatedGame, TxError> {
		if self.len() >= self.max_games {
			return Err(host_err("game capacity reached"));
		}
		let seed = req.seed.unwrap_or_else(|| rand::rngs::OsRng.next_u64());
		let game = Game::new(RuleSet::default(), seed);
		let id = random_hex(12);
		let record = GameRecord::begin(id.clone(), &game).map_err(host_err)?;
		let tokens = [random_hex(24), random_hex(24)];
		let labels = match req.labels {
			Some([a, b]) => [clean_label(&a, "P1"), clean_label(&b, "P2")],
			None => ["P1".into(), "P2".into()],
		};
		let hosted = HostedGame {
			record,
			game,
			secrets: [SeatSecret::from_token(&tokens[0]), SeatSecret::from_token(&tokens[1])],
			labels,
		};
		let view = hosted.view();
		let mut games = self.games.lock().map_err(|_| host_err("host lock poisoned"))?;
		if games.len() >= self.max_games {
			return Err(host_err("game capacity reached"));
		}
		games.insert(id.clone(), hosted);
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

	pub fn view(&self, id: &str) -> Result<GameView, TxError> {
		let games = self.games.lock().map_err(|_| host_err("host lock poisoned"))?;
		games.get(id).map(HostedGame::view).ok_or(TxError::NotFound)
	}

	pub fn submit(&self, id: &str, tx: &Transaction) -> Result<TxResponse, TxError> {
		let mut games = self.games.lock().map_err(|_| host_err("host lock poisoned"))?;
		let hosted = games.get_mut(id).ok_or(TxError::NotFound)?;
		authorize(tx, &hosted.secrets)?;
		let receipt = apply_transaction(&mut hosted.record, &mut hosted.game, tx)?;
		Ok(TxResponse { receipt, view: hosted.view() })
	}
}

#[cfg(test)]
mod tests {
	#![allow(clippy::unwrap_used, clippy::expect_used)]

	use super::*;
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

	#[test]
	fn create_view_submit_round_trip() {
		let host = GameHost::default();
		let created = host
			.create(NewGame { seed: Some(3), labels: Some(["ana".into(), " ".into()]) })
			.unwrap();
		assert_eq!(created.view.seats[0].label, "ana");
		assert_eq!(created.view.seats[1].label, "P2");
		assert!(created.seats[0]
			.path
			.starts_with(&format!("/game/{}?seat=1&token=", created.game_id)));
		let mut view = host.view(&created.game_id).unwrap();
		for _ in 0..6 {
			let token = &created.seats[usize::from(view.to_move - 1)].token;
			let res = host.submit(&view.id, &tx_for(&view, token, view.legal[0])).unwrap();
			assert_eq!(res.view.seq, view.seq + 1);
			view = res.view;
		}
		let replayed = view.replay().unwrap();
		assert_eq!(replayed.turn_count(), view.turn);
	}

	#[test]
	fn wrong_token_and_unknown_game() {
		let host = GameHost::default();
		let created = host.create(NewGame { seed: Some(4), labels: None }).unwrap();
		let view = created.view.clone();
		let other_seat_token = &created.seats[1].token;
		assert_eq!(
			host.submit(&view.id, &tx_for(&view, other_seat_token, view.legal[0]))
				.unwrap_err(),
			TxError::Unauthorized
		);
		assert_eq!(host.view("missing").unwrap_err(), TxError::NotFound);
		assert_eq!(host.view(&view.id).unwrap().seq, 0);
	}

	#[test]
	fn capacity_is_enforced() {
		let host = GameHost::new(1);
		host.create(NewGame { seed: Some(1), labels: None }).unwrap();
		assert!(matches!(host.create(NewGame::default()), Err(TxError::Host { .. })));
	}
}
