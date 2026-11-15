//! Deterministic Fischer clocks. Wall time is the caller's input.
//!
//! Core owns remaining time, increment, and flag-fall. It never reads a
//! wall clock. Online authority (recv timestamps, disconnect) lives outside
//! this crate.

use crate::game::Player;

/// Decision (#58 Q21): 5-minute is Fischer 5+0.
pub const FIVE_MIN_INITIAL_MS: u64 = 5 * 60 * 1000;
pub const FIVE_MIN_INCREMENT_MS: u64 = 0;
/// Decision (#58 Q21): "2 + 1" is 2:00 + 1 s increment per completed move.
pub const TWO_PLUS_ONE_INITIAL_MS: u64 = 2 * 60 * 1000;
pub const TWO_PLUS_ONE_INCREMENT_MS: u64 = 1000;

/// Shipped Head-to-Head time controls.
///
/// Decision (#58 Q21): 5-minute = 5+0; 2 + 1 = 2:00 + 1 s/move.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum TimeControl {
	/// Fischer 5+0: 5:00 each, no increment.
	FiveMin,
	/// Fischer 2+1: 2:00 each, +1 second after every completed ADD/MOVE/PASS.
	TwoPlusOne,
}

impl TimeControl {
	pub fn parse(s: &str) -> Result<Self, String> {
		match s.to_ascii_lowercase().as_str() {
			"5+0" | "5min" | "five-min" | "fivemin" | "5" => Ok(Self::FiveMin),
			"2+1" | "2plus1" | "two-plus-one" | "twoplusone" => Ok(Self::TwoPlusOne),
			other => Err(format!("unknown time control '{other}' (expected 5+0|2+1)")),
		}
	}

	pub fn name(self) -> &'static str {
		match self {
			Self::FiveMin => "5+0",
			Self::TwoPlusOne => "2+1",
		}
	}

	pub fn initial_ms(self) -> u64 {
		match self {
			Self::FiveMin => FIVE_MIN_INITIAL_MS,
			Self::TwoPlusOne => TWO_PLUS_ONE_INITIAL_MS,
		}
	}

	pub fn increment_ms(self) -> u64 {
		match self {
			Self::FiveMin => FIVE_MIN_INCREMENT_MS,
			Self::TwoPlusOne => TWO_PLUS_ONE_INCREMENT_MS,
		}
	}
}

/// Result of deducting elapsed time from one player.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ClockTick {
	Ok,
	/// Remaining hit 0. The game layer maps this to [`crate::game::EndReason`].
	Flagged,
}

/// Remaining time per player plus the increment of a [`TimeControl`].
///
/// Decision (#58 Q26): the clock does not pause during a reconnect window.
/// A disconnect is the caller's choice of which `elapsed_ms` to pass into
/// [`Clock::apply_elapsed`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Clock {
	time_control: TimeControl,
	remaining_ms: [u64; 2],
	increment_ms: u64,
}

impl Clock {
	pub fn new(time_control: TimeControl) -> Self {
		let initial = time_control.initial_ms();
		Self {
			time_control,
			remaining_ms: [initial, initial],
			increment_ms: time_control.increment_ms(),
		}
	}

	pub fn time_control(&self) -> TimeControl {
		self.time_control
	}

	pub fn increment_ms(&self) -> u64 {
		self.increment_ms
	}

	pub fn remaining_ms(&self, player: Player) -> u64 {
		self.remaining_ms[player.index()]
	}

	pub fn remaining_pair(&self) -> [u64; 2] {
		self.remaining_ms
	}

	/// Deduct `elapsed_ms` from `player`.
	///
	/// Flags when elapsed **exceeds** remaining (the player overstayed). Using
	/// exactly the remaining milliseconds leaves 0 and is still a legal move;
	/// the increment, if any, is credited separately after the action.
	pub fn apply_elapsed(&mut self, player: Player, elapsed_ms: u64) -> ClockTick {
		let i = player.index();
		if elapsed_ms > self.remaining_ms[i] {
			self.remaining_ms[i] = 0;
			ClockTick::Flagged
		} else {
			self.remaining_ms[i] -= elapsed_ms;
			ClockTick::Ok
		}
	}

	/// Add the control's increment after a completed ADD, MOVE, or legal PASS.
	pub fn credit_increment(&mut self, player: Player) {
		let i = player.index();
		self.remaining_ms[i] = self.remaining_ms[i].saturating_add(self.increment_ms);
	}
}

#[cfg(test)]
mod tests {
	#![allow(clippy::unwrap_used, clippy::expect_used)]

	use super::*;
	use crate::game::{
		Action, EndReason, Game, GameStatus, Player, RuleSet, TileColor, LOW_VALUE_BOARD_LT,
	};

	#[test]
	fn five_min_is_five_plus_zero() {
		let tc = TimeControl::FiveMin;
		assert_eq!(tc.name(), "5+0");
		assert_eq!(tc.initial_ms(), 300_000);
		assert_eq!(tc.increment_ms(), 0);
		assert_eq!(TimeControl::parse("5+0").unwrap(), tc);
		assert_eq!(TimeControl::parse("5min").unwrap(), tc);
	}

	#[test]
	fn two_plus_one_is_two_minutes_plus_one_second() {
		let tc = TimeControl::TwoPlusOne;
		assert_eq!(tc.name(), "2+1");
		assert_eq!(tc.initial_ms(), 120_000);
		assert_eq!(tc.increment_ms(), 1_000);
		assert_eq!(TimeControl::parse("2+1").unwrap(), tc);
	}

	#[test]
	fn deducts_elapsed_without_flagging_when_time_remains() {
		let mut clock = Clock::new(TimeControl::FiveMin);
		assert_eq!(clock.apply_elapsed(Player::One, 1_500), ClockTick::Ok);
		assert_eq!(clock.remaining_ms(Player::One), 298_500);
		assert_eq!(clock.remaining_ms(Player::Two), 300_000);
	}

	#[test]
	fn exact_remaining_is_a_legal_move_not_a_flag() {
		let mut clock = Clock::new(TimeControl::FiveMin);
		assert_eq!(clock.apply_elapsed(Player::One, 300_000), ClockTick::Ok);
		assert_eq!(clock.remaining_ms(Player::One), 0);
	}

	#[test]
	fn overstay_flags_and_clamps_to_zero() {
		let mut clock = Clock::new(TimeControl::TwoPlusOne);
		assert_eq!(clock.apply_elapsed(Player::Two, 120_001), ClockTick::Flagged);
		assert_eq!(clock.remaining_ms(Player::Two), 0);
	}

	#[test]
	fn two_plus_one_credits_one_second_after_a_move() {
		let mut clock = Clock::new(TimeControl::TwoPlusOne);
		assert_eq!(clock.apply_elapsed(Player::One, 4_000), ClockTick::Ok);
		clock.credit_increment(Player::One);
		assert_eq!(clock.remaining_ms(Player::One), 117_000);
		assert_eq!(clock.remaining_ms(Player::Two), 120_000);
	}

	#[test]
	fn five_min_increment_is_a_no_op() {
		let mut clock = Clock::new(TimeControl::FiveMin);
		clock.apply_elapsed(Player::One, 10);
		clock.credit_increment(Player::One);
		assert_eq!(clock.remaining_ms(Player::One), 299_990);
	}

	fn live_two_tile_game() -> Game {
		let mut rules = RuleSet::default();
		rules.control_win = false;
		let mut game = Game::empty(rules);
		let spawns = game.board().spawns().to_vec();
		let t1 = game.alloc_tile(Player::One, TileColor::Green, 1, 1);
		let t2 = game.alloc_tile(Player::Two, TileColor::Blue, 1, 1);
		game.place_tile(spawns[0], t1);
		game.place_tile(spawns[4], t2);
		game.set_bench(Player::One, vec![]);
		game.set_bench(Player::Two, vec![]);
		game.set_current(Player::One);
		game.pad_above_low_value_keeping_empty_deck(&[Player::One, Player::Two]);
		game
	}

	#[test]
	fn unclocked_apply_ignores_elapsed() {
		let mut game = live_two_tile_game();
		let before = game.legal_actions();
		assert_eq!(game.apply_elapsed(Player::One, 999_999).unwrap(), ClockTick::Ok);
		assert_eq!(game.status(), GameStatus::InProgress);
		assert!(game.clock().is_none());
		assert_eq!(game.legal_actions(), before);
	}

	#[test]
	fn flag_is_a_loss_when_the_opponent_can_still_win() {
		// OPEN-Q #23
		let mut game = live_two_tile_game().with_time_control(TimeControl::FiveMin);
		assert_eq!(
			game.apply_elapsed(Player::One, FIVE_MIN_INITIAL_MS + 1).unwrap(),
			ClockTick::Flagged
		);
		assert_eq!(game.status(), GameStatus::Won { winner: Player::Two, reason: EndReason::Flag });
		assert_eq!(game.remaining_ms(Player::One), Some(0));
	}

	#[test]
	fn flag_is_a_draw_when_the_opponent_is_not_alive() {
		// OPEN-Q #23
		let mut rules = RuleSet::default();
		rules.control_win = false;
		let mut game = Game::empty(rules).with_time_control(TimeControl::FiveMin);
		let t = game.alloc_tile(Player::One, TileColor::Green, 1, 1);
		game.set_bench(Player::One, vec![t]);
		game.set_bench(Player::Two, vec![]);
		game.set_current(Player::One);
		assert!(game.is_alive(Player::One));
		assert!(!game.is_alive(Player::Two));
		assert_eq!(
			game.apply_elapsed(Player::One, FIVE_MIN_INITIAL_MS + 1).unwrap(),
			ClockTick::Flagged
		);
		assert_eq!(game.status(), GameStatus::Draw { reason: EndReason::FlagDraw });
	}

	#[test]
	fn flag_is_a_draw_when_the_opponent_would_lock_out() {
		// OPEN-Q #23
		let mut rules = RuleSet::default();
		rules.control_win = false;
		assert!(rules.no_moves_loses);
		let mut game = Game::empty(rules).with_time_control(TimeControl::FiveMin);
		for id in 0..game.board().node_count() as crate::board::NodeId {
			let owner = if id % 2 == 0 { Player::One } else { Player::Two };
			let tile = game.alloc_tile(owner, TileColor::Green, 3, 1);
			game.place_tile(id, tile);
		}
		game.set_bench(Player::One, vec![]);
		game.set_bench(Player::Two, vec![]);
		game.set_current(Player::One);
		assert!(!game.has_add_or_move());
		assert_eq!(
			game.apply_elapsed(Player::One, FIVE_MIN_INITIAL_MS + 1).unwrap(),
			ClockTick::Flagged
		);
		assert_eq!(game.status(), GameStatus::Draw { reason: EndReason::FlagDraw });
	}

	#[test]
	fn flag_is_a_draw_on_a_low_value_draw() {
		// OPEN-Q #23
		let mut rules = RuleSet::default();
		rules.control_win = false;
		let mut game = Game::empty(rules).with_time_control(TimeControl::FiveMin);
		let cap = usize::from(LOW_VALUE_BOARD_LT);
		assert!(cap > 1);
		let spawns = game.board().spawns().to_vec();
		for i in 0..(cap - 1) {
			let t1 = game.alloc_tile(Player::One, TileColor::Green, 1, 1);
			let t2 = game.alloc_tile(Player::Two, TileColor::Green, 1, 1);
			game.place_tile(spawns[i], t1);
			game.place_tile(spawns[i + 4], t2);
		}
		game.set_bench(Player::One, vec![]);
		game.set_bench(Player::Two, vec![]);
		game.set_current(Player::One);
		assert!(game.low_value_applies());
		assert_eq!(
			game.apply_elapsed(Player::One, FIVE_MIN_INITIAL_MS + 1).unwrap(),
			ClockTick::Flagged
		);
		assert_eq!(game.status(), GameStatus::Draw { reason: EndReason::FlagDraw });
	}

	#[test]
	fn apply_timed_credits_increment_and_keeps_the_board_move() {
		let mut game = live_two_tile_game().with_time_control(TimeControl::TwoPlusOne);
		let from = game.tiles().find(|(_, t)| t.owner == Player::One).unwrap().0;
		let dests = game.legal_destinations(from);
		assert!(!dests.is_empty());
		let to = dests[0];
		let outcome = game.apply_timed(Action::Move { from, to }, 3_000).unwrap();
		assert_eq!(outcome.action, Action::Move { from, to });
		assert_eq!(game.remaining_ms(Player::One), Some(120_000 - 3_000 + 1_000));
		assert_eq!(game.remaining_ms(Player::Two), Some(120_000));
	}

	#[test]
	fn apply_timed_does_not_credit_increment_on_flag() {
		let mut game = live_two_tile_game().with_time_control(TimeControl::TwoPlusOne);
		let from = game.tiles().find(|(_, t)| t.owner == Player::One).unwrap().0;
		let to = game.legal_destinations(from)[0];
		let err = game.apply_timed(Action::Move { from, to }, TWO_PLUS_ONE_INITIAL_MS + 1);
		assert!(err.is_err());
		assert_eq!(game.status(), GameStatus::Won { winner: Player::Two, reason: EndReason::Flag });
		assert_eq!(game.remaining_ms(Player::One), Some(0));
	}
}
