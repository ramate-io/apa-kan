//! Game model, RuleSet, overwhelm, movement, and win detection.

use std::sync::Arc;

use rand::rngs::StdRng;
use rand::seq::SliceRandom;
use rand::SeedableRng;

use crate::board::{Board, NodeId};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Player {
	One,
	Two,
}

impl Player {
	pub fn other(self) -> Self {
		match self {
			Player::One => Player::Two,
			Player::Two => Player::One,
		}
	}

	pub fn index(self) -> usize {
		match self {
			Player::One => 0,
			Player::Two => 1,
		}
	}

	pub fn label(self) -> &'static str {
		match self {
			Player::One => "P1",
			Player::Two => "P2",
		}
	}

	pub fn shade(self) -> &'static str {
		match self {
			Player::One => "light",
			Player::Two => "dark",
		}
	}
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum TileColor {
	Green,
	Blue,
	Orange,
}

impl TileColor {
	pub const ALL: [TileColor; 3] = [TileColor::Green, TileColor::Blue, TileColor::Orange];

	pub fn name(self) -> &'static str {
		match self {
			TileColor::Green => "green",
			TileColor::Blue => "blue",
			TileColor::Orange => "orange",
		}
	}
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Tile {
	pub id: u32,
	pub owner: Player,
	pub color: TileColor,
	pub weight: u8,
	pub dots: u8,
}

/// One of each (color × weight × dots) in `1..=3`.
pub const KIND_COUNT: usize = 27;

impl Tile {
	pub fn kind_label(self) -> String {
		format!("{} w{} d{}", self.color.name(), self.weight, self.dots)
	}

	/// Stable 0..26 index for the 27 (color, weight, dots) kinds.
	pub fn kind_index(self) -> usize {
		let w = (self.weight.clamp(1, 3) - 1) as usize;
		let d = (self.dots.clamp(1, 3) - 1) as usize;
		(self.color as usize) * 9 + w * 3 + d
	}
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EndReason {
	Elimination,
	TurnLimit,
	DoublePass,
	/// Player to move had no legal ADD or MOVE (`no_moves_loses`).
	LockedOut,
	/// Occupied the center for N consecutive own turns (`center_hold_win`).
	CenterHold,
}

/// Restriction on moving the same tile on two consecutive own turns.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SamePieceTwice {
	Off,
	Always,
	WhenBenchEmpty,
}

impl SamePieceTwice {
	pub fn parse(s: &str) -> Result<Self, String> {
		match s.to_ascii_lowercase().as_str() {
			"off" | "false" | "0" => Ok(Self::Off),
			"always" | "on" | "true" => Ok(Self::Always),
			"when-bench-empty" | "when_bench_empty" | "bench-empty" => Ok(Self::WhenBenchEmpty),
			other => Err(format!(
				"unknown no-same-piece-twice mode '{other}' (expected off|always|when-bench-empty)"
			)),
		}
	}

	pub fn name(self) -> &'static str {
		match self {
			Self::Off => "off",
			Self::Always => "always",
			Self::WhenBenchEmpty => "when-bench-empty",
		}
	}
}

/// Path uniqueness constraint for a single MOVE.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PathConstraint {
	/// Current/legacy: a path may not revisit a node, including hopped nodes.
	NoNodeRevisit,
	/// A path may not traverse the same undirected circle-arc twice. Nodes may
	/// be revisited via a different arc. The landing node must be empty and
	/// differ from the start.
	NoEdgeRevisit,
	/// Both node and edge uniqueness.
	Both,
}

impl PathConstraint {
	pub fn parse(s: &str) -> Result<Self, String> {
		match s.to_ascii_lowercase().as_str() {
			"no-node-revisit" | "no_node_revisit" | "node" => Ok(Self::NoNodeRevisit),
			"no-edge-revisit" | "no_edge_revisit" | "edge" => Ok(Self::NoEdgeRevisit),
			"both" => Ok(Self::Both),
			other => Err(format!(
				"unknown path-constraint '{other}' (expected no-node-revisit|no-edge-revisit|both)"
			)),
		}
	}

	pub fn name(self) -> &'static str {
		match self {
			Self::NoNodeRevisit => "no-node-revisit",
			Self::NoEdgeRevisit => "no-edge-revisit",
			Self::Both => "both",
		}
	}

	pub fn forbids_node_revisit(self) -> bool {
		matches!(self, Self::NoNodeRevisit | Self::Both)
	}

	pub fn forbids_edge_revisit(self) -> bool {
		matches!(self, Self::NoEdgeRevisit | Self::Both)
	}
}

/// Weight comparison used when [`RuleSet::color_restricted_hops`] is on.
///
/// Interpretation of the designer's "if the weight in your color is less than,
/// you can traverse": hop only over the mover's color, comparing occupant
/// weight to mover weight with this operator. Default is `<=` (`Le`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum HopWeightCmp {
	Lt,
	Le,
}

impl HopWeightCmp {
	pub fn parse(s: &str) -> Result<Self, String> {
		match s.to_ascii_lowercase().as_str() {
			"lt" | "<" | "less" => Ok(Self::Lt),
			"le" | "<=" | "leq" | "less-equal" => Ok(Self::Le),
			other => Err(format!("unknown hop-weight-cmp '{other}' (expected lt|le)")),
		}
	}

	pub fn name(self) -> &'static str {
		match self {
			Self::Lt => "lt",
			Self::Le => "le",
		}
	}

	pub fn allows(self, occupant_weight: u8, mover_weight: u8) -> bool {
		match self {
			Self::Lt => occupant_weight < mover_weight,
			Self::Le => occupant_weight <= mover_weight,
		}
	}
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GameStatus {
	InProgress,
	Won { winner: Player, reason: EndReason },
	Draw { reason: EndReason },
}

impl GameStatus {
	pub fn is_over(self) -> bool {
		!matches!(self, GameStatus::InProgress)
	}
}

/// Configurable rules. Defaults match the redesign spec; each field is listed
/// in `rfc/rfc-001-001-001-rosette-redesign`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RuleSet {
	/// Starting tiles dealt from each bag onto the bench. Default 5.
	pub starting_bench_size: u8,
	/// Both players receive the same shuffled 27-tile bag. Default true.
	pub identical_bags: bool,
	/// Bag contains one tile of each (color × weight × dots) kind (27). Default true.
	pub bag_one_of_each_kind: bool,
	/// On a MOVE, each removed opponent tile draws one bag tile onto the mover's bench.
	pub capture_draw_on_move: bool,
	/// If true, a mismatch adds the neighbor's weight instead of 1. Default false.
	pub mismatch_uses_neighbor_weight: bool,
	/// Actions a player may take before the turn passes. Default 1.
	pub actions_per_turn: u8,
	/// Draw when this many full turns have been completed. Default 200.
	pub turn_limit: u32,
	/// ADD is only legal on empty spawn nodes. Default true.
	pub add_only_on_spawns: bool,
	/// MOVE must consume exactly `dots` steps. Default true.
	pub move_exact_steps: bool,
	/// Hopped tiles must have weight >= the mover. Default true.
	/// Ignored when [`Self::color_restricted_hops`] is on.
	pub hop_requires_ge_weight: bool,
	/// If the player to move has no legal ADD or MOVE, they lose immediately
	/// instead of passing. Default true. Double-pass draws are disabled while
	/// this is on (a pass is never legal).
	pub no_moves_loses: bool,
	/// Win by occupying the center at the end of `center_hold_n` consecutive
	/// own turns. Default true (N = [`Self::center_hold_n`]).
	pub center_hold_win: bool,
	/// Consecutive own turns holding the center required to win. Default 5.
	pub center_hold_n: u8,
	/// Disallow moving the same tile on two consecutive own turns, with a
	/// capture exception. Default [`SamePieceTwice::WhenBenchEmpty`].
	pub no_same_piece_twice: SamePieceTwice,
	/// If true, a hop may only cross tiles of the mover's color, compared with
	/// [`Self::hop_weight_cmp`]. Default true.
	pub color_restricted_hops: bool,
	/// Occupant-vs-mover weight comparison used when color-restricted hops are
	/// on. Default [`HopWeightCmp::Le`] (occupant weight `<=` mover).
	pub hop_weight_cmp: HopWeightCmp,
	/// Uniqueness constraint along a single MOVE path.
	/// Default [`PathConstraint::NoEdgeRevisit`] (`no_repeated_edge` on;
	/// node revisits via a different arc are allowed).
	pub path_constraint: PathConstraint,
}

impl Default for RuleSet {
	fn default() -> Self {
		Self {
			starting_bench_size: 5,
			identical_bags: true,
			bag_one_of_each_kind: true,
			capture_draw_on_move: true,
			mismatch_uses_neighbor_weight: false,
			actions_per_turn: 1,
			turn_limit: 200,
			add_only_on_spawns: true,
			move_exact_steps: true,
			hop_requires_ge_weight: true,
			no_moves_loses: true,
			center_hold_win: true,
			center_hold_n: 5,
			no_same_piece_twice: SamePieceTwice::WhenBenchEmpty,
			color_restricted_hops: true,
			hop_weight_cmp: HopWeightCmp::Le,
			path_constraint: PathConstraint::NoEdgeRevisit,
		}
	}
}

impl RuleSet {
	pub fn default_rules() -> Self {
		Self::default()
	}
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Action {
	Add { bench_index: usize, node: NodeId },
	Move { from: NodeId, to: NodeId },
	Pass,
}

impl Action {
	pub fn is_pass(self) -> bool {
		matches!(self, Action::Pass)
	}
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Removal {
	pub node: NodeId,
	pub tile: Tile,
	pub matches: i32,
	pub mismatches: i32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ActionOutcome {
	pub action: Action,
	pub player: Player,
	pub removals: Vec<Removal>,
	pub draws: u8,
	pub log_line: String,
}

#[derive(Clone, Debug)]
pub struct Game {
	board: Arc<Board>,
	rules: RuleSet,
	tiles: Vec<Option<Tile>>,
	benches: [Vec<Tile>; 2],
	bags: [Vec<Tile>; 2],
	current: Player,
	status: GameStatus,
	turn_count: u32,
	actions_this_turn: u8,
	consecutive_passes: u8,
	next_tile_id: u32,
	seed: u64,
	/// Tile id moved on each player's most recent completed turn.
	last_moved: [Option<u32>; 2],
	/// Tile id moved so far this turn (cleared when the turn ends).
	moved_this_turn: Option<u32>,
	/// Consecutive own turns each player has ended occupying the center.
	center_hold: [u8; 2],
}

impl Game {
	pub fn new(rules: RuleSet, seed: u64) -> Self {
		Self::new_on(Arc::new(Board::rosette_8()), rules, seed)
	}

	pub fn new_on(board: Arc<Board>, rules: RuleSet, seed: u64) -> Self {
		let n = board.node_count();
		let mut next_tile_id = 1u32;
		let mut rng = StdRng::seed_from_u64(seed);
		let p1_bag =
			build_bag(Player::One, rules.bag_one_of_each_kind, &mut next_tile_id, &mut rng);
		let p2_bag = if rules.identical_bags {
			p1_bag
				.iter()
				.map(|t| {
					let id = next_tile_id;
					next_tile_id += 1;
					Tile { id, owner: Player::Two, ..*t }
				})
				.collect()
		} else {
			build_bag(Player::Two, rules.bag_one_of_each_kind, &mut next_tile_id, &mut rng)
		};
		let mut game = Self {
			board,
			rules,
			tiles: vec![None; n],
			benches: [Vec::new(), Vec::new()],
			bags: [p1_bag, p2_bag],
			current: Player::One,
			status: GameStatus::InProgress,
			turn_count: 0,
			actions_this_turn: 0,
			consecutive_passes: 0,
			next_tile_id,
			seed,
			last_moved: [None, None],
			moved_this_turn: None,
			center_hold: [0, 0],
		};
		game.deal_opening();
		game
	}

	/// Empty board, empty benches and bags. For constructed test positions.
	pub fn empty(rules: RuleSet) -> Self {
		Self::empty_on(Arc::new(Board::rosette_8()), rules)
	}

	pub fn empty_on(board: Arc<Board>, rules: RuleSet) -> Self {
		let n = board.node_count();
		Self {
			board,
			rules,
			tiles: vec![None; n],
			benches: [Vec::new(), Vec::new()],
			bags: [Vec::new(), Vec::new()],
			current: Player::One,
			status: GameStatus::InProgress,
			turn_count: 0,
			actions_this_turn: 0,
			consecutive_passes: 0,
			next_tile_id: 1,
			seed: 0,
			last_moved: [None, None],
			moved_this_turn: None,
			center_hold: [0, 0],
		}
	}

	fn deal_opening(&mut self) {
		let n = self.rules.starting_bench_size as usize;
		for p in [Player::One, Player::Two] {
			for _ in 0..n {
				let _ = self.draw_from_bag(p);
			}
		}
	}

	fn draw_from_bag(&mut self, player: Player) -> Option<Tile> {
		let bag = &mut self.bags[player.index()];
		let tile = bag.pop()?;
		self.benches[player.index()].push(tile);
		Some(tile)
	}

	pub fn board(&self) -> &Board {
		&self.board
	}

	pub fn rules(&self) -> &RuleSet {
		&self.rules
	}

	pub fn seed(&self) -> u64 {
		self.seed
	}

	pub fn current_player(&self) -> Player {
		self.current
	}

	pub fn status(&self) -> GameStatus {
		self.status
	}

	pub fn turn_count(&self) -> u32 {
		self.turn_count
	}

	pub fn actions_this_turn(&self) -> u8 {
		self.actions_this_turn
	}

	pub fn consecutive_passes(&self) -> u8 {
		self.consecutive_passes
	}

	/// Consecutive own turns `player` has ended occupying the center.
	pub fn center_hold(&self, player: Player) -> u8 {
		self.center_hold[player.index()]
	}

	/// Tile moved on `player`'s previous completed turn, if any.
	pub fn last_moved_tile(&self, player: Player) -> Option<u32> {
		self.last_moved[player.index()]
	}

	pub fn moved_this_turn(&self) -> Option<u32> {
		self.moved_this_turn
	}

	pub fn tile_at(&self, node: NodeId) -> Option<Tile> {
		self.tiles.get(node as usize).copied().flatten()
	}

	pub fn tiles(&self) -> impl Iterator<Item = (NodeId, Tile)> + '_ {
		self.tiles
			.iter()
			.enumerate()
			.filter_map(|(i, t)| t.map(|tile| (i as NodeId, tile)))
	}

	pub fn bench(&self, player: Player) -> &[Tile] {
		&self.benches[player.index()]
	}

	pub fn bag_len(&self, player: Player) -> usize {
		self.bags[player.index()].len()
	}

	pub fn bag(&self, player: Player) -> &[Tile] {
		&self.bags[player.index()]
	}

	/// Reorder remaining bag tiles. The multiset is unchanged; capture-draw
	/// order becomes the sampled permutation (determinization).
	pub fn shuffle_bags(&mut self, rng: &mut impl rand::Rng) {
		self.bags[0].shuffle(rng);
		self.bags[1].shuffle(rng);
	}

	pub fn board_count(&self, player: Player) -> usize {
		self.tiles
			.iter()
			.filter(|t| t.map(|x| x.owner == player).unwrap_or(false))
			.count()
	}

	pub fn is_alive(&self, player: Player) -> bool {
		self.board_count(player) > 0 || !self.benches[player.index()].is_empty()
	}

	pub fn set_current(&mut self, player: Player) {
		self.current = player;
	}

	#[cfg(test)]
	pub(crate) fn set_status(&mut self, status: GameStatus) {
		self.status = status;
	}

	pub fn place_tile(&mut self, node: NodeId, tile: Tile) {
		if tile.id >= self.next_tile_id {
			self.next_tile_id = tile.id + 1;
		}
		self.tiles[node as usize] = Some(tile);
	}

	pub fn clear_tile(&mut self, node: NodeId) -> Option<Tile> {
		self.tiles[node as usize].take()
	}

	pub fn set_bench(&mut self, player: Player, tiles: Vec<Tile>) {
		for t in &tiles {
			if t.id >= self.next_tile_id {
				self.next_tile_id = t.id + 1;
			}
		}
		self.benches[player.index()] = tiles;
	}

	pub fn set_bag(&mut self, player: Player, tiles: Vec<Tile>) {
		for t in &tiles {
			if t.id >= self.next_tile_id {
				self.next_tile_id = t.id + 1;
			}
		}
		self.bags[player.index()] = tiles;
	}

	pub fn alloc_tile(&mut self, owner: Player, color: TileColor, weight: u8, dots: u8) -> Tile {
		let id = self.next_tile_id;
		self.next_tile_id += 1;
		Tile { id, owner, color, weight, dots }
	}

	pub fn tile_score(&self, node: NodeId) -> Option<(i32, i32)> {
		let tile = self.tile_at(node)?;
		let mut matches = i32::from(tile.weight);
		let mut mismatches = 0i32;
		for &nb in self.board.neighbors(node) {
			if let Some(other) = self.tile_at(nb) {
				if other.color == tile.color {
					matches += 1;
				} else if self.rules.mismatch_uses_neighbor_weight {
					mismatches += i32::from(other.weight);
				} else {
					mismatches += 1;
				}
			}
		}
		Some((matches, mismatches))
	}

	pub fn is_overwhelmed(&self, node: NodeId) -> bool {
		match self.tile_score(node) {
			Some((m, n)) => n >= m,
			None => false,
		}
	}

	/// Would `tile` be overwhelmed if placed at `node` (other tiles unchanged)?
	pub fn would_overwhelm_add(&self, tile: Tile, node: NodeId) -> bool {
		if self.tile_at(node).is_some() {
			return false;
		}
		let mut matches = i32::from(tile.weight);
		let mut mismatches = 0i32;
		for &nb in self.board.neighbors(node) {
			if let Some(other) = self.tile_at(nb) {
				if other.color == tile.color {
					matches += 1;
				} else if self.rules.mismatch_uses_neighbor_weight {
					mismatches += i32::from(other.weight);
				} else {
					mismatches += 1;
				}
			}
		}
		mismatches >= matches
	}

	pub fn collect_removals(&self) -> Vec<Removal> {
		let mut out = Vec::new();
		for (node, tile) in self.tiles() {
			if let Some((matches, mismatches)) = self.tile_score(node) {
				if mismatches >= matches {
					out.push(Removal { node, tile, matches, mismatches });
				}
			}
		}
		out.sort_by_key(|r| r.node);
		out
	}

	pub fn legal_actions(&self) -> Vec<Action> {
		if self.status.is_over() {
			return Vec::new();
		}
		let mut acts = Vec::new();
		acts.extend(self.legal_adds());
		acts.extend(self.legal_moves());
		if acts.is_empty() && !self.rules.no_moves_loses {
			acts.push(Action::Pass);
		}
		acts
	}

	pub fn legal_adds(&self) -> Vec<Action> {
		if self.status.is_over() {
			return Vec::new();
		}
		let bench = &self.benches[self.current.index()];
		if bench.is_empty() {
			return Vec::new();
		}
		let mut dests = Vec::new();
		if self.rules.add_only_on_spawns {
			for &s in self.board.spawns() {
				if self.tile_at(s).is_none() {
					dests.push(s);
				}
			}
		} else {
			for id in 0..self.board.node_count() as NodeId {
				if self.tile_at(id).is_none() {
					dests.push(id);
				}
			}
		}
		let mut acts = Vec::new();
		for i in 0..bench.len() {
			for &node in &dests {
				acts.push(Action::Add { bench_index: i, node });
			}
		}
		acts
	}

	pub fn legal_moves(&self) -> Vec<Action> {
		if self.status.is_over() {
			return Vec::new();
		}
		let mut acts = Vec::new();
		for (node, tile) in self.tiles() {
			if tile.owner != self.current {
				continue;
			}
			let restricted = self.same_piece_is_restricted(tile.id);
			for dest in self.destinations(node) {
				if restricted && !self.move_removes_opponent(node, dest) {
					continue;
				}
				acts.push(Action::Move { from: node, to: dest });
			}
		}
		acts
	}

	fn same_piece_is_restricted(&self, tile_id: u32) -> bool {
		if self.last_moved[self.current.index()] != Some(tile_id) {
			return false;
		}
		match self.rules.no_same_piece_twice {
			SamePieceTwice::Off => false,
			SamePieceTwice::Always => true,
			SamePieceTwice::WhenBenchEmpty => self.benches[self.current.index()].is_empty(),
		}
	}

	/// True if moving the tile at `from` to `to` would remove at least one
	/// opponent tile under simultaneous overwhelm.
	pub fn move_removes_opponent(&self, from: NodeId, to: NodeId) -> bool {
		let Some(tile) = self.tile_at(from) else {
			return false;
		};
		let mut preview = self.clone();
		preview.tiles[from as usize] = None;
		preview.tiles[to as usize] = Some(tile);
		let opponent = tile.owner.other();
		preview.collect_removals().iter().any(|r| r.tile.owner == opponent)
	}

	pub fn has_add_or_move(&self) -> bool {
		!self.legal_adds().is_empty() || !self.legal_moves().is_empty()
	}

	/// Destinations that are legal for the tile at `from` under path rules
	/// *and* the same-piece restriction (including the capture exception).
	pub fn legal_destinations(&self, from: NodeId) -> Vec<NodeId> {
		let Some(tile) = self.tile_at(from) else {
			return Vec::new();
		};
		if tile.owner != self.current {
			return Vec::new();
		}
		let restricted = self.same_piece_is_restricted(tile.id);
		self.destinations(from)
			.into_iter()
			.filter(|&to| !restricted || self.move_removes_opponent(from, to))
			.collect()
	}

	/// Exact-`dots` destinations for the tile at `from`, with the mover vacated.
	pub fn destinations(&self, from: NodeId) -> Vec<NodeId> {
		let Some(tile) = self.tile_at(from) else {
			return Vec::new();
		};
		let steps = tile.dots.max(1);
		let mut occupancy = self.tiles.clone();
		occupancy[from as usize] = None;
		let forbid_node = self.rules.path_constraint.forbids_node_revisit();
		let forbid_edge = self.rules.path_constraint.forbids_edge_revisit();
		let mut found = Vec::new();
		let mut stack = vec![PathState {
			node: from,
			remaining: steps,
			visited_nodes: 1u32 << from,
			visited_edges: 0,
		}];
		while let Some(state) = stack.pop() {
			if state.remaining == 0 {
				if state.node != from && occupancy[state.node as usize].is_none() {
					if !found.contains(&state.node) {
						found.push(state.node);
					}
				}
				continue;
			}
			let circles: Vec<u8> = self.board.circles_through(state.node).collect();
			for circle in circles {
				for ccw in [true, false] {
					let Some(next) = self.board.next_on_circle(circle, state.node, ccw) else {
						continue;
					};
					let Some(edge) = self.board.undirected_arc_index(circle, state.node, next)
					else {
						continue;
					};
					if forbid_edge && state.visited_edges & (1u64 << edge) != 0 {
						continue;
					}
					if occupancy[next as usize].is_none() {
						if forbid_node && state.visited_nodes & (1u32 << next) != 0 {
							continue;
						}
						stack.push(PathState {
							node: next,
							remaining: state.remaining - 1,
							visited_nodes: state.visited_nodes | (1u32 << next),
							visited_edges: state.visited_edges | (1u64 << edge),
						});
					} else if let Some((land, hopped_nodes, hopped_edges)) =
						self.hop_along(&occupancy, circle, state.node, ccw, tile)
					{
						if forbid_edge && state.visited_edges & hopped_edges != 0 {
							continue;
						}
						if forbid_node
							&& (state.visited_nodes & hopped_nodes != 0
								|| state.visited_nodes & (1u32 << land) != 0)
						{
							continue;
						}
						stack.push(PathState {
							node: land,
							remaining: state.remaining - 1,
							visited_nodes: state.visited_nodes | hopped_nodes | (1u32 << land),
							visited_edges: state.visited_edges | hopped_edges,
						});
					}
				}
			}
		}
		found.sort();
		found
	}

	fn hop_occupant_ok(&self, occupant: Tile, mover: Tile) -> bool {
		if self.rules.color_restricted_hops {
			occupant.color == mover.color
				&& self.rules.hop_weight_cmp.allows(occupant.weight, mover.weight)
		} else if self.rules.hop_requires_ge_weight {
			occupant.weight >= mover.weight
		} else {
			true
		}
	}

	fn hop_along(
		&self,
		occupancy: &[Option<Tile>],
		circle: u8,
		from: NodeId,
		ccw: bool,
		mover: Tile,
	) -> Option<(NodeId, u32, u64)> {
		let mut n = self.board.next_on_circle(circle, from, ccw)?;
		if occupancy[n as usize].is_none() {
			return None;
		}
		let mut hopped_nodes = 0u32;
		let mut hopped_edges = 0u64;
		let mut prev = from;
		let start = n;
		loop {
			let edge = self.board.undirected_arc_index(circle, prev, n)?;
			hopped_edges |= 1u64 << edge;
			let occupant = occupancy[n as usize]?;
			if !self.hop_occupant_ok(occupant, mover) {
				return None;
			}
			hopped_nodes |= 1u32 << n;
			prev = n;
			n = self.board.next_on_circle(circle, n, ccw)?;
			if n == start || n == from {
				return None;
			}
			if occupancy[n as usize].is_none() {
				let land_edge = self.board.undirected_arc_index(circle, prev, n)?;
				hopped_edges |= 1u64 << land_edge;
				return Some((n, hopped_nodes, hopped_edges));
			}
		}
	}

	pub fn apply(&mut self, action: Action) -> Result<ActionOutcome, String> {
		if self.status.is_over() {
			return Err("game is over".into());
		}
		let player = self.current;
		match action {
			Action::Add { bench_index, node } => self.apply_add(bench_index, node)?,
			Action::Move { from, to } => self.apply_move(from, to)?,
			Action::Pass => {
				if self.has_add_or_move() {
					return Err("pass is only legal when no ADD or MOVE exists".into());
				}
				if self.rules.no_moves_loses {
					self.status =
						GameStatus::Won { winner: player.other(), reason: EndReason::LockedOut };
				}
			}
		}

		if action.is_pass() {
			self.consecutive_passes = self.consecutive_passes.saturating_add(1);
		} else {
			self.consecutive_passes = 0;
		}

		let removals = self.collect_removals();
		let opponent = player.other();
		let mut draws = 0u8;
		let is_move = matches!(action, Action::Move { .. });
		for rem in &removals {
			self.tiles[rem.node as usize] = None;
			if is_move && self.rules.capture_draw_on_move && rem.tile.owner == opponent {
				if self.draw_from_bag(player).is_some() {
					draws = draws.saturating_add(1);
				}
			}
		}

		let log_line = format_log(player, action, &removals, draws, self.board.as_ref());

		self.check_elimination();
		if self.status == GameStatus::InProgress
			&& !self.rules.no_moves_loses
			&& self.consecutive_passes >= 2
		{
			self.status = GameStatus::Draw { reason: EndReason::DoublePass };
		}

		self.actions_this_turn = self.actions_this_turn.saturating_add(1);
		let cap = self.rules.actions_per_turn.max(1);
		let turn_done = action.is_pass()
			|| self.actions_this_turn >= cap
			|| self.status.is_over()
			|| !self.has_add_or_move();
		if turn_done && !self.status.is_over() {
			self.finish_turn(player);
		}

		Ok(ActionOutcome { action, player, removals, draws, log_line })
	}

	fn finish_turn(&mut self, who: Player) {
		self.last_moved[who.index()] = self.moved_this_turn.take();
		self.update_center_hold(who);
		self.actions_this_turn = 0;
		self.turn_count = self.turn_count.saturating_add(1);
		if self.status.is_over() {
			return;
		}
		if self.turn_count >= self.rules.turn_limit {
			self.status = GameStatus::Draw { reason: EndReason::TurnLimit };
			return;
		}
		self.current = who.other();
		if self.rules.no_moves_loses && !self.has_add_or_move() {
			self.status = GameStatus::Won { winner: who, reason: EndReason::LockedOut };
		}
	}

	fn update_center_hold(&mut self, who: Player) {
		if !self.rules.center_hold_win {
			self.center_hold = [0, 0];
			return;
		}
		let center = self.board.center();
		let occupant = self.tile_at(center);
		if occupant.map(|t| t.owner == who).unwrap_or(false) {
			self.center_hold[who.index()] = self.center_hold[who.index()].saturating_add(1);
			if self.center_hold[who.index()] >= self.rules.center_hold_n.max(1) {
				self.status = GameStatus::Won { winner: who, reason: EndReason::CenterHold };
			}
		} else {
			self.center_hold[who.index()] = 0;
		}
		let opp = who.other();
		let opp_holds = occupant.map(|t| t.owner == opp).unwrap_or(false);
		if !opp_holds {
			self.center_hold[opp.index()] = 0;
		}
	}

	fn apply_add(&mut self, bench_index: usize, node: NodeId) -> Result<(), String> {
		if self.rules.add_only_on_spawns && !self.board.is_spawn(node) {
			return Err("ADD is only legal on spawn nodes".into());
		}
		if self.tile_at(node).is_some() {
			return Err("destination is occupied".into());
		}
		let bench = &mut self.benches[self.current.index()];
		if bench_index >= bench.len() {
			return Err("bench index out of range".into());
		}
		let tile = bench.remove(bench_index);
		self.tiles[node as usize] = Some(tile);
		Ok(())
	}

	fn apply_move(&mut self, from: NodeId, to: NodeId) -> Result<(), String> {
		let tile = self.tile_at(from).ok_or_else(|| "no tile at source".to_string())?;
		if tile.owner != self.current {
			return Err("not your tile".into());
		}
		if !self.destinations(from).contains(&to) {
			return Err("illegal MOVE destination".into());
		}
		if self.same_piece_is_restricted(tile.id) && !self.move_removes_opponent(from, to) {
			return Err(
				"cannot move the same tile on two consecutive turns unless the move captures"
					.into(),
			);
		}
		self.tiles[from as usize] = None;
		self.tiles[to as usize] = Some(tile);
		self.moved_this_turn = Some(tile.id);
		Ok(())
	}

	fn check_elimination(&mut self) {
		if self.status.is_over() {
			return;
		}
		let p1 = self.is_alive(Player::One);
		let p2 = self.is_alive(Player::Two);
		match (p1, p2) {
			(false, false) => {
				self.status = GameStatus::Draw { reason: EndReason::Elimination };
			}
			(false, true) => {
				self.status =
					GameStatus::Won { winner: Player::Two, reason: EndReason::Elimination };
			}
			(true, false) => {
				self.status =
					GameStatus::Won { winner: Player::One, reason: EndReason::Elimination };
			}
			(true, true) => {}
		}
	}
}

#[derive(Clone, Copy)]
struct PathState {
	node: NodeId,
	remaining: u8,
	visited_nodes: u32,
	visited_edges: u64,
}

fn build_bag(owner: Player, one_of_each: bool, next_id: &mut u32, rng: &mut StdRng) -> Vec<Tile> {
	let mut bag = Vec::new();
	if !one_of_each {
		return bag;
	}
	for color in TileColor::ALL {
		for weight in 1..=3 {
			for dots in 1..=3 {
				let id = *next_id;
				*next_id += 1;
				bag.push(Tile { id, owner, color, weight, dots });
			}
		}
	}
	bag.shuffle(rng);
	bag
}

fn format_log(
	player: Player,
	action: Action,
	removals: &[Removal],
	draws: u8,
	board: &Board,
) -> String {
	let act = match action {
		Action::Add { bench_index, node } => {
			format!("ADD bench[{bench_index}] to {}{}", node_tag(board, node), node)
		}
		Action::Move { from, to } => format!("MOVE {} -> {}", from, to),
		Action::Pass => "PASS".to_string(),
	};
	let mut line = format!("{} ({}) {}", player.label(), player.shade(), act);
	if !removals.is_empty() {
		let bits: Vec<String> = removals
			.iter()
			.map(|r| {
				format!(
					"{} {} at {} (matches={}, mismatches={})",
					r.tile.owner.label(),
					r.tile.kind_label(),
					r.node,
					r.matches,
					r.mismatches
				)
			})
			.collect();
		line.push_str("; removed ");
		line.push_str(&bits.join("; "));
	}
	if draws > 0 {
		line.push_str(&format!("; drew {draws} from bag"));
	}
	line
}

fn node_tag(board: &Board, node: NodeId) -> &'static str {
	if board.is_spawn(node) {
		"spawn "
	} else if node == board.center() {
		"center "
	} else {
		"node "
	}
}

#[cfg(test)]
mod tests {
	#![allow(clippy::unwrap_used, clippy::expect_used)]

	use super::*;
	use rand::rngs::StdRng;
	use rand::SeedableRng;

	fn green(game: &mut Game, owner: Player, weight: u8, dots: u8) -> Tile {
		game.alloc_tile(owner, TileColor::Green, weight, dots)
	}
	fn take(game: &mut Game, owner: Player, color: TileColor, weight: u8, dots: u8) -> Tile {
		game.alloc_tile(owner, color, weight, dots)
	}
	fn put(game: &mut Game, node: NodeId, owner: Player, color: TileColor, weight: u8, dots: u8) {
		let tile = take(game, owner, color, weight, dots);
		game.place_tile(node, tile);
	}

	#[test]
	fn opening_deal_is_five_from_identical_bags() {
		let a = Game::new(RuleSet::default(), 7);
		let b = Game::new(RuleSet::default(), 7);
		assert_eq!(a.bench(Player::One).len(), 5);
		assert_eq!(a.bench(Player::Two).len(), 5);
		assert_eq!(a.bag_len(Player::One), 22);
		assert_eq!(a.bag_len(Player::Two), 22);
		let kinds = |g: &Game, p: Player| {
			g.bench(p).iter().map(|t| (t.color, t.weight, t.dots)).collect::<Vec<_>>()
		};
		assert_eq!(kinds(&a, Player::One), kinds(&b, Player::One));
		assert_eq!(kinds(&a, Player::One), kinds(&a, Player::Two));
		assert!(a.bench(Player::One).iter().all(|t| t.owner == Player::One));
		assert!(a.bench(Player::Two).iter().all(|t| t.owner == Player::Two));
	}

	#[test]
	fn double_green_overwhelmed_by_two_oranges_survives_with_green_neighbor() {
		let mut game = Game::empty(RuleSet::default());
		let spawn = game.board().spawns()[0];
		let nbs = game.board().neighbors(spawn).to_vec();
		assert!(nbs.len() >= 3);
		put(&mut game, spawn, Player::One, TileColor::Green, 2, 1);
		put(&mut game, nbs[0], Player::Two, TileColor::Orange, 1, 1);
		put(&mut game, nbs[1], Player::One, TileColor::Orange, 3, 2);
		let (m, n) = game.tile_score(spawn).unwrap();
		assert_eq!((m, n), (2, 2), "own weight 2, two different-color neighbors");
		assert!(game.is_overwhelmed(spawn));

		put(&mut game, nbs[2], Player::Two, TileColor::Green, 1, 3);
		let (m, n) = game.tile_score(spawn).unwrap();
		assert_eq!((m, n), (3, 2), "double green plus one green neighbor");
		assert!(!game.is_overwhelmed(spawn));
	}

	#[test]
	fn overwhelm_is_by_color_not_owner() {
		let mut game = Game::empty(RuleSet::default());
		let spawn = game.board().spawns()[1];
		let nbs = game.board().neighbors(spawn).to_vec();
		put(&mut game, spawn, Player::One, TileColor::Green, 1, 1);
		put(&mut game, nbs[0], Player::Two, TileColor::Green, 3, 1);
		let (m, n) = game.tile_score(spawn).unwrap();
		assert_eq!(m, 2);
		assert_eq!(n, 0);
		assert!(!game.is_overwhelmed(spawn));
	}

	#[test]
	fn mismatch_weight_option_counts_neighbor_weight() {
		let mut rules = RuleSet::default();
		rules.mismatch_uses_neighbor_weight = true;
		let mut game = Game::empty(rules);
		let spawn = game.board().spawns()[2];
		let nbs = game.board().neighbors(spawn).to_vec();
		put(&mut game, spawn, Player::One, TileColor::Green, 3, 1);
		put(&mut game, nbs[0], Player::Two, TileColor::Orange, 3, 1);
		let (m, n) = game.tile_score(spawn).unwrap();
		assert_eq!(m, 3);
		assert_eq!(n, 3);
		assert!(game.is_overwhelmed(spawn));
	}

	#[test]
	fn simultaneous_removal_logs_owner_node_and_counts() {
		let mut game = Game::empty(RuleSet::default());
		let s0 = game.board().spawns()[0];
		let s1 = game.board().spawns()[1];
		let n0 = game.board().neighbors(s0).to_vec();
		let shared = *n0.iter().find(|n| game.board().neighbors(s1).contains(n)).unwrap_or(&n0[0]);
		put(&mut game, s0, Player::One, TileColor::Green, 1, 1);
		put(&mut game, n0[0], Player::Two, TileColor::Orange, 1, 1);
		if n0.len() > 1 {
			put(&mut game, n0[1], Player::Two, TileColor::Orange, 1, 1);
		}
		let _ = shared;
		let _ = s1;
		game.set_current(Player::One);
		{
			let _t = take(&mut game, Player::One, TileColor::Blue, 1, 1);
			game.set_bench(Player::One, vec![_t]);
		};
		let empty_spawn = game
			.board()
			.spawns()
			.iter()
			.copied()
			.find(|&s| game.tile_at(s).is_none())
			.unwrap();
		let outcome = game.apply(Action::Add { bench_index: 0, node: empty_spawn }).unwrap();
		assert!(game.collect_removals().is_empty(), "removals already applied");
		for rem in &outcome.removals {
			assert!(rem.mismatches >= rem.matches);
			assert!(outcome.log_line.contains(&format!("matches={}", rem.matches)));
			assert!(outcome.log_line.contains(&format!("mismatches={}", rem.mismatches)));
			assert!(outcome.log_line.contains(&format!("at {}", rem.node)));
		}
	}

	#[test]
	fn move_is_exactly_d_steps_not_up_to_d() {
		let mut game = Game::empty(RuleSet::default());
		let from = game.board().spawns()[0];
		let mut step1 = 0u8;
		let mut step2 = 0u8;
		let mut found = false;
		for circle in game.board().circles_through(from) {
			for ccw in [true, false] {
				let a = game.board().next_on_circle(circle, from, ccw).unwrap();
				let b = game.board().next_on_circle(circle, a, ccw).unwrap();
				if !game.board().neighbors(from).contains(&b) {
					step1 = a;
					step2 = b;
					found = true;
					break;
				}
			}
			if found {
				break;
			}
		}
		assert!(found, "spawn should have a 2-step circle walk that is not a direct neighbor");
		let tile = green(&mut game, Player::One, 1, 2);
		game.place_tile(from, tile);
		game.set_current(Player::One);
		let dests2 = game.destinations(from);
		assert!(
			dests2.contains(&step2),
			"d=2 must reach the two-step node {step2}, got {dests2:?}"
		);
		game.clear_tile(from);
		let one = green(&mut game, Player::Two, 1, 1);
		game.place_tile(from, one);
		game.set_current(Player::Two);
		let dests1 = game.destinations(from);
		assert!(dests1.contains(&step1), "d=1 should reach {step1}");
		assert!(!dests1.contains(&step2), "d=1 must not reach the two-step node {step2}");
		assert_ne!(dests1, dests2, "exact-d MOVE: d=1 and d=2 destination sets differ");
	}

	#[test]
	fn move_cannot_revisit_a_node() {
		let mut game = Game::empty(RuleSet::default());
		let from = game.board().spawns()[3];
		put(&mut game, from, Player::One, TileColor::Blue, 1, 2);
		game.set_current(Player::One);
		assert!(!game.destinations(from).contains(&from));
	}

	#[test]
	fn hop_costs_one_and_requires_weight() {
		let mut rules = RuleSet::default();
		rules.color_restricted_hops = false;
		let mut game = Game::empty(rules);
		let b = game.board().clone();
		let (from, hop_over, land) = {
			let circle = 0u8;
			let cycle = b.circle_cycle(circle);
			(cycle[0], cycle[1], cycle[2])
		};
		put(&mut game, from, Player::One, TileColor::Green, 1, 1);
		put(&mut game, hop_over, Player::Two, TileColor::Orange, 2, 1);
		game.set_current(Player::One);
		let dests = game.destinations(from);
		assert!(dests.contains(&land), "d=1 hop over heavier/equal tile to {land}, got {dests:?}");

		game.clear_tile(hop_over);
		put(&mut game, hop_over, Player::Two, TileColor::Orange, 1, 1);
		game.clear_tile(from);
		put(&mut game, from, Player::One, TileColor::Green, 3, 1);
		game.set_current(Player::One);
		let dests = game.destinations(from);
		assert!(!dests.contains(&land), "cannot hop a lighter tile");
	}

	#[test]
	fn hop_along_a_run_of_equal_or_greater_weight() {
		let mut rules = RuleSet::default();
		rules.color_restricted_hops = false;
		let mut game = Game::empty(rules);
		let b = game.board().clone();
		let circle = 0u8;
		let cycle = b.circle_cycle(circle).to_vec();
		let from = cycle[0];
		let a = cycle[1];
		let c = cycle[2];
		let land = cycle[3];
		put(&mut game, from, Player::One, TileColor::Blue, 2, 1);
		put(&mut game, a, Player::Two, TileColor::Green, 2, 1);
		put(&mut game, c, Player::One, TileColor::Orange, 3, 1);
		game.set_current(Player::One);
		let dests = game.destinations(from);
		assert!(dests.contains(&land), "hop the two-tile run to {land}");
	}

	#[test]
	fn add_only_on_empty_spawns_and_suicidal_add_is_legal() {
		let mut game = Game::empty(RuleSet::default());
		let spawn = game.board().spawns()[0];
		let nbs = game.board().neighbors(spawn).to_vec();
		put(&mut game, nbs[0], Player::Two, TileColor::Orange, 1, 1);
		put(&mut game, nbs[1], Player::Two, TileColor::Orange, 1, 1);
		let tile = green(&mut game, Player::One, 1, 1);
		assert!(game.would_overwhelm_add(tile, spawn));
		game.set_bench(Player::One, vec![tile]);
		game.set_current(Player::One);
		let outcome = game.apply(Action::Add { bench_index: 0, node: spawn }).unwrap();
		assert!(outcome.removals.iter().any(|r| r.node == spawn));
		assert!(game.tile_at(spawn).is_none());
	}

	#[test]
	fn capture_draws_on_move_not_on_add() {
		let mut game = Game::empty(RuleSet::default());
		let spawn = game.board().spawns()[0];
		let nbs = game.board().neighbors(spawn).to_vec();
		put(&mut game, nbs[0], Player::Two, TileColor::Orange, 1, 1);
		put(&mut game, nbs[1], Player::One, TileColor::Blue, 1, 1);
		let extra = game.alloc_tile(Player::One, TileColor::Green, 3, 3);
		game.set_bag(Player::One, vec![extra]);
		{
			let _t = take(&mut game, Player::One, TileColor::Green, 1, 1);
			game.set_bench(Player::One, vec![_t]);
		};
		game.set_current(Player::One);
		let empty = game
			.board()
			.spawns()
			.iter()
			.copied()
			.find(|&s| game.tile_at(s).is_none())
			.unwrap();
		let bag_before = game.bag_len(Player::One);
		let outcome = game.apply(Action::Add { bench_index: 0, node: empty }).unwrap();
		assert_eq!(game.bag_len(Player::One), bag_before, "ADD captures do not draw");
		assert_eq!(outcome.draws, 0);
	}

	#[test]
	fn move_capture_draws_one_per_opponent_tile() {
		let mut game = Game::empty(RuleSet::default());
		let spawn = game.board().spawns()[0];
		let nbs = game.board().neighbors(spawn).to_vec();
		assert!(nbs.len() >= 3);
		let land = nbs[0];
		let from = *game
			.board()
			.neighbors(land)
			.iter()
			.find(|n| **n != spawn && !nbs.contains(n))
			.unwrap_or(&nbs[2]);
		put(&mut game, spawn, Player::Two, TileColor::Green, 2, 1);
		put(&mut game, nbs[1], Player::One, TileColor::Orange, 1, 1);
		put(&mut game, from, Player::One, TileColor::Blue, 3, 1);
		assert!(game.destinations(from).contains(&land), "w3 d=1 should step to empty neighbor");
		let extra = game.alloc_tile(Player::One, TileColor::Orange, 2, 2);
		game.set_bag(Player::One, vec![extra]);
		game.set_current(Player::One);
		let bag_before = game.bag_len(Player::One);
		let bench_before = game.bench(Player::One).len();
		let (m, n) = {
			let mut preview = game.clone();
			preview.tiles[from as usize] = None;
			preview.tiles[land as usize] = game.tile_at(from);
			preview.tile_score(spawn).unwrap()
		};
		assert!(n >= m, "landing a different color should overwhelm the double green");
		let outcome = game.apply(Action::Move { from, to: land }).unwrap();
		assert!(
			outcome.removals.iter().any(|r| r.node == spawn && r.tile.owner == Player::Two),
			"opponent tile should be removed: {:?}",
			outcome.removals
		);
		assert_eq!(outcome.draws, 1);
		assert_eq!(game.bag_len(Player::One), bag_before - 1);
		assert_eq!(game.bench(Player::One).len(), bench_before + 1);
	}

	#[test]
	fn win_when_opponent_has_empty_board_and_empty_bench() {
		let mut game = Game::empty(RuleSet::default());
		let spawn = game.board().spawns()[0];
		put(&mut game, spawn, Player::One, TileColor::Green, 3, 1);
		{
			let _t = take(&mut game, Player::One, TileColor::Blue, 1, 1);
			game.set_bench(Player::One, vec![_t]);
		};
		game.set_current(Player::Two);
		game.set_bench(Player::Two, vec![]);
		assert!(!game.is_alive(Player::Two));
		game.set_current(Player::One);
		let empty = game.board().spawns()[1];
		let outcome = game.apply(Action::Add { bench_index: 0, node: empty });
		assert!(outcome.is_ok());
		assert_eq!(
			game.status(),
			GameStatus::Won { winner: Player::One, reason: EndReason::Elimination }
		);
	}

	#[test]
	fn remaining_bag_does_not_keep_a_player_alive() {
		let mut game = Game::empty(RuleSet::default());
		{
			let _t = take(&mut game, Player::Two, TileColor::Orange, 1, 1);
			game.set_bag(Player::Two, vec![_t]);
		};
		game.set_bench(Player::Two, vec![]);
		assert!(!game.is_alive(Player::Two));
	}

	fn fill_board_all_green(game: &mut Game) {
		let n = game.board().node_count();
		for id in 0..n as NodeId {
			let owner = if id % 2 == 0 { Player::One } else { Player::Two };
			put(game, id, owner, TileColor::Green, 3, 1);
		}
	}

	#[test]
	fn double_pass_is_a_draw() {
		let mut rules = RuleSet::default();
		rules.no_moves_loses = false;
		let mut game = Game::empty(rules);
		fill_board_all_green(&mut game);
		assert!(!game.has_add_or_move());
		game.apply(Action::Pass).unwrap();
		game.apply(Action::Pass).unwrap();
		assert_eq!(game.status(), GameStatus::Draw { reason: EndReason::DoublePass });
	}

	#[test]
	fn turn_limit_is_a_draw() {
		let mut rules = RuleSet::default();
		rules.no_moves_loses = false;
		rules.turn_limit = 2;
		let mut game = Game::empty(rules);
		fill_board_all_green(&mut game);
		game.apply(Action::Pass).unwrap();
		assert_eq!(game.status(), GameStatus::InProgress);
		game.apply(Action::Pass).unwrap();
		assert!(matches!(
			game.status(),
			GameStatus::Draw { reason: EndReason::DoublePass }
				| GameStatus::Draw { reason: EndReason::TurnLimit }
		));
	}

	#[test]
	fn turn_limit_without_double_pass() {
		let mut rules = RuleSet::default();
		rules.turn_limit = 1;
		let mut game = Game::empty(rules);
		let spawn = game.board().spawns()[0];
		{
			let _t = take(&mut game, Player::One, TileColor::Green, 3, 1);
			game.set_bench(Player::One, vec![_t]);
		};
		{
			let _t = take(&mut game, Player::Two, TileColor::Orange, 3, 1);
			game.set_bench(Player::Two, vec![_t]);
		};
		game.apply(Action::Add { bench_index: 0, node: spawn }).unwrap();
		assert_eq!(game.status(), GameStatus::Draw { reason: EndReason::TurnLimit });
	}

	#[test]
	fn pass_rejected_when_add_exists() {
		let mut game = Game::empty(RuleSet::default());
		{
			let _t = take(&mut game, Player::One, TileColor::Green, 1, 1);
			game.set_bench(Player::One, vec![_t]);
		};
		assert!(game.apply(Action::Pass).is_err());
	}

	#[test]
	fn new_game_has_legal_adds_on_all_spawns() {
		let game = Game::new(RuleSet::default(), 1);
		let adds = game.legal_adds();
		assert_eq!(adds.len(), 5 * 8);
		assert!(game.legal_moves().is_empty());
	}

	fn inner_ring(game: &Game) -> Vec<NodeId> {
		let b = game.board();
		let r = b.construction_radius();
		let target = 2.0 * r * (3.0 * std::f64::consts::PI / 8.0).cos();
		(0..b.node_count() as NodeId)
			.filter(|&id| (b.radius_of(id) - target).abs() < 0.05 * r)
			.collect()
	}

	#[test]
	fn locked_down_player_loses_instead_of_passing() {
		let mut game = Game::empty(RuleSet::default());
		assert!(game.rules().no_moves_loses);
		fill_board_all_green(&mut game);
		assert!(!game.has_add_or_move());
		assert!(game.legal_actions().is_empty());
		game.set_current(Player::One);
		game.apply(Action::Pass).unwrap();
		assert_eq!(
			game.status(),
			GameStatus::Won { winner: Player::Two, reason: EndReason::LockedOut }
		);
	}

	#[test]
	fn opponent_is_locked_out_immediately_after_turn_switch() {
		let mut rules = RuleSet::default();
		rules.no_moves_loses = true;
		let mut game = Game::empty(rules);
		let spawns = game.board().spawns().to_vec();
		// Fill every node except spawn 0 so P1 can ADD and then P2 has nowhere
		// to ADD and no empty landing for a MOVE.
		for id in 0..game.board().node_count() as NodeId {
			if id == spawns[0] {
				continue;
			}
			let owner = if id == spawns[1] { Player::Two } else { Player::One };
			put(&mut game, id, owner, TileColor::Green, 3, 1);
		}
		{
			let t = take(&mut game, Player::One, TileColor::Green, 3, 1);
			game.set_bench(Player::One, vec![t]);
		}
		game.set_bench(Player::Two, vec![]);
		game.set_current(Player::One);
		assert!(game.has_add_or_move());
		game.apply(Action::Add { bench_index: 0, node: spawns[0] }).unwrap();
		assert_eq!(
			game.status(),
			GameStatus::Won { winner: Player::One, reason: EndReason::LockedOut }
		);
	}

	#[test]
	fn center_hold_wins_at_exactly_n() {
		let mut rules = RuleSet::default();
		rules.center_hold_win = true;
		rules.center_hold_n = 2;
		let mut game = Game::empty(rules);
		let center = game.board().center();
		let inner = inner_ring(&game);
		let from = inner[0];
		put(&mut game, from, Player::One, TileColor::Blue, 3, 1);
		{
			let t = take(&mut game, Player::One, TileColor::Green, 3, 1);
			game.set_bench(Player::One, vec![t]);
		}
		{
			let t = take(&mut game, Player::Two, TileColor::Orange, 3, 1);
			game.set_bench(Player::Two, vec![t]);
		}
		game.set_current(Player::One);
		assert!(game.destinations(from).contains(&center));
		game.apply(Action::Move { from, to: center }).unwrap();
		assert_eq!(game.center_hold(Player::One), 1);
		assert_eq!(game.status(), GameStatus::InProgress);

		let p2_spawn = game
			.board()
			.spawns()
			.iter()
			.copied()
			.find(|&s| game.tile_at(s).is_none())
			.unwrap();
		game.apply(Action::Add { bench_index: 0, node: p2_spawn }).unwrap();
		assert_eq!(game.center_hold(Player::One), 1, "P2's turn must not increment P1");
		assert_eq!(game.center_hold(Player::Two), 0);

		let p1_spawn = game
			.board()
			.spawns()
			.iter()
			.copied()
			.find(|&s| game.tile_at(s).is_none())
			.unwrap();
		game.apply(Action::Add { bench_index: 0, node: p1_spawn }).unwrap();
		assert_eq!(game.center_hold(Player::One), 2);
		assert_eq!(
			game.status(),
			GameStatus::Won { winner: Player::One, reason: EndReason::CenterHold }
		);
	}

	#[test]
	fn center_hold_resets_when_vacated() {
		let mut rules = RuleSet::default();
		rules.center_hold_win = true;
		rules.center_hold_n = 5;
		rules.no_same_piece_twice = SamePieceTwice::Off;
		let mut game = Game::empty(rules);
		let center = game.board().center();
		let inner = inner_ring(&game);
		let from = inner[0];
		put(&mut game, from, Player::One, TileColor::Blue, 3, 1);
		put(&mut game, inner[1], Player::One, TileColor::Green, 3, 1);
		{
			let t = take(&mut game, Player::Two, TileColor::Orange, 3, 1);
			game.set_bench(Player::Two, vec![t]);
		}
		game.set_current(Player::One);
		game.apply(Action::Move { from, to: center }).unwrap();
		assert_eq!(game.center_hold(Player::One), 1);
		let p2_spawn = game.board().spawns()[0];
		game.apply(Action::Add { bench_index: 0, node: p2_spawn }).unwrap();
		let leave = game.destinations(center);
		assert!(!leave.is_empty(), "center piece must be able to step off");
		game.apply(Action::Move { from: center, to: leave[0] }).unwrap();
		assert_eq!(game.center_hold(Player::One), 0);
		assert_eq!(game.status(), GameStatus::InProgress);
	}

	#[test]
	fn no_same_piece_twice_blocks_repeat_unless_capture() {
		let mut rules = RuleSet::default();
		rules.no_same_piece_twice = SamePieceTwice::Always;
		let mut game = Game::empty(rules);
		let inner = inner_ring(&game);
		let a = inner[0];
		let center = game.board().center();
		put(&mut game, a, Player::One, TileColor::Blue, 3, 1);
		{
			let t = take(&mut game, Player::Two, TileColor::Orange, 3, 1);
			game.set_bench(Player::Two, vec![t]);
		}
		game.set_current(Player::One);
		assert!(game.destinations(a).contains(&center));
		let tile_id = game.tile_at(a).unwrap().id;
		game.apply(Action::Move { from: a, to: center }).unwrap();
		assert_eq!(game.last_moved_tile(Player::One), Some(tile_id));

		let p2_spawn = game.board().spawns()[0];
		game.apply(Action::Add { bench_index: 0, node: p2_spawn }).unwrap();

		let back = a;
		assert!(
			game.destinations(center).contains(&back),
			"path back to the inner node should exist"
		);
		assert!(
			!game.legal_destinations(center).contains(&back),
			"repeat MOVE without a capture must be absent from legal destinations"
		);
		assert!(
			game.apply(Action::Move { from: center, to: back }).is_err(),
			"repeat MOVE without a capture must be rejected"
		);
	}

	#[test]
	fn no_same_piece_twice_allows_repeat_that_captures() {
		let mut rules = RuleSet::default();
		rules.no_same_piece_twice = SamePieceTwice::Always;
		rules.no_moves_loses = false;
		let mut game = Game::empty(rules);
		let spawn = game.board().spawns()[0];
		let nbs = game.board().neighbors(spawn).to_vec();
		assert!(nbs.len() >= 3);
		let land = nbs[0];
		let from = *game
			.board()
			.neighbors(land)
			.iter()
			.find(|n| **n != spawn && !nbs.contains(n))
			.expect("land has a neighbor away from spawn");
		put(&mut game, from, Player::One, TileColor::Blue, 3, 1);
		{
			let t = take(&mut game, Player::Two, TileColor::Orange, 3, 1);
			game.set_bench(Player::Two, vec![t]);
		}
		game.set_current(Player::One);
		let parking = game
			.destinations(from)
			.into_iter()
			.find(|&d| d != land)
			.expect("a non-capturing first step");
		let tile_id = game.tile_at(from).unwrap().id;
		game.apply(Action::Move { from, to: parking }).unwrap();
		assert_eq!(game.last_moved_tile(Player::One), Some(tile_id));
		let p2_spawn = game
			.board()
			.spawns()
			.iter()
			.copied()
			.find(|&s| s != spawn && game.tile_at(s).is_none())
			.unwrap();
		game.apply(Action::Add { bench_index: 0, node: p2_spawn }).unwrap();

		let mover = game.clear_tile(parking).expect("mover still on the board");
		assert_eq!(mover.id, tile_id);
		game.place_tile(from, mover);
		put(&mut game, spawn, Player::Two, TileColor::Green, 2, 1);
		put(&mut game, nbs[1], Player::One, TileColor::Orange, 1, 1);
		if game.tile_at(land).is_some() {
			let _ = game.clear_tile(land);
		}
		game.set_current(Player::One);
		assert!(game.destinations(from).contains(&land), "w3 d=1 should step to {land}");
		assert!(
			game.move_removes_opponent(from, land),
			"landing a different color should overwhelm the double green"
		);
		assert!(
			game.legal_destinations(from).contains(&land),
			"capture exception must keep the repeat MOVE legal"
		);
		let outcome = game.apply(Action::Move { from, to: land }).unwrap();
		assert!(
			outcome.removals.iter().any(|r| r.node == spawn && r.tile.owner == Player::Two),
			"capture exception must fire: {:?}",
			outcome.removals
		);
	}

	#[test]
	fn no_same_piece_twice_when_bench_empty_only() {
		let mut rules = RuleSet::default();
		rules.no_same_piece_twice = SamePieceTwice::WhenBenchEmpty;
		let mut game = Game::empty(rules);
		let inner = inner_ring(&game);
		let a = inner[0];
		let center = game.board().center();
		put(&mut game, a, Player::One, TileColor::Blue, 3, 1);
		{
			let t = take(&mut game, Player::One, TileColor::Green, 3, 1);
			game.set_bench(Player::One, vec![t]);
		}
		{
			let t = take(&mut game, Player::Two, TileColor::Orange, 3, 1);
			game.set_bench(Player::Two, vec![t]);
		}
		game.set_current(Player::One);
		game.apply(Action::Move { from: a, to: center }).unwrap();
		let p2_spawn = game.board().spawns()[0];
		game.apply(Action::Add { bench_index: 0, node: p2_spawn }).unwrap();
		assert!(!game.bench(Player::One).is_empty());
		assert!(game.legal_destinations(center).contains(&a));
		game.apply(Action::Move { from: center, to: a }).unwrap();
	}

	#[test]
	fn color_restricted_hops_only_lighter_same_color() {
		assert_eq!(RuleSet::default().hop_weight_cmp, HopWeightCmp::Le);
		let mut rules = RuleSet::default();
		rules.color_restricted_hops = true;
		rules.hop_weight_cmp = HopWeightCmp::Lt;
		let mut game = Game::empty(rules);
		let b = game.board().clone();
		let cycle = b.circle_cycle(0).to_vec();
		let (from, hop_over, land) = (cycle[0], cycle[1], cycle[2]);

		put(&mut game, from, Player::One, TileColor::Green, 3, 1);
		put(&mut game, hop_over, Player::Two, TileColor::Orange, 3, 1);
		game.set_current(Player::One);
		assert!(
			!game.destinations(from).contains(&land),
			"other-color occupant cannot be hopped when color_restricted_hops is on"
		);

		game.clear_tile(hop_over);
		put(&mut game, hop_over, Player::Two, TileColor::Green, 3, 1);
		assert!(
			!game.destinations(from).contains(&land),
			"equal-weight same color is not < under hop_weight_cmp=Lt"
		);

		game.clear_tile(hop_over);
		put(&mut game, hop_over, Player::Two, TileColor::Green, 1, 1);
		assert!(
			game.destinations(from).contains(&land),
			"lighter same-color occupant is legal: 'if the weight in your color is less than, you can traverse'"
		);

		let mut le_rules = rules;
		le_rules.hop_weight_cmp = HopWeightCmp::Le;
		let mut le_game = Game::empty(le_rules);
		put(&mut le_game, from, Player::One, TileColor::Green, 3, 1);
		put(&mut le_game, hop_over, Player::Two, TileColor::Green, 3, 1);
		le_game.set_current(Player::One);
		assert!(
			le_game.destinations(from).contains(&land),
			"hop_weight_cmp=Le allows equal-weight same color"
		);
	}

	#[test]
	fn edge_revisit_path_legal_only_under_no_edge_revisit() {
		fn dests(constraint: PathConstraint) -> Vec<NodeId> {
			let mut rules = RuleSet::default();
			rules.path_constraint = constraint;
			let mut game = Game::empty(rules);
			let inner = inner_ring(&game);
			let start = inner[0];
			let nbs = game.board().neighbors(start).to_vec();
			let spawn =
				*nbs.iter().find(|n| game.board().is_spawn(**n)).expect("inner has a spawn");
			for id in 0..game.board().node_count() as NodeId {
				if id == start || id == game.board().center() || id == spawn {
					continue;
				}
				put(&mut game, id, Player::Two, TileColor::Orange, 1, 1);
			}
			put(&mut game, start, Player::One, TileColor::Blue, 3, 3);
			game.set_current(Player::One);
			game.destinations(start)
		}

		let via_edge = dests(PathConstraint::NoEdgeRevisit);
		let via_node = dests(PathConstraint::NoNodeRevisit);
		let via_both = dests(PathConstraint::Both);
		assert!(
			via_edge.contains(&{
				// spawn neighbor of start; identified inside dests() but recompute
				let game = Game::empty(RuleSet::default());
				let inner = inner_ring(&game);
				let start = inner[0];
				*game
					.board()
					.neighbors(start)
					.iter()
					.find(|n| game.board().is_spawn(**n))
					.unwrap()
			}),
			"NoEdgeRevisit may return through the second center-arc and land on the spawn, got {via_edge:?}"
		);
		let spawn = {
			let game = Game::empty(RuleSet::default());
			let inner = inner_ring(&game);
			let start = inner[0];
			*game
				.board()
				.neighbors(start)
				.iter()
				.find(|n| game.board().is_spawn(**n))
				.unwrap()
		};
		assert!(via_edge.contains(&spawn));
		assert!(
			!via_node.contains(&spawn),
			"NoNodeRevisit cannot revisit the start inner node to reach the spawn in 3 steps, got {via_node:?}"
		);
		assert!(
			!via_both.contains(&spawn),
			"Both is at least as strict as NoNodeRevisit, got {via_both:?}"
		);
	}

	#[test]
	fn center_move_counts_by_path_constraint() {
		fn count(dots: u8, constraint: PathConstraint) -> usize {
			let mut rules = RuleSet::default();
			rules.path_constraint = constraint;
			let mut game = Game::empty(rules);
			let center = game.board().center();
			put(&mut game, center, Player::One, TileColor::Blue, 1, dots);
			game.set_current(Player::One);
			game.destinations(center).len()
		}

		let node_d1 = count(1, PathConstraint::NoNodeRevisit);
		let edge_d1 = count(1, PathConstraint::NoEdgeRevisit);
		let both_d1 = count(1, PathConstraint::Both);
		let node_d2 = count(2, PathConstraint::NoNodeRevisit);
		let edge_d2 = count(2, PathConstraint::NoEdgeRevisit);
		let both_d2 = count(2, PathConstraint::Both);
		let node_d3 = count(3, PathConstraint::NoNodeRevisit);
		let edge_d3 = count(3, PathConstraint::NoEdgeRevisit);
		let both_d3 = count(3, PathConstraint::Both);
		let node_d4 = count(4, PathConstraint::NoNodeRevisit);
		let edge_d4 = count(4, PathConstraint::NoEdgeRevisit);
		let both_d4 = count(4, PathConstraint::Both);

		assert_eq!((node_d1, edge_d1, both_d1), (8, 8, 8), "d=1 from center: 8 inner neighbors");
		assert_eq!((node_d2, edge_d2, both_d2), (8, 8, 8), "d=2 from center: 8 spawns");
		assert_eq!((node_d3, edge_d3, both_d3), (16, 16, 16), "d=3 from center: 8 inner + 8 outer");
		assert_eq!((node_d4, edge_d4, both_d4), (16, 16, 16), "d=4 from center: 16 dests");
		fn inner_count(dots: u8, constraint: PathConstraint) -> usize {
			let mut rules = RuleSet::default();
			rules.path_constraint = constraint;
			let mut game = Game::empty(rules);
			let start = inner_ring(&game)[0];
			put(&mut game, start, Player::One, TileColor::Blue, 1, dots);
			game.set_current(Player::One);
			game.destinations(start).len()
		}
		assert_eq!(inner_count(3, PathConstraint::NoNodeRevisit), 14);
		assert_eq!(inner_count(3, PathConstraint::NoEdgeRevisit), 14);
	}

	#[test]
	fn turn_limit_still_draws_under_new_defaults() {
		let mut rules = RuleSet::default();
		rules.turn_limit = 1;
		let mut game = Game::empty(rules);
		let spawn = game.board().spawns()[0];
		{
			let t = take(&mut game, Player::One, TileColor::Green, 3, 1);
			game.set_bench(Player::One, vec![t]);
		}
		{
			let t = take(&mut game, Player::Two, TileColor::Orange, 3, 1);
			game.set_bench(Player::Two, vec![t]);
		}
		game.apply(Action::Add { bench_index: 0, node: spawn }).unwrap();
		assert_eq!(game.status(), GameStatus::Draw { reason: EndReason::TurnLimit });
	}

	#[test]
	fn kind_index_covers_the_27_kinds() {
		use std::collections::HashSet;
		let mut seen = HashSet::new();
		for color in TileColor::ALL {
			for weight in 1..=3 {
				for dots in 1..=3 {
					let t = Tile { id: 0, owner: Player::One, color, weight, dots };
					assert!(t.kind_index() < KIND_COUNT);
					assert!(seen.insert(t.kind_index()));
				}
			}
		}
		assert_eq!(seen.len(), KIND_COUNT);
	}

	#[test]
	fn shuffle_bags_keeps_the_same_kinds() {
		let mut game = Game::new(RuleSet::default(), 12);
		let snapshot = |g: &Game, p: Player| {
			let mut v: Vec<_> = g.bag(p).iter().map(|t| t.kind_index()).collect();
			v.sort_unstable();
			v
		};
		let before = snapshot(&game, Player::One);
		let mut rng = StdRng::seed_from_u64(99);
		game.shuffle_bags(&mut rng);
		assert_eq!(before, snapshot(&game, Player::One));
	}
}
