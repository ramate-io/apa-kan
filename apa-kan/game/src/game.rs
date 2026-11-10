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

impl Tile {
	pub fn kind_label(self) -> String {
		format!("{} w{} d{}", self.color.name(), self.weight, self.dots)
	}
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EndReason {
	Elimination,
	TurnLimit,
	DoublePass,
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
	pub hop_requires_ge_weight: bool,
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
		if acts.is_empty() {
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
			for dest in self.destinations(node) {
				acts.push(Action::Move { from: node, to: dest });
			}
		}
		acts
	}

	pub fn has_add_or_move(&self) -> bool {
		!self.legal_adds().is_empty() || !self.legal_moves().is_empty()
	}

	/// Exact-`dots` destinations for the tile at `from`, with the mover vacated.
	pub fn destinations(&self, from: NodeId) -> Vec<NodeId> {
		let Some(tile) = self.tile_at(from) else {
			return Vec::new();
		};
		let steps = tile.dots.max(1);
		let mut occupancy = self.tiles.clone();
		occupancy[from as usize] = None;
		let mut found = Vec::new();
		let mut stack = vec![PathState { node: from, remaining: steps, visited: 1u32 << from }];
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
					if occupancy[next as usize].is_none() {
						if state.visited & (1u32 << next) == 0 {
							stack.push(PathState {
								node: next,
								remaining: state.remaining - 1,
								visited: state.visited | (1u32 << next),
							});
						}
					} else if let Some((land, hopped_mask)) =
						self.hop_along(&occupancy, circle, state.node, ccw, tile.weight)
					{
						if state.visited & hopped_mask == 0 && state.visited & (1u32 << land) == 0 {
							stack.push(PathState {
								node: land,
								remaining: state.remaining - 1,
								visited: state.visited | hopped_mask | (1u32 << land),
							});
						}
					}
				}
			}
		}
		found.sort();
		found
	}

	fn hop_along(
		&self,
		occupancy: &[Option<Tile>],
		circle: u8,
		from: NodeId,
		ccw: bool,
		mover_weight: u8,
	) -> Option<(NodeId, u32)> {
		let mut n = self.board.next_on_circle(circle, from, ccw)?;
		if occupancy[n as usize].is_none() {
			return None;
		}
		let mut hopped = 0u32;
		let start = n;
		loop {
			let occupant = occupancy[n as usize]?;
			if self.rules.hop_requires_ge_weight && occupant.weight < mover_weight {
				return None;
			}
			hopped |= 1u32 << n;
			n = self.board.next_on_circle(circle, n, ccw)?;
			if n == start || n == from {
				return None;
			}
			if occupancy[n as usize].is_none() {
				return Some((n, hopped));
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
		if self.status == GameStatus::InProgress && self.consecutive_passes >= 2 {
			self.status = GameStatus::Draw { reason: EndReason::DoublePass };
		}

		self.actions_this_turn = self.actions_this_turn.saturating_add(1);
		let cap = self.rules.actions_per_turn.max(1);
		let turn_done = action.is_pass()
			|| self.actions_this_turn >= cap
			|| self.status.is_over()
			|| !self.has_add_or_move();
		if turn_done && !self.status.is_over() {
			self.current = self.current.other();
			self.actions_this_turn = 0;
			self.turn_count = self.turn_count.saturating_add(1);
			if self.turn_count >= self.rules.turn_limit {
				self.status = GameStatus::Draw { reason: EndReason::TurnLimit };
			}
		}

		Ok(ActionOutcome { action, player, removals, draws, log_line })
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
		self.tiles[from as usize] = None;
		self.tiles[to as usize] = Some(tile);
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
	visited: u32,
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
		let mut game = Game::empty(RuleSet::default());
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
		let mut game = Game::empty(RuleSet::default());
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
		let mut game = Game::empty(RuleSet::default());
		fill_board_all_green(&mut game);
		assert!(!game.has_add_or_move());
		game.apply(Action::Pass).unwrap();
		game.apply(Action::Pass).unwrap();
		assert_eq!(game.status(), GameStatus::Draw { reason: EndReason::DoublePass });
	}

	#[test]
	fn turn_limit_is_a_draw() {
		let mut rules = RuleSet::default();
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
}
