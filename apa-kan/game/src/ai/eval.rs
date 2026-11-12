//! Heuristic evaluation. Positive is good for `me`.

use crate::game::{Game, GameStatus, Player, LOW_VALUE_BOARD_LT};

pub const WIN: i32 = 50_000;

/// ADD-option value of one budget token. Kept well below a surviving board
/// tile so search spends to develop instead of hoarding. #24 used 90/token
/// and the low-value ending never fired.
const BUDGET_TOKEN: i32 = 24;

/// Integer closeness scale for the low-value race (`0..=` this).
const LOW_VALUE_CLOSE_MAX: i32 = 64;

pub fn terminal_score(game: &Game, me: Player) -> Option<i32> {
	match game.status() {
		GameStatus::InProgress => {
			if game.rules().no_moves_loses && !game.has_add_or_move() {
				let ply = game.turn_count() as i32;
				if game.current_player() == me {
					Some(-WIN + ply)
				} else {
					Some(WIN - ply)
				}
			} else {
				None
			}
		}
		GameStatus::Won { winner, .. } => {
			let ply = game.turn_count() as i32;
			if winner == me {
				Some(WIN - ply)
			} else {
				Some(-WIN + ply)
			}
		}
		// Turn-limit and other draws are 0 so a side that is ahead will not
		// turtle into the 200-turn cap.
		GameStatus::Draw { .. } => Some(0),
	}
}

/// Pre-center-rush heuristic kept so `MinimaxLegacy` can be compared with the
/// current evaluator on the same search.
pub fn evaluate_legacy(game: &Game, me: Player, repeat_count: u16) -> i32 {
	if let Some(term) = terminal_score(game, me) {
		return term;
	}
	let opp = me.other();
	let mut material = 0i32;
	let mut at_risk = 0i32;
	let mut margin = 0i32;
	let mut contact = 0i32;
	let mut geometry = 0i32;
	let mut spawn_control = 0i32;
	let board = game.board();
	let r0 = board.construction_radius();
	let turn = game.turn_count();
	let my_board = game.board_count(me) as i32;
	let opp_board = game.board_count(opp) as i32;

	let mut contact_pairs = 0i32;
	for (node, tile) in game.tiles() {
		let sign = if tile.owner == me { 1 } else { -1 };
		material += sign * (48 + 24 * i32::from(tile.weight) + 6 * i32::from(tile.dots));
		if let Some((matches, mismatches)) = game.tile_score(node) {
			margin += sign * 8 * (matches - mismatches);
			if mismatches >= matches - 1 {
				at_risk += sign;
			}
		}
		let mut enemy_nbs = 0i32;
		for &nb in board.neighbors(node) {
			if let Some(other) = game.tile_at(nb) {
				if other.owner != tile.owner {
					enemy_nbs += 1;
				}
			}
		}
		if tile.owner == me {
			contact_pairs += enemy_nbs;
		}
		contact += sign * 20 * enemy_nbs;

		let r = board.radius_of(node);
		let geo = if node == board.center() {
			if game.rules().center_hold_win {
				180
			} else {
				22
			}
		} else if board.is_spawn(node) {
			if turn > 20 {
				-12
			} else {
				3
			}
		} else if r < r0 * 1.15 {
			16
		} else {
			9
		};
		geometry += sign * geo;
	}

	for &spawn in board.spawns() {
		match game.tile_at(spawn) {
			Some(tile) => {
				spawn_control += if tile.owner == me { 6 } else { -6 };
			}
			None => {
				let mut mine = 0i32;
				let mut theirs = 0i32;
				for &nb in board.neighbors(spawn) {
					if let Some(t) = game.tile_at(nb) {
						if t.owner == me {
							mine += 1;
						} else {
							theirs += 1;
						}
					}
				}
				spawn_control += 8 * (mine - theirs);
			}
		}
	}

	let mut bench_score = 0i32;
	for tile in game.bench(me) {
		bench_score += 28 + 10 * i32::from(tile.weight);
	}
	for tile in game.bench(opp) {
		bench_score -= 28 + 10 * i32::from(tile.weight);
	}
	let bag = 3 * (game.bag_len(me) as i32 - game.bag_len(opp) as i32);
	let presence = 32 * (my_board - opp_board);

	let mobility = 3 * (mobility_of(game, me) - mobility_of(game, opp));

	let mut score = material
		+ bench_score
		+ bag + presence
		+ margin
		+ contact
		+ geometry
		+ spawn_control
		+ mobility
		- 22 * at_risk;

	if game.rules().center_hold_win {
		let n = i32::from(game.rules().center_hold_n.max(1));
		let unit = 8_000 / n;
		score += unit * i32::from(game.center_hold(me)) - unit * i32::from(game.center_hold(opp));
	}

	if contact_pairs == 0 && my_board > 0 && opp_board > 0 {
		score -= 70 * (material + presence).signum();
	}

	if repeat_count >= 2 {
		let extra = i32::from(repeat_count - 1);
		score -= 30 * extra;
		if score > 0 {
			score -= 110 * extra;
		}
	}

	if score.abs() < WIN / 2 {
		let rem = game.rules().turn_limit.saturating_sub(turn);
		if rem <= 90 {
			score = (i64::from(score) * i64::from(rem) / 90) as i32;
		}
	}

	score
}

/// Material, mobility, center-hold / petal-majority progress, next-turn
/// center threats, overwhelm pressure on the center occupant, lockout-aware
/// mobility, and a fade toward 0 as the turn-limit approaches.
pub fn evaluate(game: &Game, me: Player, repeat_count: u16) -> i32 {
	if let Some(term) = terminal_score(game, me) {
		return term;
	}
	let opp = me.other();
	let mut material = 0i32;
	let mut at_risk = 0i32;
	let mut margin = 0i32;
	let mut contact = 0i32;
	let mut geometry = 0i32;
	let mut spawn_control = 0i32;
	let board = game.board();
	let r0 = board.construction_radius();
	let turn = game.turn_count();
	let my_board = game.board_count(me) as i32;
	let opp_board = game.board_count(opp) as i32;
	let center = board.center();

	let mut contact_pairs = 0i32;
	for (node, tile) in game.tiles() {
		let sign = if tile.owner == me { 1 } else { -1 };
		material += sign * (48 + 24 * i32::from(tile.weight) + 6 * i32::from(tile.dots));
		if let Some((matches, mismatches)) = game.tile_score(node) {
			margin += sign * 8 * (matches - mismatches);
			if mismatches >= matches - 1 {
				at_risk += sign;
			}
		}
		let mut enemy_nbs = 0i32;
		for &nb in board.neighbors(node) {
			if let Some(other) = game.tile_at(nb) {
				if other.owner != tile.owner {
					enemy_nbs += 1;
				}
			}
		}
		if tile.owner == me {
			contact_pairs += enemy_nbs;
		}
		contact += sign * 20 * enemy_nbs;

		let r = board.radius_of(node);
		let geo = if node == center {
			if game.rules().center_hold_win {
				420
			} else if game.rules().control_win {
				180
			} else {
				22
			}
		} else if board.inner_ring().contains(&node) {
			if game.rules().center_hold_win
				|| game.rules().petal_majority_win
				|| game.rules().control_win
			{
				55
			} else {
				16
			}
		} else if board.is_spawn(node) {
			if turn > 20 {
				-12
			} else {
				3
			}
		} else if r < r0 * 1.15 {
			16
		} else {
			9
		};
		geometry += sign * geo;
	}

	for &spawn in board.spawns() {
		match game.tile_at(spawn) {
			Some(tile) => {
				spawn_control += if tile.owner == me { 6 } else { -6 };
			}
			None => {
				let mut mine = 0i32;
				let mut theirs = 0i32;
				for &nb in board.neighbors(spawn) {
					if let Some(t) = game.tile_at(nb) {
						if t.owner == me {
							mine += 1;
						} else {
							theirs += 1;
						}
					}
				}
				spawn_control += 8 * (mine - theirs);
			}
		}
	}

	let mut bench_score = 0i32;
	for tile in game.bench(me) {
		bench_score += 28 + 10 * i32::from(tile.weight);
	}
	for tile in game.bench(opp) {
		bench_score -= 28 + 10 * i32::from(tile.weight);
	}
	let bag = 3 * (game.bag_len(me) as i32 - game.bag_len(opp) as i32);
	let budget = BUDGET_TOKEN * (game.budget(me) as i32 - game.budget(opp) as i32);
	let presence = 32 * (my_board - opp_board);
	let mobility = 4 * (mobility_of(game, me) - mobility_of(game, opp));

	let mut score = material
		+ bench_score
		+ bag + budget
		+ presence
		+ margin
		+ contact
		+ geometry
		+ spawn_control
		+ mobility
		- 22 * at_risk;

	score += race_score(game, me, opp);

	if contact_pairs == 0 && my_board > 0 && opp_board > 0 {
		score -= 70 * (material + presence).signum();
	}

	if repeat_count >= 2 {
		let extra = i32::from(repeat_count - 1);
		score -= 30 * extra;
		if score > 0 {
			score -= 110 * extra;
		}
	}

	if score.abs() < WIN / 2 {
		let rem = game.rules().turn_limit.saturating_sub(turn);
		if rem <= 90 {
			score = (i64::from(score) * i64::from(rem) / 90) as i32;
		}
	}

	score
}

fn race_score(game: &Game, me: Player, opp: Player) -> i32 {
	let mut s = 0i32;
	let center = game.board().center();
	let occupant = game.tile_at(center);
	let me_can = can_reach_center(game, me);
	let opp_can = can_reach_center(game, opp);

	if game.rules().center_hold_win {
		let n = i32::from(game.rules().center_hold_n.max(1));
		let my = i32::from(game.center_hold(me));
		let their = i32::from(game.center_hold(opp));
		s += (WIN / 3) * my / n;
		s -= (WIN / 2) * their / n;
		match occupant {
			Some(t) if t.owner == me => {
				s += 900;
				if let Some((matches, mismatches)) = game.tile_score(center) {
					s -= 180 * (mismatches - (matches - 1)).max(0);
				}
			}
			Some(t) if t.owner == opp => {
				s -= 1_200;
				if let Some((matches, mismatches)) = game.tile_score(center) {
					s += 280 * mismatches;
					if mismatches >= matches {
						s += 2_400;
					}
				}
			}
			_ => {
				if me_can {
					s += 700;
				}
				if opp_can {
					s -= 850;
				}
			}
		}
	}

	let my_petals = i32::from(game.petal_count(me));
	let opp_petals = i32::from(game.petal_count(opp));
	s += 90 * my_petals - 110 * opp_petals;
	if game.rules().petal_majority_win {
		let n = i32::from(game.rules().petal_majority_n.max(1));
		let need = i32::from(game.petal_majority_need());
		s += (WIN / 4) * i32::from(game.petal_majority(me)) / n;
		s -= (WIN / 3) * i32::from(game.petal_majority(opp)) / n;
		if my_petals >= need {
			s += 800;
		}
		if opp_petals >= need {
			s -= 1_000;
		}
	}

	if game.rules().control_win {
		let n = i32::from(game.rules().control_hold_turns.max(1));
		let need = i32::from(game.control_need());
		let my_ctl = i32::from(game.control_count(me));
		let opp_ctl = i32::from(game.control_count(opp));
		s += 240 * my_ctl - 280 * opp_ctl;
		s += (WIN / 4) * i32::from(game.control_hold(me)) / n;
		s -= (WIN / 3) * i32::from(game.control_hold(opp)) / n;
		if my_ctl >= need {
			s += 900;
		}
		if opp_ctl >= need {
			s -= 1_100;
		}
	}

	if game.rules().insufficient_tiles_loss.is_on() {
		let need = i32::from(game.control_need());
		let my_r = game.reachable_tiles(me) as i32;
		let opp_r = game.reachable_tiles(opp) as i32;
		s += 350 * (my_r.min(need) - opp_r.min(need));
		if my_r < need {
			s -= 2_400;
		}
		if opp_r < need {
			s += 2_800;
		}
	}

	s += low_value_score(game, me, opp);
	s
}

/// How many board tiles sit at or above the low-value cap (`< 5` qualifies).
fn tiles_over_low_value_cap(n: i32) -> i32 {
	(n - (i32::from(LOW_VALUE_BOARD_LT) - 1)).max(0)
}

/// Closeness of the low-value preconditions, `0..=LOW_VALUE_CLOSE_MAX`.
///
/// Budget closeness is 0 while either side still holds a full opening stack
/// (the bottleneck is the larger remaining budget) and 64 when both are 0.
/// Board closeness is 64 when both already have fewer than
/// [`LOW_VALUE_BOARD_LT`] tiles and fades as counts rise above that floor.
/// The product is 0 in the opening (full budgets, empty board) so this does
/// not fight the control-win race.
fn low_value_closeness(game: &Game, me: Player, opp: Player) -> i32 {
	let my_b = game.budget(me) as i32;
	let opp_b = game.budget(opp) as i32;
	let start = i32::from(game.rules().starting_bench_size.max(1));
	let bottleneck = my_b.max(opp_b);
	let budget_close = ((start - bottleneck.min(start)) * LOW_VALUE_CLOSE_MAX / start).max(0);
	let over_sum = tiles_over_low_value_cap(game.board_count(me) as i32)
		+ tiles_over_low_value_cap(game.board_count(opp) as i32);
	let board_close = LOW_VALUE_CLOSE_MAX * 4 / (4 + over_sum);
	budget_close * board_close / LOW_VALUE_CLOSE_MAX
}

/// Reward holding a lower lowest-value piece than the opponent, scaled up as
/// budgets approach 0 and board counts approach the `< 5` threshold. The
/// player who would win if the ending fired is nudged to spend leftover
/// budget; the player who would lose is nudged to keep a blocker token or
/// stay at 5+ tiles. No-op unless both sides have a board piece (elimination
/// already covers an empty board).
fn low_value_score(game: &Game, me: Player, opp: Player) -> i32 {
	let Some(my_key) = game.lowest_piece_keys(me) else {
		return 0;
	};
	let Some(opp_key) = game.lowest_piece_keys(opp) else {
		return 0;
	};
	let close = low_value_closeness(game, me, opp);
	let primary_delta = opp_key.0 - my_key.0;
	let tb_delta = opp_key.1 - my_key.1;
	// Always-on hedge: a cheaper lowest piece is a small plus even early.
	// Primary pips dominate; the zag-weighted tie-break is a nudge.
	let mut s = 18 * primary_delta + 4 * tb_delta;
	s += close * (32 * primary_delta + 6 * tb_delta);
	let my_b = game.budget(me) as i32;
	let opp_b = game.budget(opp) as i32;
	let my_n = game.board_count(me) as i32;
	let opp_n = game.board_count(opp) as i32;
	let over_sum = tiles_over_low_value_cap(my_n) + tiles_over_low_value_cap(opp_n);
	if my_key < opp_key {
		s += 8 * close;
		// Leftover budget (either side) delays a win we would take.
		s -= close * (my_b + opp_b) * 2;
		s -= close * my_b;
		s -= close * over_sum * 6;
	} else if opp_key < my_key {
		s -= 10 * close;
		if my_b > 0 {
			s += close * 16;
		}
		if my_n >= i32::from(LOW_VALUE_BOARD_LT) {
			s += close * 12;
		}
	}
	s
}

fn can_reach_center(game: &Game, player: Player) -> bool {
	let center = game.board().center();
	if game.tile_at(center).is_some() {
		return false;
	}
	if player != game.current_player() {
		// Approximate: an inner-ring tile with a vacant center is a 1-step
		// threat on the next turn if it is not same-piece-restricted.
		return game
			.board()
			.inner_ring()
			.iter()
			.any(|&n| game.tile_at(n).map(|t| t.owner == player).unwrap_or(false));
	}
	game.board().inner_ring().iter().any(|&n| {
		game.tile_at(n).map(|t| t.owner == player).unwrap_or(false)
			&& game.legal_destinations(n).contains(&center)
	})
}

/// Cheap mobility: empty adjacent nodes + empty spawns if the bench can ADD.
fn mobility_of(game: &Game, player: Player) -> i32 {
	let mut m = 0i32;
	for (node, tile) in game.tiles() {
		if tile.owner != player {
			continue;
		}
		for &nb in game.board().neighbors(node) {
			if game.tile_at(nb).is_none() {
				m += 1;
			}
		}
	}
	if game.budget(player) > 0 && !game.bench(player).is_empty() {
		m += game.board().spawns().iter().filter(|&&s| game.tile_at(s).is_none()).count() as i32;
	}
	m
}

pub fn eval_unit(game: &Game, me: Player, repeat_count: u16) -> f64 {
	if let Some(term) = terminal_score(game, me) {
		return f64::from(term) / f64::from(WIN);
	}
	(f64::from(evaluate(game, me, repeat_count)) / 400.0).tanh()
}

#[cfg(test)]
mod tests {
	#![allow(clippy::unwrap_used, clippy::expect_used)]

	use super::*;
	use crate::game::{BudgetToken, EndReason, Game, GameStatus, RuleSet, TileColor};

	#[test]
	fn turn_limit_draw_scores_zero() {
		let mut game = Game::empty(RuleSet::default());
		game.set_status(GameStatus::Draw { reason: EndReason::TurnLimit });
		assert_eq!(evaluate(&game, Player::One, 0), 0);
		assert_eq!(terminal_score(&game, Player::Two), Some(0));
	}

	#[test]
	fn elimination_win_is_near_mate() {
		let mut game = Game::empty(RuleSet::default());
		game.set_status(GameStatus::Won { winner: Player::One, reason: EndReason::Elimination });
		let s = evaluate(&game, Player::One, 0);
		assert!(s > WIN / 2);
		assert_eq!(evaluate(&game, Player::Two, 0), -s);
	}

	#[test]
	fn repeating_while_ahead_is_penalized() {
		let mut game = Game::empty(RuleSet::default());
		let spawn = game.board().spawns()[0];
		let tile = game.alloc_tile(Player::One, crate::game::TileColor::Green, 3, 1);
		game.place_tile(spawn, tile);
		let base = evaluate(&game, Player::One, 1);
		let penalized = evaluate(&game, Player::One, 3);
		assert!(penalized < base);
	}

	#[test]
	fn locked_out_is_a_loss_for_the_side_to_move() {
		let mut game = Game::empty(RuleSet::default());
		game.set_status(GameStatus::Won { winner: Player::Two, reason: EndReason::LockedOut });
		let s = evaluate(&game, Player::One, 0);
		assert!(s < -WIN / 2);
		assert_eq!(evaluate(&game, Player::Two, 0), -s);
	}

	#[test]
	fn no_moves_loses_empty_board_is_lockout_for_side_to_move() {
		let game = Game::empty(RuleSet::default());
		assert!(game.rules().no_moves_loses);
		assert!(!game.has_add_or_move());
		assert_eq!(terminal_score(&game, Player::One), Some(-WIN));
		assert_eq!(terminal_score(&game, Player::Two), Some(WIN));
	}

	#[test]
	fn center_hold_win_is_near_mate() {
		let mut game = Game::empty(RuleSet::default());
		game.set_status(GameStatus::Won { winner: Player::One, reason: EndReason::CenterHold });
		let s = evaluate(&game, Player::One, 0);
		assert!(s > WIN / 2);
		assert_eq!(evaluate(&game, Player::Two, 0), -s);
	}

	#[test]
	fn center_hold_win_keeps_winner_to_move() {
		use crate::game::{Action, TileColor};

		let mut rules = RuleSet::default();
		rules.center_hold_win = true;
		rules.center_hold_n = 1;
		rules.no_same_piece_twice = crate::game::SamePieceTwice::Off;
		let mut game = Game::empty(rules);
		let center = game.board().center();
		let from = game.board().neighbors(center)[0];
		let mover = game.alloc_tile(Player::One, TileColor::Blue, 3, 1);
		game.place_tile(from, mover);
		let opp = game.alloc_tile(Player::Two, TileColor::Orange, 3, 1);
		game.set_bench(Player::Two, vec![opp]);
		game.set_current(Player::One);
		game.apply(Action::Move { from, to: center }).unwrap();
		assert_eq!(
			game.status(),
			GameStatus::Won { winner: Player::One, reason: EndReason::CenterHold }
		);
		assert_eq!(game.current_player(), Player::One);
		let from_mover = terminal_score(&game, Player::One).expect("terminal");
		assert!(from_mover > WIN / 2);
		// Negating a STM terminal would flip the win into a loss — the minimax
		// bug that made it vacate the center instead of completing the hold.
		assert!(-terminal_score(&game, game.current_player()).unwrap() < -WIN / 2);
	}

	#[test]
	fn occupying_center_scores_higher_when_center_hold_is_on() {
		let mut off_rules = RuleSet::default();
		off_rules.center_hold_win = false;
		off_rules.control_win = false;
		let mut on_rules = off_rules;
		on_rules.center_hold_win = true;
		let mut off = Game::empty(off_rules);
		let mut on = Game::empty(on_rules);
		assert!(on.rules().center_hold_win);
		assert!(!off.rules().center_hold_win);
		let center = off.board().center();
		let tile = off.alloc_tile(Player::One, crate::game::TileColor::Blue, 3, 1);
		off.place_tile(center, tile);
		on.place_tile(center, tile);
		assert!(evaluate(&on, Player::One, 0) > evaluate(&off, Player::One, 0));
	}

	#[test]
	fn opponent_near_center_hold_is_urgent() {
		let mut rules = RuleSet::default();
		rules.center_hold_win = true;
		rules.control_win = false;
		let mut quiet = Game::empty(rules);
		let mut threat = Game::empty(rules);
		let center = quiet.board().center();
		let spawn = quiet.board().spawns()[0];
		let tile = quiet.alloc_tile(Player::Two, crate::game::TileColor::Blue, 3, 1);
		let p1 = quiet.alloc_tile(Player::One, crate::game::TileColor::Green, 3, 1);
		quiet.place_tile(center, tile);
		quiet.place_tile(spawn, p1);
		threat.place_tile(center, tile);
		threat.place_tile(spawn, p1);
		threat.set_center_hold(Player::Two, 4);
		assert!(evaluate(&threat, Player::One, 0) < evaluate(&quiet, Player::One, 0) - 5_000);
	}

	#[test]
	fn petal_majority_win_is_near_mate() {
		let mut game = Game::empty(RuleSet::default());
		game.set_status(GameStatus::Won { winner: Player::One, reason: EndReason::PetalMajority });
		let s = evaluate(&game, Player::One, 0);
		assert!(s > WIN / 2);
		assert_eq!(evaluate(&game, Player::Two, 0), -s);
	}

	#[test]
	fn control_win_is_near_mate() {
		let mut game = Game::empty(RuleSet::default());
		game.set_status(GameStatus::Won { winner: Player::One, reason: EndReason::Control });
		let s = evaluate(&game, Player::One, 0);
		assert!(s > WIN / 2);
		assert_eq!(evaluate(&game, Player::Two, 0), -s);
	}

	#[test]
	fn insufficient_tiles_loss_is_near_mate() {
		let mut game = Game::empty(RuleSet::default());
		game.set_status(GameStatus::Won {
			winner: Player::Two,
			reason: EndReason::InsufficientTiles,
		});
		let s = evaluate(&game, Player::One, 0);
		assert!(s < -WIN / 2);
		assert_eq!(evaluate(&game, Player::Two, 0), -s);
	}

	#[test]
	fn control_occupancy_is_scored_when_control_win_is_on() {
		let mut off_rules = RuleSet::default();
		off_rules.center_hold_win = false;
		off_rules.control_win = false;
		let mut on_rules = off_rules;
		on_rules.control_win = true;
		let mut off = Game::empty(off_rules);
		let mut on = Game::empty(on_rules);
		let inner = off.board().inner_ring().to_vec();
		for &node in inner.iter().take(4) {
			let tile = off.alloc_tile(Player::One, crate::game::TileColor::Green, 3, 1);
			off.place_tile(node, tile);
			on.place_tile(node, tile);
		}
		assert!(evaluate(&on, Player::One, 0) > evaluate(&off, Player::One, 0));
	}

	fn quiet_rules() -> RuleSet {
		let mut rules = RuleSet::default();
		rules.control_win = false;
		rules.center_hold_win = false;
		rules.petal_majority_win = false;
		rules
	}

	fn place_isolated(game: &mut Game, spawn_idx: usize, player: Player, zag: bool, dots: u8) {
		let spawn = game.board().spawns()[spawn_idx];
		let tile = if zag {
			game.alloc_ring_kinds(player, &[(TileColor::Green, true)], 1, dots)
		} else {
			game.alloc_tile(player, TileColor::Green, 1, dots)
		};
		game.place_tile(spawn, tile);
	}

	fn set_equal_budget(game: &mut Game, n: usize) {
		game.set_budget_tokens(Player::One, vec![BudgetToken::Blank; n]);
		game.set_budget_tokens(Player::Two, vec![BudgetToken::Blank; n]);
	}

	/// P1 spawn 0, P2 spawn 4, optional extra high-value tiles on later spawns.
	fn low_value_pos(p1_dots: u8, p1_zag: bool, p2_dots: u8, p2_zag: bool, budget: usize) -> Game {
		let mut game = Game::empty(quiet_rules());
		place_isolated(&mut game, 0, Player::One, p1_zag, p1_dots);
		place_isolated(&mut game, 4, Player::Two, p2_zag, p2_dots);
		set_equal_budget(&mut game, budget);
		game
	}

	fn add_high_extras(game: &mut Game, each: usize) {
		let spawns = game.board().spawns().to_vec();
		// Remaining spawns 1,2,3 for P1 and 5,6,7 for P2 — not adjacent.
		let p1_slots = [1usize, 2, 3];
		let p2_slots = [5usize, 6, 7];
		for i in 0..each {
			let t1 = game.alloc_rings(
				Player::One,
				&[TileColor::Blue, TileColor::Orange, TileColor::Green],
				3,
				3,
			);
			game.place_tile(spawns[p1_slots[i]], t1);
			let t2 = game.alloc_rings(
				Player::Two,
				&[TileColor::Blue, TileColor::Orange, TileColor::Green],
				3,
				3,
			);
			game.place_tile(spawns[p2_slots[i]], t2);
		}
	}

	#[test]
	fn low_value_win_is_near_mate() {
		let mut game = Game::empty(RuleSet::default());
		game.set_status(GameStatus::Won { winner: Player::One, reason: EndReason::LowValue });
		let s = evaluate(&game, Player::One, 0);
		assert!(s > WIN / 2);
		assert_eq!(evaluate(&game, Player::Two, 0), -s);
	}

	#[test]
	fn lower_lowest_piece_scores_higher_when_budgets_are_gone() {
		let cheap = low_value_pos(1, false, 3, false, 0);
		let equal = low_value_pos(3, false, 3, false, 0);
		let cur = evaluate(&cheap, Player::One, 0) - evaluate(&equal, Player::One, 0);
		let old = evaluate_legacy(&cheap, Player::One, 0) - evaluate_legacy(&equal, Player::One, 0);
		assert!(cur > 1_000, "cur={cur}");
		assert!(cur > old + 1_000, "cur={cur} old={old}");
	}

	#[test]
	fn low_value_piece_gap_grows_as_budgets_fall() {
		let cheap_full = low_value_pos(1, false, 3, false, 7);
		let equal_full = low_value_pos(3, false, 3, false, 7);
		let cheap_empty = low_value_pos(1, false, 3, false, 0);
		let equal_empty = low_value_pos(3, false, 3, false, 0);
		let gap_full =
			evaluate(&cheap_full, Player::One, 0) - evaluate(&equal_full, Player::One, 0);
		let gap_empty =
			evaluate(&cheap_empty, Player::One, 0) - evaluate(&equal_empty, Player::One, 0);
		assert!(gap_empty > gap_full + 1_000, "full={gap_full} empty={gap_empty}");
		let legacy_full = evaluate_legacy(&cheap_full, Player::One, 0)
			- evaluate_legacy(&equal_full, Player::One, 0);
		let legacy_empty = evaluate_legacy(&cheap_empty, Player::One, 0)
			- evaluate_legacy(&equal_empty, Player::One, 0);
		assert_eq!(legacy_full, legacy_empty);
	}

	#[test]
	fn low_value_piece_gap_grows_as_boards_approach_threshold() {
		let sparse_cheap = low_value_pos(1, false, 3, false, 0);
		let sparse_equal = low_value_pos(3, false, 3, false, 0);
		let mut crowded_cheap = low_value_pos(1, false, 3, false, 0);
		let mut crowded_equal = low_value_pos(3, false, 3, false, 0);
		add_high_extras(&mut crowded_cheap, 3);
		add_high_extras(&mut crowded_equal, 3);
		assert_eq!(sparse_cheap.board_count(Player::One), 1);
		assert_eq!(crowded_cheap.board_count(Player::One), 4);
		// 4 is still under the cap. Park extra tiles on the inner ring so both
		// sides go to 6 (over the <5 threshold) without changing lowest keys.
		let inner = crowded_cheap.board().inner_ring().to_vec();
		for &node in inner.iter().take(2) {
			let t1 = crowded_cheap.alloc_tile(Player::One, TileColor::Blue, 3, 3);
			crowded_cheap.place_tile(node, t1);
			let t2 = crowded_equal.alloc_tile(Player::One, TileColor::Blue, 3, 3);
			crowded_equal.place_tile(node, t2);
		}
		for &node in inner.iter().rev().take(2) {
			let t1 = crowded_cheap.alloc_tile(Player::Two, TileColor::Orange, 3, 3);
			crowded_cheap.place_tile(node, t1);
			let t2 = crowded_equal.alloc_tile(Player::Two, TileColor::Orange, 3, 3);
			crowded_equal.place_tile(node, t2);
		}
		assert!(crowded_cheap.board_count(Player::One) >= usize::from(LOW_VALUE_BOARD_LT));
		assert!(crowded_cheap.board_count(Player::Two) >= usize::from(LOW_VALUE_BOARD_LT));
		let gap_sparse =
			evaluate(&sparse_cheap, Player::One, 0) - evaluate(&sparse_equal, Player::One, 0);
		let gap_crowded =
			evaluate(&crowded_cheap, Player::One, 0) - evaluate(&crowded_equal, Player::One, 0);
		assert!(gap_sparse > gap_crowded, "sparse={gap_sparse} crowded={gap_crowded}");
	}

	#[test]
	fn zag_tiebreak_prefers_the_normal_lowest_piece() {
		let zag_vs_normal = low_value_pos(1, true, 1, false, 0);
		let both_normal = low_value_pos(1, false, 1, false, 0);
		assert!(evaluate(&zag_vs_normal, Player::One, 0) < evaluate(&both_normal, Player::One, 0));
		assert_eq!(
			evaluate_legacy(&zag_vs_normal, Player::One, 0),
			evaluate_legacy(&both_normal, Player::One, 0)
		);
	}

	#[test]
	fn budget_token_is_worth_less_than_a_surviving_tile() {
		let mut fewer = low_value_pos(2, false, 2, false, 6);
		let mut more = low_value_pos(2, false, 2, false, 7);
		// One extra P1 token vs one extra P1 board tile (isolated spawn).
		fewer.set_budget_tokens(Player::One, vec![BudgetToken::Blank; 6]);
		more.set_budget_tokens(Player::One, vec![BudgetToken::Blank; 7]);
		fewer.set_budget_tokens(Player::Two, vec![BudgetToken::Blank; 6]);
		more.set_budget_tokens(Player::Two, vec![BudgetToken::Blank; 6]);
		let token = evaluate(&more, Player::One, 0) - evaluate(&fewer, Player::One, 0);
		let spawn = fewer.board().spawns()[1];
		let tile = fewer.alloc_tile(Player::One, TileColor::Blue, 2, 2);
		fewer.place_tile(spawn, tile);
		let board_tile = evaluate(&fewer, Player::One, 0) - evaluate(&more, Player::One, 0) + token;
		assert!(token > 0 && token < 40, "token={token}");
		assert!(board_tile > token, "board={board_tile} token={token}");
	}
}
