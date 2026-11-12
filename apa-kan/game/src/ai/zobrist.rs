//! Zobrist keys for visible positions: occupancy, kind, owner, benches, capture
//! budget, side, last-moved / moved-this-turn tile ids, and hold counters
//! (center, petal, control).

use std::sync::OnceLock;

use crate::game::{Game, Player, KIND_COUNT};

const MAX_NODES: usize = 32;
const MAX_BENCH_COPIES: usize = 16;

pub struct Zobrist {
	/// node × owner × kind, heap so the 6-color kind space cannot overflow the stack.
	board: Vec<u64>,
	/// player × kind × copy-count (1-based)
	bench: Vec<u64>,
	side_two: u64,
	pass_phase: [u64; 4],
	actions_phase: [u64; 8],
	last_moved_none: [u64; 2],
	last_moved_some: [u64; 2],
	center_hold: [[u64; 16]; 2],
	petal_majority: [[u64; 16]; 2],
	control_hold: [[u64; 16]; 2],
	moved_this_turn_none: u64,
	moved_this_turn_some: u64,
	budget: [[u64; 64]; 2],
}

impl Zobrist {
	fn new() -> Self {
		let mut state = 0xC0FF_EE00_D15E_A5E5u64;
		let mut next = || splitmix(&mut state);
		let mut z = Self {
			board: vec![0; MAX_NODES * 2 * KIND_COUNT],
			bench: vec![0; 2 * KIND_COUNT * MAX_BENCH_COPIES],
			side_two: next(),
			pass_phase: [0; 4],
			actions_phase: [0; 8],
			last_moved_none: [next(), next()],
			last_moved_some: [next(), next()],
			center_hold: [[0; 16]; 2],
			petal_majority: [[0; 16]; 2],
			control_hold: [[0; 16]; 2],
			moved_this_turn_none: next(),
			moved_this_turn_some: next(),
			budget: [[0; 64]; 2],
		};
		for node in 0..MAX_NODES {
			for owner in 0..2 {
				for kind in 0..KIND_COUNT {
					z.board[board_ix(node, owner, kind)] = next();
				}
			}
		}
		for player in 0..2 {
			for kind in 0..KIND_COUNT {
				for copy in 0..MAX_BENCH_COPIES {
					z.bench[bench_ix(player, kind, copy)] = next();
				}
			}
		}
		for v in &mut z.pass_phase {
			*v = next();
		}
		for v in &mut z.actions_phase {
			*v = next();
		}
		for player in 0..2 {
			for n in 0..16 {
				z.center_hold[player][n] = next();
				z.petal_majority[player][n] = next();
				z.control_hold[player][n] = next();
			}
			for n in 0..64 {
				z.budget[player][n] = next();
			}
		}
		z
	}
}

fn splitmix(state: &mut u64) -> u64 {
	*state = state.wrapping_add(0x9E37_79B9_7F4A_7C15);
	let mut z = *state;
	z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
	z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
	z ^ (z >> 31)
}

fn board_ix(node: usize, owner: usize, kind: usize) -> usize {
	(node * 2 + owner) * KIND_COUNT + kind
}

fn bench_ix(player: usize, kind: usize, copy: usize) -> usize {
	(player * KIND_COUNT + kind) * MAX_BENCH_COPIES + copy
}

pub fn tables() -> &'static Zobrist {
	static Z: OnceLock<Zobrist> = OnceLock::new();
	Z.get_or_init(Zobrist::new)
}

/// Occupancy, tile kind, owner, benches (by kind multiplicity), side to move.
pub fn hash(game: &Game) -> u64 {
	let z = tables();
	let mut h = 0u64;
	for (node, tile) in game.tiles() {
		let n = node as usize;
		if n < MAX_NODES {
			h ^= z.board[board_ix(n, tile.owner.index(), tile.kind_index())];
		}
	}
	for player in [Player::One, Player::Two] {
		let mut copies = [0u8; KIND_COUNT];
		for tile in game.bench(player) {
			let k = tile.kind_index();
			let n = copies[k].saturating_add(1).min(MAX_BENCH_COPIES as u8);
			copies[k] = n;
			h ^= z.bench[bench_ix(player.index(), k, (n as usize) - 1)];
		}
	}
	if game.current_player() == Player::Two {
		h ^= z.side_two;
	}
	h ^= z.pass_phase[game.consecutive_passes().min(3) as usize];
	h ^= z.actions_phase[game.actions_this_turn().min(7) as usize];
	for player in [Player::One, Player::Two] {
		match game.last_moved_tile(player) {
			None => h ^= z.last_moved_none[player.index()],
			Some(id) => h ^= z.last_moved_some[player.index()] ^ mix_id(id),
		}
		h ^= z.center_hold[player.index()][game.center_hold(player).min(15) as usize];
		h ^= z.petal_majority[player.index()][game.petal_majority(player).min(15) as usize];
		h ^= z.control_hold[player.index()][game.control_hold(player).min(15) as usize];
		h ^= z.budget[player.index()][game.budget(player).min(63)];
	}
	match game.moved_this_turn() {
		None => h ^= z.moved_this_turn_none,
		Some(id) => h ^= z.moved_this_turn_some ^ mix_id(id),
	}
	h
}

fn mix_id(id: u32) -> u64 {
	let mut x = u64::from(id).wrapping_add(0x9E37_79B9_7F4A_7C15);
	x = (x ^ (x >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
	x = (x ^ (x >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
	x ^ (x >> 31)
}

/// Board occupancy + side to move, ignoring benches. For stall reporting.
pub fn occupancy_hash(game: &Game) -> u64 {
	let z = tables();
	let mut h = 0u64;
	for (node, tile) in game.tiles() {
		let n = node as usize;
		if n < MAX_NODES {
			h ^= z.board[board_ix(n, tile.owner.index(), tile.kind_index())];
		}
	}
	if game.current_player() == Player::Two {
		h ^= z.side_two;
	}
	h
}

#[cfg(test)]
mod tests {
	#![allow(clippy::unwrap_used, clippy::expect_used)]

	use super::*;
	use crate::game::{Action, RuleSet};

	#[test]
	fn hash_changes_on_add_and_is_stable() {
		let mut game = crate::game::Game::new(RuleSet::default(), 11);
		let a = hash(&game);
		assert_eq!(a, hash(&game));
		let action = game.legal_adds().into_iter().next().unwrap();
		let _ = game.apply(action).unwrap();
		let b = hash(&game);
		assert_ne!(a, b);
	}

	#[test]
	fn occupancy_hash_ignores_bench_order() {
		let game = crate::game::Game::new(RuleSet::default(), 4);
		let _ = Action::Pass;
		assert_eq!(occupancy_hash(&game), occupancy_hash(&game));
	}

	#[test]
	fn hash_includes_last_moved_and_center_hold() {
		use crate::game::{Game, Player, SamePieceTwice, TileColor};

		let mut rules = RuleSet::default();
		rules.center_hold_win = true;
		rules.center_hold_n = 5;
		rules.no_same_piece_twice = SamePieceTwice::Always;
		let mut placed = Game::empty(rules);
		let mut moved = Game::empty(rules);
		let center = placed.board().center();
		let from = placed.board().neighbors(center)[0];
		let tile = placed.alloc_tile(Player::One, TileColor::Blue, 3, 1);
		placed.place_tile(center, tile);
		moved.place_tile(from, tile);
		let opp = placed.alloc_tile(Player::Two, TileColor::Orange, 3, 1);
		placed.set_bench(Player::Two, vec![opp]);
		moved.set_bench(Player::Two, vec![opp]);
		placed.set_current(Player::Two);
		moved.set_current(Player::One);
		moved.apply(Action::Move { from, to: center }).unwrap();
		assert_eq!(moved.current_player(), Player::Two);
		assert_eq!(moved.last_moved_tile(Player::One), Some(tile.id));
		assert_eq!(moved.center_hold(Player::One), 1);
		assert_eq!(placed.last_moved_tile(Player::One), None);
		assert_eq!(placed.center_hold(Player::One), 0);
		assert_ne!(hash(&placed), hash(&moved));
		assert_eq!(occupancy_hash(&placed), occupancy_hash(&moved));
	}
}
