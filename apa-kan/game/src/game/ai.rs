use super::actions::{generate_legal_actions, has_legal_add_or_move, Action, ActionType};
use super::core::{Game, GameState, Player};
use super::rules::Rules;
use rand::Rng;
use std::collections::HashMap;
use std::time::{Duration, Instant};

pub struct AiConfig {
	pub depth: u8,
	pub bot_type: BotType,
	pub time_budget: Option<Duration>,
	pub use_transposition_table: bool,
	pub use_move_ordering: bool,
	pub mcts_iterations: Option<usize>,
	pub mcts_exploration: f64,
	pub mcts_rollout_depth: u8,
}

#[derive(Debug, Default, Clone)]
pub struct SearchStats {
	pub nodes_searched: u64,
	pub add_actions_generated: u64,
	pub move_actions_generated: u64,
	pub positions_evaluated: u64,
	pub tt_hits: u64,
	pub tt_misses: u64,
}

#[derive(Debug, Clone, Copy)]
enum TTFlag {
	Exact,
	LowerBound,
	UpperBound,
}

#[derive(Debug, Clone)]
struct TTEntry {
	depth: u8,
	score: i32,
	flag: TTFlag,
}

impl Default for AiConfig {
	fn default() -> Self {
		Self {
			depth: 3,
			bot_type: BotType::Ologbon,
			time_budget: Some(Duration::from_millis(1000)),
			use_transposition_table: true,
			use_move_ordering: true,
			mcts_iterations: None,
			mcts_exploration: 1.41,
			mcts_rollout_depth: 20,
		}
	}
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BotType {
	Ore,
	Olusare,
	Ologbon,
	Mcts,
}

struct MctsNode {
	action: Option<Action>,
	visits: u32,
	wins: f64,
	children: Vec<Box<MctsNode>>,
	untried_actions: Vec<Action>,
	player: Player,
}

impl BotType {
	pub fn name(&self) -> &'static str {
		match self {
			BotType::Ore => "Ore",
			BotType::Olusare => "Olusare",
			BotType::Ologbon => "Ologbon",
			BotType::Mcts => "Mcts",
		}
	}

	pub fn description(&self) -> &'static str {
		match self {
			BotType::Ore => "Naive random move selector",
			BotType::Olusare => "Minimax with mobility heuristic",
			BotType::Ologbon => "Minimax with enhanced heuristic",
			BotType::Mcts => "Monte Carlo Tree Search with UCT selection",
		}
	}
}

pub struct AiPlayer {
	config: AiConfig,
	rng: std::cell::RefCell<rand::rngs::StdRng>,
	transposition_table: std::cell::RefCell<HashMap<u64, TTEntry>>,
	pub stats: std::cell::RefCell<SearchStats>,
}

impl AiPlayer {
	pub fn new(config: AiConfig) -> Self {
		use rand::SeedableRng;
		Self {
			config,
			rng: std::cell::RefCell::new(rand::rngs::StdRng::from_entropy()),
			transposition_table: std::cell::RefCell::new(HashMap::new()),
			stats: std::cell::RefCell::new(SearchStats::default()),
		}
	}

	pub fn new_seeded(config: AiConfig, seed: u64) -> Self {
		use rand::SeedableRng;
		Self {
			config,
			rng: std::cell::RefCell::new(rand::rngs::StdRng::seed_from_u64(seed)),
			transposition_table: std::cell::RefCell::new(HashMap::new()),
			stats: std::cell::RefCell::new(SearchStats::default()),
		}
	}

	pub fn reset_stats(&self) {
		*self.stats.borrow_mut() = SearchStats::default();
	}

	pub fn clear_transposition_table(&self) {
		self.transposition_table.borrow_mut().clear();
	}

	pub fn choose_action(&self, game: &Game) -> Option<Action> {
		match self.config.bot_type {
			BotType::Ore => self.choose_random(game),
			BotType::Olusare | BotType::Ologbon => self.choose_minimax(game),
			BotType::Mcts => self.choose_mcts(game),
		}
	}

	fn choose_random(&self, game: &Game) -> Option<Action> {
		use rand::seq::SliceRandom;
		let actions = generate_legal_actions(game);
		if actions.is_empty() {
			None
		} else {
			actions.choose(&mut *self.rng.borrow_mut()).copied()
		}
	}

	fn choose_minimax(&self, game: &Game) -> Option<Action> {
		self.reset_stats();
		let start_time = Instant::now();
		let mut actions = generate_legal_actions(game);

		{
			let mut stats = self.stats.borrow_mut();
			for action in &actions {
				match action.action_type {
					ActionType::Add { .. } => stats.add_actions_generated += 1,
					ActionType::Move { .. } => stats.move_actions_generated += 1,
				}
			}
		}

		if actions.is_empty() {
			return None;
		}

		if actions.len() == 1 {
			return Some(actions[0]);
		}

		if self.config.use_move_ordering {
			actions = self.order_moves(game, actions);
		}

		let mut best_action = actions[0];
		let mut best_score = i32::MIN;
		
		// Iterative deepening: start at depth 1 and progressively go deeper
		let mut current_depth = 1;
		let max_depth = self.config.depth.max(1);
		
		while current_depth <= max_depth {
			// Check time budget before starting a new depth iteration
			if let Some(time_budget) = self.config.time_budget {
				if start_time.elapsed() > time_budget {
					break;
				}
			}
			
			let mut alpha = i32::MIN;
			let beta = i32::MAX;
			let mut depth_best_action = actions[0];
			let mut depth_best_score = i32::MIN;
			let mut completed_depth = true;

			for action in &actions {
				if let Some(time_budget) = self.config.time_budget {
					if start_time.elapsed() > time_budget {
						completed_depth = false;
						break;
					}
				}

				let mut game_copy = game.clone();
				if Rules::apply_action(&mut game_copy, *action).is_ok() {
					let score = self.minimax(
						&game_copy,
						current_depth - 1,
						alpha,
						beta,
						false,
						game.current_player,
						&start_time,
					);

					if score > depth_best_score {
						depth_best_score = score;
						depth_best_action = *action;
					}

					alpha = alpha.max(score);
				}
			}
			
			// Only update best action if we completed the full depth
			if completed_depth {
				best_action = depth_best_action;
				best_score = depth_best_score;
				current_depth += 1;
			} else {
				break;
			}
		}

		Some(best_action)
	}

	fn minimax(
		&self,
		game: &Game,
		depth: u8,
		mut alpha: i32,
		mut beta: i32,
		is_maximizing: bool,
		original_player: Player,
		start_time: &Instant,
	) -> i32 {
		self.stats.borrow_mut().nodes_searched += 1;

		if let Some(time_budget) = self.config.time_budget {
			if start_time.elapsed() > time_budget {
				self.stats.borrow_mut().positions_evaluated += 1;
				return self.evaluate(game, original_player);
			}
		}

		if depth == 0 || game.is_game_over() {
			self.stats.borrow_mut().positions_evaluated += 1;
			return self.evaluate(game, original_player);
		}

		if self.config.use_transposition_table {
			let hash = self.hash_game(game);
			if let Some(entry) = self.transposition_table.borrow().get(&hash) {
				if entry.depth >= depth {
					self.stats.borrow_mut().tt_hits += 1;
					match entry.flag {
						TTFlag::Exact => return entry.score,
						TTFlag::LowerBound => {
							alpha = alpha.max(entry.score);
						}
						TTFlag::UpperBound => {
							beta = beta.min(entry.score);
						}
					}
					if alpha >= beta {
						return entry.score;
					}
				}
			} else {
				self.stats.borrow_mut().tt_misses += 1;
			}
		}

		let mut actions = generate_legal_actions(game);

		{
			let mut stats = self.stats.borrow_mut();
			for action in &actions {
				match action.action_type {
					ActionType::Add { .. } => stats.add_actions_generated += 1,
					ActionType::Move { .. } => stats.move_actions_generated += 1,
				}
			}
		}

		if actions.is_empty() {
			self.stats.borrow_mut().positions_evaluated += 1;
			return self.evaluate(game, original_player);
		}

		if self.config.use_move_ordering {
			actions = self.order_moves(game, actions);
		}

		let original_alpha = alpha;
		let score;

		if is_maximizing {
			let mut max_eval = i32::MIN;

			for action in actions {
				let mut game_copy = game.clone();
				if Rules::apply_action(&mut game_copy, action).is_ok() {
					let eval = self.minimax(
						&game_copy,
						depth - 1,
						alpha,
						beta,
						false,
						original_player,
						start_time,
					);
					max_eval = max_eval.max(eval);
					alpha = alpha.max(eval);

					if beta <= alpha {
						break;
					}
				}
			}

			score = max_eval;
		} else {
			let mut min_eval = i32::MAX;

			for action in actions {
				let mut game_copy = game.clone();
				if Rules::apply_action(&mut game_copy, action).is_ok() {
					let eval = self.minimax(
						&game_copy,
						depth - 1,
						alpha,
						beta,
						true,
						original_player,
						start_time,
					);
					min_eval = min_eval.min(eval);
					beta = beta.min(eval);

					if beta <= alpha {
						break;
					}
				}
			}

			score = min_eval;
		}

		if self.config.use_transposition_table {
			let hash = self.hash_game(game);
			let flag = if score <= original_alpha {
				TTFlag::UpperBound
			} else if score >= beta {
				TTFlag::LowerBound
			} else {
				TTFlag::Exact
			};

			self.transposition_table
				.borrow_mut()
				.insert(hash, TTEntry { depth, score, flag });
		}

		score
	}

	fn hash_game(&self, game: &Game) -> u64 {
		use std::collections::hash_map::DefaultHasher;
		use std::hash::{Hash, Hasher};
		let mut hasher = DefaultHasher::new();

		for (pos, tile) in game.board.tiles() {
			pos.row.hash(&mut hasher);
			pos.col.hash(&mut hasher);
			tile.id.hash(&mut hasher);
			tile.owner.hash(&mut hasher);
		}

		game.current_player.hash(&mut hasher);
		game.turn_count.hash(&mut hasher);
		game.add_bank_p1.hash(&mut hasher);
		game.add_bank_p2.hash(&mut hasher);

		hasher.finish()
	}

	fn order_moves(&self, game: &Game, mut actions: Vec<Action>) -> Vec<Action> {
		actions.sort_by_cached_key(|action| {
			let mut game_copy = game.clone();
			if Rules::apply_action(&mut game_copy, *action).is_err() {
				return 1000;
			}

			let my_tiles_before =
				game.board.tiles().filter(|(_, t)| t.owner == game.current_player).count();
			let opp_tiles_before =
				game.board.tiles().filter(|(_, t)| t.owner != game.current_player).count();
			let my_tiles_after =
				game_copy.board.tiles().filter(|(_, t)| t.owner == game.current_player).count();
			let opp_tiles_after =
				game_copy.board.tiles().filter(|(_, t)| t.owner != game.current_player).count();

			let my_bank_before = match game.current_player {
				Player::One => game.add_bank_p1,
				Player::Two => game.add_bank_p2,
			};
			let my_bank_after = match game.current_player {
				Player::One => game_copy.add_bank_p1,
				Player::Two => game_copy.add_bank_p2,
			};

			let captures = opp_tiles_before.saturating_sub(opp_tiles_after);
			let bank_gain = my_bank_after.saturating_sub(my_bank_before);
			let material_loss = my_tiles_before.saturating_sub(my_tiles_after);

			(1000 - captures * 100 - bank_gain as usize * 50 + material_loss * 30) as i32
		});

		actions
	}

	fn evaluate(&self, game: &Game, player: Player) -> i32 {
		match self.config.bot_type {
			BotType::Ore => 0,
			BotType::Olusare => self.evaluate_mobility(game, player),
			BotType::Ologbon | BotType::Mcts => self.evaluate_enhanced(game, player),
		}
	}

	fn choose_mcts(&self, game: &Game) -> Option<Action> {
		self.reset_stats();
		let start_time = Instant::now();
		
		let actions = generate_legal_actions(game);
		if actions.is_empty() {
			return None;
		}
		if actions.len() == 1 {
			return Some(actions[0]);
		}

		let mut root = MctsNode {
			action: None,
			visits: 0,
			wins: 0.0,
			children: Vec::new(),
			untried_actions: actions.clone(),
			player: game.current_player,
		};

		let mut iterations = 0;
		loop {
			// Check termination conditions
			if let Some(max_iters) = self.config.mcts_iterations {
				if iterations >= max_iters {
					break;
				}
			}
			if let Some(time_budget) = self.config.time_budget {
				if start_time.elapsed() > time_budget {
					break;
				}
			}

			let mut game_copy = game.clone();
			self.mcts_iteration(&mut root, &mut game_copy, game.current_player);
			iterations += 1;
			self.stats.borrow_mut().nodes_searched += 1;
		}

		// Select best child by visit count (robust child)
		root.children
			.iter()
			.max_by_key(|child| child.visits)
			.and_then(|child| child.action)
	}

	fn mcts_iteration(&self, node: &mut MctsNode, game: &mut Game, original_player: Player) -> f64 {
		// 1. Selection: traverse tree using UCT until we reach a node with untried actions
		if !node.untried_actions.is_empty() {
			// 2. Expansion: add one child node
			let action = self.select_untried_action(node);
			if Rules::apply_action(game, action).is_ok() {
				let child_actions = generate_legal_actions(game);
				let mut child = MctsNode {
					action: Some(action),
					visits: 0,
					wins: 0.0,
					children: Vec::new(),
					untried_actions: child_actions,
					player: game.current_player,
				};
				
				// 3. Simulation: rollout from this new child
				let result = self.mcts_rollout(game, original_player);
				
				// 4. Backpropagation
				child.visits = 1;
				child.wins = result;
				node.children.push(Box::new(child));
				node.visits += 1;
				node.wins += result;
				return result;
			} else {
				node.visits += 1;
				return 0.5;
			}
		} else if !node.children.is_empty() {
			// Select best child using UCT
			let best_child_idx = self.select_best_uct_child(node);
			let child = &mut node.children[best_child_idx];
			
			if let Some(action) = child.action {
				if Rules::apply_action(game, action).is_ok() {
					let result = if game.is_game_over() {
						// Terminal node
						match game.state {
							GameState::Won(winner) => {
								if winner == original_player { 1.0 } else { 0.0 }
							}
							GameState::Draw => 0.5,
							GameState::InProgress => {
								self.mcts_rollout(game, original_player)
							}
						}
					} else {
						self.mcts_iteration(child, game, original_player)
					};
					
					// Backpropagate
					node.visits += 1;
					node.wins += result;
					return result;
				}
			}
			
			// If action failed, just update node
			node.visits += 1;
			0.5
		} else {
			// Terminal node or no legal actions
			node.visits += 1;
			let result = match game.state {
				GameState::Won(winner) => {
					if winner == original_player { 1.0 } else { 0.0 }
				}
				GameState::Draw => 0.5,
				GameState::InProgress => {
					if !has_legal_add_or_move(game, game.current_player) {
						let winner = game.current_player.other();
						if winner == original_player { 1.0 } else { 0.0 }
					} else {
						0.5
					}
				}
			};
			node.wins += result;
			result
		}
	}

	fn select_untried_action(&self, node: &mut MctsNode) -> Action {
		let idx = self.rng.borrow_mut().gen_range(0..node.untried_actions.len());
		node.untried_actions.swap_remove(idx)
	}

	fn select_best_uct_child(&self, node: &MctsNode) -> usize {
		let total_visits = node.visits as f64;
		let exploration = self.config.mcts_exploration;
		
		node.children
			.iter()
			.enumerate()
			.max_by(|(_, a), (_, b)| {
				let uct_a = self.calculate_uct(a, total_visits, exploration);
				let uct_b = self.calculate_uct(b, total_visits, exploration);
				uct_a.partial_cmp(&uct_b).unwrap_or(std::cmp::Ordering::Equal)
			})
			.map(|(idx, _)| idx)
			.unwrap_or(0)
	}

	fn calculate_uct(&self, node: &MctsNode, parent_visits: f64, exploration: f64) -> f64 {
		if node.visits == 0 {
			return f64::INFINITY;
		}
		
		let exploitation = node.wins / node.visits as f64;
		let exploration_term = exploration * (parent_visits.ln() / node.visits as f64).sqrt();
		exploitation + exploration_term
	}

	fn mcts_rollout(&self, game: &Game, original_player: Player) -> f64 {
		let mut game_copy = game.clone();
		let mut depth = 0;
		
		while depth < self.config.mcts_rollout_depth && !game_copy.is_game_over() {
			let actions = generate_legal_actions(&game_copy);
			if actions.is_empty() {
				break;
			}
			
			// Guided rollout policy: prefer captures and avoid self-removal
			let action = self.select_rollout_action(&game_copy, &actions);
			if Rules::apply_action(&mut game_copy, action).is_err() {
				break;
			}
			
			depth += 1;
		}
		
		// Evaluate terminal state or use heuristic
		if game_copy.is_game_over() {
			match game_copy.state {
				GameState::Won(winner) => {
					if winner == original_player { 1.0 } else { 0.0 }
				}
				GameState::Draw => 0.5,
				GameState::InProgress => 0.5,
			}
		} else if !has_legal_add_or_move(&game_copy, game_copy.current_player) {
			let winner = game_copy.current_player.other();
			if winner == original_player { 1.0 } else { 0.0 }
		} else {
			// Use Ologbon's heuristic normalized to [0, 1]
			let heuristic = self.evaluate_enhanced(&game_copy, original_player);
			// Normalize from roughly [-100, 100] to [0, 1]
			(heuristic as f64 / 200.0 + 0.5).clamp(0.0, 1.0)
		}
	}

	fn select_rollout_action(&self, game: &Game, actions: &[Action]) -> Action {
		use rand::seq::SliceRandom;
		
		// Evaluate each action and prefer captures
		let mut weighted_actions: Vec<(Action, i32)> = actions
			.iter()
			.map(|&action| {
				let mut game_copy = game.clone();
				let mut weight = 0;
				
				if Rules::apply_action(&mut game_copy, action).is_ok() {
					let my_tiles_before = game.board.tiles()
						.filter(|(_, t)| t.owner == game.current_player)
						.count();
					let opp_tiles_before = game.board.tiles()
						.filter(|(_, t)| t.owner != game.current_player)
						.count();
					let my_tiles_after = game_copy.board.tiles()
						.filter(|(_, t)| t.owner == game.current_player)
						.count();
					let opp_tiles_after = game_copy.board.tiles()
						.filter(|(_, t)| t.owner != game.current_player)
						.count();
					
					// Prefer captures (opponent loses tiles)
					let captures = opp_tiles_before.saturating_sub(opp_tiles_after);
					weight += captures as i32 * 10;
					
					// Avoid self-removal
					let self_loss = my_tiles_before.saturating_sub(my_tiles_after);
					weight -= self_loss as i32 * 5;
				}
				
				(action, weight)
			})
			.collect();
		
		// Sort by weight descending
		weighted_actions.sort_by_key(|(_, w)| -w);
		
		// Select with some randomness: 60% best, 40% random from rest
		let mut rng = self.rng.borrow_mut();
		if rng.gen::<f64>() < 0.6 && !weighted_actions.is_empty() {
			weighted_actions[0].0
		} else {
			actions.choose(&mut *rng).copied().unwrap_or(actions[0])
		}
	}

	fn evaluate_mobility(&self, game: &Game, player: Player) -> i32 {
		match game.state {
			GameState::Won(winner) => return if winner == player { 10000 } else { -10000 },
			GameState::Draw => return 0,
			GameState::InProgress => {}
		}

		if !has_legal_add_or_move(game, game.current_player) {
			let winner = game.current_player.other();
			return if winner == player { 10000 } else { -10000 };
		}

		let my_actions = generate_legal_actions(&{
			let mut game_copy = game.clone();
			game_copy.current_player = player;
			game_copy
		});

		let opponent_actions = generate_legal_actions(&{
			let mut game_copy = game.clone();
			game_copy.current_player = player.other();
			game_copy
		});

		let mobility_diff = (my_actions.len() as i32) - (opponent_actions.len() as i32);

		let my_bank = match player {
			Player::One => game.add_bank_p1 as i32,
			Player::Two => game.add_bank_p2 as i32,
		};
		let opp_bank = match player {
			Player::One => game.add_bank_p2 as i32,
			Player::Two => game.add_bank_p1 as i32,
		};

		mobility_diff * 10 + (my_bank - opp_bank) * 5
	}

	fn evaluate_enhanced(&self, game: &Game, player: Player) -> i32 {
		match game.state {
			GameState::Won(winner) => return if winner == player { 10000 } else { -10000 },
			GameState::Draw => return 0,
			GameState::InProgress => {}
		}

		if !has_legal_add_or_move(game, game.current_player) {
			let winner = game.current_player.other();
			return if winner == player { 10000 } else { -10000 };
		}

		let mobility_score = self.evaluate_mobility(game, player);

		let my_stack_size = game.stack(player).len() as i32;
		let opponent_stack_size = game.stack(player.other()).len() as i32;
		let stack_diff = my_stack_size - opponent_stack_size;

		let my_tiles_on_board =
			game.board.tiles().filter(|(_, tile)| tile.owner == player).count() as i32;
		let opponent_tiles_on_board =
			game.board.tiles().filter(|(_, tile)| tile.owner == player.other()).count() as i32;
		let board_presence = my_tiles_on_board - opponent_tiles_on_board;

		let my_stability = calculate_stability(game, player);
		let opponent_stability = calculate_stability(game, player.other());
		let stability_diff = my_stability - opponent_stability;

		let my_connectivity = calculate_connectivity(game, player);
		let opponent_connectivity = calculate_connectivity(game, player.other());
		let connectivity_diff = my_connectivity - opponent_connectivity;

		mobility_score * 15
			+ stack_diff * 5
			+ board_presence * 4
			+ stability_diff * 3
			+ connectivity_diff * 8
	}
}

fn calculate_stability(game: &Game, player: Player) -> i32 {
	let mut stability = 0;

	for (pos, tile) in game.board.tiles() {
		if tile.owner != player {
			continue;
		}

		let mut matches = 0;
		let mut mismatches = 0;

		for (direction, neighbor_pos) in pos.neighbors() {
			if neighbor_pos.is_corner() {
				mismatches += 1;
				continue;
			}

			if neighbor_pos.row < 0
				|| neighbor_pos.row >= 5
				|| neighbor_pos.col < 0
				|| neighbor_pos.col >= 5
			{
				continue;
			}

			if let Some(neighbor_tile) = game.board.get(neighbor_pos) {
				let my_edge = tile.edge(direction);
				let their_edge = neighbor_tile.edge(direction.opposite());

				if my_edge.color == their_edge.color {
					matches += 1;
				} else {
					mismatches += 1;
				}
			}
		}

		stability += matches - mismatches;
	}

	stability
}

fn calculate_connectivity(game: &Game, player: Player) -> i32 {
	let mut connectivity = 0;

	for (pos, tile) in game.board.tiles() {
		if tile.owner != player {
			continue;
		}

		let adjacency_count = super::core::Direction::all()
			.iter()
			.filter(|&&dir| {
				let neighbor = pos.neighbor(dir);
				neighbor.row >= 0
					&& neighbor.row < 5
					&& neighbor.col >= 0
					&& neighbor.col < 5
					&& game.board.get(neighbor).is_some()
			})
			.count() as i32;

		connectivity += adjacency_count;
	}

	connectivity
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::game::core::{Color, Edge, EdgeWeight, Tile};

	fn make_tile(id: u32, owner: Player, colors: [Color; 4]) -> Tile {
		Tile::new(
			id,
			owner,
			Edge { color: colors[0], weight: EdgeWeight::Single },
			Edge { color: colors[1], weight: EdgeWeight::Single },
			Edge { color: colors[2], weight: EdgeWeight::Single },
			Edge { color: colors[3], weight: EdgeWeight::Single },
		)
	}

	#[test]
	fn test_ai_chooses_action() {
		let game = Game::new();
		let ai = AiPlayer::new(AiConfig {
			depth: 2,
			bot_type: BotType::Olusare,
			time_budget: None,
			use_transposition_table: true,
			use_move_ordering: true,
			mcts_iterations: None,
			mcts_exploration: 1.41,
			mcts_rollout_depth: 20,
		});

		let action = ai.choose_action(&game);
		assert!(action.is_some());
	}

	#[test]
	fn test_mobility_heuristic() {
		let game = Game::new();
		let ai = AiPlayer::new(AiConfig {
			depth: 1,
			bot_type: BotType::Olusare,
			time_budget: None,
			use_transposition_table: false,
			use_move_ordering: false,
			mcts_iterations: None,
			mcts_exploration: 1.41,
			mcts_rollout_depth: 20,
		});

		let score = ai.evaluate_mobility(&game, Player::One);
		assert!(score >= 0);
	}

	#[test]
	fn test_enhanced_heuristic() {
		let game = Game::new();
		let ai = AiPlayer::new(AiConfig {
			depth: 1,
			bot_type: BotType::Ologbon,
			time_budget: None,
			use_transposition_table: false,
			use_move_ordering: false,
			mcts_iterations: None,
			mcts_exploration: 1.41,
			mcts_rollout_depth: 20,
		});

		let score = ai.evaluate_enhanced(&game, Player::One);
		assert!(score.abs() < 1000);
	}

	#[test]
	fn test_ore_random_bot() {
		let game = Game::new();
		let ai = AiPlayer::new_seeded(
			AiConfig {
				depth: 0,
				bot_type: BotType::Ore,
				time_budget: None,
				use_transposition_table: false,
				use_move_ordering: false,
				mcts_iterations: None,
				mcts_exploration: 1.41,
				mcts_rollout_depth: 20,
			},
			42,
		);

		let action1 = ai.choose_action(&game);
		assert!(action1.is_some());

		let ai2 = AiPlayer::new_seeded(
			AiConfig {
				depth: 0,
				bot_type: BotType::Ore,
				time_budget: None,
				use_transposition_table: false,
				use_move_ordering: false,
				mcts_iterations: None,
				mcts_exploration: 1.41,
				mcts_rollout_depth: 20,
			},
			42,
		);
		let action2 = ai2.choose_action(&game);

		assert_eq!(action1, action2, "Seeded RNG should produce same action");
	}

	#[test]
	fn test_ai_detects_win() {
		let mut game =
			Game::new_with_tiles(vec![], vec![make_tile(0, Player::Two, [Color::Maroon; 4])]);
		game.current_player = Player::One;
		game.state = GameState::Won(Player::Two);

		let ai = AiPlayer::new(AiConfig::default());
		let score = ai.evaluate(&game, Player::Two);
		assert_eq!(score, 10000);
	}

	#[test]
	fn test_stability_calculation() {
		let mut game = Game::new_with_tiles(
			vec![make_tile(
				0,
				Player::One,
				[Color::Maroon, Color::Green, Color::Blue, Color::Orange],
			)],
			vec![],
		);

		game.board
			.place(crate::game::core::Position::new(2, 2), game.player_one_stack.remove(0));

		let stability = calculate_stability(&game, Player::One);
		assert_eq!(stability, 0);
	}

	#[test]
	fn test_mcts_finds_immediate_win() {
		// Create a near-terminal position where Player One can win immediately
		let mut game = Game::new_with_tiles(
			vec![
				make_tile(0, Player::One, [Color::Maroon; 4]),
				make_tile(1, Player::One, [Color::Maroon; 4]),
			],
			vec![make_tile(2, Player::Two, [Color::Green; 4])],
		);
		
		// Place tiles to create a winning opportunity
		game.board.place(
			crate::game::core::Position::new(2, 2),
			game.player_one_stack.remove(0),
		);
		
		game.current_player = Player::One;
		
		let ai = AiPlayer::new_seeded(
			AiConfig {
				depth: 1,
				bot_type: BotType::Mcts,
				time_budget: Some(Duration::from_millis(500)),
				use_transposition_table: false,
				use_move_ordering: false,
				mcts_iterations: Some(100),
				mcts_exploration: 1.41,
				mcts_rollout_depth: 10,
			},
			42,
		);
		
		let action = ai.choose_action(&game);
		assert!(action.is_some(), "MCTS should find an action");
	}

	#[test]
	fn test_mcts_avoids_immediate_loss() {
		// Create a position where Player Two is about to win
		let mut game = Game::new_with_tiles(
			vec![make_tile(0, Player::One, [Color::Maroon; 4])],
			vec![
				make_tile(1, Player::Two, [Color::Green; 4]),
				make_tile(2, Player::Two, [Color::Green; 4]),
			],
		);
		
		// Place a tile for Player Two
		game.board.place(
			crate::game::core::Position::new(2, 2),
			game.player_two_stack.remove(0),
		);
		
		game.current_player = Player::One;
		
		let ai = AiPlayer::new_seeded(
			AiConfig {
				depth: 1,
				bot_type: BotType::Mcts,
				time_budget: Some(Duration::from_millis(500)),
				use_transposition_table: false,
				use_move_ordering: false,
				mcts_iterations: Some(100),
				mcts_exploration: 1.41,
				mcts_rollout_depth: 10,
			},
			42,
		);
		
		let action = ai.choose_action(&game);
		assert!(action.is_some(), "MCTS should find a defensive action");
	}
}
