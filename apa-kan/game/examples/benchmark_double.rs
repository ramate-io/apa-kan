//! Quick two-action scored benchmark: Olusare / Ologbon mirrors and cross, 5x5 and 7x7.

use apa_kan::game::{
	ActionType, AiConfig, AiPlayer, BotType, Game, GameState, Player, RuleSet, Rules,
};
use std::fs;
use std::io::Write as _;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

#[derive(Clone, Default)]
struct Pairing {
	label: String,
	p1_wins: usize,
	draws: usize,
	p2_wins: usize,
	moves: usize,
	adds: usize,
	turns: usize,
	games: usize,
	double_pass: usize,
	turn_limit: usize,
	elimination: usize,
	other_win: usize,
	unfinished: usize,
}

impl Pairing {
	fn record(&mut self, game: &Game, adds: usize, moves: usize) {
		self.games += 1;
		self.turns += game.turn_count;
		self.adds += adds;
		self.moves += moves;
		match game.state {
			GameState::Won(Player::One) => self.p1_wins += 1,
			GameState::Won(Player::Two) => self.p2_wins += 1,
			GameState::Draw => self.draws += 1,
			GameState::InProgress => self.unfinished += 1,
		}
		if game.consecutive_passes >= 2 {
			self.double_pass += 1;
		} else if game.turn_count >= 100 {
			self.turn_limit += 1;
		} else if matches!(game.state, GameState::Won(_)) {
			let p1 = game.board.tiles().filter(|(_, t)| t.owner == Player::One).count();
			let p2 = game.board.tiles().filter(|(_, t)| t.owner == Player::Two).count();
			if p1 == 0 || p2 == 0 {
				self.elimination += 1;
			} else {
				self.other_win += 1;
			}
		}
	}

	fn move_share(&self) -> f64 {
		let total = self.adds + self.moves;
		if total == 0 {
			0.0
		} else {
			100.0 * self.moves as f64 / total as f64
		}
	}

	fn avg_turns(&self) -> f64 {
		if self.games == 0 {
			0.0
		} else {
			self.turns as f64 / self.games as f64
		}
	}

	fn summary_line(&self) -> String {
		format!(
			"{}  games {}  P1 {}-{}-{} P2  MOVE {:.1}%  avg turns {:.1}  ends: double_pass {}  turn_limit {}  elimination {}  other_win {}  unfinished {}",
			self.label,
			self.games,
			self.p1_wins,
			self.draws,
			self.p2_wins,
			self.move_share(),
			self.avg_turns(),
			self.double_pass,
			self.turn_limit,
			self.elimination,
			self.other_win,
			self.unfinished
		)
	}
}

fn ai(bot: BotType, seed: u64, think: Duration) -> AiPlayer {
	AiPlayer::new_seeded(
		AiConfig {
			depth: 3,
			bot_type: bot,
			time_budget: Some(think),
			use_transposition_table: true,
			use_move_ordering: true,
			mcts_iterations: None,
			mcts_exploration: 1.41,
			mcts_rollout_depth: 12,
		},
		seed,
	)
}

fn play(
	p1_bot: BotType,
	p2_bot: BotType,
	rules: RuleSet,
	think: Duration,
	seed: u64,
) -> (Game, usize, usize) {
	let p1 = ai(p1_bot, seed, think);
	let p2 = ai(p2_bot, seed.wrapping_add(7919), think);
	let mut game = Game::new_with_rules(rules);
	let mut adds = 0usize;
	let mut moves = 0usize;
	let mut guard = 0usize;
	while !game.is_game_over() && game.turn_count < 100 && guard < 400 {
		guard += 1;
		let bot = if game.current_player == Player::One { &p1 } else { &p2 };
		let Some(action) = bot.choose_action(&game) else {
			break;
		};
		match action.action_type {
			ActionType::Add { .. } => adds += 1,
			ActionType::Move { .. } => moves += 1,
			ActionType::Pass => {}
		}
		if Rules::apply_action(&mut game, action).is_err() {
			break;
		}
	}
	(game, adds, moves)
}

fn run_pairing(
	name: &str,
	bot_a: BotType,
	bot_b: BotType,
	rules: RuleSet,
	games_per_seat: usize,
	think: Duration,
	base_seed: u64,
) -> Pairing {
	let mut stats = Pairing { label: name.to_string(), ..Pairing::default() };
	for i in 0..games_per_seat {
		let (game, adds, moves) = play(bot_a, bot_b, rules, think, base_seed + i as u64);
		stats.record(&game, adds, moves);
	}
	for i in 0..games_per_seat {
		let (game, adds, moves) = play(bot_b, bot_a, rules, think, base_seed + 10_000 + i as u64);
		stats.record(&game, adds, moves);
	}
	stats
}

fn main() {
	let start = Instant::now();
	let think = Duration::from_millis(50);
	let games_per_seat = 10;
	let base_seed = 42u64;

	let jobs: Vec<(String, BotType, BotType, RuleSet)> = vec![
		(
			"5x5 Olusare vs Olusare".into(),
			BotType::Olusare,
			BotType::Olusare,
			RuleSet::scored_double(),
		),
		(
			"5x5 Ologbon vs Ologbon".into(),
			BotType::Ologbon,
			BotType::Ologbon,
			RuleSet::scored_double(),
		),
		(
			"5x5 Olusare vs Ologbon".into(),
			BotType::Olusare,
			BotType::Ologbon,
			RuleSet::scored_double(),
		),
		(
			"7x7 Olusare vs Olusare".into(),
			BotType::Olusare,
			BotType::Olusare,
			RuleSet::scored_7x7_double(),
		),
		(
			"7x7 Ologbon vs Ologbon".into(),
			BotType::Ologbon,
			BotType::Ologbon,
			RuleSet::scored_7x7_double(),
		),
		(
			"7x7 Olusare vs Ologbon".into(),
			BotType::Olusare,
			BotType::Ologbon,
			RuleSet::scored_7x7_double(),
		),
	];

	println!("Two-action scored benchmark");
	println!(
		"think {}ms/action, {} games/seat ({} games/pairing), opening handicap on",
		think.as_millis(),
		games_per_seat,
		games_per_seat * 2
	);
	println!("P1 W-D-L is P1 wins-draws-P2 wins across both seats (first listed bot is P1 for the first {} games).", games_per_seat);
	println!();

	let collected: Arc<Mutex<Vec<Pairing>>> = Arc::new(Mutex::new(Vec::new()));
	let mut handles = Vec::new();
	for (i, (name, a, b, rules)) in jobs.into_iter().enumerate() {
		let collected = Arc::clone(&collected);
		let handle = thread::spawn(move || {
			let stats = run_pairing(
				&name,
				a,
				b,
				rules,
				games_per_seat,
				think,
				base_seed + (i as u64) * 100_000,
			);
			println!("{}", stats.summary_line());
			collected.lock().expect("lock").push(stats);
		});
		handles.push(handle);
	}
	for handle in handles {
		handle.join().expect("thread");
	}

	let mut results = collected.lock().expect("lock").clone();
	results.sort_by(|a, b| a.label.cmp(&b.label));

	let out = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
		.join("benchmark_results")
		.join("benchmark_double.txt");
	fs::create_dir_all(out.parent().expect("parent")).expect("mkdir");
	let mut file = fs::File::create(&out).expect("write log");
	writeln!(file, "Two-action scored benchmark").unwrap();
	writeln!(
		file,
		"think {}ms/action, {} games/seat ({} games/pairing), opening handicap on",
		think.as_millis(),
		games_per_seat,
		games_per_seat * 2
	)
	.unwrap();
	writeln!(file, "Seat format: P1 <P1-wins>-<draws>-<P2-wins> P2").unwrap();
	writeln!(
		file,
		"First listed bot sits P1 for the first {} games, then seats swap.",
		games_per_seat
	)
	.unwrap();
	writeln!(file).unwrap();
	for stats in &results {
		writeln!(file, "{}", stats.summary_line()).unwrap();
	}
	writeln!(file).unwrap();
	writeln!(file, "Total time: {:.2}s", start.elapsed().as_secs_f64()).unwrap();
	println!("\nTotal time: {:.2}s", start.elapsed().as_secs_f64());
	println!("Wrote {}", out.display());
}
