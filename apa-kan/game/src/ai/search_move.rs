//! Search-facing actions. ADD is keyed by kind so transpositions are stable.

use crate::board::NodeId;
use crate::game::{Action, Game, KIND_DOTS};

use super::SearchConfig;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SearchMove {
	/// Low 16 bits: [`crate::game::Tile::kind_index`] (color-state × dots).
	/// Bit 16: [`crate::game::CaptureMark::bit`]. Bits 17–19:
	/// [`crate::game::Emblem::bit`] (0..=4). `KIND_COUNT` is not multiplied
	/// (it already fills a `u16`).
	Add {
		kind: u32,
		node: NodeId,
	},
	Move {
		from: NodeId,
		to: NodeId,
	},
	Claim {
		node: NodeId,
	},
	Swap {
		from: NodeId,
		to: NodeId,
	},
	Convert {
		node: NodeId,
	},
	/// [`Action::Recover`] (graveyard → bench).
	Recover,
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
			Action::Claim { node } => SearchMove::Claim { node },
			Action::Swap { from, to } => SearchMove::Swap { from, to },
			Action::Convert { node } => SearchMove::Convert { node },
			Action::Recover => SearchMove::Recover,
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
			SearchMove::Claim { node } => Some(Action::Claim { node }),
			SearchMove::Swap { from, to } => Some(Action::Swap { from, to }),
			SearchMove::Convert { node } => Some(Action::Convert { node }),
			SearchMove::Recover => Some(Action::Recover),
			SearchMove::Pass => Some(Action::Pass),
		}
	}

	/// Packed TT key: 0 = MOVE, u16::MAX = PASS, u16::MAX-1 = CLAIM,
	/// u16::MAX-3 = SWAP, u16::MAX-4 = CONVERT, 1..=COLOR_STATES = ADD
	/// color-state. (`u16::MAX-2` is reserved as the empty-TT dummy in
	/// minimax.) `b` is `dots | (capture_bit << 2) | (emblem << 3)`.
	pub fn pack(self) -> (u16, u8, u8) {
		match self {
			SearchMove::Add { kind, node } => {
				let dots_n = KIND_DOTS as u32;
				let base = kind & 0xFFFF;
				let capture = ((kind >> 16) as u8) & 1;
				let emblem = ((kind >> 17) as u8) & 7;
				let state = (base / dots_n + 1) as u16;
				let dots = (base % dots_n) as u8;
				(state, node, dots | (capture << 2) | (emblem << 3))
			}
			SearchMove::Move { from, to } => (0, from, to),
			SearchMove::Claim { node } => (u16::MAX - 1, node, 0),
			SearchMove::Swap { from, to } => (u16::MAX - 3, from, to),
			SearchMove::Convert { node } => (u16::MAX - 4, node, 0),
			SearchMove::Recover => (u16::MAX - 5, 0, 0),
			SearchMove::Pass => (u16::MAX, 0, 0),
		}
	}

	pub fn unpack(kind: u16, a: u8, b: u8) -> Option<Self> {
		match kind {
			u16::MAX => Some(SearchMove::Pass),
			k if k == u16::MAX - 1 => Some(SearchMove::Claim { node: a }),
			k if k == u16::MAX - 3 => Some(SearchMove::Swap { from: a, to: b }),
			k if k == u16::MAX - 4 => Some(SearchMove::Convert { node: a }),
			k if k == u16::MAX - 5 => Some(SearchMove::Recover),
			0 => Some(SearchMove::Move { from: a, to: b }),
			state
				if (1..=crate::game::COLOR_STATES as u16).contains(&state)
					&& (b & 3) < KIND_DOTS as u8
					&& ((b >> 2) & 1) <= 1
					&& (b >> 3) <= 4 =>
			{
				let dots = b & 3;
				let capture = (b >> 2) & 1;
				let emblem = b >> 3;
				Some(SearchMove::Add {
					kind: u32::from(state - 1) * KIND_DOTS as u32
						+ u32::from(dots) + (u32::from(capture) << 16)
						+ (u32::from(emblem) << 17),
					node: a,
				})
			}
			_ => None,
		}
	}
}

pub fn search_kind(tile: crate::game::Tile) -> u32 {
	(tile.kind_index() as u32)
		| (u32::from(tile.capture.bit()) << 16)
		| (u32::from(tile.emblem.bit()) << 17)
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
			game.add_spawns(current)
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
	for sw in game.legal_swaps() {
		out.push(SearchMove::from_action(game, sw));
	}
	for conv in game.legal_converts() {
		out.push(SearchMove::from_action(game, conv));
	}
	for claim in game.legal_claims() {
		out.push(SearchMove::from_action(game, claim));
	}
	for rec in game.legal_recovers() {
		out.push(SearchMove::from_action(game, rec));
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

/// Captures, plus SWAPs that seize or vacate the center. Used so LMR and
/// qsearch do not prune the saboteur dislodge.
pub fn is_tactical(game: &Game, mv: SearchMove, outcome: &crate::game::ActionOutcome) -> bool {
	if is_capture(outcome, game.current_player()) {
		return true;
	}
	match mv {
		SearchMove::Swap { from, to } => {
			let center = game.board().center();
			from == center || to == center
		}
		SearchMove::Convert { .. } => true,
		_ => false,
	}
}

/// Cheap move-ordering key (higher is searched first). Callers add TT/killer/history.
pub fn cheap_order(game: &Game, mv: SearchMove, config: SearchConfig) -> i32 {
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
				// Vacating a healthy center is rarely right under control/hold.
				// An overwhelmed occupant is a claim target; fleeing is counterplay.
				let tax = if game.rules().center_hold_win || game.rules().control_win {
					if game.is_overwhelmed(from) {
						config.order_vacate_overwhelmed
					} else {
						config.order_vacate_center
					}
				} else {
					40
				};
				s -= tax;
			}
			s += center_threat_bonus(game, to, config);
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
		SearchMove::Claim { node } => {
			let mut s = 280;
			if let Some(tile) = game.tile_at(node) {
				if tile.owner != game.current_player() {
					s += 160;
				} else {
					s += 40;
				}
				s += 12 * i32::from(tile.weight);
			}
			if game.budget_token_count(game.current_player()) < 3 {
				s += 30;
			}
			let center = game.board().center();
			if node == center || game.board().inner_ring().contains(&node) {
				s += config.order_claim_center;
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
			s += center_threat_bonus(game, node, config);
			let braid_scoring = game.rules().braid_scoring;
			let braid_min = braid_scoring.min_slots();
			for nb in game.board().incident_others(node) {
				if let Some(other) = game.tile_at(nb) {
					if other.owner == game.current_player() {
						s += 10;
					}
					let have = other.non_zag_colors_multiset();
					let braid_hit = view.braid_size >= braid_min
						&& if braid_scoring.exact_match() {
							(0..6).all(|i| {
								view.braid_counts[i] == 0 || have[i] == view.braid_counts[i]
							})
						} else {
							(0..6).all(|i| have[i] >= view.braid_counts[i])
						};
					let bead_unmatched =
						view.bead != 0 && (view.bead & other.non_zag_color_mask()) != view.bead;
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
					} else if (view.normal != 0 || view.braid_size >= braid_min || view.bead != 0)
						&& (other.normal_mask() != 0
							|| other.braid_ring_count() >= braid_min
							|| other.bead_ring_count() > 0)
					{
						s += 6;
					}
				}
			}
			s
		}
		SearchMove::Swap { from, to } => {
			let mut s = 80;
			let center = game.board().center();
			let inner = game.board().inner_ring();
			if to == center {
				s += 1200;
			} else if inner.contains(&to) {
				s += 220;
			}
			if from == center {
				let tax = if game.rules().center_hold_win || game.rules().control_win {
					if game.is_overwhelmed(from) {
						config.order_vacate_overwhelmed
					} else {
						config.order_vacate_center
					}
				} else {
					40
				};
				s -= tax;
			} else if inner.contains(&from) {
				s += 80;
			}
			s += center_threat_bonus(game, to, config);
			if let Some(partner) = game.tile_at(to) {
				if partner.owner != game.current_player() {
					s += 200;
					if to == center {
						s += 2500;
					}
				}
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
						}
					}
				}
			}
			s
		}
		SearchMove::Convert { node } => {
			let mut s = 600;
			let center = game.board().center();
			if node == center {
				s += 2800;
			} else if game.board().inner_ring().contains(&node) {
				s += 500;
			}
			if let Some(tile) = game.tile_at(node) {
				s += 40 * i32::from(tile.weight);
			}
			s
		}
		// Neutral: no ordering bias for graveyard returns.
		SearchMove::Recover => 0,
		SearchMove::Pass => -1_000,
	}
}

fn center_threat_bonus(game: &Game, dest: crate::board::NodeId, config: SearchConfig) -> i32 {
	if config.order_center_threat == 0 {
		return 0;
	}
	let center = game.board().center();
	let Some(occ) = game.tile_at(center) else {
		return 0;
	};
	if occ.owner == game.current_player() {
		return 0;
	}
	if !game.board().incident_others(dest).any(|nb| nb == center) && dest != center {
		return 0;
	}
	config.order_center_threat
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
		let moon = dotted.with_emblem(crate::game::Emblem::CrescentMoon);
		let moon_add = SearchMove::Add { kind: search_kind(moon), node: 4 };
		let packed = moon_add.pack();
		assert_eq!(SearchMove::unpack(packed.0, packed.1, packed.2), Some(moon_add));
		assert_ne!(dotted_add, moon_add);
		let wall = dotted.with_emblem(crate::game::Emblem::Blocker);
		let wall_add = SearchMove::Add { kind: search_kind(wall), node: 4 };
		let packed = wall_add.pack();
		assert_eq!(SearchMove::unpack(packed.0, packed.1, packed.2), Some(wall_add));
		let rose = dotted.with_emblem(crate::game::Emblem::Rosette);
		let rose_add = SearchMove::Add { kind: search_kind(rose), node: 4 };
		let packed = rose_add.pack();
		assert_eq!(SearchMove::unpack(packed.0, packed.1, packed.2), Some(rose_add));
		assert_ne!(wall_add, rose_add);
		let mv = SearchMove::Move { from: 2, to: 7 };
		let (k, a, b) = mv.pack();
		assert_eq!(SearchMove::unpack(k, a, b), Some(mv));
		let claim = SearchMove::Claim { node: 9 };
		let packed = claim.pack();
		assert_eq!(SearchMove::unpack(packed.0, packed.1, packed.2), Some(claim));
		let sw = SearchMove::Swap { from: 3, to: 0 };
		let packed = sw.pack();
		assert_eq!(SearchMove::unpack(packed.0, packed.1, packed.2), Some(sw));
		assert_ne!(packed.0, 0);
		assert_ne!(packed.0, u16::MAX);
		assert_ne!(packed.0, u16::MAX - 1);
		assert_ne!(packed.0, u16::MAX - 2);
		let conv = SearchMove::Convert { node: 0 };
		let packed = conv.pack();
		assert_eq!(SearchMove::unpack(packed.0, packed.1, packed.2), Some(conv));
		assert_ne!(packed.0, sw.pack().0);
	}

	#[test]
	fn cheap_order_prefers_swapping_an_opponent_off_center() {
		let mut game = Game::empty(RuleSet::default());
		let center = game.board().center();
		let inner = game.board().inner_ring()[0];
		let outer = *game
			.board()
			.neighbors(inner)
			.iter()
			.find(|&&n| n != center)
			.expect("inner node has a non-center neighbor");
		let saboteur =
			game.alloc_tile(crate::game::Player::One, crate::game::TileColor::Green, 1, 1);
		game.place_tile(inner, saboteur);
		let holder =
			game.alloc_tile(crate::game::Player::Two, crate::game::TileColor::Orange, 2, 1);
		game.place_tile(center, holder);
		let friend = game.alloc_tile(crate::game::Player::One, crate::game::TileColor::Blue, 1, 1);
		game.place_tile(outer, friend);
		game.set_current(crate::game::Player::One);
		let kick = SearchMove::Swap { from: inner, to: center };
		let rotate = SearchMove::Swap { from: inner, to: outer };
		let cfg = crate::ai::SearchConfig::counterplay();
		assert!(
			cheap_order(&game, kick, cfg) > cheap_order(&game, rotate, cfg),
			"center-kick {} vs own-rotate {}",
			cheap_order(&game, kick, cfg),
			cheap_order(&game, rotate, cfg)
		);
		assert!(is_tactical(
			&game,
			kick,
			&crate::game::ActionOutcome {
				action: crate::game::Action::Swap { from: inner, to: center },
				player: crate::game::Player::One,
				removals: Vec::new(),
				draws: 0,
				log_line: String::new(),
				emblem: crate::game::EmblemTrigger::default(),
			}
		));
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
		let cfg = crate::ai::SearchConfig::enhanced();
		assert!(cheap_order(&game, hit, cfg) > cheap_order(&game, miss_mv, cfg));
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

	#[test]
	fn apply_move_dot_capture_flips_opponent_onto_mover_bench() {
		let mut game = Game::empty(RuleSet::default());
		let spawn = game.board().spawns()[0];
		let nbs = game.board().neighbors(spawn).to_vec();
		let land = nbs[0];
		let from = *game
			.board()
			.neighbors(land)
			.iter()
			.find(|n| **n != spawn && !nbs.contains(n))
			.unwrap_or(&nbs[2]);
		let opp = game.alloc_tile(crate::game::Player::Two, crate::game::TileColor::Green, 2, 1);
		let opp_id = opp.id;
		game.place_tile(spawn, opp);
		let holder =
			game.alloc_tile(crate::game::Player::One, crate::game::TileColor::Orange, 1, 1);
		game.place_tile(nbs[1], holder);
		let mover = game
			.alloc_tile(crate::game::Player::One, crate::game::TileColor::Blue, 3, 1)
			.with_capture(CaptureMark::Dot);
		game.place_tile(from, mover);
		game.set_current(crate::game::Player::One);
		let outcome = apply_move(&mut game, SearchMove::Move { from, to: land }).unwrap();
		assert!(outcome.removals.iter().any(|r| r.tile.id == opp_id));
		assert!(game
			.bench(crate::game::Player::One)
			.iter()
			.any(|t| t.id == opp_id && t.owner == crate::game::Player::One));
		assert!(game.deck_slots(crate::game::Player::One).iter().any(|s| s
			.tile()
			.is_some_and(|t| t.id == opp_id && t.owner == crate::game::Player::One)));
	}
}
