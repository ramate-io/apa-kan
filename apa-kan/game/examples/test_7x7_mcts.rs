// Quick test to diagnose 7x7 Mcts performance
use apa_kan::game::{AiConfig, AiPlayer, BotType, Game, RuleSet, Rules};
use std::time::{Duration, Instant};

fn main() {
	let time_budget = Duration::from_millis(500);

	println!("Testing Mcts performance on 5x5 vs 7x7");
	println!("==========================================\n");

	// Test 5x5
	println!("5x5 scored:");
	let config = AiConfig {
		depth: 3,
		bot_type: BotType::Mcts,
		time_budget: Some(time_budget),
		use_transposition_table: true,
		use_move_ordering: true,
		mcts_iterations: None,
		mcts_exploration: 1.41,
		mcts_rollout_depth: 20,
	};

	let mut game = Game::new_with_rules(RuleSet::scored());
	let ai = AiPlayer::new(config);
	let mut total_iterations = 0u64;
	let mut move_count = 0;

	for _ in 0..5 {
		if game.is_game_over() || game.turn_count >= 20 {
			break;
		}
		ai.reset_stats();
		if let Some(action) = ai.choose_action(&game) {
			let iters = ai.stats.borrow().nodes_searched;
			total_iterations += iters;
			move_count += 1;
			println!("  Move {}: {} iterations", move_count, iters);
			if Rules::apply_action(&mut game, action).is_err() {
				break;
			}
		}
	}
	println!("  Average: {:.0} iterations/move\n", total_iterations as f64 / move_count as f64);

	// Test 7x7
	println!("7x7 scored:");
	let config2 = AiConfig {
		depth: 3,
		bot_type: BotType::Mcts,
		time_budget: Some(time_budget),
		use_transposition_table: true,
		use_move_ordering: true,
		mcts_iterations: None,
		mcts_exploration: 1.41,
		mcts_rollout_depth: 20,
	};
	let mut game = Game::new_with_rules(RuleSet::scored_7x7());
	let ai = AiPlayer::new(config2);
	let mut total_iterations = 0u64;
	let mut move_count = 0;

	for _ in 0..5 {
		if game.is_game_over() || game.turn_count >= 20 {
			break;
		}
		ai.reset_stats();
		if let Some(action) = ai.choose_action(&game) {
			let iters = ai.stats.borrow().nodes_searched;
			total_iterations += iters;
			move_count += 1;
			println!("  Move {}: {} iterations", move_count, iters);
			if Rules::apply_action(&mut game, action).is_err() {
				break;
			}
		}
	}
	println!("  Average: {:.0} iterations/move", total_iterations as f64 / move_count as f64);
}
