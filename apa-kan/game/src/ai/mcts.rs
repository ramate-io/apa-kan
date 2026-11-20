//! PUCT/UCT with RAVE, optional move-ordering priors, tree reuse, Zobrist
//! stats sharing, factored ADDs, leaf search, and a win/loss solver.
//!
//! Optional levers ([`SearchConfig`]): progressive widening, heuristic vs
//! uniform cut-off playouts, move priors + prior-ordered widening, tree
//! reuse, transposition-aware stats, factored ADDs, 1–2 ply leaf search,
//! MCTS-solver, and root-parallel independent trees.
//!
//! Root-parallel seeding: worker `t` uses `StdRng::seed_from_u64(base ^
//! (t+1).wrapping_mul(GOLDEN))` where `base` is drawn once from the caller's
//! RNG. `threads=1` does not draw that extra seed and is bit-identical to
//! sequential search. Combined root visits are summed after every worker
//! joins, so the chosen move does not depend on completion order.

use std::collections::HashMap;
use std::time::Instant;

use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};

use crate::game::{Action, Game, Player};

use super::eval::{eval_unit, evaluate, terminal_score, WIN};
use super::search_move::{apply_move, cheap_order, legal_search_moves, SearchMove};
use super::zobrist;
use super::{count_repeats, SearchConfig, SearchLimit, SearchStats};

const GOLDEN: u64 = 0x9E37_79B9_7F4A_7C15;

/// One outgoing decision. `Tile` is a virtual choose-tile ply (no apply).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
enum Edge {
	Real(SearchMove),
	Tile(u32),
}

impl Edge {
	fn as_real(self) -> Option<SearchMove> {
		match self {
			Edge::Real(mv) => Some(mv),
			Edge::Tile(_) => None,
		}
	}

	fn is_virtual(self) -> bool {
		matches!(self, Edge::Tile(_))
	}
}

struct Node {
	parent: Option<usize>,
	action: Option<Edge>,
	hash: u64,
	to_move: Player,
	children: Vec<usize>,
	untried: Vec<Edge>,
	priors: HashMap<Edge, f64>,
	visits: u32,
	value: f64,
	prior: f64,
	rave_visits: HashMap<Edge, u32>,
	rave_value: HashMap<Edge, f64>,
	/// Proven outcome for `to_move`: +1 win, −1 loss, 0 draw.
	proven: Option<i8>,
}

#[derive(Clone, Copy, Default)]
struct Transpo {
	visits: u32,
	value: f64,
	proven: Option<i8>,
}

pub struct MctsTree {
	nodes: Vec<Node>,
	root: usize,
	transpo: HashMap<u64, Transpo>,
}

impl MctsTree {
	fn new_with(game: &Game, last_move: Option<SearchMove>, config: SearchConfig) -> Self {
		let root = make_root_with(game, last_move, None, config);
		Self { nodes: vec![root], root: 0, transpo: HashMap::new() }
	}

	fn sync(&mut self, game: &Game, last_move: Option<SearchMove>, config: SearchConfig) {
		if !config.mcts_tree_reuse {
			*self = Self::new_with(game, last_move, config);
			return;
		}
		let h = zobrist::hash(game);
		if self.nodes.get(self.root).map(|n| n.hash) == Some(h) {
			return;
		}
		if let Some(idx) = find_hash(&self.nodes, self.root, h, 6) {
			self.root = idx;
			self.nodes[idx].parent = None;
		} else {
			*self = Self::new_with(game, last_move, config);
		}
	}
}

fn pw_limit(visits: u32, config: SearchConfig) -> usize {
	let a = f64::from(config.pw_a);
	let alpha = f64::from(config.pw_alpha);
	1 + (a * (visits as f64).powf(alpha)) as usize
}

fn should_expand(node: &Node, config: SearchConfig) -> bool {
	if node.untried.is_empty() {
		return false;
	}
	if node.proven.is_some() {
		return false;
	}
	if !config.mcts_progressive_widening {
		return true;
	}
	node.children.len() < pw_limit(node.visits, config)
}

fn prepare_untried(
	game: &Game,
	last_move: Option<SearchMove>,
	pending: Option<u32>,
	config: SearchConfig,
) -> Vec<Edge> {
	let legal = legal_search_moves(game);
	let mut untried = if let Some(kind) = pending {
		legal
			.into_iter()
			.filter_map(|mv| match mv {
				SearchMove::Add { kind: k, node } if k == kind => {
					Some(Edge::Real(SearchMove::Add { kind: k, node }))
				}
				_ => None,
			})
			.collect()
	} else if config.mcts_factored_moves {
		let mut kinds = Vec::new();
		let mut seen = std::collections::HashSet::new();
		let mut rest = Vec::new();
		for mv in legal {
			match mv {
				SearchMove::Add { kind, .. } => {
					if seen.insert(kind) {
						kinds.push(Edge::Tile(kind));
					}
				}
				other => rest.push(Edge::Real(other)),
			}
		}
		kinds.extend(rest);
		kinds
	} else {
		legal.into_iter().map(Edge::Real).collect()
	};
	if config.mcts_progressive_widening
		|| config.move_ordering
		|| config.prior_widening
		|| config.mcts_move_priors
	{
		untried.sort_by_cached_key(|edge| order_score(game, *edge, last_move));
	}
	untried
}

fn order_score(game: &Game, edge: Edge, last_move: Option<SearchMove>) -> i32 {
	match edge {
		Edge::Real(mv) => {
			let mut s = cheap_order(game, mv);
			if is_reverse(mv, last_move) {
				s -= 500;
			}
			s
		}
		Edge::Tile(kind) => legal_search_moves(game)
			.into_iter()
			.filter_map(|mv| match mv {
				SearchMove::Add { kind: k, .. } if k == kind => Some(cheap_order(game, mv)),
				_ => None,
			})
			.max()
			.unwrap_or(12),
	}
}

fn softmax_priors(
	game: &Game,
	edges: &[Edge],
	last_move: Option<SearchMove>,
	config: SearchConfig,
) -> HashMap<Edge, f64> {
	let mut out = HashMap::new();
	if edges.is_empty() {
		return out;
	}
	if !config.mcts_move_priors {
		for &e in edges {
			out.insert(e, 1.0);
		}
		return out;
	}
	let temp = f64::from(config.prior_temperature).max(0.05);
	let scores: Vec<f64> = edges
		.iter()
		.map(|e| f64::from(order_score(game, *e, last_move)) / temp)
		.collect();
	let max = scores.iter().copied().fold(f64::NEG_INFINITY, f64::max);
	let exps: Vec<f64> = scores.iter().map(|s| (s - max).exp()).collect();
	let sum: f64 = exps.iter().sum::<f64>().max(1e-12);
	for (e, w) in edges.iter().zip(exps) {
		out.insert(*e, w / sum);
	}
	out
}

fn make_root_with(
	game: &Game,
	last_move: Option<SearchMove>,
	pending: Option<u32>,
	config: SearchConfig,
) -> Node {
	let untried = prepare_untried(game, last_move, pending, config);
	let priors = softmax_priors(game, &untried, last_move, config);
	Node {
		parent: None,
		action: pending.map(Edge::Tile),
		hash: if let Some(kind) = pending {
			zobrist::hash(game) ^ mix_kind(kind)
		} else {
			zobrist::hash(game)
		},
		to_move: game.current_player(),
		children: Vec::new(),
		untried,
		priors,
		visits: 0,
		value: 0.0,
		prior: 1.0,
		rave_visits: HashMap::new(),
		rave_value: HashMap::new(),
		proven: None,
	}
}

fn mix_kind(kind: u32) -> u64 {
	let mut x = u64::from(kind).wrapping_add(0xA5A5_5A5A_C3C3_3C3C);
	x = (x ^ (x >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
	x = (x ^ (x >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
	x ^ (x >> 31)
}

fn find_hash(nodes: &[Node], root: usize, hash: u64, depth: u8) -> Option<usize> {
	if nodes.get(root).map(|n| n.hash) == Some(hash) {
		return Some(root);
	}
	if depth == 0 {
		return None;
	}
	for &c in &nodes.get(root)?.children {
		if let Some(found) = find_hash(nodes, c, hash, depth - 1) {
			return Some(found);
		}
	}
	None
}

pub fn search(
	tree: &mut Option<MctsTree>,
	game: &Game,
	played: &[u64],
	last_move: Option<SearchMove>,
	rng: &mut impl Rng,
	limit: SearchLimit,
	config: SearchConfig,
) -> (Action, SearchStats) {
	let legal = legal_search_moves(game);
	let fallback = legal.first().and_then(|m| m.to_action(game)).unwrap_or(Action::Pass);
	if legal.is_empty() {
		return (Action::Pass, SearchStats::default());
	}

	let threads = config.mcts_root_threads.max(1);
	if threads > 1 {
		return search_root_parallel(game, played, last_move, rng, limit, config, &legal, fallback);
	}

	let t = tree.get_or_insert_with(|| MctsTree::new_with(game, last_move, config));
	t.sync(game, last_move, config);
	let (iters, snaps) = run_iterations(t, game, played, last_move, rng, limit, config, &legal);
	let best = pick_best(t, last_move, &legal);
	let (mcts_max_depth, mcts_avg_depth) = tree_depth_stats(t);
	let spread = root_spread(t, last_move, legal.len(), &snaps, best, config);
	(
		best.to_action(game).unwrap_or(fallback),
		SearchStats {
			iterations: iters,
			nodes: u64::from(iters),
			mcts_max_depth,
			mcts_avg_depth,
			root_entropy: spread.entropy,
			root_eff_branch: spread.eff_branch,
			root_top1_share: spread.top1_share,
			root_near_best_q: spread.near_best_q,
			root_q_spread_top10: spread.q_spread_top10,
			root_best_stability: spread.stability,
			root_legal: legal.len() as u16,
			..SearchStats::default()
		},
	)
}

fn run_iterations(
	tree: &mut MctsTree,
	game: &Game,
	played: &[u64],
	last_move: Option<SearchMove>,
	rng: &mut impl Rng,
	limit: SearchLimit,
	config: SearchConfig,
	legal: &[SearchMove],
) -> (u32, Vec<SearchMove>) {
	let mut iterations = 0u32;
	let mut snaps: Vec<SearchMove> = Vec::new();
	const MAX_ITERS: u32 = 8_000;
	let sample_every = match limit {
		SearchLimit::Work { iters, .. } => (iters / 8).max(1),
		SearchLimit::Time(_) => 8,
	};
	match limit {
		SearchLimit::Time(think) => {
			let deadline = Instant::now() + think;
			while Instant::now() < deadline && iterations < MAX_ITERS {
				iterations += 1;
				iterate(tree, game, played, last_move, rng, config);
				if iterations % sample_every == 0 {
					snaps.push(pick_best(tree, last_move, legal));
				}
				if config.mcts_solver && tree.nodes[tree.root].proven.is_some() {
					break;
				}
				if iterations % 8 == 0 && Instant::now() >= deadline {
					break;
				}
			}
		}
		SearchLimit::Work { iters, .. } => {
			while iterations < iters {
				iterations += 1;
				iterate(tree, game, played, last_move, rng, config);
				if iterations % sample_every == 0 {
					snaps.push(pick_best(tree, last_move, legal));
				}
				if config.mcts_solver && tree.nodes[tree.root].proven.is_some() {
					break;
				}
			}
		}
	}
	(iterations, snaps)
}

fn pick_best(tree: &MctsTree, last_move: Option<SearchMove>, legal: &[SearchMove]) -> SearchMove {
	let root = tree.root;
	let chooser = tree.nodes[root].to_move;
	let mut best_visits = 0u32;
	let mut best_proven = i8::MIN;
	let mut best = legal[0];
	for &c in &tree.nodes[root].children {
		let n = &tree.nodes[c];
		let proven = proven_for_chooser(chooser, n).unwrap_or(0);
		let mut vis = n.visits;
		if let Some(mv) = n.action.and_then(Edge::as_real) {
			if is_reverse(mv, last_move) {
				vis /= 4;
			}
		}
		let better_proven = proven > best_proven;
		let better_visits = proven == best_proven && vis > best_visits;
		if better_proven || better_visits {
			best_proven = proven;
			best_visits = vis;
			if let Some(mv) = resolve_real(tree, c) {
				best = mv;
			}
		}
	}
	best
}

fn resolve_real(tree: &MctsTree, idx: usize) -> Option<SearchMove> {
	match tree.nodes.get(idx)?.action {
		Some(Edge::Real(mv)) => Some(mv),
		Some(Edge::Tile(_)) => {
			let mut best_v = 0u32;
			let mut best = None;
			for &g in &tree.nodes[idx].children {
				let v = tree.nodes[g].visits;
				if v >= best_v {
					best_v = v;
					best = tree.nodes[g].action.and_then(Edge::as_real);
				}
			}
			best.or_else(|| tree.nodes[idx].untried.iter().copied().find_map(Edge::as_real))
		}
		None => None,
	}
}

fn search_root_parallel(
	game: &Game,
	played: &[u64],
	last_move: Option<SearchMove>,
	rng: &mut impl Rng,
	limit: SearchLimit,
	config: SearchConfig,
	legal: &[SearchMove],
	fallback: Action,
) -> (Action, SearchStats) {
	let threads = config.mcts_root_threads.max(1) as usize;
	let seeds: Vec<u64> = (0..threads)
		.map(|t| rng.gen::<u64>() ^ (t as u64 + 1).wrapping_mul(GOLDEN))
		.collect();
	let iter_budget = match limit {
		SearchLimit::Work { iters, .. } => Some(iters),
		SearchLimit::Time(_) => None,
	};
	let played = played.to_vec();
	let game = game.clone();
	let results = std::thread::scope(|scope| {
		let mut handles = Vec::with_capacity(threads);
		for (t, seed) in seeds.into_iter().enumerate() {
			let game = &game;
			let played = &played;
			handles.push(scope.spawn(move || {
				let mut rng = StdRng::seed_from_u64(seed);
				let mut tree = MctsTree::new_with(game, last_move, config);
				let worker_limit = match (limit, iter_budget) {
					(SearchLimit::Time(d), _) => SearchLimit::Time(d),
					(SearchLimit::Work { nodes, .. }, Some(iters)) => {
						let n = threads as u32;
						let base = iters / n;
						let rem = iters % n;
						let share = base + u32::from((t as u32) < rem);
						if share == 0 {
							return (Vec::new(), 0, 0, 0.0, Spread::default());
						}
						SearchLimit::Work { nodes, iters: share }
					}
					_ => limit,
				};
				let legal = legal_search_moves(game);
				let (iterations, snaps) = run_iterations(
					&mut tree,
					game,
					played,
					last_move,
					&mut rng,
					worker_limit,
					config,
					&legal,
				);
				let (max_d, avg_d) = tree_depth_stats(&tree);
				let best = if legal.is_empty() {
					SearchMove::Pass
				} else {
					pick_best(&tree, last_move, &legal)
				};
				let spread = root_spread(&tree, last_move, legal.len(), &snaps, best, config);
				let mut visits = Vec::new();
				collect_real_visits(&tree, tree.root, &mut visits);
				(visits, iterations, max_d, avg_d, spread)
			}));
		}
		handles
			.into_iter()
			.map(|h| h.join().unwrap_or_else(|_| (Vec::new(), 0, 0, 0.0, Spread::default())))
			.collect::<Vec<_>>()
	});

	let mut combined: HashMap<SearchMove, u32> = HashMap::new();
	let mut iterations = 0u32;
	let mut max_d = 0u8;
	let mut avg_num = 0.0f64;
	let mut avg_den = 0.0f64;
	let mut spread_acc = Spread::default();
	let mut spread_n = 0u32;
	for (visits, iters, d, avg, sp) in results {
		iterations += iters;
		max_d = max_d.max(d);
		if avg > 0.0 {
			avg_num += f64::from(avg) * f64::from(iters);
			avg_den += f64::from(iters);
		}
		spread_acc.entropy += sp.entropy;
		spread_acc.eff_branch += sp.eff_branch;
		spread_acc.top1_share += sp.top1_share;
		spread_acc.near_best_q = spread_acc.near_best_q.max(sp.near_best_q);
		spread_acc.q_spread_top10 += sp.q_spread_top10;
		spread_acc.stability += sp.stability;
		spread_n += 1;
		for (mv, v) in visits {
			*combined.entry(mv).or_insert(0) += v;
		}
	}
	let mut best = legal[0];
	let mut best_v = 0u32;
	let mut best_pack = (u16::MAX, 0u8, 0u8);
	for (mv, v) in combined {
		let mut vis = v;
		if is_reverse(mv, last_move) {
			vis /= 4;
		}
		let pack = mv.pack();
		if vis > best_v || (vis == best_v && pack < best_pack) {
			best_v = vis;
			best = mv;
			best_pack = pack;
		}
	}
	let avg = if avg_den <= 0.0 { 0.0 } else { avg_num / avg_den };
	let dn = spread_n.max(1) as f32;
	(
		best.to_action(&game).unwrap_or(fallback),
		SearchStats {
			iterations,
			nodes: u64::from(iterations),
			mcts_max_depth: max_d,
			mcts_avg_depth: avg as f32,
			root_entropy: spread_acc.entropy / dn,
			root_eff_branch: spread_acc.eff_branch / dn,
			root_top1_share: spread_acc.top1_share / dn,
			root_near_best_q: spread_acc.near_best_q,
			root_q_spread_top10: spread_acc.q_spread_top10 / dn,
			root_best_stability: spread_acc.stability / dn,
			root_legal: legal.len() as u16,
			..SearchStats::default()
		},
	)
}

fn collect_real_visits(tree: &MctsTree, idx: usize, out: &mut Vec<(SearchMove, u32)>) {
	for &c in &tree.nodes[idx].children {
		match tree.nodes[c].action {
			Some(Edge::Real(mv)) => out.push((mv, tree.nodes[c].visits)),
			Some(Edge::Tile(_)) => collect_real_visits(tree, c, out),
			None => {}
		}
	}
}

/// Max tree depth from the current root, and visit-weighted mean depth of
/// non-root nodes. Root is depth 0. Virtual choose-tile plies are skipped so
/// factored trees stay comparable with flat ones. Unvisited nodes are ignored.
fn tree_depth_stats(tree: &MctsTree) -> (u8, f32) {
	let mut max_d = 0u8;
	let mut wsum = 0.0f64;
	let mut vsum = 0.0f64;
	let mut stack = vec![(tree.root, 0u8)];
	let mut seen = vec![false; tree.nodes.len()];
	while let Some((idx, d)) = stack.pop() {
		if idx >= tree.nodes.len() || seen[idx] {
			continue;
		}
		seen[idx] = true;
		let n = &tree.nodes[idx];
		if n.visits > 0 {
			max_d = max_d.max(d);
			if d > 0 {
				wsum += f64::from(n.visits) * f64::from(d);
				vsum += f64::from(n.visits);
			}
		}
		for &c in &n.children {
			let step = match tree.nodes.get(c).and_then(|ch| ch.action) {
				Some(Edge::Real(_)) => 1u8,
				_ => 0,
			};
			stack.push((c, d.saturating_add(step)));
		}
	}
	let avg = if vsum <= 0.0 { 0.0 } else { wsum / vsum };
	(max_d, avg as f32)
}

#[derive(Clone, Copy, Default)]
struct Spread {
	entropy: f32,
	eff_branch: f32,
	top1_share: f32,
	near_best_q: u16,
	q_spread_top10: f32,
	stability: f32,
}

fn root_spread(
	tree: &MctsTree,
	_last_move: Option<SearchMove>,
	_legal_n: usize,
	snaps: &[SearchMove],
	final_best: SearchMove,
	config: SearchConfig,
) -> Spread {
	let mut rows: Vec<(u32, f64)> = Vec::new();
	fn walk(tree: &MctsTree, idx: usize, config: SearchConfig, rows: &mut Vec<(u32, f64)>) {
		for &c in &tree.nodes[idx].children {
			match tree.nodes[c].action {
				Some(Edge::Real(_)) => {
					let (vis, q) = child_vis_q(tree, c, config);
					if vis > 0.0 {
						rows.push((vis as u32, q));
					}
				}
				Some(Edge::Tile(_)) => walk(tree, c, config, rows),
				None => {}
			}
		}
	}
	walk(tree, tree.root, config, &mut rows);
	if rows.is_empty() {
		return Spread { stability: if snaps.is_empty() { 0.0 } else { 1.0 }, ..Spread::default() };
	}
	let total: f64 = rows.iter().map(|(v, _)| f64::from(*v)).sum::<f64>().max(1.0);
	let mut entropy = 0.0f64;
	let mut top1 = 0u32;
	let mut best_q = f64::NEG_INFINITY;
	let mut best_q_n = 1u32;
	for &(v, q) in &rows {
		top1 = top1.max(v);
		if q > best_q {
			best_q = q;
			best_q_n = v;
		}
		let p = f64::from(v) / total;
		if p > 0.0 {
			entropy -= p * p.ln();
		}
	}
	let noise = 1.0 / (f64::from(best_q_n).sqrt().max(1.0));
	let near = rows.iter().filter(|(_, q)| *q >= best_q - noise).count() as u16;
	let mut by_vis = rows;
	by_vis.sort_by(|a, b| b.0.cmp(&a.0));
	let top = if by_vis.len() > 10 { &by_vis[..10] } else { &by_vis[..] };
	let q_max = top.iter().map(|(_, q)| *q).fold(f64::NEG_INFINITY, f64::max);
	let q_min = top.iter().map(|(_, q)| *q).fold(f64::INFINITY, f64::min);
	let q_spread =
		if q_max.is_finite() && q_min.is_finite() { (q_max - q_min) as f32 } else { 0.0 };
	let matched = snaps.iter().filter(|m| **m == final_best).count();
	let stability = if snaps.is_empty() { 1.0 } else { matched as f32 / snaps.len() as f32 };
	Spread {
		entropy: entropy as f32,
		eff_branch: entropy.exp() as f32,
		top1_share: (f64::from(top1) / total) as f32,
		near_best_q: near,
		q_spread_top10: q_spread,
		stability,
	}
}

fn iterate(
	tree: &mut MctsTree,
	game: &Game,
	played: &[u64],
	last_move: Option<SearchMove>,
	rng: &mut impl Rng,
	config: SearchConfig,
) {
	let mut state = game.clone();
	state.shuffle_bags(rng);

	let mut idx = tree.root;
	let mut path: Vec<usize> = vec![idx];
	let mut actions: Vec<(Player, SearchMove)> = Vec::new();

	loop {
		if config.mcts_solver && tree.nodes[idx].proven.is_some() {
			break;
		}
		if state.status().is_over() {
			break;
		}
		if count_repeats(played, &[], zobrist::occupancy_hash(&state)) >= 3 {
			break;
		}
		if !tree.nodes[idx].children.is_empty() && !should_expand(&tree.nodes[idx], config) {
			let next = puct_child(tree, idx, config, &path);
			if let Some(edge) = tree.nodes[next].action {
				if let Edge::Real(mv) = edge {
					let player = state.current_player();
					let _ = apply_move(&mut state, mv);
					actions.push((player, mv));
				}
			}
			idx = next;
			path.push(idx);
			continue;
		}
		break;
	}

	if !state.status().is_over()
		&& tree.nodes[idx].proven.is_none()
		&& should_expand(&tree.nodes[idx], config)
	{
		let edge = pick_untried(&mut tree.nodes[idx].untried, &state, last_move, rng, config);
		let player = state.current_player();
		if let Edge::Real(mv) = edge {
			let _ = apply_move(&mut state, mv);
			actions.push((player, mv));
		}
		let child_pending = match edge {
			Edge::Tile(kind) => Some(kind),
			Edge::Real(_) => None,
		};
		let hash = match child_pending {
			Some(kind) => tree.nodes[idx].hash ^ mix_kind(kind),
			None => zobrist::hash(&state),
		};
		let prior = tree.nodes[idx].priors.get(&edge).copied().unwrap_or_else(|| {
			if config.mcts_move_priors {
				0.05
			} else {
				prior_of(&state, player, played)
			}
		});
		let proven = if config.mcts_solver && child_pending.is_none() {
			terminal_proven(&state)
		} else {
			None
		};
		let untried = if state.status().is_over() || proven.is_some() {
			Vec::new()
		} else {
			prepare_untried(&state, last_move, child_pending, config)
		};
		let priors = softmax_priors(&state, &untried, last_move, config);
		let child = Node {
			parent: Some(idx),
			action: Some(edge),
			hash,
			to_move: state.current_player(),
			children: Vec::new(),
			untried,
			priors,
			visits: 0,
			value: 0.0,
			prior,
			rave_visits: HashMap::new(),
			rave_value: HashMap::new(),
			proven,
		};
		let child_idx = tree.nodes.len();
		tree.nodes[idx].children.push(child_idx);
		tree.nodes.push(child);
		idx = child_idx;
		path.push(idx);
		if matches!(edge, Edge::Tile(_))
			&& !state.status().is_over()
			&& should_expand(&tree.nodes[idx], config)
		{
			let dest = pick_untried(&mut tree.nodes[idx].untried, &state, last_move, rng, config);
			if let Edge::Real(mv) = dest {
				let dest_player = state.current_player();
				let _ = apply_move(&mut state, mv);
				actions.push((dest_player, mv));
			}
			let dest_prior = tree.nodes[idx].priors.get(&dest).copied().unwrap_or(0.05);
			let dest_proven = if config.mcts_solver { terminal_proven(&state) } else { None };
			let dest_untried = if state.status().is_over() || dest_proven.is_some() {
				Vec::new()
			} else {
				prepare_untried(&state, last_move, None, config)
			};
			let dest_priors = softmax_priors(&state, &dest_untried, last_move, config);
			let dest_node = Node {
				parent: Some(idx),
				action: Some(dest),
				hash: zobrist::hash(&state),
				to_move: state.current_player(),
				children: Vec::new(),
				untried: dest_untried,
				priors: dest_priors,
				visits: 0,
				value: 0.0,
				prior: dest_prior,
				rave_visits: HashMap::new(),
				rave_value: HashMap::new(),
				proven: dest_proven,
			};
			let dest_idx = tree.nodes.len();
			tree.nodes[idx].children.push(dest_idx);
			tree.nodes.push(dest_node);
			idx = dest_idx;
			path.push(idx);
		}
	}

	let value = if let Some(p) = tree.nodes[idx].proven {
		proven_root_value(tree.nodes[idx].to_move, p, game.current_player())
	} else if let Some(term) = terminal_score(&state, game.current_player()) {
		if config.mcts_solver {
			if let Some(pr) = terminal_proven(&state) {
				tree.nodes[idx].proven = Some(pr);
			}
		}
		f64::from(term) / f64::from(WIN)
	} else {
		let mut playout_actions = Vec::new();
		let v = if config.mcts_heuristic_playouts {
			heavy_playout(
				&mut state,
				rng,
				game.current_player(),
				played,
				&mut playout_actions,
				config,
			)
		} else {
			random_playout(
				&mut state,
				rng,
				game.current_player(),
				played,
				&mut playout_actions,
				config,
			)
		};
		update_rave(tree, &path, &actions, &playout_actions, game.current_player(), v);
		v
	};

	backprop(tree, &path, game.current_player(), value, config);
	update_proven(tree, &path, config);
}

fn terminal_proven(state: &Game) -> Option<i8> {
	let me = state.current_player();
	let term = terminal_score(state, me)?;
	if term > WIN / 2 {
		Some(1)
	} else if term < -WIN / 2 {
		Some(-1)
	} else {
		Some(0)
	}
}

fn proven_root_value(to_move: Player, proven: i8, root: Player) -> f64 {
	let v = f64::from(proven);
	if to_move == root {
		v
	} else {
		-v
	}
}

fn proven_for_chooser(chooser: Player, child: &Node) -> Option<i8> {
	let p = child.proven?;
	if child.to_move == chooser {
		Some(p)
	} else {
		Some(-p)
	}
}

fn is_reverse(mv: SearchMove, last: Option<SearchMove>) -> bool {
	match (mv, last) {
		(
			SearchMove::Move { from, to },
			Some(SearchMove::Move { from: prev_from, to: prev_to }),
		) => from == prev_to && to == prev_from,
		_ => false,
	}
}

fn pick_untried(
	untried: &mut Vec<Edge>,
	state: &Game,
	last_move: Option<SearchMove>,
	rng: &mut impl Rng,
	config: SearchConfig,
) -> Edge {
	if untried.is_empty() {
		return Edge::Real(SearchMove::Pass);
	}
	if config.mcts_progressive_widening || config.prior_widening {
		return untried.pop().unwrap_or(Edge::Real(SearchMove::Pass));
	}
	if rng.gen_bool(f64::from(config.mcts_untried_random)) {
		let i = rng.gen_range(0..untried.len());
		return untried.swap_remove(i);
	}
	let mut best_i = 0usize;
	let mut best_s = i32::MIN;
	for (i, edge) in untried.iter().enumerate() {
		let s = order_score(state, *edge, last_move);
		if s > best_s {
			best_s = s;
			best_i = i;
		}
	}
	untried.swap_remove(best_i)
}

fn prior_of(state: &Game, mover: Player, played: &[u64]) -> f64 {
	let h = zobrist::occupancy_hash(state);
	let repeats = count_repeats(played, &[], h);
	let ev = evaluate(state, mover, repeats);
	(f64::from(ev) / 350.0).tanh() * 0.5 + 0.5
}

fn child_vis_q(tree: &MctsTree, idx: usize, config: SearchConfig) -> (f64, f64) {
	let n = &tree.nodes[idx];
	let virtual_node = n.action.is_some_and(Edge::is_virtual);
	if config.mcts_transpositions && !virtual_node {
		if let Some(t) = tree.transpo.get(&n.hash) {
			if t.visits > 0 {
				return (f64::from(t.visits), t.value / f64::from(t.visits));
			}
		}
	}
	let vis = f64::from(n.visits);
	let q = if n.visits == 0 { 0.0 } else { n.value / vis };
	(vis, q)
}

fn puct_child(tree: &MctsTree, idx: usize, config: SearchConfig, path: &[usize]) -> usize {
	let chooser = tree.nodes[idx].to_move;
	let children = &tree.nodes[idx].children;
	if config.mcts_solver {
		for &c in children {
			if path.contains(&c) {
				continue;
			}
			if proven_for_chooser(chooser, &tree.nodes[c]) == Some(1) {
				return c;
			}
		}
	}
	let mut pool: Vec<usize> = children
		.iter()
		.copied()
		.filter(|&c| !path.contains(&c))
		.filter(|&c| {
			if !config.mcts_solver {
				return true;
			}
			proven_for_chooser(chooser, &tree.nodes[c]) != Some(-1)
		})
		.collect();
	if pool.is_empty() {
		pool = children.iter().copied().filter(|c| !path.contains(c)).collect();
	}
	if pool.is_empty() {
		return children[0];
	}
	let parent_n = tree.nodes[idx].visits.max(1) as f64;
	let mut best_idx = pool[0];
	let mut best = f64::NEG_INFINITY;
	let mut prior_sum = 0.0;
	let rave_eq = f64::from(config.rave_eq).max(1.0);
	let c_puct = f64::from(config.puct);
	for &c in &pool {
		prior_sum += tree.nodes[c].prior.max(0.05);
	}
	if prior_sum <= 0.0 {
		prior_sum = 1.0;
	}
	for &c in &pool {
		let n = &tree.nodes[c];
		let (vis, q) = child_vis_q(tree, c, config);
		let (rv, rn) = rave_q(n, n.action);
		let beta = if vis + rn <= 0.0 { 0.0 } else { rn / (vis + rn + 4.0 * vis * rn / rave_eq) };
		let mixed = (1.0 - beta) * q + beta * rv;
		let p = n.prior.max(0.05) / prior_sum;
		let u = c_puct * p * parent_n.sqrt() / (1.0 + vis);
		let score = mixed + u;
		if score > best {
			best = score;
			best_idx = c;
		}
	}
	best_idx
}

fn rave_q(node: &Node, action: Option<Edge>) -> (f64, f64) {
	let Some(mv) = action else {
		return (0.0, 0.0);
	};
	let rn = *node.rave_visits.get(&mv).unwrap_or(&0) as f64;
	if rn <= 0.0 {
		return (0.0, 0.0);
	}
	let rv = node.rave_value.get(&mv).copied().unwrap_or(0.0) / rn;
	(rv, rn)
}

fn backprop(
	tree: &mut MctsTree,
	path: &[usize],
	root_player: Player,
	value: f64,
	config: SearchConfig,
) {
	for (i, &idx) in path.iter().enumerate() {
		let actor = if i == 0 { tree.nodes[idx].to_move } else { tree.nodes[path[i - 1]].to_move };
		let signed = if actor == root_player { value } else { -value };
		{
			let node = &mut tree.nodes[idx];
			node.visits += 1;
			node.value += signed;
		}
		let virtual_node = tree.nodes[idx].action.is_some_and(Edge::is_virtual);
		if config.mcts_transpositions && !virtual_node {
			let hash = tree.nodes[idx].hash;
			let t = tree.transpo.entry(hash).or_default();
			t.visits += 1;
			t.value += signed;
			if let Some(p) = tree.nodes[idx].proven {
				t.proven = Some(p);
			}
		}
	}
}

fn update_proven(tree: &mut MctsTree, path: &[usize], config: SearchConfig) {
	if !config.mcts_solver {
		return;
	}
	for &idx in path.iter().rev() {
		if tree.nodes[idx].proven.is_some() {
			continue;
		}
		if tree.nodes[idx].children.is_empty() {
			continue;
		}
		let chooser = tree.nodes[idx].to_move;
		let mut any_win = false;
		let mut all_proven = true;
		let mut all_loss = true;
		for &c in &tree.nodes[idx].children {
			match proven_for_chooser(chooser, &tree.nodes[c]) {
				Some(v) if v > 0 => {
					any_win = true;
					all_loss = false;
				}
				Some(v) if v < 0 => {}
				Some(_) => all_loss = false,
				None => {
					all_proven = false;
					all_loss = false;
				}
			}
		}
		if any_win {
			// A single forced win is enough; remaining untried moves do not matter.
			tree.nodes[idx].proven = Some(1);
		} else if !tree.nodes[idx].untried.is_empty() {
			continue;
		} else if all_proven && all_loss {
			tree.nodes[idx].proven = Some(-1);
		} else if all_proven {
			tree.nodes[idx].proven = Some(0);
		}
	}
}

fn update_rave(
	tree: &mut MctsTree,
	path: &[usize],
	select_actions: &[(Player, SearchMove)],
	playout_actions: &[(Player, SearchMove)],
	root_player: Player,
	value: f64,
) {
	let later: Vec<(Player, SearchMove)> =
		select_actions.iter().copied().chain(playout_actions.iter().copied()).collect();
	for (path_i, &idx) in path.iter().enumerate() {
		let to_move = tree.nodes[idx].to_move;
		let signed = if to_move == root_player { value } else { -value };
		for &(player, mv) in later.iter().skip(path_i) {
			if player != to_move {
				continue;
			}
			let edge = Edge::Real(mv);
			let node = &mut tree.nodes[idx];
			*node.rave_visits.entry(edge).or_insert(0) += 1;
			*node.rave_value.entry(edge).or_insert(0.0) += signed;
		}
	}
}

fn random_playout(
	game: &mut Game,
	rng: &mut impl Rng,
	root: Player,
	played: &[u64],
	out_actions: &mut Vec<(Player, SearchMove)>,
	config: SearchConfig,
) -> f64 {
	for _ in 0..config.playout_cutoff {
		if game.status().is_over() {
			break;
		}
		let moves = legal_search_moves(game);
		if moves.is_empty() {
			break;
		}
		let player = game.current_player();
		let mv = moves[rng.gen_range(0..moves.len())];
		if apply_move(game, mv).is_err() {
			break;
		}
		out_actions.push((player, mv));
	}
	leaf_value(game, root, played, config)
}

fn heavy_playout(
	game: &mut Game,
	rng: &mut impl Rng,
	root: Player,
	played: &[u64],
	out_actions: &mut Vec<(Player, SearchMove)>,
	config: SearchConfig,
) -> f64 {
	let epsilon = f64::from(config.playout_epsilon).clamp(0.0, 1.0);
	for _ in 0..config.playout_cutoff {
		if game.status().is_over() {
			break;
		}
		let moves = legal_search_moves(game);
		if moves.is_empty() {
			break;
		}
		let player = game.current_player();
		let mv = if rng.gen_bool(epsilon) {
			moves[rng.gen_range(0..moves.len())]
		} else {
			greedy_sample(game, &moves, rng, played)
		};
		if apply_move(game, mv).is_err() {
			break;
		}
		out_actions.push((player, mv));
	}
	leaf_value(game, root, played, config)
}

fn leaf_value(game: &Game, root: Player, played: &[u64], config: SearchConfig) -> f64 {
	if let Some(term) = terminal_score(game, root) {
		return f64::from(term) / f64::from(WIN);
	}
	if config.playout_leaf_depth == 0 {
		let h = zobrist::occupancy_hash(game);
		let repeats = count_repeats(played, &[], h);
		return eval_unit(game, root, repeats);
	}
	let side = game.current_player();
	let v = short_search(game, side, played, config.playout_leaf_depth, config);
	if side == root {
		v
	} else {
		-v
	}
}

fn short_search(game: &Game, me: Player, played: &[u64], depth: u8, config: SearchConfig) -> f64 {
	if let Some(term) = terminal_score(game, me) {
		return f64::from(term) / f64::from(WIN);
	}
	if depth == 0 {
		let h = zobrist::occupancy_hash(game);
		let repeats = count_repeats(played, &[], h);
		return eval_unit(game, me, repeats);
	}
	let mut moves = legal_search_moves(game);
	if moves.is_empty() {
		let h = zobrist::occupancy_hash(game);
		let repeats = count_repeats(played, &[], h);
		return eval_unit(game, me, repeats);
	}
	moves.sort_by_cached_key(|mv| std::cmp::Reverse(cheap_order(game, *mv)));
	let cap = if depth >= 2 { usize::from(config.q_capture_cap).clamp(4, 12) } else { 12usize };
	if moves.len() > cap {
		moves.truncate(cap);
	}
	let mover = game.current_player();
	let mut best = if mover == me { f64::NEG_INFINITY } else { f64::INFINITY };
	let mut any = false;
	for mv in moves {
		let mut child = game.clone();
		if apply_move(&mut child, mv).is_err() {
			continue;
		};
		let v = short_search(&child, me, played, depth.saturating_sub(1), config);
		any = true;
		if mover == me {
			if v > best {
				best = v;
			}
		} else if v < best {
			best = v;
		}
	}
	if !any {
		let h = zobrist::occupancy_hash(game);
		let repeats = count_repeats(played, &[], h);
		return eval_unit(game, me, repeats);
	}
	best
}

fn greedy_sample(
	game: &Game,
	moves: &[SearchMove],
	rng: &mut impl Rng,
	played: &[u64],
) -> SearchMove {
	let cap = 10usize;
	let sample: Vec<SearchMove> = if moves.len() <= cap {
		moves.to_vec()
	} else {
		let mut idx: Vec<usize> = (0..moves.len()).collect();
		for i in 0..cap {
			let j = rng.gen_range(i..idx.len());
			idx.swap(i, j);
		}
		idx[..cap].iter().map(|&i| moves[i]).collect()
	};
	let me = game.current_player();
	let mut best = sample[0];
	let mut best_s = i32::MIN;
	for mv in sample {
		let mut child = game.clone();
		if apply_move(&mut child, mv).is_err() {
			continue;
		}
		let h = zobrist::occupancy_hash(&child);
		let repeats = count_repeats(played, &[], h);
		let s = evaluate(&child, me, repeats);
		if s > best_s {
			best_s = s;
			best = mv;
		}
	}
	best
}

pub fn advance(tree: &mut Option<MctsTree>, game_after: &Game, config: SearchConfig) {
	if !config.mcts_tree_reuse {
		*tree = None;
		return;
	}
	if let Some(t) = tree.as_mut() {
		t.sync(game_after, None, config);
	}
}

#[cfg(test)]
mod tests {
	#![allow(clippy::unwrap_used, clippy::expect_used)]

	use super::*;
	use crate::ai::Searcher;
	use crate::game::{Action, EndReason, Game, GameStatus, Player, RuleSet, TileColor};
	use rand::rngs::StdRng;
	use rand::SeedableRng;

	fn hold_win_position() -> (Game, crate::board::NodeId, crate::board::NodeId) {
		let mut rules = RuleSet::default();
		rules.center_hold_win = true;
		rules.center_hold_n = 1;
		rules.no_same_piece_twice = crate::game::SamePieceTwice::Off;
		let mut game = Game::empty(rules);
		let center = game.board().center();
		let from = game.board().neighbors(center).first().copied().expect("center has neighbors");
		let mover = game.alloc_tile(Player::One, TileColor::Blue, 3, 1);
		game.place_tile(from, mover);
		let opp = game.alloc_tile(Player::Two, TileColor::Orange, 3, 1);
		game.set_bench(Player::Two, vec![opp]);
		game.pad_above_low_value();
		game.set_current(Player::One);
		(game, from, center)
	}

	#[test]
	fn threads_one_is_deterministic_with_deep_mcts() {
		let game = Game::new(RuleSet::default(), 21);
		let cfg = SearchConfig::deep_mcts();
		assert_eq!(cfg.mcts_root_threads, 1);
		let limit = SearchLimit::work(120, 20);
		let mut a = Searcher::with_config(cfg);
		let mut b = Searcher::with_config(cfg);
		let mut rng_a = StdRng::seed_from_u64(21);
		let mut rng_b = StdRng::seed_from_u64(21);
		let ma = a.choose_with(&game, crate::ai::Actor::Mcts, &mut rng_a, limit);
		let mb = b.choose_with(&game, crate::ai::Actor::Mcts, &mut rng_b, limit);
		assert_eq!(ma, mb);
		assert_eq!(a.stats().iterations, b.stats().iterations);
	}

	#[test]
	fn mcts_takes_cascade_control_win_at_target_four() {
		use crate::game::EndgameRule;

		let mut rules = RuleSet::default();
		rules.center_hold_win = false;
		rules.control_win = true;
		rules.endgame_rule = EndgameRule::ControlCascade;
		rules.control_hold_turns = 1;
		let mut game = Game::empty(rules);
		let center = game.board().center();
		let inner = game.board().inner_ring().to_vec();
		let occupant = game.alloc_tile(Player::One, TileColor::Green, 3, 1);
		game.place_tile(center, occupant);
		for &node in inner.iter().take(2) {
			let tile = game.alloc_tile(Player::One, TileColor::Green, 3, 1);
			game.place_tile(node, tile);
		}
		let land = inner[2];
		let from = game
			.board()
			.neighbors(land)
			.iter()
			.copied()
			.find(|&n| game.board().is_spawn(n) && game.tile_at(n).is_none())
			.expect("empty inner has a spawn neighbor");
		let mover = game.alloc_tile(Player::One, TileColor::Green, 3, 1);
		game.place_tile(from, mover);
		game.set_budget_tokens(Player::One, Vec::new());
		let mut hot = std::collections::HashSet::new();
		for (node, tile) in game.tiles() {
			if tile.owner == Player::One {
				hot.insert(node);
				hot.extend(game.board().neighbors(node).iter().copied());
			}
		}
		hot.insert(land);
		hot.extend(game.board().neighbors(land).iter().copied());
		let p2_homes: Vec<_> = game
			.board()
			.spawns()
			.iter()
			.copied()
			.filter(|s| !hot.contains(s) && game.tile_at(*s).is_none())
			.take(2)
			.collect();
		assert!(p2_homes.len() >= 2, "need two distant spawns for P2, got {p2_homes:?}");
		for &s in &p2_homes {
			let opp = game.alloc_tile(Player::Two, TileColor::Orange, 3, 1);
			game.place_tile(s, opp);
		}
		game.set_budget_tokens(Player::Two, Vec::new());
		assert_eq!(game.control_need(), 4);
		assert!(game.is_alive(Player::Two));
		game.set_current(Player::One);
		let mut rng = StdRng::seed_from_u64(1);
		let mut searcher = Searcher::new();
		let action = searcher.choose_with(
			&game,
			crate::ai::Actor::Mcts,
			&mut rng,
			SearchLimit::work(800, 160),
		);
		assert!(game.apply(action).is_ok());
		assert_eq!(
			game.status(),
			GameStatus::Won { winner: Player::One, reason: EndReason::Control },
			"MCTS should take the cascade control win, got {:?} after {action:?}",
			game.status()
		);
	}

	#[test]
	fn solver_picks_immediate_center_hold_win() {
		let (game, from, center) = hold_win_position();
		let mut cfg = SearchConfig::enhanced();
		cfg.mcts_solver = true;
		cfg.mcts_root_threads = 1;
		cfg.mcts_tree_reuse = false;
		let mut tree = None;
		let mut rng = StdRng::seed_from_u64(1);
		let (action, _) =
			search(&mut tree, &game, &[], None, &mut rng, SearchLimit::work(200, 48), cfg);
		assert_eq!(action, Action::Move { from, to: center });
		let t = tree.as_ref().expect("tree");
		let mut proven_win = false;
		for &c in &t.nodes[t.root].children {
			if t.nodes[c].action == Some(Edge::Real(SearchMove::Move { from, to: center })) {
				if t.nodes[c].proven == Some(-1) || t.nodes[c].proven == Some(1) {
					proven_win = true;
				}
				let mut child = game.clone();
				apply_move(&mut child, SearchMove::Move { from, to: center }).unwrap();
				assert_eq!(
					child.status(),
					GameStatus::Won { winner: Player::One, reason: EndReason::CenterHold }
				);
			}
		}
		assert!(
			proven_win || t.nodes[t.root].proven == Some(1),
			"solver should prove the hold-win child or the root"
		);
	}

	#[test]
	fn tree_reuse_keeps_the_played_subtree() {
		let game = Game::new(RuleSet::default(), 5);
		let mut cfg = SearchConfig::enhanced();
		cfg.mcts_tree_reuse = true;
		cfg.mcts_root_threads = 1;
		let mut tree = None;
		let mut rng = StdRng::seed_from_u64(5);
		let (action, _) =
			search(&mut tree, &game, &[], None, &mut rng, SearchLimit::work(80, 32), cfg);
		let t = tree.as_ref().expect("tree");
		let mv = SearchMove::from_action(&game, action);
		let mut child_visits = 0u32;
		for &c in &t.nodes[t.root].children {
			if t.nodes[c].action.and_then(Edge::as_real) == Some(mv) {
				child_visits = t.nodes[c].visits;
			}
		}
		let mut after = game.clone();
		after.apply(action).unwrap();
		advance(&mut tree, &after, cfg);
		let t = tree.as_ref().expect("reused");
		assert_eq!(t.nodes[t.root].hash, zobrist::hash(&after));
		assert_eq!(
			t.nodes[t.root].visits, child_visits,
			"reused root should keep the played child's visit count"
		);
		assert!(t.nodes[t.root].visits > 0);
	}

	#[test]
	fn tree_reuse_off_rebuilds() {
		let game = Game::new(RuleSet::default(), 6);
		let mut cfg = SearchConfig::enhanced();
		cfg.mcts_tree_reuse = false;
		let mut tree = None;
		let mut rng = StdRng::seed_from_u64(6);
		let (action, _) =
			search(&mut tree, &game, &[], None, &mut rng, SearchLimit::work(40, 12), cfg);
		let mut after = game.clone();
		after.apply(action).unwrap();
		advance(&mut tree, &after, cfg);
		assert!(tree.is_none(), "reuse=off should drop the tree");
	}

	#[test]
	fn root_spread_is_populated() {
		let game = Game::new(RuleSet::default(), 8);
		let mut rng = StdRng::seed_from_u64(8);
		let mut searcher = Searcher::new();
		let _ = searcher.choose_with(
			&game,
			crate::ai::Actor::Mcts,
			&mut rng,
			SearchLimit::work(80, 32),
		);
		let st = searcher.stats();
		assert!(st.root_legal >= 8);
		assert!(st.root_eff_branch >= 1.0);
		assert!(st.root_top1_share > 0.0 && st.root_top1_share <= 1.0 + 1e-3);
		assert!(st.root_entropy >= 0.0);
	}
}
