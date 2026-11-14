//! Compact per-game benchmark logs: JSONL records that replay a game exactly.

use serde::{Deserialize, Serialize};

use crate::game::{
	Action, ActionOutcome, BraidFailMismatch, BudgetToken, EndReason, Game, GameStatus,
	HopWeightCmp, InsufficientTilesLoss, LowValueResolution, PassThroughRingCmp, PathConstraint,
	Player, RingType, RuleSet, SamePieceTwice, Tile, TileColor, ZagMismatchCount,
};

/// XOR mixed into the game seed for the searcher's `StdRng` in `apa-kan-bench`.
pub const SEARCH_RNG_XOR: u64 = 0x51ed;

/// JSONL schema version written by this module.
pub const SCHEMA_VERSION: u32 = 1;

pub fn search_rng_seed(game_seed: u64) -> u64 {
	game_seed ^ SEARCH_RNG_XOR
}

fn skip_zero_u8(v: &u8) -> bool {
	*v == 0
}

fn skip_zero_usize(v: &usize) -> bool {
	*v == 0
}

fn player_num(p: Player) -> u8 {
	match p {
		Player::One => 1,
		Player::Two => 2,
	}
}

fn player_from_num(n: u8) -> Result<Player, String> {
	match n {
		1 => Ok(Player::One),
		2 => Ok(Player::Two),
		other => Err(format!("unknown player {other}")),
	}
}

fn end_reason_name(reason: EndReason) -> &'static str {
	match reason {
		EndReason::Elimination => "elimination",
		EndReason::TurnLimit => "turn_limit",
		EndReason::DoublePass => "double_pass",
		EndReason::LockedOut => "locked_out",
		EndReason::CenterHold => "center_hold",
		EndReason::PetalMajority => "petal_majority",
		EndReason::Control => "control",
		EndReason::InsufficientTiles => "insufficient_tiles",
		EndReason::LowValue => "low_value",
	}
}

fn end_reason_parse(s: &str) -> Result<EndReason, String> {
	match s {
		"elimination" => Ok(EndReason::Elimination),
		"turn_limit" => Ok(EndReason::TurnLimit),
		"double_pass" => Ok(EndReason::DoublePass),
		"locked_out" => Ok(EndReason::LockedOut),
		"center_hold" => Ok(EndReason::CenterHold),
		"petal_majority" => Ok(EndReason::PetalMajority),
		"control" => Ok(EndReason::Control),
		"insufficient_tiles" => Ok(EndReason::InsufficientTiles),
		"low_value" => Ok(EndReason::LowValue),
		other => Err(format!("unknown end reason '{other}'")),
	}
}

/// One ring on a logged tile: color tag (`g`/`b`/`o`/`p`/`t`/`m`) and type flags.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct RingLog {
	pub c: String,
	#[serde(default, skip_serializing_if = "std::ops::Not::not")]
	pub z: bool,
	#[serde(default, skip_serializing_if = "std::ops::Not::not")]
	pub b: bool,
}

/// Full tile identity: ids, owner, dots, weight (= ring-slot count), rings.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct TileLog {
	pub id: u32,
	pub owner: u8,
	pub dots: u8,
	pub weight: u8,
	pub n_rings: u8,
	pub normal_rings: u8,
	pub zag_rings: u8,
	#[serde(default, skip_serializing_if = "skip_zero_u8")]
	pub braid_rings: u8,
	pub rings: Vec<RingLog>,
}

impl TileLog {
	pub fn from_tile(tile: Tile) -> Self {
		let rings = tile
			.ring_colors()
			.iter()
			.enumerate()
			.map(|(i, c)| RingLog {
				c: c.tag().to_string(),
				z: tile.ring_is_zag(i),
				b: tile.ring_is_braid(i),
			})
			.collect();
		Self {
			id: tile.id,
			owner: player_num(tile.owner),
			dots: tile.dots,
			weight: tile.weight,
			n_rings: tile.n_rings,
			normal_rings: tile.normal_ring_count(),
			zag_rings: tile.zag_ring_count(),
			braid_rings: tile.braid_ring_count(),
			rings,
		}
	}

	pub fn to_tile(&self) -> Result<Tile, String> {
		let owner = player_from_num(self.owner)?;
		let parts: Result<Vec<(TileColor, RingType)>, String> = self
			.rings
			.iter()
			.map(|r| {
				let typ = if r.b {
					RingType::Braid
				} else if r.z {
					RingType::Zag
				} else {
					RingType::Normal
				};
				Ok((TileColor::parse(&r.c)?, typ))
			})
			.collect();
		let mut tile = Tile::with_typed_rings(self.id, owner, &parts?, self.n_rings, self.dots);
		tile.id = self.id;
		debug_assert_eq!(tile.weight, tile.n_rings);
		Ok(tile)
	}
}

fn tiles_from_log(tiles: &[TileLog]) -> Result<Vec<Tile>, String> {
	tiles.iter().map(TileLog::to_tile).collect()
}

fn tiles_to_log(tiles: &[Tile]) -> Vec<TileLog> {
	tiles.iter().copied().map(TileLog::from_tile).collect()
}

/// Piece identity on an action: dots plus normal/zag/braid ring counts.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct PieceRef {
	pub id: u32,
	pub dots: u8,
	pub n_rings: u8,
	pub normal_rings: u8,
	pub zag_rings: u8,
	#[serde(default, skip_serializing_if = "skip_zero_u8")]
	pub braid_rings: u8,
}

impl PieceRef {
	pub fn from_tile(tile: Tile) -> Self {
		Self {
			id: tile.id,
			dots: tile.dots,
			n_rings: tile.n_rings,
			normal_rings: tile.normal_ring_count(),
			zag_rings: tile.zag_ring_count(),
			braid_rings: tile.braid_ring_count(),
		}
	}
}

/// All [`RuleSet`] flags, using the same names as the summary log / CLI.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct RulesLog {
	pub starting_bench: u8,
	pub bag_size: u8,
	pub self_capture_budget: bool,
	pub identical_bags: bool,
	pub bag_one_of_each_kind: bool,
	pub max_rings: u8,
	pub zag_numer: u8,
	#[serde(default)]
	pub braid_numer: u8,
	pub capture_draw_on_move: bool,
	pub mismatch_uses_neighbor_weight: bool,
	pub actions_per_turn: u8,
	pub turn_limit: u32,
	pub add_only_on_spawns: bool,
	pub move_exact_steps: bool,
	pub hop_requires_ge_weight: bool,
	pub no_moves_loses: bool,
	pub center_hold_win: bool,
	pub center_hold_n: u8,
	pub no_same_piece_twice: String,
	pub color_restricted_hops: bool,
	pub hop_weight_cmp: String,
	pub petal_majority_win: bool,
	pub petal_majority_n: u8,
	pub control_win: bool,
	pub control_n: u8,
	pub control_hold_turns: u8,
	pub insufficient_tiles_loss: String,
	pub path_constraint: String,
	pub counted_pass_through: bool,
	pub pass_through_ring_cmp: String,
	pub zag_mismatch: String,
	/// Absent on logs from before independent channels. Replay of those
	/// logs is skipped because capture outcomes used the old overrides.
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub braid_fail_mismatch: Option<String>,
	/// Present (`true`) once scoring walks generating-circle arcs, so the
	/// two inner↔center petals each count. Older independent-channel logs
	/// omit it (unique-neighbor scoring); full replay of those is skipped.
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub score_incident_arcs: Option<bool>,
}

impl RulesLog {
	pub fn from_rules(r: RuleSet) -> Self {
		Self {
			starting_bench: r.starting_bench_size,
			bag_size: r.bag_size,
			self_capture_budget: r.self_capture_budget,
			identical_bags: r.identical_bags,
			bag_one_of_each_kind: r.bag_one_of_each_kind,
			max_rings: r.max_rings,
			zag_numer: r.zag_numer,
			braid_numer: r.braid_numer,
			capture_draw_on_move: r.capture_draw_on_move,
			mismatch_uses_neighbor_weight: r.mismatch_uses_neighbor_weight,
			actions_per_turn: r.actions_per_turn,
			turn_limit: r.turn_limit,
			add_only_on_spawns: r.add_only_on_spawns,
			move_exact_steps: r.move_exact_steps,
			hop_requires_ge_weight: r.hop_requires_ge_weight,
			no_moves_loses: r.no_moves_loses,
			center_hold_win: r.center_hold_win,
			center_hold_n: r.center_hold_n,
			no_same_piece_twice: r.no_same_piece_twice.name().into(),
			color_restricted_hops: r.color_restricted_hops,
			hop_weight_cmp: r.hop_weight_cmp.name().into(),
			petal_majority_win: r.petal_majority_win,
			petal_majority_n: r.petal_majority_n,
			control_win: r.control_win,
			control_n: r.control_n,
			control_hold_turns: r.control_hold_turns,
			insufficient_tiles_loss: r.insufficient_tiles_loss.name().into(),
			path_constraint: r.path_constraint.name().into(),
			counted_pass_through: r.counted_pass_through,
			pass_through_ring_cmp: r.pass_through_ring_cmp.name().into(),
			zag_mismatch: r.zag_mismatch.name().into(),
			braid_fail_mismatch: Some(r.braid_fail_mismatch.name().into()),
			score_incident_arcs: Some(true),
		}
	}

	pub fn to_rules(&self) -> Result<RuleSet, String> {
		Ok(RuleSet {
			starting_bench_size: self.starting_bench,
			bag_size: self.bag_size,
			self_capture_budget: self.self_capture_budget,
			identical_bags: self.identical_bags,
			bag_one_of_each_kind: self.bag_one_of_each_kind,
			max_rings: self.max_rings,
			zag_numer: self.zag_numer,
			braid_numer: self.braid_numer,
			capture_draw_on_move: self.capture_draw_on_move,
			mismatch_uses_neighbor_weight: self.mismatch_uses_neighbor_weight,
			actions_per_turn: self.actions_per_turn,
			turn_limit: self.turn_limit,
			add_only_on_spawns: self.add_only_on_spawns,
			move_exact_steps: self.move_exact_steps,
			hop_requires_ge_weight: self.hop_requires_ge_weight,
			no_moves_loses: self.no_moves_loses,
			center_hold_win: self.center_hold_win,
			center_hold_n: self.center_hold_n,
			no_same_piece_twice: SamePieceTwice::parse(&self.no_same_piece_twice)?,
			color_restricted_hops: self.color_restricted_hops,
			hop_weight_cmp: HopWeightCmp::parse(&self.hop_weight_cmp)?,
			petal_majority_win: self.petal_majority_win,
			petal_majority_n: self.petal_majority_n,
			control_win: self.control_win,
			control_n: self.control_n,
			control_hold_turns: self.control_hold_turns,
			insufficient_tiles_loss: InsufficientTilesLoss::parse(&self.insufficient_tiles_loss)?,
			path_constraint: PathConstraint::parse(&self.path_constraint)?,
			counted_pass_through: self.counted_pass_through,
			pass_through_ring_cmp: PassThroughRingCmp::parse(&self.pass_through_ring_cmp)?,
			zag_mismatch: ZagMismatchCount::parse(&self.zag_mismatch)?,
			braid_fail_mismatch: match &self.braid_fail_mismatch {
				Some(s) => BraidFailMismatch::parse(s)?,
				None => BraidFailMismatch::One,
			},
		})
	}
}

/// Opening deal: the public remaining deck (`p*_bench`), leftover hidden bag
/// (empty on new games; kept so historical JSONL replays), and spawn tokens.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct OpeningLog {
	pub p1_bench: Vec<TileLog>,
	pub p2_bench: Vec<TileLog>,
	pub p1_bag: Vec<TileLog>,
	pub p2_bag: Vec<TileLog>,
	pub p1_tokens: usize,
	pub p2_tokens: usize,
}

impl OpeningLog {
	pub fn from_game(game: &Game) -> Self {
		Self {
			p1_bench: tiles_to_log(game.bench(Player::One)),
			p2_bench: tiles_to_log(game.bench(Player::Two)),
			p1_bag: tiles_to_log(game.bag(Player::One)),
			p2_bag: tiles_to_log(game.bag(Player::Two)),
			p1_tokens: game.budget_token_count(Player::One),
			p2_tokens: game.budget_token_count(Player::Two),
		}
	}
}

/// One applied action. `kind` is ADD/MOVE/PASS; capture flags are outcomes.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct ActionLog {
	pub turn: u32,
	pub player: u8,
	pub kind: String,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub from: Option<u8>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub to: Option<u8>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub bench_index: Option<usize>,
	#[serde(default, skip_serializing_if = "skip_zero_u8")]
	pub capture: u8,
	#[serde(default, skip_serializing_if = "skip_zero_u8")]
	pub self_capture: u8,
	#[serde(default, skip_serializing_if = "skip_zero_u8")]
	pub draws: u8,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub piece: Option<PieceRef>,
	pub tokens: usize,
	pub budget: usize,
	#[serde(default, skip_serializing_if = "skip_zero_usize")]
	pub surplus: usize,
}

impl ActionLog {
	pub fn record(
		action: Action,
		piece: Option<Tile>,
		outcome: &ActionOutcome,
		game_after: &Game,
	) -> Self {
		let player = outcome.player;
		let (kind, from, to, bench_index) = match action {
			Action::Add { bench_index, node } => {
				("ADD".to_string(), None, Some(node), Some(bench_index))
			}
			Action::Move { from, to } => ("MOVE".to_string(), Some(from), Some(to), None),
			Action::Pass => ("PASS".to_string(), None, None, None),
		};
		let capture = outcome.removals.iter().filter(|r| r.tile.owner != player).count() as u8;
		let self_capture = outcome.removals.iter().filter(|r| r.tile.owner == player).count() as u8;
		Self {
			turn: game_after.turn_count(),
			player: player_num(player),
			kind,
			from,
			to,
			bench_index,
			capture,
			self_capture,
			draws: outcome.draws,
			piece: piece.map(PieceRef::from_tile),
			tokens: game_after.budget_token_count(player),
			budget: game_after.budget(player),
			surplus: game_after.budget_surplus(player),
		}
	}

	pub fn to_action(&self) -> Result<Action, String> {
		match self.kind.as_str() {
			"ADD" => {
				let bench_index = self.bench_index.ok_or("ADD missing bench_index")?;
				let node = self.to.ok_or("ADD missing to")?;
				Ok(Action::Add { bench_index, node })
			}
			"MOVE" => {
				let from = self.from.ok_or("MOVE missing from")?;
				let to = self.to.ok_or("MOVE missing to")?;
				Ok(Action::Move { from, to })
			}
			"PASS" => Ok(Action::Pass),
			other => Err(format!("unknown action kind '{other}'")),
		}
	}

	pub fn tags(&self) -> Vec<&'static str> {
		let mut tags = Vec::new();
		match self.kind.as_str() {
			"ADD" => tags.push("ADD"),
			"MOVE" => tags.push("MOVE"),
			"PASS" => tags.push("PASS"),
			_ => {}
		}
		if self.capture > 0 {
			tags.push("capture");
		}
		if self.self_capture > 0 {
			tags.push("self-capture");
		}
		tags
	}
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct LowValueLog {
	pub kind: String,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub winner: Option<u8>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub pair: Option<usize>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub by_piece_count: Option<bool>,
}

impl LowValueLog {
	pub fn from_resolution(res: LowValueResolution) -> Self {
		match res {
			LowValueResolution::Draw => {
				Self { kind: "draw".into(), winner: None, pair: None, by_piece_count: None }
			}
			LowValueResolution::Win { winner, pair, by_piece_count } => Self {
				kind: "win".into(),
				winner: Some(player_num(winner)),
				pair: Some(pair),
				by_piece_count: Some(by_piece_count),
			},
		}
	}
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct EndLog {
	pub status: String,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub reason: Option<String>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub winner: Option<u8>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub low_value: Option<LowValueLog>,
}

impl EndLog {
	pub fn from_status(status: GameStatus, low_value: Option<LowValueResolution>) -> Self {
		let low_value = low_value.map(LowValueLog::from_resolution);
		match status {
			GameStatus::InProgress => {
				Self { status: "in_progress".into(), reason: None, winner: None, low_value }
			}
			GameStatus::Won { winner, reason } => Self {
				status: "won".into(),
				reason: Some(end_reason_name(reason).into()),
				winner: Some(player_num(winner)),
				low_value,
			},
			GameStatus::Draw { reason } => Self {
				status: "draw".into(),
				reason: Some(end_reason_name(reason).into()),
				winner: None,
				low_value,
			},
		}
	}

	pub fn to_status(&self) -> Result<GameStatus, String> {
		match self.status.as_str() {
			"in_progress" => Ok(GameStatus::InProgress),
			"won" => {
				let winner = player_from_num(self.winner.ok_or("won missing winner")?)?;
				let reason = end_reason_parse(self.reason.as_deref().ok_or("won missing reason")?)?;
				Ok(GameStatus::Won { winner, reason })
			}
			"draw" => {
				let reason =
					end_reason_parse(self.reason.as_deref().ok_or("draw missing reason")?)?;
				Ok(GameStatus::Draw { reason })
			}
			other => Err(format!("unknown status '{other}'")),
		}
	}
}

/// Optional extra snapshot: board / bench / token stack at the end.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct BoardPieceLog {
	pub node: u8,
	pub id: u32,
	pub dots: u8,
	pub n_rings: u8,
	pub normal_rings: u8,
	pub zag_rings: u8,
	#[serde(default, skip_serializing_if = "skip_zero_u8")]
	pub braid_rings: u8,
}

impl BoardPieceLog {
	fn from_placed(node: u8, tile: Tile) -> Self {
		Self {
			node,
			id: tile.id,
			dots: tile.dots,
			n_rings: tile.n_rings,
			normal_rings: tile.normal_ring_count(),
			zag_rings: tile.zag_ring_count(),
			braid_rings: tile.braid_ring_count(),
		}
	}
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct TokenStackLog {
	pub tokens: usize,
	pub budget: usize,
	pub surplus: usize,
}

impl TokenStackLog {
	fn from_game(game: &Game, player: Player) -> Self {
		Self {
			tokens: game.budget_token_count(player),
			budget: game.budget(player),
			surplus: game.budget_surplus(player),
		}
	}
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct FinalLog {
	pub p1_board: Vec<BoardPieceLog>,
	pub p2_board: Vec<BoardPieceLog>,
	pub p1_bench: Vec<TileLog>,
	pub p2_bench: Vec<TileLog>,
	pub p1_tokens: TokenStackLog,
	pub p2_tokens: TokenStackLog,
}

impl FinalLog {
	pub fn from_game(game: &Game) -> Self {
		let mut p1_board = Vec::new();
		let mut p2_board = Vec::new();
		for (node, tile) in game.tiles() {
			let piece = BoardPieceLog::from_placed(node, tile);
			if tile.owner == Player::One {
				p1_board.push(piece);
			} else {
				p2_board.push(piece);
			}
		}
		Self {
			p1_board,
			p2_board,
			p1_bench: tiles_to_log(game.bench(Player::One)),
			p2_bench: tiles_to_log(game.bench(Player::Two)),
			p1_tokens: TokenStackLog::from_game(game, Player::One),
			p2_tokens: TokenStackLog::from_game(game, Player::Two),
		}
	}
}

/// One JSONL record: everything needed to reconstruct a game from the log alone.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct GameLog {
	pub v: u32,
	pub game: u32,
	pub pairing: String,
	pub p1: String,
	pub p2: String,
	pub seed: u64,
	pub search_rng_seed: u64,
	pub think_ms: u64,
	pub scenario: String,
	pub rules: RulesLog,
	pub opening: OpeningLog,
	pub turns: u32,
	pub end: EndLog,
	pub actions: Vec<ActionLog>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub final_state: Option<FinalLog>,
}

impl GameLog {
	pub fn begin(
		game: u32,
		pairing: impl Into<String>,
		p1: impl Into<String>,
		p2: impl Into<String>,
		think_ms: u64,
		scenario: impl Into<String>,
		live: &Game,
	) -> Self {
		Self {
			v: SCHEMA_VERSION,
			game,
			pairing: pairing.into(),
			p1: p1.into(),
			p2: p2.into(),
			seed: live.seed(),
			search_rng_seed: search_rng_seed(live.seed()),
			think_ms,
			scenario: scenario.into(),
			rules: RulesLog::from_rules(*live.rules()),
			opening: OpeningLog::from_game(live),
			turns: 0,
			end: EndLog::from_status(GameStatus::InProgress, None),
			actions: Vec::new(),
			final_state: None,
		}
	}

	pub fn push_action(
		&mut self,
		action: Action,
		piece: Option<Tile>,
		outcome: &ActionOutcome,
		game_after: &Game,
	) {
		self.actions.push(ActionLog::record(action, piece, outcome, game_after));
	}

	pub fn finish(&mut self, live: &Game, low_value: Option<LowValueResolution>) {
		self.turns = live.turn_count();
		self.end = EndLog::from_status(live.status(), low_value);
		self.final_state = Some(FinalLog::from_game(live));
	}
}

pub fn low_value_of(game: &Game) -> Option<LowValueResolution> {
	match game.status() {
		GameStatus::Won { reason: EndReason::LowValue, .. }
		| GameStatus::Draw { reason: EndReason::LowValue } => Some(game.low_value_resolution()),
		_ => None,
	}
}

/// Rebuild a game from a log's opening deal, bags, rules, and action list.
pub fn replay(log: &GameLog) -> Result<Game, String> {
	if log.v != SCHEMA_VERSION {
		return Err(format!("unsupported log schema v={}", log.v));
	}
	let rules = log.rules.to_rules()?;
	let mut game = Game::empty(rules);
	game.set_bag(Player::One, tiles_from_log(&log.opening.p1_bag)?);
	game.set_bag(Player::Two, tiles_from_log(&log.opening.p2_bag)?);
	game.set_bench(Player::One, tiles_from_log(&log.opening.p1_bench)?);
	game.set_bench(Player::Two, tiles_from_log(&log.opening.p2_bench)?);
	game.set_budget_tokens(Player::One, vec![BudgetToken::Blank; log.opening.p1_tokens]);
	game.set_budget_tokens(Player::Two, vec![BudgetToken::Blank; log.opening.p2_tokens]);
	game.set_current(Player::One);
	for (i, act) in log.actions.iter().enumerate() {
		let action = act.to_action().map_err(|e| format!("action {i}: {e}"))?;
		game.apply(action).map_err(|e| format!("action {i} apply: {e}"))?;
	}
	Ok(game)
}

/// Compare terminal physics: status, turns, board, benches, bags, token stacks.
pub fn assert_same_terminal(live: &Game, replayed: &Game) -> Result<(), String> {
	if live.status() != replayed.status() {
		return Err(format!("status {:?} vs {:?}", live.status(), replayed.status()));
	}
	if live.turn_count() != replayed.turn_count() {
		return Err(format!("turns {} vs {}", live.turn_count(), replayed.turn_count()));
	}
	let live_tiles: Vec<_> = live.tiles().collect();
	let replay_tiles: Vec<_> = replayed.tiles().collect();
	if live_tiles != replay_tiles {
		return Err(format!("board {live_tiles:?} vs {replay_tiles:?}"));
	}
	for p in [Player::One, Player::Two] {
		if live.bench(p) != replayed.bench(p) {
			return Err(format!("{} bench mismatch", p.label()));
		}
		if live.bag(p) != replayed.bag(p) {
			return Err(format!("{} bag mismatch", p.label()));
		}
		if live.budget(p) != replayed.budget(p)
			|| live.budget_token_count(p) != replayed.budget_token_count(p)
			|| live.budget_surplus(p) != replayed.budget_surplus(p)
		{
			return Err(format!(
				"{} tokens live={{n={} budget={} surplus={}}} replay={{n={} budget={} surplus={}}}",
				p.label(),
				live.budget_token_count(p),
				live.budget(p),
				live.budget_surplus(p),
				replayed.budget_token_count(p),
				replayed.budget(p),
				replayed.budget_surplus(p)
			));
		}
		if live.budget_tokens(p) != replayed.budget_tokens(p) {
			return Err(format!("{} token-stack identity mismatch", p.label()));
		}
	}
	Ok(())
}

pub fn encode_jsonl(logs: &[GameLog]) -> Result<String, String> {
	let mut out = String::new();
	for log in logs {
		let line = serde_json::to_string(log).map_err(|e| e.to_string())?;
		out.push_str(&line);
		out.push('\n');
	}
	Ok(out)
}

pub fn decode_jsonl(text: &str) -> Result<Vec<GameLog>, String> {
	let mut out = Vec::new();
	for (i, line) in text.lines().enumerate() {
		let line = line.trim();
		if line.is_empty() {
			continue;
		}
		let log: GameLog =
			serde_json::from_str(line).map_err(|e| format!("jsonl line {}: {e}", i + 1))?;
		out.push(log);
	}
	Ok(out)
}

pub fn decode_jsonl_gz(bytes: &[u8]) -> Result<Vec<GameLog>, String> {
	use std::io::Read;
	let mut dec = flate2::read::GzDecoder::new(bytes);
	let mut text = String::new();
	dec.read_to_string(&mut text).map_err(|e| e.to_string())?;
	decode_jsonl(&text)
}

/// True when `path` should be gzip-compressed (`.gz` / `.jsonl.gz`).
pub fn path_is_gzip(path: &std::path::Path) -> bool {
	path.extension().is_some_and(|ext| ext == "gz")
}

/// Sibling of a summary `.txt`: `foo.txt` → `foo.jsonl.gz`.
pub fn default_games_log_path(summary: &std::path::Path) -> std::path::PathBuf {
	summary.with_extension("jsonl.gz")
}

/// Piece looked up *before* `apply`, so ADD still sees the bench slot.
pub fn piece_for_action(game: &Game, action: Action) -> Option<Tile> {
	match action {
		Action::Add { bench_index, .. } => {
			game.bench(game.current_player()).get(bench_index).copied()
		}
		Action::Move { from, .. } => game.tile_at(from),
		Action::Pass => None,
	}
}

#[cfg(test)]
mod tests {
	#![allow(clippy::unwrap_used, clippy::expect_used)]

	use super::*;
	use crate::ai::{Actor, Searcher};
	use rand::rngs::StdRng;
	use rand::SeedableRng;
	use std::time::Duration;

	fn play_to_log(
		rules: RuleSet,
		seed: u64,
		p1: Actor,
		p2: Actor,
		think: Duration,
		max_turns: u32,
	) -> (Game, GameLog) {
		let mut game = Game::new(rules, seed);
		let mut log = GameLog::begin(
			1,
			format!("{} vs {}", p1.name(), p2.name()),
			p1.name(),
			p2.name(),
			think.as_millis() as u64,
			"default",
			&game,
		);
		let mut rng = StdRng::seed_from_u64(search_rng_seed(seed));
		let mut s1 = Searcher::new();
		let mut s2 = Searcher::new();
		let mut guard = 0u32;
		loop {
			if game.status().is_over() {
				break;
			}
			guard += 1;
			if guard > max_turns {
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
			let action = searcher.choose(&game, actor, &mut rng, think);
			let piece = piece_for_action(&game, action);
			match game.apply(action) {
				Ok(outcome) => log.push_action(action, piece, &outcome, &game),
				Err(_) => {
					let fallback = game.legal_actions().into_iter().next().unwrap_or(Action::Pass);
					let piece = piece_for_action(&game, fallback);
					if let Ok(outcome) = game.apply(fallback) {
						log.push_action(fallback, piece, &outcome, &game);
					}
					break;
				}
			}
			s1.advance(&game);
			s2.advance(&game);
		}
		log.finish(&game, low_value_of(&game));
		(game, log)
	}

	fn required_keys() -> &'static [&'static str] {
		&[
			"v",
			"game",
			"pairing",
			"p1",
			"p2",
			"seed",
			"search_rng_seed",
			"think_ms",
			"scenario",
			"rules",
			"opening",
			"turns",
			"end",
			"actions",
		]
	}

	#[test]
	fn jsonl_roundtrip_keeps_required_fields() {
		let mut rules = RuleSet::default();
		rules.bag_size = 11;
		rules.turn_limit = 24;
		let (_live, log) =
			play_to_log(rules, 7, Actor::Random, Actor::Greedy, Duration::from_millis(1), 24);
		let jsonl = encode_jsonl(std::slice::from_ref(&log)).unwrap();
		assert_eq!(jsonl.lines().count(), 1);
		for key in required_keys() {
			assert!(jsonl.contains(&format!("\"{key}\"")), "missing {key} in {jsonl}");
		}
		let parsed = decode_jsonl(&jsonl).unwrap();
		assert_eq!(parsed.len(), 1);
		assert_eq!(parsed[0], log);
		assert_eq!(parsed[0].v, SCHEMA_VERSION);
		assert_eq!(parsed[0].rules.bag_size, 11);
		assert!(!parsed[0].rules.capture_draw_on_move);
		assert!(!parsed[0].opening.p1_bench.is_empty());
		assert_eq!(parsed[0].opening.p1_bench.len(), parsed[0].opening.p2_bench.len());
		assert!(parsed[0].opening.p1_bag.is_empty());
		assert!(parsed[0].opening.p2_bag.is_empty());
		assert!(!parsed[0].actions.is_empty() || parsed[0].turns == 0);
	}

	#[test]
	fn action_kinds_are_add_move_pass_with_capture_flags() {
		let mut rules = RuleSet::default();
		rules.turn_limit = 80;
		let (_live, log) =
			play_to_log(rules, 3, Actor::Greedy, Actor::Greedy, Duration::from_millis(1), 80);
		assert!(!log.actions.is_empty());
		for act in &log.actions {
			assert!(
				matches!(act.kind.as_str(), "ADD" | "MOVE" | "PASS"),
				"unexpected kind {}",
				act.kind
			);
			match act.kind.as_str() {
				"ADD" => {
					assert!(act.bench_index.is_some());
					assert!(act.to.is_some());
					assert!(act.piece.is_some());
				}
				"MOVE" => {
					assert!(act.from.is_some());
					assert!(act.to.is_some());
					assert!(act.piece.is_some());
					let piece = act.piece.as_ref().unwrap();
					assert_eq!(
						piece.n_rings,
						piece.normal_rings + piece.zag_rings + piece.braid_rings
					);
				}
				"PASS" => {
					assert!(act.from.is_none());
					assert!(act.to.is_none());
				}
				_ => unreachable!(),
			}
			let tags = act.tags();
			if act.capture > 0 {
				assert!(tags.contains(&"capture"));
			}
			if act.self_capture > 0 {
				assert!(tags.contains(&"self-capture"));
			}
		}
	}

	#[test]
	fn replay_from_log_matches_live_random_game() {
		let mut rules = RuleSet::default();
		rules.bag_size = 11;
		rules.turn_limit = 60;
		let (live, log) =
			play_to_log(rules, 11, Actor::Random, Actor::Random, Duration::from_millis(1), 60);
		let jsonl = encode_jsonl(std::slice::from_ref(&log)).unwrap();
		let parsed = decode_jsonl(&jsonl).unwrap();
		let replayed = replay(&parsed[0]).expect("replay");
		assert_same_terminal(&live, &replayed).unwrap();
		assert_eq!(replayed.status(), parsed[0].end.to_status().unwrap());
		assert_eq!(replayed.turn_count(), parsed[0].turns);
	}

	#[test]
	fn replay_from_log_matches_live_greedy_game() {
		let mut rules = RuleSet::default();
		rules.bag_size = 11;
		rules.self_capture_budget = true;
		rules.turn_limit = 80;
		let (live, log) =
			play_to_log(rules, 1, Actor::Greedy, Actor::Greedy, Duration::from_millis(1), 80);
		let replayed = replay(&log).expect("replay");
		assert_same_terminal(&live, &replayed).unwrap();
		if let Some(final_state) = &log.final_state {
			assert_eq!(final_state.p1_tokens.budget, live.budget(Player::One));
			assert_eq!(final_state.p2_tokens.surplus, live.budget_surplus(Player::Two));
			let p1_n = live.tiles().filter(|(_, t)| t.owner == Player::One).count();
			assert_eq!(final_state.p1_board.len(), p1_n);
		}
	}

	#[test]
	fn replay_scripted_first_legal_actions() {
		let mut rules = RuleSet::default();
		rules.turn_limit = 16;
		let mut live = Game::new(rules, 42);
		let mut log =
			GameLog::begin(3, "scripted vs scripted", "scripted", "scripted", 0, "default", &live);
		for _ in 0..12 {
			if live.status().is_over() {
				break;
			}
			let action = live.legal_actions().into_iter().next().unwrap_or(Action::Pass);
			let piece = piece_for_action(&live, action);
			let outcome = live.apply(action).unwrap();
			log.push_action(action, piece, &outcome, &live);
		}
		log.finish(&live, low_value_of(&live));
		let json = serde_json::to_string(&log).unwrap();
		assert!(!json.contains('\n'));
		let parsed: GameLog = serde_json::from_str(&json).unwrap();
		let replayed = replay(&parsed).unwrap();
		assert_same_terminal(&live, &replayed).unwrap();
		assert_eq!(parsed.seed, 42);
		assert_eq!(parsed.search_rng_seed, search_rng_seed(42));
		assert_eq!(parsed.rules.to_rules().unwrap(), rules);
	}

	#[test]
	fn low_value_end_fields_roundtrip() {
		let res = LowValueResolution::Win { winner: Player::Two, pair: 1, by_piece_count: true };
		let end = EndLog::from_status(
			GameStatus::Won { winner: Player::Two, reason: EndReason::LowValue },
			Some(res),
		);
		let json = serde_json::to_string(&end).unwrap();
		assert!(json.contains("\"low_value\""));
		assert!(json.contains("\"pair\":1"));
		assert!(json.contains("\"by_piece_count\":true"));
		let parsed: EndLog = serde_json::from_str(&json).unwrap();
		assert_eq!(parsed, end);
		assert_eq!(
			parsed.to_status().unwrap(),
			GameStatus::Won { winner: Player::Two, reason: EndReason::LowValue }
		);
		let lv = parsed.low_value.unwrap();
		assert_eq!(lv.kind, "win");
		assert_eq!(lv.winner, Some(2));
		assert_eq!(lv.pair, Some(1));
		assert_eq!(lv.by_piece_count, Some(true));
	}

	#[test]
	fn rules_log_roundtrips_every_flag() {
		let mut r = RuleSet::default();
		r.bag_size = 11;
		r.self_capture_budget = false;
		r.center_hold_win = true;
		r.petal_majority_win = true;
		r.zag_numer = 0;
		r.zag_mismatch = ZagMismatchCount::Max;
		r.pass_through_ring_cmp = PassThroughRingCmp::Gt;
		r.turn_limit = 50;
		r.braid_fail_mismatch = BraidFailMismatch::Sum;
		let log = RulesLog::from_rules(r);
		assert_eq!(log.to_rules().unwrap(), r);
	}

	fn opening_weight_equals_rings(log: &GameLog) -> bool {
		log.opening
			.p1_bench
			.iter()
			.chain(log.opening.p2_bench.iter())
			.chain(log.opening.p1_bag.iter())
			.chain(log.opening.p2_bag.iter())
			.all(|t| t.weight == t.n_rings)
	}

	fn assert_committed_jsonl_gz_replays(rel: &str) {
		let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(rel);
		if !path.exists() {
			return;
		}
		let bytes = std::fs::read(&path).unwrap();
		let logs = decode_jsonl_gz(&bytes).expect("decode gzip jsonl");
		assert_eq!(logs.len(), 108, "standard H2H is 9 pairings × 12 games");
		let derived_weight = logs.iter().all(opening_weight_equals_rings);
		let independent_channels = logs.iter().all(|log| log.rules.braid_fail_mismatch.is_some());
		let incident_arc_scoring =
			logs.iter().all(|log| log.rules.score_incident_arcs == Some(true));
		for (i, log) in logs.iter().enumerate() {
			assert_eq!(log.v, SCHEMA_VERSION);
			assert_eq!(log.rules.bag_size, 11, "{rel}");
			assert!(log.rules.self_capture_budget, "{rel}");
			assert_eq!(log.search_rng_seed, search_rng_seed(log.seed));
			assert!(!log.opening.p1_bench.is_empty());
			assert!(!log.actions.is_empty() || log.turns == 0);
			if !derived_weight || !independent_channels || !incident_arc_scoring {
				continue;
			}
			let replayed =
				replay(log).unwrap_or_else(|e| panic!("replay {rel} game {}: {e}", i + 1));
			assert_eq!(
				replayed.status(),
				log.end.to_status().unwrap(),
				"game {} {} g{}",
				i + 1,
				log.pairing,
				log.game
			);
			assert_eq!(replayed.turn_count(), log.turns);
			if let Some(final_state) = &log.final_state {
				assert_eq!(final_state.p1_tokens.budget, replayed.budget(Player::One));
				assert_eq!(final_state.p2_tokens.budget, replayed.budget(Player::Two));
				assert_eq!(final_state.p1_tokens.surplus, replayed.budget_surplus(Player::One));
				assert_eq!(final_state.p2_tokens.surplus, replayed.budget_surplus(Player::Two));
				let p1_n = replayed.tiles().filter(|(_, t)| t.owner == Player::One).count();
				let p2_n = replayed.tiles().filter(|(_, t)| t.owner == Player::Two).count();
				assert_eq!(final_state.p1_board.len(), p1_n);
				assert_eq!(final_state.p2_board.len(), p2_n);
			}
		}
	}

	#[test]
	fn committed_deck11_jsonl_gz_replays_all_games() {
		assert_committed_jsonl_gz_replays("benchmark_results/low_value_pairs_deck11_on.jsonl.gz");
	}

	#[test]
	fn committed_defaults_bag11_jsonl_gz_replays_all_games() {
		assert_committed_jsonl_gz_replays("benchmark_results/defaults_bag11.jsonl.gz");
	}

	#[test]
	fn committed_braid_rings_deck11_jsonl_gz_replays_all_games() {
		assert_committed_jsonl_gz_replays("benchmark_results/braid_rings_deck11.jsonl.gz");
	}

	#[test]
	fn committed_weight_from_rings_deck11_jsonl_gz_replays_all_games() {
		assert_committed_jsonl_gz_replays("benchmark_results/weight_from_rings_deck11.jsonl.gz");
	}

	#[test]
	fn committed_public_deck_tokens_jsonl_gz_replays_all_games() {
		assert_committed_jsonl_gz_replays("benchmark_results/public_deck_tokens.jsonl.gz");
	}

	#[test]
	fn committed_independent_channels_jsonl_gz_replays_all_games() {
		assert_committed_jsonl_gz_replays("benchmark_results/independent_channels.jsonl.gz");
	}

	#[test]
	fn committed_braid_fail_sum_jsonl_gz_replays_all_games() {
		assert_committed_jsonl_gz_replays("benchmark_results/braid_fail_sum.jsonl.gz");
	}

	#[test]
	fn committed_braid_fail_weight_jsonl_gz_replays_all_games() {
		assert_committed_jsonl_gz_replays("benchmark_results/braid_fail_weight.jsonl.gz");
	}

	#[test]
	fn committed_doubles_disallowed_jsonl_gz_replays_all_games() {
		assert_committed_jsonl_gz_replays("benchmark_results/doubles_disallowed.jsonl.gz");
	}

	#[test]
	fn committed_doubles_allowed_jsonl_gz_replays_all_games() {
		assert_committed_jsonl_gz_replays("benchmark_results/doubles_allowed.jsonl.gz");
	}

	#[test]
	fn default_games_log_path_is_jsonl_gz_sibling() {
		let p =
			std::path::Path::new("apa-kan/game/benchmark_results/low_value_pairs_deck11_on.txt");
		assert_eq!(
			default_games_log_path(p),
			std::path::PathBuf::from(
				"apa-kan/game/benchmark_results/low_value_pairs_deck11_on.jsonl.gz"
			)
		);
		assert!(path_is_gzip(&default_games_log_path(p)));
		assert!(!path_is_gzip(std::path::Path::new("out.jsonl")));
	}
}
