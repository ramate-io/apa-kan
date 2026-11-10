// Phase 3 benchmark: All bots including MCTS, 500ms/move, parallel execution
// New defaults: corner-adjacent ADD + bank 7 + fixed rotation
// Seat-correct win counting, incremental disk writes

use apa_kan::game::{
	Action, ActionType, AiConfig, AiPlayer, BotType, Game, GameState, Player, RuleSet, Rules,
};
use std::collections::{HashMap, HashSet};
use std::fs::{self, File, OpenOptions};
use std::io::Write as _;
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::thread;
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
			ActionType::Pass => {}
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
	DoublePass,
	Elimination,
}

#[derive(Debug, Clone, Default)]
struct MoveProfile {
	nodes_searched: u64,
	add_actions: u64,
	move_actions: u64,
	duration: Duration,
	mcts_iterations: u64,
	effective_depth: u8,
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

	fn avg_mcts_iterations(&self) -> f64 {
		if self.move_profiles.is_empty() {
			return 0.0;
		}
		let total: u64 = self.move_profiles.iter().map(|p| p.mcts_iterations).sum();
		total as f64 / self.move_profiles.len() as f64
	}

	fn avg_effective_depth(&self) -> f64 {
		if self.move_profiles.is_empty() {
			return 0.0;
		}
		let total: u64 = self.move_profiles.iter().map(|p| p.effective_depth as u64).sum();
		total as f64 / self.move_profiles.len() as f64
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
	unique_outcomes: HashSet<u64>,
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
			unique_outcomes: HashSet::new(),
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

	fn avg_turns_per_game(&self) -> f64 {
		if self.total_games == 0 {
			0.0
		} else {
			self.total_turns as f64 / self.total_games as f64
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

	fn avg_mcts_iterations(&self) -> f64 {
		if self.profiles.is_empty() {
			return 0.0;
		}
		let sum: f64 = self.profiles.iter().map(|p| p.avg_mcts_iterations()).sum();
		sum / self.profiles.len() as f64
	}

	fn avg_effective_depth(&self) -> f64 {
		if self.profiles.is_empty() {
			return 0.0;
		}
		let sum: f64 = self.profiles.iter().map(|p| p.avg_effective_depth()).sum();
		sum / self.profiles.len() as f64
	}

	fn wilson_95_ci(&self) -> (f64, f64) {
		let n = self.first_seat_wins + self.second_seat_wins;
		if n == 0 {
			return (0.0, 1.0);
		}
		let p = self.first_seat_wins as f64 / n as f64;
		let z = 1.96;
		let denominator = 1.0 + z * z / n as f64;
		let center = (p + z * z / (2.0 * n as f64)) / denominator;
		let margin = z * (p * (1.0 - p) / n as f64 + z * z / (4.0 * n as f64 * n as f64)).sqrt()
			/ denominator;
		(center - margin, center + margin)
	}

	fn print_summary(&self) {
		println!("\n{} vs {}:", self.p1_name, self.p2_name);
		println!("  Games: {}", self.total_games);
		println!(
			"  First seat wins: {} ({:.1}%)",
			self.first_seat_wins,
			self.first_seat_win_rate() * 100.0
		);
		println!(
			"  Second seat wins: {} ({:.1}%)",
			self.second_seat_wins,
			(1.0 - self.first_seat_win_rate()) * 100.0
		);
		println!("  Draws: {}", self.draws);

		let (ci_low, ci_high) = self.wilson_95_ci();
		println!("  First seat win rate 95% CI: [{:.1}%, {:.1}%]", ci_low * 100.0, ci_high * 100.0);

		println!("  MOVE%: {:.2}%", self.move_pct());
		println!("  Captures/game: {:.2}", self.captures_per_game());
		println!("  Avg turns/game: {:.1}", self.avg_turns_per_game());
		println!("  Unique outcomes: {}", self.unique_outcomes.len());

		let (avg_adds, avg_moves) = self.avg_branching_factor();
		println!("  Branching factor: ADD={:.1}, MOVE={:.1}", avg_adds, avg_moves);

		// Only show MCTS/minimax stats if relevant
		let mcts_iters = self.avg_mcts_iterations();
		if mcts_iters > 0.0 {
			println!("  Avg MCTS iterations/move: {:.0}", mcts_iters);
		}

		let eff_depth = self.avg_effective_depth();
		if eff_depth > 0.0 {
			println!("  Avg effective depth (minimax): {:.1}", eff_depth);
		}
	}
}

fn play_game(p1: &AiPlayer, p2: &AiPlayer, rule_set: RuleSet, track_depth: bool) -> MatchResult {
	let mut game = Game::new_with_rules(rule_set);
	let mut p1_actions = ActionStats::default();
	let mut p2_actions = ActionStats::default();
	let mut profile = GameProfile::default();

	while game.turn_count < 100 {
		let current_player = game.current_player;
		let ai = if current_player == Player::One { p1 } else { p2 };
		let tiles_before = game.board.tiles().count();

		ai.reset_stats();
		let move_start = Instant::now();
		if let Some(action) = ai.choose_action(&game) {
			let move_duration = move_start.elapsed();
			let stats = ai.stats.borrow();

			// For iterative deepening minimax, track effective depth reached
			let effective_depth = if track_depth {
				// Estimate based on nodes searched and branching factor
				let total_actions = stats.add_actions_generated + stats.move_actions_generated;
				if total_actions > 0 && stats.nodes_searched > 0 {
					let avg_bf = total_actions as f64 / stats.nodes_searched as f64;
					if avg_bf > 1.0 {
						(stats.nodes_searched as f64).log(avg_bf) as u8
					} else {
						1
					}
				} else {
					0
				}
			} else {
				0
			};

			profile.move_profiles.push(MoveProfile {
				nodes_searched: stats.nodes_searched,
				add_actions: stats.add_actions_generated,
				move_actions: stats.move_actions_generated,
				duration: move_duration,
				mcts_iterations: stats.nodes_searched,
				effective_depth,
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
				let reason = if game.rule_set.scored_mode {
					let loser = winner.other();
					let loser_tiles = game.board.tiles().filter(|(_, t)| t.owner == loser).count();
					if loser_tiles == 0 {
						EndReason::Elimination
					} else {
						EndReason::NoLegalMoves
					}
				} else {
					EndReason::NoLegalMoves
				};
				return MatchResult {
					first_player_won: winner == Player::One,
					is_draw: false,
					turns: game.turn_count,
					reason,
					p1_actions,
					p2_actions,
					profile,
				};
			}
			GameState::Draw => {
				let reason = if game.consecutive_passes >= 2 {
					EndReason::DoublePass
				} else if game.turn_count >= 100 {
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
	games_per_side: usize,
	rule_set: RuleSet,
	time_budget: Duration,
	base_seed: u64,
	output_dir: &Path,
) -> PairingStats {
	let mut stats = PairingStats::new(bot1_type.name(), bot2_type.name());
	let track_depth = matches!(bot1_type, BotType::Olusare | BotType::Ologbon)
		|| matches!(bot2_type, BotType::Olusare | BotType::Ologbon);

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
				depth: 10,
				bot_type: bot1_type,
				time_budget: Some(time_budget),
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
				depth: 10,
				bot_type: bot2_type,
				time_budget: Some(time_budget),
				use_transposition_table: true,
				use_move_ordering: true,
				mcts_iterations: None,
				mcts_exploration: 1.41,
				mcts_rollout_depth: 20,
			},
			seed.wrapping_add(1),
		);

		let result = play_game(&p1, &p2, rule_set, track_depth);
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

		// Write incremental result
		let result_file =
			output_dir.join(format!("{}_vs_{}.txt", bot1_type.name(), bot2_type.name()));
		if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(&result_file) {
			writeln!(
				file,
				"Game {}: {} turns, first_won={}, draw={}, captures={}",
				i + 1,
				result.turns,
				result.first_player_won,
				result.is_draw,
				result.p1_actions.captures + result.p2_actions.captures
			)
			.ok();
		}
	}

	// Write final summary
	let summary_file =
		output_dir.join(format!("{}_vs_{}_summary.txt", bot1_type.name(), bot2_type.name()));
	if let Ok(mut file) = File::create(&summary_file) {
		writeln!(file, "{} vs {}", stats.p1_name, stats.p2_name).ok();
		writeln!(file, "Games: {}", stats.total_games).ok();
		writeln!(
			file,
			"First seat wins: {} ({:.1}%)",
			stats.first_seat_wins,
			stats.first_seat_win_rate() * 100.0
		)
		.ok();
		writeln!(file, "Second seat wins: {}", stats.second_seat_wins).ok();
		writeln!(file, "Draws: {}", stats.draws).ok();
		let (ci_low, ci_high) = stats.wilson_95_ci();
		writeln!(file, "95% CI: [{:.1}%, {:.1}%]", ci_low * 100.0, ci_high * 100.0).ok();
		writeln!(file, "MOVE%: {:.2}%", stats.move_pct()).ok();
		writeln!(file, "Captures/game: {:.2}", stats.captures_per_game()).ok();
		writeln!(file, "Unique outcomes: {}", stats.unique_outcomes.len()).ok();
	}

	stats
}

fn main() {
	let start_time = Instant::now();
	let time_budget = Duration::from_millis(500);
	let games_per_side = 10;
	let base_seed = 42;

	let output_dir = Path::new("benchmark_results");
	fs::create_dir_all(output_dir).expect("Failed to create output directory");

	println!("Final Benchmark: Scored Mode Evaluation");
	println!("========================================");
	println!("Time budget: {}ms/move", time_budget.as_millis());
	println!("Games per side: {}", games_per_side);
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

		// Generate all pairings including mirrors
		let mut pairings = Vec::new();
		for (i, &bot1) in bots.iter().enumerate() {
			for &bot2 in bots.iter().skip(i) {
				pairings.push((bot1, bot2));
			}
		}

		println!("Running {} pairings in parallel...\n", pairings.len());

		// Run pairings in parallel
		let results: Arc<Mutex<HashMap<String, PairingStats>>> =
			Arc::new(Mutex::new(HashMap::new()));
		let mut handles = vec![];

		for (bot1, bot2) in pairings {
			let results = Arc::clone(&results);
			let output_dir = output_dir.to_path_buf();

			let handle = thread::spawn(move || {
				let stats = run_pairing(
					bot1,
					bot2,
					games_per_side,
					rule_set,
					time_budget,
					base_seed,
					&output_dir,
				);
				let key = format!("{}_{}_vs_{}", rule_name, bot1.name(), bot2.name());
				results.lock().unwrap().insert(key, stats);
			});

			handles.push(handle);
		}

		// Wait for all threads to complete
		for handle in handles {
			handle.join().expect("Thread panicked");
		}

		// Print summaries for this rule set
		println!("\n--- {} Results ---\n", rule_name);

		let results_guard = results.lock().unwrap();
		let mut keys: Vec<_> = results_guard.keys().cloned().collect();
		keys.sort();

		for key in keys {
			if let Some(stats) = results_guard.get(&key) {
				stats.print_summary();
			}
		}
		drop(results_guard);
	}

	println!("\n\nTotal time: {:.2}s", start_time.elapsed().as_secs_f64());
	println!("Results written to: {:?}", output_dir);
}
