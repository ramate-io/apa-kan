//! Compact per-game benchmark logs: JSONL records that replay a game exactly.

use serde::{Deserialize, Serialize};

use crate::deck::{DeckArchetype, PickStrategy};
use crate::game::{
	color_drift_diagonal, color_drift_diagonal_value, color_drift_zero, Action, ActionOutcome,
	BeadScoring, BlockerBlocks, BraidFailMismatch, BraidScoring, BudgetToken, CaptureMark,
	CaptureMode, ChannelMode, ColorDrift, Emblem, EmblemWeights, EndReason, EndgameRule, Game,
	GameStatus, HopWeightCmp, InsufficientTilesLoss, LowValueResolution, PassThroughRingCmp,
	PathConstraint, Player, RingCountWeights, RingType, RuleSet, SamePieceTwice, SwapMode, Tile,
	TileColor, TurnGainKind, ZagMismatchCount, DOT_PERCENT_DEFAULT,
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

fn skip_zero_u16(v: &u16) -> bool {
	*v == 0
}

fn skip_zero_usize(v: &usize) -> bool {
	*v == 0
}

fn skip_zero_u32(v: &u32) -> bool {
	*v == 0
}

fn skip_zero_f32(v: &f32) -> bool {
	*v == 0.0
}

fn skip_zero_f64(v: &f64) -> bool {
	*v == 0.0
}

fn default_independent() -> String {
	BeadScoring::Independent.name().to_string()
}

fn skip_independent(v: &String) -> bool {
	v == BeadScoring::Independent.name()
}

fn default_current_braid() -> String {
	BraidScoring::Current.name().to_string()
}

fn skip_current_braid(v: &String) -> bool {
	v == BraidScoring::Current.name()
}

fn default_additive_channel_mode() -> String {
	ChannelMode::Additive.name().to_string()
}

fn skip_additive_channel_mode(v: &String) -> bool {
	v == ChannelMode::Additive.name()
}

fn default_cascade_capture_mode() -> String {
	CaptureMode::Cascade.name().to_string()
}

fn skip_cascade_capture_mode(v: &String) -> bool {
	v == CaptureMode::Cascade.name()
}

fn default_ring_count_weights() -> String {
	RingCountWeights::default().label()
}

fn skip_default_ring_count_weights(v: &String) -> bool {
	*v == RingCountWeights::default().label()
}

fn default_zero_color_drift() -> String {
	"0".into()
}

fn skip_zero_str(v: &String) -> bool {
	v.is_empty() || v == "0"
}

fn color_drift_label(drift: ColorDrift) -> String {
	color_drift_diagonal_value(drift).unwrap_or(0).to_string()
}

fn parse_color_drift(s: &str) -> ColorDrift {
	if s.is_empty() || s == "0" {
		return color_drift_zero();
	}
	s.parse::<i32>()
		.map(color_drift_diagonal)
		.unwrap_or_else(|_| color_drift_zero())
}

fn default_mixed() -> String {
	DeckArchetype::Mixed.name().to_string()
}

fn skip_mixed(v: &String) -> bool {
	v == DeckArchetype::Mixed.name()
}

fn default_triangle() -> String {
	CaptureMark::Triangle.name().to_string()
}

fn skip_triangle(v: &String) -> bool {
	v == "triangle"
}

fn default_none() -> String {
	Emblem::None.name().to_string()
}

fn skip_none(v: &String) -> bool {
	v == "none"
}

fn default_emblem_weights() -> String {
	EmblemWeights::default().label()
}

fn skip_default_emblem_weights(v: &String) -> bool {
	*v == EmblemWeights::default().label()
}

fn default_dot_percent() -> u8 {
	DOT_PERCENT_DEFAULT
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
	/// Bead ring. Absent/false on logs from before beads.
	#[serde(default, skip_serializing_if = "std::ops::Not::not")]
	pub e: bool,
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
	#[serde(default, skip_serializing_if = "skip_zero_u8")]
	pub bead_rings: u8,
	/// Capture mark. Absent on logs from before capture modifiers; those
	/// replay as triangle (spawn-token dest).
	#[serde(default = "default_triangle")]
	pub capture: String,
	/// Emblem slot. Absent on logs from before emblems; those replay as none.
	#[serde(default = "default_none", skip_serializing_if = "skip_none")]
	pub emblem: String,
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
				e: tile.ring_is_bead(i),
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
			bead_rings: tile.bead_ring_count(),
			capture: tile.capture.name().to_string(),
			emblem: tile.emblem.name().to_string(),
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
				} else if r.e {
					RingType::Bead
				} else {
					RingType::Normal
				};
				Ok((TileColor::parse(&r.c)?, typ))
			})
			.collect();
		let mut tile = Tile::with_typed_rings(self.id, owner, &parts?, self.n_rings, self.dots)
			.with_capture(CaptureMark::parse(&self.capture).unwrap_or(CaptureMark::Triangle))
			.with_emblem(Emblem::parse(&self.emblem).unwrap_or(Emblem::None));
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

/// Piece identity on an action: dots plus normal/zag/braid/bead ring counts.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct PieceRef {
	pub id: u32,
	pub dots: u8,
	pub n_rings: u8,
	pub normal_rings: u8,
	pub zag_rings: u8,
	#[serde(default, skip_serializing_if = "skip_zero_u8")]
	pub braid_rings: u8,
	#[serde(default, skip_serializing_if = "skip_zero_u8")]
	pub bead_rings: u8,
	#[serde(default = "default_triangle", skip_serializing_if = "skip_triangle")]
	pub capture: String,
	#[serde(default = "default_none", skip_serializing_if = "skip_none")]
	pub emblem: String,
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
			bead_rings: tile.bead_ring_count(),
			capture: tile.capture.name().to_string(),
			emblem: tile.emblem.name().to_string(),
		}
	}
}

/// All [`RuleSet`] flags, using the same names as the summary log / CLI.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct RulesLog {
	pub starting_bench: u8,
	pub bag_size: u8,
	/// Full collection size. Absent/0 on logs from before deck-building;
	/// replay then uses `bag_size`.
	#[serde(default, skip_serializing_if = "skip_zero_u8")]
	pub deck_size: u8,
	pub self_capture_budget: bool,
	pub identical_bags: bool,
	pub bag_one_of_each_kind: bool,
	pub max_rings: u8,
	pub zag_numer: u8,
	/// `singles,doubles,triples` generation weights. Absent on older logs;
	/// replay uses historical 7:5:4 (tiles themselves are stored). Live
	/// [`RuleSet`] default is 4:3:1.
	#[serde(
		default = "default_ring_count_weights",
		skip_serializing_if = "skip_default_ring_count_weights"
	)]
	pub ring_count_weights: String,
	/// RFC ColorPass.drift diagonal (or `0`). Absent on older logs; replay uses 0.
	#[serde(default = "default_zero_color_drift", skip_serializing_if = "skip_zero_str")]
	pub color_drift: String,
	#[serde(default)]
	pub braid_numer: u8,
	/// Absent/0 on logs from before beads; replay then uses 0 (beads off).
	#[serde(default, skip_serializing_if = "skip_zero_u8")]
	pub bead_numer: u8,
	/// Absent on logs from before capture modifiers; replay uses 50.
	#[serde(default = "default_dot_percent")]
	pub dot_percent: u8,
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
	/// `control-cascade` (live default) or `low-value` (baseline fixed-5).
	/// Absent on older logs; replay uses `low-value`, which matches those ends.
	#[serde(default = "default_low_value_endgame")]
	pub endgame_rule: String,
	/// Extra turn for the material leader on a non-winning cascade drop.
	/// Absent on older logs; replay uses off (PR #74).
	#[serde(default, skip_serializing_if = "std::ops::Not::not")]
	pub cascade_turn_gain: bool,
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
	/// Present (`true`) once normal/bead/braid matches ignore zag colors.
	/// Older logs omit it (zag colors counted as matches); full replay of
	/// those is skipped because capture outcomes differ.
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub zag_match_non_zag: Option<bool>,
	/// `independent` (rule A, live Additive default) or `shared` (rule B fold).
	/// Absent on older logs; replay uses independent, which matches those captures.
	#[serde(default = "default_independent", skip_serializing_if = "skip_independent")]
	pub bead_scoring: String,
	/// Live RuleSet default is `braid-only-exact`. Absent on older logs;
	/// replay uses `current` (coverage) so pre-exact captures still match.
	#[serde(default = "default_current_braid", skip_serializing_if = "skip_current_braid")]
	pub braid_scoring: String,
	/// Lone-heavy 1-slot braids (`--single-braids`). Absent/false on older logs.
	#[serde(default, skip_serializing_if = "std::ops::Not::not")]
	pub allow_single_braids: bool,
	/// Unique node-pair scoring (`--link-once`). Absent/false on older logs
	/// that scored each generating-circle petal. When `score_incident_arcs`
	/// is `Some(false)`, replay also treats this as on.
	#[serde(default, skip_serializing_if = "std::ops::Not::not")]
	pub score_unique_neighbors: bool,
	/// Live RuleSet default is `additive`. Absent on older logs; replay uses
	/// additive, which matches those captures. Independent-max logs write
	/// the field (skip only serializes additive).
	#[serde(
		default = "default_additive_channel_mode",
		skip_serializing_if = "skip_additive_channel_mode"
	)]
	pub channel_mode: String,
	/// `local-claim` (live default) or `cascade` (PR #70 full-board sweep).
	/// Absent on older logs; replay uses cascade, matching those captures.
	#[serde(
		default = "default_cascade_capture_mode",
		skip_serializing_if = "skip_cascade_capture_mode"
	)]
	pub capture_mode: String,
	/// `none,crescent,zs[,blocker,rosette]` generation weights. Absent on
	/// older logs; replay uses the live default only for new deals, not
	/// historical tiles (those already store `emblem` per tile).
	#[serde(
		default = "default_emblem_weights",
		skip_serializing_if = "skip_default_emblem_weights"
	)]
	pub emblem_weights: String,
	/// What Blocker stops as a MOVE intermediate. Absent on older logs;
	/// replay uses `traversal`.
	#[serde(
		default = "default_blocker_blocks",
		skip_serializing_if = "skip_default_blocker_blocks"
	)]
	pub blocker_blocks: String,
	/// Live RuleSet default is `normal-only`. Absent on older logs; replay
	/// uses off. Historical comparison-mode names fold to on.
	#[serde(default = "default_swap_off", skip_serializing_if = "skip_swap_off")]
	pub swap_mode: String,
	/// Historical. Live swaps always ban echoing the last pair.
	#[serde(default, skip_serializing)]
	pub swap_no_immediate_back: bool,
	/// Historical flag; ignored.
	#[serde(default, skip_serializing)]
	pub swap_no_just_moved: bool,
}

fn default_low_value_endgame() -> String {
	EndgameRule::LowValue.name().to_string()
}

fn default_control_ratchet() -> u8 {
	5
}

fn default_blocker_blocks() -> String {
	BlockerBlocks::default().name().to_string()
}

fn skip_default_blocker_blocks(v: &String) -> bool {
	*v == BlockerBlocks::default().name()
}

fn default_swap_off() -> String {
	SwapMode::Off.name().to_string()
}

fn skip_swap_off(v: &String) -> bool {
	*v == SwapMode::Off.name()
}

impl RulesLog {
	pub fn from_rules(r: RuleSet) -> Self {
		Self {
			starting_bench: r.starting_bench_size,
			bag_size: r.bag_size,
			deck_size: r.deck_size,
			self_capture_budget: r.self_capture_budget,
			identical_bags: r.identical_bags,
			bag_one_of_each_kind: r.bag_one_of_each_kind,
			max_rings: r.max_rings,
			zag_numer: r.zag_numer,
			ring_count_weights: r.ring_count_weights.label(),
			color_drift: color_drift_label(r.color_drift),
			braid_numer: r.braid_numer,
			bead_numer: r.bead_numer,
			dot_percent: r.dot_percent,
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
			endgame_rule: r.endgame_rule.name().into(),
			cascade_turn_gain: r.cascade_turn_gain,
			insufficient_tiles_loss: r.insufficient_tiles_loss.name().into(),
			path_constraint: r.path_constraint.name().into(),
			counted_pass_through: r.counted_pass_through,
			pass_through_ring_cmp: r.pass_through_ring_cmp.name().into(),
			zag_mismatch: r.zag_mismatch.name().into(),
			braid_fail_mismatch: Some(r.braid_fail_mismatch.name().into()),
			score_incident_arcs: Some(!r.score_unique_neighbors),
			zag_match_non_zag: Some(true),
			bead_scoring: r.bead_scoring.name().into(),
			braid_scoring: r.braid_scoring.name().into(),
			allow_single_braids: r.allow_single_braids,
			score_unique_neighbors: r.score_unique_neighbors,
			channel_mode: r.channel_mode.name().into(),
			capture_mode: r.capture_mode.name().into(),
			emblem_weights: r.emblem_weights.label(),
			blocker_blocks: r.blocker_blocks.name().into(),
			swap_mode: r.swap_mode.name().into(),
			swap_no_immediate_back: false,
			swap_no_just_moved: false,
		}
	}

	pub fn to_rules(&self) -> Result<RuleSet, String> {
		Ok(RuleSet {
			starting_bench_size: self.starting_bench,
			bag_size: self.bag_size,
			deck_size: if self.deck_size == 0 { self.bag_size } else { self.deck_size },
			self_capture_budget: self.self_capture_budget,
			identical_bags: self.identical_bags,
			bag_one_of_each_kind: self.bag_one_of_each_kind,
			max_rings: self.max_rings,
			zag_numer: self.zag_numer,
			ring_count_weights: RingCountWeights::parse(&self.ring_count_weights)
				.unwrap_or_default(),
			color_drift: parse_color_drift(&self.color_drift),
			braid_numer: self.braid_numer,
			bead_numer: self.bead_numer,
			dot_percent: self.dot_percent,
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
			endgame_rule: EndgameRule::parse(&self.endgame_rule)?,
			cascade_turn_gain: self.cascade_turn_gain,
			insufficient_tiles_loss: InsufficientTilesLoss::parse(&self.insufficient_tiles_loss)?,
			path_constraint: PathConstraint::parse(&self.path_constraint)?,
			counted_pass_through: self.counted_pass_through,
			pass_through_ring_cmp: PassThroughRingCmp::parse(&self.pass_through_ring_cmp)?,
			zag_mismatch: ZagMismatchCount::parse(&self.zag_mismatch)?,
			braid_fail_mismatch: match &self.braid_fail_mismatch {
				Some(s) => BraidFailMismatch::parse(s)?,
				None => BraidFailMismatch::One,
			},
			braid_scoring: BraidScoring::parse(&self.braid_scoring)
				.unwrap_or(BraidScoring::Current),
			allow_single_braids: self.allow_single_braids,
			bead_scoring: BeadScoring::parse(&self.bead_scoring)?,
			score_unique_neighbors: self.score_unique_neighbors
				|| self.score_incident_arcs == Some(false),
			channel_mode: ChannelMode::parse(&self.channel_mode)?,
			capture_mode: CaptureMode::parse(&self.capture_mode)?,
			emblem_weights: EmblemWeights::parse(&self.emblem_weights)
				.unwrap_or_else(|_| EmblemWeights::default()),
			blocker_blocks: BlockerBlocks::parse(&self.blocker_blocks).unwrap_or_default(),
			swap_mode: SwapMode::parse_log(&self.swap_mode).unwrap_or(SwapMode::Off),
		})
	}
}

/// Opening deal: the public 11-tile bench (`p*_bench`), leftover **reserves**
/// (`p*_bag`; never usable in play), and spawn tokens.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct OpeningLog {
	pub p1_bench: Vec<TileLog>,
	pub p2_bench: Vec<TileLog>,
	pub p1_bag: Vec<TileLog>,
	pub p2_bag: Vec<TileLog>,
	pub p1_tokens: usize,
	pub p2_tokens: usize,
	/// One-way control-cascade checkpoint at the opening deal.
	#[serde(default = "default_control_ratchet")]
	pub control_ratchet: u8,
	#[serde(default, skip_serializing_if = "skip_zero_u8")]
	pub extra_turns_p1: u8,
	#[serde(default, skip_serializing_if = "skip_zero_u8")]
	pub extra_turns_p2: u8,
}

/// Full collection, revealed 7-picks, and +4 extensions. Absent on logs from
/// before deck-building; those games already stored the public bench in
/// [`OpeningLog`].
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct SetupLog {
	pub deck_seed: u64,
	pub deck_noise: u8,
	pub pick: String,
	pub p1_deck: Vec<TileLog>,
	pub p2_deck: Vec<TileLog>,
	pub p1_pick7: Vec<TileLog>,
	pub p2_pick7: Vec<TileLog>,
	pub p1_extend: Vec<TileLog>,
	pub p2_extend: Vec<TileLog>,
	#[serde(default = "default_mixed", skip_serializing_if = "skip_mixed")]
	pub p1_arch: String,
	#[serde(default = "default_mixed", skip_serializing_if = "skip_mixed")]
	pub p2_arch: String,
}

impl SetupLog {
	pub fn from_record(rec: &crate::deck::SetupRecord) -> Self {
		Self {
			deck_seed: rec.deck_seed,
			deck_noise: rec.deck_noise,
			pick: rec.strategy.name().into(),
			p1_deck: tiles_to_log(&rec.decks[0]),
			p2_deck: tiles_to_log(&rec.decks[1]),
			p1_pick7: tiles_to_log(&rec.pick7[0]),
			p2_pick7: tiles_to_log(&rec.pick7[1]),
			p1_extend: tiles_to_log(&rec.extend[0]),
			p2_extend: tiles_to_log(&rec.extend[1]),
			p1_arch: rec.p1_deck.name().into(),
			p2_arch: rec.p2_deck.name().into(),
		}
	}

	pub fn from_game(game: &Game) -> Option<Self> {
		game.setup_record().map(Self::from_record)
	}

	pub fn to_record(&self) -> Result<crate::deck::SetupRecord, String> {
		Ok(crate::deck::SetupRecord {
			decks: [tiles_from_log(&self.p1_deck)?, tiles_from_log(&self.p2_deck)?],
			pick7: [tiles_from_log(&self.p1_pick7)?, tiles_from_log(&self.p2_pick7)?],
			extend: [tiles_from_log(&self.p1_extend)?, tiles_from_log(&self.p2_extend)?],
			strategy: PickStrategy::parse(&self.pick).unwrap_or(PickStrategy::Heuristic),
			deck_seed: self.deck_seed,
			deck_noise: self.deck_noise,
			p1_deck: DeckArchetype::parse(&self.p1_arch).unwrap_or(DeckArchetype::Mixed),
			p2_deck: DeckArchetype::parse(&self.p2_arch).unwrap_or(DeckArchetype::Mixed),
		})
	}
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
			control_ratchet: game.control_ratchet(),
			extra_turns_p1: game.extra_turns(Player::One),
			extra_turns_p2: game.extra_turns(Player::Two),
		}
	}
}

/// One applied action. `kind` is ADD/MOVE/CLAIM/PASS; capture flags are outcomes.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
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
	/// `bench` or `tokens`. Absent when the action captured nothing.
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub capture_dest: Option<String>,
	/// One entry per simultaneous removal. Same dest on every entry.
	#[serde(default, skip_serializing_if = "Vec::is_empty")]
	pub captures: Vec<CaptureEventLog>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub piece: Option<PieceRef>,
	/// Partner tile on a SWAP (the occupant of `to` before the exchange).
	#[serde(skip_serializing_if = "Option::is_none")]
	pub partner: Option<PieceRef>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub partner_owner: Option<u8>,
	pub tokens: usize,
	pub budget: usize,
	#[serde(default, skip_serializing_if = "skip_zero_usize")]
	pub surplus: usize,
	/// Minimax completed iterative-deepening depth this move. Absent for MCTS.
	#[serde(default, skip_serializing_if = "skip_zero_u8")]
	pub mm_depth: u8,
	/// MCTS tree max depth this move (root = 0). Absent for minimax.
	#[serde(default, skip_serializing_if = "skip_zero_u8")]
	pub mcts_max_depth: u8,
	/// Visit-weighted mean MCTS node depth this move, excluding the root.
	#[serde(default, skip_serializing_if = "skip_zero_f32")]
	pub mcts_avg_depth: f32,
	#[serde(default, skip_serializing_if = "skip_zero_f32")]
	pub root_entropy: f32,
	#[serde(default, skip_serializing_if = "skip_zero_f32")]
	pub root_eff_branch: f32,
	#[serde(default, skip_serializing_if = "skip_zero_f32")]
	pub root_top1_share: f32,
	#[serde(default, skip_serializing_if = "skip_zero_u16")]
	pub root_near_best_q: u16,
	#[serde(default, skip_serializing_if = "skip_zero_f32")]
	pub root_q_spread_top10: f32,
	#[serde(default, skip_serializing_if = "skip_zero_f32")]
	pub root_best_stability: f32,
	#[serde(default, skip_serializing_if = "skip_zero_u16")]
	pub root_legal: u16,
	#[serde(default, skip_serializing_if = "skip_zero_u8")]
	pub crescent_saves: u8,
	#[serde(default, skip_serializing_if = "skip_zero_u8")]
	pub zs_bonuses: u8,
	/// Tiles that were overwhelmed before this action and are still on the
	/// board, no longer overwhelmed, afterwards.
	#[serde(default, skip_serializing_if = "skip_zero_u8")]
	pub healed: u8,
	/// Overwhelmed tiles remaining on the board after this action.
	#[serde(default, skip_serializing_if = "skip_zero_u8")]
	pub leftover_overwhelmed: u8,
	/// One-way control-cascade checkpoint after this action.
	#[serde(default = "default_control_ratchet")]
	pub control_ratchet: u8,
	#[serde(default, skip_serializing_if = "skip_zero_u8")]
	pub extra_turns_p1: u8,
	#[serde(default, skip_serializing_if = "skip_zero_u8")]
	pub extra_turns_p2: u8,
	#[serde(default, skip_serializing_if = "std::ops::Not::not")]
	pub extra_turn_active: bool,
	/// `mover` or `opponent` when this action awarded cascade turn gain.
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub turn_gain: Option<String>,
}

/// One captured tile: original owner and where it went.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct CaptureEventLog {
	pub node: u8,
	pub owner: u8,
	pub dest: String,
	/// Some incident edge had a non-zero bead match or bead mismatch.
	#[serde(default, skip_serializing_if = "std::ops::Not::not")]
	pub bead_contrib: bool,
	/// Would not have been overwhelmed with the bead channel zeroed.
	#[serde(default, skip_serializing_if = "std::ops::Not::not")]
	pub bead_needed: bool,
	/// Fresh 2-ring ADD popped; a 1-bead neighbor was decisive.
	#[serde(default, skip_serializing_if = "std::ops::Not::not")]
	pub spawn_two_ring_single_bead: bool,
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
			Action::Claim { node } => ("CLAIM".to_string(), None, Some(node), None),
			Action::Swap { from, to } => ("SWAP".to_string(), Some(from), Some(to), None),
			Action::Pass => ("PASS".to_string(), None, None, None),
		};
		let capture = outcome.removals.iter().filter(|r| r.tile.owner != player).count() as u8;
		let self_capture = outcome.removals.iter().filter(|r| r.tile.owner == player).count() as u8;
		let captures: Vec<CaptureEventLog> = outcome
			.removals
			.iter()
			.map(|r| CaptureEventLog {
				node: r.node,
				owner: player_num(r.tile.owner),
				dest: r.dest.name().to_string(),
				bead_contrib: r.bead_contrib,
				bead_needed: r.bead_needed,
				spawn_two_ring_single_bead: r.spawn_two_ring_single_bead,
			})
			.collect();
		let capture_dest = captures.first().map(|c| c.dest.clone());
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
			capture_dest,
			captures,
			piece: piece.map(PieceRef::from_tile),
			partner: None,
			partner_owner: None,
			tokens: game_after.budget_token_count(player),
			budget: game_after.budget(player),
			surplus: game_after.budget_surplus(player),
			mm_depth: 0,
			mcts_max_depth: 0,
			mcts_avg_depth: 0.0,
			root_entropy: 0.0,
			root_eff_branch: 0.0,
			root_top1_share: 0.0,
			root_near_best_q: 0,
			root_q_spread_top10: 0.0,
			root_best_stability: 0.0,
			root_legal: 0,
			crescent_saves: outcome.emblem.crescent_saves,
			zs_bonuses: outcome.emblem.zs_bonuses,
			healed: 0,
			leftover_overwhelmed: game_after.overwhelmed_nodes().len() as u8,
			control_ratchet: game_after.control_ratchet(),
			extra_turns_p1: game_after.extra_turns(Player::One),
			extra_turns_p2: game_after.extra_turns(Player::Two),
			extra_turn_active: game_after.extra_turn_active(),
			turn_gain: game_after.last_turn_gain().map(TurnGainKind::name).map(str::to_string),
		}
	}

	pub fn with_overwhelm_delta(mut self, healed: u8) -> Self {
		self.healed = healed;
		self
	}

	pub fn with_partner(mut self, tile: Tile) -> Self {
		self.partner_owner = Some(player_num(tile.owner));
		self.partner = Some(PieceRef::from_tile(tile));
		self
	}

	pub fn with_search(mut self, stats: crate::ai::SearchStats) -> Self {
		self.mm_depth = stats.depth;
		self.mcts_max_depth = stats.mcts_max_depth;
		self.mcts_avg_depth = stats.mcts_avg_depth;
		self.root_entropy = stats.root_entropy;
		self.root_eff_branch = stats.root_eff_branch;
		self.root_top1_share = stats.root_top1_share;
		self.root_near_best_q = stats.root_near_best_q;
		self.root_q_spread_top10 = stats.root_q_spread_top10;
		self.root_best_stability = stats.root_best_stability;
		self.root_legal = stats.root_legal;
		self
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
			"CLAIM" => {
				let node = self.to.ok_or("CLAIM missing to")?;
				Ok(Action::Claim { node })
			}
			"SWAP" => {
				let from = self.from.ok_or("SWAP missing from")?;
				let to = self.to.ok_or("SWAP missing to")?;
				Ok(Action::Swap { from, to })
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
			"CLAIM" => tags.push("CLAIM"),
			"SWAP" => tags.push("SWAP"),
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
	/// Ratcheted control target at game end (`2..=control_n` under cascade).
	/// Recorded so control wins are reconstructible by checkpoint.
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub control_target: Option<u8>,
	/// `max(p1, p2)` of board + usable spawn budget at game end.
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub max_mat: Option<u8>,
	/// Stored one-way ratchet at game end (stays put even if `max_mat <= 1`).
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub control_ratchet: Option<u8>,
	/// `immediate` for a unique-holder drop win, `hold` for `control_hold_turns`.
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub control_win_kind: Option<String>,
	#[serde(default, skip_serializing_if = "skip_zero_u32")]
	pub turn_gain_awarded: u32,
	#[serde(default, skip_serializing_if = "skip_zero_u32")]
	pub turn_gain_mover: u32,
	#[serde(default, skip_serializing_if = "skip_zero_u32")]
	pub turn_gain_opponent: u32,
	#[serde(default, skip_serializing_if = "skip_zero_u32")]
	pub extra_turns_played: u32,
	#[serde(default, skip_serializing_if = "std::ops::Not::not")]
	pub ended_on_extra: bool,
	#[serde(default, skip_serializing_if = "std::ops::Not::not")]
	pub ended_after_extra: bool,
}

impl EndLog {
	pub fn from_status(status: GameStatus, low_value: Option<LowValueResolution>) -> Self {
		let low_value = low_value.map(LowValueLog::from_resolution);
		match status {
			GameStatus::InProgress => Self {
				status: "in_progress".into(),
				reason: None,
				winner: None,
				low_value,
				control_target: None,
				max_mat: None,
				control_ratchet: None,
				control_win_kind: None,
				turn_gain_awarded: 0,
				turn_gain_mover: 0,
				turn_gain_opponent: 0,
				extra_turns_played: 0,
				ended_on_extra: false,
				ended_after_extra: false,
			},
			GameStatus::Won { winner, reason } => Self {
				status: "won".into(),
				reason: Some(end_reason_name(reason).into()),
				winner: Some(player_num(winner)),
				low_value,
				control_target: None,
				max_mat: None,
				control_ratchet: None,
				control_win_kind: None,
				turn_gain_awarded: 0,
				turn_gain_mover: 0,
				turn_gain_opponent: 0,
				extra_turns_played: 0,
				ended_on_extra: false,
				ended_after_extra: false,
			},
			GameStatus::Draw { reason } => Self {
				status: "draw".into(),
				reason: Some(end_reason_name(reason).into()),
				winner: None,
				low_value,
				control_target: None,
				max_mat: None,
				control_ratchet: None,
				control_win_kind: None,
				turn_gain_awarded: 0,
				turn_gain_mover: 0,
				turn_gain_opponent: 0,
				extra_turns_played: 0,
				ended_on_extra: false,
				ended_after_extra: false,
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
	#[serde(default, skip_serializing_if = "skip_zero_u8")]
	pub bead_rings: u8,
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
			bead_rings: tile.bead_ring_count(),
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
	#[serde(default = "default_control_ratchet")]
	pub control_ratchet: u8,
	#[serde(default, skip_serializing_if = "skip_zero_u8")]
	pub extra_turns_p1: u8,
	#[serde(default, skip_serializing_if = "skip_zero_u8")]
	pub extra_turns_p2: u8,
	#[serde(default, skip_serializing_if = "std::ops::Not::not")]
	pub extra_turn_active: bool,
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
			control_ratchet: game.control_ratchet(),
			extra_turns_p1: game.extra_turns(Player::One),
			extra_turns_p2: game.extra_turns(Player::Two),
			extra_turn_active: game.extra_turn_active(),
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
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub budget_nodes: Option<u64>,
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub budget_iters: Option<u32>,
	pub scenario: String,
	pub rules: RulesLog,
	pub opening: OpeningLog,
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub setup: Option<SetupLog>,
	pub turns: u32,
	pub end: EndLog,
	pub actions: Vec<ActionLog>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub final_state: Option<FinalLog>,
	/// `braid vs bead`, `mixed vs mixed`, …
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub deck_matchup: Option<String>,
	#[serde(default, skip_serializing_if = "skip_zero_u32")]
	pub bead_contrib_captures: u32,
	#[serde(default, skip_serializing_if = "skip_zero_u32")]
	pub bead_needed_captures: u32,
	#[serde(default, skip_serializing_if = "skip_zero_u32")]
	pub spawn_two_ring_single_bead: u32,
	#[serde(default, skip_serializing_if = "skip_zero_u32")]
	pub crescent_saves: u32,
	#[serde(default, skip_serializing_if = "skip_zero_u32")]
	pub zs_bonuses: u32,
	/// Mean minimax completed-ID depth over minimax moves in this game.
	#[serde(default, skip_serializing_if = "skip_zero_f64")]
	pub mm_depth_avg: f64,
	#[serde(default, skip_serializing_if = "skip_zero_u8")]
	pub mm_depth_max: u8,
	/// Mean visit-weighted MCTS tree depth over MCTS moves in this game.
	#[serde(default, skip_serializing_if = "skip_zero_f64")]
	pub mcts_depth_avg: f64,
	#[serde(default, skip_serializing_if = "skip_zero_u8")]
	pub mcts_depth_max: u8,
	#[serde(default, skip_serializing_if = "skip_zero_f64")]
	pub root_entropy_avg: f64,
	#[serde(default, skip_serializing_if = "skip_zero_f64")]
	pub root_eff_branch_avg: f64,
	#[serde(default, skip_serializing_if = "skip_zero_f64")]
	pub root_top1_share_avg: f64,
	#[serde(default, skip_serializing_if = "skip_zero_f64")]
	pub root_near_best_q_avg: f64,
	#[serde(default, skip_serializing_if = "skip_zero_f64")]
	pub root_q_spread_top10_avg: f64,
	#[serde(default, skip_serializing_if = "skip_zero_f64")]
	pub root_best_stability_avg: f64,
	#[serde(default, skip_serializing_if = "skip_zero_f64")]
	pub root_legal_avg: f64,
	/// Search-lever label for P1 (empty on logs from before search profiles).
	#[serde(default, skip_serializing_if = "String::is_empty")]
	pub p1_search: String,
	#[serde(default, skip_serializing_if = "String::is_empty")]
	pub p2_search: String,
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
			budget_nodes: None,
			budget_iters: None,
			scenario: scenario.into(),
			rules: RulesLog::from_rules(*live.rules()),
			opening: OpeningLog::from_game(live),
			setup: SetupLog::from_game(live),
			turns: 0,
			end: EndLog::from_status(GameStatus::InProgress, None),
			actions: Vec::new(),
			final_state: None,
			deck_matchup: live
				.setup_record()
				.map(|r| format!("{} vs {}", r.p1_deck.name(), r.p2_deck.name())),
			bead_contrib_captures: 0,
			bead_needed_captures: 0,
			spawn_two_ring_single_bead: 0,
			crescent_saves: 0,
			zs_bonuses: 0,
			mm_depth_avg: 0.0,
			mm_depth_max: 0,
			mcts_depth_avg: 0.0,
			mcts_depth_max: 0,
			root_entropy_avg: 0.0,
			root_eff_branch_avg: 0.0,
			root_top1_share_avg: 0.0,
			root_near_best_q_avg: 0.0,
			root_q_spread_top10_avg: 0.0,
			root_best_stability_avg: 0.0,
			root_legal_avg: 0.0,
			p1_search: String::new(),
			p2_search: String::new(),
		}
	}

	pub fn push_action(
		&mut self,
		action: Action,
		piece: Option<Tile>,
		outcome: &ActionOutcome,
		game_after: &Game,
	) {
		self.push_action_partner(action, piece, None, outcome, game_after);
	}

	pub fn push_action_partner(
		&mut self,
		action: Action,
		piece: Option<Tile>,
		partner: Option<Tile>,
		outcome: &ActionOutcome,
		game_after: &Game,
	) {
		let mut rec = ActionLog::record(action, piece, outcome, game_after);
		if let Some(p) = partner {
			rec = rec.with_partner(p);
		}
		self.actions.push(rec);
	}

	pub fn push_action_with_search(
		&mut self,
		action: Action,
		piece: Option<Tile>,
		outcome: &ActionOutcome,
		game_after: &Game,
		stats: crate::ai::SearchStats,
	) {
		self.push_action_with_search_partner(action, piece, None, outcome, game_after, stats);
	}

	pub fn push_action_with_search_partner(
		&mut self,
		action: Action,
		piece: Option<Tile>,
		partner: Option<Tile>,
		outcome: &ActionOutcome,
		game_after: &Game,
		stats: crate::ai::SearchStats,
	) {
		let mut rec = ActionLog::record(action, piece, outcome, game_after).with_search(stats);
		if let Some(p) = partner {
			rec = rec.with_partner(p);
		}
		self.actions.push(rec);
	}

	pub fn finish(&mut self, live: &Game, low_value: Option<LowValueResolution>) {
		self.turns = live.turn_count();
		self.end = EndLog::from_status(live.status(), low_value);
		self.end.control_target = live.cascade_control_target().or(Some(live.control_need()));
		let max_mat = live.max_material().min(u8::MAX as usize) as u8;
		self.end.max_mat = Some(max_mat);
		self.end.control_ratchet = Some(live.control_ratchet());
		if matches!(live.status(), GameStatus::Won { reason: EndReason::Control, .. }) {
			self.end.control_win_kind =
				Some(if live.control_win_immediate() { "immediate".into() } else { "hold".into() });
		}
		self.end.turn_gain_awarded = live.turn_gain_awarded();
		self.end.turn_gain_mover = live.turn_gain_mover();
		self.end.turn_gain_opponent = live.turn_gain_opponent();
		self.end.extra_turns_played = live.extra_turns_played();
		self.end.ended_on_extra = live.ended_on_extra_turn();
		self.end.ended_after_extra = live.ended_after_extra_turn();
		self.final_state = Some(FinalLog::from_game(live));
		let mut contrib = 0u32;
		let mut needed = 0u32;
		let mut spawn = 0u32;
		for act in &self.actions {
			for cap in &act.captures {
				if cap.bead_contrib {
					contrib += 1;
				}
				if cap.bead_needed {
					needed += 1;
				}
				if cap.spawn_two_ring_single_bead {
					spawn += 1;
				}
			}
		}
		self.bead_contrib_captures = contrib;
		self.bead_needed_captures = needed;
		self.spawn_two_ring_single_bead = spawn;
		self.crescent_saves = self.actions.iter().map(|a| u32::from(a.crescent_saves)).sum();
		self.zs_bonuses = self.actions.iter().map(|a| u32::from(a.zs_bonuses)).sum();
		let mut mm_sum = 0.0f64;
		let mut mm_n = 0u32;
		let mut mm_max = 0u8;
		let mut mcts_sum = 0.0f64;
		let mut mcts_n = 0u32;
		let mut mcts_max = 0u8;
		let mut ent = 0.0f64;
		let mut eff = 0.0f64;
		let mut top1 = 0.0f64;
		let mut near = 0.0f64;
		let mut qspread = 0.0f64;
		let mut stab = 0.0f64;
		let mut legal = 0.0f64;
		let mut spread_n = 0u32;
		for act in &self.actions {
			if act.mm_depth > 0 {
				mm_sum += f64::from(act.mm_depth);
				mm_n += 1;
				mm_max = mm_max.max(act.mm_depth);
			}
			if act.mcts_max_depth > 0 || act.mcts_avg_depth > 0.0 {
				mcts_sum += f64::from(act.mcts_avg_depth);
				mcts_n += 1;
				mcts_max = mcts_max.max(act.mcts_max_depth);
			}
			if act.root_legal > 0 || act.root_eff_branch > 0.0 {
				ent += f64::from(act.root_entropy);
				eff += f64::from(act.root_eff_branch);
				top1 += f64::from(act.root_top1_share);
				near += f64::from(act.root_near_best_q);
				qspread += f64::from(act.root_q_spread_top10);
				stab += f64::from(act.root_best_stability);
				legal += f64::from(act.root_legal);
				spread_n += 1;
			}
		}
		self.mm_depth_avg = if mm_n == 0 { 0.0 } else { mm_sum / f64::from(mm_n) };
		self.mm_depth_max = mm_max;
		self.mcts_depth_avg = if mcts_n == 0 { 0.0 } else { mcts_sum / f64::from(mcts_n) };
		self.mcts_depth_max = mcts_max;
		if spread_n == 0 {
			self.root_entropy_avg = 0.0;
			self.root_eff_branch_avg = 0.0;
			self.root_top1_share_avg = 0.0;
			self.root_near_best_q_avg = 0.0;
			self.root_q_spread_top10_avg = 0.0;
			self.root_best_stability_avg = 0.0;
			self.root_legal_avg = 0.0;
		} else {
			let n = f64::from(spread_n);
			self.root_entropy_avg = ent / n;
			self.root_eff_branch_avg = eff / n;
			self.root_top1_share_avg = top1 / n;
			self.root_near_best_q_avg = near / n;
			self.root_q_spread_top10_avg = qspread / n;
			self.root_best_stability_avg = stab / n;
			self.root_legal_avg = legal / n;
		}
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
	Ok(replay_center_captures(log)?.0)
}

/// Replay `log`, counting opponent and self-captures at the origin.
///
/// Returns `(replayed game, opponent tiles popped at center, own tiles popped
/// at center)`.
pub fn replay_center_captures(log: &GameLog) -> Result<(Game, u32, u32), String> {
	if log.v != SCHEMA_VERSION {
		return Err(format!("unsupported log schema v={}", log.v));
	}
	let rules = log.rules.to_rules()?;
	let mut game = if let Some(setup) = &log.setup {
		Game::from_setup_record(rules, log.seed, setup.to_record()?)
	} else {
		let mut game = Game::empty(rules);
		game.set_bag(Player::One, tiles_from_log(&log.opening.p1_bag)?);
		game.set_bag(Player::Two, tiles_from_log(&log.opening.p2_bag)?);
		game.set_bench(Player::One, tiles_from_log(&log.opening.p1_bench)?);
		game.set_bench(Player::Two, tiles_from_log(&log.opening.p2_bench)?);
		game
	};
	game.set_budget_tokens(Player::One, vec![BudgetToken::Blank; log.opening.p1_tokens]);
	game.set_budget_tokens(Player::Two, vec![BudgetToken::Blank; log.opening.p2_tokens]);
	game.set_current(Player::One);
	let center = game.board().center();
	let mut opp = 0u32;
	let mut slf = 0u32;
	for (i, act) in log.actions.iter().enumerate() {
		let action = act.to_action().map_err(|e| format!("action {i}: {e}"))?;
		let player = game.current_player();
		let outcome = game.apply(action).map_err(|e| format!("action {i} apply: {e}"))?;
		for rem in &outcome.removals {
			if rem.node != center {
				continue;
			}
			if rem.tile.owner != player {
				opp += 1;
			} else {
				slf += 1;
			}
		}
	}
	Ok((game, opp, slf))
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
		Action::Claim { node } => game.tile_at(node),
		Action::Swap { from, .. } => game.tile_at(from),
		Action::Pass => None,
	}
}

/// Partner tile looked up *before* `apply` (occupant of SWAP `to`).
pub fn partner_for_action(game: &Game, action: Action) -> Option<Tile> {
	match action {
		Action::Swap { to, .. } => game.tile_at(to),
		_ => None,
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
			let partner = partner_for_action(&game, action);
			match game.apply(action) {
				Ok(outcome) => {
					log.push_action_partner(action, piece, partner, &outcome, &game);
				}
				Err(_) => {
					let fallback = game.legal_actions().into_iter().next().unwrap_or(Action::Pass);
					let piece = piece_for_action(&game, fallback);
					let partner = partner_for_action(&game, fallback);
					if let Ok(outcome) = game.apply(fallback) {
						log.push_action_partner(fallback, piece, partner, &outcome, &game);
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
			"setup",
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
		assert_eq!(parsed[0].rules.deck_size, crate::game::BAG_SIZE as u8);
		assert!(!parsed[0].rules.capture_draw_on_move);
		assert_eq!(parsed[0].opening.p1_bench.len(), 11);
		assert_eq!(parsed[0].opening.p1_bench.len(), parsed[0].opening.p2_bench.len());
		assert_eq!(parsed[0].opening.p1_bag.len(), 16);
		assert_eq!(parsed[0].opening.p2_bag.len(), 16);
		let setup = parsed[0].setup.as_ref().expect("setup log");
		assert_eq!(setup.p1_deck.len(), 27);
		assert_eq!(setup.p1_pick7.len(), 7);
		assert_eq!(setup.p1_extend.len(), 4);
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
				matches!(act.kind.as_str(), "ADD" | "MOVE" | "CLAIM" | "SWAP" | "PASS"),
				"unexpected kind {}",
				act.kind
			);
			match act.kind.as_str() {
				"ADD" => {
					assert!(act.bench_index.is_some());
					assert!(act.to.is_some());
					assert!(act.piece.is_some());
				}
				"MOVE" | "SWAP" => {
					assert!(act.from.is_some());
					assert!(act.to.is_some());
					assert!(act.piece.is_some());
					let piece = act.piece.as_ref().unwrap();
					assert_eq!(
						piece.n_rings,
						piece.normal_rings + piece.zag_rings + piece.braid_rings + piece.bead_rings
					);
					if act.kind == "SWAP" {
						assert_eq!(piece.n_rings, 1);
						assert!(act.partner.is_some());
						assert!(act.partner_owner.is_some());
					}
				}
				"CLAIM" => {
					assert!(act.to.is_some());
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
		r.bead_scoring = BeadScoring::SharedChannel;
		r.braid_scoring = BraidScoring::BraidOnlyMatch;
		r.score_unique_neighbors = true;
		let log = RulesLog::from_rules(r);
		assert_eq!(log.to_rules().unwrap(), r);
		assert_eq!(log.dot_percent, DOT_PERCENT_DEFAULT);
		assert_eq!(log.bead_scoring, "shared");
		assert_eq!(log.braid_scoring, "braid-only-match");
		assert!(log.allow_single_braids);
		assert!(log.score_unique_neighbors);
		assert_eq!(log.score_incident_arcs, Some(false));
	}

	#[test]
	fn missing_braid_scoring_replays_as_current() {
		let mut r = RuleSet::default();
		r.braid_scoring = BraidScoring::Current;
		r.allow_single_braids = false;
		let log = RulesLog::from_rules(r);
		let json = serde_json::to_string(&log).unwrap();
		assert!(!json.contains("braid_scoring"), "{json}");
		assert!(!json.contains("score_unique_neighbors"), "{json}");
		assert!(!json.contains("allow_single_braids"), "{json}");
		let parsed: RulesLog = serde_json::from_str(&json).unwrap();
		assert_eq!(parsed.braid_scoring, "current");
		assert!(!parsed.score_unique_neighbors);
		assert!(!parsed.allow_single_braids);
		assert_eq!(parsed.to_rules().unwrap().braid_scoring, BraidScoring::Current);
		assert!(!parsed.to_rules().unwrap().score_unique_neighbors);
	}

	#[test]
	fn live_defaults_write_exact_single_braids_and_normal_only_swap() {
		let log = RulesLog::from_rules(RuleSet::default());
		let json = serde_json::to_string(&log).unwrap();
		assert!(json.contains("braid-only-exact"), "{json}");
		assert!(json.contains("allow_single_braids"), "{json}");
		assert!(json.contains("normal-only"), "{json}");
		assert!(json.contains("4,3,1"), "{json}");
		assert!(json.contains("\"160\""), "{json}");
		let parsed: RulesLog = serde_json::from_str(&json).unwrap();
		assert_eq!(parsed.to_rules().unwrap(), RuleSet::default());
	}

	#[test]
	fn missing_bead_scoring_replays_as_independent() {
		let mut r = RuleSet::default();
		r.bead_scoring = BeadScoring::Independent;
		let log = RulesLog::from_rules(r);
		let json = serde_json::to_string(&log).unwrap();
		assert!(!json.contains("bead_scoring"), "{json}");
		let parsed: RulesLog = serde_json::from_str(&json).unwrap();
		assert_eq!(parsed.bead_scoring, "independent");
		assert_eq!(parsed.to_rules().unwrap().bead_scoring, BeadScoring::Independent);
	}

	#[test]
	fn missing_channel_mode_replays_as_additive() {
		let mut r = RuleSet::additive();
		r.bead_scoring = BeadScoring::Independent;
		let log = RulesLog::from_rules(r);
		let json = serde_json::to_string(&log).unwrap();
		assert!(!json.contains("channel_mode"), "{json}");
		let parsed: RulesLog = serde_json::from_str(&json).unwrap();
		assert_eq!(parsed.channel_mode, "additive");
		assert_eq!(parsed.to_rules().unwrap().channel_mode, ChannelMode::Additive);
	}

	#[test]
	fn missing_cascade_turn_gain_replays_as_off() {
		let mut r = RuleSet::default();
		r.cascade_turn_gain = false;
		let log = RulesLog::from_rules(r);
		let json = serde_json::to_string(&log).unwrap();
		assert!(!json.contains("cascade_turn_gain"), "{json}");
		let parsed: RulesLog = serde_json::from_str(&json).unwrap();
		assert!(!parsed.cascade_turn_gain);
		assert!(!parsed.to_rules().unwrap().cascade_turn_gain);
		let on = RulesLog::from_rules(RuleSet::default());
		assert!(on.cascade_turn_gain);
		assert!(serde_json::to_string(&on).unwrap().contains("cascade_turn_gain"));
	}

	#[test]
	fn finish_records_cascade_turn_gain() {
		use crate::game::{Action, Game};

		let (mut game, dest) = Game::cascade_turn_gain_mover_add_drop();
		let mut log = GameLog::begin(1, "t", "a", "b", 0, "default", &game);
		assert!(log.rules.cascade_turn_gain);
		let action = Action::Add { bench_index: 0, node: dest };
		let outcome = game.apply(action).unwrap();
		log.push_action(action, None, &outcome, &game);
		log.finish(&game, None);
		assert_eq!(log.actions[0].turn_gain.as_deref(), Some("mover"));
		assert!(log.actions[0].extra_turn_active);
		assert_eq!(log.end.turn_gain_awarded, 1);
		assert_eq!(log.end.turn_gain_mover, 1);
		assert_eq!(log.end.turn_gain_opponent, 0);
		assert_eq!(log.end.extra_turns_played, 1);
		assert_eq!(log.final_state.as_ref().map(|s| s.extra_turn_active), Some(true));
	}

	#[test]
	fn missing_endgame_rule_replays_as_low_value() {
		let mut r = RuleSet::default();
		r.endgame_rule = EndgameRule::LowValue;
		let log = RulesLog::from_rules(r);
		let mut value: serde_json::Value =
			serde_json::from_str(&serde_json::to_string(&log).unwrap()).unwrap();
		value.as_object_mut().unwrap().remove("endgame_rule");
		let parsed: RulesLog = serde_json::from_value(value).unwrap();
		assert_eq!(parsed.endgame_rule, "low-value");
		assert_eq!(parsed.to_rules().unwrap().endgame_rule, EndgameRule::LowValue);
		let cascade = RulesLog::from_rules(RuleSet::default());
		assert_eq!(cascade.endgame_rule, "control-cascade");
		assert!(serde_json::to_string(&cascade).unwrap().contains("control-cascade"));
	}

	#[test]
	fn finish_records_control_target_and_max_mat() {
		use crate::game::{Game, Player, TileColor};

		let mut game = Game::empty(RuleSet::default());
		let center = game.board().center();
		let inner = game.board().inner_ring().to_vec();
		let occupant = game.alloc_tile(Player::One, TileColor::Green, 3, 1);
		game.place_tile(center, occupant);
		for &node in inner.iter().take(3) {
			let tile = game.alloc_tile(Player::One, TileColor::Green, 3, 1);
			game.place_tile(node, tile);
		}
		game.set_budget_tokens(Player::One, Vec::new());
		game.set_budget_tokens(Player::Two, Vec::new());
		game.evaluate_turn_end_for_test(Player::One);
		assert_eq!(
			game.status(),
			GameStatus::Won { winner: Player::One, reason: EndReason::Control }
		);
		let mut log = GameLog::begin(1, "t", "a", "b", 0, "default", &game);
		log.finish(&game, None);
		assert_eq!(log.end.control_target, Some(4));
		assert_eq!(log.end.max_mat, Some(4));
		assert_eq!(log.end.control_ratchet, Some(4));
		assert_eq!(log.end.control_win_kind.as_deref(), Some("hold"));
		assert_eq!(log.end.reason.as_deref(), Some("control"));
	}

	#[test]
	fn finish_records_immediate_control_drop_win() {
		use crate::game::{Action, Game};

		let (mut game, extra) = Game::cascade_unique_holder_drop_position(3);
		assert_eq!(game.control_ratchet(), 5);
		let mut log = GameLog::begin(1, "t", "a", "b", 0, "default", &game);
		assert_eq!(log.opening.control_ratchet, 5);
		game.apply(Action::Claim { node: extra }).unwrap();
		log.finish(&game, None);
		assert!(game.control_win_immediate());
		assert_eq!(log.end.control_target, Some(4));
		assert_eq!(log.end.control_ratchet, Some(4));
		assert_eq!(log.end.control_win_kind.as_deref(), Some("immediate"));
		assert_eq!(log.end.reason.as_deref(), Some("control"));
		assert_eq!(log.end.winner, Some(1));
		assert_eq!(log.final_state.as_ref().map(|s| s.control_ratchet), Some(4));
	}

	#[test]
	fn missing_capture_mode_replays_as_cascade() {
		let mut r = RuleSet::default();
		r.capture_mode = CaptureMode::Cascade;
		let log = RulesLog::from_rules(r);
		let json = serde_json::to_string(&log).unwrap();
		assert!(!json.contains("capture_mode"), "{json}");
		let parsed: RulesLog = serde_json::from_str(&json).unwrap();
		assert_eq!(parsed.capture_mode, "cascade");
		assert_eq!(parsed.to_rules().unwrap().capture_mode, CaptureMode::Cascade);
	}

	#[test]
	fn jsonl_records_capture_dest_per_removal() {
		use crate::game::{Action, CaptureDest, CaptureMark, Game, Player, TileColor};

		let mut game = Game::empty(RuleSet::default());
		let spawn = game.board().spawns()[0];
		let nbs = game.board().neighbors(spawn).to_vec();
		let land = nbs[0];
		let from = *game
			.board()
			.neighbors(land)
			.iter()
			.find(|n| **n != spawn && !nbs.contains(n))
			.unwrap_or(&nbs[2]);
		let victim = game.alloc_tile(Player::Two, TileColor::Green, 1, 1);
		game.place_tile(spawn, victim);
		let helper = game.alloc_rings(
			Player::One,
			&[TileColor::Orange, TileColor::Teal, TileColor::Maroon],
			3,
			1,
		);
		game.place_tile(nbs[1], helper);
		let mover = game
			.alloc_tile(Player::One, TileColor::Blue, 3, 1)
			.with_capture(CaptureMark::Dot);
		game.place_tile(from, mover);
		game.set_current(Player::One);
		let mut log = GameLog::begin(1, "t", "g", "g", 0, "default", &game);
		let action = Action::Move { from, to: land };
		let piece = piece_for_action(&game, action);
		let outcome = game.apply(action).unwrap();
		log.push_action(action, piece, &outcome, &game);
		log.finish(&game, None);
		let act = log.actions.last().unwrap();
		assert_eq!(act.capture_dest.as_deref(), Some("bench"));
		assert!(!act.captures.is_empty());
		assert!(act.captures.iter().all(|c| c.dest == "bench"));
		assert!(outcome.removals.iter().all(|r| r.dest == CaptureDest::Bench));
	}

	#[test]
	fn jsonl_records_and_replays_a_claim() {
		use crate::game::{Action, CaptureDest, Game, Player, TileColor};

		let mut game = Game::empty(RuleSet::default());
		assert_eq!(game.rules().capture_mode, CaptureMode::LocalClaim);
		let spawn = game.board().spawns()[0];
		let nbs = game.board().neighbors(spawn).to_vec();
		let victim = game.alloc_tile(Player::Two, TileColor::Green, 1, 1);
		game.place_tile(spawn, victim);
		let heavy = game.alloc_rings(
			Player::One,
			&[TileColor::Orange, TileColor::Teal, TileColor::Maroon],
			1,
			1,
		);
		game.place_tile(nbs[0], heavy);
		let heavy2 = game.alloc_rings(
			Player::One,
			&[TileColor::Orange, TileColor::Teal, TileColor::Maroon],
			1,
			1,
		);
		game.place_tile(nbs[1], heavy2);
		assert!(game.is_overwhelmed(spawn));
		game.set_current(Player::One);
		let tokens = game.budget_token_count(Player::One);
		let mut log = GameLog::begin(1, "t", "g", "g", 0, "default", &game);
		let action = Action::Claim { node: spawn };
		let piece = piece_for_action(&game, action);
		let outcome = game.apply(action).unwrap();
		log.push_action(action, piece, &outcome, &game);
		log.finish(&game, None);
		let act = log.actions.last().unwrap();
		assert_eq!(act.kind, "CLAIM");
		assert_eq!(act.to, Some(spawn));
		assert_eq!(act.capture_dest.as_deref(), Some("tokens"));
		assert_eq!(act.captures.len(), 1);
		assert_eq!(act.captures[0].dest, "tokens");
		assert_eq!(outcome.removals[0].dest, CaptureDest::Tokens);
		assert_eq!(game.budget_token_count(Player::One), tokens + 1);
		assert_eq!(log.rules.capture_mode, "local-claim");
		let json = encode_jsonl(std::slice::from_ref(&log)).unwrap();
		assert!(json.contains("\"kind\":\"CLAIM\""), "{json}");
		assert!(json.contains("\"capture_mode\":\"local-claim\""), "{json}");
		let parsed = decode_jsonl(&json).unwrap();
		assert_eq!(parsed.len(), 1);
		assert_eq!(parsed[0].actions[0].kind, "CLAIM");
		assert_eq!(parsed[0].actions[0].to_action().unwrap(), Action::Claim { node: spawn });
		assert_eq!(parsed[0].rules.to_rules().unwrap().capture_mode, CaptureMode::LocalClaim);
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
		assert_committed_jsonl_gz_replays_n(rel, 108);
	}

	fn assert_committed_jsonl_gz_replays_n(rel: &str, expected: usize) {
		let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(rel);
		if !path.exists() {
			return;
		}
		let bytes = std::fs::read(&path).unwrap();
		let logs = decode_jsonl_gz(&bytes).expect("decode gzip jsonl");
		assert_eq!(logs.len(), expected, "{rel} expected {expected} games");
		let derived_weight = logs.iter().all(opening_weight_equals_rings);
		let independent_channels = logs.iter().all(|log| log.rules.braid_fail_mismatch.is_some());
		let incident_arc_scoring = logs.iter().all(|log| log.rules.score_incident_arcs.is_some());
		let zag_match_non_zag = logs.iter().all(|log| log.rules.zag_match_non_zag == Some(true));
		for (i, log) in logs.iter().enumerate() {
			assert_eq!(log.v, SCHEMA_VERSION);
			assert_eq!(log.rules.bag_size, 11, "{rel}");
			assert!(log.rules.self_capture_budget, "{rel}");
			assert_eq!(log.search_rng_seed, search_rng_seed(log.seed));
			assert!(!log.opening.p1_bench.is_empty());
			assert!(!log.actions.is_empty() || log.turns == 0);
			if !derived_weight
				|| !independent_channels
				|| !incident_arc_scoring
				|| !zag_match_non_zag
			{
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
	fn committed_center_double_edge_doubles_allowed_jsonl_gz_replays_all_games() {
		assert_committed_jsonl_gz_replays(
			"benchmark_results/center_double_edge_doubles_allowed/h2h.jsonl.gz",
		);
	}

	#[test]
	fn committed_center_double_edge_doubles_disallowed_jsonl_gz_replays_all_games() {
		assert_committed_jsonl_gz_replays(
			"benchmark_results/center_double_edge_doubles_disallowed/h2h.jsonl.gz",
		);
	}

	#[test]
	fn committed_work_budget_doubles_allowed_jsonl_gz_replays_all_games() {
		assert_committed_jsonl_gz_replays(
			"benchmark_results/work_budget_doubles_allowed/h2h.jsonl.gz",
		);
	}

	#[test]
	fn committed_capture_modifiers_jsonl_gz_replays_all_games() {
		assert_committed_jsonl_gz_replays("benchmark_results/capture_modifiers/h2h.jsonl.gz");
	}

	#[test]
	fn committed_zag_color_exclusion_jsonl_gz_replays_all_games() {
		assert_committed_jsonl_gz_replays("benchmark_results/zag_color_exclusion/h2h.jsonl.gz");
	}

	#[test]
	fn committed_work_budget_doubles_disallowed_jsonl_gz_replays_all_games() {
		assert_committed_jsonl_gz_replays(
			"benchmark_results/work_budget_doubles_disallowed/h2h.jsonl.gz",
		);
	}

	#[test]
	fn committed_bead_shared_channel_independent_shallow_replays() {
		assert_committed_jsonl_gz_replays_n(
			"benchmark_results/bead_shared_channel/independent_shallow.jsonl.gz",
			50,
		);
	}

	#[test]
	fn committed_bead_shared_channel_independent_deep_replays() {
		assert_committed_jsonl_gz_replays_n(
			"benchmark_results/bead_shared_channel/independent_deep.jsonl.gz",
			10,
		);
	}

	#[test]
	fn committed_bead_shared_channel_shared_shallow_replays() {
		assert_committed_jsonl_gz_replays_n(
			"benchmark_results/bead_shared_channel/shared_shallow.jsonl.gz",
			50,
		);
	}

	#[test]
	fn committed_bead_shared_channel_shared_deep_replays() {
		assert_committed_jsonl_gz_replays_n(
			"benchmark_results/bead_shared_channel/shared_deep.jsonl.gz",
			10,
		);
	}

	#[test]
	fn committed_bead_shared_channel_groups_shallow_replays() {
		assert_committed_jsonl_gz_replays_n(
			"benchmark_results/bead_shared_channel/groups/groups_shallow.jsonl.gz",
			50,
		);
	}

	#[test]
	fn committed_bead_shared_channel_groups_deep_replays() {
		assert_committed_jsonl_gz_replays_n(
			"benchmark_results/bead_shared_channel/groups/groups_deep.jsonl.gz",
			10,
		);
	}

	#[test]
	fn committed_local_claim_shallow_replays() {
		assert_committed_jsonl_gz_replays_n(
			"benchmark_results/local_claim/h2h_shallow.jsonl.gz",
			100,
		);
	}

	#[test]
	fn committed_control_cascade_shallow_replays() {
		// Pre-ratchet live-target log: decode only (ends would diverge).
		let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
			.join("benchmark_results/control_cascade/h2h_shallow.jsonl.gz");
		if !path.exists() {
			return;
		}
		let bytes = std::fs::read(&path).unwrap();
		let logs = decode_jsonl_gz(&bytes).expect("decode gzip jsonl");
		assert_eq!(logs.len(), 100);
	}

	#[test]
	fn committed_control_cascade_ratchet_shallow_replays() {
		assert_committed_jsonl_gz_replays_n(
			"benchmark_results/control_cascade/h2h_shallow_ratchet.jsonl.gz",
			100,
		);
	}

	#[test]
	fn committed_control_cascade_turn_gain_shallow_replays() {
		assert_committed_jsonl_gz_replays_n(
			"benchmark_results/control_cascade_turn_gain/h2h_shallow.jsonl.gz",
			100,
		);
	}

	#[test]
	fn committed_ring_dist_channels_shallow_replays() {
		for rel in [
			"benchmark_results/ring_dist_channels/h2h_a_additive_enhanced.jsonl.gz",
			"benchmark_results/ring_dist_channels/h2h_b_imax_enhanced.jsonl.gz",
			"benchmark_results/ring_dist_channels/h2h_a_additive_counterplay.jsonl.gz",
			"benchmark_results/ring_dist_channels/h2h_b_imax_counterplay.jsonl.gz",
		] {
			assert_committed_jsonl_gz_replays_n(rel, 100);
		}
	}

	#[test]
	fn committed_singles_swap_shallow_replays() {
		for rel in [
			"benchmark_results/singles_swap/h2h_b_adj_any_counterplay_noback.jsonl.gz",
			"benchmark_results/singles_swap/h2h_b_locked_counterplay.jsonl.gz",
		] {
			assert_committed_jsonl_gz_replays_n(rel, 100);
		}
	}

	#[test]
	fn committed_single_braid_exact_swap_shallow_replays() {
		for rel in [
			"benchmark_results/single_braid_exact_swap/h2h_full_link_once.jsonl.gz",
			"benchmark_results/single_braid_exact_swap/h2h_full_double.jsonl.gz",
			"benchmark_results/single_braid_exact_swap/h2h_full_double_noswap.jsonl.gz",
			"benchmark_results/single_braid_exact_swap/h2h_full_double_nobraid.jsonl.gz",
			"benchmark_results/single_braid_exact_swap/h2h_defaults.jsonl.gz",
		] {
			assert_committed_jsonl_gz_replays_n(rel, 100);
		}
	}

	#[test]
	fn committed_exactplay_shallow_replays() {
		for rel in [
			"benchmark_results/exactplay/h2h_self.jsonl.gz",
			"benchmark_results/exactplay/h2h_exact_as_p1.jsonl.gz",
			"benchmark_results/exactplay/h2h_exact_as_p2.jsonl.gz",
		] {
			assert_committed_jsonl_gz_replays_n(rel, 100);
		}
	}

	#[test]
	fn committed_center_link_once_shallow_replays() {
		for rel in [
			"benchmark_results/center_link_once/h2h_431_add_bom.jsonl.gz",
			"benchmark_results/center_link_once/h2h_431_add.jsonl.gz",
			"benchmark_results/center_link_once/h2h_431_imax.jsonl.gz",
			"benchmark_results/center_link_once/h2h_431_imax_bom.jsonl.gz",
			"benchmark_results/center_link_once/h2h_754_add.jsonl.gz",
			"benchmark_results/center_link_once/h2h_754_add_bom.jsonl.gz",
			"benchmark_results/center_link_once/h2h_754_imax.jsonl.gz",
			"benchmark_results/center_link_once/h2h_754_imax_bom.jsonl.gz",
		] {
			assert_committed_jsonl_gz_replays_n(rel, 100);
		}
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
