//! State machine for `game/<id>` streams.
//!
//! ```text
//!   game.create (seq 0) ──► Open ──game.join──► Revealing ──game.reveal──► Playing ──game.move*──► Finished
//!                             │                    │                         └──game.resign──────► Finished
//!                             │                    └──game.cancel (joiner)──► Cancelled (no_reveal)
//!                             └──game.cancel (creator)──────────────────────► Cancelled
//! ```
//!
//! [`GameState::apply`] is pure and deterministic: same state + same
//! transaction ⇒ same next state, on any host or peer. It checks the
//! sequence number and `prev` link, then the payload against the phase, the
//! signer's seat, and the engine. It never verifies signatures; callers run
//! [`crate::admit`] (hosts) or [`crate::verify_self_contained`] (replay) first
//! and pass the resulting id.

use apa_kan_core::game::Action;
use apa_kan_core::{Game, RuleSet};
use apa_kan_view::{
	player_of, seat_of, GamePhase, GameRecord, GameView, HostedGameView, LedgerHead, Seat,
	SeatClaim, StatusView, VIEW_SCHEMA,
};
use sha3::{Digest, Sha3_256};

use crate::envelope::{verify_self_contained, SignedTx, TxPayload, GENESIS_PREV};
use crate::{TxError, TxReceipt};

/// Seat label cap.
pub const MAX_LABEL_LEN: usize = 40;
/// Domain for the deal seed derived at reveal.
pub const DEAL_SEED_DOMAIN: &[u8] = b"apa-kan/deal-seed/v2";
/// Domain for the creator's salt commitment in `game.create`.
pub const DEAL_COMMIT_DOMAIN: &[u8] = b"apa-kan/deal-commit/v1";

/// `game/<id>` for a game id.
pub fn game_stream(id: &str) -> String {
	format!("game/{id}")
}

/// Game id of a `game/<id>` stream. Ids are 1..=64 of `[0-9a-z-]`.
pub fn game_id_of(stream: &str) -> Option<&str> {
	let id = stream.strip_prefix("game/")?;
	let ok = !id.is_empty()
		&& id.len() <= 64
		&& id.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'z' | b'-'));
	ok.then_some(id)
}

fn clean_label(s: &str, seat: Seat) -> String {
	let t: String = s.trim().chars().filter(|c| !c.is_control()).take(MAX_LABEL_LEN).collect();
	if t.is_empty() {
		format!("P{seat}")
	} else {
		t
	}
}

/// Hex commitment to `salt` for `stream`, put in `game.create`. Bound to the
/// stream so a commitment can't be reused in another game.
pub fn seed_commitment(stream: &str, salt: &[u8; 32]) -> String {
	let mut h = Sha3_256::new();
	h.update(DEAL_COMMIT_DOMAIN);
	h.update((stream.len() as u64).to_le_bytes());
	h.update(stream.as_bytes());
	h.update(salt);
	hex::encode(h.finalize())
}

/// Domain for deriving a creator's salt from a device secret.
pub const DEAL_SALT_DOMAIN: &[u8] = b"apa-kan/deal-salt/v1";

/// Deterministic per-game salt from a device secret, so a creator can reveal
/// after a reload without storing anything. Any 32 random bytes work; this
/// is a client convenience, not a protocol rule.
pub fn deal_salt(secret: &[u8; 32], stream: &str) -> [u8; 32] {
	let mut h = Sha3_256::new();
	h.update(DEAL_SALT_DOMAIN);
	h.update(secret);
	h.update(stream.as_bytes());
	h.finalize().into()
}

fn parse_hash(s: &str) -> Option<[u8; 32]> {
	if s.len() != 64 || !s.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f')) {
		return None;
	}
	hex::decode(s).ok()?.try_into().ok()
}

/// Deal seed from the creator's revealed salt and the join transaction's id.
/// The creator committed to the salt before seeing the join; the joiner
/// signed the join before seeing the salt. Neither picks the deal alone.
pub fn deal_seed(salt: &[u8; 32], join_id: &[u8; 32]) -> u64 {
	let mut h = Sha3_256::new();
	h.update(DEAL_SEED_DOMAIN);
	h.update(salt);
	h.update(join_id);
	let d: [u8; 32] = h.finalize().into();
	let mut b = [0u8; 8];
	b.copy_from_slice(&d[..8]);
	u64::from_le_bytes(b)
}

/// Folded state of one game stream.
#[derive(Clone, Debug)]
pub struct GameState {
	pub id: String,
	pub chain: String,
	pub phase: GamePhase,
	pub seats: [Option<SeatClaim>; 2],
	/// Hex account that created the game.
	pub creator: String,
	pub record: Option<GameRecord>,
	pub game: Option<Game>,
	/// Seat that resigned, if the game ended that way.
	pub resigned: Option<Seat>,
	/// Why a cancelled game was cancelled (`creator_withdrew`, `no_reveal`).
	pub cancel_reason: Option<String>,
	/// Creator's hex salt commitment from `game.create`.
	pub seed_commit: String,
	/// Number of applied transactions (next `seq`).
	pub next_seq: u64,
	/// Hex id of the last applied transaction.
	pub head: String,
	join_id: Option<[u8; 32]>,
}

fn seat_index(seat: Seat) -> Result<usize, TxError> {
	player_of(seat)
		.map(|p| p.index())
		.ok_or(TxError::BadEnvelope { reason: format!("seat must be 1 or 2, got {seat}") })
}

impl GameState {
	/// Apply `tx` (already verified, with id `tx_id`) to `state` (`None` for a
	/// stream with no transactions yet). Returns the next state and a receipt;
	/// on error nothing changes.
	pub fn apply(
		state: Option<&GameState>,
		tx: &SignedTx,
		tx_id: &[u8; 32],
	) -> Result<(GameState, TxReceipt), TxError> {
		let b = &tx.body;
		let id = game_id_of(&b.stream)
			.ok_or_else(|| TxError::UnsupportedStream { stream: b.stream.clone() })?;
		let signer = b.signer.to_hex();
		let mut log_line = None;
		let mut next = match state {
			None => {
				if b.seq != 0 {
					return Err(TxError::NotFound);
				}
				if b.prev != GENESIS_PREV {
					return Err(TxError::PrevMismatch { expected: GENESIS_PREV.into() });
				}
				let TxPayload::GameCreate { seat, label, seed_commit } = &b.payload else {
					return Err(TxError::NotFound);
				};
				if parse_hash(seed_commit).is_none() {
					return Err(TxError::BadEnvelope {
						reason: "seed_commit must be 64 lowercase hex".into(),
					});
				}
				let mut seats: [Option<SeatClaim>; 2] = [None, None];
				seats[seat_index(*seat)?] = Some(SeatClaim {
					seat: *seat,
					account: signer.clone(),
					label: clean_label(label, *seat),
				});
				GameState {
					id: id.to_string(),
					chain: b.chain.clone(),
					phase: GamePhase::Open,
					seats,
					creator: signer.clone(),
					record: None,
					game: None,
					resigned: None,
					cancel_reason: None,
					seed_commit: seed_commit.clone(),
					next_seq: 0,
					head: GENESIS_PREV.into(),
					join_id: None,
				}
			}
			Some(s) => {
				if b.stream != game_stream(&s.id) || b.chain != s.chain {
					return Err(TxError::UnsupportedStream { stream: b.stream.clone() });
				}
				if b.seq != s.next_seq {
					return Err(TxError::StaleSeq { current: s.next_seq, got: b.seq });
				}
				if b.prev != s.head {
					return Err(TxError::PrevMismatch { expected: s.head.clone() });
				}
				let mut n = s.clone();
				match &b.payload {
					TxPayload::GameCreate { .. } => {
						return Err(TxError::StaleSeq { current: s.next_seq, got: 0 });
					}
					TxPayload::GameJoin { label } => {
						if s.phase != GamePhase::Open {
							return Err(TxError::WrongPhase { phase: s.phase });
						}
						if s.seats.iter().flatten().any(|c| c.account == signer) {
							return Err(TxError::AlreadySeated);
						}
						let i = s
							.seats
							.iter()
							.position(Option::is_none)
							.ok_or(TxError::WrongPhase { phase: s.phase })?;
						let seat = i as Seat + 1;
						n.seats[i] = Some(SeatClaim {
							seat,
							account: signer.clone(),
							label: clean_label(label, seat),
						});
						n.join_id = Some(*tx_id);
						n.phase = GamePhase::Revealing;
					}
					TxPayload::GameReveal { salt } => {
						if s.phase != GamePhase::Revealing {
							return Err(TxError::WrongPhase { phase: s.phase });
						}
						if s.creator != signer {
							return Err(TxError::Unauthorized);
						}
						let salt = parse_hash(salt).ok_or(TxError::BadReveal)?;
						if seed_commitment(&b.stream, &salt) != s.seed_commit {
							return Err(TxError::BadReveal);
						}
						let join_id = s.join_id.ok_or_else(|| TxError::Host {
							reason: "revealing without a join".into(),
						})?;
						let game = Game::new(RuleSet::default(), deal_seed(&salt, &join_id));
						let record = GameRecord::begin(s.id.clone(), &game)
							.map_err(|reason| TxError::Host { reason })?;
						n.record = Some(record);
						n.game = Some(game);
						n.phase = GamePhase::Playing;
					}
					TxPayload::GameMove { action } => {
						let my_seat = n.require_playing_seat(&signer)?;
						let (Some(game), Some(record)) = (n.game.as_mut(), n.record.as_mut())
						else {
							return Err(TxError::Host {
								reason: "playing without a position".into(),
							});
						};
						let to_move = seat_of(game.current_player());
						if my_seat != to_move {
							return Err(TxError::NotYourTurn { to_move });
						}
						let outcome = game
							.apply(Action::from(*action))
							.map_err(|reason| TxError::Illegal { reason })?;
						record.actions.push(*action);
						log_line = Some(outcome.log_line);
						if game.status().is_over() {
							n.phase = GamePhase::Finished;
						}
					}
					TxPayload::GameResign => {
						let my_seat = n.require_playing_seat(&signer)?;
						n.resigned = Some(my_seat);
						n.phase = GamePhase::Finished;
					}
					TxPayload::GameCancel => {
						// Open: only the creator withdraws. Revealing: only the
						// joiner may abandon (the creator, who can already
						// compute the deal, may not cancel to re-roll it).
						let (allowed, reason) = match s.phase {
							GamePhase::Open => (s.creator == signer, "creator_withdrew"),
							GamePhase::Revealing => (
								s.creator != signer
									&& s.seats.iter().flatten().any(|c| c.account == signer),
								"no_reveal",
							),
							phase => return Err(TxError::WrongPhase { phase }),
						};
						if !allowed {
							return Err(TxError::Unauthorized);
						}
						n.phase = GamePhase::Cancelled;
						n.cancel_reason = Some(reason.into());
					}
					other => {
						return Err(TxError::BadEnvelope {
							reason: format!("{} is not a game transaction", other.kind()),
						})
					}
				}
				n
			}
		};
		let seq = next.next_seq;
		next.next_seq += 1;
		next.head = hex::encode(tx_id);
		let receipt = TxReceipt {
			tx_id: next.head.clone(),
			stream: b.stream.clone(),
			seq,
			kind: b.payload.kind().into(),
			phase: Some(next.phase),
			log_line,
		};
		Ok((next, receipt))
	}

	fn require_playing_seat(&self, signer: &str) -> Result<Seat, TxError> {
		match self.phase {
			GamePhase::Playing => {}
			GamePhase::Finished => return Err(TxError::GameOver),
			phase => return Err(TxError::WrongPhase { phase }),
		}
		self.seats
			.iter()
			.flatten()
			.find(|c| c.account == signer)
			.map(|c| c.seat)
			.ok_or(TxError::Unauthorized)
	}

	/// Derived read model.
	pub fn view(&self) -> HostedGameView {
		let label = |i: usize| {
			self.seats[i]
				.as_ref()
				.map(|c| c.label.clone())
				.unwrap_or_else(|| format!("P{}", i + 1))
		};
		let game = match (&self.record, &self.game) {
			(Some(record), Some(game)) => {
				let (l1, l2) = (label(0), label(1));
				let mut v = GameView::new(record, game, [&l1, &l2]);
				if let Some(loser) = self.resigned {
					v.status = StatusView::Won { winner: 3 - loser, reason: "Resign".into() };
					v.legal.clear();
				}
				Some(v)
			}
			_ => None,
		};
		HostedGameView {
			v: VIEW_SCHEMA,
			id: self.id.clone(),
			phase: self.phase,
			ledger: LedgerHead {
				chain: self.chain.clone(),
				stream: game_stream(&self.id),
				seq: self.next_seq,
				head: self.head.clone(),
			},
			seats: self.seats.clone(),
			seed_commit: self.seed_commit.clone(),
			cancel_reason: self.cancel_reason.clone(),
			game,
		}
	}
}

/// Fold committed transactions (in `seq` order) into state, re-verifying
/// every signature, chain id, and link.
pub fn replay(chain: &str, txs: &[SignedTx]) -> Result<Option<GameState>, TxError> {
	let mut state: Option<GameState> = None;
	for tx in txs {
		let id = verify_self_contained(tx, chain)?;
		let (next, _) = GameState::apply(state.as_ref(), tx, &id)?;
		state = Some(next);
	}
	Ok(state)
}

#[cfg(test)]
mod tests {
	#![allow(clippy::unwrap_used, clippy::expect_used)]

	use super::*;
	use crate::envelope::{admit, KeyStatus, TxSigner};
	use apa_kan_account::Seed;
	use apa_kan_view::ActionWire;

	const CHAIN: &str = "test";
	const SALT: [u8; 32] = [5; 32];

	struct Table {
		a: TxSigner,
		b: TxSigner,
		state: Option<GameState>,
		log: Vec<SignedTx>,
	}

	impl Table {
		fn new() -> Self {
			Self {
				a: TxSigner::from_seed(&Seed::from_bytes([1; 32])),
				b: TxSigner::from_seed(&Seed::from_bytes([2; 32])),
				state: None,
				log: Vec::new(),
			}
		}

		fn build(&self, who: &TxSigner, payload: TxPayload) -> SignedTx {
			let (seq, prev) = match &self.state {
				Some(s) => (s.next_seq, s.head.clone()),
				None => (0, GENESIS_PREV.to_string()),
			};
			who.sign(CHAIN, "game/t1", seq, &prev, payload).unwrap()
		}

		fn try_send(&mut self, tx: SignedTx) -> Result<TxReceipt, TxError> {
			let id = admit(&tx, CHAIN, KeyStatus::UnknownAccount)?;
			let (next, r) = GameState::apply(self.state.as_ref(), &tx, &id)?;
			self.state = Some(next);
			self.log.push(tx);
			Ok(r)
		}

		fn send(&mut self, who: u8, payload: TxPayload) -> Result<TxReceipt, TxError> {
			let tx =
				if who == 1 { self.build(&self.a, payload) } else { self.build(&self.b, payload) };
			self.try_send(tx)
		}

		fn create(&mut self, who: u8, seat: Seat, label: &str) -> Result<TxReceipt, TxError> {
			let seed_commit = seed_commitment("game/t1", &SALT);
			self.send(who, TxPayload::GameCreate { seat, label: label.into(), seed_commit })
		}

		fn reveal(&mut self) -> Result<TxReceipt, TxError> {
			self.send(1, TxPayload::GameReveal { salt: hex::encode(SALT) })
		}

		fn s(&self) -> &GameState {
			self.state.as_ref().unwrap()
		}
	}

	#[test]
	fn create_join_play_and_replay() {
		let mut t = Table::new();
		assert_eq!(
			t.send(1, TxPayload::GameMove { action: ActionWire::Pass }),
			Err(TxError::NotFound)
		);
		t.create(1, 2, "ana").unwrap();
		assert_eq!(t.s().phase, GamePhase::Open);
		assert!(t.s().view().game.is_none());
		assert_eq!(t.s().view().open_seat(), Some(1));
		// Creator can't also take the other seat.
		assert_eq!(
			t.send(1, TxPayload::GameJoin { label: "x".into() }),
			Err(TxError::AlreadySeated)
		);
		t.send(2, TxPayload::GameJoin { label: "bo".into() }).unwrap();
		assert_eq!(t.s().phase, GamePhase::Revealing);
		assert!(t.s().view().game.is_none());
		// Only the creator reveals.
		assert_eq!(
			t.send(2, TxPayload::GameReveal { salt: hex::encode(SALT) }),
			Err(TxError::Unauthorized)
		);
		t.reveal().unwrap();
		assert_eq!(t.s().phase, GamePhase::Playing);
		let v = t.s().view();
		assert_eq!(v.seat_of_account(&t.b.account().to_hex()), Some(1));
		assert_eq!(v.seats[1].as_ref().unwrap().label, "ana");
		assert_eq!(
			t.send(2, TxPayload::GameJoin { label: "y".into() }).unwrap_err(),
			TxError::WrongPhase { phase: GamePhase::Playing }
		);

		for _ in 0..8 {
			let g = t.s().game.as_ref().unwrap();
			let to_move = seat_of(g.current_player());
			let action: ActionWire = g.legal_actions()[0].into();
			// Seat 1 is account b; seat 2 is account a.
			let (mover, waiter) = if to_move == 1 { (2, 1) } else { (1, 2) };
			assert_eq!(
				t.send(waiter, TxPayload::GameMove { action }),
				Err(TxError::NotYourTurn { to_move })
			);
			let r = t.send(mover, TxPayload::GameMove { action }).unwrap();
			assert!(r.log_line.is_some());
		}
		let rebuilt = replay(CHAIN, &t.log).unwrap().unwrap();
		assert_eq!(
			serde_json::to_value(rebuilt.view()).unwrap(),
			serde_json::to_value(t.s().view()).unwrap()
		);
		assert_eq!(rebuilt.view().game.unwrap().seq, 8);
	}

	#[test]
	fn replay_and_fork_protection() {
		let mut t = Table::new();
		t.create(1, 1, "a").unwrap();
		let join = t.build(&t.b, TxPayload::GameJoin { label: "b".into() });
		t.try_send(join.clone()).unwrap();
		// Resubmitting the same signed tx is stale.
		assert_eq!(t.try_send(join), Err(TxError::StaleSeq { current: 2, got: 1 }));
		t.reveal().unwrap();
		// Right seq, wrong prev (a fork off the genesis) is refused.
		let g = t.s().game.as_ref().unwrap();
		let action: ActionWire = g.legal_actions()[0].into();
		let fork =
			t.a.sign(CHAIN, "game/t1", 3, &"11".repeat(32), TxPayload::GameMove { action })
				.unwrap();
		assert!(matches!(t.try_send(fork), Err(TxError::PrevMismatch { .. })));
		// A tx for another stream or chain can't be spliced in.
		let s = t.s().clone();
		let other =
			t.a.sign(CHAIN, "game/t2", s.next_seq, &s.head, TxPayload::GameMove { action })
				.unwrap();
		let id = admit(&other, CHAIN, KeyStatus::UnknownAccount).unwrap();
		assert!(matches!(
			GameState::apply(Some(&s), &other, &id),
			Err(TxError::UnsupportedStream { .. })
		));
		let wrong_chain =
			t.a.sign("prod", "game/t1", s.next_seq, &s.head, TxPayload::GameMove { action })
				.unwrap();
		assert!(matches!(t.try_send(wrong_chain), Err(TxError::WrongChain { .. })));
		// A tampered log does not replay.
		let mut log = t.log.clone();
		log[1].body.payload = TxPayload::GameJoin { label: "evil".into() };
		assert_eq!(replay(CHAIN, &log).unwrap_err(), TxError::BadSignature);
	}

	#[test]
	fn resign_and_cancel() {
		let mut t = Table::new();
		t.create(1, 1, "a").unwrap();
		assert_eq!(t.send(2, TxPayload::GameCancel), Err(TxError::Unauthorized));
		assert_eq!(
			t.send(2, TxPayload::GameResign),
			Err(TxError::WrongPhase { phase: GamePhase::Open })
		);
		t.send(1, TxPayload::GameCancel).unwrap();
		assert_eq!(t.s().phase, GamePhase::Cancelled);
		assert!(t.send(2, TxPayload::GameJoin { label: "b".into() }).is_err());

		let mut t = Table::new();
		t.create(1, 1, "a").unwrap();
		t.send(2, TxPayload::GameJoin { label: "b".into() }).unwrap();
		t.reveal().unwrap();
		let outsider = TxSigner::from_seed(&Seed::from_bytes([9; 32]));
		let tx = t.build(&outsider, TxPayload::GameResign);
		assert_eq!(t.try_send(tx), Err(TxError::Unauthorized));
		t.send(2, TxPayload::GameResign).unwrap();
		let v = t.s().view();
		assert_eq!(v.phase, GamePhase::Finished);
		let g = v.game.unwrap();
		assert_eq!(g.status, StatusView::Won { winner: 1, reason: "Resign".into() });
		assert!(g.legal.is_empty());
		assert_eq!(
			t.send(1, TxPayload::GameMove { action: ActionWire::Pass }),
			Err(TxError::GameOver)
		);
		assert_eq!(replay(CHAIN, &t.log).unwrap().unwrap().resigned, Some(2));
	}

	#[test]
	fn commit_reveal_rules() {
		let mut t = Table::new();
		// Malformed commitment.
		assert!(matches!(
			t.send(
				1,
				TxPayload::GameCreate { seat: 1, label: "a".into(), seed_commit: "x".into() }
			),
			Err(TxError::BadEnvelope { .. })
		));
		t.create(1, 1, "a").unwrap();
		assert_eq!(t.reveal(), Err(TxError::WrongPhase { phase: GamePhase::Open }));
		t.send(2, TxPayload::GameJoin { label: "b".into() }).unwrap();
		// Wrong salt, garbage salt.
		assert_eq!(
			t.send(1, TxPayload::GameReveal { salt: "66".repeat(32) }),
			Err(TxError::BadReveal)
		);
		assert_eq!(t.send(1, TxPayload::GameReveal { salt: "zz".into() }), Err(TxError::BadReveal));
		// The creator can't cancel to re-roll a deal it can already compute.
		assert_eq!(t.send(1, TxPayload::GameCancel), Err(TxError::Unauthorized));
		// The joiner can walk away from a creator who never reveals.
		t.send(2, TxPayload::GameCancel).unwrap();
		assert_eq!(t.s().phase, GamePhase::Cancelled);
		assert_eq!(t.s().view().cancel_reason.as_deref(), Some("no_reveal"));
		assert_eq!(t.reveal(), Err(TxError::WrongPhase { phase: GamePhase::Cancelled }));

		// The deal depends on the join: same salt, different joiner, other deal.
		let deal = |joiner: u8| {
			let mut t = Table::new();
			t.b = TxSigner::from_seed(&Seed::from_bytes([joiner; 32]));
			t.create(1, 1, "a").unwrap();
			t.send(2, TxPayload::GameJoin { label: "b".into() }).unwrap();
			t.reveal().unwrap();
			t.s().record.as_ref().unwrap().seed
		};
		assert_ne!(deal(2), deal(3));
		assert_eq!(deal(2), deal(2));
		// A commitment is bound to its stream.
		assert_ne!(seed_commitment("game/a", &SALT), seed_commitment("game/b", &SALT));
	}

	#[test]
	fn deal_depends_on_both_create_and_join() {
		assert_ne!(deal_seed(&[1; 32], &[2; 32]), deal_seed(&[1; 32], &[3; 32]));
		assert_ne!(deal_seed(&[1; 32], &[2; 32]), deal_seed(&[4; 32], &[2; 32]));
		assert_eq!(game_id_of("game/ab-9"), Some("ab-9"));
		assert_eq!(game_id_of("game/AB"), None);
		assert_eq!(game_id_of("account/x"), None);
	}
}
