//! Headless round-robin of random, greedy, minimax, and MCTS.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use apa_kan::ai::{self, Actor};
use apa_kan::game::{Action, EndReason, Game, GameStatus, Player, RuleSet};
use rand::rngs::StdRng;
use rand::SeedableRng;

const BOTS: [Actor; 4] = [Actor::Random, Actor::Greedy, Actor::Minimax, Actor::Mcts];

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
}

fn usage() -> &'static str {
	"Usage: apa-kan-bench [OPTIONS]\n\
	 \n\
	 Options:\n\
	   --games <n>       Games per pairing per seat (default: 20)\n\
	   --think-ms <n>    Bot think time in ms (default: 20)\n\
	   --seed <u64>      Base seed (default: 1)\n\
	   --out <path>      Output log path\n\
	   -h, --help\n"
}

fn parse<I, S>(args: I) -> Result<BenchConfig, String>
where
	I: IntoIterator<Item = S>,
	S: AsRef<str>,
{
	let mut cfg = BenchConfig {
		games: 20,
		think: Duration::from_millis(20),
		seed: 1,
		out: Path::new(env!("CARGO_MANIFEST_DIR")).join("benchmark_results/round_robin.txt"),
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
			"--out" => cfg.out = PathBuf::from(need("--out", it.next())?),
			other => return Err(format!("unknown argument '{other}'")),
		}
	}
	Ok(cfg)
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
	turn_limit: u32,
	double_pass: u32,
}

fn run(cfg: BenchConfig) {
	let started = Instant::now();
	let rules = RuleSet::default();
	let mut lines: Vec<String> = Vec::new();
	lines.push("# apa-kan round-robin".into());
	lines.push(format!(
		"# think_ms={} games_per_pairing_per_seat={} seed_base={} turn_limit={}",
		cfg.think.as_millis(),
		cfg.games,
		cfg.seed,
		rules.turn_limit
	));
	lines.push(
		"# Each row is bot1 as P1 (light) vs bot2 as P2 (dark). W-D-L is from P1's view.".into(),
	);
	lines.push("# pairing  seat  W-D-L  avg_turns  elim  turn_limit  double_pass".into());

	let mut pairing_index = 0u64;
	for &p1 in &BOTS {
		for &p2 in &BOTS {
			let mut stats = SeatStats::default();
			for g in 0..cfg.games {
				let seed = cfg
					.seed
					.wrapping_add(pairing_index.wrapping_mul(10_000))
					.wrapping_add(u64::from(g));
				let result = play_game(rules, seed, p1, p2, cfg.think);
				stats.turns += result.turns;
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
			}
			let n = cfg.games.max(1);
			let avg = f64::from(stats.turns) / f64::from(n);
			let line = format!(
				"{:<8} vs {:<8}  P1  {}-{}-{}  {:.2}  {}  {}  {}",
				p1.name(),
				p2.name(),
				stats.wins,
				stats.draws,
				stats.losses,
				avg,
				stats.elim,
				stats.turn_limit,
				stats.double_pass
			);
			println!("{line}");
			lines.push(line);
			pairing_index += 1;
		}
	}

	lines.push(format!("# elapsed_sec={:.1}", started.elapsed().as_secs_f64()));
	if let Some(parent) = cfg.out.parent() {
		let _ = fs::create_dir_all(parent);
	}
	let body = lines.join("\n") + "\n";
	if let Err(err) = fs::write(&cfg.out, &body) {
		eprintln!("failed to write {}: {err}", cfg.out.display());
		std::process::exit(1);
	}
	println!("wrote {}", cfg.out.display());
}

fn tally_reason(stats: &mut SeatStats, reason: EndReason) {
	match reason {
		EndReason::Elimination => stats.elim += 1,
		EndReason::TurnLimit => stats.turn_limit += 1,
		EndReason::DoublePass => stats.double_pass += 1,
	}
}

struct PlayResult {
	status: GameStatus,
	turns: u32,
}

fn play_game(rules: RuleSet, seed: u64, p1: Actor, p2: Actor, think: Duration) -> PlayResult {
	let mut game = Game::new(rules, seed);
	let mut rng = StdRng::seed_from_u64(seed ^ 0x51ed);
	let mut guard = 0u32;
	while !game.status().is_over() {
		guard += 1;
		if guard > rules.turn_limit.saturating_mul(4) + 8 {
			break;
		}
		let actor = match game.current_player() {
			Player::One => p1,
			Player::Two => p2,
		};
		let action = ai::choose(&game, actor, &mut rng, think);
		if game.apply(action).is_err() {
			if let Some(fallback) = game.legal_actions().into_iter().next() {
				let _ = game.apply(fallback);
			} else {
				let _ = game.apply(Action::Pass);
			}
			break;
		}
	}
	PlayResult { status: game.status(), turns: game.turn_count() }
}
