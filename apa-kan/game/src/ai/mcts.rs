//! PUCT/UCT with RAVE, eval priors, tree reuse, and heavy determinized playouts.

use std::collections::HashMap;
use std::time::Instant;

use rand::Rng;

use crate::game::{Action, Game, Player};

use super::eval::{eval_unit, evaluate, terminal_score};
use super::search_move::{apply_move, cheap_order, legal_search_moves, SearchMove};
use super::zobrist;
use super::{count_repeats, SearchStats};

const C_PUCT: f64 = 1.6;
const RAVE_EQ: f64 = 300.0;
const PLAYOUT_CUTOFF: u8 = 12;
const EPSILON: f64 = 0.18;

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
	fn new(game: &Game) -> Self {
		let root = make_root(game);
		Self { nodes: vec![root], root: 0 }
	}

	fn sync(&mut self, game: &Game) {
		let h = zobrist::hash(game);
		if self.nodes.get(self.root).map(|n| n.hash) == Some(h) {
			return;
		}
		if let Some(idx) = find_hash(&self.nodes, self.root, h, 4) {
			self.root = idx;
			self.nodes[idx].parent = None;
		} else {
			*self = Self::new(game);
		}
	}
}

fn make_root(game: &Game) -> Node {
	Node {
		parent: None,
		action: None,
		hash: zobrist::hash(game),
		to_move: game.current_player(),
		children: Vec::new(),
		untried: legal_search_moves(game),
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
	deadline: Instant,
) -> (Action, SearchStats) {
	let legal = legal_search_moves(game);
	let fallback = legal.first().and_then(|m| m.to_action(game)).unwrap_or(Action::Pass);
	if legal.is_empty() {
		return (Action::Pass, SearchStats::default());
	}

	let t = tree.get_or_insert_with(|| MctsTree::new(game));
	t.sync(game);

	let mut iterations = 0u32;
	const MAX_ITERS: u32 = 8_000;
	while Instant::now() < deadline && iterations < MAX_ITERS {
		iterations += 1;
		iterate(t, game, played, last_move, rng);
		if iterations % 8 == 0 && Instant::now() >= deadline {
			break;
		}
	}

	let root = t.root;
	let mut best_visits = 0u32;
	let mut best = legal[0];
	for &c in &t.nodes[root].children {
		let n = &t.nodes[c];
		let mut vis = n.visits;
		if is_reverse(n.action.unwrap_or(SearchMove::Pass), last_move) {
			vis = vis / 4;
		}
		if vis >= best_visits {
			best_visits = vis;
			if let Some(a) = n.action {
				best = a;
			}
		}
	}

	(
		best.to_action(game).unwrap_or(fallback),
		SearchStats { depth: 0, iterations, nodes: u64::from(iterations) },
	)
}

fn iterate(
	tree: &mut MctsTree,
	game: &Game,
	played: &[u64],
	last_move: Option<SearchMove>,
	rng: &mut impl Rng,
) {
	let mut state = game.clone();
	// No-op on the public deck. Legacy `capture_draw_on_move` positions still
	// sample a permutation of any hidden remainder.
	state.shuffle_bags(rng);

	let mut idx = tree.root;
	let mut path: Vec<usize> = vec![idx];
	let mut actions: Vec<(Player, SearchMove)> = Vec::new();

	while tree.nodes[idx].untried.is_empty() && !tree.nodes[idx].children.is_empty() {
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

	if !state.status().is_over() && !tree.nodes[idx].untried.is_empty() {
		let mv = pick_untried(&mut tree.nodes[idx].untried, &state, last_move, rng);
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
			untried: if state.status().is_over() { Vec::new() } else { legal_search_moves(&state) },
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
	} else {
		heavy_playout(&mut state, rng, game.current_player(), played, &mut playout_actions)
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
) -> SearchMove {
	if untried.is_empty() {
		return SearchMove::Pass;
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
		t.sync(game_after);
	}
}
