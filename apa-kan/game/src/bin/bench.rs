//! Head-to-head search benchmark: branching, stalling, minimax vs MCTS.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use apa_kan::ai::{self, Actor, BranchingReport, Searcher};
use apa_kan::config::apply_rule_flag;
use apa_kan::game::{Action, EndReason, Game, GameStatus, Player, RuleSet};
use rand::rngs::StdRng;
use rand::SeedableRng;

fn main() {
	match parse(std::env::args().skip(1)) {
		Ok(cfg) => run(cfg),
		Err(err) => {
			eprintln!("{err}\n\n{}", usage());
			std::process::exit(1);
		}
	}
}

struct BenchConfig {
	games: u32,
	think: Duration,
	seed: u64,
	out: PathBuf,
	record: PathBuf,
	measure_games: u32,
	bots: Vec<Actor>,
	rules: RuleSet,
	label: String,
	depth_probe: bool,
}

fn usage() -> &'static str {
	"Usage: apa-kan-bench [OPTIONS]\n\
	 \n\
	 Options:\n\
	   --games <n>          Games per pairing per seat (default: 6)\n\
	   --think-ms <n>       Bot think time in ms (default: 1000)\n\
	   --seed <u64>         Base seed (default: 1)\n\
	   --bots <list>        Comma-separated actors (default: greedy,minimax,mcts)\n\
	   --depth-probe        Also time minimax ID depth at 1000 ms and 3000 ms\n\
	   --measure-games <n>  Games for branching sample (default: 12)\n\
	   --out <path>         Output log path\n\
	   --record <path>      Full minimax vs mcts game record\n\
	   --label <text>       Header label written into the log\n\
	   Rule flags (same as apa-kan): --no-moves-loses, --center-hold-win,\n\
	   --center-hold-n, --no-same-piece-twice, --color-restricted-hops,\n\
	   --hop-weight-cmp, --petal-majority-win, --petal-majority-n,\n\
	   --path-constraint, --no-repeated-edge, --turn-limit\n\
	   -h, --help\n"
}

fn parse<I, S>(args: I) -> Result<BenchConfig, String>
where
	I: IntoIterator<Item = S>,
	S: AsRef<str>,
{
	let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("benchmark_results");
	let mut cfg = BenchConfig {
		games: 6,
		think: Duration::from_millis(1000),
		seed: 1,
		out: root.join("search_h2h.txt"),
		record: root.join("minimax_vs_mcts_game.txt"),
		measure_games: 12,
		bots: vec![Actor::Greedy, Actor::Minimax, Actor::Mcts],
		rules: RuleSet::default(),
		label: "search head-to-head".into(),
		depth_probe: true,
	};
	let mut it = args.into_iter();
	while let Some(raw) = it.next() {
		let arg = raw.as_ref();
		match arg {
			"-h" | "--help" => {
				print!("{}", usage());
				std::process::exit(0);
			}
			"--games" => {
				cfg.games = parse_u64("--games", &need("--games", it.next())?)? as u32;
			}
			"--think-ms" => {
				cfg.think = Duration::from_millis(parse_u64(
					"--think-ms",
					&need("--think-ms", it.next())?,
				)?);
			}
			"--seed" => cfg.seed = parse_u64("--seed", &need("--seed", it.next())?)?,
			"--measure-games" => {
				cfg.measure_games =
					parse_u64("--measure-games", &need("--measure-games", it.next())?)? as u32;
			}
			"--out" => cfg.out = PathBuf::from(need("--out", it.next())?),
			"--record" => cfg.record = PathBuf::from(need("--record", it.next())?),
			"--label" => cfg.label = need("--label", it.next())?,
			"--bots" => cfg.bots = parse_bots(&need("--bots", it.next())?)?,
			"--depth-probe" => cfg.depth_probe = true,
			"--no-depth-probe" => cfg.depth_probe = false,
			"--turn-limit" => {
				let n = parse_u64("--turn-limit", &need("--turn-limit", it.next())?)?;
				if n == 0 {
					return Err("invalid --turn-limit (expected >= 1)".into());
				}
				cfg.rules.turn_limit = n as u32;
			}
			other => {
				if apply_rule_flag(&mut cfg.rules, other, &mut it)? {
					continue;
				}
				return Err(format!("unknown argument '{other}'"));
			}
		}
	}
	if cfg.bots.is_empty() {
		return Err("--bots must name at least one actor".into());
	}
	Ok(cfg)
}

fn parse_bots(s: &str) -> Result<Vec<Actor>, String> {
	let mut out = Vec::new();
	for part in s.split(',') {
		let part = part.trim();
		if part.is_empty() {
			continue;
		}
		out.push(Actor::parse(part)?);
	}
	Ok(out)
}

fn need(flag: &str, v: Option<impl AsRef<str>>) -> Result<String, String> {
	v.map(|s| s.as_ref().to_string())
		.ok_or_else(|| format!("missing value for {flag}"))
}

fn parse_u64(flag: &str, v: &str) -> Result<u64, String> {
	v.parse().map_err(|_| format!("invalid {flag} '{v}'"))
}

#[derive(Clone, Default)]
struct SeatStats {
	wins: u32,
	draws: u32,
	losses: u32,
	turns: u32,
	elim: u32,
	locked_out: u32,
	center_hold: u32,
	petal_majority: u32,
	turn_limit: u32,
	double_pass: u32,
	games_repeat_ge2: u32,
	games_repeat_ge3: u32,
	games_occ_repeat_ge3: u32,
	sum_max_repeat: u32,
	sum_max_occ_repeat: u32,
	depth_sum: f64,
	depth_n: u32,
	iter_sum: f64,
	iter_n: u32,
}

fn pairings(bots: &[Actor]) -> Vec<(Actor, Actor)> {
	let mut out = Vec::new();
	for &p1 in bots {
		for &p2 in bots {
			out.push((p1, p2));
		}
	}
	out
}

fn on_off(v: bool) -> &'static str {
	if v {
		"on"
	} else {
		"off"
	}
}

fn run(cfg: BenchConfig) {
	let started = Instant::now();
	let rules = cfg.rules;
	let mut lines: Vec<String> = Vec::new();

	println!("measuring branching factor ({} games)...", cfg.measure_games);
	let branching = ai::sample_branching(cfg.measure_games, cfg.seed.wrapping_add(17), rules);
	push_branching(&mut lines, &cfg, &branching, rules.turn_limit);
	if cfg.depth_probe {
		probe_minimax_depths(&mut lines, rules, cfg.seed);
	}

	lines.push(String::new());
	lines.push(format!("# {}", cfg.label));
	lines.push(format!(
		"# think_ms={} games_per_pairing_per_seat={} seed_base={} turn_limit={}",
		cfg.think.as_millis(),
		cfg.games,
		cfg.seed,
		rules.turn_limit
	));
	lines.push(format!(
		"# rules: no_moves_loses={} center_hold_win={} center_hold_n={} no_same_piece_twice={} color_restricted_hops={} hop_weight_cmp={} petal_majority_win={} petal_majority_n={} path_constraint={}",
		on_off(rules.no_moves_loses),
		on_off(rules.center_hold_win),
		rules.center_hold_n,
		rules.no_same_piece_twice.name(),
		on_off(rules.color_restricted_hops),
		rules.hop_weight_cmp.name(),
		on_off(rules.petal_majority_win),
		rules.petal_majority_n,
		rules.path_constraint.name()
	));
	lines.push(
		"# Each row is bot1 as P1 (light) vs bot2 as P2 (dark). W-D-L is from P1's view.".into(),
	);
	lines.push(
		"# avg_depth is minimax completed ID depth per minimax move; avg_iters is MCTS iterations per MCTS move (not cumulative).".into(),
	);
	lines.push(
		"# pairing  seat  W-D-L  avg_turns  elim  locked_out  center_hold  petal_maj  turn_limit  double_pass  repeats>=3  avg_max_repeat  avg_depth  avg_iters".into(),
	);

	let record_holder: Mutex<Option<GameRecord>> = Mutex::new(None);
	let mut pairing_results: Vec<(usize, SeatStats, String)> = Vec::new();
	let pairs = pairings(&cfg.bots);

	std::thread::scope(|scope| {
		let mut handles = Vec::new();
		for (pi, &(p1, p2)) in pairs.iter().enumerate() {
			let think = cfg.think;
			let games = cfg.games;
			let seed_base = cfg.seed;
			let rec = &record_holder;
			handles.push(scope.spawn(move || {
				let (stats, line) = run_pairing(pi, p1, p2, games, think, seed_base, rules, rec);
				(pi, stats, line)
			}));
		}
		for h in handles {
			match h.join() {
				Ok(row) => pairing_results.push(row),
				Err(_) => {
					eprintln!("pairing thread panicked");
					std::process::exit(1);
				}
			}
		}
	});

	pairing_results.sort_by_key(|r| r.0);
	let mut stall_games = 0u32;
	let mut stall_rep2 = 0u32;
	let mut stall_rep3 = 0u32;
	let mut stall_occ3 = 0u32;
	let mut stall_turn_limit_and_rep3 = 0u32;
	for (_, stats, line) in &pairing_results {
		println!("{line}");
		lines.push(line.clone());
		stall_games += cfg.games;
		stall_rep2 += stats.games_repeat_ge2;
		stall_rep3 += stats.games_repeat_ge3;
		stall_occ3 += stats.games_occ_repeat_ge3;
		stall_turn_limit_and_rep3 += if stats.turn_limit > 0 && stats.games_repeat_ge3 > 0 {
			stats.turn_limit.min(stats.games_repeat_ge3)
		} else {
			0
		};
	}

	lines.push(String::new());
	lines.push("# stalling (rules unchanged; no threefold draw)".into());
	lines.push(format!(
		"# games={}  visible_pos_seen_>=2={}  visible_pos_seen_>=3={}  occupancy_seen_>=3={}",
		stall_games, stall_rep2, stall_rep3, stall_occ3
	));
	lines.push(format!(
		"# games_that_both_hit_turn_limit_and_had_a_visible_repeat_>=3 (lower bound via pairing mins) ~ {}",
		stall_turn_limit_and_rep3
	));
	lines.push(
		"# Visible hash covers occupancy, tile kind, owner, benches, side to move, last-moved tile, moved-this-turn, and center-hold counters.".into(),
	);
	lines.push(
		"# Occupancy hash ignores benches. Repeats are evidence of looping; the rules do not forbid them.".into(),
	);

	lines.push(String::new());
	lines.push("# MCTS bag determinization: remaining bag tiles are a known multiset; each MCTS iteration shuffles both bags (samples a permutation) so capture draws are chance events. Minimax uses the live bag order of the position.".into());
	lines.push("# Minimax: iterative deepening alpha-beta, Zobrist TT, TT/capture/killer/history ordering, quiescence on capture sequences.".into());

	if let Some(parent) = cfg.out.parent() {
		let _ = fs::create_dir_all(parent);
	}

	let record = record_holder.lock().unwrap_or_else(|e| e.into_inner());
	if let Some(rec) = record.as_ref() {
		if let Err(err) = fs::write(&cfg.record, rec.render()) {
			eprintln!("failed to write {}: {err}", cfg.record.display());
			std::process::exit(1);
		}
		lines.push(format!("# game_record={}", cfg.record.display()));
		println!("wrote {}", cfg.record.display());
	} else {
		lines.push("# game_record=(none — minimax vs mcts game did not complete)".into());
	}

	lines.push(format!("# elapsed_sec={:.1}", started.elapsed().as_secs_f64()));
	let body = lines.join("\n") + "\n";
	if let Err(err) = fs::write(&cfg.out, &body) {
		eprintln!("failed to write {}: {err}", cfg.out.display());
		std::process::exit(1);
	}
	println!("wrote {}", cfg.out.display());
}

fn push_branching(
	lines: &mut Vec<String>,
	cfg: &BenchConfig,
	b: &BranchingReport,
	turn_limit: u32,
) {
	lines.push("# apa-kan search benchmark".into());
	lines.push(format!(
		"# think_ms={} games_per_pairing_per_seat={} seed_base={} turn_limit={}",
		cfg.think.as_millis(),
		cfg.games,
		cfg.seed,
		turn_limit
	));
	lines.push(String::new());
	lines.push("# branching factor (legal actions per position, from greedy/random games)".into());
	lines.push(format!(
		"# sample_games={}  (greedy vs random, random vs greedy, greedy vs greedy)",
		b.games
	));
	lines.push("# kind     n_positions  avg    max".into());
	lines.push(format!("ADD       {:<12} {:<6.2} {}", b.add_n, b.add_avg(), b.add_max));
	lines.push(format!("MOVE      {:<12} {:<6.2} {}", b.move_n, b.move_avg(), b.move_max));
	lines.push(format!("ALL       {:<12} {:<6.2} {}", b.all_n, b.all_avg(), b.all_max));
}

fn run_pairing(
	pi: usize,
	p1: Actor,
	p2: Actor,
	games: u32,
	think: Duration,
	seed_base: u64,
	rules: RuleSet,
	record_holder: &Mutex<Option<GameRecord>>,
) -> (SeatStats, String) {
	let mut stats = SeatStats::default();
	for g in 0..games {
		let seed = seed_base
			.wrapping_add((pi as u64).wrapping_mul(10_000))
			.wrapping_add(u64::from(g));
		let want_record = p1 == Actor::Minimax && p2 == Actor::Mcts && g == 0;
		let result = play_game(rules, seed, p1, p2, think, want_record);
		if want_record {
			if let Ok(mut guard) = record_holder.lock() {
				*guard = result.record.clone();
			}
		}
		stats.turns += result.turns;
		stats.depth_sum += result.depth_sum;
		stats.depth_n += result.depth_n;
		stats.iter_sum += result.iter_sum;
		stats.iter_n += result.iter_n;
		if result.max_repeat >= 2 {
			stats.games_repeat_ge2 += 1;
		}
		if result.max_repeat >= 3 {
			stats.games_repeat_ge3 += 1;
		}
		if result.max_occ_repeat >= 3 {
			stats.games_occ_repeat_ge3 += 1;
		}
		stats.sum_max_repeat += result.max_repeat;
		stats.sum_max_occ_repeat += result.max_occ_repeat;
		match result.status {
			GameStatus::Won { winner, reason } => {
				if winner == Player::One {
					stats.wins += 1;
				} else {
					stats.losses += 1;
				}
				tally_reason(&mut stats, reason);
			}
			GameStatus::Draw { reason } => {
				stats.draws += 1;
				tally_reason(&mut stats, reason);
			}
			GameStatus::InProgress => {
				stats.draws += 1;
				stats.turn_limit += 1;
			}
		}
		println!(
			"  {} vs {} game {} -> {:?} turns={} max_rep={} depth={:.1} iters={:.0}",
			p1.name(),
			p2.name(),
			g + 1,
			result.status,
			result.turns,
			result.max_repeat,
			if result.depth_n == 0 { 0.0 } else { result.depth_sum / f64::from(result.depth_n) },
			if result.iter_n == 0 { 0.0 } else { result.iter_sum / f64::from(result.iter_n) }
		);
	}
	let n = games.max(1);
	let avg = f64::from(stats.turns) / f64::from(n);
	let avg_rep = f64::from(stats.sum_max_repeat) / f64::from(n);
	let avg_depth =
		if stats.depth_n == 0 { 0.0 } else { stats.depth_sum / f64::from(stats.depth_n) };
	let avg_iters = if stats.iter_n == 0 { 0.0 } else { stats.iter_sum / f64::from(stats.iter_n) };
	let line = format!(
		"{:<15} vs {:<15}  P1  {}-{}-{}  {:.2}  {}  {}  {}  {}  {}  {}  {}  {:.2}  {:.2}  {:.1}",
		p1.name(),
		p2.name(),
		stats.wins,
		stats.draws,
		stats.losses,
		avg,
		stats.elim,
		stats.locked_out,
		stats.center_hold,
		stats.petal_majority,
		stats.turn_limit,
		stats.double_pass,
		stats.games_repeat_ge3,
		avg_rep,
		avg_depth,
		avg_iters
	);
	(stats, line)
}

fn tally_reason(stats: &mut SeatStats, reason: EndReason) {
	match reason {
		EndReason::Elimination => stats.elim += 1,
		EndReason::TurnLimit => stats.turn_limit += 1,
		EndReason::DoublePass => stats.double_pass += 1,
		EndReason::LockedOut => stats.locked_out += 1,
		EndReason::CenterHold => stats.center_hold += 1,
		EndReason::PetalMajority => stats.petal_majority += 1,
	}
}

fn probe_minimax_depths(lines: &mut Vec<String>, rules: RuleSet, seed: u64) {
	use apa_kan::ai::Searcher;
	lines.push(String::new());
	lines.push("# minimax iterative-deepening depth probe (opening position)".into());
	let game = Game::new(rules, seed);
	for ms in [1000u64, 3000] {
		println!("minimax depth probe {ms} ms...");
		let mut searcher = Searcher::new();
		let mut rng = StdRng::seed_from_u64(seed ^ 0xDE97);
		let _ = searcher.choose(&game, Actor::Minimax, &mut rng, Duration::from_millis(ms));
		let st = searcher.stats();
		lines.push(format!("# think_ms={} completed_depth={} nodes={}", ms, st.depth, st.nodes));
		println!("  depth={} nodes={}", st.depth, st.nodes);
	}
}

#[derive(Clone)]
struct GameRecord {
	p1: Actor,
	p2: Actor,
	seed: u64,
	think_ms: u128,
	lines: Vec<String>,
	status: GameStatus,
	turns: u32,
}

impl GameRecord {
	fn render(&self) -> String {
		let mut out = Vec::new();
		out.push(format!(
			"# minimax vs mcts full game  seed={} think_ms={} P1={} P2={}",
			self.seed,
			self.think_ms,
			self.p1.name(),
			self.p2.name()
		));
		out.extend(self.lines.iter().cloned());
		out.push(format!("# result={:?} turns={}", self.status, self.turns));
		out.push(String::new());
		out.join("\n")
	}
}

struct PlayResult {
	status: GameStatus,
	turns: u32,
	max_repeat: u32,
	max_occ_repeat: u32,
	depth_sum: f64,
	depth_n: u32,
	iter_sum: f64,
	iter_n: u32,
	record: Option<GameRecord>,
}

fn play_game(
	rules: RuleSet,
	seed: u64,
	p1: Actor,
	p2: Actor,
	think: Duration,
	keep_record: bool,
) -> PlayResult {
	let mut game = Game::new(rules, seed);
	let mut rng = StdRng::seed_from_u64(seed ^ 0x51ed);
	let mut s1 = Searcher::new();
	let mut s2 = Searcher::new();
	let mut visible: HashMap<u64, u32> = HashMap::new();
	let mut occupancy: HashMap<u64, u32> = HashMap::new();
	let mut max_repeat = 0u32;
	let mut max_occ = 0u32;
	let mut depth_sum = 0.0;
	let mut depth_n = 0u32;
	let mut iter_sum = 0.0;
	let mut iter_n = 0u32;
	let mut record_lines = Vec::new();
	let mut guard = 0u32;

	loop {
		let vh = ai::visible_hash(&game);
		let oh = ai::occupancy_hash(&game);
		let vc = visible.entry(vh).or_insert(0);
		*vc += 1;
		max_repeat = max_repeat.max(*vc);
		let oc = occupancy.entry(oh).or_insert(0);
		*oc += 1;
		max_occ = max_occ.max(*oc);

		if game.status().is_over() {
			break;
		}
		guard += 1;
		if guard > rules.turn_limit.saturating_mul(4) + 8 {
			break;
		}
		let actor = match game.current_player() {
			Player::One => p1,
			Player::Two => p2,
		};
		let searcher = match game.current_player() {
			Player::One => &mut s1,
			Player::Two => &mut s2,
		};
		let action = searcher.choose(&game, actor, &mut rng, think);
		let st = searcher.stats();
		if actor == Actor::Minimax {
			depth_sum += f64::from(st.depth);
			depth_n += 1;
		}
		if actor == Actor::Mcts {
			iter_sum += f64::from(st.iterations);
			iter_n += 1;
		}
		match game.apply(action) {
			Ok(outcome) => {
				if keep_record {
					record_lines.push(format!(
						"{:>3}. {}   [depth={} iters={}]",
						game.turn_count(),
						outcome.log_line,
						st.depth,
						st.iterations
					));
				}
			}
			Err(_) => {
				if let Some(fallback) = game.legal_actions().into_iter().next() {
					let _ = game.apply(fallback);
				} else {
					let _ = game.apply(Action::Pass);
				}
				break;
			}
		}
		s1.advance(&game);
		s2.advance(&game);
	}

	let record = if keep_record {
		Some(GameRecord {
			p1,
			p2,
			seed,
			think_ms: think.as_millis(),
			lines: record_lines,
			status: game.status(),
			turns: game.turn_count(),
		})
	} else {
		None
	};

	PlayResult {
		status: game.status(),
		turns: game.turn_count(),
		max_repeat,
		max_occ_repeat: max_occ,
		depth_sum,
		depth_n,
		iter_sum,
		iter_n,
		record,
	}
}
