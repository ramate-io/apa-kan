//! Bot players: random, greedy one-ply, minimax with alpha-beta, and MCTS.

use std::time::{Duration, Instant};

use rand::Rng;

use crate::game::{Action, EndReason, Game, GameStatus, Player};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Actor {
	Human,
	Random,
	Greedy,
	Minimax,
	Mcts,
}

impl Actor {
	pub fn parse(s: &str) -> Result<Self, String> {
		match s.to_ascii_lowercase().as_str() {
			"human" => Ok(Actor::Human),
			"random" => Ok(Actor::Random),
			"greedy" => Ok(Actor::Greedy),
			"minimax" => Ok(Actor::Minimax),
			"mcts" => Ok(Actor::Mcts),
			other => {
				Err(format!("unknown actor '{other}' (expected human|random|greedy|minimax|mcts)"))
			}
		}
	}

	pub fn name(self) -> &'static str {
		match self {
			Actor::Human => "human",
			Actor::Random => "random",
			Actor::Greedy => "greedy",
			Actor::Minimax => "minimax",
			Actor::Mcts => "mcts",
		}
	}

	pub fn is_human(self) -> bool {
		matches!(self, Actor::Human)
	}
}

pub fn choose(game: &Game, actor: Actor, rng: &mut impl Rng, think: Duration) -> Action {
	let mut legal = game.legal_actions();
	if legal.is_empty() {
		return Action::Pass;
	}
	match actor {
		Actor::Human => legal.remove(0),
		Actor::Random => legal.remove(rng.gen_range(0..legal.len())),
		Actor::Greedy => greedy(game, rng, &legal),
		Actor::Minimax => minimax(game, rng, &legal, think),
		Actor::Mcts => mcts(game, rng, &legal, think),
	}
}

fn greedy(game: &Game, rng: &mut impl Rng, legal: &[Action]) -> Action {
	let me = game.current_player();
	let mut best_score = i32::MIN;
	let mut best: Vec<Action> = Vec::new();
	for &action in legal {
		let mut g = game.clone();
		if g.apply(action).is_err() {
			continue;
		}
		let score = evaluate(&g, me);
		if score > best_score {
			best_score = score;
			best.clear();
			best.push(action);
		} else if score == best_score {
			best.push(action);
		}
	}
	if best.is_empty() {
		return legal[0];
	}
	best[rng.gen_range(0..best.len())]
}

fn minimax(game: &Game, rng: &mut impl Rng, legal: &[Action], think: Duration) -> Action {
	let deadline = Instant::now() + think;
	let me = game.current_player();
	let mut best_action = legal[rng.gen_range(0..legal.len())];
	let max_depth = 6u8;
	for depth in 1..=max_depth {
		if Instant::now() >= deadline {
			break;
		}
		let mut ordered = legal.to_vec();
		ordered.sort_by_key(|a| move_order_key(*a));
		let mut best = i32::MIN;
		let mut picks: Vec<Action> = Vec::new();
		let mut alpha = -WIN + 1;
		let beta = WIN - 1;
		for &action in &ordered {
			if Instant::now() >= deadline {
				break;
			}
			let mut child = game.clone();
			if child.apply(action).is_err() {
				continue;
			}
			let score = -negamax(&mut child, depth - 1, -beta, -alpha, deadline, me);
			if score > best {
				best = score;
				picks.clear();
				picks.push(action);
			} else if score == best {
				picks.push(action);
			}
			if best > alpha {
				alpha = best;
			}
		}
		if Instant::now() < deadline && !picks.is_empty() {
			best_action = picks[rng.gen_range(0..picks.len())];
		}
	}
	best_action
}

const WIN: i32 = 10_000;

fn negamax(
	game: &mut Game,
	depth: u8,
	mut alpha: i32,
	beta: i32,
	deadline: Instant,
	_root: Player,
) -> i32 {
	if Instant::now() >= deadline {
		return evaluate(game, game.current_player());
	}
	if let Some(term) = terminal_score(game, game.current_player()) {
		return term;
	}
	if depth == 0 {
		return evaluate(game, game.current_player());
	}
	let mut legal = game.legal_actions();
	if legal.is_empty() {
		return evaluate(game, game.current_player());
	}
	legal.sort_by_key(|a| move_order_key(*a));
	let mut best = -WIN;
	for &action in &legal {
		if Instant::now() >= deadline {
			break;
		}
		let mut child = game.clone();
		if child.apply(action).is_err() {
			continue;
		}
		let score = -negamax(&mut child, depth - 1, -beta, -alpha, deadline, _root);
		if score > best {
			best = score;
		}
		if best > alpha {
			alpha = best;
		}
		if alpha >= beta {
			break;
		}
	}
	best
}

fn move_order_key(action: Action) -> i32 {
	match action {
		Action::Move { .. } => 0,
		Action::Add { .. } => 1,
		Action::Pass => 2,
	}
}

fn mcts(game: &Game, rng: &mut impl Rng, legal: &[Action], think: Duration) -> Action {
	let deadline = Instant::now() + think;
	let mut nodes: Vec<MctsNode> = Vec::new();
	nodes.push(MctsNode {
		parent: None,
		action: None,
		untried: legal.to_vec(),
		children: Vec::new(),
		visits: 0,
		value: 0.0,
	});
	let root_player = game.current_player();
	while Instant::now() < deadline {
		let mut state = game.clone();
		let mut idx = 0usize;
		while nodes[idx].untried.is_empty() && !nodes[idx].children.is_empty() {
			idx = uct_child(&nodes, idx);
			if let Some(action) = nodes[idx].action {
				let _ = state.apply(action);
			}
		}
		if let Some(term) = terminal_score(&state, root_player) {
			backprop(&mut nodes, idx, term as f64 / WIN as f64);
			continue;
		}
		if !nodes[idx].untried.is_empty() {
			let pick = rng.gen_range(0..nodes[idx].untried.len());
			let action = nodes[idx].untried.swap_remove(pick);
			let _ = state.apply(action);
			let child = MctsNode {
				parent: Some(idx),
				action: Some(action),
				untried: state.legal_actions(),
				children: Vec::new(),
				visits: 0,
				value: 0.0,
			};
			let child_idx = nodes.len();
			nodes[idx].children.push(child_idx);
			nodes.push(child);
			idx = child_idx;
		}
		let value = rollout(&mut state, rng, root_player, 24);
		backprop(&mut nodes, idx, value);
	}
	let mut best_visits = 0u32;
	let mut best_action = legal[0];
	for &c in &nodes[0].children {
		if nodes[c].visits > best_visits {
			best_visits = nodes[c].visits;
			if let Some(a) = nodes[c].action {
				best_action = a;
			}
		}
	}
	best_action
}

struct MctsNode {
	parent: Option<usize>,
	action: Option<Action>,
	untried: Vec<Action>,
	children: Vec<usize>,
	visits: u32,
	value: f64,
}

fn uct_child(nodes: &[MctsNode], idx: usize) -> usize {
	let parent_visits = nodes[idx].visits.max(1) as f64;
	let mut best_idx = nodes[idx].children[0];
	let mut best = f64::NEG_INFINITY;
	for &c in &nodes[idx].children {
		let n = &nodes[c];
		let vis = n.visits.max(1) as f64;
		let exploit = n.value / vis;
		let explore = 1.4 * (parent_visits.ln() / vis).sqrt();
		let uct = exploit + explore;
		if uct > best {
			best = uct;
			best_idx = c;
		}
	}
	best_idx
}

fn backprop(nodes: &mut [MctsNode], mut idx: usize, mut value: f64) {
	loop {
		nodes[idx].visits += 1;
		nodes[idx].value += value;
		value = -value;
		match nodes[idx].parent {
			Some(p) => idx = p,
			None => break,
		}
	}
}

fn rollout(game: &mut Game, rng: &mut impl Rng, root: Player, depth: u8) -> f64 {
	for _ in 0..depth {
		if game.status().is_over() {
			break;
		}
		let legal = game.legal_actions();
		if legal.is_empty() {
			break;
		}
		let action = legal[rng.gen_range(0..legal.len())];
		if game.apply(action).is_err() {
			break;
		}
	}
	if let Some(term) = terminal_score(game, root) {
		return term as f64 / WIN as f64;
	}
	evaluate(game, root) as f64 / 400.0
}

fn terminal_score(game: &Game, me: Player) -> Option<i32> {
	match game.status() {
		GameStatus::InProgress => None,
		GameStatus::Won { winner, .. } => {
			if winner == me {
				Some(WIN - game.turn_count() as i32)
			} else {
				Some(-WIN + game.turn_count() as i32)
			}
		}
		GameStatus::Draw { reason: EndReason::Elimination } => Some(0),
		GameStatus::Draw { .. } => Some(0),
	}
}

/// Material, tiles at risk, mobility. Positive is good for `me`.
pub fn evaluate(game: &Game, me: Player) -> i32 {
	if let Some(term) = terminal_score(game, me) {
		return term;
	}
	let opp = me.other();
	let mut material = 0i32;
	let mut at_risk = 0i32;
	for (_node, tile) in game.tiles() {
		let value = 12 + 8 * i32::from(tile.weight) + 2 * i32::from(tile.dots);
		let sign = if tile.owner == me { 1 } else { -1 };
		material += sign * value;
		if let Some((m, n)) = game.tile_score(_node) {
			if m - n <= 1 {
				at_risk += sign;
			}
		}
	}
	let bench = 18 * (game.bench(me).len() as i32 - game.bench(opp).len() as i32);
	let bag = 4 * (game.bag_len(me) as i32 - game.bag_len(opp) as i32);
	let board_presence = 25 * (game.board_count(me) as i32 - game.board_count(opp) as i32);
	let mut g = game.clone();
	g.set_current(me);
	let my_mob = g.legal_actions().len() as i32;
	g.set_current(opp);
	let opp_mob = g.legal_actions().len() as i32;
	material + bench + bag + board_presence - 20 * at_risk + 2 * (my_mob - opp_mob)
}

#[cfg(test)]
mod tests {
	#![allow(clippy::unwrap_used, clippy::expect_used)]

	use super::*;
	use crate::game::RuleSet;
	use rand::rngs::StdRng;
	use rand::SeedableRng;
	use std::time::Duration;

	#[test]
	fn random_plays_a_legal_action() {
		let game = Game::new(RuleSet::default(), 3);
		let mut rng = StdRng::seed_from_u64(3);
		let action = choose(&game, Actor::Random, &mut rng, Duration::from_millis(1));
		let legal = game.legal_actions();
		assert!(legal.contains(&action));
	}

	#[test]
	fn greedy_prefers_something_legal() {
		let game = Game::new(RuleSet::default(), 4);
		let mut rng = StdRng::seed_from_u64(4);
		let action = choose(&game, Actor::Greedy, &mut rng, Duration::from_millis(1));
		assert!(game.legal_actions().contains(&action));
	}

	#[test]
	fn all_bots_can_play_a_short_game() {
		for actor in [Actor::Random, Actor::Greedy, Actor::Minimax, Actor::Mcts] {
			let mut game = Game::new(RuleSet::default(), 9);
			let mut rng = StdRng::seed_from_u64(9);
			for _ in 0..8 {
				if game.status().is_over() {
					break;
				}
				let action = choose(&game, actor, &mut rng, Duration::from_millis(8));
				assert!(game.apply(action).is_ok(), "{:?} failed on {:?}", actor, action);
			}
		}
	}

	#[test]
	fn actor_parse_rejects_unknown() {
		assert!(Actor::parse("ologbon").is_err());
		assert_eq!(Actor::parse("MINIMAX").unwrap(), Actor::Minimax);
	}

	#[test]
	fn evaluate_scores_lockout_and_center_hold_as_terminals() {
		let mut lock_rules = RuleSet::default();
		lock_rules.no_moves_loses = true;
		let mut locked = crate::game::Game::empty(lock_rules);
		for id in 0..locked.board().node_count() as u8 {
			let owner = if id % 2 == 0 { Player::One } else { Player::Two };
			let tile = locked.alloc_tile(owner, crate::game::TileColor::Green, 3, 1);
			locked.place_tile(id, tile);
		}
		locked.set_current(Player::One);
		locked.apply(crate::game::Action::Pass).unwrap();
		assert!(matches!(
			locked.status(),
			GameStatus::Won { winner: Player::Two, reason: EndReason::LockedOut }
		));
		assert!(evaluate(&locked, Player::Two) >= WIN - 50);
		assert!(evaluate(&locked, Player::One) <= -WIN + 50);

		let mut hold_rules = RuleSet::default();
		hold_rules.center_hold_win = true;
		hold_rules.center_hold_n = 1;
		let mut hold = crate::game::Game::empty(hold_rules);
		let center = hold.board().center();
		let inner = {
			let b = hold.board();
			let r = b.construction_radius();
			let target = 2.0 * r * (3.0 * std::f64::consts::PI / 8.0).cos();
			(0..b.node_count() as u8)
				.find(|&id| (b.radius_of(id) - target).abs() < 0.05 * r)
				.expect("inner")
		};
		let t = hold.alloc_tile(Player::One, crate::game::TileColor::Blue, 3, 1);
		hold.place_tile(inner, t);
		{
			let extra = hold.alloc_tile(Player::Two, crate::game::TileColor::Orange, 3, 1);
			hold.set_bench(Player::Two, vec![extra]);
		}
		hold.set_current(Player::One);
		hold.apply(crate::game::Action::Move { from: inner, to: center }).unwrap();
		assert!(matches!(
			hold.status(),
			GameStatus::Won { winner: Player::One, reason: EndReason::CenterHold }
		));
		assert!(evaluate(&hold, Player::One) >= WIN - 50);
		assert!(evaluate(&hold, Player::Two) <= -WIN + 50);
	}

	#[test]
	fn greedy_prefers_immediate_center_hold_win() {
		let mut rules = RuleSet::default();
		rules.center_hold_win = true;
		rules.center_hold_n = 1;
		let mut game = crate::game::Game::empty(rules);
		let center = game.board().center();
		let inner = {
			let b = game.board();
			let r = b.construction_radius();
			let target = 2.0 * r * (3.0 * std::f64::consts::PI / 8.0).cos();
			(0..b.node_count() as u8)
				.find(|&id| (b.radius_of(id) - target).abs() < 0.05 * r)
				.expect("inner")
		};
		let t = game.alloc_tile(Player::One, crate::game::TileColor::Blue, 3, 1);
		game.place_tile(inner, t);
		{
			let extra = game.alloc_tile(Player::One, crate::game::TileColor::Green, 3, 1);
			game.set_bench(Player::One, vec![extra]);
		}
		{
			let extra = game.alloc_tile(Player::Two, crate::game::TileColor::Orange, 3, 1);
			game.set_bench(Player::Two, vec![extra]);
		}
		game.set_current(Player::One);
		let mut rng = StdRng::seed_from_u64(1);
		let action = choose(&game, Actor::Greedy, &mut rng, Duration::from_millis(1));
		assert_eq!(action, crate::game::Action::Move { from: inner, to: center });
	}
}
