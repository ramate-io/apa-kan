//! Write side of a hosted Apa Kan game: one [`Transaction`] submits one action.
//!
//! A transaction names the game, the seat, the record sequence number it was
//! built against, and the action. [`apply_transaction`] checks it against the
//! authoritative [`GameRecord`] + [`Game`] pair and either appends the action
//! or returns a [`TxError`] without touching state. Sequence numbers make
//! submission idempotent-safe and race-free: two clients can't both play move
//! `n`, and a client that missed an opponent move gets [`TxError::StaleSeq`]
//! and refetches the view.
//!
//! Authorization is a per-seat bearer secret for now ([`TxAuth::SeatToken`]).
//! Hosts store only [`SeatSecret::hash`]. The enum is the seam for signed
//! ML-DSA transactions bound to an account once accounts own seats.
//!
//! No HTTP, no storage, no randomness: the host (`apa-kan-server`) mints ids
//! and secrets and keeps the games.

use apa_kan_core::game::Action;
use apa_kan_core::Game;
use apa_kan_view::{player_of, seat_of, ActionWire, GameRecord, Seat, StatusView};
use serde::{Deserialize, Serialize};
use sha3::{Digest, Sha3_256};

/// How a transaction proves it may act for its seat.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum TxAuth {
	/// Bearer secret handed out when the game was created (invite link).
	SeatToken { token: String },
}

/// Submit one action.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Transaction {
	pub game_id: String,
	/// The record's `seq` this action was chosen against.
	pub seq: u64,
	pub seat: Seat,
	pub action: ActionWire,
	pub auth: TxAuth,
}

/// What an accepted transaction did.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TxReceipt {
	pub game_id: String,
	/// Record `seq` after this action.
	pub seq: u64,
	pub seat: Seat,
	pub action: ActionWire,
	pub log_line: String,
	pub removals: usize,
	pub status: StatusView,
	pub to_move: Seat,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, thiserror::Error)]
#[serde(tag = "error", rename_all = "snake_case")]
pub enum TxError {
	#[error("game not found")]
	NotFound,
	#[error("transaction is for game {got}, not {expected}")]
	WrongGame { expected: String, got: String },
	#[error("stale sequence: game is at {current}, transaction was built at {got}")]
	StaleSeq { current: u64, got: u64 },
	#[error("seat must be 1 or 2, got {seat}")]
	BadSeat { seat: Seat },
	#[error("seat token does not match")]
	Unauthorized,
	#[error("game is over")]
	GameOver,
	#[error("not your turn: seat {to_move} is to move")]
	NotYourTurn { to_move: Seat },
	#[error("illegal action: {reason}")]
	Illegal { reason: String },
	#[error("host error: {reason}")]
	Host { reason: String },
}

/// A seat's bearer secret. Hosts keep only the hash.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SeatSecret {
	pub hash: [u8; 32],
}

impl SeatSecret {
	pub fn from_token(token: &str) -> Self {
		let mut h = Sha3_256::new();
		h.update(b"apa-kan/seat-token/v1\0");
		h.update(token.as_bytes());
		Self { hash: h.finalize().into() }
	}

	/// Constant-time compare of `token`'s hash against this secret.
	pub fn matches(&self, token: &str) -> bool {
		let other = Self::from_token(token).hash;
		let mut diff = 0u8;
		for (a, b) in self.hash.iter().zip(other.iter()) {
			diff |= a ^ b;
		}
		diff == 0
	}

	pub fn to_hex(&self) -> String {
		hex::encode(self.hash)
	}
}

/// Check `tx.auth` against the seat secrets. Seat range is checked first.
pub fn authorize(tx: &Transaction, secrets: &[SeatSecret; 2]) -> Result<(), TxError> {
	let player = player_of(tx.seat).ok_or(TxError::BadSeat { seat: tx.seat })?;
	match &tx.auth {
		TxAuth::SeatToken { token } => {
			if secrets[player.index()].matches(token) {
				Ok(())
			} else {
				Err(TxError::Unauthorized)
			}
		}
	}
}

/// Validate and apply `tx` to the authoritative pair. `game` must be
/// `record` replayed. On error neither is changed. Authorization is the
/// caller's job ([`authorize`]) so hosts can choose the scheme.
pub fn apply_transaction(
	record: &mut GameRecord,
	game: &mut Game,
	tx: &Transaction,
) -> Result<TxReceipt, TxError> {
	if tx.game_id != record.id {
		return Err(TxError::WrongGame { expected: record.id.clone(), got: tx.game_id.clone() });
	}
	let player = player_of(tx.seat).ok_or(TxError::BadSeat { seat: tx.seat })?;
	if tx.seq != record.seq() {
		return Err(TxError::StaleSeq { current: record.seq(), got: tx.seq });
	}
	if game.status().is_over() {
		return Err(TxError::GameOver);
	}
	if game.current_player() != player {
		return Err(TxError::NotYourTurn { to_move: seat_of(game.current_player()) });
	}
	let action: Action = tx.action.into();
	let mut next = game.clone();
	let outcome = next.apply(action).map_err(|reason| TxError::Illegal { reason })?;
	*game = next;
	record.actions.push(tx.action);
	Ok(TxReceipt {
		game_id: record.id.clone(),
		seq: record.seq(),
		seat: tx.seat,
		action: tx.action,
		log_line: outcome.log_line,
		removals: outcome.removals.len(),
		status: StatusView::from_status(game.status()),
		to_move: seat_of(game.current_player()),
	})
}

#[cfg(test)]
mod tests {
	#![allow(clippy::unwrap_used, clippy::expect_used)]

	use super::*;
	use apa_kan_core::bench_log::assert_same_terminal;
	use apa_kan_core::RuleSet;

	fn hosted() -> (GameRecord, Game) {
		let game = Game::new(RuleSet::default(), 11);
		(GameRecord::begin("g11", &game).unwrap(), game)
	}

	fn tx(record: &GameRecord, game: &Game, action: ActionWire) -> Transaction {
		Transaction {
			game_id: record.id.clone(),
			seq: record.seq(),
			seat: seat_of(game.current_player()),
			action,
			auth: TxAuth::SeatToken { token: "t".into() },
		}
	}

	#[test]
	fn accepted_transactions_extend_a_replayable_record() {
		let (mut rec, mut game) = hosted();
		for _ in 0..12 {
			let a = game.legal_actions()[0].into();
			let t = tx(&rec, &game, a);
			let r = apply_transaction(&mut rec, &mut game, &t).unwrap();
			assert_eq!(r.seq, rec.seq());
		}
		assert_same_terminal(&game, &rec.replay().unwrap()).unwrap();
	}

	#[test]
	fn rejections_leave_state_untouched() {
		let (mut rec, mut game) = hosted();
		let before = (rec.clone(), game.legal_actions());
		let legal: ActionWire = game.legal_actions()[0].into();

		let mut stale = tx(&rec, &game, legal);
		stale.seq = 5;
		assert_eq!(
			apply_transaction(&mut rec, &mut game, &stale),
			Err(TxError::StaleSeq { current: 0, got: 5 })
		);

		let mut wrong_seat = tx(&rec, &game, legal);
		wrong_seat.seat = 2;
		assert_eq!(
			apply_transaction(&mut rec, &mut game, &wrong_seat),
			Err(TxError::NotYourTurn { to_move: 1 })
		);

		let mut bad_seat = tx(&rec, &game, legal);
		bad_seat.seat = 3;
		assert_eq!(
			apply_transaction(&mut rec, &mut game, &bad_seat),
			Err(TxError::BadSeat { seat: 3 })
		);

		let mut other_game = tx(&rec, &game, legal);
		other_game.game_id = "nope".into();
		assert!(matches!(
			apply_transaction(&mut rec, &mut game, &other_game),
			Err(TxError::WrongGame { .. })
		));

		let illegal = tx(&rec, &game, ActionWire::Move { from: 200, to: 201 });
		assert!(matches!(
			apply_transaction(&mut rec, &mut game, &illegal),
			Err(TxError::Illegal { .. })
		));

		assert_eq!(rec, before.0);
		assert_eq!(game.legal_actions(), before.1);
	}

	#[test]
	fn replaying_the_same_seq_twice_is_rejected() {
		let (mut rec, mut game) = hosted();
		let t = tx(&rec, &game, game.legal_actions()[0].into());
		apply_transaction(&mut rec, &mut game, &t).unwrap();
		assert!(matches!(
			apply_transaction(&mut rec, &mut game, &t),
			Err(TxError::StaleSeq { current: 1, got: 0 })
		));
	}

	#[test]
	fn seat_tokens_hash_and_authorize() {
		let secrets = [SeatSecret::from_token("p1-secret"), SeatSecret::from_token("p2-secret")];
		assert_ne!(secrets[0], secrets[1]);
		assert_eq!(secrets[0].to_hex().len(), 64);
		let mut t = Transaction {
			game_id: "g".into(),
			seq: 0,
			seat: 1,
			action: ActionWire::Pass,
			auth: TxAuth::SeatToken { token: "p1-secret".into() },
		};
		assert_eq!(authorize(&t, &secrets), Ok(()));
		t.seat = 2;
		assert_eq!(authorize(&t, &secrets), Err(TxError::Unauthorized));
		t.seat = 0;
		assert_eq!(authorize(&t, &secrets), Err(TxError::BadSeat { seat: 0 }));
	}

	#[test]
	fn wire_shapes() {
		let t = Transaction {
			game_id: "g".into(),
			seq: 4,
			seat: 2,
			action: ActionWire::Claim { node: 3 },
			auth: TxAuth::SeatToken { token: "x".into() },
		};
		let json = serde_json::to_value(&t).unwrap();
		assert_eq!(json["action"]["kind"], "claim");
		assert_eq!(json["auth"]["type"], "seat_token");
		let err = serde_json::to_value(TxError::StaleSeq { current: 2, got: 1 }).unwrap();
		assert_eq!(err["error"], "stale_seq");
		assert_eq!(err["current"], 2);
	}
}
