#![allow(dead_code)]

// Corrected benchmark with proper seat swapping and measurements
// Fixes: seat swapping, per-bot stats, branching from legal actions, capture causation, per-bot depth

use apa_kan::game::{
	actions::generate_legal_actions, ActionType, AiConfig, AiPlayer, BotType, Game, GameState,
	Player, RuleSet, Rules,
};
use std::collections::HashSet;
use std::fs::{self, OpenOptions};
use std::io::Write as _;
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Default)]
struct BranchingStats {
	positions: usize,
	total_adds: usize,
	total_moves: usize,
	add_counts: Vec<usize>,
	move_counts: Vec<usize>,
}

impl BranchingStats {
	fn record(&mut self, adds: usize, moves: usize) {
		self.positions += 1;
		self.total_adds += adds;
		self.total_moves += moves;
		self.add_counts.push(adds);
		self.move_counts.push(moves);
	}

	fn mean_adds(&self) -> f64 {
		if self.positions == 0 {
			0.0
		} else {
			self.total_adds as f64 / self.positions as f64
		}
	}

	fn mean_moves(&self) -> f64 {
		if self.positions == 0 {
			0.0
		} else {
			self.total_moves as f64 / self.positions as f64
		}
	}

	fn p95(&self, counts: &[usize]) -> usize {
		if counts.is_empty() {
			return 0;
		}
		let mut sorted = counts.to_vec();
		sorted.sort_unstable();
		let idx = ((sorted.len() as f64 * 0.95) as usize).min(sorted.len() - 1);
		sorted[idx]
	}

	fn max(&self, counts: &[usize]) -> usize {
		counts.iter().copied().max().unwrap_or(0)
	}
}

#[derive(Debug, Clone, Default)]
struct BotStats {
	wins: usize,
	losses: usize,
	draws: usize,
	wins_as_p1: usize,
	losses_as_p1: usize,
	draws_as_p1: usize,
	wins_as_p2: usize,
	losses_as_p2: usize,
	draws_as_p2: usize,
	total_moves: usize,
	total_actions: usize,
	move_captures: usize,
	other_captures: usize,
	mcts_iterations: Vec<u64>,
	minimax_depths: Vec<u8>,
}

#[derive(Debug, Clone, Default)]
struct PairingStats {
	bot_a_name: String,
	bot_b_name: String,
	bot_a_stats: BotStats,
	bot_b_stats: BotStats,
	total_games: usize,
	p1_wins: usize,
	p2_wins: usize,
	draws: usize,
	avg_turns: f64,
	branching: BranchingStats,
	unique_outcomes: HashSet<String>,
}

impl PairingStats {
	fn record_game(&mut self, result: GameResult) {
		self.total_games += 1;
		self.avg_turns = (self.avg_turns * (self.total_games - 1) as f64 + result.turns as f64)
			/ self.total_games as f64;

		// Record seat stats
		if result.p1_won {
			self.p1_wins += 1;
		} else if result.p2_won {
			self.p2_wins += 1;
		} else {
			self.draws += 1;
		}

		// Record bot-specific stats
		let (a_was_p1, a_stats, b_stats) = if result.bot_a_was_p1 {
			(
				true,
				(&result.p1_actions, result.p1_won, result.p2_won),
				(&result.p2_actions, result.p2_won, result.p1_won),
			)
		} else {
			(
				false,
				(&result.p2_actions, result.p2_won, result.p1_won),
				(&result.p1_actions, result.p1_won, result.p2_won),
			)
		};

		// Bot A stats
		if a_stats.1 {
			self.bot_a_stats.wins += 1;
			if a_was_p1 {
				self.bot_a_stats.wins_as_p1 += 1;
			} else {
				self.bot_a_stats.wins_as_p2 += 1;
			}
		} else if a_stats.2 {
			self.bot_a_stats.losses += 1;
			if a_was_p1 {
				self.bot_a_stats.losses_as_p1 += 1;
			} else {
				self.bot_a_stats.losses_as_p2 += 1;
			}
		} else {
			self.bot_a_stats.draws += 1;
			if a_was_p1 {
				self.bot_a_stats.draws_as_p1 += 1;
			} else {
				self.bot_a_stats.draws_as_p2 += 1;
			}
		}

		self.bot_a_stats.total_moves += a_stats.0.moves;
		self.bot_a_stats.total_actions += a_stats.0.adds + a_stats.0.moves;
		self.bot_a_stats.move_captures += a_stats.0.move_captures;
		self.bot_a_stats.other_captures += a_stats.0.other_captures;

		// Bot B stats
		if b_stats.1 {
			self.bot_b_stats.wins += 1;
			if !a_was_p1 {
				self.bot_b_stats.wins_as_p1 += 1;
			} else {
				self.bot_b_stats.wins_as_p2 += 1;
			}
		} else if b_stats.2 {
			self.bot_b_stats.losses += 1;
			if !a_was_p1 {
				self.bot_b_stats.losses_as_p1 += 1;
			} else {
				self.bot_b_stats.losses_as_p2 += 1;
			}
		} else {
			self.bot_b_stats.draws += 1;
			if !a_was_p1 {
				self.bot_b_stats.draws_as_p1 += 1;
			} else {
				self.bot_b_stats.draws_as_p2 += 1;
			}
		}

		self.bot_b_stats.total_moves += b_stats.0.moves;
		self.bot_b_stats.total_actions += b_stats.0.adds + b_stats.0.moves;
		self.bot_b_stats.move_captures += b_stats.0.move_captures;
		self.bot_b_stats.other_captures += b_stats.0.other_captures;

		// Branching
		for (adds, moves) in &result.branching {
			self.branching.record(*adds, *moves);
		}

		// Unique outcomes
		self.unique_outcomes.insert(format!(
			"p1_won:{},turns:{},tile_diff:{}",
			result.p1_won, result.turns, result.final_tile_diff
		));
	}

	fn print_summary(&self) {
		println!("\n{} vs {}:", self.bot_a_name, self.bot_b_name);
		println!("  Total games: {}", self.total_games);
		println!();

		// Bot A stats
		println!(
			"  {} overall: {}-{}-{}",
			self.bot_a_name, self.bot_a_stats.wins, self.bot_a_stats.losses, self.bot_a_stats.draws
		);
		println!(
			"    As P1: {}-{}-{}, As P2: {}-{}-{}",
			self.bot_a_stats.wins_as_p1,
			self.bot_a_stats.losses_as_p1,
			self.bot_a_stats.draws_as_p1,
			self.bot_a_stats.wins_as_p2,
			self.bot_a_stats.losses_as_p2,
			self.bot_a_stats.draws_as_p2
		);

		let a_score = self.bot_a_stats.wins as f64 + self.bot_a_stats.draws as f64 * 0.5;
		let a_total =
			(self.bot_a_stats.wins + self.bot_a_stats.losses + self.bot_a_stats.draws) as f64;
		let (a_ci_low, a_ci_high) = wilson_95_ci(a_score, a_total);
		println!(
			"    Score: {:.1}% (95% CI: [{:.1}%, {:.1}%])",
			(a_score / a_total * 100.0),
			a_ci_low * 100.0,
			a_ci_high * 100.0
		);

		let a_move_pct = if self.bot_a_stats.total_actions > 0 {
			self.bot_a_stats.total_moves as f64 / self.bot_a_stats.total_actions as f64 * 100.0
		} else {
			0.0
		};
		println!("    MOVE%: {:.2}%", a_move_pct);
		println!(
			"    Captures: {} from MOVE, {} other",
			self.bot_a_stats.move_captures, self.bot_a_stats.other_captures
		);

		if !self.bot_a_stats.mcts_iterations.is_empty() {
			let avg_mcts: f64 = self.bot_a_stats.mcts_iterations.iter().sum::<u64>() as f64
				/ self.bot_a_stats.mcts_iterations.len() as f64;
			println!("    Avg MCTS iterations/move: {:.0}", avg_mcts);
		}

		if !self.bot_a_stats.minimax_depths.is_empty() {
			let avg_depth: f64 =
				self.bot_a_stats.minimax_depths.iter().map(|&d| d as f64).sum::<f64>()
					/ self.bot_a_stats.minimax_depths.len() as f64;
			println!("    Avg minimax depth: {:.1}", avg_depth);
		}

		println!();

		// Bot B stats (only if different bot)
		if self.bot_a_name != self.bot_b_name {
			println!(
				"  {} overall: {}-{}-{}",
				self.bot_b_name,
				self.bot_b_stats.wins,
				self.bot_b_stats.losses,
				self.bot_b_stats.draws
			);
			println!(
				"    As P1: {}-{}-{}, As P2: {}-{}-{}",
				self.bot_b_stats.wins_as_p1,
				self.bot_b_stats.losses_as_p1,
				self.bot_b_stats.draws_as_p1,
				self.bot_b_stats.wins_as_p2,
				self.bot_b_stats.losses_as_p2,
				self.bot_b_stats.draws_as_p2
			);

			let b_score = self.bot_b_stats.wins as f64 + self.bot_b_stats.draws as f64 * 0.5;
			let b_total =
				(self.bot_b_stats.wins + self.bot_b_stats.losses + self.bot_b_stats.draws) as f64;
			let (b_ci_low, b_ci_high) = wilson_95_ci(b_score, b_total);
			println!(
				"    Score: {:.1}% (95% CI: [{:.1}%, {:.1}%])",
				(b_score / b_total * 100.0),
				b_ci_low * 100.0,
				b_ci_high * 100.0
			);

			let b_move_pct = if self.bot_b_stats.total_actions > 0 {
				self.bot_b_stats.total_moves as f64 / self.bot_b_stats.total_actions as f64 * 100.0
			} else {
				0.0
			};
			println!("    MOVE%: {:.2}%", b_move_pct);
			println!(
				"    Captures: {} from MOVE, {} other",
				self.bot_b_stats.move_captures, self.bot_b_stats.other_captures
			);

			if !self.bot_b_stats.mcts_iterations.is_empty() {
				let avg_mcts: f64 = self.bot_b_stats.mcts_iterations.iter().sum::<u64>() as f64
					/ self.bot_b_stats.mcts_iterations.len() as f64;
				println!("    Avg MCTS iterations/move: {:.0}", avg_mcts);
			}

			if !self.bot_b_stats.minimax_depths.is_empty() {
				let avg_depth: f64 =
					self.bot_b_stats.minimax_depths.iter().map(|&d| d as f64).sum::<f64>()
						/ self.bot_b_stats.minimax_depths.len() as f64;
				println!("    Avg minimax depth: {:.1}", avg_depth);
			}

			println!();
		}

		// Seat stats
		println!("  Seat stats: P1 {}-{}-{} P2", self.p1_wins, self.draws, self.p2_wins);
		println!("  Avg turns/game: {:.1}", self.avg_turns);
		println!("  Unique outcomes: {}", self.unique_outcomes.len());
		println!();

		// Branching (per-position legal actions)
		println!(
			"  Per-position branching: ADD mean={:.1}, p95={}, max={}",
			self.branching.mean_adds(),
			self.branching.p95(&self.branching.add_counts),
			self.branching.max(&self.branching.add_counts)
		);
		println!(
			"                         MOVE mean={:.1}, p95={}, max={}",
			self.branching.mean_moves(),
			self.branching.p95(&self.branching.move_counts),
			self.branching.max(&self.branching.move_counts)
		);
	}
}

fn wilson_95_ci(successes: f64, total: f64) -> (f64, f64) {
	if total == 0.0 {
		return (0.0, 1.0);
	}
	let p = successes / total;
	let z = 1.96;
	let denom = 1.0 + z * z / total;
	let center = (p + z * z / (2.0 * total)) / denom;
	let margin = z * (p * (1.0 - p) / total + z * z / (4.0 * total * total)).sqrt() / denom;
	(center - margin, center + margin)
}

#[derive(Debug, Clone, Default)]
struct PlayerActionStats {
	adds: usize,
	moves: usize,
	move_captures: usize,
	other_captures: usize,
}

#[derive(Debug, Clone)]
struct GameResult {
	p1_won: bool,
	p2_won: bool,
	turns: usize,
	p1_actions: PlayerActionStats,
	p2_actions: PlayerActionStats,
	bot_a_was_p1: bool,
	branching: Vec<(usize, usize)>,
	final_tile_diff: i32,
}

fn play_game(p1: &AiPlayer, p2: &AiPlayer, rule_set: RuleSet, bot_a_is_p1: bool) -> GameResult {
	let mut game = Game::new_with_rules(rule_set);
	let mut p1_actions = PlayerActionStats::default();
	let mut p2_actions = PlayerActionStats::default();
	let mut branching = Vec::new();

	while game.turn_count < 100 && !game.is_game_over() {
		let current_player = game.current_player;
		let ai = if current_player == Player::One { p1 } else { p2 };
		let action_stats =
			if current_player == Player::One { &mut p1_actions } else { &mut p2_actions };

		// Measure per-position branching from actual legal actions
		let legal_actions = generate_legal_actions(&game);
		let add_count = legal_actions
			.iter()
			.filter(|a| matches!(a.action_type, ActionType::Add { .. }))
			.count();
		let move_count = legal_actions
			.iter()
			.filter(|a| matches!(a.action_type, ActionType::Move { .. }))
			.count();
		branching.push((add_count, move_count));

		ai.reset_stats();
		if let Some(action) = ai.choose_action(&game) {
			let is_move = matches!(action.action_type, ActionType::Move { .. });

			// Count tiles before
			let opp = current_player.other();
			let opp_tiles_before = game.board.tiles().filter(|(_, t)| t.owner == opp).count();

			match action.action_type {
				ActionType::Add { .. } => action_stats.adds += 1,
				ActionType::Move { .. } => action_stats.moves += 1,
				ActionType::Pass => {}
			}

			if Rules::apply_action(&mut game, action).is_err() {
				break;
			}

			// Count captures
			let opp_tiles_after = game.board.tiles().filter(|(_, t)| t.owner == opp).count();
			let captured = opp_tiles_before.saturating_sub(opp_tiles_after);

			if captured > 0 {
				if is_move {
					action_stats.move_captures += captured;
				} else {
					action_stats.other_captures += captured;
				}
			}
		} else {
			break;
		}
	}

	let p1_tiles = game.board.tiles().filter(|(_, t)| t.owner == Player::One).count() as i32;
	let p2_tiles = game.board.tiles().filter(|(_, t)| t.owner == Player::Two).count() as i32;

	GameResult {
		p1_won: matches!(game.state, GameState::Won(Player::One)),
		p2_won: matches!(game.state, GameState::Won(Player::Two)),
		turns: game.turn_count,
		p1_actions,
		p2_actions,
		bot_a_was_p1: bot_a_is_p1,
		branching,
		final_tile_diff: p1_tiles - p2_tiles,
	}
}

fn run_pairing(
	bot_a: BotType,
	bot_b: BotType,
	games_per_seat: usize,
	rule_set: RuleSet,
	time_budget: Duration,
	base_seed: u64,
	output_dir: &Path,
) -> PairingStats {
	let config = AiConfig {
		depth: 3,
		bot_type: bot_a,
		time_budget: Some(time_budget),
		use_transposition_table: true,
		use_move_ordering: true,
		mcts_iterations: None,
		mcts_exploration: 1.41,
		mcts_rollout_depth: 20,
	};

	let mut stats = PairingStats {
		bot_a_name: bot_a.name().to_string(),
		bot_b_name: bot_b.name().to_string(),
		..Default::default()
	};

	println!("  Running {} vs {} ({} games/seat)...", bot_a.name(), bot_b.name(), games_per_seat);

	// Play N games with bot_a as P1
	for game_idx in 0..games_per_seat {
		let seed_a = base_seed + game_idx as u64;
		let seed_b = base_seed + 1000 + game_idx as u64;

		let p1 = AiPlayer::new_seeded(AiConfig { bot_type: bot_a, ..config }, seed_a);
		let p2 = AiPlayer::new_seeded(AiConfig { bot_type: bot_b, ..config }, seed_b);

		let result = play_game(&p1, &p2, rule_set, true);

		// Collect per-bot stats
		if p1.stats.borrow().nodes_searched > 0 && bot_a == BotType::Mcts {
			stats.bot_a_stats.mcts_iterations.push(p1.stats.borrow().nodes_searched);
		}
		if p1.stats.borrow().nodes_searched > 0 && bot_a != BotType::Mcts && bot_a != BotType::Ore {
			// Estimate depth for minimax
			let s = p1.stats.borrow();
			let total = s.add_actions_generated + s.move_actions_generated;
			if total > 0 && s.nodes_searched > 0 {
				let bf = total as f64 / s.nodes_searched as f64;
				if bf > 1.0 {
					stats.bot_a_stats.minimax_depths.push((s.nodes_searched as f64).log(bf) as u8);
				}
			}
		}

		if p2.stats.borrow().nodes_searched > 0 && bot_b == BotType::Mcts {
			stats.bot_b_stats.mcts_iterations.push(p2.stats.borrow().nodes_searched);
		}
		if p2.stats.borrow().nodes_searched > 0 && bot_b != BotType::Mcts && bot_b != BotType::Ore {
			let s = p2.stats.borrow();
			let total = s.add_actions_generated + s.move_actions_generated;
			if total > 0 && s.nodes_searched > 0 {
				let bf = total as f64 / s.nodes_searched as f64;
				if bf > 1.0 {
					stats.bot_b_stats.minimax_depths.push((s.nodes_searched as f64).log(bf) as u8);
				}
			}
		}

		stats.record_game(result);
	}

	// Play N games with bot_b as P1 (swap seats)
	for game_idx in 0..games_per_seat {
		let seed_a = base_seed + 2000 + game_idx as u64;
		let seed_b = base_seed + 3000 + game_idx as u64;

		let p1 = AiPlayer::new_seeded(AiConfig { bot_type: bot_b, ..config }, seed_b);
		let p2 = AiPlayer::new_seeded(AiConfig { bot_type: bot_a, ..config }, seed_a);

		let result = play_game(&p1, &p2, rule_set, false);

		// Collect per-bot stats
		if p1.stats.borrow().nodes_searched > 0 && bot_b == BotType::Mcts {
			stats.bot_b_stats.mcts_iterations.push(p1.stats.borrow().nodes_searched);
		}
		if p1.stats.borrow().nodes_searched > 0 && bot_b != BotType::Mcts && bot_b != BotType::Ore {
			let s = p1.stats.borrow();
			let total = s.add_actions_generated + s.move_actions_generated;
			if total > 0 && s.nodes_searched > 0 {
				let bf = total as f64 / s.nodes_searched as f64;
				if bf > 1.0 {
					stats.bot_b_stats.minimax_depths.push((s.nodes_searched as f64).log(bf) as u8);
				}
			}
		}

		if p2.stats.borrow().nodes_searched > 0 && bot_a == BotType::Mcts {
			stats.bot_a_stats.mcts_iterations.push(p2.stats.borrow().nodes_searched);
		}
		if p2.stats.borrow().nodes_searched > 0 && bot_a != BotType::Mcts && bot_a != BotType::Ore {
			let s = p2.stats.borrow();
			let total = s.add_actions_generated + s.move_actions_generated;
			if total > 0 && s.nodes_searched > 0 {
				let bf = total as f64 / s.nodes_searched as f64;
				if bf > 1.0 {
					stats.bot_a_stats.minimax_depths.push((s.nodes_searched as f64).log(bf) as u8);
				}
			}
		}

		stats.record_game(result);
	}

	// Write results
	let filename = format!(
		"{}_{}_vs_{}.txt",
		if rule_set.board_size == 5 && rule_set.scored_mode {
			"5x5scored"
		} else if rule_set.board_size == 7 && rule_set.scored_mode {
			"7x7scored"
		} else {
			"5x5old"
		},
		bot_a.name(),
		bot_b.name()
	);
	let path = output_dir.join(filename);
	let mut file = OpenOptions::new().create(true).append(true).open(path).unwrap();
	writeln!(file, "Completed {} games", stats.total_games).unwrap();

	stats
}

fn main() {
	let start_time = Instant::now();
	let time_budget = Duration::from_millis(500);
	let games_per_seat = 10;
	let base_seed = 42;

	let output_dir = Path::new("benchmark_results_corrected");
	fs::create_dir_all(output_dir).expect("Failed to create output directory");

	println!("Corrected Benchmark: Scored Mode Evaluation");
	println!("============================================");
	println!("Time budget: {}ms/move", time_budget.as_millis());
	println!("Games per seat: {} (total {} per cross-pairing)", games_per_seat, games_per_seat * 2);
	println!();

	let rule_sets = vec![
		("5x5-scored", RuleSet::scored()),
		("7x7-scored", RuleSet::scored_7x7()),
		("5x5-old", RuleSet::old()),
	];

	let bots = vec![BotType::Ore, BotType::Olusare, BotType::Ologbon, BotType::Mcts];

	for (rule_name, rule_set) in rule_sets {
		println!("\n=== {} ===", rule_name);
		println!(
			"Board: {}x{}, Bank: {}, Scored: {}",
			rule_set.board_size, rule_set.board_size, rule_set.add_bank_size, rule_set.scored_mode
		);

		let mut pairings = Vec::new();
		for (i, &bot1) in bots.iter().enumerate() {
			for &bot2 in bots.iter().skip(i) {
				pairings.push((bot1, bot2));
			}
		}

		println!("Running {} pairings in parallel...\n", pairings.len());

		let results: Arc<Mutex<Vec<PairingStats>>> = Arc::new(Mutex::new(Vec::new()));
		let mut handles = vec![];

		for (bot1, bot2) in pairings {
			let results = Arc::clone(&results);
			let output_dir = output_dir.to_path_buf();

			let handle = thread::spawn(move || {
				let stats = run_pairing(
					bot1,
					bot2,
					games_per_seat,
					rule_set,
					time_budget,
					base_seed,
					&output_dir,
				);
				results.lock().unwrap().push(stats);
			});

			handles.push(handle);
		}

		for handle in handles {
			handle.join().expect("Thread panicked");
		}

		println!("\n--- {} Results ---\n", rule_name);

		let results_guard = results.lock().unwrap();
		for stats in results_guard.iter() {
			stats.print_summary();
		}
	}

	println!("\n\nTotal time: {:.2}s", start_time.elapsed().as_secs_f64());
	println!("Results written to: {:?}", output_dir);
}
