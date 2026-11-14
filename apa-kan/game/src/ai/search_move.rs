//! Search-facing actions. ADD is keyed by kind so transpositions are stable.

use crate::board::NodeId;
use crate::game::{Action, Game, KIND_DOTS};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SearchMove {
	/// Low 16 bits: [`crate::game::Tile::kind_index`] (color-state × dots).
	/// High bits: [`crate::game::CaptureMark::bit`] so dot vs triangle ADDs of
	/// the same colors/types stay distinct in the TT.
	Add {
		kind: u32,
		node: NodeId,
	},
	Move {
		from: NodeId,
		to: NodeId,
	},
	Pass,
}

impl SearchMove {
	pub fn from_action(game: &Game, action: Action) -> Self {
		match action {
			Action::Add { bench_index, node } => {
				let kind = game
					.bench(game.current_player())
					.get(bench_index)
					.map(|t| search_kind(*t))
					.unwrap_or(0);
				SearchMove::Add { kind, node }
			}
			Action::Move { from, to } => SearchMove::Move { from, to },
			Action::Pass => SearchMove::Pass,
		}
	}

	pub fn to_action(self, game: &Game) -> Option<Action> {
		match self {
			SearchMove::Add { kind, node } => {
				let idx = game
					.bench(game.current_player())
					.iter()
					.position(|t| search_kind(*t) == kind)?;
				Some(Action::Add { bench_index: idx, node })
			}
			SearchMove::Move { from, to } => Some(Action::Move { from, to }),
			SearchMove::Pass => Some(Action::Pass),
		}
	}

	/// Packed TT key: 0 = MOVE, u16::MAX = PASS, 1..=COLOR_STATES = ADD color-state.
	/// `b` is `dots | (capture_bit << 2)`.
	pub fn pack(self) -> (u16, u8, u8) {
		match self {
			SearchMove::Add { kind, node } => {
				let dots_n = KIND_DOTS as u32;
				let base = kind & 0xFFFF;
				let capture = ((kind >> 16) as u8) & 1;
				let state = (base / dots_n + 1) as u16;
				let dots = (base % dots_n) as u8;
				(state, node, dots | (capture << 2))
			}
			SearchMove::Move { from, to } => (0, from, to),
			SearchMove::Pass => (u16::MAX, 0, 0),
		}
	}

	pub fn unpack(kind: u16, a: u8, b: u8) -> Option<Self> {
		match kind {
			u16::MAX => Some(SearchMove::Pass),
			0 => Some(SearchMove::Move { from: a, to: b }),
			state
				if (1..=crate::game::COLOR_STATES as u16).contains(&state)
					&& (b & 3) < KIND_DOTS as u8
					&& (b >> 2) <= 1 =>
			{
				let dots = b & 3;
				let capture = b >> 2;
				Some(SearchMove::Add {
					kind: u32::from(state - 1) * KIND_DOTS as u32
						+ u32::from(dots) + (u32::from(capture) << 16),
					node: a,
				})
			}
			_ => None,
		}
	}
}

pub fn search_kind(tile: crate::game::Tile) -> u32 {
	(tile.kind_index() as u32) | (u32::from(tile.capture.bit()) << 16)
}

fn kind_index_of(kind: u32) -> usize {
	(kind & 0xFFFF) as usize
}

fn capture_bit_of(kind: u32) -> u8 {
	((kind >> 16) as u8) & 1
}

/// Legal actions with duplicate ADD kinds collapsed (one index per kind).
pub fn legal_search_moves(game: &Game) -> Vec<SearchMove> {
	if game.status().is_over() {
		return Vec::new();
	}
	let mut out = Vec::new();
	let current = game.current_player();
	let bench = game.bench(current);
	// `budget()` is usable ADD permission (min(tokens, remaining deck)).
	if game.budget(current) > 0 && !bench.is_empty() {
		let mut seen = std::collections::HashSet::<u32>::new();
		let dests: Vec<NodeId> = if game.rules().add_only_on_spawns {
			game.board()
				.spawns()
				.iter()
				.copied()
				.filter(|&s| game.tile_at(s).is_none())
				.collect()
		} else {
			(0..game.board().node_count() as NodeId)
				.filter(|&id| game.tile_at(id).is_none())
				.collect()
		};
		for tile in bench {
			let k = search_kind(*tile);
			if !seen.insert(k) {
				continue;
			}
			for &node in &dests {
				out.push(SearchMove::Add { kind: k, node });
			}
		}
	}
	for mv in game.legal_moves() {
		out.push(SearchMove::from_action(game, mv));
	}
	if out.is_empty() && !game.rules().no_moves_loses {
		out.push(SearchMove::Pass);
	}
	out
}

pub fn apply_move(game: &mut Game, mv: SearchMove) -> Result<crate::game::ActionOutcome, String> {
	let action = mv.to_action(game).ok_or_else(|| "search move not applicable".to_string())?;
	game.apply(action)
}

/// True when the action removed at least one tile (capture / overwhelm).
/// Own-tile removals count: they follow the acting tile's capture mark.
pub fn is_capture(outcome: &crate::game::ActionOutcome, _mover: crate::game::Player) -> bool {
	!outcome.removals.is_empty() || outcome.draws > 0
}

/// Cheap move-ordering key (higher is searched first). Callers add TT/killer/history.
pub fn cheap_order(game: &Game, mv: SearchMove) -> i32 {
	match mv {
		SearchMove::Move { from, to } => {
			let mut s = 50;
			let center = game.board().center();
			if to == center {
				s += 900;
			} else if game.board().inner_ring().contains(&to) {
				s += 220;
			}
			if from == center {
				// Vacating our own center is almost never right under hold-to-win.
				s -= if game.rules().center_hold_win || game.rules().control_win {
					1_200
				} else {
					40
				};
			}
			if let Some(tile) = game.tile_at(from) {
				for nb in game.board().incident_others(to) {
					if let Some(other) = game.tile_at(nb) {
						if other.owner != tile.owner {
							s += 35;
						}
						let (matched, mismatches) = game.edge_score(tile, other);
						if matched > 0 {
							s += 8;
						} else if mismatches > 0 {
							s += 18;
						} else if tile.present_toward(other) && other.present_toward(tile) {
							s += 12;
						}
					}
				}
				if tile.capture.is_dot() {
					if game.bench(tile.owner).len() < 4 {
						s += 10;
					}
				} else if game.budget_token_count(tile.owner) < 3 {
					s += 10;
				}
			}
			s
		}
		SearchMove::Add { kind, node } => {
			let mut s = 12;
			let view = crate::game::decode_color_state(kind_index_of(kind) / KIND_DOTS);
			let add_all = view.normal | view.zag | view.braid | view.bead;
			let me = game.current_player();
			if capture_bit_of(kind) == 1 {
				if game.bench(me).len() < 4 {
					s += 8;
				}
			} else if game.budget_token_count(me) < 3 {
				s += 8;
			}
			for nb in game.board().incident_others(node) {
				if let Some(other) = game.tile_at(nb) {
					if other.owner == game.current_player() {
						s += 10;
					}
					let braid_hit = view.braid_size >= 2
						&& (0..6).all(|i| other.color_counts()[i] >= view.braid_counts[i]);
					let bead_unmatched =
						view.bead != 0 && (view.bead & other.color_mask()) != view.bead;
					if braid_hit {
						s += 14 + i32::from(view.braid_size);
					} else if bead_unmatched {
						s += 20;
					} else if other.normal_mask() & view.normal != 0 {
						s += 16;
					} else if other.color_mask() & view.zag != 0
						|| other
							.ring_colors()
							.iter()
							.enumerate()
							.any(|(i, c)| other.ring_is_zag(i) && add_all & c.bit() != 0)
					{
						s += 18;
					} else if (view.normal != 0 || view.braid_size >= 2 || view.bead != 0)
						&& (other.normal_mask() != 0
							|| other.braid_ring_count() >= 2
							|| other.bead_ring_count() > 0)
					{
						s += 6;
					}
				}
			}
			s
		}
		SearchMove::Pass => -1_000,
	}
}

#[cfg(test)]
mod tests {
	#![allow(clippy::unwrap_used, clippy::expect_used)]

	use super::*;
	use crate::game::{CaptureMark, Game, RuleSet};

	#[test]
	fn collapsed_adds_are_no_more_than_full_legal() {
		let game = Game::new(RuleSet::default(), 5);
		let full = game.legal_actions().len();
		let collapsed = legal_search_moves(&game).len();
		assert!(collapsed <= full);
		assert!(collapsed > 0);
	}

	#[test]
	fn round_trip_add_kind() {
		let game = Game::new(RuleSet::default(), 6);
		let mv = legal_search_moves(&game)[0];
		let action = mv.to_action(&game).unwrap();
		assert_eq!(SearchMove::from_action(&game, action), mv);
	}

	#[test]
	fn pack_round_trip_add_and_move() {
		let add = SearchMove::Add { kind: KIND_DOTS as u32 + 1, node: 3 };
		let (k, a, b) = add.pack();
		assert_eq!(SearchMove::unpack(k, a, b), Some(add));
		let zag_add = SearchMove::Add { kind: 20 * KIND_DOTS as u32 + 2, node: 5 };
		let packed = zag_add.pack();
		assert_eq!(SearchMove::unpack(packed.0, packed.1, packed.2), Some(zag_add));
		let braid = crate::game::Tile::with_typed_rings(
			0,
			crate::game::Player::One,
			&[
				(crate::game::TileColor::Orange, crate::game::RingType::Braid),
				(crate::game::TileColor::Green, crate::game::RingType::Braid),
			],
			1,
			1,
		);
		let braid_add = SearchMove::Add { kind: search_kind(braid), node: 4 };
		let packed = braid_add.pack();
		assert_eq!(SearchMove::unpack(packed.0, packed.1, packed.2), Some(braid_add));
		let dotted = braid.with_capture(CaptureMark::Dot);
		let dotted_add = SearchMove::Add { kind: search_kind(dotted), node: 4 };
		let packed = dotted_add.pack();
		assert_eq!(SearchMove::unpack(packed.0, packed.1, packed.2), Some(dotted_add));
		assert_ne!(braid_add, dotted_add);
		let mv = SearchMove::Move { from: 2, to: 7 };
		let (k, a, b) = mv.pack();
		assert_eq!(SearchMove::unpack(k, a, b), Some(mv));
	}

	#[test]
	fn cheap_order_prefers_a_fulfilled_braid_add() {
		let mut game = Game::empty(RuleSet::default());
		let spawn = game.board().spawns()[0];
		let nbs = game.board().neighbors(spawn).to_vec();
		let cover = game.alloc_rings(
			crate::game::Player::Two,
			&[crate::game::TileColor::Orange, crate::game::TileColor::Green],
			1,
			1,
		);
		game.place_tile(nbs[0], cover);
		let braid = crate::game::Tile::with_typed_rings(
			9,
			crate::game::Player::One,
			&[
				(crate::game::TileColor::Orange, crate::game::RingType::Braid),
				(crate::game::TileColor::Green, crate::game::RingType::Braid),
			],
			1,
			1,
		);
		let miss = crate::game::Tile::mono(
			10,
			crate::game::Player::One,
			crate::game::TileColor::Blue,
			1,
			1,
		);
		let hit = SearchMove::Add { kind: search_kind(braid), node: spawn };
		let miss_mv = SearchMove::Add { kind: search_kind(miss), node: spawn };
		assert!(cheap_order(&game, hit) > cheap_order(&game, miss_mv));
	}

	#[test]
	fn no_moves_loses_does_not_inject_pass() {
		let game = Game::empty(RuleSet::default());
		assert!(game.rules().no_moves_loses);
		assert!(legal_search_moves(&game).is_empty());
	}

	#[test]
	fn pass_is_injected_when_no_moves_loses_is_off() {
		let mut rules = RuleSet::default();
		rules.no_moves_loses = false;
		let game = Game::empty(rules);
		assert_eq!(legal_search_moves(&game), vec![SearchMove::Pass]);
	}
}
