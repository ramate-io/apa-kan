//! Load a reconstructable JSONL game log and run the bot's search on a turn.

use std::path::PathBuf;
use std::time::Instant;

use apa_kan::ai::{
	self, Actor, SearchConfig, SearchLimit, SearchTrace, Searcher, LIGHT_ITERS_PER_MOVE,
	LIGHT_NODES_PER_MOVE, WIN,
};
use apa_kan::bench_log::GameLog;
use apa_kan::game::{Action, Game};
use apa_kan::play_log;
use rand::rngs::StdRng;
use rand::SeedableRng;

fn main() {
	match run(std::env::args().skip(1)) {
		Ok(()) => {}
		Err(err) => {
			eprintln!("{err}\n\n{}", usage());
			std::process::exit(1);
		}
	}
}

fn usage() -> &'static str {
	"Usage: apa-kan-analyze --log <path.jsonl[.gz]> [OPTIONS]\n\
	 \n\
	 Search one position:\n\
	   --log <path>         JSONL / JSONL.GZ from apa-kan-bench or the playable app\n\
	   --game <n>           1-based record in a multi-game file (default: 1)\n\
	   --action <n>         1-based action index; search the position before it\n\
	   --turn <n>           First logged action whose turn >= n\n\
	   --search-profile <exactplay|enhanced|...>  (default: log's p1_search, else exactplay)\n\
	   --budget nodes:N,iters:M  Work budget (default: light nodes:33000,iters:650)\n\
	   --think-ms <n>       Wall-clock bound instead of --budget\n\
	   --depth <1..16>      Cap iterative deepening (still bounded by --budget/--think-ms)\n\
	 \n\
	 Diagnose a log (branching, depth, prune stats, forced control wins):\n\
	   --diagnose           Measure from --log (default: opening_2211 300-game JSONL)\n\
	   --sample <n>         Positions per phase for timed search (default: 4)\n\
	   --out <path>         Write the diagnose report (default: stdout)\n\
	   -h, --help\n"
}

struct Cfg {
	log: Option<PathBuf>,
	game: u32,
	action: Option<usize>,
	turn: Option<u32>,
	search: SearchConfig,
	search_set: bool,
	limit: SearchLimit,
	limit_set: bool,
	depth: Option<u8>,
	diagnose: bool,
	sample: usize,
	out: Option<PathBuf>,
}

fn parse<I, S>(args: I) -> Result<Cfg, String>
where
	I: IntoIterator<Item = S>,
	S: AsRef<str>,
{
	let mut cfg = Cfg {
		log: None,
		game: 1,
		action: None,
		turn: None,
		search: SearchConfig::exactplay(),
		search_set: false,
		limit: SearchLimit::work(LIGHT_NODES_PER_MOVE, LIGHT_ITERS_PER_MOVE),
		limit_set: false,
		depth: None,
		diagnose: false,
		sample: 4,
		out: None,
	};
	let mut it = args.into_iter();
	while let Some(raw) = it.next() {
		let arg = raw.as_ref();
		match arg {
			"-h" | "--help" => {
				print!("{}", usage());
				std::process::exit(0);
			}
			"--log" => cfg.log = Some(PathBuf::from(need("--log", it.next())?)),
			"--game" => {
				cfg.game = parse_u32("--game", &need("--game", it.next())?)?;
				if cfg.game == 0 {
					return Err("--game is 1-based".into());
				}
			}
			"--action" => {
				let n: usize = need("--action", it.next())?
					.parse()
					.map_err(|_| "invalid --action".to_string())?;
				cfg.action = Some(n);
			}
			"--turn" => cfg.turn = Some(parse_u32("--turn", &need("--turn", it.next())?)?),
			"--search-profile" => {
				cfg.search = SearchConfig::parse_profile(&need("--search-profile", it.next())?)?;
				cfg.search_set = true;
			}
			"--budget" => {
				cfg.limit = parse_budget(&need("--budget", it.next())?)?;
				cfg.limit_set = true;
			}
			"--think-ms" => {
				let ms = parse_u32("--think-ms", &need("--think-ms", it.next())?)?;
				cfg.limit = SearchLimit::time_ms(u64::from(ms));
				cfg.limit_set = true;
			}
			"--depth" => {
				let n = parse_u32("--depth", &need("--depth", it.next())?)?;
				if !(1..=16).contains(&n) {
					return Err("invalid --depth (expected 1..16)".into());
				}
				cfg.depth = Some(n as u8);
			}
			"--diagnose" => cfg.diagnose = true,
			"--sample" => {
				let n = parse_u32("--sample", &need("--sample", it.next())?)?;
				cfg.sample = n.max(1) as usize;
			}
			"--out" => cfg.out = Some(PathBuf::from(need("--out", it.next())?)),
			other => return Err(format!("unknown argument '{other}'")),
		}
	}
	Ok(cfg)
}

fn run<I, S>(args: I) -> Result<(), String>
where
	I: IntoIterator<Item = S>,
	S: AsRef<str>,
{
	let cfg = parse(args)?;
	if cfg.diagnose {
		return diagnose(&cfg);
	}
	let path = cfg.log.as_ref().ok_or("--log is required")?;
	let logs = play_log::load_path(path)?;
	if logs.is_empty() {
		return Err(format!("{} contains no games", path.display()));
	}
	let idx = cfg.game.saturating_sub(1) as usize;
	let log = logs.get(idx).ok_or_else(|| {
		format!("--game {} is past the end of the file ({} games)", cfg.game, logs.len())
	})?;
	let (game, action_1based, pending) = position_from_log(log, &cfg)?;
	let search = if cfg.search_set { cfg.search } else { search_from_log(log) };
	let mut searcher = Searcher::with_config(search);
	if let Some(d) = cfg.depth {
		searcher.set_max_id_depth(d);
	}
	let mut rng = StdRng::seed_from_u64(log.search_rng_seed);
	let t0 = Instant::now();
	let action = searcher.choose_with(&game, Actor::Minimax, &mut rng, cfg.limit);
	let ms = t0.elapsed().as_secs_f64() * 1000.0;
	let st = searcher.stats();
	let tr = searcher.trace();
	let nps = if ms > 0.0 { st.nodes as f64 / (ms / 1000.0) } else { 0.0 };
	println!(
		"log={} game={} action={} turn={}",
		path.display(),
		log.game,
		action_1based,
		game.turn_count()
	);
	println!(
		"to_move={} legal={} search={}",
		game.current_player().label(),
		game.legal_actions().len(),
		search.label()
	);
	println!("limit={} max_id_depth={}", cfg.limit.label(), cfg.depth.unwrap_or(16));
	if let Some(played) = pending {
		println!("logged_move={}", fmt_action(played));
	}
	println!(
		"best={} score={} depth={} nodes={} nps={:.0} ms={:.1}",
		fmt_action(action),
		st.score,
		st.depth,
		st.nodes,
		nps,
		ms
	);
	println!("pv={}", fmt_pv(searcher.principal_variation()));
	print_trace(&tr);
	Ok(())
}

fn position_from_log(log: &GameLog, cfg: &Cfg) -> Result<(Game, usize, Option<Action>), String> {
	if let Some(a) = cfg.action {
		let (game, pending) = play_log::replay_before_action(log, a)?;
		Ok((game, a, pending))
	} else if let Some(turn) = cfg.turn {
		let (game, action_1based) = play_log::replay_at_turn(log, turn)?;
		let pending = log
			.actions
			.get(action_1based.saturating_sub(1))
			.map(|a| a.to_action())
			.transpose()?;
		Ok((game, action_1based, pending))
	} else {
		Err("specify --action N or --turn N (or --diagnose)".into())
	}
}

fn search_from_log(log: &GameLog) -> SearchConfig {
	if log.p1_search.contains("profile=exactplay") || log.p1_search.is_empty() {
		SearchConfig::exactplay()
	} else if log.p1_search.contains("profile=enhanced") {
		SearchConfig::enhanced()
	} else if log.p1_search.contains("profile=counterplay") {
		SearchConfig::counterplay()
	} else {
		SearchConfig::exactplay()
	}
}

fn diagnose(cfg: &Cfg) -> Result<(), String> {
	let default_log = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
		.join("benchmark_results/opening_2211/h2h_n300_mm_r1.jsonl.gz");
	let path = cfg.log.clone().unwrap_or(default_log);
	if !path.exists() {
		return Err(format!("diagnose log not found: {}", path.display()));
	}
	eprintln!("loading {} ...", path.display());
	let logs = play_log::load_path(&path)?;
	eprintln!("loaded {} games", logs.len());
	let mut lines = Vec::new();
	lines.push(format!("# search depth diagnosis  log={}  games={}", path.display(), logs.len()));
	lines.push(format!("# search={}", cfg.search.label()));
	lines.push(format!("# {}", cfg.limit.label()));
	lines.extend(branching_report(&logs)?);
	lines.extend(search_sample_report(&logs, cfg)?);
	lines.extend(forced_control_report(&logs, cfg)?);
	let body = lines.join("\n") + "\n";
	if let Some(out) = cfg.out.as_ref() {
		std::fs::write(out, &body)
			.map_err(|e| format!("failed to write {}: {e}", out.display()))?;
		println!("wrote {}", out.display());
	} else {
		print!("{body}");
	}
	Ok(())
}

#[derive(Default)]
struct VerbAcc {
	n: u64,
	sum: u64,
	max: u32,
}

impl VerbAcc {
	fn add(&mut self, v: u32) {
		self.n += 1;
		self.sum += u64::from(v);
		self.max = self.max.max(v);
	}

	fn avg(&self) -> f64 {
		if self.n == 0 {
			0.0
		} else {
			self.sum as f64 / self.n as f64
		}
	}
}

#[derive(Default)]
struct PhaseAcc {
	positions: u64,
	add: VerbAcc,
	mv: VerbAcc,
	swap: VerbAcc,
	claim: VerbAcc,
	convert: VerbAcc,
	all: VerbAcc,
	search_all: VerbAcc,
}

impl PhaseAcc {
	fn record(&mut self, game: &Game) {
		self.positions += 1;
		self.add.add(game.legal_adds().len() as u32);
		self.mv.add(game.legal_moves().len() as u32);
		self.swap.add(game.legal_swaps().len() as u32);
		self.claim.add(game.legal_claims().len() as u32);
		self.convert.add(game.legal_converts().len() as u32);
		self.all.add(game.legal_actions().len() as u32);
		self.search_all.add(ai::legal_search_moves(game).len() as u32);
	}

	fn row(&self, name: &str) -> String {
		format!(
			"{name}  n={}  all={:.1}/{}  search={:.1}/{}  add={:.1}/{}  move={:.1}/{}  swap={:.1}/{}  claim={:.1}/{}  convert={:.1}/{}",
			self.positions,
			self.all.avg(),
			self.all.max,
			self.search_all.avg(),
			self.search_all.max,
			self.add.avg(),
			self.add.max,
			self.mv.avg(),
			self.mv.max,
			self.swap.avg(),
			self.swap.max,
			self.claim.avg(),
			self.claim.max,
			self.convert.avg(),
			self.convert.max
		)
	}
}

fn phase_of(turn: u32) -> &'static str {
	if turn <= 8 {
		"early"
	} else if turn <= 35 {
		"mid"
	} else {
		"late"
	}
}

fn branching_report(logs: &[GameLog]) -> Result<Vec<String>, String> {
	let mut early = PhaseAcc::default();
	let mut mid = PhaseAcc::default();
	let mut late = PhaseAcc::default();
	let mut ratchet = PhaseAcc::default();
	let mut all = PhaseAcc::default();
	let mut control_n = 0u32;
	let mut lockout_n = 0u32;
	let mut claim_win_n = 0u32;
	for log in logs {
		match log.end.reason.as_deref() {
			Some("control") => control_n += 1,
			Some("locked_out") => lockout_n += 1,
			Some("elimination") => claim_win_n += 1,
			_ => {}
		}
		let mut game = play_log::replay_prefix(log, 0)?;
		for act in &log.actions {
			let ratchet_turn = act.played_as_ratchet || game.ratchet_reward_active();
			match phase_of(game.turn_count().max(1)) {
				"early" => early.record(&game),
				"mid" => mid.record(&game),
				_ => late.record(&game),
			}
			if ratchet_turn {
				ratchet.record(&game);
			}
			all.record(&game);
			let action = act.to_action()?;
			if game.apply(action).is_err() {
				break;
			}
		}
	}
	Ok(vec![
		String::new(),
		"# branching (legal actions by phase; search= kind-collapsed ADD)".into(),
		format!("# terminals: control={control_n} lockout={lockout_n} elimination={claim_win_n}"),
		all.row("all"),
		early.row("early(t<=8)"),
		mid.row("mid(t<=35)"),
		late.row("late"),
		ratchet.row("ratchet"),
	])
}

fn search_sample_report(logs: &[GameLog], cfg: &Cfg) -> Result<Vec<String>, String> {
	let mut lines = vec![
		String::new(),
		"# timed exactplay minimax on sampled positions (light budget unless overridden)".into(),
		"# phase  game  action  turn  legal  depth  score  nodes  nps  ms  first_cut%  tt_cut%  lmr  pvs_re  same_side  q_nodes  term_w/l  first_win_ply  max_ply".into(),
	];
	let mut picked: Vec<(&str, usize, usize)> = Vec::new();
	let want = cfg.sample;
	let mut n_early = 0usize;
	let mut n_mid = 0usize;
	let mut n_late = 0usize;
	for (li, log) in logs.iter().take(24).enumerate() {
		for (i, _act) in log.actions.iter().enumerate() {
			let turn = if i == 0 { 1 } else { log.actions[i.saturating_sub(1)].turn.max(1) };
			let phase = phase_of(turn);
			let slot = match phase {
				"early" if n_early < want => {
					n_early += 1;
					Some("early")
				}
				"mid" if n_mid < want => {
					n_mid += 1;
					Some("mid")
				}
				"late" if n_late < want => {
					n_late += 1;
					Some("late")
				}
				_ => None,
			};
			if let Some(p) = slot {
				picked.push((p, li, i + 1));
			}
		}
		if n_early >= want && n_mid >= want && n_late >= want {
			break;
		}
	}
	for (phase, li, action_1based) in picked {
		let Some(log) = logs.get(li) else {
			continue;
		};
		let game = play_log::replay_prefix(log, action_1based - 1)?;
		let line = run_search_line(phase, log, action_1based, &game, cfg.search, cfg.limit, None);
		eprintln!("{line}");
		lines.push(line);
	}
	Ok(lines)
}

fn run_search_line(
	phase: &str,
	log: &GameLog,
	action_1based: usize,
	game: &Game,
	search: SearchConfig,
	limit: SearchLimit,
	depth: Option<u8>,
) -> String {
	let mut searcher = Searcher::with_config(search);
	if let Some(d) = depth {
		searcher.set_max_id_depth(d);
	}
	let mut rng =
		StdRng::seed_from_u64(log.search_rng_seed ^ u64::from(log.game) ^ action_1based as u64);
	let t0 = Instant::now();
	let _ = searcher.choose_with(game, Actor::Minimax, &mut rng, limit);
	let ms = t0.elapsed().as_secs_f64() * 1000.0;
	let st = searcher.stats();
	let tr = searcher.trace();
	let nps = if ms > 0.0 { st.nodes as f64 / (ms / 1000.0) } else { 0.0 };
	let first_pct = if tr.beta_cuts == 0 {
		0.0
	} else {
		100.0 * tr.first_move_cuts as f64 / tr.beta_cuts as f64
	};
	let tt_pct =
		if tr.tt_probes == 0 { 0.0 } else { 100.0 * tr.tt_cuts as f64 / tr.tt_probes as f64 };
	format!(
		"{phase}  g{}  a{}  t{}  legal={}  depth={}  score={}  nodes={}  nps={:.0}  ms={:.0}  first_cut={:.0}%  tt_cut={:.1}%  lmr={}/{}  pvs_re={}  same_side={}  q={}  term={}/{}  win_ply={}  max_ply={}",
		log.game,
		action_1based,
		game.turn_count(),
		game.legal_actions().len(),
		st.depth,
		st.score,
		st.nodes,
		nps,
		ms,
		first_pct,
		tt_pct,
		tr.lmr_reductions,
		tr.lmr_researches,
		tr.pvs_researches,
		tr.same_side_children,
		tr.q_nodes,
		tr.terminal_wins,
		tr.terminal_losses,
		tr.first_win_ply,
		tr.max_ply
	)
}

fn forced_control_report(logs: &[GameLog], cfg: &Cfg) -> Result<Vec<String>, String> {
	let mut lines = vec![
		String::new(),
		"# forced control wins: last N actions of control-ended games; min ID depth to score > WIN/2".into(),
	];
	let control: Vec<&GameLog> = logs
		.iter()
		.filter(|l| l.end.reason.as_deref() == Some("control"))
		.take(4)
		.collect();
	if control.is_empty() {
		lines.push("# no control-ended games in this log".into());
		return Ok(lines);
	}
	let budgets = [
		("1x-light", SearchLimit::work(LIGHT_NODES_PER_MOVE, LIGHT_ITERS_PER_MOVE)),
		("4x", SearchLimit::work(LIGHT_NODES_PER_MOVE * 4, LIGHT_ITERS_PER_MOVE * 4)),
	];
	for log in control {
		let n = log.actions.len();
		if n == 0 {
			continue;
		}
		lines.push(format!(
			"# game {} winner={:?} kind={:?} turns={} actions={}",
			log.game, log.end.winner, log.end.control_win_kind, log.turns, n
		));
		for back in [1usize, 2, 3] {
			if back > n {
				continue;
			}
			let action_1based = n - back + 1;
			let game = play_log::replay_prefix(log, action_1based - 1)?;
			let min = min_depth_for_win(&game, log, cfg.search);
			let line = match min {
				Some((depth, nodes, ms, score)) => {
					format!(
						"  back={back} t={} legal={}  min_depth={depth} nodes={nodes} ms={ms:.0} score={score} winish={}",
						game.turn_count(),
						game.legal_actions().len(),
						score > WIN / 2
					)
				}
				None => format!(
					"  back={back} t={} legal={}  min_depth=none (no win score through depth 8)",
					game.turn_count(),
					game.legal_actions().len()
				),
			};
			eprintln!("{line}");
			lines.push(line);
			if back == 1 || back == 3 {
				let budget_slice = if back == 1 { budgets.as_slice() } else { &budgets[..2] };
				for (label, limit) in budget_slice {
					let line = run_search_line(
						&format!("ctrl-{label}-b{back}"),
						log,
						action_1based,
						&game,
						cfg.search,
						*limit,
						None,
					);
					eprintln!("{line}");
					lines.push(format!("    {line}"));
				}
			}
		}
	}
	Ok(lines)
}

fn min_depth_for_win(
	game: &Game,
	log: &GameLog,
	search: SearchConfig,
) -> Option<(u8, u64, f64, i32)> {
	for depth in 1..=8u8 {
		let mut searcher = Searcher::with_config(search);
		searcher.set_max_id_depth(depth);
		let mut rng = StdRng::seed_from_u64(log.search_rng_seed ^ 0xD1);
		let t0 = Instant::now();
		let _ =
			searcher.choose_with(game, Actor::Minimax, &mut rng, SearchLimit::work(250_000, 5_000));
		let ms = t0.elapsed().as_secs_f64() * 1000.0;
		let st = searcher.stats();
		if st.score > WIN / 2 {
			return Some((st.depth, st.nodes, ms, st.score));
		}
	}
	None
}

fn print_trace(tr: &SearchTrace) {
	println!(
		"trace tt_probes={} hits={} cuts={}  beta_cuts={} first_move_cuts={} ({:.0}%)",
		tr.tt_probes,
		tr.tt_hits,
		tr.tt_cuts,
		tr.beta_cuts,
		tr.first_move_cuts,
		if tr.beta_cuts == 0 {
			0.0
		} else {
			100.0 * tr.first_move_cuts as f64 / tr.beta_cuts as f64
		}
	);
	println!(
		"trace lmr={}/{} pvs_null={} pvs_re={} q_nodes={} same_side={} opening={} ratchet={}",
		tr.lmr_reductions,
		tr.lmr_researches,
		tr.pvs_nulls,
		tr.pvs_researches,
		tr.q_nodes,
		tr.same_side_children,
		tr.opening_children,
		tr.ratchet_children
	);
	println!(
		"trace terminals={} wins={} losses={} draws={} first_win_ply={} first_loss_ply={} max_ply={}",
		tr.terminals,
		tr.terminal_wins,
		tr.terminal_losses,
		tr.terminal_draws,
		tr.first_win_ply,
		tr.first_loss_ply,
		tr.max_ply
	);
}

fn fmt_action(action: Action) -> String {
	match action {
		Action::Add { bench_index, node } => format!("ADD bench{bench_index}->{node}"),
		Action::Move { from, to } => format!("MOVE {from}->{to}"),
		Action::Swap { from, to } => format!("SWAP {from}<->{to}"),
		Action::Claim { node } => format!("CLAIM {node}"),
		Action::Convert { node } => format!("CONVERT {node}"),
		Action::Pass => "PASS".into(),
	}
}

fn fmt_pv(pv: &[Action]) -> String {
	if pv.is_empty() {
		"-".into()
	} else {
		pv.iter().map(|a| fmt_action(*a)).collect::<Vec<_>>().join("  ")
	}
}

fn need(flag: &str, v: Option<impl AsRef<str>>) -> Result<String, String> {
	v.map(|s| s.as_ref().to_string())
		.ok_or_else(|| format!("missing value for {flag}"))
}

fn parse_u32(flag: &str, v: &str) -> Result<u32, String> {
	v.parse().map_err(|_| format!("invalid {flag} '{v}'"))
}

fn parse_budget(spec: &str) -> Result<SearchLimit, String> {
	let spec = spec.trim();
	if spec == "light" {
		return Ok(SearchLimit::work(LIGHT_NODES_PER_MOVE, LIGHT_ITERS_PER_MOVE));
	}
	if spec == "calibrated" {
		return Ok(SearchLimit::calibrated());
	}
	let mut nodes = LIGHT_NODES_PER_MOVE;
	let mut iters = LIGHT_ITERS_PER_MOVE;
	for part in spec.split(',') {
		let (k, v) = part.split_once(':').ok_or_else(|| {
			format!("invalid --budget '{spec}' (expected nodes:N,iters:M|light|calibrated)")
		})?;
		match k.trim() {
			"nodes" => nodes = v.trim().parse().map_err(|_| format!("invalid nodes '{v}'"))?,
			"iters" => iters = v.trim().parse().map_err(|_| format!("invalid iters '{v}'"))?,
			other => return Err(format!("unknown --budget key '{other}'")),
		}
	}
	Ok(SearchLimit::work(nodes, iters))
}

#[cfg(test)]
mod tests {
	#![allow(clippy::unwrap_used)]

	use super::*;

	#[test]
	fn parse_budget_light_and_nodes() {
		assert_eq!(parse_budget("light").unwrap(), SearchLimit::work(33_000, 650));
		assert_eq!(parse_budget("nodes:1000,iters:10").unwrap(), SearchLimit::work(1000, 10));
	}

	#[test]
	fn fmt_action_covers_verbs() {
		assert_eq!(fmt_action(Action::Pass), "PASS");
		assert_eq!(fmt_action(Action::Claim { node: 3 }), "CLAIM 3");
	}
}
