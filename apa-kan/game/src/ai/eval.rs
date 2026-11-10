//! Heuristic evaluation. Positive is good for `me`.

use crate::game::{Game, GameStatus, Player};

pub const WIN: i32 = 50_000;

pub fn terminal_score(game: &Game, me: Player) -> Option<i32> {
	match game.status() {
		GameStatus::InProgress => None,
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

/// Material (board + bench), tiles at risk, safe-match margin, mobility,
/// spawn control, contact pressure, a repetition penalty, and fade-to-0
/// as the turn-limit (scored 0) approaches.
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
			22
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
	if !game.bench(player).is_empty() {
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
		let base = evaluate(&game, Player::One, 1);
		let penalized = evaluate(&game, Player::One, 3);
		assert!(penalized < base);
	}
}
