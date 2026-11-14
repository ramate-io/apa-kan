//! Iterative-deepening alpha-beta with TT, killers/history, and quiescence.

use std::time::Instant;

use rand::Rng;

use crate::game::{Action, Game, Player};

use super::eval::{evaluate, evaluate_legacy, terminal_score, WIN};
use super::search_move::{apply_move, cheap_order, is_capture, legal_search_moves, SearchMove};
use super::zobrist;
use super::{count_repeats, HistoryTable, SearchLimit, SearchStats};

enum Stop {
	Time(Instant),
	Nodes(u64),
}

pub const MAX_PLY: usize = 48;
const TT_MASK: usize = (1 << 18) - 1;
const Q_DEPTH: u8 = 4;

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
}

impl TranspositionTable {
	pub fn new() -> Self {
		Self { entries: vec![TTEntry::default(); TT_MASK + 1] }
	}

	fn probe(&self, hash: u64) -> Option<&TTEntry> {
		let e = &self.entries[hash as usize & TT_MASK];
		if e.hash == hash {
			Some(e)
		} else {
			None
		}
	}

	fn store(&mut self, hash: u64, depth: u8, score: i32, flag: Bound, best: Option<SearchMove>) {
		let slot = hash as usize & TT_MASK;
		let cur = &self.entries[slot];
		if cur.hash == hash && cur.depth > depth {
			return;
		}
		let (kind, a, b) = best.map(SearchMove::pack).unwrap_or((u16::MAX - 1, 0, 0));
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
}

impl MinimaxState {
	pub fn new() -> Self {
		Self {
			tt: TranspositionTable::new(),
			killers: [[None; 2]; MAX_PLY],
			history: HistoryTable::new(),
			last_move: None,
			legacy: false,
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
	};

	let me = game.current_player();
	let mut best_move = legal[rng.gen_range(0..legal.len())];
	let mut completed_depth = 0u8;

	for depth in 1..=16u8 {
		if ctx.over_budget() {
			break;
		}
		ctx.abort = false;
		let mut ordered = legal.clone();
		order_moves(
			game,
			&mut ordered,
			Some(best_move),
			0,
			ctx.killers,
			ctx.history,
			ctx.last_move,
		);
		let mut alpha = -WIN;
		let beta = WIN;
		let mut best = -WIN - 1;
		let mut pick = best_move;
		let mut completed = true;
		for &mv in &ordered {
			if ctx.exhausted() {
				completed = false;
				break;
			}
			let Some(mut child) = child_pos(game, mv) else {
				continue;
			};
			ctx.path.push(zobrist::hash(&child));
			ctx.path_occ.push(zobrist::occupancy_hash(&child));
			let mut score = score_from_mover(
				&mut child,
				me,
				depth.saturating_sub(1),
				-beta,
				-alpha,
				1,
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
		} else {
			break;
		}
	}

	let action = best_move.to_action(game).unwrap_or(fallback);
	(action, SearchStats { depth: completed_depth, iterations: 0, nodes: ctx.nodes })
}

fn child_pos(game: &Game, mv: SearchMove) -> Option<Game> {
	let mut child = game.clone();
	apply_move(&mut child, mv).ok()?;
	Some(child)
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
		return quiesce(game, alpha, beta, 0, ply, ctx);
	}

	let mut tt_move = None;
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

	let mut moves = legal_search_moves(game);
	if moves.is_empty() {
		return eval_now(game, ctx);
	}
	order_moves(game, &mut moves, tt_move, ply, ctx.killers, ctx.history, ctx.last_move);

	let mut best = -WIN;
	let mut best_mv = moves[0];
	let original_alpha = alpha;
	for &mv in &moves {
		if ctx.exhausted() {
			break;
		}
		let Some(mut child) = child_pos(game, mv) else {
			continue;
		};
		ctx.path.push(zobrist::hash(&child));
		ctx.path_occ.push(zobrist::occupancy_hash(&child));
		let score =
			score_from_mover(&mut child, me, depth.saturating_sub(1), -beta, -alpha, ply + 1, ctx);
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
			store_killer(ctx.killers, ply, mv);
			ctx.history.bump(mv, depth);
			break;
		}
	}

	if !ctx.abort {
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
	if qply >= Q_DEPTH {
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
		if !is_capture(&out, me) {
			continue;
		}
		let gain = out.removals.iter().filter(|r| r.tile.owner != me).count() as i32;
		captures.push((gain, mv, child));
	}
	captures.sort_by(|a, b| b.0.cmp(&a.0));
	if captures.len() > 8 {
		captures.truncate(8);
	}
	for (_, _, mut child) in captures {
		if ctx.exhausted() {
			break;
		}
		ctx.path.push(zobrist::hash(&child));
		ctx.path_occ.push(zobrist::occupancy_hash(&child));
		let score = if let Some(term) = terminal_score(&child, me) {
			term
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
		evaluate(game, me, repeats)
	}
}

/// Score `child` from `mover`'s point of view.
///
/// Center-hold, petal-majority, and control wins are applied in `finish_turn`
/// *before* the side-to-move flips, so the child's `current_player()` is still
/// the winner. Insufficient-tiles losses are also set before the switch (the
/// loser may still be to-move). Negating a terminal scored from STM would turn
/// our own win into a loss (the recorded failure mode: vacating the center on
/// the turn that would have completed the hold). Lockouts already switch STM
/// to the loser, so both conventions agree there.
fn score_from_mover(
	child: &mut Game,
	mover: Player,
	depth: u8,
	alpha: i32,
	beta: i32,
	ply: usize,
	ctx: &mut Ctx<'_>,
) -> i32 {
	if let Some(term) = terminal_score(child, mover) {
		return term;
	}
	-negamax(child, depth, alpha, beta, ply, ctx)
}

fn center_relevant(game: &Game, mv: SearchMove) -> bool {
	let center = game.board().center();
	match mv {
		SearchMove::Move { to, .. } => to == center || game.board().inner_ring().contains(&to),
		SearchMove::Add { node, .. } => game.board().inner_ring().contains(&node),
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
		_ => false,
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
) {
	moves.sort_by_cached_key(|mv| {
		let mut s = cheap_order(game, *mv);
		if Some(*mv) == tt_move {
			s += 1_000_000;
		}
		if ply < MAX_PLY {
			if killers[ply][0] == Some(*mv) {
				s += 90_000;
			} else if killers[ply][1] == Some(*mv) {
				s += 80_000;
			}
		}
		if ply == 0 && is_reverse(*mv, last_move) && !center_relevant(game, *mv) {
			s -= 400_000;
		}
		if center_relevant(game, *mv) {
			s += 8_000;
		}
		s += history.get(*mv);
		-s
	});
}
