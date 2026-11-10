//! Search-facing actions. ADD is keyed by kind so transpositions are stable.

use crate::board::NodeId;
use crate::game::{Action, Game, KIND_COUNT};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SearchMove {
	Add { kind: u8, node: NodeId },
	Move { from: NodeId, to: NodeId },
	Pass,
}

impl SearchMove {
	pub fn from_action(game: &Game, action: Action) -> Self {
		match action {
			Action::Add { bench_index, node } => {
				let kind = game
					.bench(game.current_player())
					.get(bench_index)
					.map(|t| t.kind_index() as u8)
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
					.position(|t| t.kind_index() as u8 == kind)?;
				Some(Action::Add { bench_index: idx, node })
			}
			SearchMove::Move { from, to } => Some(Action::Move { from, to }),
			SearchMove::Pass => Some(Action::Pass),
		}
	}

	pub fn pack(self) -> (u8, u8, u8) {
		match self {
			SearchMove::Add { kind, node } => (kind, node, 0),
			SearchMove::Move { from, to } => (27, from, to),
			SearchMove::Pass => (255, 0, 0),
		}
	}

	pub fn unpack(kind: u8, a: u8, b: u8) -> Option<Self> {
		match kind {
			255 => Some(SearchMove::Pass),
			27 => Some(SearchMove::Move { from: a, to: b }),
			k if (k as usize) < KIND_COUNT => Some(SearchMove::Add { kind: k, node: a }),
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
				out.push(SearchMove::Add { kind: k as u8, node });
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
						if other.color != tile.color {
							s += 12;
						} else {
							s += 8;
						}
					}
				}
			}
			s
		}
		SearchMove::Add { kind, node } => {
			let mut s = 12;
			let color_w_d = kind as usize;
			let color = color_w_d / 9;
			for &nb in game.board().neighbors(node) {
				if let Some(other) = game.tile_at(nb) {
					if other.owner == game.current_player() {
						s += 10;
					}
					if other.color as usize == color {
						s += 16;
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
