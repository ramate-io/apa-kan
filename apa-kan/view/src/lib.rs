//! Read side of a hosted Apa Kan game.
//!
//! Core is deterministic: rules + seed + the completed 7+4 deal + the ordered
//! action list rebuild the exact [`Game`]. So the wire contract is:
//!
//! - [`ActionWire`] — one [`Action`] as tagged JSON.
//! - [`GameRecord`] — the replayable transcript (rules, deal, actions). Any
//!   client can [`GameRecord::replay`] it with `apa-kan-core` and get the same
//!   position the server holds.
//! - [`GameView`] — a snapshot for thin clients and humans: status, side to
//!   move, board, seats, legal actions, plus the record itself.
//!
//! - [`HostedGameView`] — derived state of one `game/<id>` ledger stream:
//!   phase (open / playing / finished / cancelled), seat claims by account,
//!   the stream head a client builds its next transaction on, and the
//!   [`GameView`] once both seats are in.
//!
//! Views are the read side only: nothing here mutates. Every state change is
//! a signed transaction in `apa-kan-tx`. HTTP lives in
//! `apa-kan-server`. This crate has no Bevy, no HTTP, and no randomness.
//!
//! Hosted games are unclocked for now: the record has no time control because
//! a server-authoritative clock needs wall-time handling the server does not
//! have yet.

use apa_kan_core::bench_log::{RulesLog, SetupLog};
use apa_kan_core::game::{Action, BudgetToken, EndReason, GameStatus, Player};
use apa_kan_core::{Game, NodeId, RuleSet};
use serde::{Deserialize, Serialize};

/// Wire schema version for [`GameRecord`] and [`GameView`].
pub const VIEW_SCHEMA: u32 = 1;

/// Seat number on the wire: 1 = P1 (light, moves first), 2 = P2.
pub type Seat = u8;

pub fn seat_of(player: Player) -> Seat {
	match player {
		Player::One => 1,
		Player::Two => 2,
	}
}

pub fn player_of(seat: Seat) -> Option<Player> {
	match seat {
		1 => Some(Player::One),
		2 => Some(Player::Two),
		_ => None,
	}
}

/// One action as tagged JSON: `{"kind":"move","from":3,"to":9}`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ActionWire {
	Add { bench_index: usize, node: NodeId },
	Move { from: NodeId, to: NodeId },
	Claim { node: NodeId },
	Swap { from: NodeId, to: NodeId },
	Convert { node: NodeId },
	Pass,
}

impl From<Action> for ActionWire {
	fn from(a: Action) -> Self {
		match a {
			Action::Add { bench_index, node } => Self::Add { bench_index, node },
			Action::Move { from, to } => Self::Move { from, to },
			Action::Claim { node } => Self::Claim { node },
			Action::Swap { from, to } => Self::Swap { from, to },
			Action::Convert { node } => Self::Convert { node },
			Action::Pass => Self::Pass,
		}
	}
}

impl From<ActionWire> for Action {
	fn from(a: ActionWire) -> Self {
		match a {
			ActionWire::Add { bench_index, node } => Self::Add { bench_index, node },
			ActionWire::Move { from, to } => Self::Move { from, to },
			ActionWire::Claim { node } => Self::Claim { node },
			ActionWire::Swap { from, to } => Self::Swap { from, to },
			ActionWire::Convert { node } => Self::Convert { node },
			ActionWire::Pass => Self::Pass,
		}
	}
}

/// Replayable transcript of one hosted game.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct GameRecord {
	pub v: u32,
	pub id: String,
	pub seed: u64,
	pub rules: RulesLog,
	/// Completed 27-tile collections and 7+4 picks for both seats.
	pub setup: SetupLog,
	/// Blank spawn tokens each seat starts with.
	pub p1_tokens: usize,
	pub p2_tokens: usize,
	/// Applied actions in order. `actions.len()` is the record's sequence
	/// number; a transaction must name it to be accepted.
	pub actions: Vec<ActionWire>,
}

impl GameRecord {
	/// Snapshot the opening of `game` (before any action) under `id`.
	/// The game must carry a setup record (any `Game::new*` / `start*` deal).
	pub fn begin(id: impl Into<String>, game: &Game) -> Result<Self, String> {
		if game.turn_count() != 0 {
			return Err("GameRecord::begin needs an opening position".into());
		}
		let setup = SetupLog::from_game(game).ok_or("game has no setup record")?;
		Ok(Self {
			v: VIEW_SCHEMA,
			id: id.into(),
			seed: game.seed(),
			rules: RulesLog::from_rules(*game.rules()),
			setup,
			p1_tokens: game.budget_token_count(Player::One),
			p2_tokens: game.budget_token_count(Player::Two),
			actions: Vec::new(),
		})
	}

	/// Number of applied actions.
	pub fn seq(&self) -> u64 {
		self.actions.len() as u64
	}

	pub fn rule_set(&self) -> Result<RuleSet, String> {
		self.rules.to_rules()
	}

	/// The opening position, before any action.
	pub fn opening(&self) -> Result<Game, String> {
		if self.v != VIEW_SCHEMA {
			return Err(format!("unsupported record schema v={}", self.v));
		}
		let rules = self.rule_set()?;
		let mut game = Game::from_setup_record(rules, self.seed, self.setup.to_record()?);
		game.set_budget_tokens(Player::One, vec![BudgetToken::Blank; self.p1_tokens]);
		game.set_budget_tokens(Player::Two, vec![BudgetToken::Blank; self.p2_tokens]);
		game.set_current(Player::One);
		Ok(game)
	}

	/// Rebuild the current position by applying every recorded action.
	pub fn replay(&self) -> Result<Game, String> {
		self.replay_prefix(self.actions.len())
	}

	/// Rebuild the position after the first `n` actions.
	pub fn replay_prefix(&self, n: usize) -> Result<Game, String> {
		let mut game = self.opening()?;
		for (i, a) in self.actions.iter().take(n).enumerate() {
			game.apply((*a).into()).map_err(|e| format!("action {i}: {e}"))?;
		}
		Ok(game)
	}
}

/// Who sits in a seat.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SeatView {
	pub seat: Seat,
	pub label: String,
	pub bench: usize,
	/// Usable ADD permission: min(spawn tokens, remaining deck).
	pub budget: usize,
	pub board_tiles: usize,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum StatusView {
	InProgress,
	Won { winner: Seat, reason: String },
	Draw { reason: String },
}

impl StatusView {
	pub fn from_status(s: GameStatus) -> Self {
		match s {
			GameStatus::InProgress => Self::InProgress,
			GameStatus::Won { winner, reason } => {
				Self::Won { winner: seat_of(winner), reason: reason_name(reason) }
			}
			GameStatus::Draw { reason } => Self::Draw { reason: reason_name(reason) },
		}
	}

	pub fn is_over(&self) -> bool {
		!matches!(self, Self::InProgress)
	}
}

fn reason_name(r: EndReason) -> String {
	format!("{r:?}")
}

/// One occupied node.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PieceView {
	pub node: NodeId,
	pub owner: Seat,
	pub tile_id: u32,
	pub kind: String,
}

/// Snapshot of a hosted game.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct GameView {
	pub v: u32,
	pub id: String,
	pub seq: u64,
	pub status: StatusView,
	pub to_move: Seat,
	pub turn: u32,
	pub seats: [SeatView; 2],
	pub board: Vec<PieceView>,
	/// Legal actions for the side to move (empty once the game is over).
	pub legal: Vec<ActionWire>,
	pub record: GameRecord,
}

impl GameView {
	/// Build a view of `game`, which must be `record` replayed.
	pub fn new(record: &GameRecord, game: &Game, labels: [&str; 2]) -> Self {
		let status = StatusView::from_status(game.status());
		let seat = |p: Player, label: &str| SeatView {
			seat: seat_of(p),
			label: label.to_string(),
			bench: game.bench(p).len(),
			budget: game.budget(p),
			board_tiles: game.tiles().filter(|(_, t)| t.owner == p).count(),
		};
		let legal = if status.is_over() {
			Vec::new()
		} else {
			game.legal_actions().into_iter().map(ActionWire::from).collect()
		};
		Self {
			v: VIEW_SCHEMA,
			id: record.id.clone(),
			seq: record.seq(),
			status,
			to_move: seat_of(game.current_player()),
			turn: game.turn_count(),
			seats: [seat(Player::One, labels[0]), seat(Player::Two, labels[1])],
			board: game
				.tiles()
				.map(|(node, t)| PieceView {
					node,
					owner: seat_of(t.owner),
					tile_id: t.id,
					kind: t.kind_label(),
				})
				.collect(),
			legal,
			record: record.clone(),
		}
	}

	/// Replay the embedded record. Clients use this to get a playable `Game`.
	pub fn replay(&self) -> Result<Game, String> {
		self.record.replay()
	}
}

/// Where a ledger stream's chain ends: what the next transaction builds on.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LedgerHead {
	/// Deployment / network id every transaction must name.
	pub chain: String,
	/// Stream id, e.g. `game/<id>`.
	pub stream: String,
	/// Next transaction's `seq` (number of committed transactions).
	pub seq: u64,
	/// Hex hash of the last committed transaction (the next one's `prev`).
	pub head: String,
}

/// An account holding a seat.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SeatClaim {
	pub seat: Seat,
	/// Hex `AccountId`.
	pub account: String,
	pub label: String,
	/// Ephemeral X25519 key (hex) for encrypted UDP previews, if the seat
	/// sent one in its `game.create` / `game.join`.
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub preview_pk: Option<String>,
}

/// How a hosted game ended, for ratings. Present once the game is decided:
/// finished on the board, resigned, or forfeited.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct GameResultView {
	/// Winning seat (`None` for a draw).
	pub winner: Option<Seat>,
	/// Losing seat (`None` for a draw).
	pub loser: Option<Seat>,
	/// `board` (engine win), `draw`, `resign` (includes leaving a started
	/// game), or `no_reveal_forfeit` (the creator never revealed).
	pub reason: String,
}

/// Lifecycle of a hosted game stream.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GamePhase {
	/// Created; waiting for the other seat to join.
	Open,
	/// Both seats in; waiting for the creator to reveal the committed salt
	/// that fixes the deal. The joiner may cancel (`no_reveal`).
	Revealing,
	Playing,
	Finished,
	/// Creator cancelled before anyone joined.
	Cancelled,
}

/// Derived state of one hosted game stream.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct HostedGameView {
	pub v: u32,
	pub id: String,
	pub phase: GamePhase,
	pub ledger: LedgerHead,
	pub seats: [Option<SeatClaim>; 2],
	/// Creator's hex salt commitment (`game.create`).
	#[serde(default)]
	pub seed_commit: String,
	/// Why the game was cancelled, if it was.
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub cancel_reason: Option<String>,
	/// Position, once the creator has revealed (the deal is fixed then).
	pub game: Option<GameView>,
	/// Rating-ready result once the game is decided.
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub result: Option<GameResultView>,
}

impl HostedGameView {
	/// Seat held by `account` (hex), if any.
	pub fn seat_of_account(&self, account: &str) -> Option<Seat> {
		self.seats.iter().flatten().find(|c| c.account == account).map(|c| c.seat)
	}

	/// First open seat, if the game still takes a join.
	pub fn open_seat(&self) -> Option<Seat> {
		if self.phase != GamePhase::Open {
			return None;
		}
		self.seats.iter().position(Option::is_none).map(|i| i as Seat + 1)
	}
}

/// Status of one key on an account (mirrors the account crate).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum KeyState {
	/// Added by an email recovery that has not finished its cooldown.
	PendingRecovery,
	Active,
	Revoked,
}

/// One key on an account.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AccountKeyView {
	/// Hex ML-DSA-65 verifying key.
	pub vk: String,
	pub label: String,
	pub status: KeyState,
	/// `seq` of the account transaction that added the key.
	pub added_seq: u64,
}

/// Where an email recovery stands.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RecoveryStage {
	/// The pending key asked; the host has not issued the emailed token yet.
	Requested,
	/// The operator recorded the token hash and cooldown; the token was mailed.
	Challenged,
	/// The pending key proved the token; it finishes once the cooldown passes.
	Confirmed,
}

/// An email recovery in progress.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecoveryView {
	/// Hex key being recovered onto the account.
	pub pending_vk: String,
	/// `full_recovery` or `email_link_approval`.
	pub kind: String,
	pub stage: RecoveryStage,
	/// When `account.recovery_finish` becomes acceptable (set by the operator).
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub usable_at_unix: Option<i64>,
}

/// A passkey that can restore the account seed (the wrapped seed itself is
/// served by `GET /v1/accounts/{id}/passkeys/{credential_id}`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PasskeyBackupView {
	/// Hex WebAuthn credential id.
	pub credential_id: String,
	pub added_seq: u64,
}

/// `GET /v1/accounts/{id}`: the fold of `account/<id>`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AccountView {
	pub v: u32,
	/// Hex account id.
	pub id: String,
	pub ledger: LedgerHead,
	pub label: String,
	pub keys: Vec<AccountKeyView>,
	/// An email is on file (the address itself is not in the view).
	pub email_set: bool,
	pub recovery_opt_in: bool,
	pub approve_via_email: bool,
	/// Passkeys holding a wrapped copy of the seed.
	#[serde(default)]
	pub passkeys: Vec<PasskeyBackupView>,
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub recovery: Option<RecoveryView>,
}

/// `GET /v1/admin/roles`: the fold of `admin/roles`.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RolesView {
	pub v: u32,
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub ledger: Option<LedgerHead>,
	/// Hex account id → sorted role names.
	pub roles: std::collections::BTreeMap<String, Vec<String>>,
}

#[cfg(test)]
mod tests {
	#![allow(clippy::unwrap_used, clippy::expect_used)]

	use super::*;
	use apa_kan_core::bench_log::assert_same_terminal;

	fn opening(seed: u64) -> Game {
		Game::new(RuleSet::default(), seed)
	}

	/// Play the first legal action each ply (a deterministic, tactic-free walk).
	fn walk(record: &mut GameRecord, live: &mut Game, plies: usize) {
		for _ in 0..plies {
			if live.status().is_over() {
				break;
			}
			let action = live.legal_actions()[0];
			live.apply(action).unwrap();
			record.actions.push(action.into());
		}
	}

	#[test]
	fn action_wire_round_trips_through_json() {
		let all = [
			Action::Add { bench_index: 2, node: 7 },
			Action::Move { from: 1, to: 9 },
			Action::Claim { node: 4 },
			Action::Swap { from: 3, to: 5 },
			Action::Convert { node: 0 },
			Action::Pass,
		];
		for a in all {
			let w = ActionWire::from(a);
			let json = serde_json::to_string(&w).unwrap();
			let back: ActionWire = serde_json::from_str(&json).unwrap();
			assert_eq!(Action::from(back), a);
		}
		assert_eq!(
			serde_json::to_string(&ActionWire::Move { from: 3, to: 9 }).unwrap(),
			r#"{"kind":"move","from":3,"to":9}"#
		);
	}

	#[test]
	fn record_opening_matches_live_opening() {
		for seed in [1, 7, 42] {
			let live = opening(seed);
			let rec = GameRecord::begin(format!("g{seed}"), &live).unwrap();
			let rebuilt = rec.opening().unwrap();
			assert_same_terminal(&live, &rebuilt).unwrap();
			assert_eq!(live.legal_actions(), rebuilt.legal_actions());
			assert_eq!(live.current_player(), rebuilt.current_player());
		}
	}

	#[test]
	fn replay_matches_live_after_many_actions_and_json() {
		let mut live = opening(5);
		let mut rec = GameRecord::begin("g5", &live).unwrap();
		walk(&mut rec, &mut live, 60);
		let json = serde_json::to_string(&rec).unwrap();
		let back: GameRecord = serde_json::from_str(&json).unwrap();
		assert_eq!(back, rec);
		let replayed = back.replay().unwrap();
		assert_same_terminal(&live, &replayed).unwrap();
		assert_eq!(live.legal_actions(), replayed.legal_actions());
		assert_eq!(rec.seq(), rec.actions.len() as u64);
	}

	#[test]
	fn view_reports_side_to_move_and_legal_actions() {
		let mut live = opening(9);
		let mut rec = GameRecord::begin("g9", &live).unwrap();
		walk(&mut rec, &mut live, 3);
		let view = GameView::new(&rec, &live, ["ana", "bo"]);
		assert_eq!(view.seq, 3);
		assert_eq!(view.to_move, seat_of(live.current_player()));
		assert_eq!(view.seats[0].label, "ana");
		assert_eq!(view.legal.len(), live.legal_actions().len());
		assert_eq!(view.board.len(), live.tiles().count());
		let json = serde_json::to_string(&view).unwrap();
		let back: GameView = serde_json::from_str(&json).unwrap();
		assert_eq!(back, view);
		assert_same_terminal(&live, &back.replay().unwrap()).unwrap();
	}

	#[test]
	fn begin_rejects_started_games_and_bad_schema() {
		let mut live = opening(3);
		let a = live.legal_actions()[0];
		live.apply(a).unwrap();
		assert!(GameRecord::begin("x", &live).is_err());
		let mut rec = GameRecord::begin("y", &opening(3)).unwrap();
		rec.v = 99;
		assert!(rec.replay().is_err());
	}

	#[test]
	fn seats() {
		assert_eq!(player_of(1), Some(Player::One));
		assert_eq!(player_of(2), Some(Player::Two));
		assert_eq!(player_of(0), None);
		assert_eq!(seat_of(Player::Two), 2);
		assert!(!StatusView::InProgress.is_over());
	}
}
