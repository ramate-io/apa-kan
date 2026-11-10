// Scoped depth-3 benchmark: 3 variants, 3 pairings, 20 games/side
// Run pairings sequentially to completion

use apa_kan::game::{
	Action, ActionType, AiConfig, AiPlayer, BotType, Game, GameState, Player, RuleSet, Rules,
};
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Copy, Default)]
struct ActionStats {
	adds: usize,
	moves: usize,
	captures: usize,
}

impl ActionStats {
	fn add_action(&mut self, action: Action, tiles_before: usize, tiles_after: usize) {
		match action.action_type {
			ActionType::Add { .. } => self.adds += 1,
			ActionType::Move { .. } => self.moves += 1,
		}
		if tiles_after < tiles_before {
			self.captures += tiles_before - tiles_after;
		}
	}
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum EndReason {
	NoLegalMoves,
	RepetitionDraw,
	TurnLimitDraw,
}

#[derive(Debug, Clone, Default)]
struct MoveProfile {
	nodes_searched: u64,
	add_actions: u64,
	move_actions: u64,
	duration: Duration,
}

#[derive(Debug, Clone, Default)]
struct GameProfile {
	move_profiles: Vec<MoveProfile>,
}

impl GameProfile {
	fn avg_nodes_per_move(&self) -> f64 {
		if self.move_profiles.is_empty() {
			return 0.0;
		}
		let total: u64 = self.move_profiles.iter().map(|p| p.nodes_searched).sum();
		total as f64 / self.move_profiles.len() as f64
	}

	fn avg_branching_factor(&self) -> (f64, f64) {
		if self.move_profiles.is_empty() {
			return (0.0, 0.0);
		}
		let total_adds: u64 = self.move_profiles.iter().map(|p| p.add_actions).sum();
		let total_moves: u64 = self.move_profiles.iter().map(|p| p.move_actions).sum();
		let count = self.move_profiles.len() as f64;
		(total_adds as f64 / count, total_moves as f64 / count)
	}

	fn avg_time_per_move(&self) -> (f64, f64) {
		if self.move_profiles.is_empty() {
			return (0.0, 0.0);
		}
		let mut times: Vec<f64> =
			self.move_profiles.iter().map(|p| p.duration.as_secs_f64() * 1000.0).collect();
		times.sort_by(|a, b| a.partial_cmp(b).unwrap());
		let mean = times.iter().sum::<f64>() / times.len() as f64;
		let p95_idx = ((times.len() as f64) * 0.95) as usize;
		let p95 = if p95_idx < times.len() {
			times[p95_idx]
		} else {
			times.last().copied().unwrap_or(0.0)
		};
		(mean, p95)
	}
}

#[derive(Debug, Clone)]
struct MatchResult {
	first_player_won: bool,
	is_draw: bool,
	turns: usize,
	reason: EndReason,
	p1_actions: ActionStats,
	p2_actions: ActionStats,
	profile: GameProfile,
}

#[derive(Debug)]
struct PairingStats {
	p1_name: String,
	p2_name: String,
	first_seat_wins: usize,
	second_seat_wins: usize,
	draws: usize,
	total_games: usize,
	total_turns: usize,
	total_moves: usize,
	total_captures: usize,
	unique_outcomes: std::collections::HashSet<u64>,
	profiles: Vec<GameProfile>,
}

impl PairingStats {
	fn new(p1_name: &str, p2_name: &str) -> Self {
		Self {
			p1_name: p1_name.to_string(),
			p2_name: p2_name.to_string(),
			first_seat_wins: 0,
			second_seat_wins: 0,
			draws: 0,
			total_games: 0,
			total_turns: 0,
			total_moves: 0,
			total_captures: 0,
			unique_outcomes: std::collections::HashSet::new(),
			profiles: Vec::new(),
		}
	}

	fn first_seat_win_rate(&self) -> f64 {
		let decided = self.first_seat_wins + self.second_seat_wins;
		if decided == 0 {
			0.0
		} else {
			self.first_seat_wins as f64 / decided as f64
		}
	}

	fn move_pct(&self) -> f64 {
		if self.total_moves + self.total_games * 11 > 0 {
			(self.total_moves as f64 / (self.total_moves + self.total_games * 11) as f64) * 100.0
		} else {
			0.0
		}
	}

	fn captures_per_game(&self) -> f64 {
		if self.total_games == 0 {
			0.0
		} else {
			self.total_captures as f64 / self.total_games as f64
		}
	}

	fn avg_nodes_per_move(&self) -> f64 {
		if self.profiles.is_empty() {
			return 0.0;
		}
		let sum: f64 = self.profiles.iter().map(|p| p.avg_nodes_per_move()).sum();
		sum / self.profiles.len() as f64
	}

	fn avg_branching_factor(&self) -> (f64, f64) {
		if self.profiles.is_empty() {
			return (0.0, 0.0);
		}
		let (total_adds, total_moves) = self
			.profiles
			.iter()
			.map(|p| p.avg_branching_factor())
			.fold((0.0, 0.0), |(a1, m1), (a2, m2)| (a1 + a2, m1 + m2));
		(total_adds / self.profiles.len() as f64, total_moves / self.profiles.len() as f64)
	}

	fn avg_time_per_move(&self) -> (f64, f64) {
		if self.profiles.is_empty() {
			return (0.0, 0.0);
		}
		let mut all_times: Vec<f64> = Vec::new();
		for p in &self.profiles {
			for m in &p.move_profiles {
				all_times.push(m.duration.as_secs_f64() * 1000.0);
			}
		}
		if all_times.is_empty() {
			return (0.0, 0.0);
		}
		all_times.sort_by(|a, b| a.partial_cmp(b).unwrap());
		let mean = all_times.iter().sum::<f64>() / all_times.len() as f64;
		let p95_idx = ((all_times.len() as f64) * 0.95) as usize;
		let p95 = if p95_idx < all_times.len() {
			all_times[p95_idx]
		} else {
			all_times.last().copied().unwrap_or(0.0)
		};
		(mean, p95)
	}
}

fn play_game(p1: &AiPlayer, p2: &AiPlayer, rule_set: RuleSet) -> MatchResult {
	let mut game = Game::new_with_rules(rule_set);
	let mut p1_actions = ActionStats::default();
	let mut p2_actions = ActionStats::default();
	let mut profile = GameProfile::default();

	while game.turn_count < 100 {
		let current_player = game.current_player;
		let ai = if current_player == Player::One { p1 } else { p2 };
		let tiles_before = game.board.tiles().count();

		let move_start = Instant::now();
		if let Some(action) = ai.choose_action(&game) {
			let move_duration = move_start.elapsed();
			let stats = ai.stats.borrow();
			profile.move_profiles.push(MoveProfile {
				nodes_searched: stats.nodes_searched,
				add_actions: stats.add_actions_generated,
				move_actions: stats.move_actions_generated,
				duration: move_duration,
			});

			if Rules::apply_action(&mut game, action).is_err() {
				break;
			}
			let tiles_after = game.board.tiles().count();
			if current_player == Player::One {
				p1_actions.add_action(action, tiles_before, tiles_after);
			} else {
				p2_actions.add_action(action, tiles_before, tiles_after);
			}
		} else {
			return MatchResult {
				first_player_won: current_player == Player::Two,
				is_draw: false,
				turns: game.turn_count,
				reason: EndReason::NoLegalMoves,
				p1_actions,
				p2_actions,
				profile,
			};
		}

		match game.state {
			GameState::Won(winner) => {
				return MatchResult {
					first_player_won: winner == Player::One,
					is_draw: false,
					turns: game.turn_count,
					reason: EndReason::NoLegalMoves,
					p1_actions,
					p2_actions,
					profile,
				};
			}
			GameState::Draw => {
				let reason = if game.turn_count >= 100 {
					EndReason::TurnLimitDraw
				} else {
					EndReason::RepetitionDraw
				};
				return MatchResult {
					first_player_won: false,
					is_draw: true,
					turns: game.turn_count,
					reason,
					p1_actions,
					p2_actions,
					profile,
				};
			}
			GameState::InProgress => {}
		}
	}

	MatchResult {
		first_player_won: false,
		is_draw: true,
		turns: game.turn_count,
		reason: EndReason::TurnLimitDraw,
		p1_actions,
		p2_actions,
		profile,
	}
}

fn run_pairing(
	bot1_type: BotType,
	bot2_type: BotType,
	depth: u8,
	games_per_side: usize,
	rule_set: RuleSet,
	base_seed: u64,
) -> PairingStats {
	let mut stats = PairingStats::new(bot1_type.name(), bot2_type.name());

	println!(
		"  Running {} vs {} ({} games/side)...",
		bot1_type.name(),
		bot2_type.name(),
		games_per_side
	);

	for i in 0..games_per_side {
		let seed = base_seed.wrapping_mul(1000).wrapping_add(i as u64);
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
			seed,
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
			seed.wrapping_add(1),
		);

		let result = play_game(&p1, &p2, rule_set);
		stats.profiles.push(result.profile.clone());
		stats.total_games += 1;
		stats.total_turns += result.turns;
		stats.total_moves += result.p1_actions.moves + result.p2_actions.moves;
		stats.total_captures += result.p1_actions.captures + result.p2_actions.captures;

		let outcome_hash = result.turns as u64 * 1000
			+ if result.is_draw {
				999
			} else if result.first_player_won {
				1
			} else {
				2
			};
		stats.unique_outcomes.insert(outcome_hash);

		if result.is_draw {
			stats.draws += 1;
		} else if result.first_player_won {
			stats.first_seat_wins += 1;
		} else {
			stats.second_seat_wins += 1;
		}
	}

	for i in 0..games_per_side {
		let seed = base_seed.wrapping_mul(3000).wrapping_add(i as u64);
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
			seed,
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
			seed.wrapping_add(1),
		);

		let result = play_game(&p1, &p2, rule_set);
		stats.profiles.push(result.profile.clone());
		stats.total_games += 1;
		stats.total_turns += result.turns;
		stats.total_moves += result.p1_actions.moves + result.p2_actions.moves;
		stats.total_captures += result.p1_actions.captures + result.p2_actions.captures;

		let outcome_hash = result.turns as u64 * 1000
			+ if result.is_draw {
				999
			} else if result.first_player_won {
				1
			} else {
				2
			};
		stats.unique_outcomes.insert(outcome_hash);

		if result.is_draw {
			stats.draws += 1;
		} else if result.first_player_won {
			stats.first_seat_wins += 1;
		} else {
			stats.second_seat_wins += 1;
		}
	}

	stats
}

fn main() {
	println!("=== Depth 3 Scoped Benchmark ===\n");
	println!("Configuration:");
	println!("  Depth: 3");
	println!("  Games per side: 20 (40 total per pairing)");
	println!("  Optimizations: TT + MO enabled\n");

	let depth = 3;
	let games_per_side = 20;
	let base_seed = 42;
	let bots = [BotType::Olusare, BotType::Ologbon];

	let variants = [
		("Old Rules", RuleSet::old()),
		("A+B (bank=7)", RuleSet::new()),
		("A+B (bank=5)", RuleSet::with_bank_size(5)),
	];

	for (variant_name, rule_set) in &variants {
		println!("\n{}", "=".repeat(60));
		println!("Variant: {}", variant_name);
		println!("{}", "=".repeat(60));

		let start = Instant::now();
		let mut all_pairings = Vec::new();

		for (i, &bot1) in bots.iter().enumerate() {
			for &bot2 in bots.iter().skip(i) {
				let stats = run_pairing(bot1, bot2, depth, games_per_side, *rule_set, base_seed);
				all_pairings.push(stats);
			}
		}

		let elapsed = start.elapsed().as_secs_f64();

		println!("\nResults:");
		println!(
			"{:<12} {:<12} {:>9} {:>9} {:>6} | MOVE%  Cap/Gm Unique",
			"Bot1", "Bot2", "1st Wins", "2nd Wins", "Draws"
		);
		println!("{}", "-".repeat(70));

		for stats in &all_pairings {
			println!(
				"{:<12} {:<12} {:>9} {:>9} {:>6} | {:>5.1}% {:>6.1} {:>6}",
				stats.p1_name,
				stats.p2_name,
				stats.first_seat_wins,
				stats.second_seat_wins,
				stats.draws,
				stats.move_pct(),
				stats.captures_per_game(),
				stats.unique_outcomes.len()
			);
		}

		println!("\nFirst-Seat Win Rates:");
		for stats in &all_pairings {
			let fwr = stats.first_seat_win_rate();
			let decided = stats.first_seat_wins + stats.second_seat_wins;
			let stderr =
				if decided > 0 { ((fwr * (1.0 - fwr)) / decided as f64).sqrt() } else { 0.0 };
			let ci_95 = 1.96 * stderr;
			println!(
				"  {} vs {}: {:.1}% [{:.1}%, {:.1}%]",
				stats.p1_name,
				stats.p2_name,
				fwr * 100.0,
				(fwr - ci_95).max(0.0) * 100.0,
				(fwr + ci_95).min(1.0) * 100.0
			);
		}

		println!("\nCompute Profile:");
		for stats in &all_pairings {
			let (mean_time, p95_time) = stats.avg_time_per_move();
			let (avg_adds, avg_moves) = stats.avg_branching_factor();
			println!("  {} vs {}:", stats.p1_name, stats.p2_name);
			println!("    Nodes/move: {:.0}", stats.avg_nodes_per_move());
			println!("    Branching: {:.1} ADD, {:.1} MOVE", avg_adds, avg_moves);
			println!("    Time/move: {:.0}ms mean, {:.0}ms p95", mean_time, p95_time);
		}

		println!(
			"\nRuntime: {:.1}s for {} games",
			elapsed,
			all_pairings.iter().map(|s| s.total_games).sum::<usize>()
		);
	}

	println!("\n{}", "=".repeat(60));
	println!("Benchmark complete");
}
