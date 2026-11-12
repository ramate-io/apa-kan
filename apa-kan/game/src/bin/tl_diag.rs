//! Replay the turn-limit draws from a bag-11 refund-on H2H log and dump
//! late-game capture / engagement diagnostics.
//!
//! Default set is the nine `D t200` games in
//! `benchmark_results/low_value_pairs_deck11_on.txt` (seed 1, 12/pairing/seat).

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use apa_kan::ai::{self, Actor, Searcher};
use apa_kan::game::{Action, Game, GameStatus, Player, RuleSet, Tile, LOW_VALUE_BOARD_LT};
use apa_kan::scenario;
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

fn usage() -> &'static str {
	"Usage: apa-kan-tl-diag [OPTIONS]\n\
	 \n\
	 Replay the nine turn-limit draws from the bag-11 refund-on pair-wise\n\
	 low-value bench (or a custom --games list) and print end-position plus\n\
	 late-game capture/engagement stats.\n\
	 \n\
	 Options:\n\
	   --think-ms <n>   Bot think time in ms (default: 1000)\n\
	   --late <n>       Last N turns to mix/capture-sample (default: 30)\n\
	   --out <path>     Report path\n\
	   --games <list>   Comma-separated p1:p2:seed (default: the nine TL draws)\n\
	   --bag-size <n>   (default: 11)\n\
	   -h, --help\n"
}

struct Cfg {
	think: Duration,
	late: u32,
	out: PathBuf,
	games: Vec<Spec>,
	rules: RuleSet,
}

struct Spec {
	p1: Actor,
	p2: Actor,
	seed: u64,
	log_game: String,
}

fn default_games() -> Vec<Spec> {
	// Pairing index i: seed = 1 + i*10000 + (g-1). From the committed log.
	vec![
		spec("greedy", "greedy", 6, "greedy vs greedy g6"),
		spec("greedy", "greedy", 9, "greedy vs greedy g9"),
		spec("minimax", "minimax", 40004, "minimax vs minimax g4"),
		spec("minimax", "minimax", 40006, "minimax vs minimax g6"),
		spec("minimax", "mcts", 50010, "minimax vs mcts g10"),
		spec("mcts", "minimax", 70003, "mcts vs minimax g3"),
		spec("mcts", "minimax", 70009, "mcts vs minimax g9"),
		spec("mcts", "mcts", 80009, "mcts vs mcts g9"),
		spec("mcts", "mcts", 80011, "mcts vs mcts g11"),
	]
}

fn spec(p1: &str, p2: &str, seed: u64, log_game: &str) -> Spec {
	Spec {
		p1: Actor::parse(p1).expect("builtin actor"),
		p2: Actor::parse(p2).expect("builtin actor"),
		seed,
		log_game: log_game.into(),
	}
}

fn parse<I, S>(args: I) -> Result<Cfg, String>
where
	I: IntoIterator<Item = S>,
	S: AsRef<str>,
{
	let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("benchmark_results");
	let mut cfg = Cfg {
		think: Duration::from_millis(1000),
		late: 30,
		out: root.join("tl_draws_deck11_on.txt"),
		games: default_games(),
		rules: {
			let mut r = RuleSet::default();
			r.bag_size = 11;
			r.self_capture_budget = true;
			r
		},
	};
	let mut it = args.into_iter();
	while let Some(raw) = it.next() {
		let arg = raw.as_ref();
		match arg {
			"-h" | "--help" => {
				print!("{}", usage());
				std::process::exit(0);
			}
			"--think-ms" => {
				let v = need("--think-ms", it.next())?;
				cfg.think = Duration::from_millis(parse_u64("--think-ms", &v)?);
			}
			"--late" => {
				cfg.late = parse_u64("--late", &need("--late", it.next())?)? as u32;
			}
			"--out" => cfg.out = PathBuf::from(need("--out", it.next())?),
			"--bag-size" => {
				cfg.rules.bag_size =
					parse_u64("--bag-size", &need("--bag-size", it.next())?)? as u8;
			}
			"--games" => {
				cfg.games = parse_games(&need("--games", it.next())?)?;
			}
			other => return Err(format!("unknown argument '{other}'")),
		}
	}
	Ok(cfg)
}

fn parse_games(s: &str) -> Result<Vec<Spec>, String> {
	let mut out = Vec::new();
	for part in s.split(',') {
		let part = part.trim();
		if part.is_empty() {
			continue;
		}
		let bits: Vec<&str> = part.split(':').collect();
		if bits.len() != 3 {
			return Err(format!("expected p1:p2:seed, got '{part}'"));
		}
		let p1 = Actor::parse(bits[0])?;
		let p2 = Actor::parse(bits[1])?;
		let seed = parse_u64("seed", bits[2])?;
		out.push(Spec {
			p1,
			p2,
			seed,
			log_game: format!("{} vs {} seed={seed}", p1.name(), p2.name()),
		});
	}
	if out.is_empty() {
		return Err("--games must name at least one p1:p2:seed".into());
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

struct Report {
	label: String,
	status: String,
	turns: u32,
	matched_tl: bool,
	lv_ready: bool,
	p1_usable: usize,
	p2_usable: usize,
	late_n: u32,
	late_had: u32,
	late_either: u32,
	late_declined: u32,
	type_either: u32,
	body: String,
}

fn run(cfg: Cfg) {
	let started = Instant::now();
	println!(
		"replaying {} games  think_ms={} late={} bag_size={}",
		cfg.games.len(),
		cfg.think.as_millis(),
		cfg.late,
		cfg.rules.bag_size
	);
	let mut reports: Vec<(usize, Report)> = Vec::new();
	std::thread::scope(|scope| {
		let mut handles = Vec::new();
		for (i, spec) in cfg.games.iter().enumerate() {
			let think = cfg.think;
			let late = cfg.late;
			let rules = cfg.rules;
			let p1 = spec.p1;
			let p2 = spec.p2;
			let seed = spec.seed;
			let label = spec.log_game.clone();
			handles.push(scope.spawn(move || {
				println!("  start {label} seed={seed}");
				let report = replay(rules, p1, p2, seed, think, late, &label);
				println!("  done  {label} -> {} turns={}", report.status, report.turns);
				(i, report)
			}));
		}
		for h in handles {
			match h.join() {
				Ok(row) => reports.push(row),
				Err(_) => {
					eprintln!("replay thread panicked");
					std::process::exit(1);
				}
			}
		}
	});
	reports.sort_by_key(|r| r.0);
	let mut lines = Vec::new();
	lines.push("# apa-kan turn-limit draw diagnostic".into());
	lines.push(format!(
		"# think_ms={} late_turns={} bag_size={} self_capture_budget=on seed protocol = bench (rng = seed ^ 0x51ed)",
		cfg.think.as_millis(),
		cfg.late,
		cfg.rules.bag_size
	));
	lines.push("# Search bots are time-bounded, so a replay may diverge from the logged game; greedy is deterministic.".into());
	lines.push("# summary  pairing  result  turns  matched_tl  lv_ready  P1_usable/P2_usable  1v1_type_kills  late_cap_to_move  late_either  late_declined".into());
	for (_, r) in &reports {
		lines.push(format!(
			"#   {}  {}  t{}  matched_tl={}  lv_ready={}  usable={}/{}  type_kills={}  cap={}/{}  either={}/{}  declined={}/{}",
			r.label,
			r.status,
			r.turns,
			r.matched_tl,
			r.lv_ready,
			r.p1_usable,
			r.p2_usable,
			r.type_either,
			r.late_had,
			r.late_n,
			r.late_either,
			r.late_n,
			r.late_declined,
			r.late_n
		));
	}
	lines.push(String::new());
	for (_, r) in &reports {
		lines.push(r.body.clone());
		lines.push(String::new());
	}
	lines.push(format!("# elapsed_sec={:.1}", started.elapsed().as_secs_f64()));
	let body = lines.join("\n") + "\n";
	if let Some(parent) = cfg.out.parent() {
		let _ = fs::create_dir_all(parent);
	}
	if let Err(err) = fs::write(&cfg.out, &body) {
		eprintln!("failed to write {}: {err}", cfg.out.display());
		std::process::exit(1);
	}
	print!("{body}");
	println!("wrote {}", cfg.out.display());
}

fn replay(
	rules: RuleSet,
	p1: Actor,
	p2: Actor,
	seed: u64,
	think: Duration,
	late: u32,
	label: &str,
) -> Report {
	let mut game = Game::start(rules, seed, None).unwrap_or_else(|e| {
		eprintln!("{e}");
		std::process::exit(1);
	});
	let mut rng = StdRng::seed_from_u64(seed ^ 0x51ed);
	let mut s1 = Searcher::new();
	let mut s2 = Searcher::new();
	let mut history: Vec<TurnSnap> = Vec::new();
	let mut guard = 0u32;

	loop {
		if game.status().is_over() {
			break;
		}
		guard += 1;
		if guard > rules.turn_limit.saturating_mul(4) + 8 {
			break;
		}
		let snap_before = snapshot(&game);
		let actor = match game.current_player() {
			Player::One => p1,
			Player::Two => p2,
		};
		let searcher = match game.current_player() {
			Player::One => &mut s1,
			Player::Two => &mut s2,
		};
		let action = searcher.choose(&game, actor, &mut rng, think);
		let outcome = match game.apply(action) {
			Ok(o) => o,
			Err(_) => {
				if let Some(fallback) = game.legal_actions().into_iter().next() {
					match game.apply(fallback) {
						Ok(o) => o,
						Err(_) => break,
					}
				} else {
					let _ = game.apply(Action::Pass);
					break;
				}
			}
		};
		let opp_caps = outcome.removals.iter().filter(|r| r.tile.owner != outcome.player).count();
		let self_caps = outcome.removals.iter().filter(|r| r.tile.owner == outcome.player).count();
		history.push(TurnSnap {
			turn: game.turn_count(),
			player: outcome.player,
			kind: match outcome.action {
				Action::Add { .. } => "ADD",
				Action::Move { .. } => "MOVE",
				Action::Pass => "PASS",
			},
			opp_caps: opp_caps as u32,
			self_caps: self_caps as u32,
			had_cap: snap_before.p_to_move_caps > 0,
			declined: snap_before.p_to_move_caps > 0 && opp_caps == 0 && self_caps == 0,
			hash: snap_before.hash,
			p1_caps: snap_before.p1_caps,
			p2_caps: snap_before.p2_caps,
		});
		s1.advance(&game);
		s2.advance(&game);
	}

	let rendered = render(label, p1, p2, seed, &game, &history, late);
	Report {
		label: label.into(),
		status: format_status(game.status()),
		turns: game.turn_count(),
		matched_tl: matches!(
			game.status(),
			GameStatus::Draw { reason: apa_kan::game::EndReason::TurnLimit }
		),
		lv_ready: rendered.lv_ready,
		p1_usable: game.budget(Player::One),
		p2_usable: game.budget(Player::Two),
		late_n: rendered.late_n,
		late_had: rendered.late_had,
		late_either: rendered.late_either,
		late_declined: rendered.late_declined,
		type_either: rendered.type_either,
		body: rendered.body,
	}
}

struct Snap {
	hash: u64,
	p1_caps: u32,
	p2_caps: u32,
	p_to_move_caps: u32,
}

struct TurnSnap {
	turn: u32,
	player: Player,
	kind: &'static str,
	opp_caps: u32,
	self_caps: u32,
	had_cap: bool,
	declined: bool,
	hash: u64,
	p1_caps: u32,
	p2_caps: u32,
}

fn snapshot(game: &Game) -> Snap {
	let p1 = capturing_moves(game, Player::One);
	let p2 = capturing_moves(game, Player::Two);
	let to_move = match game.current_player() {
		Player::One => p1,
		Player::Two => p2,
	};
	Snap { hash: ai::visible_hash(game), p1_caps: p1, p2_caps: p2, p_to_move_caps: to_move }
}

fn capturing_moves(game: &Game, player: Player) -> u32 {
	let mut n = 0u32;
	for (node, tile) in game.tiles() {
		if tile.owner != player {
			continue;
		}
		for dest in game.destinations(node) {
			if game.move_removes_opponent(node, dest) {
				n += 1;
			}
		}
	}
	n
}

fn capturing_adds(game: &Game, player: Player) -> u32 {
	if game.budget(player) == 0 || game.bench(player).is_empty() {
		return 0;
	}
	let mut g = game.clone();
	g.set_current(player);
	let mut n = 0u32;
	for action in g.legal_adds() {
		let mut preview = g.clone();
		if let Ok(out) = preview.apply(action) {
			if out.removals.iter().any(|r| r.tile.owner != player) {
				n += 1;
			}
		}
	}
	n
}

fn node_role(game: &Game, n: apa_kan::board::NodeId) -> &'static str {
	let b = game.board();
	if n == b.center() {
		"center"
	} else if b.inner_ring().contains(&n) {
		"inner"
	} else if b.is_spawn(n) {
		"spawn"
	} else {
		"mid"
	}
}

fn fmt_piece(game: &Game, node: apa_kan::board::NodeId, tile: Tile) -> String {
	format!(
		"{} d{} n{} z{} w{} @{}({})",
		scenario::tile_tag(tile),
		tile.dots,
		tile.normal_ring_count(),
		tile.zag_ring_count(),
		tile.weight,
		node,
		node_role(game, node)
	)
}

fn pieces_of(game: &Game, player: Player) -> Vec<String> {
	let mut v: Vec<_> = game
		.tiles()
		.filter(|(_, t)| t.owner == player)
		.map(|(n, t)| fmt_piece(game, n, t))
		.collect();
	v.sort();
	v
}

fn contact_pairs(game: &Game) -> u32 {
	let mut n = 0u32;
	for (node, tile) in game.tiles() {
		if tile.owner != Player::One {
			continue;
		}
		for &nb in game.board().neighbors(node) {
			if game.tile_at(nb).map(|t| t.owner == Player::Two).unwrap_or(false) {
				n += 1;
			}
		}
	}
	n
}

fn can_approach(game: &Game, player: Player) -> u32 {
	let opp = player.other();
	let mut n = 0u32;
	for (node, tile) in game.tiles() {
		if tile.owner != player {
			continue;
		}
		for dest in game.destinations(node) {
			let next_to_opp = game
				.board()
				.neighbors(dest)
				.iter()
				.any(|&nb| game.tile_at(nb).map(|t| t.owner == opp).unwrap_or(false));
			if next_to_opp {
				n += 1;
			}
		}
	}
	n
}

/// If `a` and `b` were sole neighbors, who dies?
fn type_kill(game: &Game, a: Tile, b: Tile) -> (bool, bool) {
	let (m_ab, x_ab) = game.edge_score(a, b);
	let (m_ba, x_ba) = game.edge_score(b, a);
	let a_dies = x_ab >= i32::from(a.weight) + m_ab;
	let b_dies = x_ba >= i32::from(b.weight) + m_ba;
	(a_dies, b_dies)
}

struct TypeThreat {
	pairs: u32,
	p1_kills_p2: u32,
	p2_kills_p1: u32,
	either: u32,
	examples: Vec<String>,
}

fn type_threats(game: &Game) -> TypeThreat {
	let p1: Vec<Tile> =
		game.tiles().filter(|(_, t)| t.owner == Player::One).map(|(_, t)| t).collect();
	let p2: Vec<Tile> =
		game.tiles().filter(|(_, t)| t.owner == Player::Two).map(|(_, t)| t).collect();
	let mut t =
		TypeThreat { pairs: 0, p1_kills_p2: 0, p2_kills_p1: 0, either: 0, examples: Vec::new() };
	for a in &p1 {
		for b in &p2 {
			t.pairs += 1;
			let (a_dies, b_dies) = type_kill(game, *a, *b);
			if b_dies {
				t.p1_kills_p2 += 1;
			}
			if a_dies {
				t.p2_kills_p1 += 1;
			}
			if a_dies || b_dies {
				t.either += 1;
				if t.examples.len() < 4 {
					t.examples.push(format!(
						"{} vs {} -> P1dies={} P2dies={}",
						scenario::tile_tag(*a),
						scenario::tile_tag(*b),
						a_dies,
						b_dies
					));
				}
			}
		}
	}
	t
}

fn format_status(st: GameStatus) -> String {
	match st {
		GameStatus::InProgress => "InProgress".into(),
		GameStatus::Won { winner, reason } => format!("Won {:?} {:?}", winner, reason),
		GameStatus::Draw { reason } => format!("Draw {:?}", reason),
	}
}

fn budget_line(game: &Game, player: Player) -> String {
	format!(
		"bench={} bag={} usable={} surplus={} tokens={} board={}",
		game.bench(player).len(),
		game.bag_len(player),
		game.budget(player),
		game.budget_surplus(player),
		game.budget_token_count(player),
		game.board_count(player),
	)
}

struct Rendered {
	body: String,
	lv_ready: bool,
	late_n: u32,
	late_had: u32,
	late_either: u32,
	late_declined: u32,
	type_either: u32,
}

fn render(
	label: &str,
	p1: Actor,
	p2: Actor,
	seed: u64,
	game: &Game,
	history: &[TurnSnap],
	late: u32,
) -> Rendered {
	let mut out = Vec::new();
	out.push(format!("## {label}  seed={seed}  {} vs {}", p1.name(), p2.name()));
	out.push(format!(
		"result={}  turns={}  (log expected Draw TurnLimit at 200)",
		format_status(game.status()),
		game.turn_count()
	));
	let lv_ready = game.budget(Player::One) == 0
		&& game.budget(Player::Two) == 0
		&& game.board_count(Player::One) < usize::from(LOW_VALUE_BOARD_LT)
		&& game.board_count(Player::Two) < usize::from(LOW_VALUE_BOARD_LT);
	out.push(format!("low_value_trigger_ready={}  (both usable=0 and both board<5)", lv_ready));
	out.push(format!("P1 {}", budget_line(game, Player::One)));
	out.push(format!("P2 {}", budget_line(game, Player::Two)));
	let p1_bench = scenario::format_kind_list(game.bench(Player::One));
	let p2_bench = scenario::format_kind_list(game.bench(Player::Two));
	out.push(format!("P1 bench [{}]", if p1_bench.is_empty() { "-" } else { &p1_bench }));
	out.push(format!("P2 bench [{}]", if p2_bench.is_empty() { "-" } else { &p2_bench }));
	let p1p = pieces_of(game, Player::One);
	let p2p = pieces_of(game, Player::Two);
	out.push(format!("P1 board ({})", p1p.len()));
	for s in &p1p {
		out.push(format!("  {s}"));
	}
	out.push(format!("P2 board ({})", p2p.len()));
	for s in &p2p {
		out.push(format!("  {s}"));
	}

	let threat = type_threats(game);
	out.push(format!(
		"type_threats  pairs={}  P1_kills_P2_type={}  P2_kills_P1_type={}  either={}",
		threat.pairs, threat.p1_kills_p2, threat.p2_kills_p1, threat.either
	));
	for e in &threat.examples {
		out.push(format!("  e.g. {e}"));
	}
	out.push(format!(
		"geometry  contact_edges={}  P1_approach_dests={}  P2_approach_dests={}  P1_cap_moves={}  P2_cap_moves={}  P1_cap_adds={}  P2_cap_adds={}",
		contact_pairs(game),
		can_approach(game, Player::One),
		can_approach(game, Player::Two),
		capturing_moves(game, Player::One),
		capturing_moves(game, Player::Two),
		capturing_adds(game, Player::One),
		capturing_adds(game, Player::Two),
	));

	let start_turn = game.turn_count().saturating_sub(late);
	let late_h: Vec<&TurnSnap> = history.iter().filter(|h| h.turn > start_turn).collect();
	let n = late_h.len() as u32;
	let moves = late_h.iter().filter(|h| h.kind == "MOVE").count() as u32;
	let adds = late_h.iter().filter(|h| h.kind == "ADD").count() as u32;
	let passes = late_h.iter().filter(|h| h.kind == "PASS").count() as u32;
	let opp_caps: u32 = late_h.iter().map(|h| h.opp_caps).sum();
	let self_caps: u32 = late_h.iter().map(|h| h.self_caps).sum();
	let had = late_h.iter().filter(|h| h.had_cap).count() as u32;
	let declined = late_h.iter().filter(|h| h.declined).count() as u32;
	let either_cap = late_h.iter().filter(|h| h.p1_caps + h.p2_caps > 0).count() as u32;
	let mut hash_n: HashMap<u64, u32> = HashMap::new();
	let mut max_rep = 0u32;
	for h in &late_h {
		let e = hash_n.entry(h.hash).or_insert(0);
		*e += 1;
		max_rep = max_rep.max(*e);
	}
	out.push(format!(
		"late last {late} turns: n={n} MOVE={moves} ADD={adds} PASS={passes} opp_caps={opp_caps} self_caps={self_caps}"
	));
	out.push(format!(
		"late capture_avail  to_move={had}/{n}  either_side={either_cap}/{n}  declined_noncapture={declined}/{n}  unique_pos={}  max_repeat={}",
		hash_n.len(),
		max_rep
	));
	out.push("late mix (turn player KIND cap? declined? opp/self caps)".into());
	for h in late_h.iter().rev().take(12).collect::<Vec<_>>().into_iter().rev() {
		out.push(format!(
			"  t{:<3} {} {:<4} cap_avail={} declined={} opp={} self={}",
			h.turn,
			h.player.label(),
			h.kind,
			h.had_cap,
			h.declined,
			h.opp_caps,
			h.self_caps
		));
	}
	Rendered {
		body: out.join("\n"),
		lv_ready,
		late_n: n,
		late_had: had,
		late_either: either_cap,
		late_declined: declined,
		type_either: threat.either,
	}
}
