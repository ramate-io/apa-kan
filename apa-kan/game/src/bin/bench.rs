//! Headless round-robin of random, greedy, minimax, and MCTS.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use apa_kan::ai::{self, Actor};
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
	bots: Vec<Actor>,
	rules: RuleSet,
	label: String,
}

fn usage() -> &'static str {
	"Usage: apa-kan-bench [OPTIONS]\n\
	 \n\
	 Options:\n\
	   --games <n>       Games per pairing per seat (default: 20)\n\
	   --think-ms <n>    Bot think time in ms (default: 20)\n\
	   --seed <u64>      Base seed (default: 1)\n\
	   --bots <list>     Comma-separated actors (default: random,greedy,minimax,mcts)\n\
	   --out <path>      Output log path\n\
	   --label <text>    Header label written into the log\n\
	   Rule flags (same as apa-kan): --no-moves-loses, --center-hold-win,\n\
	   --center-hold-n, --no-same-piece-twice, --color-restricted-hops,\n\
	   --hop-weight-cmp, --path-constraint, --no-repeated-edge, --turn-limit\n\
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
		bots: vec![Actor::Random, Actor::Greedy, Actor::Minimax, Actor::Mcts],
		rules: RuleSet::default(),
		label: "round-robin".into(),
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
			"--label" => cfg.label = need("--label", it.next())?,
			"--bots" => {
				cfg.bots = parse_bots(&need("--bots", it.next())?)?;
			}
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
	turn_limit: u32,
	double_pass: u32,
}

fn run(cfg: BenchConfig) {
	let started = Instant::now();
	let rules = cfg.rules;
	let mut lines: Vec<String> = Vec::new();
	lines.push(format!("# apa-kan {}", cfg.label));
	lines.push(format!(
		"# think_ms={} games_per_pairing_per_seat={} seed_base={} turn_limit={}",
		cfg.think.as_millis(),
		cfg.games,
		cfg.seed,
		rules.turn_limit
	));
	lines.push(format!(
		"# rules: no_moves_loses={} center_hold_win={} center_hold_n={} no_same_piece_twice={} color_restricted_hops={} hop_weight_cmp={} path_constraint={}",
		on_off(rules.no_moves_loses),
		on_off(rules.center_hold_win),
		rules.center_hold_n,
		rules.no_same_piece_twice.name(),
		on_off(rules.color_restricted_hops),
		rules.hop_weight_cmp.name(),
		rules.path_constraint.name()
	));
	lines.push(
		"# Each row is bot1 as P1 (light) vs bot2 as P2 (dark). W-D-L is from P1's view.".into(),
	);
	lines.push(
		"# pairing  seat  W-D-L  avg_turns  elim  locked_out  center_hold  turn_limit".into(),
	);

	let mut pairing_index = 0u64;
	let mut double_pass_total = 0u32;
	for &p1 in &cfg.bots {
		for &p2 in &cfg.bots {
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
				"{:<8} vs {:<8}  P1  {}-{}-{}  {:.2}  {}  {}  {}  {}",
				p1.name(),
				p2.name(),
				stats.wins,
				stats.draws,
				stats.losses,
				avg,
				stats.elim,
				stats.locked_out,
				stats.center_hold,
				stats.turn_limit
			);
			println!("{line}");
			lines.push(line);
			double_pass_total += stats.double_pass;
			pairing_index += 1;
		}
	}

	lines.push(format!(
		"# elapsed_sec={:.1} double_pass={}",
		started.elapsed().as_secs_f64(),
		double_pass_total
	));
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

fn on_off(v: bool) -> &'static str {
	if v {
		"on"
	} else {
		"off"
	}
}

fn tally_reason(stats: &mut SeatStats, reason: EndReason) {
	match reason {
		EndReason::Elimination => stats.elim += 1,
		EndReason::TurnLimit => stats.turn_limit += 1,
		EndReason::DoublePass => stats.double_pass += 1,
		EndReason::LockedOut => stats.locked_out += 1,
		EndReason::CenterHold => stats.center_hold += 1,
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
