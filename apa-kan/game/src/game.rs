//! Game model, RuleSet, overwhelm, movement, and win detection.

use std::sync::Arc;

use rand::rngs::StdRng;
use rand::seq::SliceRandom;
use rand::{Rng, SeedableRng};

use crate::board::{Board, NodeId};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Player {
	One,
	Two,
}

impl Player {
	pub fn other(self) -> Self {
		match self {
			Player::One => Player::Two,
			Player::Two => Player::One,
		}
	}

	pub fn index(self) -> usize {
		match self {
			Player::One => 0,
			Player::Two => 1,
		}
	}

	pub fn label(self) -> &'static str {
		match self {
			Player::One => "P1",
			Player::Two => "P2",
		}
	}

	pub fn shade(self) -> &'static str {
		match self {
			Player::One => "light",
			Player::Two => "dark",
		}
	}
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum TileColor {
	Green = 0,
	Blue = 1,
	Orange = 2,
	LavenderPink = 3,
	Teal = 4,
	Maroon = 5,
}

/// Common 81, uncommon 9, rare 1. Total 262.
pub const COLOR_WEIGHTS: [(TileColor, u32); 6] = [
	(TileColor::Green, 81),
	(TileColor::Blue, 81),
	(TileColor::Orange, 81),
	(TileColor::LavenderPink, 9),
	(TileColor::Teal, 9),
	(TileColor::Maroon, 1),
];
pub const COLOR_WEIGHT_TOTAL: u32 = 262;
/// Ring 2/3: weight 7 keep the last existing color, weight 1 draw any color.
pub const RING_SAME_WEIGHT: u32 = 7;
pub const RING_ANY_WEIGHT: u32 = 1;
pub const RING_EXIST_TOTAL: u32 = RING_SAME_WEIGHT + RING_ANY_WEIGHT;
/// Each ring is independently a zag with probability `zag_numer / ZAG_DENOM`.
pub const ZAG_DENOM: u8 = 7;
pub const ZAG_NUMER_DEFAULT: u8 = 1;
/// Sampled bag size (replaces the old 27 unique 3-color kinds).
pub const BAG_SIZE: usize = 27;
pub const MAX_COLOR_RINGS: u8 = 3;
/// Each of 6 colors is absent (0), a normal ring (1), or a zag ring (2).
pub const COLOR_STATES: usize = 729;

impl TileColor {
	pub const ALL: [TileColor; 6] = [
		TileColor::Green,
		TileColor::Blue,
		TileColor::Orange,
		TileColor::LavenderPink,
		TileColor::Teal,
		TileColor::Maroon,
	];
	pub const COMMON: [TileColor; 3] = [TileColor::Green, TileColor::Blue, TileColor::Orange];

	pub fn name(self) -> &'static str {
		match self {
			TileColor::Green => "green",
			TileColor::Blue => "blue",
			TileColor::Orange => "orange",
			TileColor::LavenderPink => "lavender-pink",
			TileColor::Teal => "teal",
			TileColor::Maroon => "maroon",
		}
	}

	pub fn tag(self) -> &'static str {
		match self {
			TileColor::Green => "g",
			TileColor::Blue => "b",
			TileColor::Orange => "o",
			TileColor::LavenderPink => "p",
			TileColor::Teal => "t",
			TileColor::Maroon => "m",
		}
	}

	pub fn parse(s: &str) -> Result<Self, String> {
		match s.to_ascii_lowercase().replace('_', "-").as_str() {
			"green" | "g" => Ok(Self::Green),
			"blue" | "b" => Ok(Self::Blue),
			"orange" | "o" => Ok(Self::Orange),
			"lavender-pink" | "lavender" | "pink" | "lp" | "p" => Ok(Self::LavenderPink),
			"teal" | "t" => Ok(Self::Teal),
			"maroon" | "m" => Ok(Self::Maroon),
			other => Err(format!(
				"unknown color '{other}' (green|blue|orange|lavender-pink|teal|maroon)"
			)),
		}
	}

	pub fn bit(self) -> u8 {
		1u8 << (self as u8)
	}
}

/// Draw one color from the 81/81/81/9/9/1 tree.
pub fn sample_color(rng: &mut impl Rng) -> TileColor {
	let mut roll = rng.gen_range(0..COLOR_WEIGHT_TOTAL);
	for &(color, w) in &COLOR_WEIGHTS {
		if roll < w {
			return color;
		}
		roll -= w;
	}
	TileColor::Maroon
}

/// Independent 1/`ZAG_DENOM` zag rolls. `zag_numer == 0` skips the RNG so a
/// `--zags off` bag matches the #17 (no-zag) deal for the same seed.
pub fn sample_zag_flags(rng: &mut impl Rng, n_rings: u8, zag_numer: u8) -> [bool; 3] {
	let mut zag = [false; 3];
	if zag_numer == 0 {
		return zag;
	}
	let numer = u32::from(zag_numer.min(ZAG_DENOM));
	for flag in zag.iter_mut().take(n_rings.clamp(1, MAX_COLOR_RINGS) as usize) {
		*flag = rng.gen_range(0..u32::from(ZAG_DENOM)) < numer;
	}
	zag
}

/// 1–3 distinct ring colors. Ring 1 is always drawn. Rings 2 and 3
/// keep the last existing color with weight 7, or draw any color with weight 1
/// (a redraw of an already-present color is not a new ring). Each ring is
/// then independently marked zag or normal.
pub fn sample_rings(rng: &mut impl Rng, max_rings: u8) -> ([TileColor; 3], u8) {
	let max_rings = max_rings.clamp(1, MAX_COLOR_RINGS);
	let r1 = sample_color(rng);
	let mut rings = [r1, r1, r1];
	let mut n = 1u8;
	if max_rings >= 2 && rng.gen_range(0..RING_EXIST_TOTAL) < RING_ANY_WEIGHT {
		let c = sample_color(rng);
		if c != r1 {
			rings[1] = c;
			n = 2;
		}
	}
	if max_rings >= 3 && rng.gen_range(0..RING_EXIST_TOTAL) < RING_ANY_WEIGHT {
		let last = rings[(n - 1) as usize];
		let c = sample_color(rng);
		if c != last && !rings[..n as usize].contains(&c) {
			rings[n as usize] = c;
			n += 1;
		}
	}
	(rings, n)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Tile {
	pub id: u32,
	pub owner: Player,
	/// Distinct-color rings in generation; tests may stack same-color zags.
	/// First [`Self::n_rings`] entries live.
	pub rings: [TileColor; 3],
	pub n_rings: u8,
	/// Parallel to [`Self::rings`]: that ring is a zag (forced mismatch).
	pub zag: [bool; 3],
	pub weight: u8,
	pub dots: u8,
}

/// Kind space: color-state (3^6) × weight × dots.
pub const KIND_COUNT: usize = COLOR_STATES * 9;

impl Tile {
	pub fn mono(id: u32, owner: Player, color: TileColor, weight: u8, dots: u8) -> Self {
		Self::with_rings(id, owner, &[color], weight, dots)
	}

	pub fn with_rings(id: u32, owner: Player, colors: &[TileColor], weight: u8, dots: u8) -> Self {
		let parts: Vec<(TileColor, bool)> = {
			let mut out = Vec::new();
			for &c in colors.iter().take(MAX_COLOR_RINGS as usize) {
				if !out.iter().any(|(x, _)| *x == c) {
					out.push((c, false));
				}
			}
			out
		};
		Self::with_ring_kinds(id, owner, &parts, weight, dots)
	}

	/// Rings in listed order, including duplicate colors (for stacked zags).
	pub fn with_ring_kinds(
		id: u32,
		owner: Player,
		parts: &[(TileColor, bool)],
		weight: u8,
		dots: u8,
	) -> Self {
		let first = parts.first().map(|p| p.0).unwrap_or(TileColor::Green);
		let mut rings = [first; 3];
		let mut zag = [false; 3];
		let mut n = 0u8;
		for &(c, is_zag) in parts.iter().take(MAX_COLOR_RINGS as usize) {
			rings[n as usize] = c;
			zag[n as usize] = is_zag;
			n += 1;
		}
		if n == 0 {
			n = 1;
		}
		Self { id, owner, rings, n_rings: n, zag, weight, dots }
	}

	pub fn ring_colors(&self) -> &[TileColor] {
		&self.rings[..self.n_rings.clamp(1, MAX_COLOR_RINGS) as usize]
	}

	pub fn ring_is_zag(self, index: usize) -> bool {
		index < self.n_rings.clamp(1, MAX_COLOR_RINGS) as usize && self.zag[index]
	}

	pub fn primary(self) -> TileColor {
		self.rings[0]
	}

	pub fn has_color(self, color: TileColor) -> bool {
		self.ring_colors().contains(&color)
	}

	pub fn has_normal(self, color: TileColor) -> bool {
		self.ring_colors().iter().enumerate().any(|(i, &c)| c == color && !self.zag[i])
	}

	/// Concentric strokes drawn for this tile: one per ring, extra weight
	/// strokes reuse the last ring (so a weight-3 one-ring zag shows three
	/// zag rings).
	pub fn ring_strokes(self) -> u8 {
		self.weight.max(self.n_rings).max(1)
	}

	fn stroke_idx(self, k: u8) -> usize {
		let n = self.n_rings.clamp(1, MAX_COLOR_RINGS) as usize;
		(k as usize).min(n.saturating_sub(1))
	}

	pub fn stroke_color(self, k: u8) -> TileColor {
		self.rings[self.stroke_idx(k)]
	}

	pub fn stroke_is_zag(self, k: u8) -> bool {
		self.zag[self.stroke_idx(k)]
	}

	pub fn color_mask(self) -> u8 {
		let mut m = 0u8;
		for &c in self.ring_colors() {
			m |= c.bit();
		}
		m.max(1)
	}

	pub fn normal_mask(self) -> u8 {
		let mut m = 0u8;
		for (i, &c) in self.ring_colors().iter().enumerate() {
			if !self.zag[i] {
				m |= c.bit();
			}
		}
		m
	}

	/// Any ring color, normal or zag.
	pub fn shares_color(self, other: Tile) -> bool {
		(self.color_mask() & other.color_mask()) != 0
	}

	/// Shared **normal** ring. Zags never produce a match. A color that
	/// either tile holds only as zags does not count.
	pub fn shares_normal(self, other: Tile) -> bool {
		(self.normal_mask() & other.normal_mask()) != 0
	}

	/// How many of this tile's zag **strokes** have their color on `target`
	/// (normal or zag). Extra weight strokes of a zag last-ring count, matching
	/// the concentric rings that are drawn. Own zags do not mismatch their owner.
	/// A zag whose color `target` lacks is absent (not a hit).
	pub fn zag_hits_on(self, target: Tile) -> i32 {
		(0..self.ring_strokes())
			.filter(|&k| self.stroke_is_zag(k) && target.has_color(self.stroke_color(k)))
			.count() as i32
	}

	/// True if this tile has a real presence toward `dst`: a normal ring, or a
	/// zag whose color `dst` holds. Non-matching zags are treated as empty.
	pub fn present_toward(self, dst: Tile) -> bool {
		self.normal_mask() != 0 || self.zag_hits_on(dst) > 0
	}

	pub fn kind_label(self) -> String {
		let rings = self
			.ring_colors()
			.iter()
			.enumerate()
			.map(
				|(i, c)| {
					if self.zag[i] {
						format!("{}-zag", c.name())
					} else {
						c.name().to_string()
					}
				},
			)
			.collect::<Vec<_>>()
			.join("+");
		format!("{rings} w{} d{}", self.weight, self.dots)
	}

	/// Per color: 0 absent, 1 normal, 2 zag. Duplicate-color zags collapse to 2.
	pub fn color_trits(self) -> [u8; 6] {
		let mut t = [0u8; 6];
		for (i, &c) in self.ring_colors().iter().enumerate() {
			let idx = c as usize;
			if idx >= 6 {
				continue;
			}
			if self.zag[i] {
				t[idx] = 2;
			} else if t[idx] == 0 {
				t[idx] = 1;
			}
		}
		t
	}

	/// `color_state * 9 + (weight-1)*3 + (dots-1)`.
	pub fn kind_index(self) -> usize {
		let w = (self.weight.clamp(1, 3) - 1) as usize;
		let d = (self.dots.clamp(1, 3) - 1) as usize;
		color_trits_state(self.color_trits()) * 9 + w * 3 + d
	}
}

pub fn color_trits_state(trits: [u8; 6]) -> usize {
	let mut state = 0usize;
	let mut pow = 1usize;
	for t in trits {
		state += (t as usize).min(2) * pow;
		pow *= 3;
	}
	state
}

/// `(normal_mask, zag_mask)` from a `kind / 9` color-state.
pub fn color_state_masks(state: usize) -> (u8, u8) {
	let mut s = state;
	let mut normal = 0u8;
	let mut zag = 0u8;
	for i in 0..6u8 {
		let trit = s % 3;
		s /= 3;
		match trit {
			1 => normal |= 1 << i,
			2 => zag |= 1 << i,
			_ => {}
		}
	}
	(normal, zag)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EndReason {
	Elimination,
	TurnLimit,
	DoublePass,
	/// Player to move had no legal ADD or MOVE (`no_moves_loses`).
	LockedOut,
	/// Occupied the center for N consecutive own turns (`center_hold_win`).
	CenterHold,
	/// Held a majority of inner-ring (petal) nodes for N consecutive own turns.
	PetalMajority,
	/// Occupied at least `control_n` of the 9-node control set (center + inner
	/// ring) for `control_hold_turns` consecutive own turns.
	Control,
	/// Could no longer occupy 5 control-set nodes (`insufficient_tiles_loss`).
	InsufficientTiles,
}

/// Restriction on moving the same tile on two consecutive own turns.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SamePieceTwice {
	Off,
	Always,
	WhenBenchEmpty,
}

impl SamePieceTwice {
	pub fn parse(s: &str) -> Result<Self, String> {
		match s.to_ascii_lowercase().as_str() {
			"off" | "false" | "0" => Ok(Self::Off),
			"always" | "on" | "true" => Ok(Self::Always),
			"when-bench-empty" | "when_bench_empty" | "bench-empty" => Ok(Self::WhenBenchEmpty),
			other => Err(format!(
				"unknown no-same-piece-twice mode '{other}' (expected off|always|when-bench-empty)"
			)),
		}
	}

	pub fn name(self) -> &'static str {
		match self {
			Self::Off => "off",
			Self::Always => "always",
			Self::WhenBenchEmpty => "when-bench-empty",
		}
	}
}

/// Path uniqueness constraint for a single MOVE.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PathConstraint {
	/// Current/legacy: a path may not revisit a node, including hopped nodes.
	NoNodeRevisit,
	/// A path may not traverse the same undirected circle-arc twice. Nodes may
	/// be revisited via a different arc. The landing node must be empty and
	/// differ from the start.
	NoEdgeRevisit,
	/// Both node and edge uniqueness.
	Both,
}

impl PathConstraint {
	pub fn parse(s: &str) -> Result<Self, String> {
		match s.to_ascii_lowercase().as_str() {
			"no-node-revisit" | "no_node_revisit" | "node" => Ok(Self::NoNodeRevisit),
			"no-edge-revisit" | "no_edge_revisit" | "edge" => Ok(Self::NoEdgeRevisit),
			"both" => Ok(Self::Both),
			other => Err(format!(
				"unknown path-constraint '{other}' (expected no-node-revisit|no-edge-revisit|both)"
			)),
		}
	}

	pub fn name(self) -> &'static str {
		match self {
			Self::NoNodeRevisit => "no-node-revisit",
			Self::NoEdgeRevisit => "no-edge-revisit",
			Self::Both => "both",
		}
	}

	pub fn forbids_node_revisit(self) -> bool {
		matches!(self, Self::NoNodeRevisit | Self::Both)
	}

	pub fn forbids_edge_revisit(self) -> bool {
		matches!(self, Self::NoEdgeRevisit | Self::Both)
	}
}

/// Weight comparison used when [`RuleSet::color_restricted_hops`] is on.
///
/// Interpretation of the designer's "if the weight in your color is less than,
/// you can traverse": hop only over the mover's color, comparing occupant
/// weight to mover weight with this operator. Default is `<=` (`Le`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum HopWeightCmp {
	Lt,
	Le,
}

impl HopWeightCmp {
	pub fn parse(s: &str) -> Result<Self, String> {
		match s.to_ascii_lowercase().as_str() {
			"lt" | "<" | "less" => Ok(Self::Lt),
			"le" | "<=" | "leq" | "less-equal" => Ok(Self::Le),
			other => Err(format!("unknown hop-weight-cmp '{other}' (expected lt|le)")),
		}
	}

	pub fn name(self) -> &'static str {
		match self {
			Self::Lt => "lt",
			Self::Le => "le",
		}
	}

	pub fn allows(self, occupant_weight: u8, mover_weight: u8) -> bool {
		match self {
			Self::Lt => occupant_weight < mover_weight,
			Self::Le => occupant_weight <= mover_weight,
		}
	}
}

/// Lose when board tiles + bench cannot reach the control-set occupancy
/// threshold (`control_n`, default 5). Default [`Self::Off`].
///
/// Bag draws happen only on a MOVE that captures, so "can they still get to
/// 5?" depends on whether future draws are counted:
///
/// - [`Self::Strict`]: lose as soon as `board + bench < 5`, ignoring the bag.
/// - [`Self::NoRecapture`]: lose when `board + bench < 5` **and** they cannot
///   gain a tile this turn: the bag is empty, capture-draw is off, or they
///   have no legal MOVE that would remove an opponent tile (previewed
///   overwhelm). Having a capturing MOVE with a non-empty bag saves them.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum InsufficientTilesLoss {
	Off,
	Strict,
	NoRecapture,
}

impl InsufficientTilesLoss {
	pub fn parse(s: &str) -> Result<Self, String> {
		match s.to_ascii_lowercase().as_str() {
			"off" | "false" | "0" => Ok(Self::Off),
			"strict" => Ok(Self::Strict),
			"no-recapture" | "no_recapture" | "norecapture" => Ok(Self::NoRecapture),
			other => Err(format!(
				"unknown insufficient-tiles-loss '{other}' (expected off|strict|no-recapture)"
			)),
		}
	}

	pub fn name(self) -> &'static str {
		match self {
			Self::Off => "off",
			Self::Strict => "strict",
			Self::NoRecapture => "no-recapture",
		}
	}

	pub fn is_on(self) -> bool {
		!matches!(self, Self::Off)
	}
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GameStatus {
	InProgress,
	Won { winner: Player, reason: EndReason },
	Draw { reason: EndReason },
}

impl GameStatus {
	pub fn is_over(self) -> bool {
		!matches!(self, GameStatus::InProgress)
	}
}

/// Configurable rules. Defaults match the redesign spec; each field is listed
/// in `rfc/rfc-001-001-001-rosette-redesign`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RuleSet {
	/// Starting tiles dealt from each bag onto the bench. Default 7.
	/// The bag is [`BAG_SIZE`] independently sampled tiles; a deal of 7
	/// leaves 20 capture draws. Empty bag skips the draw.
	pub starting_bench_size: u8,
	/// Both players receive the same shuffled [`BAG_SIZE`]-tile bag. Default true.
	pub identical_bags: bool,
	/// Fill the bag by sampling [`BAG_SIZE`] tiles (historical name: one-of-each
	/// 27-kind). Default true. False leaves an empty bag.
	pub bag_one_of_each_kind: bool,
	/// Max distinct rings per tile (1–3). Default 3. `1` is the
	/// single-ring baseline (still the six-color tree).
	pub max_rings: u8,
	/// Each ring is a zag with probability `zag_numer / 7`. Default 1 (1/7).
	/// `0` disables zags (`--zags off` / `--zag-chance 0`).
	pub zag_numer: u8,
	/// On a MOVE, each removed opponent tile draws one bag tile onto the mover's bench.
	pub capture_draw_on_move: bool,
	/// If true, a mismatch adds the neighbor's weight instead of 1. Default false.
	pub mismatch_uses_neighbor_weight: bool,
	/// Actions a player may take before the turn passes. Default 1.
	pub actions_per_turn: u8,
	/// Draw when this many full turns have been completed. Default 200.
	pub turn_limit: u32,
	/// ADD is only legal on empty spawn nodes. Default true.
	pub add_only_on_spawns: bool,
	/// MOVE must consume exactly `dots` steps. Default true.
	pub move_exact_steps: bool,
	/// Hopped tiles must have weight >= the mover. Default true.
	/// Ignored when [`Self::color_restricted_hops`] is on.
	pub hop_requires_ge_weight: bool,
	/// If the player to move has no legal ADD or MOVE, they lose immediately
	/// instead of passing. Default true. Double-pass draws are disabled while
	/// this is on (a pass is never legal).
	pub no_moves_loses: bool,
	/// Win by occupying the center at the end of `center_hold_n` consecutive
	/// own turns. Default false (optional; N = [`Self::center_hold_n`]).
	pub center_hold_win: bool,
	/// Consecutive own turns holding the center required to win. Default 5.
	pub center_hold_n: u8,
	/// Disallow moving the same tile on two consecutive own turns, with a
	/// capture exception. Default [`SamePieceTwice::WhenBenchEmpty`].
	pub no_same_piece_twice: SamePieceTwice,
	/// If true, a hop may only cross tiles of the mover's color, compared with
	/// [`Self::hop_weight_cmp`]. Default true.
	pub color_restricted_hops: bool,
	/// Occupant-vs-mover weight comparison used when color-restricted hops are
	/// on. Default [`HopWeightCmp::Le`] (occupant weight `<=` mover).
	pub hop_weight_cmp: HopWeightCmp,
	/// Win by occupying a majority of inner-ring nodes (petal attachments to
	/// the center; 5 of 8 on the default rosette) for `petal_majority_n`
	/// consecutive own turns. Experimental; default false.
	pub petal_majority_win: bool,
	/// Consecutive own turns holding a petal majority required to win. Default 1.
	pub petal_majority_n: u8,
	/// Win by occupying at least [`Self::control_n`] nodes of the control set
	/// (center + 8 inner-ring = 9) for [`Self::control_hold_turns`] consecutive
	/// own turns. Default true (k = 5, t = 1). Intended as a center-hold
	/// replacement so grabbing the center alone is not a rush to victory.
	pub control_win: bool,
	/// Control-set occupancy needed to start the hold. Default 5 of 9.
	pub control_n: u8,
	/// Consecutive own turns holding `control_n` control-set nodes. Default 1.
	pub control_hold_turns: u8,
	/// Lose when board + bench cannot reach 5 tiles (see
	/// [`InsufficientTilesLoss`]). Default off.
	pub insufficient_tiles_loss: InsufficientTilesLoss,
	/// Uniqueness constraint along a single MOVE path.
	/// Default [`PathConstraint::NoEdgeRevisit`] (`no_repeated_edge` on;
	/// node revisits via a different arc are allowed).
	pub path_constraint: PathConstraint,
}

impl Default for RuleSet {
	fn default() -> Self {
		Self {
			starting_bench_size: 7,
			identical_bags: true,
			bag_one_of_each_kind: true,
			max_rings: MAX_COLOR_RINGS,
			zag_numer: ZAG_NUMER_DEFAULT,
			capture_draw_on_move: true,
			mismatch_uses_neighbor_weight: false,
			actions_per_turn: 1,
			turn_limit: 200,
			add_only_on_spawns: true,
			move_exact_steps: true,
			hop_requires_ge_weight: true,
			no_moves_loses: true,
			center_hold_win: false,
			center_hold_n: 5,
			no_same_piece_twice: SamePieceTwice::WhenBenchEmpty,
			color_restricted_hops: true,
			hop_weight_cmp: HopWeightCmp::Le,
			petal_majority_win: false,
			petal_majority_n: 1,
			control_win: true,
			control_n: 5,
			control_hold_turns: 1,
			insufficient_tiles_loss: InsufficientTilesLoss::Off,
			path_constraint: PathConstraint::NoEdgeRevisit,
		}
	}
}

impl RuleSet {
	pub fn default_rules() -> Self {
		Self::default()
	}
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Action {
	Add { bench_index: usize, node: NodeId },
	Move { from: NodeId, to: NodeId },
	Pass,
}

impl Action {
	pub fn is_pass(self) -> bool {
		matches!(self, Action::Pass)
	}
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Removal {
	pub node: NodeId,
	pub tile: Tile,
	pub matches: i32,
	pub mismatches: i32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ActionOutcome {
	pub action: Action,
	pub player: Player,
	pub removals: Vec<Removal>,
	pub draws: u8,
	pub log_line: String,
}

#[derive(Clone, Debug)]
pub struct Game {
	board: Arc<Board>,
	rules: RuleSet,
	tiles: Vec<Option<Tile>>,
	benches: [Vec<Tile>; 2],
	bags: [Vec<Tile>; 2],
	current: Player,
	status: GameStatus,
	turn_count: u32,
	actions_this_turn: u8,
	consecutive_passes: u8,
	next_tile_id: u32,
	seed: u64,
	/// Tile id moved on each player's most recent completed turn.
	last_moved: [Option<u32>; 2],
	/// Tile id moved so far this turn (cleared when the turn ends).
	moved_this_turn: Option<u32>,
	/// Consecutive own turns each player has ended occupying the center.
	center_hold: [u8; 2],
	/// Consecutive own turns holding a majority of inner-ring (petal) nodes.
	petal_majority: [u8; 2],
	/// Consecutive own turns occupying at least `control_n` control-set nodes.
	control_hold: [u8; 2],
}

impl Game {
	pub fn new(rules: RuleSet, seed: u64) -> Self {
		Self::new_on(Arc::new(Board::rosette_8()), rules, seed)
	}

	pub fn start(
		rules: RuleSet,
		seed: u64,
		scenario: Option<&crate::scenario::Scenario>,
	) -> Result<Self, String> {
		match scenario {
			Some(s) => Self::from_scenario(rules, seed, s),
			None => Ok(Self::new(rules, seed)),
		}
	}

	/// Opening deal from a named or file-backed [`crate::scenario::Scenario`].
	pub fn from_scenario(
		rules: RuleSet,
		seed: u64,
		scenario: &crate::scenario::Scenario,
	) -> Result<Self, String> {
		if scenario.generative {
			let mut rules = rules;
			rules.identical_bags = scenario.mirror;
			return Ok(Self::new(rules, seed));
		}
		let board = Arc::new(Board::rosette_8());
		let n = board.node_count();
		let (bags, benches) = crate::scenario::materialize(scenario, seed)?;
		let mut next_tile_id = 1u32;
		let to_tiles = |owner: Player, kinds: Vec<crate::scenario::TileSpec>, next_id: &mut u32| {
			kinds
				.into_iter()
				.map(|spec| {
					let id = *next_id;
					*next_id += 1;
					Tile::with_ring_kinds(id, owner, &spec.parts(), spec.weight, spec.dots)
				})
				.collect::<Vec<_>>()
		};
		let p1_bag = to_tiles(Player::One, bags[0].clone(), &mut next_tile_id);
		let p2_bag = to_tiles(Player::Two, bags[1].clone(), &mut next_tile_id);
		let p1_bench = to_tiles(Player::One, benches[0].clone(), &mut next_tile_id);
		let p2_bench = to_tiles(Player::Two, benches[1].clone(), &mut next_tile_id);
		let dealt = !p1_bench.is_empty() || !p2_bench.is_empty();
		let mut game = Self {
			board,
			rules,
			tiles: vec![None; n],
			benches: [p1_bench, p2_bench],
			bags: [p1_bag, p2_bag],
			current: Player::One,
			status: GameStatus::InProgress,
			turn_count: 0,
			actions_this_turn: 0,
			consecutive_passes: 0,
			next_tile_id,
			seed,
			last_moved: [None, None],
			moved_this_turn: None,
			center_hold: [0, 0],
			petal_majority: [0, 0],
			control_hold: [0, 0],
		};
		if !dealt {
			game.deal_opening();
		}
		Ok(game)
	}

	pub fn new_on(board: Arc<Board>, rules: RuleSet, seed: u64) -> Self {
		let n = board.node_count();
		let mut next_tile_id = 1u32;
		let mut rng = StdRng::seed_from_u64(seed);
		let p1_bag = build_bag(
			Player::One,
			rules.bag_one_of_each_kind,
			rules.max_rings,
			rules.zag_numer,
			&mut next_tile_id,
			&mut rng,
		);
		let p2_bag = if rules.identical_bags {
			p1_bag
				.iter()
				.map(|t| {
					let id = next_tile_id;
					next_tile_id += 1;
					Tile { id, owner: Player::Two, ..*t }
				})
				.collect()
		} else {
			build_bag(
				Player::Two,
				rules.bag_one_of_each_kind,
				rules.max_rings,
				rules.zag_numer,
				&mut next_tile_id,
				&mut rng,
			)
		};
		let mut game = Self {
			board,
			rules,
			tiles: vec![None; n],
			benches: [Vec::new(), Vec::new()],
			bags: [p1_bag, p2_bag],
			current: Player::One,
			status: GameStatus::InProgress,
			turn_count: 0,
			actions_this_turn: 0,
			consecutive_passes: 0,
			next_tile_id,
			seed,
			last_moved: [None, None],
			moved_this_turn: None,
			center_hold: [0, 0],
			petal_majority: [0, 0],
			control_hold: [0, 0],
		};
		game.deal_opening();
		game
	}

	/// Empty board, empty benches and bags. For constructed test positions.
	pub fn empty(rules: RuleSet) -> Self {
		Self::empty_on(Arc::new(Board::rosette_8()), rules)
	}

	pub fn empty_on(board: Arc<Board>, rules: RuleSet) -> Self {
		let n = board.node_count();
		Self {
			board,
			rules,
			tiles: vec![None; n],
			benches: [Vec::new(), Vec::new()],
			bags: [Vec::new(), Vec::new()],
			current: Player::One,
			status: GameStatus::InProgress,
			turn_count: 0,
			actions_this_turn: 0,
			consecutive_passes: 0,
			next_tile_id: 1,
			seed: 0,
			last_moved: [None, None],
			moved_this_turn: None,
			center_hold: [0, 0],
			petal_majority: [0, 0],
			control_hold: [0, 0],
		}
	}

	fn deal_opening(&mut self) {
		let n = self.rules.starting_bench_size as usize;
		for p in [Player::One, Player::Two] {
			for _ in 0..n {
				let _ = self.draw_from_bag(p);
			}
		}
	}

	fn draw_from_bag(&mut self, player: Player) -> Option<Tile> {
		let bag = &mut self.bags[player.index()];
		let tile = bag.pop()?;
		self.benches[player.index()].push(tile);
		Some(tile)
	}

	pub fn board(&self) -> &Board {
		&self.board
	}

	pub fn rules(&self) -> &RuleSet {
		&self.rules
	}

	pub fn seed(&self) -> u64 {
		self.seed
	}

	pub fn current_player(&self) -> Player {
		self.current
	}

	pub fn status(&self) -> GameStatus {
		self.status
	}

	pub fn turn_count(&self) -> u32 {
		self.turn_count
	}

	pub fn actions_this_turn(&self) -> u8 {
		self.actions_this_turn
	}

	pub fn consecutive_passes(&self) -> u8 {
		self.consecutive_passes
	}

	/// Consecutive own turns `player` has ended occupying the center.
	pub fn center_hold(&self, player: Player) -> u8 {
		self.center_hold[player.index()]
	}

	/// Consecutive own turns `player` has ended holding a petal majority.
	pub fn petal_majority(&self, player: Player) -> u8 {
		self.petal_majority[player.index()]
	}

	/// How many inner-ring (petal-attachment) nodes `player` occupies.
	pub fn petal_count(&self, player: Player) -> u8 {
		self.board
			.inner_ring()
			.iter()
			.filter(|&&id| self.tile_at(id).map(|t| t.owner == player).unwrap_or(false))
			.count() as u8
	}

	/// Majority threshold: more than half of the inner-ring nodes (5 of 8).
	pub fn petal_majority_need(&self) -> u8 {
		let n = self.board.inner_ring().len();
		(n / 2 + 1) as u8
	}

	/// Consecutive own turns `player` has ended occupying `control_n` of the
	/// control set (center + inner ring).
	pub fn control_hold(&self, player: Player) -> u8 {
		self.control_hold[player.index()]
	}

	/// How many of the 9 control-set nodes (center + 8 inner-ring) `player`
	/// occupies.
	pub fn control_count(&self, player: Player) -> u8 {
		let mut n = self.petal_count(player);
		let center = self.board.center();
		if self.tile_at(center).map(|t| t.owner == player).unwrap_or(false) {
			n = n.saturating_add(1);
		}
		n
	}

	/// Occupancy needed to start a control hold (`control_n`, capped at the
	/// 9-node set).
	pub fn control_need(&self) -> u8 {
		let set = 1u8 + self.board.inner_ring().len() as u8;
		self.rules.control_n.max(1).min(set)
	}

	/// Board tiles + bench tiles. Bag tiles are not in play until drawn.
	pub fn reachable_tiles(&self, player: Player) -> usize {
		self.board_count(player) + self.bench(player).len()
	}

	#[cfg(test)]
	pub(crate) fn set_control_hold(&mut self, player: Player, n: u8) {
		self.control_hold[player.index()] = n;
	}

	/// Tile moved on `player`'s previous completed turn, if any.
	pub fn last_moved_tile(&self, player: Player) -> Option<u32> {
		self.last_moved[player.index()]
	}

	pub fn moved_this_turn(&self) -> Option<u32> {
		self.moved_this_turn
	}

	pub fn tile_at(&self, node: NodeId) -> Option<Tile> {
		self.tiles.get(node as usize).copied().flatten()
	}

	pub fn tiles(&self) -> impl Iterator<Item = (NodeId, Tile)> + '_ {
		self.tiles
			.iter()
			.enumerate()
			.filter_map(|(i, t)| t.map(|tile| (i as NodeId, tile)))
	}

	pub fn bench(&self, player: Player) -> &[Tile] {
		&self.benches[player.index()]
	}

	pub fn bag_len(&self, player: Player) -> usize {
		self.bags[player.index()].len()
	}

	pub fn bag(&self, player: Player) -> &[Tile] {
		&self.bags[player.index()]
	}

	/// Reorder remaining bag tiles. The multiset is unchanged; capture-draw
	/// order becomes the sampled permutation (determinization).
	pub fn shuffle_bags(&mut self, rng: &mut impl rand::Rng) {
		self.bags[0].shuffle(rng);
		self.bags[1].shuffle(rng);
	}

	pub fn board_count(&self, player: Player) -> usize {
		self.tiles
			.iter()
			.filter(|t| t.map(|x| x.owner == player).unwrap_or(false))
			.count()
	}

	pub fn is_alive(&self, player: Player) -> bool {
		self.board_count(player) > 0 || !self.benches[player.index()].is_empty()
	}

	pub fn set_current(&mut self, player: Player) {
		self.current = player;
	}

	#[cfg(test)]
	pub(crate) fn set_status(&mut self, status: GameStatus) {
		self.status = status;
	}

	#[cfg(test)]
	pub(crate) fn set_center_hold(&mut self, player: Player, n: u8) {
		self.center_hold[player.index()] = n;
	}

	pub fn place_tile(&mut self, node: NodeId, tile: Tile) {
		if tile.id >= self.next_tile_id {
			self.next_tile_id = tile.id + 1;
		}
		self.tiles[node as usize] = Some(tile);
	}

	pub fn clear_tile(&mut self, node: NodeId) -> Option<Tile> {
		self.tiles[node as usize].take()
	}

	pub fn set_bench(&mut self, player: Player, tiles: Vec<Tile>) {
		for t in &tiles {
			if t.id >= self.next_tile_id {
				self.next_tile_id = t.id + 1;
			}
		}
		self.benches[player.index()] = tiles;
	}

	pub fn set_bag(&mut self, player: Player, tiles: Vec<Tile>) {
		for t in &tiles {
			if t.id >= self.next_tile_id {
				self.next_tile_id = t.id + 1;
			}
		}
		self.bags[player.index()] = tiles;
	}

	pub fn alloc_tile(&mut self, owner: Player, color: TileColor, weight: u8, dots: u8) -> Tile {
		self.alloc_rings(owner, &[color], weight, dots)
	}

	pub fn alloc_rings(
		&mut self,
		owner: Player,
		colors: &[TileColor],
		weight: u8,
		dots: u8,
	) -> Tile {
		let id = self.next_tile_id;
		self.next_tile_id += 1;
		Tile::with_rings(id, owner, colors, weight, dots)
	}

	pub fn alloc_ring_kinds(
		&mut self,
		owner: Player,
		parts: &[(TileColor, bool)],
		weight: u8,
		dots: u8,
	) -> Tile {
		let id = self.next_tile_id;
		self.next_tile_id += 1;
		Tile::with_ring_kinds(id, owner, parts, weight, dots)
	}

	/// Match/mismatch this `tile` receives from neighbor `other`.
	/// Matches: +1 if they share a **normal** color. Zags never produce a
	/// match — a color that either tile holds only as zags does not count.
	/// Mismatches: count of `other`'s zag strokes whose color `tile` has
	/// (normal or zag). A zag whose color `tile` lacks is empty: no match,
	/// no mismatch, and it does not create a conflict. Ordinary +1 (or
	/// neighbor weight) mismatch only when both tiles are actually present
	/// toward each other and they share no normal color.
	pub fn edge_score(&self, tile: Tile, other: Tile) -> (i32, i32) {
		let matched = tile.shares_normal(other);
		let matches = i32::from(matched);
		let zag = other.zag_hits_on(tile);
		let mismatches = if zag > 0 {
			zag
		} else if matched {
			0
		} else if tile.present_toward(other) && other.present_toward(tile) {
			if self.rules.mismatch_uses_neighbor_weight {
				i32::from(other.weight)
			} else {
				1
			}
		} else {
			0
		};
		(matches, mismatches)
	}

	pub fn tile_score(&self, node: NodeId) -> Option<(i32, i32)> {
		let tile = self.tile_at(node)?;
		let mut matches = i32::from(tile.weight);
		let mut mismatches = 0i32;
		for &nb in self.board.neighbors(node) {
			if let Some(other) = self.tile_at(nb) {
				let (m, x) = self.edge_score(tile, other);
				matches += m;
				mismatches += x;
			}
		}
		Some((matches, mismatches))
	}

	pub fn is_overwhelmed(&self, node: NodeId) -> bool {
		match self.tile_score(node) {
			Some((m, n)) => n >= m,
			None => false,
		}
	}

	/// Would `tile` be overwhelmed if placed at `node` (other tiles unchanged)?
	pub fn would_overwhelm_add(&self, tile: Tile, node: NodeId) -> bool {
		if self.tile_at(node).is_some() {
			return false;
		}
		let mut matches = i32::from(tile.weight);
		let mut mismatches = 0i32;
		for &nb in self.board.neighbors(node) {
			if let Some(other) = self.tile_at(nb) {
				let (m, x) = self.edge_score(tile, other);
				matches += m;
				mismatches += x;
			}
		}
		mismatches >= matches
	}

	pub fn collect_removals(&self) -> Vec<Removal> {
		let mut out = Vec::new();
		for (node, tile) in self.tiles() {
			if let Some((matches, mismatches)) = self.tile_score(node) {
				if mismatches >= matches {
					out.push(Removal { node, tile, matches, mismatches });
				}
			}
		}
		out.sort_by_key(|r| r.node);
		out
	}

	pub fn legal_actions(&self) -> Vec<Action> {
		if self.status.is_over() {
			return Vec::new();
		}
		let mut acts = Vec::new();
		acts.extend(self.legal_adds());
		acts.extend(self.legal_moves());
		if acts.is_empty() && !self.rules.no_moves_loses {
			acts.push(Action::Pass);
		}
		acts
	}

	pub fn legal_adds(&self) -> Vec<Action> {
		if self.status.is_over() {
			return Vec::new();
		}
		let bench = &self.benches[self.current.index()];
		if bench.is_empty() {
			return Vec::new();
		}
		let mut dests = Vec::new();
		if self.rules.add_only_on_spawns {
			for &s in self.board.spawns() {
				if self.tile_at(s).is_none() {
					dests.push(s);
				}
			}
		} else {
			for id in 0..self.board.node_count() as NodeId {
				if self.tile_at(id).is_none() {
					dests.push(id);
				}
			}
		}
		let mut acts = Vec::new();
		for i in 0..bench.len() {
			for &node in &dests {
				acts.push(Action::Add { bench_index: i, node });
			}
		}
		acts
	}

	pub fn legal_moves(&self) -> Vec<Action> {
		if self.status.is_over() {
			return Vec::new();
		}
		let mut acts = Vec::new();
		for (node, tile) in self.tiles() {
			if tile.owner != self.current {
				continue;
			}
			let restricted = self.same_piece_is_restricted(tile.id);
			for dest in self.destinations(node) {
				if restricted && !self.move_removes_opponent(node, dest) {
					continue;
				}
				acts.push(Action::Move { from: node, to: dest });
			}
		}
		acts
	}

	fn same_piece_is_restricted(&self, tile_id: u32) -> bool {
		self.same_piece_is_restricted_for(self.current, tile_id)
	}

	fn same_piece_is_restricted_for(&self, player: Player, tile_id: u32) -> bool {
		if self.last_moved[player.index()] != Some(tile_id) {
			return false;
		}
		match self.rules.no_same_piece_twice {
			SamePieceTwice::Off => false,
			SamePieceTwice::Always => true,
			SamePieceTwice::WhenBenchEmpty => self.benches[player.index()].is_empty(),
		}
	}

	/// True if `player` has a legal MOVE (as if it were their turn) that would
	/// remove at least one opponent tile. Used by `no-recapture` insufficient-
	/// tiles: a capturing MOVE is the only way to draw from the bag.
	pub fn has_capturing_move(&self, player: Player) -> bool {
		for (node, tile) in self.tiles() {
			if tile.owner != player {
				continue;
			}
			for dest in self.destinations(node) {
				if self.move_removes_opponent(node, dest) {
					return true;
				}
			}
		}
		false
	}

	fn cannot_gain_tiles(&self, player: Player) -> bool {
		if self.bag_len(player) == 0 || !self.rules.capture_draw_on_move {
			return true;
		}
		!self.has_capturing_move(player)
	}

	fn is_insufficient(&self, player: Player) -> bool {
		let need = usize::from(self.control_need());
		if self.reachable_tiles(player) >= need {
			return false;
		}
		match self.rules.insufficient_tiles_loss {
			InsufficientTilesLoss::Off => false,
			InsufficientTilesLoss::Strict => true,
			InsufficientTilesLoss::NoRecapture => self.cannot_gain_tiles(player),
		}
	}

	/// True if moving the tile at `from` to `to` would remove at least one
	/// opponent tile under simultaneous overwhelm.
	pub fn move_removes_opponent(&self, from: NodeId, to: NodeId) -> bool {
		let Some(tile) = self.tile_at(from) else {
			return false;
		};
		let mut preview = self.clone();
		preview.tiles[from as usize] = None;
		preview.tiles[to as usize] = Some(tile);
		let opponent = tile.owner.other();
		preview.collect_removals().iter().any(|r| r.tile.owner == opponent)
	}

	pub fn has_add_or_move(&self) -> bool {
		!self.legal_adds().is_empty() || !self.legal_moves().is_empty()
	}

	/// Destinations that are legal for the tile at `from` under path rules
	/// *and* the same-piece restriction (including the capture exception).
	pub fn legal_destinations(&self, from: NodeId) -> Vec<NodeId> {
		let Some(tile) = self.tile_at(from) else {
			return Vec::new();
		};
		if tile.owner != self.current {
			return Vec::new();
		}
		let restricted = self.same_piece_is_restricted(tile.id);
		self.destinations(from)
			.into_iter()
			.filter(|&to| !restricted || self.move_removes_opponent(from, to))
			.collect()
	}

	/// Exact-`dots` destinations for the tile at `from`, with the mover vacated.
	pub fn destinations(&self, from: NodeId) -> Vec<NodeId> {
		let Some(tile) = self.tile_at(from) else {
			return Vec::new();
		};
		let steps = tile.dots.max(1);
		let mut occupancy = self.tiles.clone();
		occupancy[from as usize] = None;
		let forbid_node = self.rules.path_constraint.forbids_node_revisit();
		let forbid_edge = self.rules.path_constraint.forbids_edge_revisit();
		let mut found = Vec::new();
		let mut stack = vec![PathState {
			node: from,
			remaining: steps,
			visited_nodes: 1u32 << from,
			visited_edges: 0,
		}];
		while let Some(state) = stack.pop() {
			if state.remaining == 0 {
				if state.node != from && occupancy[state.node as usize].is_none() {
					if !found.contains(&state.node) {
						found.push(state.node);
					}
				}
				continue;
			}
			let circles: Vec<u8> = self.board.circles_through(state.node).collect();
			for circle in circles {
				for ccw in [true, false] {
					let Some(next) = self.board.next_on_circle(circle, state.node, ccw) else {
						continue;
					};
					let Some(edge) = self.board.undirected_arc_index(circle, state.node, next)
					else {
						continue;
					};
					if forbid_edge && state.visited_edges & (1u64 << edge) != 0 {
						continue;
					}
					if occupancy[next as usize].is_none() {
						if forbid_node && state.visited_nodes & (1u32 << next) != 0 {
							continue;
						}
						stack.push(PathState {
							node: next,
							remaining: state.remaining - 1,
							visited_nodes: state.visited_nodes | (1u32 << next),
							visited_edges: state.visited_edges | (1u64 << edge),
						});
					} else if let Some((land, hopped_nodes, hopped_edges)) =
						self.hop_along(&occupancy, circle, state.node, ccw, tile)
					{
						if forbid_edge && state.visited_edges & hopped_edges != 0 {
							continue;
						}
						if forbid_node
							&& (state.visited_nodes & hopped_nodes != 0
								|| state.visited_nodes & (1u32 << land) != 0)
						{
							continue;
						}
						stack.push(PathState {
							node: land,
							remaining: state.remaining - 1,
							visited_nodes: state.visited_nodes | hopped_nodes | (1u32 << land),
							visited_edges: state.visited_edges | hopped_edges,
						});
					}
				}
			}
		}
		found.sort();
		found
	}

	fn hop_occupant_ok(&self, occupant: Tile, mover: Tile) -> bool {
		if self.rules.color_restricted_hops {
			occupant.shares_normal(mover)
				&& self.rules.hop_weight_cmp.allows(occupant.weight, mover.weight)
		} else if self.rules.hop_requires_ge_weight {
			occupant.weight >= mover.weight
		} else {
			true
		}
	}

	fn hop_along(
		&self,
		occupancy: &[Option<Tile>],
		circle: u8,
		from: NodeId,
		ccw: bool,
		mover: Tile,
	) -> Option<(NodeId, u32, u64)> {
		let mut n = self.board.next_on_circle(circle, from, ccw)?;
		if occupancy[n as usize].is_none() {
			return None;
		}
		let mut hopped_nodes = 0u32;
		let mut hopped_edges = 0u64;
		let mut prev = from;
		let start = n;
		loop {
			let edge = self.board.undirected_arc_index(circle, prev, n)?;
			hopped_edges |= 1u64 << edge;
			let occupant = occupancy[n as usize]?;
			if !self.hop_occupant_ok(occupant, mover) {
				return None;
			}
			hopped_nodes |= 1u32 << n;
			prev = n;
			n = self.board.next_on_circle(circle, n, ccw)?;
			if n == start || n == from {
				return None;
			}
			if occupancy[n as usize].is_none() {
				let land_edge = self.board.undirected_arc_index(circle, prev, n)?;
				hopped_edges |= 1u64 << land_edge;
				return Some((n, hopped_nodes, hopped_edges));
			}
		}
	}

	pub fn apply(&mut self, action: Action) -> Result<ActionOutcome, String> {
		if self.status.is_over() {
			return Err("game is over".into());
		}
		let player = self.current;
		match action {
			Action::Add { bench_index, node } => self.apply_add(bench_index, node)?,
			Action::Move { from, to } => self.apply_move(from, to)?,
			Action::Pass => {
				if self.has_add_or_move() {
					return Err("pass is only legal when no ADD or MOVE exists".into());
				}
				if self.rules.no_moves_loses {
					self.status =
						GameStatus::Won { winner: player.other(), reason: EndReason::LockedOut };
				}
			}
		}

		if action.is_pass() {
			self.consecutive_passes = self.consecutive_passes.saturating_add(1);
		} else {
			self.consecutive_passes = 0;
		}

		let removals = self.collect_removals();
		let opponent = player.other();
		let mut draws = 0u8;
		let is_move = matches!(action, Action::Move { .. });
		for rem in &removals {
			self.tiles[rem.node as usize] = None;
			if is_move && self.rules.capture_draw_on_move && rem.tile.owner == opponent {
				if self.draw_from_bag(player).is_some() {
					draws = draws.saturating_add(1);
				}
			}
		}

		let log_line = format_log(player, action, &removals, draws, self.board.as_ref());

		self.check_elimination();
		if self.status == GameStatus::InProgress
			&& !self.rules.no_moves_loses
			&& self.consecutive_passes >= 2
		{
			self.status = GameStatus::Draw { reason: EndReason::DoublePass };
		}

		self.actions_this_turn = self.actions_this_turn.saturating_add(1);
		let cap = self.rules.actions_per_turn.max(1);
		let turn_done = action.is_pass()
			|| self.actions_this_turn >= cap
			|| self.status.is_over()
			|| !self.has_add_or_move();
		if turn_done && !self.status.is_over() {
			self.finish_turn(player);
		}

		Ok(ActionOutcome { action, player, removals, draws, log_line })
	}

	fn finish_turn(&mut self, who: Player) {
		self.last_moved[who.index()] = self.moved_this_turn.take();
		self.update_center_hold(who);
		if !self.status.is_over() {
			self.update_petal_majority(who);
		}
		if !self.status.is_over() {
			self.update_control(who);
		}
		if !self.status.is_over() {
			self.check_insufficient_tiles();
		}
		self.actions_this_turn = 0;
		self.turn_count = self.turn_count.saturating_add(1);
		if self.status.is_over() {
			return;
		}
		if self.turn_count >= self.rules.turn_limit {
			self.status = GameStatus::Draw { reason: EndReason::TurnLimit };
			return;
		}
		self.current = who.other();
		if self.rules.no_moves_loses && !self.has_add_or_move() {
			self.status = GameStatus::Won { winner: who, reason: EndReason::LockedOut };
		}
	}

	fn update_center_hold(&mut self, who: Player) {
		if !self.rules.center_hold_win {
			self.center_hold = [0, 0];
			return;
		}
		let center = self.board.center();
		let occupant = self.tile_at(center);
		if occupant.map(|t| t.owner == who).unwrap_or(false) {
			self.center_hold[who.index()] = self.center_hold[who.index()].saturating_add(1);
			if self.center_hold[who.index()] >= self.rules.center_hold_n.max(1) {
				self.status = GameStatus::Won { winner: who, reason: EndReason::CenterHold };
			}
		} else {
			self.center_hold[who.index()] = 0;
		}
		let opp = who.other();
		let opp_holds = occupant.map(|t| t.owner == opp).unwrap_or(false);
		if !opp_holds {
			self.center_hold[opp.index()] = 0;
		}
	}

	fn update_petal_majority(&mut self, who: Player) {
		if !self.rules.petal_majority_win {
			self.petal_majority = [0, 0];
			return;
		}
		let need = self.petal_majority_need();
		if self.petal_count(who) >= need {
			self.petal_majority[who.index()] = self.petal_majority[who.index()].saturating_add(1);
			if self.petal_majority[who.index()] >= self.rules.petal_majority_n.max(1) {
				self.status = GameStatus::Won { winner: who, reason: EndReason::PetalMajority };
			}
		} else {
			self.petal_majority[who.index()] = 0;
		}
		let opp = who.other();
		if self.petal_count(opp) < need {
			self.petal_majority[opp.index()] = 0;
		}
	}

	fn update_control(&mut self, who: Player) {
		if !self.rules.control_win {
			self.control_hold = [0, 0];
			return;
		}
		let need = self.control_need();
		if self.control_count(who) >= need {
			self.control_hold[who.index()] = self.control_hold[who.index()].saturating_add(1);
			if self.control_hold[who.index()] >= self.rules.control_hold_turns.max(1) {
				self.status = GameStatus::Won { winner: who, reason: EndReason::Control };
			}
		} else {
			self.control_hold[who.index()] = 0;
		}
		let opp = who.other();
		if self.control_count(opp) < need {
			self.control_hold[opp.index()] = 0;
		}
	}

	/// Both players are checked at the turn boundary. If both are insufficient
	/// at once, the game is a draw (`EndReason::InsufficientTiles`).
	fn check_insufficient_tiles(&mut self) {
		if !self.rules.insufficient_tiles_loss.is_on() || self.status.is_over() {
			return;
		}
		let p1 = self.is_insufficient(Player::One);
		let p2 = self.is_insufficient(Player::Two);
		match (p1, p2) {
			(true, true) => {
				self.status = GameStatus::Draw { reason: EndReason::InsufficientTiles };
			}
			(true, false) => {
				self.status =
					GameStatus::Won { winner: Player::Two, reason: EndReason::InsufficientTiles };
			}
			(false, true) => {
				self.status =
					GameStatus::Won { winner: Player::One, reason: EndReason::InsufficientTiles };
			}
			(false, false) => {}
		}
	}

	#[cfg(test)]
	pub(crate) fn evaluate_insufficient_tiles_for_test(&mut self) {
		self.check_insufficient_tiles();
	}

	fn apply_add(&mut self, bench_index: usize, node: NodeId) -> Result<(), String> {
		if self.rules.add_only_on_spawns && !self.board.is_spawn(node) {
			return Err("ADD is only legal on spawn nodes".into());
		}
		if self.tile_at(node).is_some() {
			return Err("destination is occupied".into());
		}
		let bench = &mut self.benches[self.current.index()];
		if bench_index >= bench.len() {
			return Err("bench index out of range".into());
		}
		let tile = bench.remove(bench_index);
		self.tiles[node as usize] = Some(tile);
		Ok(())
	}

	fn apply_move(&mut self, from: NodeId, to: NodeId) -> Result<(), String> {
		let tile = self.tile_at(from).ok_or_else(|| "no tile at source".to_string())?;
		if tile.owner != self.current {
			return Err("not your tile".into());
		}
		if !self.destinations(from).contains(&to) {
			return Err("illegal MOVE destination".into());
		}
		if self.same_piece_is_restricted(tile.id) && !self.move_removes_opponent(from, to) {
			return Err(
				"cannot move the same tile on two consecutive turns unless the move captures"
					.into(),
			);
		}
		self.tiles[from as usize] = None;
		self.tiles[to as usize] = Some(tile);
		self.moved_this_turn = Some(tile.id);
		Ok(())
	}

	fn check_elimination(&mut self) {
		if self.status.is_over() {
			return;
		}
		let p1 = self.is_alive(Player::One);
		let p2 = self.is_alive(Player::Two);
		match (p1, p2) {
			(false, false) => {
				self.status = GameStatus::Draw { reason: EndReason::Elimination };
			}
			(false, true) => {
				self.status =
					GameStatus::Won { winner: Player::Two, reason: EndReason::Elimination };
			}
			(true, false) => {
				self.status =
					GameStatus::Won { winner: Player::One, reason: EndReason::Elimination };
			}
			(true, true) => {}
		}
	}
}

#[derive(Clone, Copy)]
struct PathState {
	node: NodeId,
	remaining: u8,
	visited_nodes: u32,
	visited_edges: u64,
}

pub fn sample_tile(
	owner: Player,
	max_rings: u8,
	zag_numer: u8,
	next_id: &mut u32,
	rng: &mut impl Rng,
) -> Tile {
	let (rings, n_rings) = sample_rings(rng, max_rings);
	let zag = sample_zag_flags(rng, n_rings, zag_numer);
	let weight = rng.gen_range(1..=3);
	let dots = rng.gen_range(1..=3);
	let id = *next_id;
	*next_id += 1;
	Tile { id, owner, rings, n_rings, zag, weight, dots }
}

fn build_bag(
	owner: Player,
	fill: bool,
	max_rings: u8,
	zag_numer: u8,
	next_id: &mut u32,
	rng: &mut StdRng,
) -> Vec<Tile> {
	if !fill {
		return Vec::new();
	}
	let mut bag = Vec::with_capacity(BAG_SIZE);
	for _ in 0..BAG_SIZE {
		bag.push(sample_tile(owner, max_rings, zag_numer, next_id, rng));
	}
	bag.shuffle(rng);
	bag
}

fn format_log(
	player: Player,
	action: Action,
	removals: &[Removal],
	draws: u8,
	board: &Board,
) -> String {
	let act = match action {
		Action::Add { bench_index, node } => {
			format!("ADD bench[{bench_index}] to {}{}", node_tag(board, node), node)
		}
		Action::Move { from, to } => format!("MOVE {} -> {}", from, to),
		Action::Pass => "PASS".to_string(),
	};
	let mut line = format!("{} ({}) {}", player.label(), player.shade(), act);
	if !removals.is_empty() {
		let bits: Vec<String> = removals
			.iter()
			.map(|r| {
				format!(
					"{} {} at {} (matches={}, mismatches={})",
					r.tile.owner.label(),
					r.tile.kind_label(),
					r.node,
					r.matches,
					r.mismatches
				)
			})
			.collect();
		line.push_str("; removed ");
		line.push_str(&bits.join("; "));
	}
	if draws > 0 {
		line.push_str(&format!("; drew {draws} from bag"));
	}
	line
}

fn node_tag(board: &Board, node: NodeId) -> &'static str {
	if board.is_spawn(node) {
		"spawn "
	} else if node == board.center() {
		"center "
	} else {
		"node "
	}
}

#[cfg(test)]
mod tests {
	#![allow(clippy::unwrap_used, clippy::expect_used)]

	use super::*;
	use rand::rngs::StdRng;
	use rand::SeedableRng;

	fn green(game: &mut Game, owner: Player, weight: u8, dots: u8) -> Tile {
		game.alloc_tile(owner, TileColor::Green, weight, dots)
	}
	fn take(game: &mut Game, owner: Player, color: TileColor, weight: u8, dots: u8) -> Tile {
		game.alloc_tile(owner, color, weight, dots)
	}
	fn put(game: &mut Game, node: NodeId, owner: Player, color: TileColor, weight: u8, dots: u8) {
		let tile = take(game, owner, color, weight, dots);
		game.place_tile(node, tile);
	}

	fn stock_bench(game: &mut Game, player: Player, n: usize) {
		let colors = [TileColor::Green, TileColor::Blue, TileColor::Orange];
		let mut tiles = Vec::with_capacity(n);
		for i in 0..n {
			tiles.push(take(game, player, colors[i % 3], 1 + (i % 3) as u8, 1));
		}
		game.set_bench(player, tiles);
	}

	/// Center + 4 inner-ring tiles, all the same color so the center is not
	/// overwhelmed by four mismatch neighbors.
	fn occupy_control_five(game: &mut Game, player: Player) {
		let center = game.board().center();
		let inner = game.board().inner_ring().to_vec();
		put(game, center, player, TileColor::Green, 3, 1);
		for &node in inner.iter().take(4) {
			put(game, node, player, TileColor::Green, 3, 1);
		}
	}

	#[test]
	fn opening_deal_is_seven_from_identical_bags() {
		let a = Game::new(RuleSet::default(), 7);
		let b = Game::new(RuleSet::default(), 7);
		assert_eq!(a.bench(Player::One).len(), 7);
		assert_eq!(a.bench(Player::Two).len(), 7);
		assert_eq!(a.bag_len(Player::One), 20);
		assert_eq!(a.bag_len(Player::Two), 20);
		let kinds = |g: &Game, p: Player| {
			g.bench(p)
				.iter()
				.map(|t| (t.primary(), t.n_rings, t.weight, t.dots, t.color_mask()))
				.collect::<Vec<_>>()
		};
		assert_eq!(kinds(&a, Player::One), kinds(&b, Player::One));
		assert_eq!(kinds(&a, Player::One), kinds(&a, Player::Two));
		assert!(a.bench(Player::One).iter().all(|t| t.owner == Player::One));
		assert!(a.bench(Player::Two).iter().all(|t| t.owner == Player::Two));
	}

	#[test]
	fn opening_deal_respects_starting_bench_size() {
		let mut rules = RuleSet::default();
		rules.starting_bench_size = 5;
		let game = Game::new(rules, 1);
		assert_eq!(game.bench(Player::One).len(), 5);
		assert_eq!(game.bench(Player::Two).len(), 5);
		assert_eq!(game.bag_len(Player::One), 22);
		assert_eq!(game.bag_len(Player::Two), 22);
		assert_eq!(game.legal_adds().len(), 5 * 8);
	}

	#[test]
	fn default_rules_are_control_win_with_bench_seven() {
		let rules = RuleSet::default();
		assert!(!rules.center_hold_win);
		assert!(rules.control_win);
		assert_eq!(rules.control_n, 5);
		assert_eq!(rules.control_hold_turns, 1);
		assert_eq!(rules.starting_bench_size, 7);
		assert_eq!(rules.max_rings, 3);
		assert_eq!(rules.zag_numer, ZAG_NUMER_DEFAULT);
		assert!(!rules.petal_majority_win);
		assert_eq!(rules.insufficient_tiles_loss, InsufficientTilesLoss::Off);
		let game = Game::new(rules, 1);
		assert_eq!(game.bench(Player::One).len(), 7);
		assert_eq!(game.bench(Player::Two).len(), 7);
		assert_eq!(game.bag_len(Player::One), 20);
		assert_eq!(game.legal_adds().len(), 7 * 8);
	}

	#[test]
	fn double_green_overwhelmed_by_two_oranges_survives_with_green_neighbor() {
		let mut game = Game::empty(RuleSet::default());
		let spawn = game.board().spawns()[0];
		let nbs = game.board().neighbors(spawn).to_vec();
		assert!(nbs.len() >= 3);
		put(&mut game, spawn, Player::One, TileColor::Green, 2, 1);
		put(&mut game, nbs[0], Player::Two, TileColor::Orange, 1, 1);
		put(&mut game, nbs[1], Player::One, TileColor::Orange, 3, 2);
		let (m, n) = game.tile_score(spawn).unwrap();
		assert_eq!((m, n), (2, 2), "own weight 2, two different-color neighbors");
		assert!(game.is_overwhelmed(spawn));

		put(&mut game, nbs[2], Player::Two, TileColor::Green, 1, 3);
		let (m, n) = game.tile_score(spawn).unwrap();
		assert_eq!((m, n), (3, 2), "double green plus one green neighbor");
		assert!(!game.is_overwhelmed(spawn));
	}

	#[test]
	fn overwhelm_is_by_color_not_owner() {
		let mut game = Game::empty(RuleSet::default());
		let spawn = game.board().spawns()[1];
		let nbs = game.board().neighbors(spawn).to_vec();
		put(&mut game, spawn, Player::One, TileColor::Green, 1, 1);
		put(&mut game, nbs[0], Player::Two, TileColor::Green, 3, 1);
		let (m, n) = game.tile_score(spawn).unwrap();
		assert_eq!(m, 2);
		assert_eq!(n, 0);
		assert!(!game.is_overwhelmed(spawn));
	}

	#[test]
	fn mismatch_weight_option_counts_neighbor_weight() {
		let mut rules = RuleSet::default();
		rules.mismatch_uses_neighbor_weight = true;
		let mut game = Game::empty(rules);
		let spawn = game.board().spawns()[2];
		let nbs = game.board().neighbors(spawn).to_vec();
		put(&mut game, spawn, Player::One, TileColor::Green, 3, 1);
		put(&mut game, nbs[0], Player::Two, TileColor::Orange, 3, 1);
		let (m, n) = game.tile_score(spawn).unwrap();
		assert_eq!(m, 3);
		assert_eq!(n, 3);
		assert!(game.is_overwhelmed(spawn));
	}

	#[test]
	fn simultaneous_removal_logs_owner_node_and_counts() {
		let mut game = Game::empty(RuleSet::default());
		let s0 = game.board().spawns()[0];
		let s1 = game.board().spawns()[1];
		let n0 = game.board().neighbors(s0).to_vec();
		let shared = *n0.iter().find(|n| game.board().neighbors(s1).contains(n)).unwrap_or(&n0[0]);
		put(&mut game, s0, Player::One, TileColor::Green, 1, 1);
		put(&mut game, n0[0], Player::Two, TileColor::Orange, 1, 1);
		if n0.len() > 1 {
			put(&mut game, n0[1], Player::Two, TileColor::Orange, 1, 1);
		}
		let _ = shared;
		let _ = s1;
		game.set_current(Player::One);
		{
			let _t = take(&mut game, Player::One, TileColor::Blue, 1, 1);
			game.set_bench(Player::One, vec![_t]);
		};
		let empty_spawn = game
			.board()
			.spawns()
			.iter()
			.copied()
			.find(|&s| game.tile_at(s).is_none())
			.unwrap();
		let outcome = game.apply(Action::Add { bench_index: 0, node: empty_spawn }).unwrap();
		assert!(game.collect_removals().is_empty(), "removals already applied");
		for rem in &outcome.removals {
			assert!(rem.mismatches >= rem.matches);
			assert!(outcome.log_line.contains(&format!("matches={}", rem.matches)));
			assert!(outcome.log_line.contains(&format!("mismatches={}", rem.mismatches)));
			assert!(outcome.log_line.contains(&format!("at {}", rem.node)));
		}
	}

	#[test]
	fn move_is_exactly_d_steps_not_up_to_d() {
		let mut game = Game::empty(RuleSet::default());
		let from = game.board().spawns()[0];
		let mut step1 = 0u8;
		let mut step2 = 0u8;
		let mut found = false;
		for circle in game.board().circles_through(from) {
			for ccw in [true, false] {
				let a = game.board().next_on_circle(circle, from, ccw).unwrap();
				let b = game.board().next_on_circle(circle, a, ccw).unwrap();
				if !game.board().neighbors(from).contains(&b) {
					step1 = a;
					step2 = b;
					found = true;
					break;
				}
			}
			if found {
				break;
			}
		}
		assert!(found, "spawn should have a 2-step circle walk that is not a direct neighbor");
		let tile = green(&mut game, Player::One, 1, 2);
		game.place_tile(from, tile);
		game.set_current(Player::One);
		let dests2 = game.destinations(from);
		assert!(
			dests2.contains(&step2),
			"d=2 must reach the two-step node {step2}, got {dests2:?}"
		);
		game.clear_tile(from);
		let one = green(&mut game, Player::Two, 1, 1);
		game.place_tile(from, one);
		game.set_current(Player::Two);
		let dests1 = game.destinations(from);
		assert!(dests1.contains(&step1), "d=1 should reach {step1}");
		assert!(!dests1.contains(&step2), "d=1 must not reach the two-step node {step2}");
		assert_ne!(dests1, dests2, "exact-d MOVE: d=1 and d=2 destination sets differ");
	}

	#[test]
	fn move_cannot_revisit_a_node() {
		let mut game = Game::empty(RuleSet::default());
		let from = game.board().spawns()[3];
		put(&mut game, from, Player::One, TileColor::Blue, 1, 2);
		game.set_current(Player::One);
		assert!(!game.destinations(from).contains(&from));
	}

	#[test]
	fn hop_costs_one_and_requires_weight() {
		let mut rules = RuleSet::default();
		rules.color_restricted_hops = false;
		let mut game = Game::empty(rules);
		let b = game.board().clone();
		let (from, hop_over, land) = {
			let circle = 0u8;
			let cycle = b.circle_cycle(circle);
			(cycle[0], cycle[1], cycle[2])
		};
		put(&mut game, from, Player::One, TileColor::Green, 1, 1);
		put(&mut game, hop_over, Player::Two, TileColor::Orange, 2, 1);
		game.set_current(Player::One);
		let dests = game.destinations(from);
		assert!(dests.contains(&land), "d=1 hop over heavier/equal tile to {land}, got {dests:?}");

		game.clear_tile(hop_over);
		put(&mut game, hop_over, Player::Two, TileColor::Orange, 1, 1);
		game.clear_tile(from);
		put(&mut game, from, Player::One, TileColor::Green, 3, 1);
		game.set_current(Player::One);
		let dests = game.destinations(from);
		assert!(!dests.contains(&land), "cannot hop a lighter tile");
	}

	#[test]
	fn hop_along_a_run_of_equal_or_greater_weight() {
		let mut rules = RuleSet::default();
		rules.color_restricted_hops = false;
		let mut game = Game::empty(rules);
		let b = game.board().clone();
		let circle = 0u8;
		let cycle = b.circle_cycle(circle).to_vec();
		let from = cycle[0];
		let a = cycle[1];
		let c = cycle[2];
		let land = cycle[3];
		put(&mut game, from, Player::One, TileColor::Blue, 2, 1);
		put(&mut game, a, Player::Two, TileColor::Green, 2, 1);
		put(&mut game, c, Player::One, TileColor::Orange, 3, 1);
		game.set_current(Player::One);
		let dests = game.destinations(from);
		assert!(dests.contains(&land), "hop the two-tile run to {land}");
	}

	#[test]
	fn add_only_on_empty_spawns_and_suicidal_add_is_legal() {
		let mut game = Game::empty(RuleSet::default());
		let spawn = game.board().spawns()[0];
		let nbs = game.board().neighbors(spawn).to_vec();
		put(&mut game, nbs[0], Player::Two, TileColor::Orange, 1, 1);
		put(&mut game, nbs[1], Player::Two, TileColor::Orange, 1, 1);
		let tile = green(&mut game, Player::One, 1, 1);
		assert!(game.would_overwhelm_add(tile, spawn));
		game.set_bench(Player::One, vec![tile]);
		game.set_current(Player::One);
		let outcome = game.apply(Action::Add { bench_index: 0, node: spawn }).unwrap();
		assert!(outcome.removals.iter().any(|r| r.node == spawn));
		assert!(game.tile_at(spawn).is_none());
	}

	#[test]
	fn capture_draws_on_move_not_on_add() {
		let mut game = Game::empty(RuleSet::default());
		let spawn = game.board().spawns()[0];
		let nbs = game.board().neighbors(spawn).to_vec();
		put(&mut game, nbs[0], Player::Two, TileColor::Orange, 1, 1);
		put(&mut game, nbs[1], Player::One, TileColor::Blue, 1, 1);
		let extra = game.alloc_tile(Player::One, TileColor::Green, 3, 3);
		game.set_bag(Player::One, vec![extra]);
		{
			let _t = take(&mut game, Player::One, TileColor::Green, 1, 1);
			game.set_bench(Player::One, vec![_t]);
		};
		game.set_current(Player::One);
		let empty = game
			.board()
			.spawns()
			.iter()
			.copied()
			.find(|&s| game.tile_at(s).is_none())
			.unwrap();
		let bag_before = game.bag_len(Player::One);
		let outcome = game.apply(Action::Add { bench_index: 0, node: empty }).unwrap();
		assert_eq!(game.bag_len(Player::One), bag_before, "ADD captures do not draw");
		assert_eq!(outcome.draws, 0);
	}

	#[test]
	fn move_capture_draws_one_per_opponent_tile() {
		let mut game = Game::empty(RuleSet::default());
		let spawn = game.board().spawns()[0];
		let nbs = game.board().neighbors(spawn).to_vec();
		assert!(nbs.len() >= 3);
		let land = nbs[0];
		let from = *game
			.board()
			.neighbors(land)
			.iter()
			.find(|n| **n != spawn && !nbs.contains(n))
			.unwrap_or(&nbs[2]);
		put(&mut game, spawn, Player::Two, TileColor::Green, 2, 1);
		put(&mut game, nbs[1], Player::One, TileColor::Orange, 1, 1);
		put(&mut game, from, Player::One, TileColor::Blue, 3, 1);
		assert!(game.destinations(from).contains(&land), "w3 d=1 should step to empty neighbor");
		let extra = game.alloc_tile(Player::One, TileColor::Orange, 2, 2);
		game.set_bag(Player::One, vec![extra]);
		game.set_current(Player::One);
		let bag_before = game.bag_len(Player::One);
		let bench_before = game.bench(Player::One).len();
		let (m, n) = {
			let mut preview = game.clone();
			preview.tiles[from as usize] = None;
			preview.tiles[land as usize] = game.tile_at(from);
			preview.tile_score(spawn).unwrap()
		};
		assert!(n >= m, "landing a different color should overwhelm the double green");
		let outcome = game.apply(Action::Move { from, to: land }).unwrap();
		assert!(
			outcome.removals.iter().any(|r| r.node == spawn && r.tile.owner == Player::Two),
			"opponent tile should be removed: {:?}",
			outcome.removals
		);
		assert_eq!(outcome.draws, 1);
		assert_eq!(game.bag_len(Player::One), bag_before - 1);
		assert_eq!(game.bench(Player::One).len(), bench_before + 1);
	}

	#[test]
	fn win_when_opponent_has_empty_board_and_empty_bench() {
		let mut game = Game::empty(RuleSet::default());
		let spawn = game.board().spawns()[0];
		put(&mut game, spawn, Player::One, TileColor::Green, 3, 1);
		{
			let _t = take(&mut game, Player::One, TileColor::Blue, 1, 1);
			game.set_bench(Player::One, vec![_t]);
		};
		game.set_current(Player::Two);
		game.set_bench(Player::Two, vec![]);
		assert!(!game.is_alive(Player::Two));
		game.set_current(Player::One);
		let empty = game.board().spawns()[1];
		let outcome = game.apply(Action::Add { bench_index: 0, node: empty });
		assert!(outcome.is_ok());
		assert_eq!(
			game.status(),
			GameStatus::Won { winner: Player::One, reason: EndReason::Elimination }
		);
	}

	#[test]
	fn remaining_bag_does_not_keep_a_player_alive() {
		let mut game = Game::empty(RuleSet::default());
		{
			let _t = take(&mut game, Player::Two, TileColor::Orange, 1, 1);
			game.set_bag(Player::Two, vec![_t]);
		};
		game.set_bench(Player::Two, vec![]);
		assert!(!game.is_alive(Player::Two));
	}

	fn fill_board_all_green(game: &mut Game) {
		let n = game.board().node_count();
		for id in 0..n as NodeId {
			let owner = if id % 2 == 0 { Player::One } else { Player::Two };
			put(game, id, owner, TileColor::Green, 3, 1);
		}
	}

	#[test]
	fn double_pass_is_a_draw() {
		let mut rules = RuleSet::default();
		rules.no_moves_loses = false;
		rules.control_win = false;
		let mut game = Game::empty(rules);
		fill_board_all_green(&mut game);
		assert!(!game.has_add_or_move());
		game.apply(Action::Pass).unwrap();
		game.apply(Action::Pass).unwrap();
		assert_eq!(game.status(), GameStatus::Draw { reason: EndReason::DoublePass });
	}

	#[test]
	fn turn_limit_is_a_draw() {
		let mut rules = RuleSet::default();
		rules.no_moves_loses = false;
		rules.control_win = false;
		rules.turn_limit = 2;
		let mut game = Game::empty(rules);
		fill_board_all_green(&mut game);
		game.apply(Action::Pass).unwrap();
		assert_eq!(game.status(), GameStatus::InProgress);
		game.apply(Action::Pass).unwrap();
		assert!(matches!(
			game.status(),
			GameStatus::Draw { reason: EndReason::DoublePass }
				| GameStatus::Draw { reason: EndReason::TurnLimit }
		));
	}

	#[test]
	fn turn_limit_without_double_pass() {
		let mut rules = RuleSet::default();
		rules.turn_limit = 1;
		let mut game = Game::empty(rules);
		let spawn = game.board().spawns()[0];
		{
			let _t = take(&mut game, Player::One, TileColor::Green, 3, 1);
			game.set_bench(Player::One, vec![_t]);
		};
		{
			let _t = take(&mut game, Player::Two, TileColor::Orange, 3, 1);
			game.set_bench(Player::Two, vec![_t]);
		};
		game.apply(Action::Add { bench_index: 0, node: spawn }).unwrap();
		assert_eq!(game.status(), GameStatus::Draw { reason: EndReason::TurnLimit });
	}

	#[test]
	fn pass_rejected_when_add_exists() {
		let mut game = Game::empty(RuleSet::default());
		{
			let _t = take(&mut game, Player::One, TileColor::Green, 1, 1);
			game.set_bench(Player::One, vec![_t]);
		};
		assert!(game.apply(Action::Pass).is_err());
	}

	#[test]
	fn new_game_has_legal_adds_on_all_spawns() {
		let game = Game::new(RuleSet::default(), 1);
		let adds = game.legal_adds();
		assert_eq!(adds.len(), 7 * 8);
		assert!(game.legal_moves().is_empty());
	}

	fn inner_ring(game: &Game) -> Vec<NodeId> {
		let b = game.board();
		let r = b.construction_radius();
		let target = 2.0 * r * (3.0 * std::f64::consts::PI / 8.0).cos();
		(0..b.node_count() as NodeId)
			.filter(|&id| (b.radius_of(id) - target).abs() < 0.05 * r)
			.collect()
	}

	#[test]
	fn locked_down_player_loses_instead_of_passing() {
		let mut rules = RuleSet::default();
		rules.control_win = false;
		let mut game = Game::empty(rules);
		assert!(game.rules().no_moves_loses);
		fill_board_all_green(&mut game);
		assert!(!game.has_add_or_move());
		assert!(game.legal_actions().is_empty());
		game.set_current(Player::One);
		game.apply(Action::Pass).unwrap();
		assert_eq!(
			game.status(),
			GameStatus::Won { winner: Player::Two, reason: EndReason::LockedOut }
		);
	}

	#[test]
	fn opponent_is_locked_out_immediately_after_turn_switch() {
		let mut rules = RuleSet::default();
		rules.no_moves_loses = true;
		rules.control_win = false;
		let mut game = Game::empty(rules);
		let spawns = game.board().spawns().to_vec();
		// Fill every node except spawn 0 so P1 can ADD and then P2 has nowhere
		// to ADD and no empty landing for a MOVE.
		for id in 0..game.board().node_count() as NodeId {
			if id == spawns[0] {
				continue;
			}
			let owner = if id == spawns[1] { Player::Two } else { Player::One };
			put(&mut game, id, owner, TileColor::Green, 3, 1);
		}
		{
			let t = take(&mut game, Player::One, TileColor::Green, 3, 1);
			game.set_bench(Player::One, vec![t]);
		}
		game.set_bench(Player::Two, vec![]);
		game.set_current(Player::One);
		assert!(game.has_add_or_move());
		game.apply(Action::Add { bench_index: 0, node: spawns[0] }).unwrap();
		assert_eq!(
			game.status(),
			GameStatus::Won { winner: Player::One, reason: EndReason::LockedOut }
		);
	}

	#[test]
	fn center_hold_wins_at_exactly_n() {
		let mut rules = RuleSet::default();
		rules.center_hold_win = true;
		rules.center_hold_n = 2;
		let mut game = Game::empty(rules);
		let center = game.board().center();
		let inner = inner_ring(&game);
		let from = inner[0];
		put(&mut game, from, Player::One, TileColor::Blue, 3, 1);
		{
			let t = take(&mut game, Player::One, TileColor::Green, 3, 1);
			game.set_bench(Player::One, vec![t]);
		}
		{
			let t = take(&mut game, Player::Two, TileColor::Orange, 3, 1);
			game.set_bench(Player::Two, vec![t]);
		}
		game.set_current(Player::One);
		assert!(game.destinations(from).contains(&center));
		game.apply(Action::Move { from, to: center }).unwrap();
		assert_eq!(game.center_hold(Player::One), 1);
		assert_eq!(game.status(), GameStatus::InProgress);

		let p2_spawn = game
			.board()
			.spawns()
			.iter()
			.copied()
			.find(|&s| game.tile_at(s).is_none())
			.unwrap();
		game.apply(Action::Add { bench_index: 0, node: p2_spawn }).unwrap();
		assert_eq!(game.center_hold(Player::One), 1, "P2's turn must not increment P1");
		assert_eq!(game.center_hold(Player::Two), 0);

		let p1_spawn = game
			.board()
			.spawns()
			.iter()
			.copied()
			.find(|&s| game.tile_at(s).is_none())
			.unwrap();
		game.apply(Action::Add { bench_index: 0, node: p1_spawn }).unwrap();
		assert_eq!(game.center_hold(Player::One), 2);
		assert_eq!(
			game.status(),
			GameStatus::Won { winner: Player::One, reason: EndReason::CenterHold }
		);
	}

	#[test]
	fn center_hold_resets_when_vacated() {
		let mut rules = RuleSet::default();
		rules.center_hold_win = true;
		rules.center_hold_n = 5;
		rules.no_same_piece_twice = SamePieceTwice::Off;
		let mut game = Game::empty(rules);
		let center = game.board().center();
		let inner = inner_ring(&game);
		let from = inner[0];
		put(&mut game, from, Player::One, TileColor::Blue, 3, 1);
		put(&mut game, inner[1], Player::One, TileColor::Green, 3, 1);
		{
			let t = take(&mut game, Player::Two, TileColor::Orange, 3, 1);
			game.set_bench(Player::Two, vec![t]);
		}
		game.set_current(Player::One);
		game.apply(Action::Move { from, to: center }).unwrap();
		assert_eq!(game.center_hold(Player::One), 1);
		let p2_spawn = game.board().spawns()[0];
		game.apply(Action::Add { bench_index: 0, node: p2_spawn }).unwrap();
		let leave = game.destinations(center);
		assert!(!leave.is_empty(), "center piece must be able to step off");
		game.apply(Action::Move { from: center, to: leave[0] }).unwrap();
		assert_eq!(game.center_hold(Player::One), 0);
		assert_eq!(game.status(), GameStatus::InProgress);
	}

	#[test]
	fn petal_majority_wins_at_end_of_turn_when_n_is_one() {
		let mut rules = RuleSet::default();
		rules.center_hold_win = false;
		rules.control_win = false;
		rules.petal_majority_win = true;
		rules.petal_majority_n = 1;
		let mut game = Game::empty(rules);
		let inner = game.board().inner_ring().to_vec();
		assert_eq!(inner.len(), 8);
		assert_eq!(game.petal_majority_need(), 5);
		for &node in inner.iter().take(5) {
			put(&mut game, node, Player::One, TileColor::Green, 3, 1);
		}
		{
			let t = take(&mut game, Player::One, TileColor::Green, 2, 1);
			game.set_bench(Player::One, vec![t]);
		}
		{
			let t = take(&mut game, Player::Two, TileColor::Orange, 3, 1);
			game.set_bench(Player::Two, vec![t]);
		}
		game.set_current(Player::One);
		let spawn = game
			.board()
			.spawns()
			.iter()
			.copied()
			.find(|&s| game.tile_at(s).is_none())
			.unwrap();
		game.apply(Action::Add { bench_index: 0, node: spawn }).unwrap();
		assert_eq!(game.petal_count(Player::One), 5);
		assert_eq!(
			game.status(),
			GameStatus::Won { winner: Player::One, reason: EndReason::PetalMajority }
		);
	}

	#[test]
	fn petal_majority_n_three_requires_three_own_turns() {
		let mut rules = RuleSet::default();
		rules.center_hold_win = false;
		rules.control_win = false;
		rules.petal_majority_win = true;
		rules.petal_majority_n = 3;
		rules.no_same_piece_twice = SamePieceTwice::Off;
		let mut game = Game::empty(rules);
		let inner = game.board().inner_ring().to_vec();
		for &node in inner.iter().take(5) {
			put(&mut game, node, Player::One, TileColor::Green, 3, 1);
		}
		{
			let t = take(&mut game, Player::One, TileColor::Green, 2, 1);
			game.set_bench(Player::One, vec![t]);
		}
		{
			let t = take(&mut game, Player::Two, TileColor::Orange, 3, 1);
			game.set_bench(Player::Two, vec![t]);
		}
		game.set_current(Player::One);
		let p1_spawn = game
			.board()
			.spawns()
			.iter()
			.copied()
			.find(|&s| game.tile_at(s).is_none())
			.unwrap();
		game.apply(Action::Add { bench_index: 0, node: p1_spawn }).unwrap();
		assert_eq!(game.petal_majority(Player::One), 1);
		assert_eq!(game.status(), GameStatus::InProgress);

		let p2_spawn = game
			.board()
			.spawns()
			.iter()
			.copied()
			.find(|&s| game.tile_at(s).is_none())
			.unwrap();
		game.apply(Action::Add { bench_index: 0, node: p2_spawn }).unwrap();
		assert_eq!(game.petal_majority(Player::One), 1, "P2's turn must not increment P1");

		{
			let t = take(&mut game, Player::One, TileColor::Blue, 2, 1);
			game.set_bench(Player::One, vec![t]);
		}
		let p1b = game
			.board()
			.spawns()
			.iter()
			.copied()
			.find(|&s| game.tile_at(s).is_none())
			.unwrap();
		game.apply(Action::Add { bench_index: 0, node: p1b }).unwrap();
		assert_eq!(game.petal_majority(Player::One), 2);

		{
			let t = take(&mut game, Player::Two, TileColor::Green, 1, 1);
			game.set_bench(Player::Two, vec![t]);
		}
		let p2b = game
			.board()
			.spawns()
			.iter()
			.copied()
			.find(|&s| game.tile_at(s).is_none())
			.unwrap();
		game.apply(Action::Add { bench_index: 0, node: p2b }).unwrap();

		{
			let t = take(&mut game, Player::One, TileColor::Orange, 1, 1);
			game.set_bench(Player::One, vec![t]);
		}
		let p1c = game
			.board()
			.spawns()
			.iter()
			.copied()
			.find(|&s| game.tile_at(s).is_none())
			.unwrap();
		game.apply(Action::Add { bench_index: 0, node: p1c }).unwrap();
		assert_eq!(game.petal_majority(Player::One), 3);
		assert_eq!(
			game.status(),
			GameStatus::Won { winner: Player::One, reason: EndReason::PetalMajority }
		);
	}

	#[test]
	fn control_win_needs_five_of_nine_not_center_alone() {
		let mut rules = RuleSet::default();
		rules.center_hold_win = false;
		rules.control_win = true;
		rules.control_n = 5;
		rules.control_hold_turns = 1;
		let mut game = Game::empty(rules);
		let center = game.board().center();
		put(&mut game, center, Player::One, TileColor::Blue, 3, 1);
		{
			let t = take(&mut game, Player::One, TileColor::Green, 2, 1);
			game.set_bench(Player::One, vec![t]);
		}
		{
			let t = take(&mut game, Player::Two, TileColor::Orange, 3, 1);
			game.set_bench(Player::Two, vec![t]);
		}
		game.set_current(Player::One);
		let spawn = game
			.board()
			.spawns()
			.iter()
			.copied()
			.find(|&s| game.tile_at(s).is_none())
			.unwrap();
		game.apply(Action::Add { bench_index: 0, node: spawn }).unwrap();
		assert_eq!(game.control_count(Player::One), 1);
		assert_eq!(game.status(), GameStatus::InProgress);
	}

	#[test]
	fn control_win_at_end_of_turn_when_t_is_one() {
		let mut rules = RuleSet::default();
		rules.center_hold_win = false;
		rules.control_win = true;
		rules.control_n = 5;
		rules.control_hold_turns = 1;
		let mut game = Game::empty(rules);
		occupy_control_five(&mut game, Player::One);
		{
			let t = take(&mut game, Player::One, TileColor::Green, 2, 1);
			game.set_bench(Player::One, vec![t]);
		}
		stock_bench(&mut game, Player::Two, 1);
		game.set_current(Player::One);
		assert_eq!(game.control_count(Player::One), 5);
		assert_eq!(game.control_need(), 5);
		let spawn = game
			.board()
			.spawns()
			.iter()
			.copied()
			.find(|&s| game.tile_at(s).is_none())
			.unwrap();
		game.apply(Action::Add { bench_index: 0, node: spawn }).unwrap();
		assert_eq!(
			game.status(),
			GameStatus::Won { winner: Player::One, reason: EndReason::Control }
		);
	}

	#[test]
	fn control_hold_t_three_requires_three_own_turns() {
		let mut rules = RuleSet::default();
		rules.center_hold_win = false;
		rules.control_win = true;
		rules.control_n = 5;
		rules.control_hold_turns = 3;
		rules.no_same_piece_twice = SamePieceTwice::Off;
		let mut game = Game::empty(rules);
		occupy_control_five(&mut game, Player::One);
		{
			let t = take(&mut game, Player::One, TileColor::Green, 2, 1);
			game.set_bench(Player::One, vec![t]);
		}
		stock_bench(&mut game, Player::Two, 1);
		game.set_current(Player::One);
		let p1 = game
			.board()
			.spawns()
			.iter()
			.copied()
			.find(|&s| game.tile_at(s).is_none())
			.unwrap();
		game.apply(Action::Add { bench_index: 0, node: p1 }).unwrap();
		assert_eq!(game.control_hold(Player::One), 1);
		assert_eq!(game.status(), GameStatus::InProgress);

		let p2 = game
			.board()
			.spawns()
			.iter()
			.copied()
			.find(|&s| game.tile_at(s).is_none())
			.unwrap();
		game.apply(Action::Add { bench_index: 0, node: p2 }).unwrap();
		assert_eq!(game.control_hold(Player::One), 1, "P2's turn must not increment P1");

		{
			let t = take(&mut game, Player::One, TileColor::Blue, 2, 1);
			game.set_bench(Player::One, vec![t]);
		}
		let p1b = game
			.board()
			.spawns()
			.iter()
			.copied()
			.find(|&s| game.tile_at(s).is_none())
			.unwrap();
		game.apply(Action::Add { bench_index: 0, node: p1b }).unwrap();
		assert_eq!(game.control_hold(Player::One), 2);

		{
			let t = take(&mut game, Player::Two, TileColor::Green, 1, 1);
			game.set_bench(Player::Two, vec![t]);
		}
		let p2b = game
			.board()
			.spawns()
			.iter()
			.copied()
			.find(|&s| game.tile_at(s).is_none())
			.unwrap();
		game.apply(Action::Add { bench_index: 0, node: p2b }).unwrap();

		{
			let t = take(&mut game, Player::One, TileColor::Orange, 1, 1);
			game.set_bench(Player::One, vec![t]);
		}
		let p1c = game
			.board()
			.spawns()
			.iter()
			.copied()
			.find(|&s| game.tile_at(s).is_none())
			.unwrap();
		game.apply(Action::Add { bench_index: 0, node: p1c }).unwrap();
		assert_eq!(game.control_hold(Player::One), 3);
		assert_eq!(
			game.status(),
			GameStatus::Won { winner: Player::One, reason: EndReason::Control }
		);
	}

	#[test]
	fn insufficient_tiles_strict_loses_below_five_ignoring_bag() {
		let mut rules = RuleSet::default();
		rules.center_hold_win = false;
		rules.insufficient_tiles_loss = InsufficientTilesLoss::Strict;
		let mut game = Game::empty(rules);
		let inner = game.board().inner_ring().to_vec();
		for &node in inner.iter().take(3) {
			put(&mut game, node, Player::One, TileColor::Green, 3, 1);
		}
		let bag = take(&mut game, Player::One, TileColor::Blue, 2, 1);
		game.set_bag(Player::One, vec![bag]);
		let p1b = take(&mut game, Player::One, TileColor::Green, 1, 1);
		game.set_bench(Player::One, vec![p1b]);
		// P2 must sit at >= 5 reachable tiles or both players lose and it draws.
		stock_bench(&mut game, Player::Two, 5);
		game.set_current(Player::One);
		assert_eq!(game.reachable_tiles(Player::One), 4);
		assert_eq!(game.reachable_tiles(Player::Two), 5);
		let spawn = game
			.board()
			.spawns()
			.iter()
			.copied()
			.find(|&s| game.tile_at(s).is_none())
			.unwrap();
		game.apply(Action::Add { bench_index: 0, node: spawn }).unwrap();
		assert_eq!(game.reachable_tiles(Player::One), 4);
		assert_eq!(game.bag_len(Player::One), 1, "bag is ignored in strict mode");
		assert_eq!(
			game.status(),
			GameStatus::Won { winner: Player::Two, reason: EndReason::InsufficientTiles }
		);
	}

	#[test]
	fn insufficient_tiles_strict_allows_exactly_five() {
		let mut rules = RuleSet::default();
		rules.center_hold_win = false;
		rules.insufficient_tiles_loss = InsufficientTilesLoss::Strict;
		let mut game = Game::empty(rules);
		let inner = game.board().inner_ring().to_vec();
		for &node in inner.iter().take(4) {
			put(&mut game, node, Player::One, TileColor::Green, 3, 1);
		}
		let p1b = take(&mut game, Player::One, TileColor::Green, 1, 1);
		game.set_bench(Player::One, vec![p1b]);
		stock_bench(&mut game, Player::Two, 5);
		game.set_current(Player::One);
		let spawn = game.board().spawns()[0];
		game.apply(Action::Add { bench_index: 0, node: spawn }).unwrap();
		assert_eq!(game.reachable_tiles(Player::One), 5);
		assert_eq!(game.reachable_tiles(Player::Two), 5);
		assert_eq!(game.status(), GameStatus::InProgress);
	}

	#[test]
	fn insufficient_no_recapture_loses_without_a_capturing_move() {
		let mut rules = RuleSet::default();
		rules.center_hold_win = false;
		rules.insufficient_tiles_loss = InsufficientTilesLoss::NoRecapture;
		let mut game = Game::empty(rules);
		let inner = game.board().inner_ring().to_vec();
		for &node in inner.iter().take(3) {
			put(&mut game, node, Player::One, TileColor::Green, 3, 1);
		}
		let bag = take(&mut game, Player::One, TileColor::Blue, 2, 1);
		game.set_bag(Player::One, vec![bag]);
		let p1b = take(&mut game, Player::One, TileColor::Green, 1, 1);
		game.set_bench(Player::One, vec![p1b]);
		// P2 is only on the bench, so P1 has a bag but no capturing MOVE.
		stock_bench(&mut game, Player::Two, 5);
		game.set_current(Player::One);
		assert!(!game.has_capturing_move(Player::One));
		let spawn = game.board().spawns()[0];
		game.apply(Action::Add { bench_index: 0, node: spawn }).unwrap();
		assert_eq!(
			game.status(),
			GameStatus::Won { winner: Player::Two, reason: EndReason::InsufficientTiles }
		);
	}

	#[test]
	fn insufficient_no_recapture_survives_when_a_capture_can_draw() {
		let mut rules = RuleSet::default();
		rules.center_hold_win = false;
		rules.insufficient_tiles_loss = InsufficientTilesLoss::NoRecapture;
		rules.no_same_piece_twice = SamePieceTwice::Off;
		let mut game = Game::empty(rules);
		let spawn = game.board().spawns()[0];
		let nbs = game.board().neighbors(spawn).to_vec();
		let land = nbs[0];
		let from = *game
			.board()
			.neighbors(land)
			.iter()
			.find(|n| **n != spawn && !nbs.contains(n))
			.unwrap_or(&nbs[2]);
		// Proven capture geometry from `move_capture_draws_one_per_opponent_tile`.
		put(&mut game, spawn, Player::Two, TileColor::Green, 2, 1);
		put(&mut game, nbs[1], Player::One, TileColor::Orange, 1, 1);
		put(&mut game, from, Player::One, TileColor::Blue, 3, 1);
		let inner = game.board().inner_ring().to_vec();
		let extra = inner
			.iter()
			.copied()
			.find(|&n| {
				n != spawn
					&& n != land && n != from
					&& n != nbs[1] && game.tile_at(n).is_none()
					&& !game.board().neighbors(spawn).contains(&n)
			})
			.expect("an inner node away from the capture");
		put(&mut game, extra, Player::One, TileColor::Green, 3, 1);
		let bag = take(&mut game, Player::One, TileColor::Blue, 2, 1);
		game.set_bag(Player::One, vec![bag]);
		{
			let t = take(&mut game, Player::One, TileColor::Green, 1, 1);
			game.set_bench(Player::One, vec![t]);
		}
		stock_bench(&mut game, Player::Two, 5);
		game.set_current(Player::One);
		assert_eq!(game.reachable_tiles(Player::One), 4);
		assert!(game.destinations(from).contains(&land));
		assert!(game.move_removes_opponent(from, land));
		assert!(game.has_capturing_move(Player::One));
		// Turn-boundary check without playing the capture: board+bench is 4,
		// but a capturing MOVE plus a non-empty bag keeps them alive.
		game.evaluate_insufficient_tiles_for_test();
		assert_eq!(game.reachable_tiles(Player::One), 4);
		assert_eq!(game.bag_len(Player::One), 1);
		assert_eq!(game.status(), GameStatus::InProgress);
	}

	#[test]
	fn insufficient_both_players_is_a_draw() {
		let mut rules = RuleSet::default();
		rules.center_hold_win = false;
		rules.insufficient_tiles_loss = InsufficientTilesLoss::Strict;
		let mut game = Game::empty(rules);
		let inner = game.board().inner_ring().to_vec();
		for &node in inner.iter().take(3) {
			put(&mut game, node, Player::One, TileColor::Green, 3, 1);
		}
		for &node in inner.iter().skip(4).take(3) {
			put(&mut game, node, Player::Two, TileColor::Orange, 3, 1);
		}
		let p1b = take(&mut game, Player::One, TileColor::Green, 1, 1);
		game.set_bench(Player::One, vec![p1b]);
		let p2b = take(&mut game, Player::Two, TileColor::Blue, 1, 1);
		game.set_bench(Player::Two, vec![p2b]);
		game.set_current(Player::One);
		let spawn = game.board().spawns()[0];
		game.apply(Action::Add { bench_index: 0, node: spawn }).unwrap();
		assert_eq!(game.status(), GameStatus::Draw { reason: EndReason::InsufficientTiles });
	}

	#[test]
	fn no_same_piece_twice_blocks_repeat_unless_capture() {
		let mut rules = RuleSet::default();
		rules.no_same_piece_twice = SamePieceTwice::Always;
		let mut game = Game::empty(rules);
		let inner = inner_ring(&game);
		let a = inner[0];
		let center = game.board().center();
		put(&mut game, a, Player::One, TileColor::Blue, 3, 1);
		{
			let t = take(&mut game, Player::Two, TileColor::Orange, 3, 1);
			game.set_bench(Player::Two, vec![t]);
		}
		game.set_current(Player::One);
		assert!(game.destinations(a).contains(&center));
		let tile_id = game.tile_at(a).unwrap().id;
		game.apply(Action::Move { from: a, to: center }).unwrap();
		assert_eq!(game.last_moved_tile(Player::One), Some(tile_id));

		let p2_spawn = game.board().spawns()[0];
		game.apply(Action::Add { bench_index: 0, node: p2_spawn }).unwrap();

		let back = a;
		assert!(
			game.destinations(center).contains(&back),
			"path back to the inner node should exist"
		);
		assert!(
			!game.legal_destinations(center).contains(&back),
			"repeat MOVE without a capture must be absent from legal destinations"
		);
		assert!(
			game.apply(Action::Move { from: center, to: back }).is_err(),
			"repeat MOVE without a capture must be rejected"
		);
	}

	#[test]
	fn no_same_piece_twice_allows_repeat_that_captures() {
		let mut rules = RuleSet::default();
		rules.no_same_piece_twice = SamePieceTwice::Always;
		rules.no_moves_loses = false;
		let mut game = Game::empty(rules);
		let spawn = game.board().spawns()[0];
		let nbs = game.board().neighbors(spawn).to_vec();
		assert!(nbs.len() >= 3);
		let land = nbs[0];
		let from = *game
			.board()
			.neighbors(land)
			.iter()
			.find(|n| **n != spawn && !nbs.contains(n))
			.expect("land has a neighbor away from spawn");
		put(&mut game, from, Player::One, TileColor::Blue, 3, 1);
		{
			let t = take(&mut game, Player::Two, TileColor::Orange, 3, 1);
			game.set_bench(Player::Two, vec![t]);
		}
		game.set_current(Player::One);
		let parking = game
			.destinations(from)
			.into_iter()
			.find(|&d| d != land)
			.expect("a non-capturing first step");
		let tile_id = game.tile_at(from).unwrap().id;
		game.apply(Action::Move { from, to: parking }).unwrap();
		assert_eq!(game.last_moved_tile(Player::One), Some(tile_id));
		let p2_spawn = game
			.board()
			.spawns()
			.iter()
			.copied()
			.find(|&s| s != spawn && game.tile_at(s).is_none())
			.unwrap();
		game.apply(Action::Add { bench_index: 0, node: p2_spawn }).unwrap();

		let mover = game.clear_tile(parking).expect("mover still on the board");
		assert_eq!(mover.id, tile_id);
		game.place_tile(from, mover);
		put(&mut game, spawn, Player::Two, TileColor::Green, 2, 1);
		put(&mut game, nbs[1], Player::One, TileColor::Orange, 1, 1);
		if game.tile_at(land).is_some() {
			let _ = game.clear_tile(land);
		}
		game.set_current(Player::One);
		assert!(game.destinations(from).contains(&land), "w3 d=1 should step to {land}");
		assert!(
			game.move_removes_opponent(from, land),
			"landing a different color should overwhelm the double green"
		);
		assert!(
			game.legal_destinations(from).contains(&land),
			"capture exception must keep the repeat MOVE legal"
		);
		let outcome = game.apply(Action::Move { from, to: land }).unwrap();
		assert!(
			outcome.removals.iter().any(|r| r.node == spawn && r.tile.owner == Player::Two),
			"capture exception must fire: {:?}",
			outcome.removals
		);
	}

	#[test]
	fn no_same_piece_twice_when_bench_empty_only() {
		let mut rules = RuleSet::default();
		rules.no_same_piece_twice = SamePieceTwice::WhenBenchEmpty;
		let mut game = Game::empty(rules);
		let inner = inner_ring(&game);
		let a = inner[0];
		let center = game.board().center();
		put(&mut game, a, Player::One, TileColor::Blue, 3, 1);
		{
			let t = take(&mut game, Player::One, TileColor::Green, 3, 1);
			game.set_bench(Player::One, vec![t]);
		}
		{
			let t = take(&mut game, Player::Two, TileColor::Orange, 3, 1);
			game.set_bench(Player::Two, vec![t]);
		}
		game.set_current(Player::One);
		game.apply(Action::Move { from: a, to: center }).unwrap();
		let p2_spawn = game.board().spawns()[0];
		game.apply(Action::Add { bench_index: 0, node: p2_spawn }).unwrap();
		assert!(!game.bench(Player::One).is_empty());
		assert!(game.legal_destinations(center).contains(&a));
		game.apply(Action::Move { from: center, to: a }).unwrap();
	}

	#[test]
	fn color_restricted_hops_only_lighter_same_color() {
		assert_eq!(RuleSet::default().hop_weight_cmp, HopWeightCmp::Le);
		let mut rules = RuleSet::default();
		rules.color_restricted_hops = true;
		rules.hop_weight_cmp = HopWeightCmp::Lt;
		let mut game = Game::empty(rules);
		let b = game.board().clone();
		let cycle = b.circle_cycle(0).to_vec();
		let (from, hop_over, land) = (cycle[0], cycle[1], cycle[2]);

		put(&mut game, from, Player::One, TileColor::Green, 3, 1);
		put(&mut game, hop_over, Player::Two, TileColor::Orange, 3, 1);
		game.set_current(Player::One);
		assert!(
			!game.destinations(from).contains(&land),
			"other-color occupant cannot be hopped when color_restricted_hops is on"
		);

		game.clear_tile(hop_over);
		put(&mut game, hop_over, Player::Two, TileColor::Green, 3, 1);
		assert!(
			!game.destinations(from).contains(&land),
			"equal-weight same color is not < under hop_weight_cmp=Lt"
		);

		game.clear_tile(hop_over);
		put(&mut game, hop_over, Player::Two, TileColor::Green, 1, 1);
		assert!(
			game.destinations(from).contains(&land),
			"lighter same-color occupant is legal: 'if the weight in your color is less than, you can traverse'"
		);

		let mut le_rules = rules;
		le_rules.hop_weight_cmp = HopWeightCmp::Le;
		let mut le_game = Game::empty(le_rules);
		put(&mut le_game, from, Player::One, TileColor::Green, 3, 1);
		put(&mut le_game, hop_over, Player::Two, TileColor::Green, 3, 1);
		le_game.set_current(Player::One);
		assert!(
			le_game.destinations(from).contains(&land),
			"hop_weight_cmp=Le allows equal-weight same color"
		);
	}

	#[test]
	fn edge_revisit_path_legal_only_under_no_edge_revisit() {
		fn dests(constraint: PathConstraint) -> Vec<NodeId> {
			let mut rules = RuleSet::default();
			rules.path_constraint = constraint;
			let mut game = Game::empty(rules);
			let inner = inner_ring(&game);
			let start = inner[0];
			let nbs = game.board().neighbors(start).to_vec();
			let spawn =
				*nbs.iter().find(|n| game.board().is_spawn(**n)).expect("inner has a spawn");
			for id in 0..game.board().node_count() as NodeId {
				if id == start || id == game.board().center() || id == spawn {
					continue;
				}
				put(&mut game, id, Player::Two, TileColor::Orange, 1, 1);
			}
			put(&mut game, start, Player::One, TileColor::Blue, 3, 3);
			game.set_current(Player::One);
			game.destinations(start)
		}

		let via_edge = dests(PathConstraint::NoEdgeRevisit);
		let via_node = dests(PathConstraint::NoNodeRevisit);
		let via_both = dests(PathConstraint::Both);
		assert!(
			via_edge.contains(&{
				// spawn neighbor of start; identified inside dests() but recompute
				let game = Game::empty(RuleSet::default());
				let inner = inner_ring(&game);
				let start = inner[0];
				*game
					.board()
					.neighbors(start)
					.iter()
					.find(|n| game.board().is_spawn(**n))
					.unwrap()
			}),
			"NoEdgeRevisit may return through the second center-arc and land on the spawn, got {via_edge:?}"
		);
		let spawn = {
			let game = Game::empty(RuleSet::default());
			let inner = inner_ring(&game);
			let start = inner[0];
			*game
				.board()
				.neighbors(start)
				.iter()
				.find(|n| game.board().is_spawn(**n))
				.unwrap()
		};
		assert!(via_edge.contains(&spawn));
		assert!(
			!via_node.contains(&spawn),
			"NoNodeRevisit cannot revisit the start inner node to reach the spawn in 3 steps, got {via_node:?}"
		);
		assert!(
			!via_both.contains(&spawn),
			"Both is at least as strict as NoNodeRevisit, got {via_both:?}"
		);
	}

	#[test]
	fn center_move_counts_by_path_constraint() {
		fn count(dots: u8, constraint: PathConstraint) -> usize {
			let mut rules = RuleSet::default();
			rules.path_constraint = constraint;
			let mut game = Game::empty(rules);
			let center = game.board().center();
			put(&mut game, center, Player::One, TileColor::Blue, 1, dots);
			game.set_current(Player::One);
			game.destinations(center).len()
		}

		let node_d1 = count(1, PathConstraint::NoNodeRevisit);
		let edge_d1 = count(1, PathConstraint::NoEdgeRevisit);
		let both_d1 = count(1, PathConstraint::Both);
		let node_d2 = count(2, PathConstraint::NoNodeRevisit);
		let edge_d2 = count(2, PathConstraint::NoEdgeRevisit);
		let both_d2 = count(2, PathConstraint::Both);
		let node_d3 = count(3, PathConstraint::NoNodeRevisit);
		let edge_d3 = count(3, PathConstraint::NoEdgeRevisit);
		let both_d3 = count(3, PathConstraint::Both);
		let node_d4 = count(4, PathConstraint::NoNodeRevisit);
		let edge_d4 = count(4, PathConstraint::NoEdgeRevisit);
		let both_d4 = count(4, PathConstraint::Both);

		assert_eq!((node_d1, edge_d1, both_d1), (8, 8, 8), "d=1 from center: 8 inner neighbors");
		assert_eq!((node_d2, edge_d2, both_d2), (8, 8, 8), "d=2 from center: 8 spawns");
		assert_eq!((node_d3, edge_d3, both_d3), (16, 16, 16), "d=3 from center: 8 inner + 8 outer");
		assert_eq!((node_d4, edge_d4, both_d4), (16, 16, 16), "d=4 from center: 16 dests");
		fn inner_count(dots: u8, constraint: PathConstraint) -> usize {
			let mut rules = RuleSet::default();
			rules.path_constraint = constraint;
			let mut game = Game::empty(rules);
			let start = inner_ring(&game)[0];
			put(&mut game, start, Player::One, TileColor::Blue, 1, dots);
			game.set_current(Player::One);
			game.destinations(start).len()
		}
		assert_eq!(inner_count(3, PathConstraint::NoNodeRevisit), 14);
		assert_eq!(inner_count(3, PathConstraint::NoEdgeRevisit), 14);
	}

	#[test]
	fn turn_limit_still_draws_under_new_defaults() {
		let mut rules = RuleSet::default();
		rules.turn_limit = 1;
		let mut game = Game::empty(rules);
		let spawn = game.board().spawns()[0];
		{
			let t = take(&mut game, Player::One, TileColor::Green, 3, 1);
			game.set_bench(Player::One, vec![t]);
		}
		{
			let t = take(&mut game, Player::Two, TileColor::Orange, 3, 1);
			game.set_bench(Player::Two, vec![t]);
		}
		game.apply(Action::Add { bench_index: 0, node: spawn }).unwrap();
		assert_eq!(game.status(), GameStatus::Draw { reason: EndReason::TurnLimit });
	}

	#[test]
	fn kind_index_covers_mono_and_multi_rings() {
		use std::collections::HashSet;
		let mut seen = HashSet::new();
		for color in TileColor::ALL {
			for weight in 1..=3 {
				for dots in 1..=3 {
					let t = Tile::mono(0, Player::One, color, weight, dots);
					assert!(t.kind_index() < KIND_COUNT);
					assert!(seen.insert(t.kind_index()));
				}
			}
		}
		assert_eq!(seen.len(), TileColor::ALL.len() * 9);
		let dual = Tile::with_rings(0, Player::One, &[TileColor::Green, TileColor::Teal], 2, 1);
		assert!(dual.kind_index() < KIND_COUNT);
		assert!(seen.insert(dual.kind_index()));
		let zag = Tile::with_ring_kinds(
			0,
			Player::One,
			&[(TileColor::Green, false), (TileColor::Teal, true)],
			2,
			1,
		);
		assert!(zag.kind_index() < KIND_COUNT);
		assert!(seen.insert(zag.kind_index()), "zag vs normal of same colors is a distinct kind");
	}

	#[test]
	fn multi_color_tiles_match_on_any_shared_ring() {
		let mut game = Game::empty(RuleSet::default());
		let spawn = game.board().spawns()[0];
		let nbs = game.board().neighbors(spawn).to_vec();
		let a = game.alloc_rings(Player::One, &[TileColor::Green, TileColor::Teal], 1, 1);
		game.place_tile(spawn, a);
		let b = game.alloc_rings(Player::Two, &[TileColor::Teal, TileColor::Maroon], 1, 1);
		game.place_tile(nbs[0], b);
		let (m, n) = game.tile_score(spawn).unwrap();
		assert_eq!((m, n), (2, 0), "shared teal is a match");
		assert!(!game.is_overwhelmed(spawn));

		game.clear_tile(nbs[0]);
		let c = game.alloc_rings(Player::Two, &[TileColor::Orange], 1, 1);
		game.place_tile(nbs[0], c);
		let (m, n) = game.tile_score(spawn).unwrap();
		assert_eq!((m, n), (1, 1), "no shared ring is a mismatch");
	}

	#[test]
	fn color_restricted_hops_allow_any_shared_ring() {
		let mut rules = RuleSet::default();
		rules.color_restricted_hops = true;
		rules.hop_weight_cmp = HopWeightCmp::Le;
		let mut game = Game::empty(rules);
		let b = game.board().clone();
		let cycle = b.circle_cycle(0).to_vec();
		let (from, hop_over, land) = (cycle[0], cycle[1], cycle[2]);
		let mover = game.alloc_rings(Player::One, &[TileColor::Green, TileColor::Blue], 3, 1);
		game.place_tile(from, mover);
		let occ = game.alloc_rings(Player::Two, &[TileColor::Blue], 2, 1);
		game.place_tile(hop_over, occ);
		game.set_current(Player::One);
		assert!(
			game.destinations(from).contains(&land),
			"shared blue ring should allow a color-restricted hop"
		);
	}

	#[test]
	fn ring_generation_follows_color_tree_weights() {
		let mut rng = StdRng::seed_from_u64(1);
		let n = 80_000u32;
		let mut color_hits = [0u32; 6];
		let mut ring_n = [0u32; 4];
		for _ in 0..n {
			let (rings, k) = sample_rings(&mut rng, 3);
			ring_n[k as usize] += 1;
			color_hits[rings[0] as usize] += 1;
		}
		assert_eq!(ring_n[0], 0);
		assert!(ring_n[1] > ring_n[2] && ring_n[2] > ring_n[3]);
		let two_plus = f64::from(ring_n[2] + ring_n[3]) / f64::from(n);
		assert!(
			(0.12..0.22).contains(&two_plus),
			"P(2+ rings)={two_plus:.4} (expected ~0.17; ring 3 can still add a second color)"
		);
		let three = f64::from(ring_n[3]) / f64::from(n);
		assert!((0.002..0.02).contains(&three), "P(3 rings)={three:.4}");
		let total_rings = color_hits.iter().sum::<u32>() as f64;
		let share = |i: usize| f64::from(color_hits[i]) / total_rings;
		for i in 0..3 {
			assert!((0.26..0.36).contains(&share(i)), "common color {i} share {}", share(i));
		}
		for i in 3..5 {
			assert!((0.02..0.05).contains(&share(i)), "uncommon color {i} share {}", share(i));
		}
		assert!((0.001..0.008).contains(&share(5)), "maroon share {}", share(5));

		let mut rng = StdRng::seed_from_u64(2);
		for _ in 0..2_000 {
			let (_, k) = sample_rings(&mut rng, 1);
			assert_eq!(k, 1);
		}
	}

	#[test]
	fn sampled_bags_are_mirrored_and_size_27() {
		let game = Game::new(RuleSet::default(), 11);
		assert_eq!(game.bag_len(Player::One) + game.bench(Player::One).len(), BAG_SIZE);
		let p1: Vec<_> = game
			.bench(Player::One)
			.iter()
			.chain(game.bag(Player::One))
			.map(|t| (t.color_mask(), t.n_rings, t.zag, t.weight, t.dots))
			.collect();
		let p2: Vec<_> = game
			.bench(Player::Two)
			.iter()
			.chain(game.bag(Player::Two))
			.map(|t| (t.color_mask(), t.n_rings, t.zag, t.weight, t.dots))
			.collect();
		assert_eq!(p1, p2);
	}

	#[test]
	fn max_rings_one_never_deals_extra_rings() {
		let mut rules = RuleSet::default();
		rules.max_rings = 1;
		let game = Game::new(rules, 3);
		for t in game.bench(Player::One).iter().chain(game.bag(Player::One)) {
			assert_eq!(t.n_rings, 1);
		}
	}

	#[test]
	fn zag_blue_normal_orange_zag_is_one_match_one_mismatch() {
		let mut game = Game::empty(RuleSet::default());
		let spawn = game.board().spawns()[0];
		let nb = game.board().neighbors(spawn)[0];
		let a = game.alloc_ring_kinds(
			Player::One,
			&[(TileColor::Blue, false), (TileColor::Orange, true)],
			1,
			1,
		);
		let b = game.alloc_ring_kinds(
			Player::Two,
			&[(TileColor::Blue, false), (TileColor::Orange, true)],
			1,
			1,
		);
		game.place_tile(spawn, a);
		game.place_tile(nb, b);
		let (m, x) = game.tile_score(spawn).unwrap();
		assert_eq!((m, x), (2, 1), "weight 1 + blue match, one orange-zag mismatch (not 2)");
		assert!(!game.is_overwhelmed(spawn));
		let (m2, x2) = game.tile_score(nb).unwrap();
		assert_eq!((m2, x2), (2, 1));
	}

	#[test]
	fn double_orange_zag_counts_two_mismatches() {
		let mut game = Game::empty(RuleSet::default());
		let spawn = game.board().spawns()[0];
		let nb = game.board().neighbors(spawn)[0];
		let t = game.alloc_ring_kinds(
			Player::One,
			&[(TileColor::Blue, false), (TileColor::Orange, false)],
			3,
			1,
		);
		let u = game.alloc_ring_kinds(
			Player::Two,
			&[(TileColor::Orange, true), (TileColor::Orange, true)],
			1,
			1,
		);
		game.place_tile(spawn, t);
		game.place_tile(nb, u);
		let (m, x) = game.tile_score(spawn).unwrap();
		assert_eq!(m, 3, "weight 3, orange zag never matches");
		assert_eq!(x, 2, "two orange zags on U whose color T has");
	}

	#[test]
	fn one_ring_zag_is_captured_on_contact_with_that_color() {
		let mut game = Game::empty(RuleSet::default());
		let spawn = game.board().spawns()[0];
		let nb = game.board().neighbors(spawn)[0];
		let zag = game.alloc_ring_kinds(Player::One, &[(TileColor::Orange, true)], 1, 1);
		game.place_tile(spawn, zag);
		assert!(!game.is_overwhelmed(spawn), "isolated zag is inert");
		let normal = game.alloc_rings(Player::Two, &[TileColor::Orange], 1, 1);
		game.place_tile(nb, normal);
		let (m, x) = game.tile_score(spawn).unwrap();
		assert_eq!((m, x), (1, 1), "zag never matches; ordinary mismatch captures w=1");
		assert!(game.is_overwhelmed(spawn));
		let (om, ox) = game.tile_score(nb).unwrap();
		assert_eq!((om, ox), (1, 1), "U has orange; T's orange zag hits U so U is also captured");
		assert!(game.is_overwhelmed(nb));
	}

	#[test]
	fn one_ring_zag_dies_against_heavier_color_carrier() {
		let mut game = Game::empty(RuleSet::default());
		let spawn = game.board().spawns()[0];
		let nb = game.board().neighbors(spawn)[0];
		let zag = game.alloc_ring_kinds(Player::One, &[(TileColor::Orange, true)], 1, 1);
		let heavy = game.alloc_rings(Player::Two, &[TileColor::Orange], 3, 1);
		game.place_tile(spawn, zag);
		game.place_tile(nb, heavy);
		assert!(game.is_overwhelmed(spawn));
		assert!(!game.is_overwhelmed(nb), "w3 carrier takes one zag hit and lives");
	}

	#[test]
	fn zag_mismatches_zag_of_the_same_color() {
		let mut game = Game::empty(RuleSet::default());
		let spawn = game.board().spawns()[0];
		let nb = game.board().neighbors(spawn)[0];
		let a = game.alloc_ring_kinds(Player::One, &[(TileColor::Blue, true)], 2, 1);
		let b = game.alloc_ring_kinds(Player::Two, &[(TileColor::Blue, true)], 2, 1);
		game.place_tile(spawn, a);
		game.place_tile(nb, b);
		assert_eq!(game.edge_score(a, b), (0, 2), "two zag strokes, zero matches");
		let (m, x) = game.tile_score(spawn).unwrap();
		assert_eq!(m, 2, "zag never matches, only own weight");
		assert_eq!(x, 2, "neighbor's two blue zag strokes hit T's blue");
		assert!(game.is_overwhelmed(spawn), "weight 2 vs 2 zag hits is captured");
	}

	#[test]
	fn three_blue_zags_vs_three_blue_zags_is_three_mismatches() {
		let mut game = Game::empty(RuleSet::default());
		let spawn = game.board().spawns()[0];
		let nb = game.board().neighbors(spawn)[0];
		let rings = [(TileColor::Blue, true), (TileColor::Blue, true), (TileColor::Blue, true)];
		let a = game.alloc_ring_kinds(Player::One, &rings, 3, 1);
		let b = game.alloc_ring_kinds(Player::Two, &rings, 3, 1);
		game.place_tile(spawn, a);
		game.place_tile(nb, b);
		assert_eq!(game.edge_score(a, b), (0, 3));
		assert_eq!(game.edge_score(b, a), (0, 3));
		let (m, x) = game.tile_score(spawn).unwrap();
		assert_eq!((m, x), (3, 3), "weight 3, no color match, three zag hits");
		assert!(game.is_overwhelmed(spawn));
		assert!(game.is_overwhelmed(nb));
	}

	#[test]
	fn weight_three_one_ring_zag_counts_three_zag_strokes() {
		let mut game = Game::empty(RuleSet::default());
		let spawn = game.board().spawns()[0];
		let nb = game.board().neighbors(spawn)[0];
		let a = game.alloc_ring_kinds(Player::One, &[(TileColor::Blue, true)], 3, 1);
		let b = game.alloc_ring_kinds(Player::Two, &[(TileColor::Blue, true)], 3, 1);
		game.place_tile(spawn, a);
		game.place_tile(nb, b);
		assert_eq!(a.ring_strokes(), 3);
		assert_eq!(game.edge_score(a, b), (0, 3), "drawn zag rings are zag hits");
		assert!(game.is_overwhelmed(spawn));
	}

	#[test]
	fn one_zag_vs_one_zag_of_the_same_color_is_captured() {
		let mut game = Game::empty(RuleSet::default());
		let spawn = game.board().spawns()[0];
		let nb = game.board().neighbors(spawn)[0];
		let a = game.alloc_ring_kinds(Player::One, &[(TileColor::Blue, true)], 1, 1);
		let b = game.alloc_ring_kinds(Player::Two, &[(TileColor::Blue, true)], 1, 1);
		game.place_tile(spawn, a);
		game.place_tile(nb, b);
		assert_eq!(game.edge_score(a, b), (0, 1), "zags never match");
		let (m, x) = game.tile_score(spawn).unwrap();
		assert_eq!((m, x), (1, 1));
		assert!(game.is_overwhelmed(spawn));
		assert!(game.is_overwhelmed(nb));
	}

	#[test]
	fn zag_color_alone_never_counts_as_a_match() {
		let mut game = Game::empty(RuleSet::default());
		let a = game.alloc_ring_kinds(Player::One, &[(TileColor::Blue, true)], 1, 1);
		let b = game.alloc_rings(Player::Two, &[TileColor::Blue], 1, 1);
		assert!(!a.shares_normal(b));
		assert_eq!(game.edge_score(a, b), (0, 1), "ordinary mismatch; zag is not a match");
		assert_eq!(game.edge_score(b, a), (0, 1), "U's zag hits T's blue");
	}

	#[test]
	fn non_matching_zag_is_empty_not_a_mismatch() {
		let mut game = Game::empty(RuleSet::default());
		let spawn = game.board().spawns()[0];
		let nb = game.board().neighbors(spawn)[0];
		let zag = game.alloc_ring_kinds(Player::One, &[(TileColor::Orange, true)], 1, 1);
		let blue = game.alloc_rings(Player::Two, &[TileColor::Blue], 1, 1);
		assert_eq!(zag.zag_hits_on(blue), 0, "blue does not hold orange");
		assert_eq!(blue.zag_hits_on(zag), 0);
		assert_eq!(game.edge_score(zag, blue), (0, 0), "non-matching zag is empty");
		assert_eq!(game.edge_score(blue, zag), (0, 0));
		game.place_tile(spawn, zag);
		game.place_tile(nb, blue);
		let (m, x) = game.tile_score(spawn).unwrap();
		assert_eq!((m, x), (1, 0), "own weight only; orange zag does not mismatch blue");
		assert!(!game.is_overwhelmed(spawn));
		assert!(!game.is_overwhelmed(nb));
	}

	#[test]
	fn blue_normal_plus_orange_zag_vs_blue_is_one_match_zero_mismatches() {
		let mut game = Game::empty(RuleSet::default());
		let mixed = game.alloc_ring_kinds(
			Player::One,
			&[(TileColor::Blue, false), (TileColor::Orange, true)],
			1,
			1,
		);
		let blue = game.alloc_rings(Player::Two, &[TileColor::Blue], 1, 1);
		assert_eq!(game.edge_score(mixed, blue), (1, 0));
		assert_eq!(game.edge_score(blue, mixed), (1, 0), "orange zag does not hit blue-only");
		let spawn = game.board().spawns()[0];
		let nb = game.board().neighbors(spawn)[0];
		game.place_tile(spawn, mixed);
		game.place_tile(nb, blue);
		let (m, x) = game.tile_score(spawn).unwrap();
		assert_eq!((m, x), (2, 0), "weight 1 + blue match, orange zag is empty");
		assert!(!game.is_overwhelmed(spawn));
		assert!(!game.is_overwhelmed(nb));
	}

	#[test]
	fn non_matching_zag_does_not_hide_a_normal_conflict() {
		let mut game = Game::empty(RuleSet::default());
		let mixed = game.alloc_ring_kinds(
			Player::One,
			&[(TileColor::Blue, false), (TileColor::Orange, true)],
			1,
			1,
		);
		let green = game.alloc_rings(Player::Two, &[TileColor::Green], 1, 1);
		assert_eq!(game.edge_score(mixed, green), (0, 1), "blue vs green is still a conflict");
		assert_eq!(game.edge_score(green, mixed), (0, 1));
	}

	#[test]
	fn own_zag_does_not_mismatch_self() {
		let mut game = Game::empty(RuleSet::default());
		let spawn = game.board().spawns()[0];
		let nb = game.board().neighbors(spawn)[0];
		let t = game.alloc_ring_kinds(
			Player::One,
			&[(TileColor::Green, false), (TileColor::Orange, true)],
			2,
			1,
		);
		let u = game.alloc_rings(Player::Two, &[TileColor::Green], 1, 1);
		game.place_tile(spawn, t);
		game.place_tile(nb, u);
		let (m, x) = game.tile_score(spawn).unwrap();
		assert_eq!((m, x), (3, 0), "green match; T's orange zag does not mismatch T");
	}

	#[test]
	fn color_restricted_hops_ignore_zag_colors() {
		let mut rules = RuleSet::default();
		rules.color_restricted_hops = true;
		rules.hop_weight_cmp = HopWeightCmp::Le;
		let mut game = Game::empty(rules);
		let b = game.board().clone();
		let cycle = b.circle_cycle(0).to_vec();
		let (from, hop_over, land) = (cycle[0], cycle[1], cycle[2]);
		let mover = game.alloc_rings(Player::One, &[TileColor::Green], 3, 1);
		game.place_tile(from, mover);
		let occ = game.alloc_ring_kinds(Player::Two, &[(TileColor::Green, true)], 1, 1);
		game.place_tile(hop_over, occ);
		game.set_current(Player::One);
		assert!(!game.destinations(from).contains(&land), "zag green is not a hop color");
		game.clear_tile(hop_over);
		let occ_n = game.alloc_rings(Player::Two, &[TileColor::Green], 1, 1);
		game.place_tile(hop_over, occ_n);
		assert!(game.destinations(from).contains(&land), "normal green is hoppable");
	}

	#[test]
	fn zag_generation_is_about_one_in_seven() {
		let mut rng = StdRng::seed_from_u64(3);
		let n = 40_000u32;
		let mut rings = 0u32;
		let mut zags = 0u32;
		for _ in 0..n {
			let (_, k) = sample_rings(&mut rng, 3);
			let flags = sample_zag_flags(&mut rng, k, ZAG_NUMER_DEFAULT);
			rings += u32::from(k);
			zags += flags.iter().take(k as usize).filter(|z| **z).count() as u32;
		}
		let rate = f64::from(zags) / f64::from(rings);
		assert!((0.12..0.17).contains(&rate), "zag rate={rate:.4} (expected ~0.1429 = 1/7)");
		let mut rng = StdRng::seed_from_u64(4);
		for _ in 0..2_000 {
			let flags = sample_zag_flags(&mut rng, 3, 0);
			assert_eq!(flags, [false, false, false]);
		}
	}

	#[test]
	fn zag_off_deal_matches_pre_zag_ring_process() {
		let mut off = RuleSet::default();
		off.zag_numer = 0;
		let game = Game::new(off, 11);
		assert!(game
			.bench(Player::One)
			.iter()
			.chain(game.bag(Player::One))
			.all(|t| t.zag == [false, false, false]));
	}

	#[test]
	fn shuffle_bags_keeps_the_same_kinds() {
		let mut game = Game::new(RuleSet::default(), 12);
		let snapshot = |g: &Game, p: Player| {
			let mut v: Vec<_> = g.bag(p).iter().map(|t| t.kind_index()).collect();
			v.sort_unstable();
			v
		};
		let before = snapshot(&game, Player::One);
		let mut rng = StdRng::seed_from_u64(99);
		game.shuffle_bags(&mut rng);
		assert_eq!(before, snapshot(&game, Player::One));
	}
}
