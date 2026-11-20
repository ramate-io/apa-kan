//! Head-to-head search benchmark: branching, stalling, minimax vs MCTS.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use apa_kan::ai::{
	self, Actor, BranchingReport, SearchConfig, SearchLimit, Searcher, CALIBRATED_ITERS_PER_MOVE,
	CALIBRATED_NODES_PER_MOVE,
};
use apa_kan::bench_log::{self, GameLog};
use apa_kan::config::{apply_rule_flag, validate_rule_flags};
use apa_kan::deck::{DeckArchetype, PickStrategy, SetupConfig};
use apa_kan::game::{
	sample_tile_from_rules, Action, EndReason, Game, GameStatus, LowValueResolution,
	MaterialValueResolution, MaterialValueTier, Player, RuleSet, Tile, TileColor,
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
	setup: SetupConfig,
	/// If set, only these directed pairings run (not the cartesian of --bots).
	pairings: Option<Vec<(Actor, Actor)>>,
	/// Archetype matchups. Default is one row from `--p1-deck`/`--p2-deck`.
	matchups: Vec<(DeckArchetype, DeckArchetype)>,
	search: SearchConfig,
	p1_search: Option<SearchConfig>,
	p2_search: Option<SearchConfig>,
	/// Opening/midgame depth table for each search lever, then exit.
	ablate: bool,
	/// Time a handful of positions trying to complete this minimax ID depth.
	ply_probe: Option<u8>,
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
	                        Each search stays single-threaded unless --search-threads > 1.\n\
	   --search-profile <enhanced|classic|plain|tactical|positional|explorative|deep_mcts>\n\
	                        Search preset (default: enhanced; aliases: aggressive, cautious, explore, deep, focused)\n\
	   --bot-config <path.toml>  Load a SearchConfig TOML (missing keys = enhanced defaults)\n\
	   --list-search-profiles    Print named presets and their labels\n\
	   --search <k=v,...>   Overlay any knob (ordering, tt, lmr, pvs, q-depth, pw, puct, priors,\n\
	                        pw-order, reuse, mcts-tt, factored, leaf, solver, threads, ...)\n\
	   --p1-search-profile / --p2-search-profile  Per-seat profile (default: --search-profile)\n\
	   --p1-bot-config / --p2-bot-config          Per-seat TOML profile\n\
	   --p1-search / --p2-search                  Per-seat overlay\n\
	   --search-threads <1..32>  MCTS root-parallel trees (default: 1; keep 1 for deterministic --jobs benches).\n\
	                             Worker t is seeded with rng.gen() XOR (t+1)*golden; threads=1 draws nothing extra.\n\
	   --ablate-search      Depth/node table for each lever on opening + short midgame, then exit\n\
	   --ply-probe <n>      Time minimax completing ID depth n on a handful of positions, then exit\n\
	   --calibrate          Measure nodes/iters per move at --think-ms (default 1000), jobs=1, then exit\n\
	   --seed <u64>         Base seed (default: 1)\n\
	   --deck-seed <u64>    Collection generation seed (default: --seed)\n\
	   --deck-noise <0..100> Resample that percent of a mirrored collection (default: 0)\n\
	   --pick <heuristic|search|greedy>  Bench 7+4 picker (default: heuristic)\n\
	   --bots <list>        Comma-separated actors (default: greedy,minimax,mcts)\n\
	   --pairings <list>    Directed pairings a:b,c:d (default: cartesian of --bots)\n\
	   --p1-deck <mixed|braid|bead|crescent|zs|emblem|blocker|rosette>  P1 collection archetype when --deck-matchups is omitted\n\
	   --p2-deck <mixed|braid|bead|crescent|zs|emblem|blocker|rosette>  P2 collection archetype\n\
	   --deck-matchups <list>  a:b,c:d archetype pairs (mixed|braid|bead|crescent|zs|emblem|blocker|rosette). Default: --p1-deck vs --p2-deck\n\
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
	   --insufficient-tiles-loss, --endgame-rule, --starting-tokens, --starting-bench, --deck-size, --bag-size, --self-capture-budget,\n\
	   --max-rings, --zag-chance, --zags, --braid-chance, --braids, --bead-chance, --dot-chance, --zag-mismatch,\n\
	   --braid-fail-mismatch, --bead-scoring, --channel-mode, --capture-mode, --dot-chance,\n\
	   --emblem-weights, --emblems, --blocker-blocks,\n\
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
		setup: SetupConfig::default(),
		pairings: None,
		matchups: Vec::new(),
		search: SearchConfig::enhanced(),
		p1_search: None,
		p2_search: None,
		ablate: false,
		ply_probe: None,
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
			"--deck-seed" => {
				cfg.setup.deck_seed =
					Some(parse_u64("--deck-seed", &need("--deck-seed", it.next())?)?);
			}
			"--deck-noise" => {
				let n = parse_u64("--deck-noise", &need("--deck-noise", it.next())?)?;
				if n > 100 {
					return Err("invalid --deck-noise (expected 0..100 percent)".into());
				}
				cfg.setup.deck_noise = n as u8;
			}
			"--pick" => {
				cfg.setup.pick = PickStrategy::parse(&need("--pick", it.next())?)?;
			}
			"--p1-deck" => {
				cfg.setup.p1_deck = DeckArchetype::parse(&need("--p1-deck", it.next())?)?;
			}
			"--p2-deck" => {
				cfg.setup.p2_deck = DeckArchetype::parse(&need("--p2-deck", it.next())?)?;
			}
			"--deck-matchups" => {
				cfg.matchups = parse_deck_matchups(&need("--deck-matchups", it.next())?)?;
			}
			"--pairings" => {
				cfg.pairings = Some(parse_pairings_flag(&need("--pairings", it.next())?)?);
			}
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
			"--search-profile" => {
				cfg.search = SearchConfig::parse_profile(&need("--search-profile", it.next())?)?;
			}
			"--bot-config" => {
				cfg.search = SearchConfig::load_path(Path::new(&need("--bot-config", it.next())?))?;
			}
			"--list-search-profiles" => {
				print!("{}", SearchConfig::list_text());
				std::process::exit(0);
			}
			"--search" => {
				cfg.search = cfg.search.with_spec(&need("--search", it.next())?)?;
			}
			"--p1-search-profile" => {
				cfg.p1_search =
					Some(SearchConfig::parse_profile(&need("--p1-search-profile", it.next())?)?);
			}
			"--p2-search-profile" => {
				cfg.p2_search =
					Some(SearchConfig::parse_profile(&need("--p2-search-profile", it.next())?)?);
			}
			"--p1-bot-config" => {
				cfg.p1_search =
					Some(SearchConfig::load_path(Path::new(&need("--p1-bot-config", it.next())?))?);
			}
			"--p2-bot-config" => {
				cfg.p2_search =
					Some(SearchConfig::load_path(Path::new(&need("--p2-bot-config", it.next())?))?);
			}
			"--p1-search" => {
				let base = cfg.p1_search.unwrap_or(cfg.search);
				cfg.p1_search = Some(base.with_spec(&need("--p1-search", it.next())?)?);
			}
			"--p2-search" => {
				let base = cfg.p2_search.unwrap_or(cfg.search);
				cfg.p2_search = Some(base.with_spec(&need("--p2-search", it.next())?)?);
			}
			"--search-threads" => {
				let n = parse_u64("--search-threads", &need("--search-threads", it.next())?)?;
				if n == 0 || n > 32 {
					return Err("invalid --search-threads (expected 1..32)".into());
				}
				cfg.search = cfg.search.with_threads(n as u8);
			}
			"--ablate-search" => cfg.ablate = true,
			"--ply-probe" => {
				let n = parse_u64("--ply-probe", &need("--ply-probe", it.next())?)?;
				if n == 0 || n > 32 {
					return Err("invalid --ply-probe (expected 1..32)".into());
				}
				cfg.ply_probe = Some(n as u8);
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
	cfg.setup.limit = cfg.limit;
	if cfg.matchups.is_empty() {
		cfg.matchups = vec![(cfg.setup.p1_deck, cfg.setup.p2_deck)];
	}
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

fn parse_pairings_flag(s: &str) -> Result<Vec<(Actor, Actor)>, String> {
	let mut out = Vec::new();
	for part in s.split(',') {
		let part = part.trim();
		if part.is_empty() {
			continue;
		}
		let (a, b) = part.split_once(':').ok_or_else(|| {
			format!("invalid --pairings '{s}' (expected a:b,c:d with actor names)")
		})?;
		out.push((Actor::parse(a.trim())?, Actor::parse(b.trim())?));
	}
	if out.is_empty() {
		return Err("--pairings must name at least one a:b pairing".into());
	}
	Ok(out)
}

fn parse_deck_matchups(s: &str) -> Result<Vec<(DeckArchetype, DeckArchetype)>, String> {
	let mut out = Vec::new();
	for part in s.split(',') {
		let part = part.trim();
		if part.is_empty() {
			continue;
		}
		let (a, b) = part.split_once(':').ok_or_else(|| {
			format!(
				"invalid --deck-matchups '{s}' (expected a:b,c:d with mixed|braid|bead|crescent|zs|emblem|blocker|rosette)"
			)
		})?;
		out.push((DeckArchetype::parse(a.trim())?, DeckArchetype::parse(b.trim())?));
	}
	if out.is_empty() {
		return Err("--deck-matchups must name at least one a:b matchup".into());
	}
	Ok(out)
}

fn resolved_pairings(cfg: &BenchConfig) -> Vec<(Actor, Actor)> {
	cfg.pairings.clone().unwrap_or_else(|| pairings(&cfg.bots))
}

fn matchup_label(p1: DeckArchetype, p2: DeckArchetype) -> String {
	format!("{} vs {}", p1.name(), p2.name())
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
	material_value: u32,
	material_value_wins: u32,
	material_value_draws: u32,
	material_value_total: u32,
	material_value_board: u32,
	material_value_low_value: u32,
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
	bench_captures: u32,
	token_captures: u32,
	token_sum: u64,
	bench_len_sum: u64,
	snapshots: u32,
	p1_open_dots: u32,
	p1_open_n: u32,
	bead_contrib: u32,
	bead_needed: u32,
	spawn_two_ring_single_bead: u32,
	claims: u32,
	claim_own: u32,
	claim_opp: u32,
	healed: u32,
	crescent_saves: u32,
	zs_bonuses: u32,
	depth_max: u8,
	mcts_depth_sum: f64,
	mcts_depth_n: u32,
	mcts_depth_max: u8,
	root_entropy_sum: f64,
	root_eff_branch_sum: f64,
	root_top1_sum: f64,
	root_near_q_sum: f64,
	root_q_spread_sum: f64,
	root_stability_sum: f64,
	root_legal_sum: f64,
	root_spread_n: u32,
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

fn seat_search(cfg: &BenchConfig, p1: bool) -> SearchConfig {
	let mut s = if p1 { cfg.p1_search } else { cfg.p2_search }.unwrap_or(cfg.search);
	if s.mcts_root_threads == 1 && cfg.search.mcts_root_threads > 1 {
		s.mcts_root_threads = cfg.search.mcts_root_threads;
	}
	s
}

fn run(cfg: BenchConfig) {
	if cfg.calibrate {
		run_calibrate(&cfg);
		return;
	}
	if cfg.ablate {
		run_ablate(&cfg);
		return;
	}
	if cfg.ply_probe.is_some() {
		run_ply_probe(&cfg);
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
		probe_minimax_depths(
			&mut lines,
			rules,
			cfg.seed,
			cfg.scenario.as_ref(),
			cfg.setup,
			seat_search(&cfg, true),
		);
	}

	lines.push(String::new());
	lines.push(format!("# {}", cfg.label));
	lines.push(limit_header(&cfg, rules.turn_limit));
	lines.push(format!(
		"# search  p1={}  p2={}",
		seat_search(&cfg, true).label(),
		seat_search(&cfg, false).label()
	));
	lines.push(format!(
		"# rules: no_moves_loses={} center_hold_win={} center_hold_n={} no_same_piece_twice={} color_restricted_hops={} hop_weight_cmp={} counted_pass_through={} pass_through_ring_cmp={} petal_majority_win={} petal_majority_n={} control_win={} control_n={} control_hold_turns={} insufficient_tiles_loss={} endgame_rule={} starting_bench={} bag_size={} deck_size={} pick={} deck_noise={} self_capture_budget={} max_rings={} zag_numer={} braid_numer={} bead_numer={} bead_scoring={} channel_mode={} capture_mode={} dot_percent={} emblem_weights={} blocker_blocks={} zag_mismatch={} path_constraint={} scenario={}",
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
		rules.endgame_rule.name(),
		rules.starting_bench_size,
		rules.bag_size,
		rules.deck_size,
		cfg.setup.pick.name(),
		cfg.setup.deck_noise,
		on_off(rules.self_capture_budget),
		rules.max_rings,
		rules.zag_numer,
		rules.braid_numer,
		rules.bead_numer,
		rules.bead_scoring.name(),
		rules.channel_mode.name(),
		rules.capture_mode.name(),
		rules.dot_percent,
		rules.emblem_weights.label(),
		rules.blocker_blocks.name(),
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
		"# search_depth: mm_avg/mm_max = completed iterative-deepening depth; mcts_avg/mcts_max = visit-weighted tree depth (root excluded from the mean).".into(),
	);
	lines.push(
		"# root_spread: visit entropy / effective branching, top-1 visit share, #moves within 1/sqrt(n) of best Q, Q spread of top 10, best-move stability.".into(),
	);
	lines.push(
		"# pairing  matchup  seat  W-D-L  avg_turns  elim  locked_out  center_hold  petal_maj  control  insuff  low_value  material_value  turn_limit  double_pass  repeats>=3  avg_max_repeat  avg_depth  avg_iters  avg_captures  avg_self_captures".into(),
	);

	let keep_log = cfg.games_log.is_some();
	let pairs = resolved_pairings(&cfg);
	let matchups = cfg.matchups.clone();
	let stdout = Mutex::new(());
	let mut jobs = Vec::new();
	for (mi, &(p1_deck, p2_deck)) in matchups.iter().enumerate() {
		for (pi, &(p1, p2)) in pairs.iter().enumerate() {
			for g in 0..cfg.games {
				let seed = cfg
					.seed
					.wrapping_add((mi as u64).wrapping_mul(100_000))
					.wrapping_add((pi as u64).wrapping_mul(10_000))
					.wrapping_add(u64::from(g));
				jobs.push(GameJob {
					pi,
					mi,
					g,
					p1,
					p2,
					p1_deck,
					p2_deck,
					seed,
					want_record: p1 == Actor::Minimax && p2 == Actor::Mcts && g == 0 && mi == 0,
				});
			}
		}
	}
	println!(
		"running {} games across {} pairings × {} matchups with --jobs {} ({})...",
		jobs.len(),
		pairs.len(),
		matchups.len(),
		cfg.jobs,
		cfg.limit.label()
	);
	let scenario = cfg.scenario.clone();
	let limit = cfg.limit;
	let setup = cfg.setup;
	let p1_search = seat_search(&cfg, true);
	let p2_search = seat_search(&cfg, false);
	let outs = run_parallel(cfg.jobs, jobs, |job| {
		let mut job_setup = setup;
		job_setup.p1_deck = job.p1_deck;
		job_setup.p2_deck = job.p2_deck;
		let result = play_game(
			rules,
			job.seed,
			job.p1,
			job.p2,
			limit,
			job.want_record,
			keep_log,
			scenario.as_ref(),
			job_setup,
			p1_search,
			p2_search,
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

	let mut by_pair: Vec<Vec<PlayResult>> =
		(0..pairs.len() * matchups.len()).map(|_| Vec::new()).collect();
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
			log.deck_matchup = Some(matchup_label(out.job.p1_deck, out.job.p2_deck));
			logs.push(log);
		}
		let mut row = out.result.deck.clone();
		row.pairing = format!(
			"{} vs {} [{}]",
			out.job.p1.name(),
			out.job.p2.name(),
			matchup_label(out.job.p1_deck, out.job.p2_deck)
		);
		row.game = out.job.g + 1;
		deck_rows.push(row);
		let gi = out.job.mi * pairs.len() + out.job.pi;
		by_pair[gi].push(out.result);
	}

	let mut pairing_results: Vec<(usize, SeatStats, String)> = Vec::new();
	for (mi, &(p1_deck, p2_deck)) in matchups.iter().enumerate() {
		for (pi, &(p1, p2)) in pairs.iter().enumerate() {
			let gi = mi * pairs.len() + pi;
			let (stats, line) =
				summarize_pairing(p1, p2, cfg.games, &by_pair[gi], matchup_label(p1_deck, p2_deck));
			pairing_results.push((gi, stats, line));
		}
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
	let mut end_material_value = 0u32;
	let mut end_material_value_wins = 0u32;
	let mut end_material_value_draws = 0u32;
	let mut end_material_value_total = 0u32;
	let mut end_material_value_board = 0u32;
	let mut end_material_value_low_value = 0u32;
	let mut end_limit = 0u32;
	let mut end_pass = 0u32;
	let mut captures = 0u32;
	let mut self_captures = 0u32;
	let mut center_captures = 0u32;
	let mut center_self_captures = 0u32;
	let mut bench_captures = 0u32;
	let mut token_captures = 0u32;
	let mut token_sum = 0u64;
	let mut bench_len_sum = 0u64;
	let mut snapshots = 0u32;
	let mut bead_contrib = 0u32;
	let mut bead_needed = 0u32;
	let mut spawn_two_ring = 0u32;
	let mut claims = 0u32;
	let mut claim_own = 0u32;
	let mut claim_opp = 0u32;
	let mut healed = 0u32;
	let mut crescent_saves = 0u32;
	let mut zs_bonuses = 0u32;
	let mut mm_depth_sum = 0.0f64;
	let mut mm_depth_n = 0u32;
	let mut mm_depth_max = 0u8;
	let mut mcts_depth_sum = 0.0f64;
	let mut mcts_depth_n = 0u32;
	let mut mcts_depth_max = 0u8;
	let mut root_entropy_sum = 0.0f64;
	let mut root_eff_branch_sum = 0.0f64;
	let mut root_top1_sum = 0.0f64;
	let mut root_near_q_sum = 0.0f64;
	let mut root_q_spread_sum = 0.0f64;
	let mut root_stability_sum = 0.0f64;
	let mut root_legal_sum = 0.0f64;
	let mut root_spread_n = 0u32;
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
		end_material_value += stats.material_value;
		end_material_value_wins += stats.material_value_wins;
		end_material_value_draws += stats.material_value_draws;
		end_material_value_total += stats.material_value_total;
		end_material_value_board += stats.material_value_board;
		end_material_value_low_value += stats.material_value_low_value;
		end_limit += stats.turn_limit;
		end_pass += stats.double_pass;
		captures += stats.captures;
		self_captures += stats.self_captures;
		center_captures += stats.center_captures;
		center_self_captures += stats.center_self_captures;
		bench_captures += stats.bench_captures;
		token_captures += stats.token_captures;
		token_sum += stats.token_sum;
		bench_len_sum += stats.bench_len_sum;
		snapshots += stats.snapshots;
		bead_contrib += stats.bead_contrib;
		bead_needed += stats.bead_needed;
		spawn_two_ring += stats.spawn_two_ring_single_bead;
		claims += stats.claims;
		claim_own += stats.claim_own;
		claim_opp += stats.claim_opp;
		healed += stats.healed;
		crescent_saves += stats.crescent_saves;
		zs_bonuses += stats.zs_bonuses;
		mm_depth_sum += stats.depth_sum;
		mm_depth_n += stats.depth_n;
		mm_depth_max = mm_depth_max.max(stats.depth_max);
		mcts_depth_sum += stats.mcts_depth_sum;
		mcts_depth_n += stats.mcts_depth_n;
		mcts_depth_max = mcts_depth_max.max(stats.mcts_depth_max);
		root_entropy_sum += stats.root_entropy_sum;
		root_eff_branch_sum += stats.root_eff_branch_sum;
		root_top1_sum += stats.root_top1_sum;
		root_near_q_sum += stats.root_near_q_sum;
		root_q_spread_sum += stats.root_q_spread_sum;
		root_stability_sum += stats.root_stability_sum;
		root_legal_sum += stats.root_legal_sum;
		root_spread_n += stats.root_spread_n;
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
		"# end_reasons  elim={end_elim} locked_out={end_lock} center_hold={end_hold} petal_maj={end_petal} control={end_control} insuff={end_insuff} low_value={end_low_value} low_value_wins={end_low_value_wins} low_value_draws={end_low_value_draws} material_value={end_material_value} material_value_wins={end_material_value_wins} material_value_draws={end_material_value_draws} turn_limit={end_limit} double_pass={end_pass}"
	));
	lines.push(format!(
		"# low_value_pairs  pair0={end_low_value_pair0} later={end_low_value_later} piece_count={end_low_value_piece_count}"
	));
	lines.push(format!(
		"# material_value_tiers  total_material={end_material_value_total} board_tiles={end_material_value_board} low_value={end_material_value_low_value} draws={end_material_value_draws}"
	));
	println!(
		"# end_reasons  elim={end_elim} locked_out={end_lock} center_hold={end_hold} petal_maj={end_petal} control={end_control} insuff={end_insuff} low_value={end_low_value} low_value_wins={end_low_value_wins} low_value_draws={end_low_value_draws} material_value={end_material_value} material_value_wins={end_material_value_wins} material_value_draws={end_material_value_draws} turn_limit={end_limit} double_pass={end_pass}"
	);
	println!(
		"# low_value_pairs  pair0={end_low_value_pair0} later={end_low_value_later} piece_count={end_low_value_piece_count}"
	);
	println!(
		"# material_value_tiers  total_material={end_material_value_total} board_tiles={end_material_value_board} low_value={end_material_value_low_value} draws={end_material_value_draws}"
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
	let avg_bench_cap =
		if games_n == 0 { 0.0 } else { f64::from(bench_captures) / f64::from(games_n) };
	let avg_token_cap =
		if games_n == 0 { 0.0 } else { f64::from(token_captures) / f64::from(games_n) };
	let mean_tokens =
		if snapshots == 0 { 0.0 } else { token_sum as f64 / f64::from(snapshots) / 2.0 };
	let mean_bench =
		if snapshots == 0 { 0.0 } else { bench_len_sum as f64 / f64::from(snapshots) / 2.0 };
	lines.push(format!(
		"# capture_dest  bench={bench_captures} avg_per_game={avg_bench_cap:.2}  tokens={token_captures} avg_per_game={avg_token_cap:.2}"
	));
	lines.push(format!(
		"# bead_captures  contrib={bead_contrib} needed={bead_needed} spawn_two_ring_single_bead={spawn_two_ring}  of_total={captures}"
	));
	let avg_claims = if games_n == 0 { 0.0 } else { f64::from(claims) / f64::from(games_n) };
	let avg_healed = if games_n == 0 { 0.0 } else { f64::from(healed) / f64::from(games_n) };
	lines.push(format!(
		"# claims  total={claims} avg_per_game={avg_claims:.2}  own={claim_own} opponent={claim_opp}  healed={healed} healed_avg_per_game={avg_healed:.2}"
	));
	let avg_moon = if games_n == 0 { 0.0 } else { f64::from(crescent_saves) / f64::from(games_n) };
	let avg_zs = if games_n == 0 { 0.0 } else { f64::from(zs_bonuses) / f64::from(games_n) };
	lines.push(format!(
		"# emblems  crescent_saves={crescent_saves} avg_per_game={avg_moon:.2}  zs_bonuses={zs_bonuses} avg_per_game={avg_zs:.2}"
	));
	let mm_avg = if mm_depth_n == 0 { 0.0 } else { mm_depth_sum / f64::from(mm_depth_n) };
	let mcts_avg = if mcts_depth_n == 0 { 0.0 } else { mcts_depth_sum / f64::from(mcts_depth_n) };
	lines.push(format!(
		"# search_depth  mm_avg={mm_avg:.2} mm_max={mm_depth_max} mm_n={mm_depth_n}  mcts_avg={mcts_avg:.2} mcts_max={mcts_depth_max} mcts_n={mcts_depth_n}"
	));
	let rn = f64::from(root_spread_n.max(1));
	let root_line = format!(
		"# root_spread  entropy={:.2} eff_branch={:.1} top1={:.2} near_q={:.1} q_spread10={:.3} stability={:.2} legal={:.1} n={root_spread_n}",
		root_entropy_sum / rn,
		root_eff_branch_sum / rn,
		root_top1_sum / rn,
		root_near_q_sum / rn,
		root_q_spread_sum / rn,
		root_stability_sum / rn,
		root_legal_sum / rn
	);
	lines.push(root_line.clone());
	println!("{root_line}");
	{
		let mut matchup_lines: Vec<(String, u32, u32, u32, u32)> = Vec::new();
		for (mi, &(a, b)) in matchups.iter().enumerate() {
			let mut w = 0u32;
			let mut d = 0u32;
			let mut l = 0u32;
			let mut turns = 0u32;
			for (pi, _) in pairs.iter().enumerate() {
				for r in &by_pair[mi * pairs.len() + pi] {
					turns += r.turns;
					match r.status {
						GameStatus::Won { winner: Player::One, .. } => w += 1,
						GameStatus::Won { winner: Player::Two, .. } => l += 1,
						_ => d += 1,
					}
				}
			}
			matchup_lines.push((matchup_label(a, b), w, d, l, turns));
		}
		let bits: Vec<String> = matchup_lines
			.iter()
			.map(|(name, w, d, l, turns)| {
				let n = w + d + l;
				let avg = if n == 0 { 0.0 } else { f64::from(*turns) / f64::from(n) };
				format!("{name} {w}-{d}-{l} avg_turns={avg:.2}")
			})
			.collect();
		lines.push(format!("# matchup_totals  {}", bits.join("  |  ")));
		println!("# matchup_totals  {}", bits.join("  |  "));
	}
	{
		// Seat-agnostic archetype record: does braid-heavy (or bead-heavy)
		// win from both seats in the cross matchup? That's the primary
		// "dominant strategy" signal.
		let mut p1_w = [0u32; 8];
		let mut p1_d = [0u32; 8];
		let mut p1_l = [0u32; 8];
		let mut p2_w = [0u32; 8];
		let mut p2_d = [0u32; 8];
		let mut p2_l = [0u32; 8];
		let idx = |a: DeckArchetype| match a {
			DeckArchetype::Mixed => 0,
			DeckArchetype::BraidHeavy => 1,
			DeckArchetype::BeadHeavy => 2,
			DeckArchetype::Crescent => 3,
			DeckArchetype::Zs => 4,
			DeckArchetype::EmblemMix => 5,
			DeckArchetype::Blocker => 6,
			DeckArchetype::Rosette => 7,
		};
		for (mi, &(a, b)) in matchups.iter().enumerate() {
			for (pi, _) in pairs.iter().enumerate() {
				for r in &by_pair[mi * pairs.len() + pi] {
					match r.status {
						GameStatus::Won { winner: Player::One, .. } => {
							p1_w[idx(a)] += 1;
							p2_l[idx(b)] += 1;
						}
						GameStatus::Won { winner: Player::Two, .. } => {
							p1_l[idx(a)] += 1;
							p2_w[idx(b)] += 1;
						}
						_ => {
							p1_d[idx(a)] += 1;
							p2_d[idx(b)] += 1;
						}
					}
				}
			}
		}
		let names = ["mixed", "braid", "bead", "crescent", "zs", "emblem", "blocker", "rosette"];
		let bits: Vec<String> = (0..8)
			.filter(|&i| p1_w[i] + p1_d[i] + p1_l[i] + p2_w[i] + p2_d[i] + p2_l[i] > 0)
			.map(|i| {
				format!(
					"{} P1={}-{}-{} P2={}-{}-{}",
					names[i], p1_w[i], p1_d[i], p1_l[i], p2_w[i], p2_d[i], p2_l[i]
				)
			})
			.collect();
		lines.push(format!("# archetype_seat  {}", bits.join("  |  ")));
		println!("# archetype_seat  {}", bits.join("  |  "));
		let mut short_n = 0u32;
		let mut short_lock = 0u32;
		let mut short_ctrl = 0u32;
		for results in &by_pair {
			for r in results {
				if r.turns > 20 {
					continue;
				}
				short_n += 1;
				match r.status {
					GameStatus::Won { reason: EndReason::LockedOut, .. } => short_lock += 1,
					GameStatus::Won { reason: EndReason::Control, .. } => short_ctrl += 1,
					_ => {}
				}
			}
		}
		lines.push(format!(
			"# short_games  turns_le_20={short_n} lockout={short_lock} control={short_ctrl}"
		));
		println!("# short_games  turns_le_20={short_n} lockout={short_lock} control={short_ctrl}");
	}
	lines.push(format!(
		"# tokens_bench_over_time  mean_tokens_per_side={mean_tokens:.2}  mean_bench_per_side={mean_bench:.2}  snapshots={snapshots}"
	));
	let mut pick_w = [0u32; 12];
	let mut pick_d = [0u32; 12];
	let mut pick_l = [0u32; 12];
	for results in &by_pair {
		for r in results {
			debug_assert!(r.p1_open_n == 0 || r.p1_open_dots <= r.p1_open_n);
			let i = r.p1_open_dots.min(11) as usize;
			match r.status {
				GameStatus::Won { winner: Player::One, .. } => pick_w[i] += 1,
				GameStatus::Won { winner: Player::Two, .. } => pick_l[i] += 1,
				_ => pick_d[i] += 1,
			}
		}
	}
	let pick_bits: Vec<String> = (0..12)
		.filter(|&i| pick_w[i] + pick_d[i] + pick_l[i] > 0)
		.map(|i| format!("dots{}={}-{}-{}", i, pick_w[i], pick_d[i], pick_l[i]))
		.collect();
	lines.push(format!("# pick_dot_ratio  P1_opening_dots W-D-L  {}", pick_bits.join("  ")));
	println!("# captures  opponent_tiles_removed={captures}  avg_per_game={avg_cap:.2}  self_captures={self_captures}  self_avg_per_game={avg_self:.2}");
	println!("# center_captures  opponent_tiles_removed={center_captures}  avg_per_game={avg_center:.2}  self_captures={center_self_captures}  self_avg_per_game={avg_center_self:.2}");
	println!(
		"# capture_dest  bench={bench_captures} avg_per_game={avg_bench_cap:.2}  tokens={token_captures} avg_per_game={avg_token_cap:.2}"
	);
	println!(
		"# bead_captures  contrib={bead_contrib} needed={bead_needed} spawn_two_ring_single_bead={spawn_two_ring}  of_total={captures}"
	);
	println!(
		"# claims  total={claims} avg_per_game={avg_claims:.2}  own={claim_own} opponent={claim_opp}  healed={healed} healed_avg_per_game={avg_healed:.2}"
	);
	println!(
		"# emblems  crescent_saves={crescent_saves} avg_per_game={avg_moon:.2}  zs_bonuses={zs_bonuses} avg_per_game={avg_zs:.2}"
	);
	println!(
		"# search_depth  mm_avg={mm_avg:.2} mm_max={mm_depth_max} mm_n={mm_depth_n}  mcts_avg={mcts_avg:.2} mcts_max={mcts_depth_max} mcts_n={mcts_depth_n}"
	);
	println!(
		"# tokens_bench_over_time  mean_tokens_per_side={mean_tokens:.2}  mean_bench_per_side={mean_bench:.2}  snapshots={snapshots}"
	);
	println!("# pick_dot_ratio  P1_opening_dots W-D-L  {}", pick_bits.join("  "));
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
		logs.sort_by(|a, b| {
			a.pairing
				.cmp(&b.pairing)
				.then(a.deck_matchup.cmp(&b.deck_matchup))
				.then(a.game.cmp(&b.game))
		});
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
		let mut game = Game::start_with(cfg.rules, seed, cfg.scenario.as_ref(), cfg.setup)
			.unwrap_or_else(|e| {
				eprintln!("{e}");
				std::process::exit(1);
			});
		let mut rng = StdRng::seed_from_u64(bench_log::search_rng_seed(seed));
		let mut s1 = Searcher::with_config(seat_search(cfg, true));
		let mut s2 = Searcher::with_config(seat_search(cfg, false));
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
					println!(
						"  seed={seed} mcts iters={} tree_depth={:.2}/{}",
						st.iterations, st.mcts_avg_depth, st.mcts_max_depth
					);
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
	println!("# search_depth  mm_avg={mean_depth:.2}");
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
	lines.push(format!("CLAIM     {:<12} {:<6.2} {}", b.claim_n, b.claim_avg(), b.claim_max));
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
	mi: usize,
	g: u32,
	p1: Actor,
	p2: Actor,
	p1_deck: DeckArchetype,
	p2_deck: DeckArchetype,
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
	matchup: String,
) -> (SeatStats, String) {
	let mut stats = SeatStats::default();
	for result in results {
		stats.turns += result.turns;
		stats.depth_sum += result.depth_sum;
		stats.depth_n += result.depth_n;
		stats.depth_max = stats.depth_max.max(result.depth_max);
		stats.iter_sum += result.iter_sum;
		stats.mcts_depth_sum += result.mcts_depth_sum;
		stats.mcts_depth_n += result.mcts_depth_n;
		stats.mcts_depth_max = stats.mcts_depth_max.max(result.mcts_depth_max);
		stats.root_entropy_sum += result.root_entropy_sum;
		stats.root_eff_branch_sum += result.root_eff_branch_sum;
		stats.root_top1_sum += result.root_top1_sum;
		stats.root_near_q_sum += result.root_near_q_sum;
		stats.root_q_spread_sum += result.root_q_spread_sum;
		stats.root_stability_sum += result.root_stability_sum;
		stats.root_legal_sum += result.root_legal_sum;
		stats.root_spread_n += result.root_spread_n;
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
		stats.bench_captures += result.bench_captures;
		stats.token_captures += result.token_captures;
		stats.token_sum += result.token_sum;
		stats.bench_len_sum += result.bench_len_sum;
		stats.snapshots += result.snapshots;
		stats.p1_open_dots += result.p1_open_dots;
		stats.p1_open_n += result.p1_open_n;
		stats.bead_contrib += result.bead_contrib;
		stats.bead_needed += result.bead_needed;
		stats.spawn_two_ring_single_bead += result.spawn_two_ring_single_bead;
		stats.claims += result.claims;
		stats.claim_own += result.claim_own;
		stats.claim_opp += result.claim_opp;
		stats.healed += result.healed;
		stats.crescent_saves += result.crescent_saves;
		stats.zs_bonuses += result.zs_bonuses;
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
		tally_material_value(&mut stats, result.material_value);
	}
	let n = games.max(1);
	let avg = f64::from(stats.turns) / f64::from(n);
	let avg_rep = f64::from(stats.sum_max_repeat) / f64::from(n);
	let avg_depth =
		if stats.depth_n == 0 { 0.0 } else { stats.depth_sum / f64::from(stats.depth_n) };
	let avg_iters = if stats.iter_n == 0 { 0.0 } else { stats.iter_sum / f64::from(stats.iter_n) };
	let mcts_avg_depth = if stats.mcts_depth_n == 0 {
		0.0
	} else {
		stats.mcts_depth_sum / f64::from(stats.mcts_depth_n)
	};
	let avg_cap = f64::from(stats.captures) / f64::from(n);
	let avg_self = f64::from(stats.self_captures) / f64::from(n);
	let line = format!(
		"{:<15} vs {:<15}  {:<13}  P1  {}-{}-{}  {:.2}  {}  {}  {}  {}  {}  {}  {}  {}  {}  {}  {}  {:.2}  {:.2}  {:.1}  cap={:.2}  self={:.2}  mm_max={} mcts_avg={:.2} mcts_max={}",
		p1.name(),
		p2.name(),
		matchup,
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
		stats.material_value,
		stats.turn_limit,
		stats.double_pass,
		stats.games_repeat_ge3,
		avg_rep,
		avg_depth,
		avg_iters,
		avg_cap,
		avg_self,
		stats.depth_max,
		mcts_avg_depth,
		stats.mcts_depth_max
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
		EndReason::MaterialValue => stats.material_value += 1,
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

fn tally_material_value(stats: &mut SeatStats, resolution: Option<MaterialValueResolution>) {
	match resolution {
		None => {}
		Some(MaterialValueResolution::Draw) => stats.material_value_draws += 1,
		Some(MaterialValueResolution::Win { tier, .. }) => {
			stats.material_value_wins += 1;
			match tier {
				MaterialValueTier::TotalMaterial => stats.material_value_total += 1,
				MaterialValueTier::BoardTiles => stats.material_value_board += 1,
				MaterialValueTier::LowValue => stats.material_value_low_value += 1,
			}
		}
	}
}

fn probe_minimax_depths(
	lines: &mut Vec<String>,
	rules: RuleSet,
	seed: u64,
	scenario: Option<&Scenario>,
	setup: SetupConfig,
	search: SearchConfig,
) {
	use apa_kan::ai::Searcher;
	lines.push(String::new());
	lines.push(format!(
		"# minimax iterative-deepening depth probe (opening position) search={}",
		search.label()
	));
	let game = Game::start_with(rules, seed, scenario, setup).unwrap_or_else(|e| {
		eprintln!("{e}");
		std::process::exit(1);
	});
	for ms in [1000u64, 3000] {
		println!("minimax depth probe {ms} ms...");
		let mut searcher = Searcher::with_config(search);
		let mut rng = StdRng::seed_from_u64(seed ^ 0xDE97);
		let _ = searcher.choose(&game, Actor::Minimax, &mut rng, Duration::from_millis(ms));
		let st = searcher.stats();
		lines.push(format!("# think_ms={} completed_depth={} nodes={}", ms, st.depth, st.nodes));
		println!("  depth={} nodes={}", st.depth, st.nodes);
	}
}

fn ablation_profiles() -> Vec<(&'static str, SearchConfig)> {
	let mut rows = Vec::new();
	rows.push(("plain", SearchConfig::plain()));
	let mut cfg = SearchConfig::plain();
	cfg.move_ordering = true;
	rows.push(("+ordering", cfg));
	cfg.transposition = true;
	rows.push(("+tt", cfg));
	cfg.lmr = true;
	rows.push(("+lmr", cfg));
	cfg.pvs = true;
	rows.push(("+pvs", cfg));
	cfg.quiescence = true;
	rows.push(("+quiescence", cfg));
	rows.push(("classic", SearchConfig::classic()));
	rows.push(("enhanced", SearchConfig::enhanced()));
	let mut mcts_pw = SearchConfig::classic();
	mcts_pw.mcts_progressive_widening = true;
	rows.push(("classic+pw", mcts_pw));
	let mut mcts_rand = SearchConfig::classic();
	mcts_rand.mcts_heuristic_playouts = false;
	rows.push(("classic+random_playouts", mcts_rand));
	let mut priors = SearchConfig::enhanced();
	priors.mcts_move_priors = true;
	priors.prior_widening = true;
	priors.puct = 0.9;
	rows.push(("enhanced+priors", priors));
	let mut pw_prior = priors;
	pw_prior.mcts_progressive_widening = true;
	pw_prior.pw_a = 1.0;
	pw_prior.pw_alpha = 0.4;
	pw_prior.mcts_untried_random = 0.0;
	rows.push(("enhanced+prior_pw", pw_prior));
	let mut reuse_off = SearchConfig::enhanced();
	reuse_off.mcts_tree_reuse = false;
	rows.push(("enhanced+noreuse", reuse_off));
	let mut mcts_tt = SearchConfig::enhanced();
	mcts_tt.mcts_transpositions = true;
	rows.push(("enhanced+mcts_tt", mcts_tt));
	let mut factored = SearchConfig::enhanced();
	factored.mcts_factored_moves = true;
	rows.push(("enhanced+factored", factored));
	let mut leaf = SearchConfig::enhanced();
	leaf.playout_leaf_depth = 1;
	rows.push(("enhanced+leaf1", leaf));
	let mut solver = SearchConfig::enhanced();
	solver.mcts_solver = true;
	rows.push(("enhanced+solver", solver));
	rows.push(("deep_mcts", SearchConfig::deep_mcts()));
	rows
}

fn run_ablate(cfg: &BenchConfig) {
	let started = Instant::now();
	let mut lines: Vec<String> = Vec::new();
	lines.push(format!("# search ablation  {}", cfg.limit.label()));
	lines.push(format!(
		"# positions: opening seeds 1,2,3 and 6 greedy plies from seed 4. jobs={}",
		cfg.jobs
	));
	let header =
		"# profile  pos  mm_depth  mm_nodes  mcts_avg  mcts_max  mcts_iters  entropy  eff_branch  top1  near_q  q_spread10  stability  legal  mm_ms  mcts_ms";
	lines.push(header.into());
	println!("{header}");
	let mut positions = Vec::new();
	for seed in [cfg.seed, cfg.seed.wrapping_add(1), cfg.seed.wrapping_add(2)] {
		let game = Game::start_with(cfg.rules, seed, cfg.scenario.as_ref(), cfg.setup)
			.unwrap_or_else(|e| {
				eprintln!("{e}");
				std::process::exit(1);
			});
		positions.push((format!("open{seed}"), game));
	}
	let mut mid =
		Game::start_with(cfg.rules, cfg.seed.wrapping_add(3), cfg.scenario.as_ref(), cfg.setup)
			.unwrap_or_else(|e| {
				eprintln!("{e}");
				std::process::exit(1);
			});
	let mut rng = StdRng::seed_from_u64(cfg.seed ^ 0xAB1A);
	for _ in 0..6 {
		if mid.status().is_over() {
			break;
		}
		let action = ai::choose(&mid, Actor::Greedy, &mut rng, Duration::from_millis(1));
		if mid.apply(action).is_err() {
			break;
		}
	}
	positions.push(("mid6".into(), mid));

	for (name, profile) in ablation_profiles() {
		for (pos_name, game) in &positions {
			let skip_mm = name.starts_with("enhanced+") || name == "deep_mcts";
			let mut mm = Searcher::with_config(profile);
			let mut mcts = Searcher::with_config(profile);
			let mut rng_mm = StdRng::seed_from_u64(cfg.seed ^ 0x11);
			let mut rng_mc = StdRng::seed_from_u64(cfg.seed ^ 0x22);
			let t0 = Instant::now();
			let st = if skip_mm {
				apa_kan::ai::SearchStats::default()
			} else {
				let _ = mm.choose_with(game, Actor::Minimax, &mut rng_mm, cfg.limit);
				mm.stats()
			};
			let mm_ms = t0.elapsed().as_secs_f64() * 1000.0;
			let t1 = Instant::now();
			let _ = mcts.choose_with(game, Actor::Mcts, &mut rng_mc, cfg.limit);
			let mcts_ms = t1.elapsed().as_secs_f64() * 1000.0;
			let mt = mcts.stats();
			let line = format!(
				"{name}  {pos_name}  mm_depth={} mm_nodes={}  mcts_avg={:.2} mcts_max={} mcts_iters={}  entropy={:.2} eff_branch={:.1} top1={:.2} near_q={} q_spread10={:.3} stability={:.2} legal={}  mm_ms={mm_ms:.0} mcts_ms={mcts_ms:.0}",
				st.depth, st.nodes, mt.mcts_avg_depth, mt.mcts_max_depth, mt.iterations,
				mt.root_entropy, mt.root_eff_branch, mt.root_top1_share, mt.root_near_best_q,
				mt.root_q_spread_top10, mt.root_best_stability, mt.root_legal
			);
			println!("{line}");
			lines.push(line);
		}
	}
	lines.push(format!("# elapsed_sec={:.1}", started.elapsed().as_secs_f64()));
	let body = lines.join("\n") + "\n";
	if let Err(err) = fs::write(&cfg.out, &body) {
		eprintln!("failed to write {}: {err}", cfg.out.display());
		std::process::exit(1);
	}
	println!("wrote {}", cfg.out.display());
}

fn run_ply_probe(cfg: &BenchConfig) {
	let target = cfg.ply_probe.unwrap_or(10);
	let started = Instant::now();
	let mut lines: Vec<String> = Vec::new();
	lines.push(format!(
		"# minimax ply probe target={} search={}",
		target,
		seat_search(cfg, true).label()
	));
	lines.push(
		"# Each row times one searcher.choose at the work/time limit on that position.".into(),
	);
	let mut positions: Vec<(String, Game)> = Vec::new();
	for (tag, seed) in [("open", cfg.seed), ("open2", cfg.seed.wrapping_add(5))] {
		let game = Game::start_with(cfg.rules, seed, cfg.scenario.as_ref(), cfg.setup)
			.unwrap_or_else(|e| {
				eprintln!("{e}");
				std::process::exit(1);
			});
		positions.push((tag.into(), game));
	}
	let mut late = Game::start_with(cfg.rules, cfg.seed.wrapping_add(9), cfg.scenario.as_ref(), {
		let mut s = cfg.setup;
		s.p1_deck = DeckArchetype::BraidHeavy;
		s.p2_deck = DeckArchetype::BeadHeavy;
		s
	})
	.unwrap_or_else(|e| {
		eprintln!("{e}");
		std::process::exit(1);
	});
	let mut rng = StdRng::seed_from_u64(cfg.seed ^ 0x10);
	for _ in 0..24 {
		if late.status().is_over() {
			break;
		}
		let action = ai::choose(&late, Actor::Greedy, &mut rng, Duration::from_millis(1));
		if late.apply(action).is_err() {
			break;
		}
	}
	positions.push((format!("braid_vs_bead_ply{}", late.turn_count()), late));

	for (name, game) in &positions {
		let mut searcher = Searcher::with_config(seat_search(cfg, true));
		let mut rng = StdRng::seed_from_u64(cfg.seed ^ 0x33);
		let t0 = Instant::now();
		let _ = searcher.choose_with(game, Actor::Minimax, &mut rng, cfg.limit);
		let ms = t0.elapsed().as_secs_f64() * 1000.0;
		let st = searcher.stats();
		let line = format!(
			"{name}  completed_depth={} nodes={} ms={ms:.0} legal={} target={target} reached={}",
			st.depth,
			st.nodes,
			game.legal_actions().len(),
			st.depth >= target
		);
		println!("{line}");
		lines.push(line);
	}
	lines.push(format!("# elapsed_sec={:.1}", started.elapsed().as_secs_f64()));
	let body = lines.join("\n") + "\n";
	if let Err(err) = fs::write(&cfg.out, &body) {
		eprintln!("failed to write {}: {err}", cfg.out.display());
		std::process::exit(1);
	}
	println!("wrote {}", cfg.out.display());
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
	material_value: Option<MaterialValueResolution>,
	turns: u32,
	max_repeat: u32,
	max_occ_repeat: u32,
	depth_sum: f64,
	depth_n: u32,
	depth_max: u8,
	iter_sum: f64,
	iter_n: u32,
	mcts_depth_sum: f64,
	mcts_depth_n: u32,
	mcts_depth_max: u8,
	root_entropy_sum: f64,
	root_eff_branch_sum: f64,
	root_top1_sum: f64,
	root_near_q_sum: f64,
	root_q_spread_sum: f64,
	root_stability_sum: f64,
	root_legal_sum: f64,
	root_spread_n: u32,
	captures: u32,
	self_captures: u32,
	center_captures: u32,
	center_self_captures: u32,
	bench_captures: u32,
	token_captures: u32,
	token_sum: u64,
	bench_len_sum: u64,
	snapshots: u32,
	p1_open_dots: u32,
	p1_open_n: u32,
	bead_contrib: u32,
	bead_needed: u32,
	spawn_two_ring_single_bead: u32,
	claims: u32,
	claim_own: u32,
	claim_opp: u32,
	healed: u32,
	crescent_saves: u32,
	zs_bonuses: u32,
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
	setup: SetupConfig,
	p1_search: SearchConfig,
	p2_search: SearchConfig,
) -> PlayResult {
	let mut game = Game::start_with(rules, seed, scenario, setup).unwrap_or_else(|e| {
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
	let mut s1 = Searcher::with_config(p1_search);
	let mut s2 = Searcher::with_config(p2_search);
	let mut visible: HashMap<u64, u32> = HashMap::new();
	let mut occupancy: HashMap<u64, u32> = HashMap::new();
	let mut max_repeat = 0u32;
	let mut max_occ = 0u32;
	let mut depth_sum = 0.0;
	let mut depth_n = 0u32;
	let mut depth_max = 0u8;
	let mut iter_sum = 0.0;
	let mut iter_n = 0u32;
	let mut mcts_depth_sum = 0.0;
	let mut mcts_depth_n = 0u32;
	let mut mcts_depth_max = 0u8;
	let mut root_entropy_sum = 0.0;
	let mut root_eff_branch_sum = 0.0;
	let mut root_top1_sum = 0.0;
	let mut root_near_q_sum = 0.0;
	let mut root_q_spread_sum = 0.0;
	let mut root_stability_sum = 0.0;
	let mut root_legal_sum = 0.0;
	let mut root_spread_n = 0u32;
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
		log.p1_search = p1_search.label();
		log.p2_search = p2_search.label();
		log
	});
	let mut captures = 0u32;
	let mut self_captures = 0u32;
	let mut center_captures = 0u32;
	let mut center_self_captures = 0u32;
	let mut bench_captures = 0u32;
	let mut token_captures = 0u32;
	let mut bead_contrib = 0u32;
	let mut bead_needed = 0u32;
	let mut spawn_two_ring_single_bead = 0u32;
	let mut claims = 0u32;
	let mut claim_own = 0u32;
	let mut claim_opp = 0u32;
	let mut healed = 0u32;
	let mut crescent_saves = 0u32;
	let mut zs_bonuses = 0u32;
	let mut token_sum = 0u64;
	let mut bench_len_sum = 0u64;
	let mut snapshots = 0u32;
	let p1_open_dots = game.bench(Player::One).iter().filter(|t| t.capture.is_dot()).count() as u32;
	let p1_open_n = game.bench(Player::One).len() as u32;
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
		snapshots += 1;
		token_sum += game.budget_token_count(Player::One) as u64
			+ game.budget_token_count(Player::Two) as u64;
		bench_len_sum +=
			game.bench(Player::One).len() as u64 + game.bench(Player::Two).len() as u64;
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
			depth_max = depth_max.max(st.depth);
		}
		if actor == Actor::Mcts {
			iter_sum += f64::from(st.iterations);
			iter_n += 1;
			mcts_depth_sum += f64::from(st.mcts_avg_depth);
			mcts_depth_n += 1;
			mcts_depth_max = mcts_depth_max.max(st.mcts_max_depth);
			root_entropy_sum += f64::from(st.root_entropy);
			root_eff_branch_sum += f64::from(st.root_eff_branch);
			root_top1_sum += f64::from(st.root_top1_share);
			root_near_q_sum += f64::from(st.root_near_best_q);
			root_q_spread_sum += f64::from(st.root_q_spread_top10);
			root_stability_sum += f64::from(st.root_best_stability);
			root_legal_sum += f64::from(st.root_legal);
			root_spread_n += 1;
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
			Action::Claim { .. } | Action::Pass => {}
		}
		let before_overwhelmed: Vec<(u8, u32)> = game
			.tiles()
			.filter(|(n, _)| game.is_overwhelmed(*n))
			.map(|(n, t)| (n, t.id))
			.collect();
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
				bench_captures += outcome
					.removals
					.iter()
					.filter(|r| r.dest == apa_kan::game::CaptureDest::Bench)
					.count() as u32;
				token_captures += outcome
					.removals
					.iter()
					.filter(|r| r.dest == apa_kan::game::CaptureDest::Tokens)
					.count() as u32;
				bead_contrib += outcome.removals.iter().filter(|r| r.bead_contrib).count() as u32;
				bead_needed += outcome.removals.iter().filter(|r| r.bead_needed).count() as u32;
				spawn_two_ring_single_bead +=
					outcome.removals.iter().filter(|r| r.spawn_two_ring_single_bead).count() as u32;
				crescent_saves += u32::from(outcome.emblem.crescent_saves);
				zs_bonuses += u32::from(outcome.emblem.zs_bonuses);
				if matches!(action, Action::Claim { .. }) {
					claims += 1;
					claim_own +=
						outcome.removals.iter().filter(|r| r.tile.owner == outcome.player).count()
							as u32;
					claim_opp +=
						outcome.removals.iter().filter(|r| r.tile.owner != outcome.player).count()
							as u32;
				}
				let healed_now = before_overwhelmed
					.iter()
					.filter(|(n, id)| {
						game.tile_at(*n).is_some_and(|t| t.id == *id) && !game.is_overwhelmed(*n)
					})
					.count() as u32;
				healed += healed_now;
				if keep_record {
					record_lines.push(format!(
						"{:>3}. {}   [mm_depth={} iters={} mcts_depth={:.2}/{}]",
						game.turn_count(),
						outcome.log_line,
						st.depth,
						st.iterations,
						st.mcts_avg_depth,
						st.mcts_max_depth
					));
				}
				if let Some(log) = game_log.as_mut() {
					log.push_action_with_search(action, piece, &outcome, &game, st);
					if let Some(last) = log.actions.last_mut() {
						last.healed = healed_now as u8;
					}
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
	let material_value = bench_log::material_value_of(&game);
	if let Some(log) = game_log.as_mut() {
		log.finish(&game, low_value);
	}

	PlayResult {
		status: game.status(),
		low_value,
		material_value,
		turns: game.turn_count(),
		max_repeat,
		max_occ_repeat: max_occ,
		depth_sum,
		depth_n,
		depth_max,
		iter_sum,
		iter_n,
		mcts_depth_sum,
		mcts_depth_n,
		mcts_depth_max,
		root_entropy_sum,
		root_eff_branch_sum,
		root_top1_sum,
		root_near_q_sum,
		root_q_spread_sum,
		root_stability_sum,
		root_legal_sum,
		root_spread_n,
		captures,
		self_captures,
		center_captures,
		center_self_captures,
		bench_captures,
		token_captures,
		token_sum,
		bench_len_sum,
		snapshots,
		p1_open_dots,
		p1_open_n,
		bead_contrib,
		bead_needed,
		spawn_two_ring_single_bead,
		claims,
		claim_own,
		claim_opp,
		healed,
		crescent_saves,
		zs_bonuses,
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
	let mut bead_rings = 0u32;
	let mut bead_pieces = 0u32;
	let mut crescent = 0u32;
	let mut zs = 0u32;
	let mut blocker = 0u32;
	let mut rosette = 0u32;
	let mut rings = 0u32;
	for _ in 0..n {
		let t = sample_tile_from_rules(Player::One, &rules, &mut next_id, &mut rng);
		tally_tiles(std::iter::once(&t), &mut ring_n, &mut color_n);
		let k = t.n_rings.clamp(1, 3) as usize;
		zag_rings += u32::from(t.zag_ring_count());
		let b = t.braid_ring_count();
		braid_rings += u32::from(b);
		if b >= 2 {
			braid_pieces += 1;
		}
		bead_rings += u32::from(t.bead_ring_count());
		if t.bead_ring_count() >= 2 {
			bead_pieces += 1;
		}
		if t.emblem.is_crescent() {
			crescent += 1;
		}
		if t.emblem.is_two_zag_zs() {
			zs += 1;
		}
		if t.emblem.is_blocker() {
			blocker += 1;
		}
		if t.emblem.is_rosette() {
			rosette += 1;
		}
		rings += k as u32;
	}
	lines.push(format!(
		"# ring_process  sample={n} max_rings={} zag_numer={} braid_numer={} bead_numer={}  n1={} n2={} n3={}",
		rules.max_rings, rules.zag_numer, rules.braid_numer, rules.bead_numer, ring_n[1], ring_n[2], ring_n[3]
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
		lines.push(format!(
			"# ring_process_beads  pieces={n} grouped={bead_pieces} piece_rate={:.4} rings={rings} beads={bead_rings} ring_rate={:.4}",
			f64::from(bead_pieces) / f64::from(n),
			f64::from(bead_rings) / f64::from(rings)
		));
		lines.push(format!(
			"# ring_process_emblems  pieces={n} weights={} crescent={crescent} rate={:.4} zs={zs} zs_rate={:.4} blocker={blocker} blocker_rate={:.4} rosette={rosette} rosette_rate={:.4}",
			rules.emblem_weights.label(),
			f64::from(crescent) / f64::from(n),
			f64::from(zs) / f64::from(n),
			f64::from(blocker) / f64::from(n),
			f64::from(rosette) / f64::from(n)
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
					|| line.starts_with("# material_value_tiers")
					|| line.starts_with("# captures")
					|| line.starts_with("# center_captures")
					|| line.starts_with("# capture_dest")
					|| line.starts_with("# bead_captures")
					|| line.starts_with("# claims")
					|| line.starts_with("# emblems")
					|| line.starts_with("# search_depth")
					|| line.starts_with("# root_spread")
					|| line.starts_with("# matchup_totals")
					|| line.starts_with("# archetype_seat")
					|| line.starts_with("# short_games")
					|| line.starts_with("# tokens_bench_over_time")
					|| line.starts_with("# pick_dot_ratio")
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
		assert_eq!(cfg.matchups, vec![(DeckArchetype::Mixed, DeckArchetype::Mixed)]);
	}

	#[test]
	fn parse_search_profile_and_per_seat() {
		let cfg = parse(vec![
			"--search-profile".to_string(),
			"classic".to_string(),
			"--p1-search".to_string(),
			"lmr=on,pvs=on".to_string(),
			"--search-threads".to_string(),
			"4".to_string(),
		])
		.unwrap();
		assert_eq!(cfg.search.profile_name(), "classic");
		assert_eq!(cfg.search.mcts_root_threads, 4);
		let p1 = seat_search(&cfg, true);
		assert!(p1.lmr && p1.pvs);
		assert_eq!(p1.mcts_root_threads, 4);
		assert_eq!(seat_search(&cfg, false).profile_name(), "classic");
	}

	#[test]
	fn parse_personality_and_bot_config() {
		let cfg = parse(vec!["--search-profile".to_string(), "tactical".to_string()]).unwrap();
		assert_eq!(cfg.search.profile_name(), "tactical");
		let path = format!("{}/bots/explorative.toml", env!("CARGO_MANIFEST_DIR"));
		let cfg = parse(vec!["--bot-config".to_string(), path]).unwrap();
		assert_eq!(cfg.search, SearchConfig::explorative());
		assert_eq!(cfg.search.profile_name(), "explorative");
		let cfg = parse(vec!["--search-profile".to_string(), "positional".to_string()]).unwrap();
		assert_eq!(cfg.search, SearchConfig::positional());
		let cfg = parse(vec!["--search-profile".to_string(), "deep_mcts".to_string()]).unwrap();
		assert_eq!(cfg.search, SearchConfig::deep_mcts());
		assert_eq!(cfg.search.profile_name(), "deep_mcts");
	}

	#[test]
	fn parse_pairings_and_deck_matchups() {
		let cfg = parse(vec![
			"--pairings".to_string(),
			"minimax:mcts,mcts:minimax".to_string(),
			"--deck-matchups".to_string(),
			"braid:bead,bead:braid,mixed:mixed".to_string(),
			"--bead-scoring".to_string(),
			"shared".to_string(),
		])
		.unwrap();
		assert_eq!(cfg.pairings.as_ref().map(|p| p.len()), Some(2));
		assert_eq!(cfg.matchups.len(), 3);
		assert_eq!(cfg.matchups[0], (DeckArchetype::BraidHeavy, DeckArchetype::BeadHeavy));
		assert_eq!(cfg.rules.bead_scoring, apa_kan::game::BeadScoring::SharedChannel);
		assert_eq!(cfg.rules.channel_mode, apa_kan::game::ChannelMode::IndependentMax);
		let emblems = parse(vec![
			"--deck-matchups".to_string(),
			"crescent:mixed,zs:emblem,emblem:bead".to_string(),
			"--emblem-weights".to_string(),
			"1,20,5".to_string(),
		])
		.unwrap();
		assert_eq!(emblems.matchups.len(), 3);
		assert_eq!(emblems.matchups[0], (DeckArchetype::Crescent, DeckArchetype::Mixed));
		assert_eq!(emblems.matchups[1], (DeckArchetype::Zs, DeckArchetype::EmblemMix));
		assert_eq!(emblems.rules.emblem_weights.crescent, 20);
		assert_eq!(emblems.rules.emblem_weights.zs, 5);
		assert_eq!(emblems.rules.emblem_weights.blocker, 0);
		assert_eq!(emblems.rules.emblem_weights.rosette, 0);
		let extra = parse(vec![
			"--deck-matchups".to_string(),
			"blocker:mixed,rosette:bead,blocker:rosette".to_string(),
			"--emblem-weights".to_string(),
			"200,1,1,1,1".to_string(),
			"--blocker-blocks".to_string(),
			"pass".to_string(),
		])
		.unwrap();
		assert_eq!(extra.matchups[0], (DeckArchetype::Blocker, DeckArchetype::Mixed));
		assert_eq!(extra.matchups[2], (DeckArchetype::Blocker, DeckArchetype::Rosette));
		assert_eq!(extra.rules.blocker_blocks, apa_kan::game::BlockerBlocks::PassThrough);
		let off = parse(vec!["--emblems".to_string(), "off".to_string()]).unwrap();
		assert_eq!(off.rules.emblem_weights, apa_kan::game::EmblemWeights::off());
	}

	/// Default rules include beads; `--jobs 1` and `--jobs 4` must still
	/// produce identical JSONL (generation, picks, and play).
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
