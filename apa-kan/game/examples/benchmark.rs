#![allow(dead_code)]

use apa_kan::game::{
	Action, ActionType, AiConfig, AiPlayer, BotType, Game, GameState, Player, Rules,
};
use std::time::Instant;

#[derive(Debug, Clone, Copy, Default)]
struct ActionCounts {
	adds: usize,
	moves: usize,
	moves_with_rotation: usize,
}

impl ActionCounts {
	fn total(&self) -> usize {
		self.adds + self.moves
	}

	fn add_action(&mut self, action: Action) {
		match action.action_type {
			ActionType::Add { .. } => self.adds += 1,
			ActionType::Move { rotation_count, .. } => {
				self.moves += 1;
				if rotation_count > 0 {
					self.moves_with_rotation += 1;
				}
			}
			ActionType::Pass => {}
		}
	}
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum EndReason {
	NoLegalMoves,
	RepetitionDraw,
	TurnLimitDraw,
}

#[derive(Debug, Clone)]
struct MatchResult {
	winner: Option<Player>,
	turns: usize,
	reason: EndReason,
	p1_actions: ActionCounts,
	p2_actions: ActionCounts,
}

#[derive(Debug)]
struct PairingStats {
	p1_name: String,
	p2_name: String,
	p1_wins: usize,
	p2_wins: usize,
	draws: usize,
	total_games: usize,
	total_turns: usize,
	no_legal_moves_count: usize,
	repetition_draw_count: usize,
	turn_limit_draw_count: usize,
	p1_win_actions: Vec<ActionCounts>,
	p2_win_actions: Vec<ActionCounts>,
	p1_actions_all: Vec<ActionCounts>,
	p2_actions_all: Vec<ActionCounts>,
}

impl PairingStats {
	fn new(p1_name: &str, p2_name: &str) -> Self {
		Self {
			p1_name: p1_name.to_string(),
			p2_name: p2_name.to_string(),
			p1_wins: 0,
			p2_wins: 0,
			draws: 0,
			total_games: 0,
			total_turns: 0,
			no_legal_moves_count: 0,
			repetition_draw_count: 0,
			turn_limit_draw_count: 0,
			p1_win_actions: Vec::new(),
			p2_win_actions: Vec::new(),
			p1_actions_all: Vec::new(),
			p2_actions_all: Vec::new(),
		}
	}

	fn p1_win_rate(&self) -> f64 {
		if self.total_games == 0 {
			0.0
		} else {
			self.p1_wins as f64 / self.total_games as f64
		}
	}

	fn avg_turns(&self) -> f64 {
		if self.total_games == 0 {
			0.0
		} else {
			self.total_turns as f64 / self.total_games as f64
		}
	}
}

fn play_game(p1: &AiPlayer, p2: &AiPlayer, _tile_seed: u64, max_turns: usize) -> MatchResult {
	let mut game = Game::new(); // Will add seed support later if needed
	let mut p1_actions = ActionCounts::default();
	let mut p2_actions = ActionCounts::default();

	while game.turn_count < max_turns {
		let current_player = game.current_player;
		let ai = if current_player == Player::One { p1 } else { p2 };

		if let Some(action) = ai.choose_action(&game) {
			if current_player == Player::One {
				p1_actions.add_action(action);
			} else {
				p2_actions.add_action(action);
			}

			if Rules::apply_action(&mut game, action).is_err() {
				break;
			}
		} else {
			let reason = EndReason::NoLegalMoves;
			return MatchResult {
				winner: Some(current_player.other()),
				turns: game.turn_count,
				reason,
				p1_actions,
				p2_actions,
			};
		}

		match game.state {
			GameState::Won(winner) => {
				return MatchResult {
					winner: Some(winner),
					turns: game.turn_count,
					reason: EndReason::NoLegalMoves,
					p1_actions,
					p2_actions,
				};
			}
			GameState::Draw => {
				let reason = if game.turn_count >= 100 {
					EndReason::TurnLimitDraw
				} else {
					EndReason::RepetitionDraw
				};
				return MatchResult {
					winner: None,
					turns: game.turn_count,
					reason,
					p1_actions,
					p2_actions,
				};
			}
			GameState::InProgress => {}
		}
	}

	MatchResult {
		winner: None,
		turns: game.turn_count,
		reason: EndReason::TurnLimitDraw,
		p1_actions,
		p2_actions,
	}
}

fn run_pairing(
	bot1_type: BotType,
	bot2_type: BotType,
	depth: u8,
	games_per_side: usize,
	base_seed: u64,
) -> PairingStats {
	let mut stats = PairingStats::new(bot1_type.name(), bot2_type.name());

	println!(
		"Running {} vs {} ({} games per side, depth {})...",
		bot1_type.name(),
		bot2_type.name(),
		games_per_side,
		depth
	);

	let start = Instant::now();

	for i in 0..games_per_side {
		let tile_seed = base_seed.wrapping_mul(1000).wrapping_add(i as u64);
		let ai_seed = base_seed.wrapping_mul(2000).wrapping_add(i as u64);

		let p1 = AiPlayer::new_seeded(
			AiConfig {
				depth,
				bot_type: bot1_type,
				time_budget: None,
				use_transposition_table: true,
				use_move_ordering: true,
				mcts_iterations: None,
				mcts_exploration: 1.41,
				mcts_rollout_depth: 20,
			},
			ai_seed,
		);
		let p2 = AiPlayer::new_seeded(
			AiConfig {
				depth,
				bot_type: bot2_type,
				time_budget: None,
				use_transposition_table: true,
				use_move_ordering: true,
				mcts_iterations: None,
				mcts_exploration: 1.41,
				mcts_rollout_depth: 20,
			},
			ai_seed.wrapping_add(1),
		);

		let result = play_game(&p1, &p2, tile_seed, 100);

		stats.total_games += 1;
		stats.total_turns += result.turns;
		match result.reason {
			EndReason::NoLegalMoves => stats.no_legal_moves_count += 1,
			EndReason::RepetitionDraw => stats.repetition_draw_count += 1,
			EndReason::TurnLimitDraw => stats.turn_limit_draw_count += 1,
		}

		stats.p1_actions_all.push(result.p1_actions);
		stats.p2_actions_all.push(result.p2_actions);

		match result.winner {
			Some(Player::One) => {
				stats.p1_wins += 1;
				stats.p1_win_actions.push(result.p1_actions);
			}
			Some(Player::Two) => {
				stats.p2_wins += 1;
				stats.p2_win_actions.push(result.p2_actions);
			}
			None => stats.draws += 1,
		}
	}

	for i in 0..games_per_side {
		let tile_seed = base_seed.wrapping_mul(3000).wrapping_add(i as u64);
		let ai_seed = base_seed.wrapping_mul(4000).wrapping_add(i as u64);

		let p1 = AiPlayer::new_seeded(
			AiConfig {
				depth,
				bot_type: bot2_type,
				time_budget: None,
				use_transposition_table: true,
				use_move_ordering: true,
				mcts_iterations: None,
				mcts_exploration: 1.41,
				mcts_rollout_depth: 20,
			},
			ai_seed,
		);
		let p2 = AiPlayer::new_seeded(
			AiConfig {
				depth,
				bot_type: bot1_type,
				time_budget: None,
				use_transposition_table: true,
				use_move_ordering: true,
				mcts_iterations: None,
				mcts_exploration: 1.41,
				mcts_rollout_depth: 20,
			},
			ai_seed.wrapping_add(1),
		);

		let result = play_game(&p1, &p2, tile_seed, 100);

		stats.total_games += 1;
		stats.total_turns += result.turns;
		match result.reason {
			EndReason::NoLegalMoves => stats.no_legal_moves_count += 1,
			EndReason::RepetitionDraw => stats.repetition_draw_count += 1,
			EndReason::TurnLimitDraw => stats.turn_limit_draw_count += 1,
		}

		stats.p1_actions_all.push(result.p2_actions);
		stats.p2_actions_all.push(result.p1_actions);

		match result.winner {
			Some(Player::One) => {
				stats.p2_wins += 1;
				stats.p2_win_actions.push(result.p2_actions);
			}
			Some(Player::Two) => {
				stats.p1_wins += 1;
				stats.p1_win_actions.push(result.p1_actions);
			}
			None => stats.draws += 1,
		}
	}

	let elapsed = start.elapsed();
	println!(
		"  Completed in {:.2}s ({} games, avg {:.1} turns)",
		elapsed.as_secs_f64(),
		stats.total_games,
		stats.avg_turns()
	);

	stats
}

fn calculate_stats(values: &[usize]) -> (f64, f64, usize, usize) {
	if values.is_empty() {
		return (0.0, 0.0, 0, 0);
	}
	let mut sorted = values.to_vec();
	sorted.sort_unstable();
	let mean = values.iter().sum::<usize>() as f64 / values.len() as f64;
	let median = sorted[sorted.len() / 2] as f64;
	let min = sorted[0];
	let max = sorted[sorted.len() - 1];
	(mean, median, min, max)
}

fn print_action_stats(name: &str, actions: &[ActionCounts]) {
	if actions.is_empty() {
		println!("  {} (n=0): No data", name);
		return;
	}

	let adds: Vec<usize> = actions.iter().map(|a| a.adds).collect();
	let moves: Vec<usize> = actions.iter().map(|a| a.moves).collect();
	let moves_rot: Vec<usize> = actions.iter().map(|a| a.moves_with_rotation).collect();
	let totals: Vec<usize> = actions.iter().map(|a| a.total()).collect();

	let (add_mean, add_med, add_min, add_max) = calculate_stats(&adds);
	let (move_mean, move_med, move_min, move_max) = calculate_stats(&moves);
	let (move_rot_mean, move_rot_med, move_rot_min, move_rot_max) = calculate_stats(&moves_rot);
	let (total_mean, _total_med, _total_min, _total_max) = calculate_stats(&totals);

	let add_share = if total_mean > 0.0 { add_mean / total_mean * 100.0 } else { 0.0 };
	let move_share = if total_mean > 0.0 { move_mean / total_mean * 100.0 } else { 0.0 };

	println!("  {} (n={}):", name, actions.len());
	println!(
		"    ADD:  mean={:5.1}  med={:5.1}  [{:3}-{:3}]  share={:4.1}%",
		add_mean, add_med, add_min, add_max, add_share
	);
	println!(
		"    MOVE: mean={:5.1}  med={:5.1}  [{:3}-{:3}]  share={:4.1}%",
		move_mean, move_med, move_min, move_max, move_share
	);
	println!(
		"      (with rotation: mean={:5.1}  med={:5.1}  [{:3}-{:3}])",
		move_rot_mean, move_rot_med, move_rot_min, move_rot_max
	);
}

fn main() {
	println!("=== Apa Kan AI Bot Benchmark ===\n");
	println!("Bot descriptions:");
	println!("  Ore (O): {}", BotType::Ore.description());
	println!("  Olusare (U): {}", BotType::Olusare.description());
	println!("  Ologbon (B): {}", BotType::Ologbon.description());
	println!();

	let depth = 2;
	let games_per_side = 50;
	let base_seed = 42;

	let bots = [BotType::Ore, BotType::Olusare, BotType::Ologbon];
	let mut all_pairings = Vec::new();

	for (i, &bot1) in bots.iter().enumerate() {
		for &bot2 in bots.iter().skip(i) {
			let stats = run_pairing(bot1, bot2, depth, games_per_side, base_seed);
			all_pairings.push(stats);
		}
	}

	println!("\n=== Pairwise Results ===\n");
	println!(
		"{:<12} {:<12} {:>7} {:>7} {:>6} {:>6} {:>8}",
		"Player 1", "Player 2", "P1 Wins", "P2 Wins", "Draws", "Games", "Avg Turns"
	);
	println!("{:-<70}", "");

	for stats in &all_pairings {
		println!(
			"{:<12} {:<12} {:>7} {:>7} {:>6} {:>6} {:>8.1}",
			stats.p1_name,
			stats.p2_name,
			stats.p1_wins,
			stats.p2_wins,
			stats.draws,
			stats.total_games,
			stats.avg_turns()
		);
	}

	println!("\n=== End Reasons ===\n");
	println!(
		"{:<12} {:<12} {:>12} {:>15} {:>15}",
		"Player 1", "Player 2", "No Legal", "Repetition", "100-Turn Limit"
	);
	println!("{:-<70}", "");

	for stats in &all_pairings {
		println!(
			"{:<12} {:<12} {:>12} {:>15} {:>15}",
			stats.p1_name,
			stats.p2_name,
			stats.no_legal_moves_count,
			stats.repetition_draw_count,
			stats.turn_limit_draw_count
		);
	}

	println!("\n=== First-Move Advantage Analysis ===\n");
	println!(
		"{:<12} {:<12} {:>12} {:>12} {:>10}",
		"Player 1", "Player 2", "P1 Win Rate", "Sample Size", "FMA"
	);
	println!("{:-<70}", "");

	for stats in &all_pairings {
		let p1_wr = stats.p1_win_rate();
		let fma = if p1_wr > 0.5 {
			format!("+{:.1}%", (p1_wr - 0.5) * 200.0)
		} else {
			format!("{:.1}%", (p1_wr - 0.5) * 200.0)
		};

		println!(
			"{:<12} {:<12} {:>11.1}% {:>12} {:>10}",
			stats.p1_name,
			stats.p2_name,
			p1_wr * 100.0,
			stats.total_games,
			fma
		);
	}

	println!("\n=== Mirror Matches (First-Player Advantage) ===\n");
	for stats in &all_pairings {
		if stats.p1_name == stats.p2_name {
			let p1_wr = stats.p1_win_rate();
			let stderr = ((p1_wr * (1.0 - p1_wr)) / stats.total_games as f64).sqrt();
			let ci_95 = 1.96 * stderr;

			println!("{} vs {}", stats.p1_name, stats.p2_name);
			println!("  First-player win rate: {:.1}%", p1_wr * 100.0);
			println!(
				"  95% CI: [{:.1}%, {:.1}%]",
				(p1_wr - ci_95) * 100.0,
				(p1_wr + ci_95) * 100.0
			);
			println!("  Sample size: {} games", stats.total_games);
			println!(
				"  Draw rate: {:.1}%",
				(stats.draws as f64 / stats.total_games as f64) * 100.0
			);
			println!("  Avg game length: {:.1} turns", stats.avg_turns());
			println!();
		}
	}

	println!("\n=== Action Type Statistics (Winner's Actions) ===\n");
	for stats in &all_pairings {
		println!("{} vs {}:", stats.p1_name, stats.p2_name);
		if !stats.p1_win_actions.is_empty() {
			print_action_stats(&format!("{} wins as P1", stats.p1_name), &stats.p1_win_actions);
		}
		if !stats.p2_win_actions.is_empty() {
			print_action_stats(&format!("{} wins as P2", stats.p2_name), &stats.p2_win_actions);
		}
		println!();
	}

	println!("\n=== Action Type Statistics (All Player Actions) ===\n");
	for stats in &all_pairings {
		println!("{} vs {}:", stats.p1_name, stats.p2_name);
		print_action_stats(&format!("{} as P1 (all games)", stats.p1_name), &stats.p1_actions_all);
		print_action_stats(&format!("{} as P2 (all games)", stats.p2_name), &stats.p2_actions_all);
		println!();
	}

	println!("\n=== Overall Statistics ===\n");
	println!("Total games played: {}", all_pairings.iter().map(|s| s.total_games).sum::<usize>());
	println!("Total turns: {}", all_pairings.iter().map(|s| s.total_turns).sum::<usize>());
	println!(
		"Avg turns per game: {:.1}",
		all_pairings.iter().map(|s| s.total_turns).sum::<usize>() as f64
			/ all_pairings.iter().map(|s| s.total_games).sum::<usize>() as f64
	);

	println!("\n=== Reproduction Command ===");
	println!("cargo run --release --example benchmark");
	println!("\nConfiguration:");
	println!("  Search depth: {}", depth);
	println!("  Games per pairing per side: {}", games_per_side);
	println!("  Base seed: {}", base_seed);
	println!("  Turn limit: 100");
}
