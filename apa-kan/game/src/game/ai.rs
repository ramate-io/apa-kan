use super::actions::{generate_legal_actions, has_legal_add_or_move, Action};
use super::core::{Game, GameState, Player};
use super::rules::Rules;
use std::time::{Duration, Instant};

pub struct AiConfig {
	pub depth: u8,
	pub bot_type: BotType,
	pub time_budget: Option<Duration>,
}

impl Default for AiConfig {
	fn default() -> Self {
		Self {
			depth: 3,
			bot_type: BotType::Ologbon,
			time_budget: Some(Duration::from_millis(1000)),
		}
	}
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BotType {
	Ore,
	Olusare,
	Ologbon,
}

impl BotType {
	pub fn name(&self) -> &'static str {
		match self {
			BotType::Ore => "Ore",
			BotType::Olusare => "Olusare",
			BotType::Ologbon => "Ologbon",
		}
	}

	pub fn description(&self) -> &'static str {
		match self {
			BotType::Ore => "Naive random move selector",
			BotType::Olusare => "Minimax with mobility heuristic",
			BotType::Ologbon => "Minimax with enhanced heuristic",
		}
	}
}

pub struct AiPlayer {
	config: AiConfig,
	rng: std::cell::RefCell<rand::rngs::StdRng>,
}

impl AiPlayer {
	pub fn new(config: AiConfig) -> Self {
		use rand::SeedableRng;
		Self { config, rng: std::cell::RefCell::new(rand::rngs::StdRng::from_entropy()) }
	}

	pub fn new_seeded(config: AiConfig, seed: u64) -> Self {
		use rand::SeedableRng;
		Self { config, rng: std::cell::RefCell::new(rand::rngs::StdRng::seed_from_u64(seed)) }
	}

	pub fn choose_action(&self, game: &Game) -> Option<Action> {
		match self.config.bot_type {
			BotType::Ore => self.choose_random(game),
			BotType::Olusare | BotType::Ologbon => self.choose_minimax(game),
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
		let start_time = Instant::now();
		let actions = generate_legal_actions(game);

		if actions.is_empty() {
			return None;
		}

		if actions.len() == 1 {
			return Some(actions[0]);
		}

		let mut best_action = actions[0];
		let mut best_score = i32::MIN;
		let mut alpha = i32::MIN;
		let beta = i32::MAX;

		for action in actions {
			if let Some(time_budget) = self.config.time_budget {
				if start_time.elapsed() > time_budget {
					break;
				}
			}

			let mut game_copy = game.clone();
			if Rules::apply_action(&mut game_copy, action).is_ok() {
				let score = self.minimax(
					&game_copy,
					self.config.depth - 1,
					alpha,
					beta,
					false,
					game.current_player,
					&start_time,
				);

				if score > best_score {
					best_score = score;
					best_action = action;
				}

				alpha = alpha.max(score);
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
		if let Some(time_budget) = self.config.time_budget {
			if start_time.elapsed() > time_budget {
				return self.evaluate(game, original_player);
			}
		}

		if depth == 0 || game.is_game_over() {
			return self.evaluate(game, original_player);
		}

		let actions = generate_legal_actions(game);

		if actions.is_empty() {
			return self.evaluate(game, original_player);
		}

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

			max_eval
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

			min_eval
		}
	}

	fn evaluate(&self, game: &Game, player: Player) -> i32 {
		match self.config.bot_type {
			BotType::Ore => 0,
			BotType::Olusare => self.evaluate_mobility(game, player),
			BotType::Ologbon => self.evaluate_enhanced(game, player),
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
		let ai =
			AiPlayer::new(AiConfig { depth: 2, bot_type: BotType::Olusare, time_budget: None });

		let action = ai.choose_action(&game);
		assert!(action.is_some());
	}

	#[test]
	fn test_mobility_heuristic() {
		let game = Game::new();
		let ai =
			AiPlayer::new(AiConfig { depth: 1, bot_type: BotType::Olusare, time_budget: None });

		let score = ai.evaluate_mobility(&game, Player::One);
		assert!(score >= 0);
	}

	#[test]
	fn test_enhanced_heuristic() {
		let game = Game::new();
		let ai =
			AiPlayer::new(AiConfig { depth: 1, bot_type: BotType::Ologbon, time_budget: None });

		let score = ai.evaluate_enhanced(&game, Player::One);
		assert!(score.abs() < 1000);
	}

	#[test]
	fn test_ore_random_bot() {
		use rand::SeedableRng;
		let game = Game::new();
		let ai = AiPlayer::new_seeded(
			AiConfig { depth: 0, bot_type: BotType::Ore, time_budget: None },
			42,
		);

		let action1 = ai.choose_action(&game);
		assert!(action1.is_some());

		let ai2 = AiPlayer::new_seeded(
			AiConfig { depth: 0, bot_type: BotType::Ore, time_budget: None },
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
}
