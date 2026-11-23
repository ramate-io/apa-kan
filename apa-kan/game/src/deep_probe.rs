//! Deep fan-out search on replayed benchmark positions (capture-score passivity probe).

use std::collections::{HashMap, HashSet};
use std::fmt::Write as _;
use std::thread;

use crate::ai::{
	apply_move, is_capture, legal_search_moves, Actor, SearchConfig, SearchLimit, SearchMove,
	Searcher, CALIBRATED_NODES_PER_MOVE, WIN,
};
use crate::bench_log::{decode_jsonl_gz, replay_to_action, GameLog};
use crate::game::{Action, Game, Player};
use rand::rngs::StdRng;
use rand::SeedableRng;
use std::path::PathBuf;

/// Inputs for a deep probe run (filled from `apa-kan-bench` CLI).
pub struct Config {
	pub probe_logs: Vec<PathBuf>,
	pub probe_budget_mult: Vec<u64>,
	pub probe_fanout: u32,
	pub probe_jobs: usize,
	pub probe_root_shuffle: bool,
	pub probe_out: Option<PathBuf>,
	pub limit: SearchLimit,
	pub search: SearchConfig,
}

const CENTER: u8 = 0;
const INNER: [u8; 8] = [9, 1, 12, 3, 16, 6, 18, 21];

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
struct MoveTag {
	kind: u8,
	from: u8,
	to: u8,
}

impl MoveTag {
	fn from_action(a: &Action) -> Self {
		match a {
			Action::Add { bench_index, node } => {
				Self { kind: 0, from: (*bench_index).min(u8::MAX as usize) as u8, to: *node }
			}
			Action::Move { from, to } => Self { kind: 1, from: *from, to: *to },
			Action::Claim { node } => Self { kind: 2, from: 0, to: *node },
			Action::Swap { from, to } => Self { kind: 3, from: *from, to: *to },
			Action::Convert { node } => Self { kind: 4, from: 0, to: *node },
			Action::Pass => Self { kind: 5, from: 0, to: 0 },
		}
	}

	fn label(self) -> String {
		match self.kind {
			0 => format!("add@{}", self.to),
			1 => format!("move{}->{}", self.from, self.to),
			2 => format!("claim@{}", self.to),
			3 => format!("swap{}<->{}", self.from, self.to),
			4 => format!("convert@{}", self.to),
			_ => "pass".into(),
		}
	}
}

fn dest_ring(node: u8) -> &'static str {
	if node == CENTER {
		"center"
	} else if INNER.contains(&node) {
		"inner"
	} else {
		"outer"
	}
}

fn overwhelming_counts(game: &Game) -> (u8, u8) {
	let mut opp = 0u8;
	let mut own = 0u8;
	let me = game.current_player();
	for node in game.overwhelmed_nodes() {
		if let Some(tile) = game.tile_at(node) {
			if tile.owner == me {
				own += 1;
			} else {
				opp += 1;
			}
		}
	}
	(opp, own)
}

fn immediate_capture_moves(game: &Game) -> u16 {
	let me = game.current_player();
	let mut n = 0u16;
	for mv in legal_search_moves(game) {
		let mut child = game.clone();
		if let Ok(out) = apply_move(&mut child, mv) {
			if is_capture(&out, me) {
				n += 1;
			}
		}
	}
	n
}

fn score_win_visible(game: &Game, me: Player) -> bool {
	let target = game.rules().score_target;
	game.capture_score(me) + 1 >= target && overwhelming_counts(game).0 > 0
}

struct ProbeRun {
	tag: MoveTag,
	depth: u8,
	nodes: u64,
	root_score: i32,
	dest_ring: &'static str,
	proves_terminal: bool,
}

fn action_to_search_move(action: &Action) -> Option<SearchMove> {
	match action {
		Action::Move { from, to } => Some(SearchMove::Move { from: *from, to: *to }),
		Action::Swap { from, to } => Some(SearchMove::Swap { from: *from, to: *to }),
		Action::Convert { node } => Some(SearchMove::Convert { node: *node }),
		_ => None,
	}
}

fn minimax_probe(
	game: &Game,
	played: &[u64],
	last_move: Option<Action>,
	search: SearchConfig,
	nodes: u64,
	rng_seed: u64,
	root_shuffle: bool,
) -> ProbeRun {
	let mut searcher = Searcher::with_config(search);
	searcher.seed_played_hashes(played);
	searcher.set_minimax_last_move(last_move.as_ref().and_then(action_to_search_move));
	searcher.set_root_shuffle(root_shuffle);
	let mut rng = StdRng::seed_from_u64(rng_seed);
	let limit = SearchLimit::Work { nodes, iters: u32::MAX };
	let action = searcher.choose_with(game, Actor::Minimax, &mut rng, limit).clone();
	let st = searcher.stats();
	let dest = match &action {
		Action::Add { node, .. } | Action::Claim { node } | Action::Convert { node } => *node,
		Action::Move { to, .. } | Action::Swap { to, .. } => *to,
		Action::Pass => CENTER,
	};
	let proves_terminal = st.root_score.abs() >= WIN / 2;
	ProbeRun {
		tag: MoveTag::from_action(&action),
		depth: st.depth,
		nodes: st.nodes,
		root_score: st.root_score,
		dest_ring: dest_ring(dest),
		proves_terminal,
	}
}

fn sample_action_indices(log: &GameLog) -> Vec<usize> {
	let mut picks = Vec::new();
	let actions = &log.actions;
	let end = &log.end;
	let stalled = end.reason.as_deref() == Some("turn_limit");
	let mut seen_turns = HashSet::new();

	for (i, act) in actions.iter().enumerate() {
		let turn = act.turn;
		let p1 = act.p1_score;
		let p2 = act.p2_score;
		let tied = p1 == p2 || p1.abs_diff(p2) <= 1;
		if stalled && tied && turn >= 80 && seen_turns.insert(turn) {
			picks.push(i);
		}
	}

	if picks.len() < 5 {
		for (i, act) in actions.iter().enumerate() {
			if act.turn >= 50 && (i % 12 == 5) {
				picks.push(i);
			}
		}
	}

	picks.sort_unstable();
	picks.dedup();
	picks.truncate(4);
	picks
}

#[derive(Clone)]
struct PosTask {
	log_label: String,
	log: GameLog,
	ix: usize,
}

struct PosOutcome {
	row: String,
	changed_16: bool,
	changed_64: bool,
	deep_win_spot: bool,
	shallow_miss: bool,
	imm_cap: bool,
	light_ring: &'static str,
	deep_ring: Option<&'static str>,
	fan_unique: usize,
}

fn probe_position(
	task: &PosTask,
	multipliers: &[u64],
	base_nodes: u64,
	fanout: u32,
	search: SearchConfig,
	root_shuffle: bool,
) -> PosOutcome {
	let log = &task.log;
	let ix = task.ix;
	let (game, played, last) =
		replay_to_action(log, ix).unwrap_or_else(|e| panic!("replay g{} ix{ix}: {e}", log.game));
	let me = game.current_player();
	let act = &log.actions[ix];
	let (claim_opp, claim_own) = overwhelming_counts(&game);
	let imm = immediate_capture_moves(&game);
	let score_win_setup = score_win_visible(&game, me);

	let mut by_mult: HashMap<u64, ProbeRun> = HashMap::new();
	for &mult in multipliers {
		let nodes = base_nodes.saturating_mul(mult);
		let seed = log
			.search_rng_seed
			.wrapping_add((ix as u64).wrapping_mul(0x9E37))
			.wrapping_add(mult.wrapping_mul(0xD1B5));
		let run = minimax_probe(&game, &played, last.clone(), search, nodes, seed, false);
		by_mult.insert(mult, run);
	}

	let light = by_mult.get(&1).or_else(|| by_mult.get(&multipliers[0])).unwrap();
	let mid_mult = multipliers.iter().copied().find(|&m| m >= 16).unwrap_or(multipliers[0]);
	let deep_mult = multipliers.last().copied().unwrap_or(64);
	let mid = by_mult.get(&mid_mult);
	let deep = by_mult.get(&deep_mult);

	let max_mult = *multipliers.last().unwrap_or(&64);
	let mut fanout_tags: HashSet<MoveTag> = HashSet::new();
	if fanout > 0 {
		for f in 0..fanout {
			let seed = log.search_rng_seed
				^ (ix as u64).wrapping_mul(0x517c)
				^ (f as u64).wrapping_mul(0x9E3779B97F4A7C15);
			let nodes = base_nodes.saturating_mul(max_mult);
			let run = minimax_probe(&game, &played, last, search, nodes, seed, root_shuffle);
			fanout_tags.insert(run.tag);
		}
	}

	let changed_16 = mid.map(|m| m.tag != light.tag).unwrap_or(false);
	let changed_64 = deep.map(|d| d.tag != light.tag).unwrap_or(false);
	let deep_win_spot = deep.map(|d| d.proves_terminal && !light.proves_terminal).unwrap_or(false);
	let shallow_miss = imm > 0
		&& matches!((mid, deep), (Some(m), Some(d)) if light.tag != m.tag || light.tag != d.tag);

	let mut row = String::new();
	let _ = write!(
		row,
		"pos\t{}\tg{}\tix{ix}\tt{}\t{}\t{}-{}\tco={claim_opp}\tcs={claim_own}\tic={imm}\tsw={score_win_setup}",
		task.log_label,
		log.game,
		act.turn,
		me.label(),
		act.p1_score,
		act.p2_score,
	);
	for &mult in multipliers {
		if let Some(r) = by_mult.get(&mult) {
			let _ = write!(
				row,
				"\tm{mult}:{} d{} n{} sc{} ring{} term{}",
				r.tag.label(),
				r.depth,
				r.nodes,
				r.root_score,
				r.dest_ring,
				r.proves_terminal as u8
			);
		}
	}
	let _ = write!(row, "\tfan{max_mult}_uniq={} fan{max_mult}_n={fanout}", fanout_tags.len());
	if let (Some(m), Some(d)) = (mid, deep) {
		let _ = write!(
			row,
			"\td16={}\td{deep_mult}={}\tch16={}\tch64={}",
			m.depth,
			d.depth,
			u8::from(changed_16),
			u8::from(changed_64)
		);
	}

	eprintln!(
		"probe {} g{} ix{ix} t{} light={} deep={}",
		task.log_label,
		log.game,
		act.turn,
		light.tag.label(),
		deep.map(|d| d.tag.label()).unwrap_or_else(|| "-".into()),
	);

	PosOutcome {
		row,
		changed_16,
		changed_64,
		deep_win_spot,
		shallow_miss,
		imm_cap: imm > 0,
		light_ring: light.dest_ring,
		deep_ring: deep.map(|d| d.dest_ring),
		fan_unique: fanout_tags.len(),
	}
}

pub fn run(cfg: &Config) {
	let logs = &cfg.probe_logs;
	let multipliers = if cfg.probe_budget_mult.is_empty() {
		vec![1_u64, 16, 64]
	} else {
		cfg.probe_budget_mult.clone()
	};
	let fanout = cfg.probe_fanout;
	let jobs = cfg.probe_jobs.max(1);
	let base_nodes = cfg.limit.nodes().unwrap_or(CALIBRATED_NODES_PER_MOVE);
	let search = cfg.search;

	let mut tasks: Vec<PosTask> = Vec::new();
	for path in logs {
		let bytes = std::fs::read(path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
		let games =
			decode_jsonl_gz(&bytes).unwrap_or_else(|e| panic!("decode {}: {e}", path.display()));
		let label = path.file_name().and_then(|s| s.to_str()).unwrap_or("?").to_string();
		for log in games {
			for ix in sample_action_indices(&log) {
				tasks.push(PosTask { log_label: label.clone(), log: log.clone(), ix });
			}
		}
	}

	let mut lines: Vec<String> = Vec::new();
	lines.push(format!("# deep_probe  search={}", search.label()));
	lines.push(format!(
		"# budget_base_nodes={base_nodes}  mult={multipliers:?}  fanout={fanout}@max  shuffle={} jobs={jobs}",
		cfg.probe_root_shuffle
	));
	lines.push(format!("# positions_scheduled={}", tasks.len()));
	lines.push(
		"# columns: log game action turn player scores claim counts imm_cap budgets moves".into(),
	);

	let chunk = (tasks.len() + jobs - 1) / jobs.max(1);
	let handles: Vec<_> = tasks
		.chunks(chunk.max(1))
		.map(|slice| {
			let batch: Vec<PosTask> = slice.to_vec();
			let multipliers = multipliers.clone();
			let root_shuffle = cfg.probe_root_shuffle;
			thread::spawn(move || {
				batch
					.iter()
					.map(|task| {
						probe_position(task, &multipliers, base_nodes, fanout, search, root_shuffle)
					})
					.collect::<Vec<_>>()
			})
		})
		.collect();
	let mut outcomes: Vec<PosOutcome> = Vec::with_capacity(tasks.len());
	for h in handles {
		outcomes.extend(h.join().expect("probe worker panicked"));
	}
	let mut total = 0u32;
	let mut changed_16 = 0u32;
	let mut changed_64 = 0u32;
	let mut deep_win_spot = 0u32;
	let mut shallow_only_caps = 0u32;
	let mut imm_cap_positions = 0u32;
	let mut ring_light: HashMap<&'static str, u32> = HashMap::new();
	let mut ring_deep: HashMap<&'static str, u32> = HashMap::new();
	let mut fan_uniq_sum = 0u32;

	for o in &outcomes {
		lines.push(o.row.clone());
		total += 1;
		if o.changed_16 {
			changed_16 += 1;
		}
		if o.changed_64 {
			changed_64 += 1;
		}
		if o.deep_win_spot {
			deep_win_spot += 1;
		}
		if o.shallow_miss {
			shallow_only_caps += 1;
		}
		if o.imm_cap {
			imm_cap_positions += 1;
		}
		*ring_light.entry(o.light_ring).or_default() += 1;
		if let Some(r) = o.deep_ring {
			*ring_deep.entry(r).or_default() += 1;
		}
		fan_uniq_sum += o.fan_unique as u32;
	}

	lines.push(String::new());
	lines.push("# summary".into());
	lines.push(format!("positions={total}"));
	lines.push(format!(
		"move_change_rate  x16={:.3} x64={:.3}",
		f64::from(changed_16) / f64::from(total.max(1)),
		f64::from(changed_64) / f64::from(total.max(1)),
	));
	lines.push(format!("deep_only_terminal_win_flags={deep_win_spot}"));
	lines.push(format!(
		"imm_capture_positions={imm_cap_positions} shallow_missed_tactical={shallow_only_caps}"
	));
	lines.push(format!(
		"fanout_unique_mean={:.2}",
		f64::from(fan_uniq_sum) / f64::from(total.max(1))
	));
	lines.push(format!("dest_ring_light={ring_light:?} dest_ring_max_mult={ring_deep:?}"));

	let text = lines.join("\n");
	println!("{text}");
	if let Some(out) = &cfg.probe_out {
		if let Some(parent) = out.parent() {
			let _ = std::fs::create_dir_all(parent);
		}
		std::fs::write(out, &text).unwrap_or_else(|e| panic!("write {}: {e}", out.display()));
	}
}
