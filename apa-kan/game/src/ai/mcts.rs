//! PUCT/UCT with RAVE, eval priors, tree reuse, and heavy determinized playouts.
//!
//! Optional levers ([`SearchConfig`]): progressive widening, heuristic vs
//! uniform cut-off playouts, and root-parallel independent trees.
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

use super::eval::{eval_unit, evaluate, terminal_score};
use super::search_move::{apply_move, cheap_order, legal_search_moves, SearchMove};
use super::zobrist;
use super::{count_repeats, SearchConfig, SearchLimit, SearchStats};

const C_PUCT: f64 = 1.6;
const RAVE_EQ: f64 = 300.0;
const PLAYOUT_CUTOFF: u8 = 12;
const EPSILON: f64 = 0.18;
const PW_A: f64 = 1.5;
const PW_ALPHA: f64 = 0.5;
const GOLDEN: u64 = 0x9E37_79B9_7F4A_7C15;

struct Node {
	parent: Option<usize>,
	action: Option<SearchMove>,
	hash: u64,
	to_move: Player,
	children: Vec<usize>,
	untried: Vec<SearchMove>,
	visits: u32,
	value: f64,
	prior: f64,
	rave_visits: HashMap<SearchMove, u32>,
	rave_value: HashMap<SearchMove, f64>,
}

pub struct MctsTree {
	nodes: Vec<Node>,
	root: usize,
}

impl MctsTree {
	fn new_with(game: &Game, last_move: Option<SearchMove>, config: SearchConfig) -> Self {
		let root = make_root_with(game, last_move, config);
		Self { nodes: vec![root], root: 0 }
	}

	fn sync(&mut self, game: &Game, last_move: Option<SearchMove>, config: SearchConfig) {
		let h = zobrist::hash(game);
		if self.nodes.get(self.root).map(|n| n.hash) == Some(h) {
			return;
		}
		if let Some(idx) = find_hash(&self.nodes, self.root, h, 4) {
			self.root = idx;
			self.nodes[idx].parent = None;
		} else {
			*self = Self::new_with(game, last_move, config);
		}
	}
}

fn pw_limit(visits: u32) -> usize {
	1 + (PW_A * (visits as f64).powf(PW_ALPHA)) as usize
}

fn should_expand(node: &Node, config: SearchConfig) -> bool {
	if node.untried.is_empty() {
		return false;
	}
	if !config.mcts_progressive_widening {
		return true;
	}
	node.children.len() < pw_limit(node.visits)
}

fn prepare_untried(
	game: &Game,
	last_move: Option<SearchMove>,
	config: SearchConfig,
) -> Vec<SearchMove> {
	let mut untried = legal_search_moves(game);
	if config.mcts_progressive_widening || config.move_ordering {
		untried.sort_by_cached_key(|mv| {
			let mut s = cheap_order(game, *mv);
			if is_reverse(*mv, last_move) {
				s -= 500;
			}
			s
		});
	}
	untried
}

fn make_root_with(game: &Game, last_move: Option<SearchMove>, config: SearchConfig) -> Node {
	Node {
		parent: None,
		action: None,
		hash: zobrist::hash(game),
		to_move: game.current_player(),
		children: Vec::new(),
		untried: prepare_untried(game, last_move, config),
		visits: 0,
		value: 0.0,
		prior: 1.0,
		rave_visits: HashMap::new(),
		rave_value: HashMap::new(),
	}
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
	let stats = run_iterations(t, game, played, last_move, rng, limit, config);
	let best = pick_best(t, last_move, &legal);
	let (mcts_max_depth, mcts_avg_depth) = tree_depth_stats(t);
	(
		best.to_action(game).unwrap_or(fallback),
		SearchStats {
			iterations: stats,
			nodes: u64::from(stats),
			mcts_max_depth,
			mcts_avg_depth,
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
) -> u32 {
	let mut iterations = 0u32;
	const MAX_ITERS: u32 = 8_000;
	match limit {
		SearchLimit::Time(think) => {
			let deadline = Instant::now() + think;
			while Instant::now() < deadline && iterations < MAX_ITERS {
				iterations += 1;
				iterate(tree, game, played, last_move, rng, config);
				if iterations % 8 == 0 && Instant::now() >= deadline {
					break;
				}
			}
		}
		SearchLimit::Work { iters, .. } => {
			while iterations < iters {
				iterations += 1;
				iterate(tree, game, played, last_move, rng, config);
			}
		}
	}
	iterations
}

fn pick_best(tree: &MctsTree, last_move: Option<SearchMove>, legal: &[SearchMove]) -> SearchMove {
	let root = tree.root;
	let mut best_visits = 0u32;
	let mut best = legal[0];
	for &c in &tree.nodes[root].children {
		let n = &tree.nodes[c];
		let mut vis = n.visits;
		if is_reverse(n.action.unwrap_or(SearchMove::Pass), last_move) {
			vis /= 4;
		}
		if vis >= best_visits {
			best_visits = vis;
			if let Some(a) = n.action {
				best = a;
			}
		}
	}
	best
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
							return (Vec::new(), 0, 0, 0.0);
						}
						SearchLimit::Work { nodes, iters: share }
					}
					_ => limit,
				};
				let iterations = run_iterations(
					&mut tree,
					game,
					played,
					last_move,
					&mut rng,
					worker_limit,
					config,
				);
				let (max_d, avg_d) = tree_depth_stats(&tree);
				let mut visits = Vec::new();
				for &c in &tree.nodes[tree.root].children {
					if let Some(a) = tree.nodes[c].action {
						visits.push((a, tree.nodes[c].visits));
					}
				}
				(visits, iterations, max_d, avg_d)
			}));
		}
		handles
			.into_iter()
			.map(|h| h.join().unwrap_or_else(|_| (Vec::new(), 0, 0, 0.0)))
			.collect::<Vec<_>>()
	});

	let mut combined: HashMap<SearchMove, u32> = HashMap::new();
	let mut iterations = 0u32;
	let mut max_d = 0u8;
	let mut avg_num = 0.0f64;
	let mut avg_den = 0.0f64;
	for (visits, iters, d, avg) in results {
		iterations += iters;
		max_d = max_d.max(d);
		if avg > 0.0 {
			avg_num += f64::from(avg) * f64::from(iters);
			avg_den += f64::from(iters);
		}
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
	(
		best.to_action(&game).unwrap_or(fallback),
		SearchStats {
			iterations,
			nodes: u64::from(iterations),
			mcts_max_depth: max_d,
			mcts_avg_depth: avg as f32,
			..SearchStats::default()
		},
	)
}

/// Max tree depth from the current root, and visit-weighted mean depth of
/// non-root nodes. Root is depth 0. Unvisited nodes are ignored.
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
		let next = d.saturating_add(1);
		for &c in &n.children {
			stack.push((c, next));
		}
	}
	let avg = if vsum <= 0.0 { 0.0 } else { wsum / vsum };
	(max_d, avg as f32)
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
	// No-op on the public deck. Legacy `capture_draw_on_move` positions still
	// sample a permutation of any hidden remainder.
	state.shuffle_bags(rng);

	let mut idx = tree.root;
	let mut path: Vec<usize> = vec![idx];
	let mut actions: Vec<(Player, SearchMove)> = Vec::new();

	while !tree.nodes[idx].children.is_empty() && !should_expand(&tree.nodes[idx], config) {
		idx = puct_child(&tree.nodes, idx);
		if let Some(mv) = tree.nodes[idx].action {
			let player = state.current_player();
			let _ = apply_move(&mut state, mv);
			actions.push((player, mv));
		}
		path.push(idx);
		if state.status().is_over() {
			break;
		}
		if count_repeats(played, &[], zobrist::occupancy_hash(&state)) >= 3 {
			break;
		}
	}

	if !state.status().is_over() && should_expand(&tree.nodes[idx], config) {
		let mv = pick_untried(&mut tree.nodes[idx].untried, &state, last_move, rng, config);
		let player = state.current_player();
		let _ = apply_move(&mut state, mv);
		actions.push((player, mv));
		let prior = prior_of(&state, player, played);
		let child = Node {
			parent: Some(idx),
			action: Some(mv),
			hash: zobrist::hash(&state),
			to_move: state.current_player(),
			children: Vec::new(),
			untried: if state.status().is_over() {
				Vec::new()
			} else {
				prepare_untried(&state, last_move, config)
			},
			visits: 0,
			value: 0.0,
			prior,
			rave_visits: HashMap::new(),
			rave_value: HashMap::new(),
		};
		let child_idx = tree.nodes.len();
		tree.nodes[idx].children.push(child_idx);
		tree.nodes.push(child);
		idx = child_idx;
		path.push(idx);
	}

	let mut playout_actions = Vec::new();
	let value = if let Some(term) = terminal_score(&state, game.current_player()) {
		f64::from(term) / f64::from(super::eval::WIN)
	} else if config.mcts_heuristic_playouts {
		heavy_playout(&mut state, rng, game.current_player(), played, &mut playout_actions)
	} else {
		random_playout(&mut state, rng, game.current_player(), played, &mut playout_actions)
	};

	backprop(tree, &path, game.current_player(), value);
	update_rave(tree, &path, &actions, &playout_actions, game.current_player(), value);
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
	untried: &mut Vec<SearchMove>,
	state: &Game,
	last_move: Option<SearchMove>,
	rng: &mut impl Rng,
	config: SearchConfig,
) -> SearchMove {
	if untried.is_empty() {
		return SearchMove::Pass;
	}
	if config.mcts_progressive_widening {
		return untried.pop().unwrap_or(SearchMove::Pass);
	}
	if rng.gen_bool(0.25) {
		let i = rng.gen_range(0..untried.len());
		return untried.swap_remove(i);
	}
	let mut best_i = 0usize;
	let mut best_s = i32::MIN;
	for (i, mv) in untried.iter().enumerate() {
		let mut s = cheap_order(state, *mv);
		if is_reverse(*mv, last_move) {
			s -= 500;
		}
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

fn puct_child(nodes: &[Node], idx: usize) -> usize {
	let parent_n = nodes[idx].visits.max(1) as f64;
	let mut best_idx = nodes[idx].children[0];
	let mut best = f64::NEG_INFINITY;
	let mut prior_sum = 0.0;
	for &c in &nodes[idx].children {
		prior_sum += nodes[c].prior.max(0.05);
	}
	if prior_sum <= 0.0 {
		prior_sum = 1.0;
	}
	for &c in &nodes[idx].children {
		let n = &nodes[c];
		let vis = n.visits as f64;
		let q = if n.visits == 0 { 0.0 } else { n.value / vis };
		let (rv, rn) = rave_q(&nodes[idx], n.action);
		let beta = if vis + rn <= 0.0 { 0.0 } else { rn / (vis + rn + 4.0 * vis * rn / RAVE_EQ) };
		let mixed = (1.0 - beta) * q + beta * rv;
		let p = n.prior.max(0.05) / prior_sum;
		let u = C_PUCT * p * parent_n.sqrt() / (1.0 + vis);
		let score = mixed + u;
		if score > best {
			best = score;
			best_idx = c;
		}
	}
	best_idx
}

fn rave_q(node: &Node, action: Option<SearchMove>) -> (f64, f64) {
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

fn backprop(tree: &mut MctsTree, path: &[usize], root_player: Player, value: f64) {
	for &idx in path {
		let parent = tree.nodes[idx].parent;
		let here = tree.nodes[idx].to_move;
		let actor = parent.map(|p| tree.nodes[p].to_move).unwrap_or(here);
		let signed = if actor == root_player { value } else { -value };
		let node = &mut tree.nodes[idx];
		node.visits += 1;
		node.value += signed;
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
			let node = &mut tree.nodes[idx];
			*node.rave_visits.entry(mv).or_insert(0) += 1;
			*node.rave_value.entry(mv).or_insert(0.0) += signed;
		}
	}
}

fn random_playout(
	game: &mut Game,
	rng: &mut impl Rng,
	root: Player,
	played: &[u64],
	out_actions: &mut Vec<(Player, SearchMove)>,
) -> f64 {
	for _ in 0..PLAYOUT_CUTOFF {
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
	if let Some(term) = terminal_score(game, root) {
		return f64::from(term) / f64::from(super::eval::WIN);
	}
	let h = zobrist::occupancy_hash(game);
	let repeats = count_repeats(played, &[], h);
	eval_unit(game, root, repeats)
}

fn heavy_playout(
	game: &mut Game,
	rng: &mut impl Rng,
	root: Player,
	played: &[u64],
	out_actions: &mut Vec<(Player, SearchMove)>,
) -> f64 {
	for _ in 0..PLAYOUT_CUTOFF {
		if game.status().is_over() {
			break;
		}
		let moves = legal_search_moves(game);
		if moves.is_empty() {
			break;
		}
		let player = game.current_player();
		let mv = if rng.gen_bool(EPSILON) {
			moves[rng.gen_range(0..moves.len())]
		} else {
			greedy_sample(game, &moves, rng, played)
		};
		if apply_move(game, mv).is_err() {
			break;
		}
		out_actions.push((player, mv));
	}
	if let Some(term) = terminal_score(game, root) {
		return f64::from(term) / f64::from(super::eval::WIN);
	}
	let h = zobrist::occupancy_hash(game);
	let repeats = count_repeats(played, &[], h);
	eval_unit(game, root, repeats)
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

pub fn advance(tree: &mut Option<MctsTree>, game_after: &Game) {
	if let Some(t) = tree.as_mut() {
		t.sync(game_after, None, SearchConfig::classic());
	}
}
