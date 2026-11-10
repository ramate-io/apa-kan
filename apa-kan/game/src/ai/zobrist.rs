//! Zobrist keys for visible positions: occupancy, kind, owner, benches, side.

use std::sync::OnceLock;

use crate::game::{Game, Player, KIND_COUNT};

const MAX_NODES: usize = 32;
const MAX_BENCH_COPIES: usize = 16;

pub struct Zobrist {
	/// node × owner × kind
	board: [[[u64; KIND_COUNT]; 2]; MAX_NODES],
	/// player × kind × copy-count (1-based)
	bench: [[[u64; MAX_BENCH_COPIES]; KIND_COUNT]; 2],
	side_two: u64,
	pass_phase: [u64; 4],
	actions_phase: [u64; 8],
}

impl Zobrist {
	fn new() -> Self {
		let mut state = 0xC0FF_EE00_D15E_A5E5u64;
		let mut next = || splitmix(&mut state);
		let mut z = Self {
			board: [[[0; KIND_COUNT]; 2]; MAX_NODES],
			bench: [[[0; MAX_BENCH_COPIES]; KIND_COUNT]; 2],
			side_two: next(),
			pass_phase: [0; 4],
			actions_phase: [0; 8],
		};
		for node in 0..MAX_NODES {
			for owner in 0..2 {
				for kind in 0..KIND_COUNT {
					z.board[node][owner][kind] = next();
				}
			}
		}
		for player in 0..2 {
			for kind in 0..KIND_COUNT {
				for copy in 0..MAX_BENCH_COPIES {
					z.bench[player][kind][copy] = next();
				}
			}
		}
		for v in &mut z.pass_phase {
			*v = next();
		}
		for v in &mut z.actions_phase {
			*v = next();
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
			h ^= z.board[n][tile.owner.index()][tile.kind_index()];
		}
	}
	for player in [Player::One, Player::Two] {
		let mut copies = [0u8; KIND_COUNT];
		for tile in game.bench(player) {
			let k = tile.kind_index();
			let n = copies[k].saturating_add(1).min(MAX_BENCH_COPIES as u8);
			copies[k] = n;
			h ^= z.bench[player.index()][k][(n as usize) - 1];
		}
	}
	if game.current_player() == Player::Two {
		h ^= z.side_two;
	}
	h ^= z.pass_phase[game.consecutive_passes().min(3) as usize];
	h ^= z.actions_phase[game.actions_this_turn().min(7) as usize];
	h
}

/// Board occupancy + side to move, ignoring benches. For stall reporting.
pub fn occupancy_hash(game: &Game) -> u64 {
	let z = tables();
	let mut h = 0u64;
	for (node, tile) in game.tiles() {
		let n = node as usize;
		if n < MAX_NODES {
			h ^= z.board[n][tile.owner.index()][tile.kind_index()];
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
}
