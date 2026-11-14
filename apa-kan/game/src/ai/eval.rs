//! Heuristic evaluation. Positive is good for `me`.

use crate::game::{Game, GameStatus, LowValueResolution, Player};

pub const WIN: i32 = 50_000;

pub fn terminal_score(game: &Game, me: Player) -> Option<i32> {
	match game.status() {
		GameStatus::InProgress => {
			if game.rules().no_moves_loses && !game.has_add_or_move() {
				Some(lockout_terminal(game, me))
			} else if game.low_value_applies() {
				// `finish_turn` already records this; search also sees constructed
				// leaves where both sides' possible-on-board counts are below 5.
				Some(low_value_terminal(game, me))
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
		// Turn-limit and low-value (equal-count full tie) draws are 0 so a
		// side that is ahead will not turtle into the 200-turn cap.
		GameStatus::Draw { .. } => Some(0),
	}
}

fn lockout_terminal(game: &Game, me: Player) -> i32 {
	let ply = game.turn_count() as i32;
	if game.current_player() == me {
		-WIN + ply
	} else {
		WIN - ply
	}
}

fn low_value_terminal(game: &Game, me: Player) -> i32 {
	let ply = game.turn_count() as i32;
	match game.low_value_resolution() {
		LowValueResolution::Draw => 0,
		LowValueResolution::Win { winner, .. } => {
			if winner == me {
				WIN - ply
			} else {
				-WIN + ply
			}
		}
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
	let presence = 32 * (my_board - opp_board);

	let mobility = 3 * (mobility_of(game, me) - mobility_of(game, opp));

	let mut score =
		material + bench_score + presence + margin + contact + geometry + spawn_control + mobility
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
	// Usable budget only (min(tokens, remaining deck)); surplus tokens are not ADD permission.
	let budget = 90 * (game.budget(me) as i32 - game.budget(opp) as i32);
	let presence = 32 * (my_board - opp_board);
	let mobility = 4 * (mobility_of(game, me) - mobility_of(game, opp));

	let mut score =
		material
			+ bench_score
			+ budget + presence
			+ margin + contact
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
	use crate::game::{EndReason, Game, GameStatus, RuleSet};

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
		game.pad_above_low_value();
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
		off.pad_above_low_value();
		on.pad_above_low_value();
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
		quiet.pad_above_low_value();
		threat.pad_above_low_value();
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
	fn low_value_win_is_near_mate() {
		let mut game = Game::empty(RuleSet::default());
		game.set_status(GameStatus::Won { winner: Player::One, reason: EndReason::LowValue });
		let s = evaluate(&game, Player::One, 0);
		assert!(s > WIN / 2);
		assert_eq!(evaluate(&game, Player::Two, 0), -s);
		assert_eq!(terminal_score(&game, Player::One), Some(WIN));
	}

	#[test]
	fn low_value_draw_scores_zero() {
		let mut game = Game::empty(RuleSet::default());
		game.set_status(GameStatus::Draw { reason: EndReason::LowValue });
		assert_eq!(evaluate(&game, Player::One, 0), 0);
		assert_eq!(terminal_score(&game, Player::Two), Some(0));
	}

	#[test]
	fn pending_low_value_is_a_search_terminal() {
		use crate::game::{BudgetToken, TileColor};

		let mut rules = RuleSet::default();
		rules.control_win = false;
		rules.center_hold_win = false;
		rules.petal_majority_win = false;
		let mut game = Game::empty(rules);
		let p1_node = game.board().spawns()[0];
		let p2_node = game.board().spawns()[4];
		let p1 = game.alloc_tile(Player::One, TileColor::Green, 1, 1);
		let p2 = game.alloc_tile(Player::Two, TileColor::Orange, 1, 2);
		game.place_tile(p1_node, p1);
		game.place_tile(p2_node, p2);
		game.set_bench(Player::One, Vec::new());
		game.set_budget_tokens(Player::One, vec![BudgetToken::Blank; 5]);
		let p2_deck = vec![
			game.alloc_tile(Player::Two, TileColor::Green, 1, 1),
			game.alloc_tile(Player::Two, TileColor::Blue, 1, 1),
			game.alloc_tile(Player::Two, TileColor::Orange, 1, 1),
		];
		game.set_bench(Player::Two, p2_deck);
		assert_eq!(game.status(), GameStatus::InProgress);
		assert!(game.low_value_applies());
		let s = evaluate(&game, Player::One, 0);
		assert!(s > WIN / 2, "P1's lower on-board piece should be a near-mate, got {s}");
		assert_eq!(evaluate(&game, Player::Two, 0), -s);
		assert_eq!(terminal_score(&game, Player::One), Some(WIN));
	}

	#[test]
	fn low_value_pair_and_piece_count_terminals_match_status() {
		use crate::game::TileColor;

		let mut rules = RuleSet::default();
		rules.control_win = false;
		rules.center_hold_win = false;
		rules.petal_majority_win = false;

		let place = |game: &mut Game, spawn_idx: usize, player: Player, dots: u8| {
			let spawn = game.board().spawns()[spawn_idx];
			let tile = game.alloc_tile(player, TileColor::Green, 1, dots);
			game.place_tile(spawn, tile);
		};

		let mut pair = Game::empty(rules);
		place(&mut pair, 0, Player::One, 1);
		place(&mut pair, 1, Player::One, 2);
		place(&mut pair, 4, Player::Two, 1);
		place(&mut pair, 5, Player::Two, 3);
		pair.set_budget_tokens(Player::One, Vec::new());
		pair.set_budget_tokens(Player::Two, Vec::new());
		pair.evaluate_low_value_for_test();
		assert_eq!(
			pair.status(),
			GameStatus::Won { winner: Player::One, reason: EndReason::LowValue }
		);
		let pair_p1 = terminal_score(&pair, Player::One).unwrap();
		assert!(pair_p1 > WIN / 2);
		assert_eq!(terminal_score(&pair, Player::Two), Some(-pair_p1));

		let mut count = Game::empty(rules);
		place(&mut count, 0, Player::One, 1);
		place(&mut count, 1, Player::One, 3);
		place(&mut count, 4, Player::Two, 1);
		count.set_budget_tokens(Player::One, Vec::new());
		count.set_budget_tokens(Player::Two, Vec::new());
		count.evaluate_low_value_for_test();
		assert_eq!(
			count.status(),
			GameStatus::Won { winner: Player::One, reason: EndReason::LowValue }
		);
		let count_p1 = terminal_score(&count, Player::One).unwrap();
		assert!(count_p1 > WIN / 2);
		assert_eq!(terminal_score(&count, Player::Two), Some(-count_p1));
	}

	#[test]
	fn evaluate_ignores_surplus_tokens_beyond_the_bench() {
		use crate::game::{BudgetToken, TileColor};

		let mut rules = RuleSet::default();
		rules.control_win = false;
		rules.center_hold_win = false;
		let mut surplus = Game::empty(rules);
		let mut none = Game::empty(rules);
		let spawn = surplus.board().spawns()[0];
		let other = surplus.board().spawns()[4];
		let p1 = surplus.alloc_tile(Player::One, TileColor::Green, 3, 1);
		let p2 = surplus.alloc_tile(Player::Two, TileColor::Orange, 3, 1);
		surplus.place_tile(spawn, p1);
		surplus.place_tile(other, p2);
		none.place_tile(spawn, p1);
		none.place_tile(other, p2);
		surplus.set_budget_tokens(Player::One, vec![BudgetToken::Blank; 5]);
		surplus.set_budget_tokens(Player::Two, vec![BudgetToken::Blank; 5]);
		none.set_budget_tokens(Player::One, Vec::new());
		none.set_budget_tokens(Player::Two, Vec::new());
		assert_eq!(surplus.budget(Player::One), 0);
		assert_eq!(evaluate(&surplus, Player::One, 0), evaluate(&none, Player::One, 0));
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
		off.pad_above_low_value();
		on.pad_above_low_value();
		assert!(evaluate(&on, Player::One, 0) > evaluate(&off, Player::One, 0));
	}
}
