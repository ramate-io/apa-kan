//! Head-to-head search benchmark: branching, stalling, minimax vs MCTS.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use apa_kan::ai::{
	self, Actor, BranchingReport, SearchLimit, Searcher, CALIBRATED_ITERS_PER_MOVE,
	CALIBRATED_NODES_PER_MOVE,
};
use apa_kan::bench_log::{self, GameLog};
use apa_kan::config::{apply_rule_flag, validate_rule_flags};
use apa_kan::game::{
	sample_tile, Action, EndReason, Game, GameStatus, LowValueResolution, Player, RuleSet, Tile,
	TileColor,
};
use apa_kan::scenario::{self, DeckStrength, Scenario};
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
	limit: SearchLimit,
	jobs: usize,
	seed: u64,
	out: PathBuf,
	record: PathBuf,
	games_log: Option<PathBuf>,
	measure_games: u32,
	bots: Vec<Actor>,
	rules: RuleSet,
	label: String,
	depth_probe: bool,
	scenario: Option<Scenario>,
	calibrate: bool,
}

fn usage() -> &'static str {
	"Usage: apa-kan-bench [OPTIONS]\n\
	 \n\
	 Options:\n\
	   --games <n>          Games per pairing per seat (default: 6)\n\
	   --budget <spec>      Work budget: nodes:N,iters:M (default: calibrated 1000 ms)\n\
	                        nodes caps minimax nodes/move; iters caps MCTS iterations/move.\n\
	                        Pass `calibrated` for the documented 1000 ms single-thread means.\n\
	   --think-ms <n>       Wall-clock think time in ms (not reproducible). Mutually exclusive with --budget.\n\
	   --jobs <n>           Independent games in parallel (default: available_parallelism).\n\
	                        Each search stays single-threaded.\n\
	   --calibrate          Measure nodes/iters per move at --think-ms (default 1000), jobs=1, then exit\n\
	   --seed <u64>         Base seed (default: 1)\n\
	   --bots <list>        Comma-separated actors (default: greedy,minimax,mcts)\n\
	   --depth-probe        Also time minimax ID depth at 1000 ms and 3000 ms\n\
	   --measure-games <n>  Games for branching sample (default: 12)\n\
	   --out <path>         Output log path\n\
	   --record <path>      Full minimax vs mcts game record\n\
	   --games-log <path>   Per-game JSONL (default: <out>.jsonl.gz; gzip if .gz)\n\
	   --no-games-log       Disable the per-game JSONL (on by default)\n\
	   --label <text>       Header label written into the log\n\
	   --scenario <name|path> Opening deck preset or TOML file\n\
	   --list-scenarios     Print builtin deck scenarios\n\
	   Doubles allowed (--no-same-piece-twice off) is the locked default and the only\n\
	   H2H-benchmarked configuration. Other --no-same-piece-twice values remain as CLI variants.\n\
	   Rule flags (same as apa-kan): --no-moves-loses, --center-hold-win,\n\
	   --center-hold-n, --no-same-piece-twice, --color-restricted-hops,\n\
	   --hop-weight-cmp, --counted-pass-through, --graph-hops, --pass-through-ring-cmp,\n\
	   --petal-majority-win, --petal-majority-n,\n\
	   --control-win, --control-n, --control-hold-turns,\n\
	   --insufficient-tiles-loss, --starting-tokens, --starting-bench, --deck-size, --bag-size, --self-capture-budget,\n\
	   --max-rings, --zag-chance, --zags, --braid-chance, --braids, --zag-mismatch,\n\
	   --braid-fail-mismatch,\n\
	   --path-constraint, --no-repeated-edge, --turn-limit\n\
	   -h, --help\n"
}

fn parse<I, S>(args: I) -> Result<BenchConfig, String>
where
	I: IntoIterator<Item = S>,
	S: AsRef<str>,
{
	let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("benchmark_results");
	let default_out = root.join("search_h2h.txt");
	let mut games_log_set = false;
	let mut think_ms_set = false;
	let mut budget_set = false;
	let mut think = Duration::from_millis(1000);
	let mut budget_nodes = CALIBRATED_NODES_PER_MOVE;
	let mut budget_iters = CALIBRATED_ITERS_PER_MOVE;
	let mut cfg = BenchConfig {
		games: 6,
		limit: SearchLimit::calibrated(),
		jobs: default_jobs(),
		seed: 1,
		out: default_out.clone(),
		record: root.join("minimax_vs_mcts_game.txt"),
		games_log: Some(bench_log::default_games_log_path(&default_out)),
		measure_games: 12,
		bots: vec![Actor::Greedy, Actor::Minimax, Actor::Mcts],
		rules: RuleSet::default(),
		label: "search head-to-head".into(),
		depth_probe: true,
		scenario: None,
		calibrate: false,
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
				think_ms_set = true;
				think = Duration::from_millis(parse_u64(
					"--think-ms",
					&need("--think-ms", it.next())?,
				)?);
			}
			"--budget" => {
				budget_set = true;
				let spec = need("--budget", it.next())?;
				let (nodes, iters) = parse_budget(&spec)?;
				if let Some(n) = nodes {
					budget_nodes = n;
				}
				if let Some(i) = iters {
					budget_iters = i;
				}
			}
			"--jobs" => {
				let n = parse_u64("--jobs", &need("--jobs", it.next())?)?;
				if n == 0 {
					return Err("invalid --jobs (expected >= 1)".into());
				}
				cfg.jobs = n as usize;
			}
			"--calibrate" => cfg.calibrate = true,
			"--seed" => cfg.seed = parse_u64("--seed", &need("--seed", it.next())?)?,
			"--measure-games" => {
				cfg.measure_games =
					parse_u64("--measure-games", &need("--measure-games", it.next())?)? as u32;
			}
			"--out" => {
				cfg.out = PathBuf::from(need("--out", it.next())?);
				if !games_log_set {
					cfg.games_log = Some(bench_log::default_games_log_path(&cfg.out));
				}
			}
			"--record" => cfg.record = PathBuf::from(need("--record", it.next())?),
			"--games-log" => {
				let v = need("--games-log", it.next())?;
				games_log_set = true;
				if v == "off" || v == "none" {
					cfg.games_log = None;
				} else {
					cfg.games_log = Some(PathBuf::from(v));
				}
			}
			"--no-games-log" => {
				games_log_set = true;
				cfg.games_log = None;
			}
			"--label" => cfg.label = need("--label", it.next())?,
			"--scenario" => cfg.scenario = Some(Scenario::load(&need("--scenario", it.next())?)?),
			"--list-scenarios" => {
				print!("{}", Scenario::list_text());
				std::process::exit(0);
			}
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
	if think_ms_set && budget_set {
		return Err("use either --think-ms or --budget, not both".into());
	}
	if cfg.calibrate && budget_set {
		return Err("--calibrate measures wall-clock --think-ms; do not pass --budget".into());
	}
	cfg.limit = if think_ms_set || cfg.calibrate {
		SearchLimit::Time(think)
	} else {
		SearchLimit::work(budget_nodes, budget_iters)
	};
	validate_rule_flags(&cfg.rules)?;
	Ok(cfg)
}

fn default_jobs() -> usize {
	std::thread::available_parallelism().map(|n| n.get()).unwrap_or(1)
}

fn parse_budget(spec: &str) -> Result<(Option<u64>, Option<u32>), String> {
	let spec = spec.trim();
	if spec.is_empty() || spec == "calibrated" || spec == "default" {
		return Ok((Some(CALIBRATED_NODES_PER_MOVE), Some(CALIBRATED_ITERS_PER_MOVE)));
	}
	let mut nodes = None;
	let mut iters = None;
	for part in spec.split(',') {
		let part = part.trim();
		if part.is_empty() {
			continue;
		}
		let (key, val) = part.split_once(':').ok_or_else(|| {
			format!("invalid --budget '{spec}' (expected nodes:N,iters:M or calibrated)")
		})?;
		match key.trim() {
			"nodes" | "node" => {
				let n: u64 =
					val.trim().parse().map_err(|_| format!("invalid --budget nodes '{val}'"))?;
				if n == 0 {
					return Err("invalid --budget nodes (expected >= 1)".into());
				}
				nodes = Some(n);
			}
			"iters" | "iter" | "iterations" => {
				let n: u32 =
					val.trim().parse().map_err(|_| format!("invalid --budget iters '{val}'"))?;
				if n == 0 {
					return Err("invalid --budget iters (expected >= 1)".into());
				}
				iters = Some(n);
			}
			other => {
				return Err(format!("unknown --budget key '{other}' (expected nodes or iters)"));
			}
		}
	}
	if nodes.is_none() && iters.is_none() {
		return Err(format!("invalid --budget '{spec}' (expected nodes:N and/or iters:M)"));
	}
	Ok((nodes, iters))
}

fn run_parallel<T, R, F>(jobs: usize, items: Vec<T>, f: F) -> Vec<R>
where
	T: Send,
	R: Send,
	F: Fn(T) -> R + Sync,
{
	let n = items.len();
	if n == 0 {
		return Vec::new();
	}
	let jobs = jobs.max(1).min(n);
	if jobs == 1 {
		return items.into_iter().map(f).collect();
	}
	let queue = Mutex::new(items.into_iter().enumerate().collect::<Vec<(usize, T)>>());
	let slots: Vec<Mutex<Option<R>>> = (0..n).map(|_| Mutex::new(None)).collect();
	std::thread::scope(|scope| {
		for _ in 0..jobs {
			scope.spawn(|| loop {
				let next = queue.lock().unwrap_or_else(|e| e.into_inner()).pop();
				let Some((i, item)) = next else {
					break;
				};
				let r = f(item);
				*slots[i].lock().unwrap_or_else(|e| e.into_inner()) = Some(r);
			});
		}
	});
	slots
		.into_iter()
		.map(|slot| {
			slot.into_inner().unwrap_or_else(|e| e.into_inner()).unwrap_or_else(|| {
				eprintln!("parallel job produced no result");
				std::process::exit(1);
			})
		})
		.collect()
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
	control: u32,
	insuff: u32,
	low_value: u32,
	low_value_wins: u32,
	low_value_draws: u32,
	low_value_pair0: u32,
	low_value_later: u32,
	low_value_piece_count: u32,
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
	captures: u32,
	self_captures: u32,
	center_captures: u32,
	center_self_captures: u32,
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
	if cfg.calibrate {
		run_calibrate(&cfg);
		return;
	}
	let started = Instant::now();
	let rules = cfg.rules;
	let mut lines: Vec<String> = Vec::new();

	println!("measuring branching factor ({} games)...", cfg.measure_games);
	let branching = ai::sample_branching_on(
		cfg.measure_games,
		cfg.seed.wrapping_add(17),
		rules,
		cfg.scenario.as_ref(),
	);
	push_branching(&mut lines, &cfg, &branching, rules.turn_limit);
	if cfg.depth_probe {
		probe_minimax_depths(&mut lines, rules, cfg.seed, cfg.scenario.as_ref());
	}

	lines.push(String::new());
	lines.push(format!("# {}", cfg.label));
	lines.push(limit_header(&cfg, rules.turn_limit));
	lines.push(format!(
		"# rules: no_moves_loses={} center_hold_win={} center_hold_n={} no_same_piece_twice={} color_restricted_hops={} hop_weight_cmp={} counted_pass_through={} pass_through_ring_cmp={} petal_majority_win={} petal_majority_n={} control_win={} control_n={} control_hold_turns={} insufficient_tiles_loss={} starting_bench={} bag_size={} self_capture_budget={} max_rings={} zag_numer={} braid_numer={} zag_mismatch={} path_constraint={} scenario={}",
		on_off(rules.no_moves_loses),
		on_off(rules.center_hold_win),
		rules.center_hold_n,
		rules.no_same_piece_twice.name(),
		on_off(rules.color_restricted_hops),
		rules.hop_weight_cmp.name(),
		on_off(rules.counted_pass_through),
		rules.pass_through_ring_cmp.name(),
		on_off(rules.petal_majority_win),
		rules.petal_majority_n,
		on_off(rules.control_win),
		rules.control_n,
		rules.control_hold_turns,
		rules.insufficient_tiles_loss.name(),
		rules.starting_bench_size,
		rules.bag_size,
		on_off(rules.self_capture_budget),
		rules.max_rings,
		rules.zag_numer,
		rules.braid_numer,
		rules.zag_mismatch.name(),
		rules.path_constraint.name(),
		cfg.scenario.as_ref().map(|s| s.name.as_str()).unwrap_or("default")
	));
	push_ring_process(&mut lines, rules, cfg.seed);
	lines.push(
		"# Each row is bot1 as P1 (light) vs bot2 as P2 (dark). W-D-L is from P1's view.".into(),
	);
	lines.push(
		"# avg_depth is minimax completed ID depth per minimax move; avg_iters is MCTS iterations per MCTS move (not cumulative).".into(),
	);
	lines.push(
		"# pairing  seat  W-D-L  avg_turns  elim  locked_out  center_hold  petal_maj  control  insuff  low_value  turn_limit  double_pass  repeats>=3  avg_max_repeat  avg_depth  avg_iters  avg_captures  avg_self_captures".into(),
	);

	let keep_log = cfg.games_log.is_some();
	let pairs = pairings(&cfg.bots);
	let stdout = Mutex::new(());
	let mut jobs = Vec::new();
	for (pi, &(p1, p2)) in pairs.iter().enumerate() {
		for g in 0..cfg.games {
			let seed = cfg
				.seed
				.wrapping_add((pi as u64).wrapping_mul(10_000))
				.wrapping_add(u64::from(g));
			jobs.push(GameJob {
				pi,
				g,
				p1,
				p2,
				seed,
				want_record: p1 == Actor::Minimax && p2 == Actor::Mcts && g == 0,
			});
		}
	}
	println!(
		"running {} games across {} pairings with --jobs {} ({})...",
		jobs.len(),
		pairs.len(),
		cfg.jobs,
		cfg.limit.label()
	);
	let scenario = cfg.scenario.clone();
	let limit = cfg.limit;
	let outs = run_parallel(cfg.jobs, jobs, |job| {
		let result = play_game(
			rules,
			job.seed,
			job.p1,
			job.p2,
			limit,
			job.want_record,
			keep_log,
			scenario.as_ref(),
		);
		{
			let _g = stdout.lock().unwrap_or_else(|e| e.into_inner());
			println!(
				"  {} vs {} game {} -> {:?} turns={} max_rep={} depth={:.1} iters={:.0}",
				job.p1.name(),
				job.p2.name(),
				job.g + 1,
				result.status,
				result.turns,
				result.max_repeat,
				if result.depth_n == 0 {
					0.0
				} else {
					result.depth_sum / f64::from(result.depth_n)
				},
				if result.iter_n == 0 { 0.0 } else { result.iter_sum / f64::from(result.iter_n) }
			);
		}
		JobOut { job, result }
	});

	let mut by_pair: Vec<Vec<PlayResult>> = (0..pairs.len()).map(|_| Vec::new()).collect();
	let mut record: Option<GameRecord> = None;
	let mut deck_rows: Vec<DeckRow> = Vec::new();
	let mut logs: Vec<GameLog> = Vec::new();
	for mut out in outs {
		if out.job.want_record {
			record = out.result.record.clone();
		}
		if let Some(mut log) = out.result.log.take() {
			log.game = out.job.g + 1;
			log.pairing = format!("{} vs {}", out.job.p1.name(), out.job.p2.name());
			log.p1 = out.job.p1.name().to_string();
			log.p2 = out.job.p2.name().to_string();
			logs.push(log);
		}
		let mut row = out.result.deck.clone();
		row.pairing = format!("{} vs {}", out.job.p1.name(), out.job.p2.name());
		row.game = out.job.g + 1;
		deck_rows.push(row);
		by_pair[out.job.pi].push(out.result);
	}

	let mut pairing_results: Vec<(usize, SeatStats, String)> = Vec::new();
	for (pi, &(p1, p2)) in pairs.iter().enumerate() {
		let (stats, line) = summarize_pairing(p1, p2, cfg.games, &by_pair[pi]);
		pairing_results.push((pi, stats, line));
	}
	let mut stall_games = 0u32;
	let mut stall_rep2 = 0u32;
	let mut stall_rep3 = 0u32;
	let mut stall_occ3 = 0u32;
	let mut stall_turn_limit_and_rep3 = 0u32;
	let mut p1_w = 0u32;
	let mut p1_d = 0u32;
	let mut p1_l = 0u32;
	let mut end_elim = 0u32;
	let mut end_lock = 0u32;
	let mut end_hold = 0u32;
	let mut end_petal = 0u32;
	let mut end_control = 0u32;
	let mut end_insuff = 0u32;
	let mut end_low_value = 0u32;
	let mut end_low_value_wins = 0u32;
	let mut end_low_value_draws = 0u32;
	let mut end_low_value_pair0 = 0u32;
	let mut end_low_value_later = 0u32;
	let mut end_low_value_piece_count = 0u32;
	let mut end_limit = 0u32;
	let mut end_pass = 0u32;
	let mut captures = 0u32;
	let mut self_captures = 0u32;
	let mut center_captures = 0u32;
	let mut center_self_captures = 0u32;
	for (_, stats, line) in &pairing_results {
		println!("{line}");
		lines.push(line.clone());
		p1_w += stats.wins;
		p1_d += stats.draws;
		p1_l += stats.losses;
		end_elim += stats.elim;
		end_lock += stats.locked_out;
		end_hold += stats.center_hold;
		end_petal += stats.petal_majority;
		end_control += stats.control;
		end_insuff += stats.insuff;
		end_low_value += stats.low_value;
		end_low_value_wins += stats.low_value_wins;
		end_low_value_draws += stats.low_value_draws;
		end_low_value_pair0 += stats.low_value_pair0;
		end_low_value_later += stats.low_value_later;
		end_low_value_piece_count += stats.low_value_piece_count;
		end_limit += stats.turn_limit;
		end_pass += stats.double_pass;
		captures += stats.captures;
		self_captures += stats.self_captures;
		center_captures += stats.center_captures;
		center_self_captures += stats.center_self_captures;
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
	lines.push(format!(
		"# seat_totals  P1 {p1_w}-{p1_d}-{p1_l}  P2 {p1_l}-{p1_d}-{p1_w}  games={}",
		p1_w + p1_d + p1_l
	));
	lines.push(format!(
		"# end_reasons  elim={end_elim} locked_out={end_lock} center_hold={end_hold} petal_maj={end_petal} control={end_control} insuff={end_insuff} low_value={end_low_value} low_value_wins={end_low_value_wins} low_value_draws={end_low_value_draws} turn_limit={end_limit} double_pass={end_pass}"
	));
	lines.push(format!(
		"# low_value_pairs  pair0={end_low_value_pair0} later={end_low_value_later} piece_count={end_low_value_piece_count}"
	));
	println!(
		"# end_reasons  elim={end_elim} locked_out={end_lock} center_hold={end_hold} petal_maj={end_petal} control={end_control} insuff={end_insuff} low_value={end_low_value} low_value_wins={end_low_value_wins} low_value_draws={end_low_value_draws} turn_limit={end_limit} double_pass={end_pass}"
	);
	println!(
		"# low_value_pairs  pair0={end_low_value_pair0} later={end_low_value_later} piece_count={end_low_value_piece_count}"
	);
	let games_n = p1_w + p1_d + p1_l;
	let avg_cap = if games_n == 0 { 0.0 } else { f64::from(captures) / f64::from(games_n) };
	let avg_self = if games_n == 0 { 0.0 } else { f64::from(self_captures) / f64::from(games_n) };
	let avg_center =
		if games_n == 0 { 0.0 } else { f64::from(center_captures) / f64::from(games_n) };
	let avg_center_self =
		if games_n == 0 { 0.0 } else { f64::from(center_self_captures) / f64::from(games_n) };
	lines.push(format!("# captures  opponent_tiles_removed={captures}  avg_per_game={avg_cap:.2}  self_captures={self_captures}  self_avg_per_game={avg_self:.2}"));
	lines.push(format!("# center_captures  opponent_tiles_removed={center_captures}  avg_per_game={avg_center:.2}  self_captures={center_self_captures}  self_avg_per_game={avg_center_self:.2}"));
	println!("# captures  opponent_tiles_removed={captures}  avg_per_game={avg_cap:.2}  self_captures={self_captures}  self_avg_per_game={avg_self:.2}");
	println!("# center_captures  opponent_tiles_removed={center_captures}  avg_per_game={avg_center:.2}  self_captures={center_self_captures}  self_avg_per_game={avg_center_self:.2}");
	println!(
		"# seat_totals  P1 {p1_w}-{p1_d}-{p1_l}  P2 {p1_l}-{p1_d}-{p1_w}  games={}",
		p1_w + p1_d + p1_l
	);

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
		"# Visible hash covers occupancy, tile kind, owner, the public remaining deck, spawn-token count (including surplus), side to move, last-moved tile, moved-this-turn, and hold counters (center, petal, control).".into(),
	);
	lines.push(
		"# Occupancy hash ignores benches. Repeats are evidence of looping; the rules do not forbid them.".into(),
	);

	push_observed_rings(&mut lines, &deck_rows);
	push_deck_fairness(&mut lines, &deck_rows);

	lines.push(String::new());
	lines.push("# MCTS: the remaining deck is public, so bag shuffle is a no-op on default rules. Legacy `capture_draw_on_move` logs still shuffle hidden remainders. Minimax searches the live position.".into());
	lines.push("# Minimax: iterative deepening alpha-beta, Zobrist TT, TT/capture/killer/history ordering, quiescence on capture sequences.".into());

	if let Some(parent) = cfg.out.parent() {
		let _ = fs::create_dir_all(parent);
	}

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

	if let Some(path) = cfg.games_log.as_ref() {
		logs.sort_by(|a, b| a.pairing.cmp(&b.pairing).then(a.game.cmp(&b.game)));
		if let Err(err) = write_games_log(path, &logs) {
			eprintln!("failed to write {}: {err}", path.display());
			std::process::exit(1);
		}
		lines.push(format!("# games_log={}", path.display()));
		println!("wrote {} ({} games, {})", path.display(), logs.len(), games_log_size(path));
	}

	lines.push(format!("# elapsed_sec={:.1}", started.elapsed().as_secs_f64()));
	let body = lines.join("\n") + "\n";
	if let Err(err) = fs::write(&cfg.out, &body) {
		eprintln!("failed to write {}: {err}", cfg.out.display());
		std::process::exit(1);
	}
	println!("wrote {}", cfg.out.display());
}

fn run_calibrate(cfg: &BenchConfig) {
	let think = match cfg.limit {
		SearchLimit::Time(d) => d,
		SearchLimit::Work { .. } => Duration::from_millis(1000),
	};
	let seeds = [cfg.seed, cfg.seed.wrapping_add(17), cfg.seed.wrapping_add(41)];
	let moves_per = 16u32;
	println!(
		"calibrating single-thread {} ms/move ({} games × {moves_per} moves, minimax vs mcts)...",
		think.as_millis(),
		seeds.len()
	);
	let mut node_sum = 0u64;
	let mut node_n = 0u32;
	let mut node_min = u64::MAX;
	let mut node_max = 0u64;
	let mut iter_sum = 0u64;
	let mut iter_n = 0u32;
	let mut iter_min = u32::MAX;
	let mut iter_max = 0u32;
	let mut depth_sum = 0.0;
	for &seed in &seeds {
		let mut game = Game::start(cfg.rules, seed, cfg.scenario.as_ref()).unwrap_or_else(|e| {
			eprintln!("{e}");
			std::process::exit(1);
		});
		let mut rng = StdRng::seed_from_u64(bench_log::search_rng_seed(seed));
		let mut s1 = Searcher::new();
		let mut s2 = Searcher::new();
		for _ in 0..moves_per {
			if game.status().is_over() {
				break;
			}
			let actor = match game.current_player() {
				Player::One => Actor::Minimax,
				Player::Two => Actor::Mcts,
			};
			let searcher = match game.current_player() {
				Player::One => &mut s1,
				Player::Two => &mut s2,
			};
			let action = searcher.choose(&game, actor, &mut rng, think);
			let st = searcher.stats();
			match actor {
				Actor::Minimax => {
					node_sum += st.nodes;
					node_n += 1;
					node_min = node_min.min(st.nodes);
					node_max = node_max.max(st.nodes);
					depth_sum += f64::from(st.depth);
					println!("  seed={seed} minimax depth={} nodes={}", st.depth, st.nodes);
				}
				Actor::Mcts => {
					iter_sum += u64::from(st.iterations);
					iter_n += 1;
					iter_min = iter_min.min(st.iterations);
					iter_max = iter_max.max(st.iterations);
					println!("  seed={seed} mcts iters={}", st.iterations);
				}
				_ => {}
			}
			if game.apply(action).is_err() {
				break;
			}
			s1.advance(&game);
			s2.advance(&game);
		}
	}
	let mean_nodes = if node_n == 0 { 0.0 } else { node_sum as f64 / f64::from(node_n) };
	let mean_iters = if iter_n == 0 { 0.0 } else { iter_sum as f64 / f64::from(iter_n) };
	let mean_depth = if node_n == 0 { 0.0 } else { depth_sum / f64::from(node_n) };
	let rec_nodes = round_calibrated_nodes(mean_nodes);
	let rec_iters = round_calibrated_iters(mean_iters);
	println!(
		"# calibrate think_ms={} games={} moves_per={moves_per} jobs=1",
		think.as_millis(),
		seeds.len()
	);
	println!("# minimax_nodes n={node_n} mean={mean_nodes:.0} min={node_min} max={node_max} mean_depth={mean_depth:.2}");
	println!("# mcts_iters n={iter_n} mean={mean_iters:.0} min={iter_min} max={iter_max}");
	println!("# recommended --budget nodes:{rec_nodes},iters:{rec_iters}");
	println!(
		"# current constants CALIBRATED_NODES_PER_MOVE={CALIBRATED_NODES_PER_MOVE} CALIBRATED_ITERS_PER_MOVE={CALIBRATED_ITERS_PER_MOVE}"
	);
}

fn round_calibrated_nodes(mean: f64) -> u64 {
	if mean < 1.0 {
		return 1;
	}
	let step = if mean >= 10_000.0 {
		1_000.0
	} else if mean >= 1_000.0 {
		100.0
	} else {
		10.0
	};
	((mean / step).round() as u64).saturating_mul(step as u64).max(1)
}

fn round_calibrated_iters(mean: f64) -> u32 {
	if mean < 1.0 {
		return 1;
	}
	let step = if mean >= 1_000.0 { 100.0 } else { 10.0 };
	((mean / step).round() as u32).saturating_mul(step as u32).max(1)
}

fn push_branching(
	lines: &mut Vec<String>,
	cfg: &BenchConfig,
	b: &BranchingReport,
	turn_limit: u32,
) {
	lines.push("# apa-kan search benchmark".into());
	lines.push(limit_header(cfg, turn_limit));
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

fn write_games_log(path: &Path, logs: &[GameLog]) -> Result<(), String> {
	let body = bench_log::encode_jsonl(logs)?;
	if let Some(parent) = path.parent() {
		fs::create_dir_all(parent).map_err(|e| e.to_string())?;
	}
	if bench_log::path_is_gzip(path) {
		use std::io::Write;
		let file = fs::File::create(path).map_err(|e| e.to_string())?;
		let mut enc = flate2::write::GzEncoder::new(file, flate2::Compression::default());
		enc.write_all(body.as_bytes()).map_err(|e| e.to_string())?;
		enc.finish().map_err(|e| e.to_string())?;
	} else {
		fs::write(path, body).map_err(|e| e.to_string())?;
	}
	Ok(())
}

fn games_log_size(path: &Path) -> String {
	match fs::metadata(path) {
		Ok(meta) => {
			let n = meta.len();
			if n >= 1024 * 1024 {
				format!("{:.1} MiB", n as f64 / (1024.0 * 1024.0))
			} else if n >= 1024 {
				format!("{:.1} KiB", n as f64 / 1024.0)
			} else {
				format!("{n} B")
			}
		}
		Err(_) => "size unknown".into(),
	}
}

#[derive(Clone, Copy)]
struct GameJob {
	pi: usize,
	g: u32,
	p1: Actor,
	p2: Actor,
	seed: u64,
	want_record: bool,
}

struct JobOut {
	job: GameJob,
	result: PlayResult,
}

fn limit_header(cfg: &BenchConfig, turn_limit: u32) -> String {
	format!(
		"# {} jobs={} games_per_pairing_per_seat={} seed_base={} turn_limit={}",
		cfg.limit.label(),
		cfg.jobs,
		cfg.games,
		cfg.seed,
		turn_limit
	)
}

fn summarize_pairing(
	p1: Actor,
	p2: Actor,
	games: u32,
	results: &[PlayResult],
) -> (SeatStats, String) {
	let mut stats = SeatStats::default();
	for result in results {
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
		stats.captures += result.captures;
		stats.self_captures += result.self_captures;
		stats.center_captures += result.center_captures;
		stats.center_self_captures += result.center_self_captures;
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
		tally_low_value(&mut stats, result.low_value);
	}
	let n = games.max(1);
	let avg = f64::from(stats.turns) / f64::from(n);
	let avg_rep = f64::from(stats.sum_max_repeat) / f64::from(n);
	let avg_depth =
		if stats.depth_n == 0 { 0.0 } else { stats.depth_sum / f64::from(stats.depth_n) };
	let avg_iters = if stats.iter_n == 0 { 0.0 } else { stats.iter_sum / f64::from(stats.iter_n) };
	let avg_cap = f64::from(stats.captures) / f64::from(n);
	let avg_self = f64::from(stats.self_captures) / f64::from(n);
	let line = format!(
		"{:<15} vs {:<15}  P1  {}-{}-{}  {:.2}  {}  {}  {}  {}  {}  {}  {}  {}  {}  {}  {:.2}  {:.2}  {:.1}  cap={:.2}  self={:.2}",
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
		stats.control,
		stats.insuff,
		stats.low_value,
		stats.turn_limit,
		stats.double_pass,
		stats.games_repeat_ge3,
		avg_rep,
		avg_depth,
		avg_iters,
		avg_cap,
		avg_self
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
		EndReason::Control => stats.control += 1,
		EndReason::InsufficientTiles => stats.insuff += 1,
		EndReason::LowValue => stats.low_value += 1,
	}
}

fn tally_low_value(stats: &mut SeatStats, resolution: Option<LowValueResolution>) {
	match resolution {
		None => {}
		Some(LowValueResolution::Draw) => stats.low_value_draws += 1,
		Some(LowValueResolution::Win { pair, by_piece_count, .. }) => {
			stats.low_value_wins += 1;
			if by_piece_count {
				stats.low_value_piece_count += 1;
			} else if pair == 0 {
				stats.low_value_pair0 += 1;
			} else {
				stats.low_value_later += 1;
			}
		}
	}
}

fn probe_minimax_depths(
	lines: &mut Vec<String>,
	rules: RuleSet,
	seed: u64,
	scenario: Option<&Scenario>,
) {
	use apa_kan::ai::Searcher;
	lines.push(String::new());
	lines.push("# minimax iterative-deepening depth probe (opening position)".into());
	let game = Game::start(rules, seed, scenario).unwrap_or_else(|e| {
		eprintln!("{e}");
		std::process::exit(1);
	});
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
	limit: SearchLimit,
	lines: Vec<String>,
	status: GameStatus,
	turns: u32,
}

impl GameRecord {
	fn render(&self) -> String {
		let mut out = Vec::new();
		out.push(format!(
			"# minimax vs mcts full game  seed={} {} P1={} P2={}",
			self.seed,
			self.limit.label(),
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
	low_value: Option<LowValueResolution>,
	turns: u32,
	max_repeat: u32,
	max_occ_repeat: u32,
	depth_sum: f64,
	depth_n: u32,
	iter_sum: f64,
	iter_n: u32,
	captures: u32,
	self_captures: u32,
	center_captures: u32,
	center_self_captures: u32,
	record: Option<GameRecord>,
	log: Option<GameLog>,
	deck: DeckRow,
}

#[derive(Clone)]
struct DeckRow {
	pairing: String,
	game: u32,
	seed: u64,
	p1_open: DeckStrength,
	p2_open: DeckStrength,
	p1_bench: String,
	p2_bench: String,
	p1_played: DeckStrength,
	p2_played: DeckStrength,
	ring_n: [u32; 4],
	color_n: [u32; 6],
	add_rings: [u32; 4],
	move_rings: [u32; 4],
	board_positions: u32,
	board_tiles: u64,
	board_rings: u64,
	board_weight: u64,
	braid_edge_snaps: u64,
	braid_satisfied_snaps: u64,
	outcome: i8,
	turns: u32,
}

fn play_game(
	rules: RuleSet,
	seed: u64,
	p1: Actor,
	p2: Actor,
	limit: SearchLimit,
	keep_record: bool,
	keep_log: bool,
	scenario: Option<&Scenario>,
) -> PlayResult {
	let mut game = Game::start(rules, seed, scenario).unwrap_or_else(|e| {
		eprintln!("{e}");
		std::process::exit(1);
	});
	let p1_open = DeckStrength::of(game.bench(Player::One));
	let p2_open = DeckStrength::of(game.bench(Player::Two));
	let p1_bench = scenario::format_kind_list(game.bench(Player::One));
	let p2_bench = scenario::format_kind_list(game.bench(Player::Two));
	let mut ring_n = [0u32; 4];
	let mut color_n = [0u32; 6];
	tally_tiles(
		game.bench(Player::One).iter().chain(game.bench(Player::Two)),
		&mut ring_n,
		&mut color_n,
	);
	let mut rng = StdRng::seed_from_u64(bench_log::search_rng_seed(seed));
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
	let mut game_log = keep_log.then(|| {
		let mut log = GameLog::begin(
			0,
			String::new(),
			p1.name(),
			p2.name(),
			limit.think_ms(),
			scenario.map(|s| s.name.as_str()).unwrap_or("default"),
			&game,
		);
		log.budget_nodes = limit.nodes();
		log.budget_iters = limit.iters();
		log
	});
	let mut captures = 0u32;
	let mut self_captures = 0u32;
	let mut center_captures = 0u32;
	let mut center_self_captures = 0u32;
	let center = game.board().center();
	let mut guard = 0u32;
	let mut add_rings = [0u32; 4];
	let mut move_rings = [0u32; 4];
	let mut board_positions = 0u32;
	let mut board_tiles = 0u64;
	let mut board_rings = 0u64;
	let mut board_weight = 0u64;
	let mut braid_edge_snaps = 0u64;
	let mut braid_satisfied_snaps = 0u64;

	loop {
		let vh = ai::visible_hash(&game);
		let oh = ai::occupancy_hash(&game);
		let vc = visible.entry(vh).or_insert(0);
		*vc += 1;
		max_repeat = max_repeat.max(*vc);
		let oc = occupancy.entry(oh).or_insert(0);
		*oc += 1;
		max_occ = max_occ.max(*oc);

		board_positions += 1;
		for (_, t) in game.tiles() {
			board_tiles += 1;
			board_rings += u64::from(t.n_rings);
			board_weight += u64::from(t.weight);
		}
		let (with_braid, satisfied) = game.braid_edge_counts();
		braid_edge_snaps += u64::from(with_braid);
		braid_satisfied_snaps += u64::from(satisfied);

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
		let action = searcher.choose_with(&game, actor, &mut rng, limit);
		let piece = bench_log::piece_for_action(&game, action);
		let st = searcher.stats();
		if actor == Actor::Minimax {
			depth_sum += f64::from(st.depth);
			depth_n += 1;
		}
		if actor == Actor::Mcts {
			iter_sum += f64::from(st.iterations);
			iter_n += 1;
		}
		match action {
			Action::Add { bench_index, .. } => {
				if let Some(t) = game.bench(game.current_player()).get(bench_index) {
					add_rings[t.n_rings.clamp(1, 3) as usize] += 1;
				}
			}
			Action::Move { from, .. } => {
				if let Some(t) = game.tile_at(from) {
					move_rings[t.n_rings.clamp(1, 3) as usize] += 1;
				}
			}
			Action::Pass => {}
		}
		match game.apply(action) {
			Ok(outcome) => {
				captures +=
					outcome.removals.iter().filter(|r| r.tile.owner != outcome.player).count()
						as u32;
				self_captures +=
					outcome.removals.iter().filter(|r| r.tile.owner == outcome.player).count()
						as u32;
				center_captures += outcome
					.removals
					.iter()
					.filter(|r| r.node == center && r.tile.owner != outcome.player)
					.count() as u32;
				center_self_captures += outcome
					.removals
					.iter()
					.filter(|r| r.node == center && r.tile.owner == outcome.player)
					.count() as u32;
				if keep_record {
					record_lines.push(format!(
						"{:>3}. {}   [depth={} iters={}]",
						game.turn_count(),
						outcome.log_line,
						st.depth,
						st.iterations
					));
				}
				if let Some(log) = game_log.as_mut() {
					log.push_action(action, piece, &outcome, &game);
				}
			}
			Err(_) => {
				if let Some(fallback) = game.legal_actions().into_iter().next() {
					let piece = bench_log::piece_for_action(&game, fallback);
					if let Ok(outcome) = game.apply(fallback) {
						if let Some(log) = game_log.as_mut() {
							log.push_action(fallback, piece, &outcome, &game);
						}
					}
				} else {
					let piece = bench_log::piece_for_action(&game, Action::Pass);
					if let Ok(outcome) = game.apply(Action::Pass) {
						if let Some(log) = game_log.as_mut() {
							log.push_action(Action::Pass, piece, &outcome, &game);
						}
					}
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
			limit,
			lines: record_lines,
			status: game.status(),
			turns: game.turn_count(),
		})
	} else {
		None
	};

	let p1_played = {
		let mut v: Vec<_> =
			game.tiles().filter(|(_, t)| t.owner == Player::One).map(|(_, t)| t).collect();
		v.extend_from_slice(game.bench(Player::One));
		DeckStrength::of(&v)
	};
	let p2_played = {
		let mut v: Vec<_> =
			game.tiles().filter(|(_, t)| t.owner == Player::Two).map(|(_, t)| t).collect();
		v.extend_from_slice(game.bench(Player::Two));
		DeckStrength::of(&v)
	};
	let outcome = match game.status() {
		GameStatus::Won { winner: Player::One, .. } => 1,
		GameStatus::Won { winner: Player::Two, .. } => -1,
		_ => 0,
	};
	let low_value = bench_log::low_value_of(&game);
	if let Some(log) = game_log.as_mut() {
		log.finish(&game, low_value);
	}

	PlayResult {
		status: game.status(),
		low_value,
		turns: game.turn_count(),
		max_repeat,
		max_occ_repeat: max_occ,
		depth_sum,
		depth_n,
		iter_sum,
		iter_n,
		captures,
		self_captures,
		center_captures,
		center_self_captures,
		record,
		log: game_log,
		deck: DeckRow {
			pairing: String::new(),
			game: 0,
			seed,
			p1_open,
			p2_open,
			p1_bench,
			p2_bench,
			p1_played,
			p2_played,
			ring_n,
			color_n,
			add_rings,
			move_rings,
			board_positions,
			board_tiles,
			board_rings,
			board_weight,
			braid_edge_snaps,
			braid_satisfied_snaps,
			outcome,
			turns: game.turn_count(),
		},
	}
}

fn pearson(xs: &[f64], ys: &[f64]) -> Option<f64> {
	if xs.len() != ys.len() || xs.len() < 3 {
		return None;
	}
	let n = xs.len() as f64;
	let mx = xs.iter().sum::<f64>() / n;
	let my = ys.iter().sum::<f64>() / n;
	let mut num = 0.0;
	let mut dx = 0.0;
	let mut dy = 0.0;
	for (&x, &y) in xs.iter().zip(ys) {
		let a = x - mx;
		let b = y - my;
		num += a * b;
		dx += a * a;
		dy += b * b;
	}
	if dx < 1e-12 || dy < 1e-12 {
		return None;
	}
	Some(num / (dx.sqrt() * dy.sqrt()))
}

fn fmt_r(v: Option<f64>) -> String {
	match v {
		None => "n/a".into(),
		Some(r) => format!("{r:.3}"),
	}
}

fn tally_tiles<'a, I>(tiles: I, ring_n: &mut [u32; 4], color_n: &mut [u32; 6])
where
	I: IntoIterator<Item = &'a Tile>,
{
	for t in tiles {
		let k = t.n_rings.clamp(1, 3) as usize;
		ring_n[k] += 1;
		for &c in t.ring_colors() {
			color_n[c as usize] += 1;
		}
	}
}

fn push_ring_process(lines: &mut Vec<String>, rules: RuleSet, seed: u64) {
	let mut rng = StdRng::seed_from_u64(seed.wrapping_add(0x52494E47));
	let mut next_id = 1u32;
	let n = 20_000u32;
	let mut ring_n = [0u32; 4];
	let mut color_n = [0u32; 6];
	let mut zag_rings = 0u32;
	let mut braid_rings = 0u32;
	let mut braid_pieces = 0u32;
	let mut rings = 0u32;
	for _ in 0..n {
		let t = sample_tile(
			Player::One,
			rules.max_rings,
			rules.zag_numer,
			rules.braid_numer,
			&mut next_id,
			&mut rng,
		);
		tally_tiles(std::iter::once(&t), &mut ring_n, &mut color_n);
		let k = t.n_rings.clamp(1, 3) as usize;
		zag_rings += u32::from(t.zag_ring_count());
		let b = t.braid_ring_count();
		braid_rings += u32::from(b);
		if b >= 2 {
			braid_pieces += 1;
		}
		rings += k as u32;
	}
	lines.push(format!(
		"# ring_process  sample={n} max_rings={} zag_numer={} braid_numer={}  n1={} n2={} n3={}",
		rules.max_rings, rules.zag_numer, rules.braid_numer, ring_n[1], ring_n[2], ring_n[3]
	));
	if rings > 0 {
		lines.push(format!(
			"# ring_process_zags  rings={rings} zags={zag_rings} rate={:.4}",
			f64::from(zag_rings) / f64::from(rings)
		));
		lines.push(format!(
			"# ring_process_braids  pieces={n} braided={braid_pieces} piece_rate={:.4} rings={rings} braid_rings={braid_rings} ring_rate={:.4}",
			f64::from(braid_pieces) / f64::from(n),
			f64::from(braid_rings) / f64::from(rings)
		));
	}
	let names = TileColor::ALL.map(|c| c.name());
	let mut bits = Vec::new();
	for (i, name) in names.iter().enumerate() {
		bits.push(format!("{name}={}", color_n[i]));
	}
	lines.push(format!("# ring_process_colors  {}", bits.join(" ")));
}

fn push_observed_rings(lines: &mut Vec<String>, rows: &[DeckRow]) {
	let mut ring_n = [0u32; 4];
	let mut color_n = [0u32; 6];
	for r in rows {
		for i in 0..4 {
			ring_n[i] += r.ring_n[i];
		}
		for i in 0..6 {
			color_n[i] += r.color_n[i];
		}
	}
	let tiles = ring_n[1] + ring_n[2] + ring_n[3];
	lines.push(String::new());
	lines.push(format!(
		"# observed_opening_rings  tiles={tiles}  n1={} n2={} n3={}",
		ring_n[1], ring_n[2], ring_n[3]
	));
	let names = TileColor::ALL.map(|c| c.name());
	let mut bits = Vec::new();
	for (i, name) in names.iter().enumerate() {
		bits.push(format!("{name}={}", color_n[i]));
	}
	lines.push(format!("# observed_opening_colors  {}", bits.join(" ")));
	push_play_composition(lines, rows);
}

fn push_play_composition(lines: &mut Vec<String>, rows: &[DeckRow]) {
	let mut add_rings = [0u32; 4];
	let mut move_rings = [0u32; 4];
	let mut board_positions = 0u32;
	let mut board_tiles = 0u64;
	let mut board_rings = 0u64;
	let mut board_weight = 0u64;
	let mut braid_edge_snaps = 0u64;
	let mut braid_satisfied_snaps = 0u64;
	for r in rows {
		for i in 0..4 {
			add_rings[i] += r.add_rings[i];
			move_rings[i] += r.move_rings[i];
		}
		board_positions += r.board_positions;
		board_tiles += r.board_tiles;
		board_rings += r.board_rings;
		board_weight += r.board_weight;
		braid_edge_snaps += r.braid_edge_snaps;
		braid_satisfied_snaps += r.braid_satisfied_snaps;
	}
	let adds = add_rings[1] + add_rings[2] + add_rings[3];
	let moves = move_rings[1] + move_rings[2] + move_rings[3];
	lines.push(format!(
		"# add_rings  tiles={adds}  n1={} n2={} n3={}",
		add_rings[1], add_rings[2], add_rings[3]
	));
	lines.push(format!(
		"# move_rings  tiles={moves}  n1={} n2={} n3={}",
		move_rings[1], move_rings[2], move_rings[3]
	));
	if board_positions > 0 && board_tiles > 0 {
		let pos = f64::from(board_positions);
		let tiles = board_tiles as f64;
		lines.push(format!(
			"# board_sample  positions={} avg_tiles={:.2} avg_rings={:.3} avg_weight={:.3}",
			board_positions,
			tiles / pos,
			board_rings as f64 / tiles,
			board_weight as f64 / tiles
		));
	} else {
		lines.push("# board_sample  positions=0".into());
	}
	if braid_edge_snaps > 0 {
		lines.push(format!(
			"# braid_edges  occupied_with_braid={braid_edge_snaps} satisfied={braid_satisfied_snaps} rate={:.4}",
			braid_satisfied_snaps as f64 / braid_edge_snaps as f64
		));
	} else {
		lines.push("# braid_edges  occupied_with_braid=0 satisfied=0".into());
	}
}

fn push_deck_fairness(lines: &mut Vec<String>, rows: &[DeckRow]) {
	lines.push(String::new());
	lines.push("# deck fairness".into());
	lines.push(format!("# games={}", rows.len()));
	if rows.is_empty() {
		return;
	}
	let equal_open = rows.iter().filter(|r| r.p1_open == r.p2_open).count();
	lines.push(format!("# opening_strength_equal={}/{}", equal_open, rows.len()));
	lines.push(
		"# strength = weight_sum + dots_sum + n_colors on the opening bench (and on tiles that entered play).".into(),
	);

	let outcome: Vec<f64> = rows.iter().map(|r| f64::from(r.outcome)).collect();
	let turns: Vec<f64> = rows.iter().map(|r| f64::from(r.turns)).collect();
	let d_open: Vec<f64> = rows
		.iter()
		.map(|r| f64::from(r.p1_open.combined()) - f64::from(r.p2_open.combined()))
		.collect();
	let d_w: Vec<f64> = rows
		.iter()
		.map(|r| f64::from(r.p1_open.weight_sum) - f64::from(r.p2_open.weight_sum))
		.collect();
	let d_d: Vec<f64> = rows
		.iter()
		.map(|r| f64::from(r.p1_open.dots_sum) - f64::from(r.p2_open.dots_sum))
		.collect();
	let d_c: Vec<f64> = rows
		.iter()
		.map(|r| f64::from(r.p1_open.n_colors) - f64::from(r.p2_open.n_colors))
		.collect();
	let shared_w: Vec<f64> = rows.iter().map(|r| f64::from(r.p1_open.weight_sum)).collect();
	let d_played: Vec<f64> = rows
		.iter()
		.map(|r| f64::from(r.p1_played.combined()) - f64::from(r.p2_played.combined()))
		.collect();

	lines.push(format!(
		"# pearson(opening_combined_diff, outcome +1/0/-1)={}",
		fmt_r(pearson(&d_open, &outcome))
	));
	lines.push(format!(
		"# pearson(opening_weight_diff, outcome)={}",
		fmt_r(pearson(&d_w, &outcome))
	));
	lines.push(format!("# pearson(opening_dots_diff, outcome)={}", fmt_r(pearson(&d_d, &outcome))));
	lines
		.push(format!("# pearson(opening_color_diff, outcome)={}", fmt_r(pearson(&d_c, &outcome))));
	lines.push(format!(
		"# pearson(p1_opening_weight, outcome)={}  (shared-deal / first-player view)",
		fmt_r(pearson(&shared_w, &outcome))
	));
	lines
		.push(format!("# pearson(p1_opening_weight, turns)={}", fmt_r(pearson(&shared_w, &turns))));
	lines.push(format!(
		"# pearson(played_combined_diff, outcome)={}  (board+bench at end; play-influenced)",
		fmt_r(pearson(&d_played, &outcome))
	));

	let mut decided_open = 0u32;
	let mut higher_open_won = 0u32;
	let mut decided_played = 0u32;
	let mut higher_played_won = 0u32;
	for r in rows {
		if r.outcome == 0 {
			continue;
		}
		if r.p1_open.combined() != r.p2_open.combined() {
			decided_open += 1;
			let p1_higher = r.p1_open.combined() > r.p2_open.combined();
			if (p1_higher && r.outcome > 0) || (!p1_higher && r.outcome < 0) {
				higher_open_won += 1;
			}
		}
		if r.p1_played.combined() != r.p2_played.combined() {
			decided_played += 1;
			let p1_higher = r.p1_played.combined() > r.p2_played.combined();
			if (p1_higher && r.outcome > 0) || (!p1_higher && r.outcome < 0) {
				higher_played_won += 1;
			}
		}
	}
	lines.push(format!(
		"# higher_opening_strength_won={higher_open_won}/{decided_open} (excludes draws and equal opening strength)"
	));
	lines.push(format!(
		"# higher_played_strength_won={higher_played_won}/{decided_played} (excludes draws and equal end strength)"
	));

	lines.push(
		"# opening_deal  pairing  game  seed  p1  p2  p1_bench  p2_bench  outcome  turns".into(),
	);
	let mut rows_sorted = rows.to_vec();
	rows_sorted.sort_by(|a, b| a.pairing.cmp(&b.pairing).then(a.game.cmp(&b.game)));
	for r in rows_sorted {
		let oc = match r.outcome {
			1 => "P1",
			-1 => "P2",
			_ => "D",
		};
		lines.push(format!(
			"#   {}  g{}  seed={}  p1={}  p2={}  [{}]  [{}]  {}  t{}",
			r.pairing,
			r.game,
			r.seed,
			r.p1_open.label(),
			r.p2_open.label(),
			r.p1_bench,
			r.p2_bench,
			oc,
			r.turns
		));
	}
}

#[cfg(test)]
mod tests {
	#![allow(clippy::unwrap_used, clippy::expect_used)]

	use super::*;
	use apa_kan::bench_log::{decode_jsonl_gz, replay};

	fn tmp_dir(tag: &str) -> PathBuf {
		let p = std::env::temp_dir().join(format!(
			"apa-kan-h2h-{}-{}-{}",
			tag,
			std::process::id(),
			std::time::SystemTime::now()
				.duration_since(std::time::UNIX_EPOCH)
				.unwrap()
				.as_nanos()
		));
		fs::create_dir_all(&p).unwrap();
		p
	}

	fn summary_fingerprint(txt: &str) -> String {
		txt.lines()
			.filter(|line| {
				line.contains(" vs ") && line.contains(" P1 ")
					|| line.starts_with("# seat_totals")
					|| line.starts_with("# end_reasons")
					|| line.starts_with("# low_value_pairs")
					|| line.starts_with("# captures")
					|| line.starts_with("# center_captures")
			})
			.collect::<Vec<_>>()
			.join("\n")
	}

	fn run_to_dir(dir: &Path, jobs: usize) -> (String, Vec<u8>) {
		let out = dir.join("h2h.txt");
		let games_log = dir.join("h2h.jsonl.gz");
		let record = dir.join("h2h_record.txt");
		let cfg = parse(vec![
			"--games".to_string(),
			"1".to_string(),
			"--measure-games".to_string(),
			"1".to_string(),
			"--no-depth-probe".to_string(),
			"--seed".to_string(),
			"9".to_string(),
			"--budget".to_string(),
			"nodes:64,iters:6".to_string(),
			"--jobs".to_string(),
			jobs.to_string(),
			"--bots".to_string(),
			"greedy,minimax,mcts".to_string(),
			"--out".to_string(),
			out.display().to_string(),
			"--games-log".to_string(),
			games_log.display().to_string(),
			"--record".to_string(),
			record.display().to_string(),
			"--label".to_string(),
			"jobs-determinism".to_string(),
		])
		.unwrap();
		run(cfg);
		let txt = fs::read_to_string(&out).unwrap();
		let gz = fs::read(&games_log).unwrap();
		(txt, gz)
	}

	#[test]
	fn parse_budget_nodes_and_iters() {
		assert_eq!(
			parse_budget("calibrated").unwrap(),
			(Some(CALIBRATED_NODES_PER_MOVE), Some(CALIBRATED_ITERS_PER_MOVE))
		);
		assert_eq!(parse_budget("nodes:50000,iters:2200").unwrap(), (Some(50_000), Some(2_200)));
		assert_eq!(parse_budget("iters:8").unwrap(), (None, Some(8)));
		assert!(parse_budget("nodes:0").is_err());
		assert!(parse_budget("foo:1").is_err());
	}

	#[test]
	fn think_ms_and_budget_are_mutually_exclusive() {
		let err = match parse(vec![
			"--think-ms".to_string(),
			"1000".to_string(),
			"--budget".to_string(),
			"nodes:10,iters:2".to_string(),
		]) {
			Ok(_) => panic!("expected --think-ms/--budget to conflict"),
			Err(e) => e,
		};
		assert!(err.contains("--think-ms") && err.contains("--budget"));
	}

	#[test]
	fn default_limit_is_calibrated_work() {
		let cfg = parse(Vec::<String>::new()).unwrap();
		assert_eq!(cfg.limit, SearchLimit::calibrated());
		assert!(cfg.jobs >= 1);
	}

	#[test]
	fn work_budget_jobs_1_and_4_match_and_replay() {
		let dir1 = tmp_dir("j1");
		let dir4 = tmp_dir("j4");
		let (txt1, gz1) = run_to_dir(&dir1, 1);
		let (txt4, gz4) = run_to_dir(&dir4, 4);
		assert_eq!(summary_fingerprint(&txt1), summary_fingerprint(&txt4));
		let logs1 = decode_jsonl_gz(&gz1).unwrap();
		let logs4 = decode_jsonl_gz(&gz4).unwrap();
		assert_eq!(logs1.len(), 9);
		assert_eq!(
			bench_log::encode_jsonl(&logs1).unwrap(),
			bench_log::encode_jsonl(&logs4).unwrap()
		);
		for (i, log) in logs1.iter().enumerate() {
			assert_eq!(log.budget_nodes, Some(64));
			assert_eq!(log.budget_iters, Some(6));
			assert_eq!(log.think_ms, 0);
			let replayed = replay(log).unwrap_or_else(|e| panic!("replay game {}: {e}", i + 1));
			assert_eq!(replayed.status(), log.end.to_status().unwrap());
			assert_eq!(replayed.turn_count(), log.turns);
		}
		let _ = fs::remove_dir_all(dir1);
		let _ = fs::remove_dir_all(dir4);
	}
}
