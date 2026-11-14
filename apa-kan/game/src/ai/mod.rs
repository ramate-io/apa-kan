//! Bot players: random, greedy one-ply, ID alpha-beta, and PUCT/RAVE MCTS.

mod eval;
mod mcts;
mod minimax;
mod search_move;
mod zobrist;

use std::time::Duration;

use rand::Rng;

use crate::game::{Action, Game, Player, RuleSet};

pub use eval::{evaluate, terminal_score, WIN};
pub use search_move::legal_search_moves;
pub use zobrist::{hash as visible_hash, occupancy_hash};

/// Mean minimax nodes per move at 1000 ms, single-threaded, release, this VM.
///
/// Measured 2026-11-14 with `apa-kan-bench --calibrate --think-ms 1000 --seed 1`
/// (3 games × 16 moves, minimax vs MCTS): n=24, mean=131781, min=46720,
/// max=248256, mean completed ID depth=3.58. Rounded to 132000.
/// Documented in `apa-kan/game/benchmark_results/README.md`.
pub const CALIBRATED_NODES_PER_MOVE: u64 = 132_000;

/// Mean MCTS iterations per move at 1000 ms, single-threaded, release, this VM.
/// Same calibrate run: n=24, mean=2635, min=1327, max=3961. Rounded to 2600.
pub const CALIBRATED_ITERS_PER_MOVE: u32 = 2_600;

/// How a search is bounded. Work budgets replay identically; time budgets do not.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SearchLimit {
	Time(Duration),
	Work { nodes: u64, iters: u32 },
}

impl SearchLimit {
	pub fn time_ms(ms: u64) -> Self {
		Self::Time(Duration::from_millis(ms))
	}

	pub fn work(nodes: u64, iters: u32) -> Self {
		Self::Work { nodes, iters }
	}

	pub fn calibrated() -> Self {
		Self::Work { nodes: CALIBRATED_NODES_PER_MOVE, iters: CALIBRATED_ITERS_PER_MOVE }
	}

	pub fn is_work(self) -> bool {
		matches!(self, Self::Work { .. })
	}

	pub fn think_ms(self) -> u64 {
		match self {
			Self::Time(d) => d.as_millis() as u64,
			Self::Work { .. } => 0,
		}
	}

	pub fn nodes(self) -> Option<u64> {
		match self {
			Self::Work { nodes, .. } => Some(nodes),
			Self::Time(_) => None,
		}
	}

	pub fn iters(self) -> Option<u32> {
		match self {
			Self::Work { iters, .. } => Some(iters),
			Self::Time(_) => None,
		}
	}

	pub fn label(self) -> String {
		match self {
			Self::Time(d) => format!("think_ms={}", d.as_millis()),
			Self::Work { nodes, iters } => format!("budget=nodes:{nodes},iters:{iters}"),
		}
	}
}

use eval::evaluate as eval_pos;
use search_move::{apply_move, legal_search_moves as search_moves, SearchMove};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Actor {
	Human,
	Random,
	Greedy,
	Minimax,
	MinimaxLegacy,
	Mcts,
}

impl Actor {
	pub fn parse(s: &str) -> Result<Self, String> {
		match s.to_ascii_lowercase().as_str() {
			"human" => Ok(Actor::Human),
			"random" => Ok(Actor::Random),
			"greedy" => Ok(Actor::Greedy),
			"minimax" => Ok(Actor::Minimax),
			"minimax-legacy" | "minimax_legacy" | "old-minimax" => Ok(Actor::MinimaxLegacy),
			"mcts" => Ok(Actor::Mcts),
			other => Err(format!(
					"unknown actor '{other}' (expected human|random|greedy|minimax|minimax-legacy|mcts)"
				)),
		}
	}

	pub fn name(self) -> &'static str {
		match self {
			Actor::Human => "human",
			Actor::Random => "random",
			Actor::Greedy => "greedy",
			Actor::Minimax => "minimax",
			Actor::MinimaxLegacy => "minimax-legacy",
			Actor::Mcts => "mcts",
		}
	}

	pub fn is_human(self) -> bool {
		matches!(self, Actor::Human)
	}

	pub fn thinks(self) -> bool {
		matches!(self, Actor::Minimax | Actor::MinimaxLegacy | Actor::Mcts)
	}
}

#[derive(Clone, Copy, Debug, Default)]
pub struct SearchStats {
	/// Last fully completed iterative-deepening depth (minimax). Per move.
	pub depth: u8,
	/// MCTS iterations this move (not cumulative).
	pub iterations: u32,
	pub nodes: u64,
}

pub struct HistoryTable {
	moves: Vec<i32>,
	adds: Vec<i32>,
}

impl HistoryTable {
	pub fn new() -> Self {
		Self { moves: vec![0; 32 * 32], adds: vec![0; crate::game::KIND_COUNT * 32] }
	}

	pub fn get(&self, mv: SearchMove) -> i32 {
		match mv {
			SearchMove::Move { from, to } => self.moves[from as usize * 32 + to as usize],
			SearchMove::Add { kind, node } => self.adds[kind as usize * 32 + node as usize],
			SearchMove::Pass => 0,
		}
	}

	pub fn bump(&mut self, mv: SearchMove, depth: u8) {
		let inc = i32::from(depth) * i32::from(depth);
		match mv {
			SearchMove::Move { from, to } => {
				let slot = &mut self.moves[from as usize * 32 + to as usize];
				*slot = (*slot + inc).min(20_000);
			}
			SearchMove::Add { kind, node } => {
				let slot = &mut self.adds[kind as usize * 32 + node as usize];
				*slot = (*slot + inc).min(20_000);
			}
			SearchMove::Pass => {}
		}
	}
}

/// Persistent searcher: TT, killers, MCTS tree reuse, repetition list.
pub struct Searcher {
	minimax: minimax::MinimaxState,
	mcts: Option<mcts::MctsTree>,
	played_hashes: Vec<u64>,
	stats: SearchStats,
}

impl Default for Searcher {
	fn default() -> Self {
		Self::new()
	}
}

impl Searcher {
	pub fn new() -> Self {
		Self {
			minimax: minimax::MinimaxState::new(),
			mcts: None,
			played_hashes: Vec::new(),
			stats: SearchStats::default(),
		}
	}

	pub fn stats(&self) -> SearchStats {
		self.stats
	}

	pub fn observe(&mut self, game: &Game) {
		let h = zobrist::occupancy_hash(game);
		if self.played_hashes.last().copied() != Some(h) {
			self.played_hashes.push(h);
		}
	}

	/// Walk trees to `game` (after an action was applied).
	pub fn advance(&mut self, game: &Game) {
		self.observe(game);
		mcts::advance(&mut self.mcts, game);
	}

	pub fn choose(
		&mut self,
		game: &Game,
		actor: Actor,
		rng: &mut impl Rng,
		think: Duration,
	) -> Action {
		self.choose_with(game, actor, rng, SearchLimit::Time(think))
	}

	pub fn choose_with(
		&mut self,
		game: &Game,
		actor: Actor,
		rng: &mut impl Rng,
		limit: SearchLimit,
	) -> Action {
		self.observe(game);
		self.stats = SearchStats::default();
		let legal = game.legal_actions();
		if legal.is_empty() {
			return Action::Pass;
		}
		let action = match actor {
			Actor::Human => legal[0],
			Actor::Random => legal[rng.gen_range(0..legal.len())],
			Actor::Greedy => greedy(game, rng, &self.played_hashes),
			Actor::Minimax | Actor::MinimaxLegacy => {
				self.minimax.legacy = actor == Actor::MinimaxLegacy;
				let (action, stats) =
					minimax::search(&mut self.minimax, game, &self.played_hashes, rng, limit);
				self.stats = stats;
				action
			}
			Actor::Mcts => {
				let (action, stats) = mcts::search(
					&mut self.mcts,
					game,
					&self.played_hashes,
					self.minimax.last_move,
					rng,
					limit,
				);
				self.stats = stats;
				action
			}
		};
		let action = if game.legal_actions().contains(&action) { action } else { legal[0] };
		self.minimax.last_move = match action {
			Action::Move { from, to } => Some(SearchMove::Move { from, to }),
			_ => None,
		};
		action
	}
}

pub fn choose(game: &Game, actor: Actor, rng: &mut impl Rng, think: Duration) -> Action {
	let mut searcher = Searcher::new();
	searcher.choose(game, actor, rng, think)
}

pub fn choose_with(game: &Game, actor: Actor, rng: &mut impl Rng, limit: SearchLimit) -> Action {
	let mut searcher = Searcher::new();
	searcher.choose_with(game, actor, rng, limit)
}

fn greedy(game: &Game, rng: &mut impl Rng, played: &[u64]) -> Action {
	let moves = search_moves(game);
	let me = game.current_player();
	let mut best_score = i32::MIN;
	let mut best: Vec<Action> = Vec::new();
	for mv in moves {
		let mut child = game.clone();
		if apply_move(&mut child, mv).is_err() {
			continue;
		}
		let Some(action) = mv.to_action(game) else {
			continue;
		};
		let repeats = count_repeats(
			played,
			&[zobrist::occupancy_hash(&child)],
			zobrist::occupancy_hash(&child),
		);
		let score = eval_pos(&child, me, repeats);
		if score > best_score {
			best_score = score;
			best.clear();
			best.push(action);
		} else if score == best_score {
			best.push(action);
		}
	}
	if best.is_empty() {
		return game.legal_actions().into_iter().next().unwrap_or(Action::Pass);
	}
	best[rng.gen_range(0..best.len())]
}

pub fn count_repeats(history: &[u64], path: &[u64], hash: u64) -> u16 {
	let n = history.iter().chain(path.iter()).filter(|&&h| h == hash).count();
	n.min(usize::from(u16::MAX)) as u16
}

#[derive(Clone, Debug, Default)]
pub struct BranchingReport {
	pub add_n: u64,
	pub add_sum: u64,
	pub add_max: u32,
	pub move_n: u64,
	pub move_sum: u64,
	pub move_max: u32,
	pub all_n: u64,
	pub all_sum: u64,
	pub all_max: u32,
	pub games: u32,
}

impl BranchingReport {
	pub fn add_avg(&self) -> f64 {
		if self.add_n == 0 {
			0.0
		} else {
			self.add_sum as f64 / self.add_n as f64
		}
	}

	pub fn move_avg(&self) -> f64 {
		if self.move_n == 0 {
			0.0
		} else {
			self.move_sum as f64 / self.move_n as f64
		}
	}

	pub fn all_avg(&self) -> f64 {
		if self.all_n == 0 {
			0.0
		} else {
			self.all_sum as f64 / self.all_n as f64
		}
	}

	fn record(&mut self, game: &Game) {
		let adds = game.legal_adds().len() as u32;
		let moves = game.legal_moves().len() as u32;
		let all = game.legal_actions().len() as u32;
		if adds > 0 {
			self.add_n += 1;
			self.add_sum += u64::from(adds);
			self.add_max = self.add_max.max(adds);
		}
		if moves > 0 {
			self.move_n += 1;
			self.move_sum += u64::from(moves);
			self.move_max = self.move_max.max(moves);
		}
		self.all_n += 1;
		self.all_sum += u64::from(all);
		self.all_max = self.all_max.max(all);
	}
}

/// Average and max legal-action branching over real greedy/random games.
pub fn sample_branching(games: u32, seed: u64, rules: RuleSet) -> BranchingReport {
	sample_branching_on(games, seed, rules, None)
}

pub fn sample_branching_on(
	games: u32,
	seed: u64,
	rules: RuleSet,
	scenario: Option<&crate::scenario::Scenario>,
) -> BranchingReport {
	use rand::rngs::StdRng;
	use rand::SeedableRng;

	let mut report = BranchingReport { games, ..BranchingReport::default() };
	let pairings = [
		(Actor::Greedy, Actor::Random),
		(Actor::Random, Actor::Greedy),
		(Actor::Greedy, Actor::Greedy),
	];
	for n in 0..games.max(1) {
		let (p1, p2) = pairings[n as usize % pairings.len()];
		let seed = seed.wrapping_add(u64::from(n).wrapping_mul(997));
		let mut game = Game::start(rules, seed, scenario).unwrap_or_else(|e| {
			panic!("scenario deal failed: {e}");
		});
		let mut rng = StdRng::seed_from_u64(seed ^ 0xB4A7);
		let mut guard = 0u32;
		while !game.status().is_over() {
			report.record(&game);
			guard += 1;
			if guard > rules.turn_limit.saturating_mul(4) + 8 {
				break;
			}
			let actor = match game.current_player() {
				Player::One => p1,
				Player::Two => p2,
			};
			let action = choose(&game, actor, &mut rng, Duration::from_millis(1));
			if game.apply(action).is_err() {
				break;
			}
		}
	}
	report
}

#[cfg(test)]
mod tests {
	#![allow(clippy::unwrap_used, clippy::expect_used)]

	use super::*;
	use crate::game::RuleSet;
	use rand::rngs::StdRng;
	use rand::SeedableRng;
	use std::time::Duration;

	#[test]
	fn random_plays_a_legal_action() {
		let game = Game::new(RuleSet::default(), 3);
		let mut rng = StdRng::seed_from_u64(3);
		let action = choose(&game, Actor::Random, &mut rng, Duration::from_millis(1));
		let legal = game.legal_actions();
		assert!(legal.contains(&action));
	}

	#[test]
	fn greedy_prefers_something_legal() {
		let game = Game::new(RuleSet::default(), 4);
		let mut rng = StdRng::seed_from_u64(4);
		let action = choose(&game, Actor::Greedy, &mut rng, Duration::from_millis(1));
		assert!(game.legal_actions().contains(&action));
	}

	#[test]
	fn greedy_prefers_immediate_center_hold_win() {
		use crate::game::{Action, Player, TileColor};

		let mut rules = RuleSet::default();
		rules.center_hold_win = true;
		rules.center_hold_n = 1;
		rules.no_same_piece_twice = crate::game::SamePieceTwice::Off;
		let mut game = Game::empty(rules);
		let center = game.board().center();
		let from = game.board().neighbors(center).first().copied().expect("center has neighbors");
		let mover = game.alloc_tile(Player::One, TileColor::Blue, 3, 1);
		game.place_tile(from, mover);
		let opp = game.alloc_tile(Player::Two, TileColor::Orange, 3, 1);
		game.set_bench(Player::Two, vec![opp]);
		game.pad_above_low_value();
		game.set_current(Player::One);
		assert!(game.legal_destinations(from).contains(&center));
		let mut rng = StdRng::seed_from_u64(1);
		let action = choose(&game, Actor::Greedy, &mut rng, Duration::from_millis(1));
		assert_eq!(action, Action::Move { from, to: center });
	}

	#[test]
	fn minimax_takes_immediate_center_hold_win() {
		use crate::game::{Action, Player, TileColor};

		let mut rules = RuleSet::default();
		rules.center_hold_win = true;
		rules.center_hold_n = 1;
		rules.no_same_piece_twice = crate::game::SamePieceTwice::Off;
		let mut game = Game::empty(rules);
		let center = game.board().center();
		let from = game.board().neighbors(center).first().copied().expect("center has neighbors");
		let mover = game.alloc_tile(Player::One, TileColor::Blue, 3, 1);
		game.place_tile(from, mover);
		let opp = game.alloc_tile(Player::Two, TileColor::Orange, 3, 1);
		game.set_bench(Player::Two, vec![opp]);
		game.pad_above_low_value();
		game.set_current(Player::One);
		let mut rng = StdRng::seed_from_u64(1);
		let action = choose(&game, Actor::Minimax, &mut rng, Duration::from_millis(80));
		assert_eq!(action, Action::Move { from, to: center });
	}

	#[test]
	fn minimax_completes_center_hold_instead_of_vacating() {
		use crate::game::{Action, EndReason, GameStatus, Player, TileColor};

		let mut rules = RuleSet::default();
		rules.center_hold_win = true;
		rules.center_hold_n = 5;
		rules.no_same_piece_twice = crate::game::SamePieceTwice::Off;
		let mut game = Game::empty(rules);
		let center = game.board().center();
		let occupant = game.alloc_tile(Player::One, TileColor::Blue, 3, 1);
		game.place_tile(center, occupant);
		let extra = game.alloc_tile(Player::One, TileColor::Green, 2, 1);
		game.set_bench(Player::One, vec![extra]);
		let opp = game.alloc_tile(Player::Two, TileColor::Orange, 3, 1);
		game.set_bench(Player::Two, vec![opp]);
		game.pad_above_low_value();
		game.set_center_hold(Player::One, 4);
		game.set_current(Player::One);
		let mut rng = StdRng::seed_from_u64(1);
		let action = choose(&game, Actor::Minimax, &mut rng, Duration::from_millis(80));
		if let Action::Move { from, .. } = action {
			assert_ne!(from, center, "minimax vacated the center one hold from winning");
		}
		assert!(game.apply(action).is_ok());
		assert_eq!(
			game.status(),
			GameStatus::Won { winner: Player::One, reason: EndReason::CenterHold }
		);
	}

	#[test]
	fn minimax_takes_immediate_control_win() {
		use crate::game::{EndReason, GameStatus, Player, TileColor};

		let mut rules = RuleSet::default();
		rules.center_hold_win = false;
		rules.control_win = true;
		rules.control_n = 5;
		rules.control_hold_turns = 1;
		let mut game = Game::empty(rules);
		let center = game.board().center();
		let inner = game.board().inner_ring().to_vec();
		// Same color: a blue center with four green inner neighbors is overwhelmed.
		let occupant = game.alloc_tile(Player::One, TileColor::Green, 3, 1);
		game.place_tile(center, occupant);
		for &node in inner.iter().take(4) {
			let tile = game.alloc_tile(Player::One, TileColor::Green, 3, 1);
			game.place_tile(node, tile);
		}
		let extra = game.alloc_tile(Player::One, TileColor::Blue, 2, 1);
		game.set_bench(Player::One, vec![extra]);
		let opp = game.alloc_tile(Player::Two, TileColor::Orange, 3, 1);
		game.set_bench(Player::Two, vec![opp]);
		game.set_current(Player::One);
		let mut rng = StdRng::seed_from_u64(1);
		let action = choose(&game, Actor::Minimax, &mut rng, Duration::from_millis(80));
		assert!(game.apply(action).is_ok());
		assert_eq!(
			game.status(),
			GameStatus::Won { winner: Player::One, reason: EndReason::Control }
		);
	}

	#[test]
	fn minimax_completes_control_hold_instead_of_vacating() {
		use crate::game::{EndReason, GameStatus, Player, TileColor};

		let mut rules = RuleSet::default();
		rules.center_hold_win = false;
		rules.control_win = true;
		rules.control_n = 5;
		rules.control_hold_turns = 3;
		rules.no_same_piece_twice = crate::game::SamePieceTwice::Off;
		let mut game = Game::empty(rules);
		let center = game.board().center();
		let inner = game.board().inner_ring().to_vec();
		let occupant = game.alloc_tile(Player::One, TileColor::Green, 3, 1);
		game.place_tile(center, occupant);
		for &node in inner.iter().take(4) {
			let tile = game.alloc_tile(Player::One, TileColor::Green, 3, 1);
			game.place_tile(node, tile);
		}
		let extra = game.alloc_tile(Player::One, TileColor::Blue, 2, 1);
		game.set_bench(Player::One, vec![extra]);
		let opp = game.alloc_tile(Player::Two, TileColor::Orange, 3, 1);
		game.set_bench(Player::Two, vec![opp]);
		game.set_control_hold(Player::One, 2);
		game.set_current(Player::One);
		let mut rng = StdRng::seed_from_u64(1);
		let action = choose(&game, Actor::Minimax, &mut rng, Duration::from_millis(80));
		assert!(game.apply(action).is_ok());
		assert_eq!(
			game.status(),
			GameStatus::Won { winner: Player::One, reason: EndReason::Control }
		);
	}

	#[test]
	fn minimax_takes_immediate_petal_majority_win() {
		use crate::game::{EndReason, GameStatus, Player, TileColor};

		let mut rules = RuleSet::default();
		rules.center_hold_win = false;
		rules.control_win = false;
		rules.petal_majority_win = true;
		rules.petal_majority_n = 1;
		let mut game = Game::empty(rules);
		let inner = game.board().inner_ring().to_vec();
		for &node in inner.iter().take(5) {
			let tile = game.alloc_tile(Player::One, TileColor::Green, 3, 1);
			game.place_tile(node, tile);
		}
		let extra = game.alloc_tile(Player::One, TileColor::Blue, 2, 1);
		game.set_bench(Player::One, vec![extra]);
		let opp = game.alloc_tile(Player::Two, TileColor::Orange, 3, 1);
		game.set_bench(Player::Two, vec![opp]);
		game.pad_above_low_value();
		game.set_current(Player::One);
		let mut rng = StdRng::seed_from_u64(1);
		let action = choose(&game, Actor::Minimax, &mut rng, Duration::from_millis(80));
		assert!(game.apply(action).is_ok());
		assert_eq!(
			game.status(),
			GameStatus::Won { winner: Player::One, reason: EndReason::PetalMajority }
		);
	}

	#[test]
	fn all_bots_can_play_a_short_game() {
		for actor in [Actor::Random, Actor::Greedy, Actor::Minimax, Actor::Mcts] {
			let mut game = Game::new(RuleSet::default(), 9);
			let mut rng = StdRng::seed_from_u64(9);
			let mut searcher = Searcher::new();
			for _ in 0..8 {
				if game.status().is_over() {
					break;
				}
				let action = searcher.choose(&game, actor, &mut rng, Duration::from_millis(12));
				assert!(game.apply(action).is_ok(), "{:?} failed on {:?}", actor, action);
				searcher.advance(&game);
			}
		}
	}

	#[test]
	fn actor_parse_rejects_unknown() {
		assert!(Actor::parse("ologbon").is_err());
		assert_eq!(Actor::parse("MINIMAX").unwrap(), Actor::Minimax);
	}

	#[test]
	fn work_budget_replays_the_same_minimax_move() {
		let game = Game::new(RuleSet::default(), 11);
		let limit = SearchLimit::work(240, 8);
		let mut a = Searcher::new();
		let mut b = Searcher::new();
		let mut rng_a = StdRng::seed_from_u64(11);
		let mut rng_b = StdRng::seed_from_u64(11);
		let ma = a.choose_with(&game, Actor::Minimax, &mut rng_a, limit);
		let mb = b.choose_with(&game, Actor::Minimax, &mut rng_b, limit);
		assert_eq!(ma, mb);
		assert_eq!(a.stats().nodes, b.stats().nodes);
		assert_eq!(a.stats().depth, b.stats().depth);
		assert!(a.stats().nodes <= 240);
	}

	#[test]
	fn work_budget_replays_the_same_mcts_move() {
		let game = Game::new(RuleSet::default(), 12);
		let limit = SearchLimit::work(80, 12);
		let mut a = Searcher::new();
		let mut b = Searcher::new();
		let mut rng_a = StdRng::seed_from_u64(12);
		let mut rng_b = StdRng::seed_from_u64(12);
		let ma = a.choose_with(&game, Actor::Mcts, &mut rng_a, limit);
		let mb = b.choose_with(&game, Actor::Mcts, &mut rng_b, limit);
		assert_eq!(ma, mb);
		assert_eq!(a.stats().iterations, b.stats().iterations);
		assert_eq!(a.stats().iterations, 12);
	}

	#[test]
	fn minimax_reports_completed_depth() {
		let game = Game::new(RuleSet::default(), 2);
		let mut rng = StdRng::seed_from_u64(2);
		let mut searcher = Searcher::new();
		let action = searcher.choose(&game, Actor::Minimax, &mut rng, Duration::from_millis(40));
		assert!(game.legal_actions().contains(&action));
		assert!(searcher.stats().depth >= 1);
	}

	#[test]
	fn mcts_reports_iterations_per_move() {
		let mut game = Game::new(RuleSet::default(), 8);
		let mut rng = StdRng::seed_from_u64(8);
		let mut searcher = Searcher::new();
		let _ = searcher.choose(&game, Actor::Mcts, &mut rng, Duration::from_millis(30));
		let first = searcher.stats().iterations;
		assert!(first >= 1);
		let action = searcher.choose(&game, Actor::Mcts, &mut rng, Duration::from_millis(30));
		assert!(game.apply(action).is_ok());
		searcher.advance(&game);
		let _ = searcher.choose(&game, Actor::Mcts, &mut rng, Duration::from_millis(30));
		let second = searcher.stats().iterations;
		assert!(second >= 1);
		assert!(
			second < first.saturating_mul(50) + 50,
			"iterations should be per-move, not a runaway cumulative ({first} then {second})"
		);
	}

	#[test]
	fn shuffle_bags_does_not_touch_the_public_bench() {
		let mut game = Game::new(RuleSet::default(), 1);
		assert_eq!(
			game.bag_len(Player::One),
			crate::game::BAG_SIZE - crate::game::BAG_SIZE_DEFAULT
		);
		let kinds =
			|g: &Game, p: Player| g.bench(p).iter().map(|t| t.kind_index()).collect::<Vec<_>>();
		let before = kinds(&game, Player::One);
		let mut rng = StdRng::seed_from_u64(99);
		game.shuffle_bags(&mut rng);
		assert_eq!(before, kinds(&game, Player::One));
		assert_eq!(
			game.bag_len(Player::One),
			crate::game::BAG_SIZE - crate::game::BAG_SIZE_DEFAULT
		);
	}

	#[test]
	fn shuffle_bags_preserves_legacy_hidden_remainder() {
		use crate::game::TileColor;
		let mut game = Game::empty(RuleSet::default());
		let leftovers: Vec<_> = (0..6)
			.map(|i| {
				game.alloc_tile(
					Player::One,
					if i % 2 == 0 { TileColor::Green } else { TileColor::Orange },
					1,
					1,
				)
			})
			.collect();
		game.set_bag(Player::One, leftovers);
		let kinds = |g: &Game, p: Player| {
			let mut v: Vec<_> = g.bag(p).iter().map(|t| t.kind_index()).collect();
			v.sort_unstable();
			v
		};
		let before = kinds(&game, Player::One);
		let mut rng = StdRng::seed_from_u64(99);
		game.shuffle_bags(&mut rng);
		assert_eq!(before, kinds(&game, Player::One));
		assert_eq!(game.bag_len(Player::One), 6);
	}

	#[test]
	fn branching_sample_sees_adds() {
		let report = sample_branching(2, 1, RuleSet::default());
		assert!(report.add_n > 0);
		assert!(report.add_max >= 8);
		assert!(report.all_avg() > 0.0);
	}
}
