//! Search-facing actions. ADD is keyed by kind so transpositions are stable.

use crate::board::NodeId;
use crate::game::{Action, Game, KIND_COUNT};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SearchMove {
	/// `kind` is [`crate::game::Tile::kind_index`] (color-state × weight × dots).
	Add {
		kind: u16,
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
					.map(|t| t.kind_index() as u16)
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
					.position(|t| t.kind_index() as u16 == kind)?;
				Some(Action::Add { bench_index: idx, node })
			}
			SearchMove::Move { from, to } => Some(Action::Move { from, to }),
			SearchMove::Pass => Some(Action::Pass),
		}
	}

	/// Packed TT key: 0 = MOVE, u16::MAX = PASS, 1..=COLOR_STATES = ADD color-state.
	pub fn pack(self) -> (u16, u8, u8) {
		match self {
			SearchMove::Add { kind, node } => {
				let state = kind / 9 + 1;
				let wd = (kind % 9) as u8;
				(state, node, wd)
			}
			SearchMove::Move { from, to } => (0, from, to),
			SearchMove::Pass => (u16::MAX, 0, 0),
		}
	}

	pub fn unpack(kind: u16, a: u8, b: u8) -> Option<Self> {
		match kind {
			u16::MAX => Some(SearchMove::Pass),
			0 => Some(SearchMove::Move { from: a, to: b }),
			state if (1..=crate::game::COLOR_STATES as u16).contains(&state) && b < 9 => {
				Some(SearchMove::Add { kind: (state - 1) * 9 + u16::from(b), node: a })
			}
			_ => None,
		}
	}
}

/// Legal actions with duplicate ADD kinds collapsed (one index per kind).
pub fn legal_search_moves(game: &Game) -> Vec<SearchMove> {
	if game.status().is_over() {
		return Vec::new();
	}
	let mut out = Vec::new();
	let current = game.current_player();
	let bench = game.bench(current);
	if !bench.is_empty() {
		let mut seen = [false; KIND_COUNT];
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
			let k = tile.kind_index();
			if seen[k] {
				continue;
			}
			seen[k] = true;
			for &node in &dests {
				out.push(SearchMove::Add { kind: k as u16, node });
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

/// True when the action removed at least one opponent tile (capture / overwhelm).
pub fn is_capture(outcome: &crate::game::ActionOutcome, mover: crate::game::Player) -> bool {
	outcome.removals.iter().any(|r| r.tile.owner != mover) || outcome.draws > 0
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
				for &nb in game.board().neighbors(to) {
					if let Some(other) = game.tile_at(nb) {
						if other.owner != tile.owner {
							s += 35;
						}
						if other.shares_normal(tile) {
							s += 8;
						} else if other.zag_hits_on(tile) > 0 || tile.zag_hits_on(other) > 0 {
							s += 18;
						} else {
							s += 12;
						}
					}
				}
			}
			s
		}
		SearchMove::Add { kind, node } => {
			let mut s = 12;
			let (normal, zag) = crate::game::color_state_masks((kind / 9) as usize);
			let add_all = normal | zag;
			for &nb in game.board().neighbors(node) {
				if let Some(other) = game.tile_at(nb) {
					if other.owner == game.current_player() {
						s += 10;
					}
					if other.normal_mask() & normal != 0 {
						s += 16;
					} else if other.color_mask() & zag != 0
						|| other
							.ring_colors()
							.iter()
							.enumerate()
							.any(|(i, c)| other.zag[i] && add_all & c.bit() != 0)
					{
						s += 18;
					} else {
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
	use crate::game::{Game, RuleSet};

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
		let add = SearchMove::Add { kind: 9 + 4, node: 3 };
		let (k, a, b) = add.pack();
		assert_eq!(SearchMove::unpack(k, a, b), Some(add));
		let zag_add = SearchMove::Add { kind: 20 * 9 + 2, node: 5 };
		let packed = zag_add.pack();
		assert_eq!(SearchMove::unpack(packed.0, packed.1, packed.2), Some(zag_add));
		let mv = SearchMove::Move { from: 2, to: 7 };
		let (k, a, b) = mv.pack();
		assert_eq!(SearchMove::unpack(k, a, b), Some(mv));
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
