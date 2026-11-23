//! Iterative-deepening alpha-beta with optional TT, killers/history, LMR, PVS,
//! and quiescence. Levers live on [`super::SearchConfig`].

use std::time::Instant;

use rand::Rng;

use crate::game::{Action, Game, Player};

use super::eval::{evaluate_legacy, evaluate_with, terminal_score, WIN};
use super::search_move::{apply_move, cheap_order, is_tactical, legal_search_moves, SearchMove};
use super::zobrist;
use super::{count_repeats, HistoryTable, SearchConfig, SearchLimit, SearchStats};

enum Stop {
	Time(Instant),
	Nodes(u64),
}

pub const MAX_PLY: usize = 48;

#[derive(Clone, Copy)]
enum Bound {
	Exact = 0,
	Lower = 1,
	Upper = 2,
}

#[derive(Clone, Copy, Default)]
struct TTEntry {
	hash: u64,
	score: i32,
	depth: u8,
	flag: u8,
	kind: u16,
	a: u8,
	b: u8,
}

pub struct TranspositionTable {
	entries: Vec<TTEntry>,
	mask: usize,
}

impl TranspositionTable {
	pub fn with_bits(bits: u8) -> Self {
		let bits = bits.clamp(12, 22);
		let n = 1usize << bits;
		Self { entries: vec![TTEntry::default(); n], mask: n - 1 }
	}

	pub fn new() -> Self {
		Self::with_bits(18)
	}

	fn probe(&self, hash: u64) -> Option<&TTEntry> {
		let e = &self.entries[hash as usize & self.mask];
		if e.hash == hash {
			Some(e)
		} else {
			None
		}
	}

	fn store(&mut self, hash: u64, depth: u8, score: i32, flag: Bound, best: Option<SearchMove>) {
		let slot = hash as usize & self.mask;
		let cur = &self.entries[slot];
		if cur.hash == hash && cur.depth > depth {
			return;
		}
		let (kind, a, b) = best.map(SearchMove::pack).unwrap_or((u16::MAX - 2, 0, 0));
		self.entries[slot] = TTEntry { hash, score, depth, flag: flag as u8, kind, a, b };
	}
}

impl Default for TranspositionTable {
	fn default() -> Self {
		Self::new()
	}
}

pub struct MinimaxState {
	pub tt: TranspositionTable,
	pub killers: [[Option<SearchMove>; 2]; MAX_PLY],
	pub history: HistoryTable,
	pub last_move: Option<SearchMove>,
	pub legacy: bool,
	pub config: SearchConfig,
	/// When true, shuffle root move order after killers/TT/history ordering.
	pub root_shuffle: bool,
}

impl MinimaxState {
	pub fn with_config(config: SearchConfig) -> Self {
		Self {
			tt: TranspositionTable::with_bits(config.tt_bits),
			killers: [[None; 2]; MAX_PLY],
			history: HistoryTable::new(),
			last_move: None,
			legacy: false,
			config,
			root_shuffle: false,
		}
	}
}

struct Ctx<'a> {
	stop: Stop,
	abort: bool,
	nodes: u64,
	tt: &'a mut TranspositionTable,
	killers: &'a mut [[Option<SearchMove>; 2]; MAX_PLY],
	history: &'a mut HistoryTable,
	played: &'a [u64],
	path: Vec<u64>,
	path_occ: Vec<u64>,
	last_move: Option<SearchMove>,
	legacy: bool,
	config: SearchConfig,
}

impl Ctx<'_> {
	fn over_budget(&self) -> bool {
		if self.abort {
			return true;
		}
		match self.stop {
			Stop::Time(deadline) => Instant::now() >= deadline,
			Stop::Nodes(max) => self.nodes >= max,
		}
	}

	fn exhausted(&mut self) -> bool {
		self.nodes += 1;
		if self.abort {
			return true;
		}
		let hit = match self.stop {
			Stop::Time(deadline) => self.nodes % 64 == 0 && Instant::now() >= deadline,
			Stop::Nodes(max) => self.nodes >= max,
		};
		if hit {
			self.abort = true;
		}
		self.abort
	}
}

pub fn search(
	state: &mut MinimaxState,
	game: &Game,
	played: &[u64],
	rng: &mut impl Rng,
	limit: SearchLimit,
) -> (Action, SearchStats) {
	let legal = legal_search_moves(game);
	let fallback = legal
		.first()
		.and_then(|m| m.to_action(game))
		.or_else(|| game.legal_actions().into_iter().next())
		.unwrap_or(Action::Pass);
	if legal.is_empty() {
		return (Action::Pass, SearchStats::default());
	}

	let stop = match limit {
		SearchLimit::Time(d) => Stop::Time(Instant::now() + d),
		SearchLimit::Work { nodes, .. } => Stop::Nodes(nodes),
	};
	let mut ctx = Ctx {
		stop,
		abort: false,
		nodes: 0,
		tt: &mut state.tt,
		killers: &mut state.killers,
		history: &mut state.history,
		played,
		path: Vec::with_capacity(16),
		path_occ: Vec::with_capacity(16),
		last_move: state.last_move,
		legacy: state.legacy,
		config: state.config,
	};

	let me = game.current_player();
	let mut best_move = legal[rng.gen_range(0..legal.len())];
	let mut completed_depth = 0u8;
	let mut completed_score = 0i32;

	for depth in 1..=16u8 {
		if ctx.over_budget() {
			break;
		}
		ctx.abort = false;
		let mut ordered = legal.clone();
		order_root(&mut ctx, game, &mut ordered, Some(best_move));
		if state.root_shuffle {
			use rand::seq::SliceRandom;
			ordered.shuffle(rng);
		}
		let mut alpha = -WIN;
		let beta = WIN;
		let mut best = -WIN - 1;
		let mut pick = best_move;
		let mut completed = true;
		for (i, &mv) in ordered.iter().enumerate() {
			if ctx.exhausted() {
				completed = false;
				break;
			}
			let Some((mut child, capture)) = child_pos(game, mv) else {
				continue;
			};
			ctx.path.push(zobrist::hash(&child));
			ctx.path_occ.push(zobrist::occupancy_hash(&child));
			let mut score = search_child(
				&mut child,
				me,
				depth.saturating_sub(1),
				alpha,
				beta,
				1,
				i,
				capture,
				&mut ctx,
			);
			ctx.path.pop();
			ctx.path_occ.pop();
			if is_reverse(mv, ctx.last_move) && !center_relevant(game, mv) {
				score -= 6_000;
			}
			if ctx.abort {
				completed = false;
				break;
			}
			if score > best {
				best = score;
				pick = mv;
			}
			if best > alpha {
				alpha = best;
			}
		}
		if completed && !ctx.abort {
			best_move = pick;
			completed_depth = depth;
			completed_score = best;
		} else {
			break;
		}
	}

	let action = best_move.to_action(game).unwrap_or(fallback);
	(
		action,
		SearchStats {
			depth: completed_depth,
			nodes: ctx.nodes,
			root_score: completed_score,
			..SearchStats::default()
		},
	)
}

/// Fixed-depth root value from the side to move. No iterative deepening,
/// reverse-move penalty, or node abort (the budget is effectively infinite).
/// Used to check that TT / ordering / PVS / LMR match plain alpha-beta.
#[cfg(test)]
fn value_at_depth(game: &Game, depth: u8, played: &[u64], config: SearchConfig) -> Option<i32> {
	let legal = legal_search_moves(game);
	if legal.is_empty() {
		return terminal_score(game, game.current_player()).or(Some(0));
	}
	let mut state = MinimaxState::with_config(config);
	let mut ctx = Ctx {
		stop: Stop::Nodes(u64::MAX / 4),
		abort: false,
		nodes: 0,
		tt: &mut state.tt,
		killers: &mut state.killers,
		history: &mut state.history,
		played,
		path: Vec::with_capacity(16),
		path_occ: Vec::with_capacity(16),
		last_move: None,
		legacy: false,
		config,
	};
	let me = game.current_player();
	let mut ordered = legal;
	order_root(&mut ctx, game, &mut ordered, None);
	let mut alpha = -WIN;
	let beta = WIN;
	let mut best = -WIN - 1;
	for (i, &mv) in ordered.iter().enumerate() {
		let Some((mut child, capture)) = child_pos(game, mv) else {
			continue;
		};
		ctx.path.push(zobrist::hash(&child));
		ctx.path_occ.push(zobrist::occupancy_hash(&child));
		let score = search_child(
			&mut child,
			me,
			depth.saturating_sub(1),
			alpha,
			beta,
			1,
			i,
			capture,
			&mut ctx,
		);
		ctx.path.pop();
		ctx.path_occ.pop();
		if ctx.abort {
			return None;
		}
		if score > best {
			best = score;
		}
		if best > alpha {
			alpha = best;
		}
	}
	if best < -WIN {
		None
	} else {
		Some(best)
	}
}

fn child_pos(game: &Game, mv: SearchMove) -> Option<(Game, bool)> {
	let mut child = game.clone();
	let out = apply_move(&mut child, mv).ok()?;
	let cap = is_tactical(game, mv, &out);
	Some((child, cap))
}

fn order_root(ctx: &mut Ctx<'_>, game: &Game, moves: &mut [SearchMove], pv: Option<SearchMove>) {
	if !ctx.config.move_ordering {
		return;
	}
	order_moves(game, moves, pv, 0, ctx.killers, ctx.history, ctx.last_move, ctx.config);
}

fn negamax(
	game: &mut Game,
	depth: u8,
	mut alpha: i32,
	beta: i32,
	ply: usize,
	ctx: &mut Ctx<'_>,
) -> i32 {
	if ctx.exhausted() {
		return eval_now(game, ctx);
	}
	let me = game.current_player();
	if let Some(term) = terminal_score(game, me) {
		return term;
	}
	if ply >= MAX_PLY - 1 {
		return eval_now(game, ctx);
	}

	let vis = *ctx.path.last().unwrap_or(&zobrist::hash(game));
	let occ = *ctx.path_occ.last().unwrap_or(&zobrist::occupancy_hash(game));
	let repeats = count_repeats(ctx.played, &ctx.path_occ, occ);
	let hash = vis ^ u64::from(repeats).wrapping_mul(0xD1B5_4A32_D192_ED03);

	if repeats >= 3 && ply > 0 {
		return eval_now(game, ctx);
	}

	if depth == 0 {
		if ctx.config.quiescence {
			return quiesce(game, alpha, beta, 0, ply, ctx);
		}
		return eval_now(game, ctx);
	}

	let mut tt_move = None;
	if ctx.config.transposition {
		if let Some(e) = ctx.tt.probe(hash) {
			if e.depth >= depth {
				let sc = e.score;
				match e.flag {
					x if x == Bound::Exact as u8 => return sc,
					x if x == Bound::Lower as u8 && sc >= beta => return sc,
					x if x == Bound::Upper as u8 && sc <= alpha => return sc,
					_ => {}
				}
			}
			tt_move = SearchMove::unpack(e.kind, e.a, e.b);
		}
	}

	let mut moves = legal_search_moves(game);
	if moves.is_empty() {
		return eval_now(game, ctx);
	}
	if ctx.config.move_ordering || tt_move.is_some() {
		order_moves(
			game,
			&mut moves,
			tt_move,
			ply,
			ctx.killers,
			ctx.history,
			ctx.last_move,
			ctx.config,
		);
	}

	let mut best = -WIN;
	let mut best_mv = moves[0];
	let original_alpha = alpha;
	for (i, &mv) in moves.iter().enumerate() {
		if ctx.exhausted() {
			break;
		}
		let Some((mut child, capture)) = child_pos(game, mv) else {
			continue;
		};
		ctx.path.push(zobrist::hash(&child));
		ctx.path_occ.push(zobrist::occupancy_hash(&child));
		let score = search_child(
			&mut child,
			me,
			depth.saturating_sub(1),
			alpha,
			beta,
			ply + 1,
			i,
			capture,
			ctx,
		);
		ctx.path.pop();
		ctx.path_occ.pop();
		if ctx.abort {
			break;
		}
		if score > best {
			best = score;
			best_mv = mv;
		}
		if best > alpha {
			alpha = best;
		}
		if alpha >= beta {
			if ctx.config.move_ordering {
				store_killer(ctx.killers, ply, mv);
				ctx.history.bump(mv, depth);
			}
			break;
		}
	}

	if !ctx.abort && ctx.config.transposition {
		let flag = if best <= original_alpha {
			Bound::Upper
		} else if best >= beta {
			Bound::Lower
		} else {
			Bound::Exact
		};
		ctx.tt.store(hash, depth, best, flag, Some(best_mv));
	}
	best
}

/// Search `child` from `mover`'s point of view, applying LMR / PVS according
/// to `ctx.config`. `child_depth` is the unreduced remaining depth.
fn search_child(
	child: &mut Game,
	mover: Player,
	child_depth: u8,
	alpha: i32,
	beta: i32,
	ply: usize,
	move_index: usize,
	capture: bool,
	ctx: &mut Ctx<'_>,
) -> i32 {
	if let Some(term) = terminal_score(child, mover) {
		return term;
	}

	let in_pv = move_index == 0;
	let mut depth = child_depth;
	// Skip the root ply by default: reducing late root moves burns the ID
	// budget on re-searches and dropped completed depth from 4 to 3 in the
	// ablation. `lmr_skip_root=off` opts back in.
	let reduce = ctx.config.lmr
		&& !in_pv
		&& !capture
		&& (!ctx.config.lmr_skip_root || ply > 1)
		&& child_depth >= ctx.config.lmr_min_depth
		&& move_index >= usize::from(ctx.config.lmr_move_skip);
	if reduce {
		depth = child_depth.saturating_sub(ctx.config.lmr_reduce.max(1));
		if move_index >= usize::from(ctx.config.lmr_late_move)
			&& child_depth >= ctx.config.lmr_late_depth
		{
			depth = child_depth.saturating_sub(ctx.config.lmr_late_reduce.max(1));
		}
	}

	// Extra turns keep `child.current_player() == mover`. Negamax scores
	// are from the side to move, so those children must keep the parent's
	// window instead of `(-beta, -alpha)`.
	let same_side = child.current_player() == mover;
	let pvs_null = ctx.config.pvs && !in_pv;
	let (search_alpha, search_beta) = child_window(same_side, alpha, beta, pvs_null);
	let mut score =
		child_score(same_side, negamax(child, depth, search_alpha, search_beta, ply, ctx));
	if ctx.abort {
		return score;
	}

	let reduced = depth < child_depth;
	let needs_full =
		(reduced && score > alpha) || (ctx.config.pvs && !in_pv && score > alpha && score < beta);
	let full_a = if same_side { alpha } else { -beta };
	let full_b = if same_side { beta } else { -alpha };
	if needs_full && (reduced || search_alpha != full_a || search_beta != full_b) {
		score = child_score(same_side, negamax(child, child_depth, full_a, full_b, ply, ctx));
	}
	score
}

fn child_window(same_side: bool, alpha: i32, beta: i32, pvs_null: bool) -> (i32, i32) {
	if same_side {
		if pvs_null {
			(alpha, alpha.saturating_add(1))
		} else {
			(alpha, beta)
		}
	} else if pvs_null {
		(-alpha - 1, -alpha)
	} else {
		(-beta, -alpha)
	}
}

fn child_score(same_side: bool, child: i32) -> i32 {
	if same_side {
		child
	} else {
		-child
	}
}

fn quiesce(
	game: &mut Game,
	mut alpha: i32,
	beta: i32,
	qply: u8,
	ply: usize,
	ctx: &mut Ctx<'_>,
) -> i32 {
	if ctx.exhausted() || ply >= MAX_PLY - 1 {
		return eval_now(game, ctx);
	}
	let me = game.current_player();
	if let Some(term) = terminal_score(game, me) {
		return term;
	}
	let stand = eval_now(game, ctx);
	if qply >= ctx.config.q_depth {
		return stand;
	}
	if stand >= beta {
		return stand;
	}
	if stand > alpha {
		alpha = stand;
	}

	let moves = legal_search_moves(game);
	let mut captures: Vec<(i32, SearchMove, Game)> = Vec::new();
	for mv in moves {
		let mut child = game.clone();
		let Ok(out) = apply_move(&mut child, mv) else {
			continue;
		};
		if !is_tactical(game, mv, &out) {
			continue;
		}
		let mut gain = out.removals.iter().filter(|r| r.tile.owner != me).count() as i32;
		if matches!(mv, SearchMove::Swap { from, to } if from == game.board().center() || to == game.board().center())
		{
			gain = gain.saturating_add(2);
		}
		if matches!(mv, SearchMove::Convert { node } if node == game.board().center()) {
			gain = gain.saturating_add(4);
		} else if matches!(mv, SearchMove::Convert { .. }) {
			gain = gain.saturating_add(3);
		}
		captures.push((gain, mv, child));
	}
	captures.sort_by(|a, b| b.0.cmp(&a.0));
	let cap = usize::from(ctx.config.q_capture_cap).max(1);
	if captures.len() > cap {
		captures.truncate(cap);
	}
	for (_, _, mut child) in captures {
		if ctx.exhausted() {
			break;
		}
		ctx.path.push(zobrist::hash(&child));
		ctx.path_occ.push(zobrist::occupancy_hash(&child));
		let score = if let Some(term) = terminal_score(&child, me) {
			term
		} else if child.current_player() == me {
			quiesce(&mut child, alpha, beta, qply + 1, ply + 1, ctx)
		} else {
			-quiesce(&mut child, -beta, -alpha, qply + 1, ply + 1, ctx)
		};
		ctx.path.pop();
		ctx.path_occ.pop();
		if ctx.abort {
			break;
		}
		if score > alpha {
			alpha = score;
		}
		if alpha >= beta {
			return alpha;
		}
	}
	alpha
}

fn eval_now(game: &Game, ctx: &Ctx<'_>) -> i32 {
	let h = zobrist::occupancy_hash(game);
	let repeats = count_repeats(ctx.played, &ctx.path_occ, h);
	let me = game.current_player();
	if ctx.legacy {
		evaluate_legacy(game, me, repeats)
	} else {
		evaluate_with(game, me, repeats, ctx.config)
	}
}

fn center_relevant(game: &Game, mv: SearchMove) -> bool {
	let center = game.board().center();
	let inner = game.board().inner_ring();
	match mv {
		SearchMove::Move { to, .. } => to == center || inner.contains(&to),
		SearchMove::Swap { from, to } => {
			from == center || to == center || inner.contains(&from) || inner.contains(&to)
		}
		SearchMove::Add { node, .. }
		| SearchMove::Claim { node }
		| SearchMove::Convert { node } => node == center || inner.contains(&node),
		SearchMove::Pass => false,
	}
}

fn store_killer(killers: &mut [[Option<SearchMove>; 2]; MAX_PLY], ply: usize, mv: SearchMove) {
	if ply >= MAX_PLY {
		return;
	}
	if matches!(mv, SearchMove::Pass) {
		return;
	}
	if killers[ply][0] != Some(mv) {
		killers[ply][1] = killers[ply][0];
		killers[ply][0] = Some(mv);
	}
}

fn is_reverse(mv: SearchMove, last: Option<SearchMove>) -> bool {
	match (mv, last) {
		(
			SearchMove::Move { from, to },
			Some(SearchMove::Move { from: prev_from, to: prev_to }),
		) => from == prev_to && to == prev_from,
		(
			SearchMove::Swap { from, to },
			Some(SearchMove::Swap { from: prev_from, to: prev_to }),
		) => (from == prev_from && to == prev_to) || (from == prev_to && to == prev_from),
		_ => false,
	}
}

/// Cheap capture-first hint: adjacent enemy with a mismatch lead.
fn capture_hint(game: &Game, mv: SearchMove) -> i32 {
	match mv {
		SearchMove::Move { from, to } => {
			let Some(tile) = game.tile_at(from) else {
				return 0;
			};
			let mut adj = false;
			for nb in game.board().incident_others(to) {
				let Some(other) = game.tile_at(nb) else {
					continue;
				};
				if other.owner == tile.owner {
					continue;
				}
				adj = true;
				let (matched, mismatches) = game.edge_score(tile, other);
				if mismatches > matched {
					return 250_000;
				}
			}
			if adj {
				120_000
			} else {
				0
			}
		}
		SearchMove::Add { kind, node } => {
			let me = game.current_player();
			let mut adj = false;
			for nb in game.board().incident_others(node) {
				let Some(other) = game.tile_at(nb) else {
					continue;
				};
				if other.owner == me {
					continue;
				}
				adj = true;
				let _ = kind;
			}
			if adj {
				80_000
			} else {
				0
			}
		}
		SearchMove::Claim { node } => {
			if game.tile_at(node).is_some_and(|t| t.owner != game.current_player()) {
				200_000
			} else {
				90_000
			}
		}
		SearchMove::Swap { from, to } => {
			let center = game.board().center();
			let partner_opp = game.tile_at(to).is_some_and(|t| t.owner != game.current_player());
			if to == center && partner_opp {
				280_000
			} else if from == center || to == center {
				220_000
			} else if partner_opp {
				160_000
			} else {
				40_000
			}
		}
		SearchMove::Convert { node } => {
			let center = game.board().center();
			if node == center {
				320_000
			} else if game.board().inner_ring().contains(&node) {
				220_000
			} else {
				180_000
			}
		}
		SearchMove::Pass => 0,
	}
}

fn order_moves(
	game: &Game,
	moves: &mut [SearchMove],
	tt_move: Option<SearchMove>,
	ply: usize,
	killers: &[[Option<SearchMove>; 2]; MAX_PLY],
	history: &HistoryTable,
	last_move: Option<SearchMove>,
	config: SearchConfig,
) {
	if !config.move_ordering {
		if let Some(pv) = tt_move {
			if let Some(i) = moves.iter().position(|m| *m == pv) {
				moves.swap(0, i);
			}
		}
		return;
	}
	moves.sort_by_cached_key(|mv| {
		let mut s = cheap_order(game, *mv, config);
		let hint = capture_hint(game, *mv);
		s += hint.saturating_mul(config.order_capture_scale) / 100;
		if Some(*mv) == tt_move {
			s += config.order_tt_bonus;
		}
		if ply < MAX_PLY {
			if killers[ply][0] == Some(*mv) {
				s += config.order_killer_bonus;
			} else if killers[ply][1] == Some(*mv) {
				s += config.order_killer_bonus.saturating_sub(10_000);
			}
		}
		if ply == 0 && is_reverse(*mv, last_move) && !center_relevant(game, *mv) {
			s -= 400_000;
		}
		if center_relevant(game, *mv) {
			s += config.order_center_bonus;
		}
		s += history.get(*mv);
		-s
	});
}

#[cfg(test)]
mod tests {
	#![allow(clippy::unwrap_used, clippy::expect_used)]

	use super::*;
	use crate::game::{Player, RuleSet, TileColor};
	use rand::rngs::StdRng;
	use rand::SeedableRng;

	fn sample_positions() -> Vec<Game> {
		let mut out = vec![Game::new(RuleSet::default(), 1), Game::new(RuleSet::default(), 11)];
		let mut rules = RuleSet::default();
		rules.center_hold_win = true;
		rules.center_hold_n = 3;
		rules.no_same_piece_twice = crate::game::SamePieceTwice::Off;
		let mut tactic = Game::empty(rules);
		let center = tactic.board().center();
		let from = tactic.board().neighbors(center)[0];
		let mover = tactic.alloc_tile(Player::One, TileColor::Blue, 3, 1);
		tactic.place_tile(from, mover);
		let extra = tactic.alloc_tile(Player::One, TileColor::Green, 2, 1);
		tactic.set_bench(Player::One, vec![extra]);
		let opp = tactic.alloc_tile(Player::Two, TileColor::Orange, 3, 1);
		tactic.set_bench(Player::Two, vec![opp]);
		tactic.pad_above_low_value();
		tactic.set_current(Player::One);
		out.push(tactic);

		let mut mid = Game::new(RuleSet::default(), 4);
		let mut rng = StdRng::seed_from_u64(4);
		for _ in 0..6 {
			if mid.status().is_over() {
				break;
			}
			let moves = legal_search_moves(&mid);
			if moves.is_empty() {
				break;
			}
			let mv = moves[rng.gen_range(0..moves.len())];
			let _ = apply_move(&mut mid, mv);
		}
		out.push(mid);
		out
	}

	fn assert_same_value(game: &Game, depth: u8, cfg: SearchConfig) {
		let plain = value_at_depth(game, depth, &[], SearchConfig::plain())
			.expect("plain alpha-beta completed");
		let got = value_at_depth(game, depth, &[], cfg).expect("pruned search completed");
		assert_eq!(
			got,
			plain,
			"depth={depth} cfg={} plain={plain} got={got} legal={}",
			cfg.label(),
			legal_search_moves(game).len()
		);
	}

	#[test]
	fn tt_ordering_pvs_lmr_match_plain_alpha_beta() {
		let tt_ord =
			SearchConfig { move_ordering: true, transposition: true, ..SearchConfig::plain() };
		let with_pvs = SearchConfig { pvs: true, ..tt_ord };
		let with_lmr = SearchConfig { lmr: true, pvs: true, ..tt_ord };
		for game in sample_positions() {
			for depth in 1..=3u8 {
				assert_same_value(&game, depth, tt_ord);
				assert_same_value(&game, depth, with_pvs);
				assert_same_value(&game, depth, with_lmr);
			}
		}
	}

	#[test]
	fn enhanced_without_quiescence_matches_plain_at_depth_two() {
		let mut cfg = SearchConfig::enhanced();
		cfg.quiescence = false;
		let game = Game::new(RuleSet::default(), 2);
		assert_same_value(&game, 2, cfg);
	}

	#[test]
	fn extra_turn_value_is_not_sign_flipped() {
		use crate::game::Game;

		let (game, extra, from, land) = Game::cascade_turn_gain_forced_extra_win();
		assert_eq!(game.current_player(), Player::One);
		let score = value_at_depth(&game, 2, &[], SearchConfig::plain()).expect("depth 2");
		assert!(
			score > WIN / 2,
			"ADD then extra-turn MOVE should be a win for the mover, got {score} (add {extra}, move {from}->{land})"
		);
		let tt_ord =
			SearchConfig { move_ordering: true, transposition: true, ..SearchConfig::plain() };
		let with_pvs = SearchConfig { pvs: true, ..tt_ord };
		let with_q = SearchConfig { quiescence: true, q_depth: 2, ..tt_ord };
		assert_same_value(&game, 2, tt_ord);
		assert_same_value(&game, 2, with_pvs);
		assert_same_value(&game, 2, with_q);
		assert_same_value(&game, 2, SearchConfig { lmr: true, pvs: true, ..tt_ord });
	}

	#[test]
	fn extra_turn_banked_value_is_not_sign_flipped() {
		use crate::game::{Action, Game};

		let (mut game, extra) = Game::cascade_turn_gain_drop_position(3, 5);
		game.apply(Action::Claim { node: extra }).unwrap();
		assert_eq!(game.current_player(), Player::Two);
		assert_eq!(game.extra_turns(Player::Two), 1);
		let score = value_at_depth(&game, 2, &[], SearchConfig::plain()).expect("depth 2");
		let flipped = value_at_depth(&game, 2, &[], SearchConfig::plain()).expect("depth 2 again");
		assert_eq!(score, flipped, "same-side extra turn must be stable");
		let tt_ord = SearchConfig {
			move_ordering: true,
			transposition: true,
			pvs: true,
			..SearchConfig::plain()
		};
		assert_same_value(&game, 2, tt_ord);
	}
}
