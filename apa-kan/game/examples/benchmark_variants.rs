use apa_kan::game::{
	Action, ActionType, AiConfig, AiPlayer, BotType, Game, GameState, Player, RuleSet, Rules,
};
use std::time::Instant;

#[derive(Debug, Clone, Copy, Default)]
struct ActionStats {
	adds: usize,
	moves: usize,
	moves_with_rotation: usize,
	captures: usize,
}

impl ActionStats {
	fn add_action(&mut self, action: Action, tiles_before: usize, tiles_after: usize) {
		match action.action_type {
			ActionType::Add { .. } => self.adds += 1,
			ActionType::Move { rotation_count, .. } => {
				self.moves += 1;
				if rotation_count > 0 {
					self.moves_with_rotation += 1;
				}
			}
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

#[derive(Debug, Clone)]
struct MatchResult {
	first_player_won: bool,
	is_draw: bool,
	turns: usize,
	reason: EndReason,
	p1_actions: ActionStats,
	p2_actions: ActionStats,
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
	no_legal_moves_count: usize,
	repetition_draw_count: usize,
	turn_limit_draw_count: usize,
	total_moves: usize,
	total_captures: usize,
	unique_outcomes: std::collections::HashSet<u64>,
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
			no_legal_moves_count: 0,
			repetition_draw_count: 0,
			turn_limit_draw_count: 0,
			total_moves: 0,
			total_captures: 0,
			unique_outcomes: std::collections::HashSet::new(),
		}
	}

	fn first_seat_win_rate(&self) -> f64 {
		let decided_games = self.first_seat_wins + self.second_seat_wins;
		if decided_games == 0 {
			0.0
		} else {
			self.first_seat_wins as f64 / decided_games as f64
		}
	}

	fn first_seat_win_rate_with_draws(&self) -> f64 {
		if self.total_games == 0 {
			0.0
		} else {
			self.first_seat_wins as f64 / self.total_games as f64
		}
	}

	fn avg_turns(&self) -> f64 {
		if self.total_games == 0 {
			0.0
		} else {
			self.total_turns as f64 / self.total_games as f64
		}
	}

	fn avg_captures_per_game(&self) -> f64 {
		if self.total_games == 0 {
			0.0
		} else {
			self.total_captures as f64 / self.total_games as f64
		}
	}
}

fn play_game(p1: &AiPlayer, p2: &AiPlayer, rule_set: RuleSet, _base_seed: u64) -> MatchResult {
	let mut game = Game::new_with_rules(rule_set);
	let mut p1_actions = ActionStats::default();
	let mut p2_actions = ActionStats::default();

	while game.turn_count < 100 {
		let current_player = game.current_player;
		let ai = if current_player == Player::One { p1 } else { p2 };
		let tiles_before = game.board.tiles().count();

		if let Some(action) = ai.choose_action(&game) {
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
			let reason = EndReason::NoLegalMoves;
			return MatchResult {
				first_player_won: current_player == Player::Two,
				is_draw: false,
				turns: game.turn_count,
				reason,
				p1_actions,
				p2_actions,
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

	for i in 0..games_per_side {
		let seed = base_seed.wrapping_mul(1000).wrapping_add(i as u64);

		let p1 =
			AiPlayer::new_seeded(AiConfig { depth, bot_type: bot1_type, time_budget: None }, seed);
		let p2 = AiPlayer::new_seeded(
			AiConfig { depth, bot_type: bot2_type, time_budget: None },
			seed.wrapping_add(1),
		);

		let result = play_game(&p1, &p2, rule_set, seed);

		stats.total_games += 1;
		stats.total_turns += result.turns;
		match result.reason {
			EndReason::NoLegalMoves => stats.no_legal_moves_count += 1,
			EndReason::RepetitionDraw => stats.repetition_draw_count += 1,
			EndReason::TurnLimitDraw => stats.turn_limit_draw_count += 1,
		}

		stats.total_moves += result.p1_actions.moves + result.p2_actions.moves;
		stats.total_captures += result.p1_actions.captures + result.p2_actions.captures;

		let outcome_hash =
			result.turns as u64 * 1000 + if result.is_draw {
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

		let p1 =
			AiPlayer::new_seeded(AiConfig { depth, bot_type: bot2_type, time_budget: None }, seed);
		let p2 = AiPlayer::new_seeded(
			AiConfig { depth, bot_type: bot1_type, time_budget: None },
			seed.wrapping_add(1),
		);

		let result = play_game(&p1, &p2, rule_set, seed);

		stats.total_games += 1;
		stats.total_turns += result.turns;
		match result.reason {
			EndReason::NoLegalMoves => stats.no_legal_moves_count += 1,
			EndReason::RepetitionDraw => stats.repetition_draw_count += 1,
			EndReason::TurnLimitDraw => stats.turn_limit_draw_count += 1,
		}

		stats.total_moves += result.p1_actions.moves + result.p2_actions.moves;
		stats.total_captures += result.p1_actions.captures + result.p2_actions.captures;

		let outcome_hash =
			result.turns as u64 * 1000 + if result.is_draw {
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

fn print_variant_results(variant_name: &str, all_pairings: &[PairingStats], elapsed: f64) {
	println!("\n{}", "=".repeat(60));
	println!("Variant: {}", variant_name);
	println!("{}", "=".repeat(60));
	println!(
		"\n{:<12} {:<12} {:>8} {:>8} {:>6} | {:>8} {:>8} {:>8}",
		"Bot1", "Bot2", "1st Wins", "2nd Wins", "Draws", "Avg Turn", "MOVE%", "Cap/Gm"
	);
	println!("{}", "-".repeat(80));

	for stats in all_pairings {
		let move_pct = if stats.total_moves + stats.total_games * 11 > 0 {
			(stats.total_moves as f64 / (stats.total_moves + stats.total_games * 11) as f64) * 100.0
		} else {
			0.0
		};

		println!(
			"{:<12} {:<12} {:>9} {:>9} {:>6} | {:>8.1} {:>7.1}% {:>8.1} {:>8}",
			stats.p1_name,
			stats.p2_name,
			stats.first_seat_wins,
			stats.second_seat_wins,
			stats.draws,
			stats.avg_turns(),
			move_pct,
			stats.avg_captures_per_game(),
			stats.unique_outcomes.len()
		);
	}

	println!("\nFirst-Seat Win Rates (by pairing):");
	for stats in all_pairings {
		let fwr_excl = stats.first_seat_win_rate();
		let fwr_incl = stats.first_seat_win_rate_with_draws();
		let decided = stats.first_seat_wins + stats.second_seat_wins;
		let stderr = if decided > 0 {
			((fwr_excl * (1.0 - fwr_excl)) / decided as f64).sqrt()
		} else {
			0.0
		};
		let ci_95 = 1.96 * stderr;
		println!(
			"  {} vs {}: {:.1}% excl draws [{:.1}%, {:.1}%], {:.1}% incl draws | {} unique outcomes",
			stats.p1_name,
			stats.p2_name,
			fwr_excl * 100.0,
			(fwr_excl - ci_95).max(0.0) * 100.0,
			(fwr_excl + ci_95).min(1.0) * 100.0,
			fwr_incl * 100.0,
			stats.unique_outcomes.len()
		);
	}

	println!("\nEnd Reasons:");
	let total_games: usize = all_pairings.iter().map(|s| s.total_games).sum();
	let no_legal: usize = all_pairings.iter().map(|s| s.no_legal_moves_count).sum();
	let rep_draw: usize = all_pairings.iter().map(|s| s.repetition_draw_count).sum();
	let turn_draw: usize = all_pairings.iter().map(|s| s.turn_limit_draw_count).sum();
	println!(
		"  No legal moves: {} ({:.1}%)",
		no_legal,
		(no_legal as f64 / total_games as f64) * 100.0
	);
	println!(
		"  Repetition draw: {} ({:.1}%)",
		rep_draw,
		(rep_draw as f64 / total_games as f64) * 100.0
	);
	println!(
		"  100-turn draw: {} ({:.1}%)",
		turn_draw,
		(turn_draw as f64 / total_games as f64) * 100.0
	);

	println!("\nRuntime: {:.2}s for {} games", elapsed, total_games);
}

fn main() {
	println!("=== Apa Kan 4-Variant Benchmark ===\n");
	println!("Comparing rule variants:");
	println!("  (1) Old rules: no fixed rotation, no ADD bank, no immunity");
	println!("  (2) A only: fixed rotation direction");
	println!("  (3) A+B: fixed rotation + ADD bank");
	println!("  (4) A+B+C: fixed rotation + ADD bank + moved-tile immunity\n");

	let depth = 2;
	let games_per_side = 50;
	let base_seed = 42;
	let bots = [BotType::Ore, BotType::Olusare, BotType::Ologbon];

	let variants = [
		("Old Rules", RuleSet::old()),
		(
			"A Only (Fixed Rotation)",
			RuleSet { fixed_rotation: true, add_bank: false, moved_tile_immunity: false },
		),
		("A+B (Rotation + Bank)", RuleSet::new()),
		("A+B+C (All Rules)", RuleSet::with_immunity()),
	];

	for (variant_name, rule_set) in &variants {
		let start = Instant::now();
		let mut all_pairings = Vec::new();

		for (i, &bot1) in bots.iter().enumerate() {
			for &bot2 in bots.iter().skip(i) {
				let stats = run_pairing(bot1, bot2, depth, games_per_side, *rule_set, base_seed);
				all_pairings.push(stats);
			}
		}

		let elapsed = start.elapsed().as_secs_f64();
		print_variant_results(variant_name, &all_pairings, elapsed);
	}

	println!("\n\n=== Summary ===");
	println!("Depth: {}", depth);
	println!("Games per pairing: {} per side ({} total)", games_per_side, games_per_side * 2);
	println!("Total games across all variants: {}", 4 * 6 * games_per_side * 2);
}
