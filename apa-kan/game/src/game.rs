//! Game model, RuleSet, overwhelm, movement, and win detection.

use std::collections::VecDeque;
use std::sync::Arc;

use rand::seq::SliceRandom;
use rand::Rng;

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
/// A piece forms a braid with probability `braid_numer / BRAID_DENOM`.
pub const BRAID_DENOM: u8 = 7;
pub const BRAID_NUMER_DEFAULT: u8 = 1;
/// Maximum collection size (the old 27 unique 3-color kinds). CLI `--deck-size`
/// / `--bag-size` accept `1..=BAG_SIZE`. The default collection is [`BAG_SIZE`].
pub const BAG_SIZE: usize = 27;
/// Public in-game bench after the 7+4 draft ([`crate::deck::BENCH_SIZE`]).
/// Spawn tokens default separately to 7. CLI `--deck-size` is the 27-tile
/// collection, not this field.
pub const BAG_SIZE_DEFAULT: usize = 11;
/// Default opening spawn-token stack. Independent of [`BAG_SIZE_DEFAULT`].
pub const STARTING_TOKENS_DEFAULT: u8 = 7;
pub const MAX_COLOR_RINGS: u8 = 3;
/// Per live slot: empty + (6 colors × 3 types). Kind space is this cubed.
pub const RING_TYPE_COUNT: usize = 3;
pub const SLOT_KINDS: usize = 1 + 6 * RING_TYPE_COUNT;
pub const COLOR_STATES: usize = SLOT_KINDS * SLOT_KINDS * SLOT_KINDS;

/// Ring kind is independent of color: a slot is a normal match, a zag (jag),
/// or part of one braid group occupying 2–3 slots.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
#[repr(u8)]
pub enum RingType {
	#[default]
	Normal = 0,
	Zag = 1,
	Braid = 2,
}

impl RingType {
	pub fn name(self) -> &'static str {
		match self {
			Self::Normal => "normal",
			Self::Zag => "zag",
			Self::Braid => "braid",
		}
	}

	pub fn is_normal(self) -> bool {
		matches!(self, Self::Normal)
	}

	pub fn is_zag(self) -> bool {
		matches!(self, Self::Zag)
	}

	pub fn is_braid(self) -> bool {
		matches!(self, Self::Braid)
	}

	pub fn from_zag(zag: bool) -> Self {
		if zag {
			Self::Zag
		} else {
			Self::Normal
		}
	}
}

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

/// Piece-level braid roll (`braid_numer / 7`). Size 2 occupies two slots
/// (padding a 1-ring tile by repeating its color); size 3 takes every slot.
/// `braid_numer == 0` skips the RNG so `--braids off` matches the pre-braid
/// deal for the same seed.
pub fn sample_braid(
	rng: &mut impl Rng,
	rings: &mut [TileColor; 3],
	n_rings: &mut u8,
	max_rings: u8,
	braid_numer: u8,
) -> [RingType; 3] {
	let mut types = [RingType::Normal; 3];
	if braid_numer == 0 || max_rings < 2 {
		return types;
	}
	let numer = u32::from(braid_numer.min(BRAID_DENOM));
	if rng.gen_range(0..u32::from(BRAID_DENOM)) >= numer {
		return types;
	}
	let n = (*n_rings).clamp(1, MAX_COLOR_RINGS);
	let max = max_rings.clamp(1, MAX_COLOR_RINGS);
	let size = if n >= 3 && max >= 3 {
		if rng.gen_range(0..2u32) == 0 {
			3u8
		} else {
			2u8
		}
	} else {
		2u8
	};
	if n < size {
		let last = rings[(n - 1) as usize];
		for i in n..size {
			rings[i as usize] = last;
		}
		*n_rings = size;
	}
	for slot in types.iter_mut().take(size as usize) {
		*slot = RingType::Braid;
	}
	types
}

/// Independent zag rolls on remaining **normal** slots. Braid slots stay braid.
pub fn sample_zag_types(rng: &mut impl Rng, n_rings: u8, types: &mut [RingType; 3], zag_numer: u8) {
	if zag_numer == 0 {
		return;
	}
	let numer = u32::from(zag_numer.min(ZAG_DENOM));
	for slot in types.iter_mut().take(n_rings.clamp(1, MAX_COLOR_RINGS) as usize) {
		if *slot == RingType::Normal && rng.gen_range(0..u32::from(ZAG_DENOM)) < numer {
			*slot = RingType::Zag;
		}
	}
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
	/// Ring colors in slot order. Generation usually keeps colors distinct;
	/// a braid may repeat a color (orange+orange). First [`Self::n_rings`] live.
	pub rings: [TileColor; 3],
	pub n_rings: u8,
	/// Parallel to [`Self::rings`]: normal, zag (jag), or braid. At most one
	/// braid group per tile, occupying 2 or 3 slots.
	pub ring_type: [RingType; 3],
	/// Overwhelm match base and hop compare. Always equal to [`Self::n_rings`]
	/// (braid slots count). Constructor `weight` arguments are ignored.
	pub weight: u8,
	pub dots: u8,
}

/// Kind space: color-state × dots. Weight is the ring-slot count, already
/// encoded in [`Tile::color_state`].
pub const KIND_DOTS: usize = 3;
pub const KIND_COUNT: usize = COLOR_STATES * KIND_DOTS;

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
	/// `true` marks a zag; braid tests should use [`Self::with_typed_rings`].
	pub fn with_ring_kinds(
		id: u32,
		owner: Player,
		parts: &[(TileColor, bool)],
		weight: u8,
		dots: u8,
	) -> Self {
		let typed: Vec<(TileColor, RingType)> =
			parts.iter().map(|&(c, z)| (c, RingType::from_zag(z))).collect();
		Self::with_typed_rings(id, owner, &typed, weight, dots)
	}

	/// Rings in listed order with an explicit [`RingType`] per slot.
	pub fn with_typed_rings(
		id: u32,
		owner: Player,
		parts: &[(TileColor, RingType)],
		weight: u8,
		dots: u8,
	) -> Self {
		let first = parts.first().map(|p| p.0).unwrap_or(TileColor::Green);
		let mut rings = [first; 3];
		let mut ring_type = [RingType::Normal; 3];
		let mut n = 0u8;
		for &(c, typ) in parts.iter().take(MAX_COLOR_RINGS as usize) {
			rings[n as usize] = c;
			ring_type[n as usize] = typ;
			n += 1;
		}
		if n == 0 {
			n = 1;
		}
		let _ = weight;
		Self { id, owner, rings, n_rings: n, ring_type, weight: n, dots }
	}

	pub fn ring_colors(&self) -> &[TileColor] {
		&self.rings[..self.n_rings.clamp(1, MAX_COLOR_RINGS) as usize]
	}

	pub fn ring_type_at(self, index: usize) -> RingType {
		if index < self.n_rings.clamp(1, MAX_COLOR_RINGS) as usize {
			self.ring_type[index]
		} else {
			RingType::Normal
		}
	}

	pub fn ring_is_zag(self, index: usize) -> bool {
		self.ring_type_at(index).is_zag()
	}

	pub fn ring_is_braid(self, index: usize) -> bool {
		self.ring_type_at(index).is_braid()
	}

	/// Zag flags in slot order, for call sites that still compare a `[bool; 3]`.
	pub fn zag(self) -> [bool; 3] {
		self.ring_type.map(RingType::is_zag)
	}

	pub fn primary(self) -> TileColor {
		self.rings[0]
	}

	pub fn has_color(self, color: TileColor) -> bool {
		self.ring_colors().contains(&color)
	}

	pub fn has_normal(self, color: TileColor) -> bool {
		self.ring_colors()
			.iter()
			.enumerate()
			.any(|(i, &c)| c == color && self.ring_type[i].is_normal())
	}

	/// Concentric strokes drawn for this tile: one per live ring slot
	/// (braid slots included). There are no extra copies of the last ring.
	pub fn ring_strokes(self) -> u8 {
		self.n_rings.max(1)
	}

	fn stroke_idx(self, k: u8) -> usize {
		let n = self.n_rings.clamp(1, MAX_COLOR_RINGS) as usize;
		(k as usize).min(n.saturating_sub(1))
	}

	pub fn stroke_color(self, k: u8) -> TileColor {
		self.rings[self.stroke_idx(k)]
	}

	pub fn stroke_type(self, k: u8) -> RingType {
		self.ring_type[self.stroke_idx(k)]
	}

	pub fn stroke_is_zag(self, k: u8) -> bool {
		self.stroke_type(k).is_zag()
	}

	pub fn stroke_is_normal(self, k: u8) -> bool {
		self.stroke_type(k).is_normal()
	}

	pub fn stroke_is_braid(self, k: u8) -> bool {
		self.stroke_type(k).is_braid()
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
			if self.ring_type[i].is_normal() {
				m |= c.bit();
			}
		}
		m
	}

	/// How many live **rings** of `color`, any type. Braid satisfaction reads
	/// this on the neighbor. Each ring counts as 1.
	pub fn color_count(self, color: TileColor) -> u8 {
		self.ring_colors().iter().filter(|&&c| c == color).count() as u8
	}

	pub fn color_counts(self) -> [u8; 6] {
		let mut c = [0u8; 6];
		for &col in self.ring_colors() {
			c[col as usize] += 1;
		}
		c
	}

	pub fn braid_color_counts(self) -> [u8; 6] {
		let mut c = [0u8; 6];
		for (i, &col) in self.ring_colors().iter().enumerate() {
			if self.ring_type[i].is_braid() {
				c[col as usize] += 1;
			}
		}
		c
	}

	pub fn braid_colors(self) -> [Option<TileColor>; 3] {
		let mut out = [None; 3];
		let mut n = 0usize;
		for (i, &col) in self.ring_colors().iter().enumerate() {
			if self.ring_type[i].is_braid() && n < 3 {
				out[n] = Some(col);
				n += 1;
			}
		}
		out
	}

	/// Any ring color, normal, zag, or braid.
	pub fn shares_color(self, other: Tile) -> bool {
		(self.color_mask() & other.color_mask()) != 0
	}

	/// Shared **normal** ring. Zags and braids never produce a normal match.
	/// A color that either tile holds only as zags or braids does not count.
	/// Pass-through and hops still use this (both sides must hold the color
	/// as a normal). Edge scoring uses [`Self::normal_channel_matches`].
	pub fn shares_normal(self, other: Tile) -> bool {
		(self.normal_mask() & other.normal_mask()) != 0
	}

	/// True if some **normal**-ring color of this tile appears among `other`'s
	/// colors (any ring type: normal, zag, or braid).
	pub fn normal_color_in(self, other: Tile) -> bool {
		self.ring_colors()
			.iter()
			.enumerate()
			.any(|(i, &c)| self.ring_type[i].is_normal() && other.has_color(c))
	}

	/// Independent normal channel: a match when some normal-ring color of
	/// either tile is in the other tile's color multiset.
	pub fn normal_channel_matches(self, other: Tile) -> bool {
		self.normal_color_in(other) || other.normal_color_in(self)
	}

	/// How many of this tile's zag **rings** of `color` hit `target`
	/// (`target` holds that color on any ring). Counts live ring slots only.
	pub fn zag_hits_of_color(self, target: Tile, color: TileColor) -> i32 {
		if !target.has_color(color) {
			return 0;
		}
		(0..self.n_rings.min(MAX_COLOR_RINGS))
			.filter(|&k| self.stroke_is_zag(k) && self.stroke_color(k) == color)
			.count() as i32
	}

	/// How many of this tile's zag **rings** have their color on `target`
	/// (normal, zag, or braid). Counts live ring slots only. Own zags do not
	/// mismatch their owner. A zag whose color `target` lacks is absent (not
	/// a hit).
	pub fn zag_hits_on(self, target: Tile) -> i32 {
		TileColor::ALL.iter().map(|&c| self.zag_hits_of_color(target, c)).sum()
	}

	/// This tile's braid is satisfied when `other`'s facing **ring** color
	/// counts (any ring types) cover every color in the braid. Each ring
	/// counts as 1. Size is the braid's slot count (2 or 3). An orange+orange
	/// braid needs two orange rings on the neighbor.
	pub fn braid_fulfilled_by(self, other: Tile) -> Option<u8> {
		let size = self.braid_ring_count();
		if size < 2 {
			return None;
		}
		let need = self.braid_color_counts();
		let have = other.color_counts();
		if (0..6).all(|i| have[i] >= need[i]) {
			Some(size)
		} else {
			None
		}
	}

	/// True if this tile has a real presence toward `dst`: a normal ring, a
	/// braid (2–3 slots), or a zag whose color `dst` holds. Non-matching zags
	/// are treated as empty.
	pub fn present_toward(self, dst: Tile) -> bool {
		self.normal_mask() != 0 || self.braid_ring_count() >= 2 || self.zag_hits_on(dst) > 0
	}

	pub fn kind_label(self) -> String {
		let rings = self
			.ring_colors()
			.iter()
			.enumerate()
			.map(|(i, c)| match self.ring_type[i] {
				RingType::Zag => format!("{}-zag", c.name()),
				RingType::Braid => format!("{}-braid", c.name()),
				RingType::Normal => c.name().to_string(),
			})
			.collect::<Vec<_>>()
			.join("+");
		format!("{rings} w{} d{}", self.weight, self.dots)
	}

	/// Slot-coded color state: empty / (color × type) per live ring.
	pub fn color_state(self) -> usize {
		slot_state(self.rings, self.ring_type, self.n_rings)
	}

	/// `color_state * KIND_DOTS + (dots-1)`.
	pub fn kind_index(self) -> usize {
		let d = (self.dots.clamp(1, 3) - 1) as usize;
		self.color_state() * KIND_DOTS + d
	}

	/// How many of this tile's live rings are zags.
	pub fn zag_ring_count(self) -> u8 {
		(0..self.n_rings.min(MAX_COLOR_RINGS))
			.filter(|&i| self.ring_type[i as usize].is_zag())
			.count() as u8
	}

	/// How many of this tile's live rings are braid slots.
	pub fn braid_ring_count(self) -> u8 {
		(0..self.n_rings.min(MAX_COLOR_RINGS))
			.filter(|&i| self.ring_type[i as usize].is_braid())
			.count() as u8
	}

	/// How many of this tile's live rings are normal (not zag, not braid).
	pub fn normal_ring_count(self) -> u8 {
		(0..self.n_rings.min(MAX_COLOR_RINGS))
			.filter(|&i| self.ring_type[i as usize].is_normal())
			.count() as u8
	}

	/// Piece value under `weights`: `dots * w.dots + normal_rings * w.normal_ring
	/// + zag_rings * w.zag_ring + braid_rings * w.braid_ring`. Used by the
	/// low-value end condition.
	pub fn piece_value(self, weights: PieceValueWeights) -> i32 {
		weights.dots * i32::from(self.dots)
			+ weights.normal_ring * i32::from(self.normal_ring_count())
			+ weights.zag_ring * i32::from(self.zag_ring_count())
			+ weights.braid_ring * i32::from(self.braid_ring_count())
	}
}

/// Largest fulfilled braid on the edge `a`–`b`. Equal sizes pick the
/// lexicographically smaller braid color-count vector, then the lower tile id.
pub fn selected_braid(a: Tile, b: Tile) -> Option<(Tile, u8)> {
	let fa = a.braid_fulfilled_by(b);
	let fb = b.braid_fulfilled_by(a);
	match (fa, fb) {
		(None, None) => None,
		(Some(n), None) => Some((a, n)),
		(None, Some(n)) => Some((b, n)),
		(Some(na), Some(nb)) if na > nb => Some((a, na)),
		(Some(na), Some(nb)) if nb > na => Some((b, nb)),
		(Some(n), Some(_)) => {
			let ka = a.braid_color_counts();
			let kb = b.braid_color_counts();
			if ka < kb || (ka == kb && a.id <= b.id) {
				Some((a, n))
			} else {
				Some((b, n))
			}
		}
	}
}

/// Tunable weights for [`Tile::piece_value`]. Low-value resolution uses
/// [`Self::PRIMARY`] first (every ring counts 1), then [`Self::TIEBREAK`]
/// (zag and braid rings count 2). Add fields here as new ring types appear.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PieceValueWeights {
	pub dots: i32,
	pub normal_ring: i32,
	pub zag_ring: i32,
	pub braid_ring: i32,
}

impl PieceValueWeights {
	/// Step 1: value = dots + rings (every ring 1). Lower wins.
	pub const PRIMARY: Self = Self { dots: 1, normal_ring: 1, zag_ring: 1, braid_ring: 1 };
	/// Step 2 on a primary tie: zag and braid rings are worth 2. Lower wins.
	pub const TIEBREAK: Self = Self { dots: 1, normal_ring: 1, zag_ring: 2, braid_ring: 2 };
}

/// A missing piece in a low-value pair. Treated as maximum value so the
/// player who still has a tile wins that pair (more surviving pieces win
/// after every real pair ties).
const LOW_VALUE_EMPTY: (i32, i32) = (i32::MAX, i32::MAX);

/// Outcome of the progressive lowest-tile pair check used by
/// [`EndReason::LowValue`]. Search reads the resulting [`GameStatus`].
/// `pair` is 0-based into the sorted lists; `by_piece_count` is set when
/// the deciding slot compared a real tile against a missing (max-value) one.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LowValueResolution {
	Draw,
	Win { winner: Player, pair: usize, by_piece_count: bool },
}

/// Both players must have `board tiles + usable budget` strictly below this
/// for [`EndReason::LowValue`]. Usable budget is [`Game::budget`].
pub const LOW_VALUE_BOARD_LT: u8 = 5;

/// One spawn token. The opening stack is [`BudgetToken::Blank`] copies
/// ([`RuleSet::starting_bench_size`], default 7); captures push
/// [`BudgetToken::Captured`]. ADD pops from the front (blanks spend first,
/// so captured tiles stay visible). Tokens beyond the remaining deck are
/// kept as surplus; only [`Game::budget`] (min of stack length and unplaced
/// deck tiles) is usable ADD permission.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BudgetToken {
	Blank,
	Captured(Tile),
}

/// One slot in the visual deck row. Every unplaced deck tile is [`Self::Live`]
/// and selectable. [`Game::deck_slots`] returns only live tiles, so a placed
/// tile leaves the row (opening deals still show all [`RuleSet::bag_size`]
/// tiles, default 11). [`Self::Spent`] / [`Self::Empty`] remain for fixtures
/// and historical classification.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DeckSlot {
	Empty,
	Live(Tile),
	Spent(Tile),
}

impl DeckSlot {
	pub fn tile(self) -> Option<Tile> {
		match self {
			Self::Empty => None,
			Self::Live(tile) | Self::Spent(tile) => Some(tile),
		}
	}

	pub fn is_live(self) -> bool {
		matches!(self, Self::Live(_))
	}

	pub fn shows_blank_face(self) -> bool {
		matches!(self, Self::Empty)
	}
}

fn blank_budget(n: u8) -> Vec<BudgetToken> {
	vec![BudgetToken::Blank; n as usize]
}

/// Visual deck order: unplaced **bench** tiles left-to-right. Reserves (the
/// leftover collection) never appear in the row and are never usable in play.
fn visual_deck_order(bench: &[Tile], _bag: &[Tile]) -> Vec<Tile> {
	bench.to_vec()
}

pub fn slot_code(color: TileColor, typ: RingType) -> usize {
	1 + (color as usize) * RING_TYPE_COUNT + (typ as u8 as usize)
}

pub fn slot_state(rings: [TileColor; 3], types: [RingType; 3], n_rings: u8) -> usize {
	let n = n_rings.clamp(1, MAX_COLOR_RINGS) as usize;
	let mut state = 0usize;
	let mut pow = 1usize;
	for i in 0..MAX_COLOR_RINGS as usize {
		let code = if i < n { slot_code(rings[i], types[i]) } else { 0 };
		state += code * pow;
		pow *= SLOT_KINDS;
	}
	state
}

/// Decoded ADD kind color-state: masks, per-color counts, and braid size.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct ColorStateView {
	pub normal: u8,
	pub zag: u8,
	pub braid: u8,
	pub counts: [u8; 6],
	pub braid_counts: [u8; 6],
	pub braid_size: u8,
}

pub fn decode_color_state(mut state: usize) -> ColorStateView {
	let mut view = ColorStateView::default();
	for _ in 0..MAX_COLOR_RINGS {
		let code = state % SLOT_KINDS;
		state /= SLOT_KINDS;
		if code == 0 {
			continue;
		}
		let idx = code - 1;
		let color = idx / RING_TYPE_COUNT;
		let typ = idx % RING_TYPE_COUNT;
		if color >= 6 {
			continue;
		}
		let bit = 1u8 << color;
		view.counts[color] += 1;
		match typ {
			0 => view.normal |= bit,
			1 => view.zag |= bit,
			_ => {
				view.braid |= bit;
				view.braid_counts[color] += 1;
				view.braid_size += 1;
			}
		}
	}
	view
}

/// `(normal_mask, zag_mask)` from a `kind / KIND_DOTS` color-state.
pub fn color_state_masks(state: usize) -> (u8, u8) {
	let view = decode_color_state(state);
	(view.normal, view.zag)
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
	/// Both players have `board tiles + usable budget` strictly below
	/// [`LOW_VALUE_BOARD_LT`] (usable = min(tokens, unplaced deck tiles)).
	/// Sorted lowest-tile pairs decide (primary, then zag/braid-weighted
	/// tiebreak). A missing piece is maximum value, so more surviving tiles
	/// win; a draw only if the counts match and every pair ties on both passes.
	LowValue,
}

/// Restriction on moving the same tile on two consecutive own turns.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SamePieceTwice {
	Off,
	Always,
	/// Enforced only while that player's remaining (unplaced) deck is empty.
	WhenDeckEmpty,
}

impl SamePieceTwice {
	pub fn parse(s: &str) -> Result<Self, String> {
		match s.to_ascii_lowercase().as_str() {
			"off" | "false" | "0" => Ok(Self::Off),
			"always" | "on" | "true" => Ok(Self::Always),
			"when-deck-empty" | "when_deck_empty" | "deck-empty" | "when-bench-empty"
			| "when_bench_empty" | "bench-empty" => Ok(Self::WhenDeckEmpty),
			other => Err(format!(
				"unknown no-same-piece-twice mode '{other}' (expected off|always|when-deck-empty)"
			)),
		}
	}

	pub fn name(self) -> &'static str {
		match self {
			Self::Off => "off",
			Self::Always => "always",
			Self::WhenDeckEmpty => "when-deck-empty",
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

/// Ring-count comparison used when [`RuleSet::color_restricted_hops`] is on.
///
/// Interpretation of the designer's "if the weight in your color is less than,
/// you can traverse": hop only over the mover's color, comparing occupant
/// ring count to mover ring count with this operator. Default is `<=` (`Le`).
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

/// Ring-count comparison used when [`RuleSet::counted_pass_through`] is on.
///
/// Occupant may be passed through (never landed on) only if it shares a
/// normal color with the mover **and** this comparison holds. Default is
/// [`Self::Ge`] (`occupant.n_rings >= mover.n_rings`). [`Self::Gt`] is
/// strict (`>`), so a 3-ring tile can never pass through (max rings is 3).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PassThroughRingCmp {
	Ge,
	Gt,
}

impl PassThroughRingCmp {
	pub fn parse(s: &str) -> Result<Self, String> {
		match s.to_ascii_lowercase().as_str() {
			"ge" | ">=" | "geq" | "greater-equal" => Ok(Self::Ge),
			"gt" | ">" | "greater" => Ok(Self::Gt),
			other => Err(format!("unknown pass-through-ring-cmp '{other}' (expected ge|gt)")),
		}
	}

	pub fn name(self) -> &'static str {
		match self {
			Self::Ge => "ge",
			Self::Gt => "gt",
		}
	}

	pub fn allows(self, occupant_rings: u8, mover_rings: u8) -> bool {
		match self {
			Self::Ge => occupant_rings >= mover_rings,
			Self::Gt => occupant_rings > mover_rings,
		}
	}
}

/// How zag mismatches are counted on an occupied edge.
///
/// A hitting zag on either tile is an **edge** event; both tiles take the
/// same count. Default [`Self::Sum`] adds every hitting zag stroke.
/// [`Self::Max`] still counts up, but per color takes the larger of the two
/// tiles' hitting zag-stroke counts of that color.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ZagMismatchCount {
	/// Edge count = hitting zag strokes on both tiles. A orange zag vs B
	/// orange zag = 2. Candidate default (readable: add up the hitting zags).
	Sum,
	/// For each color, max(A's hitting zag strokes of that color, B's).
	/// Sum those maxima. Orange zag vs orange zag = 1; 3 blue vs 3 blue = 3;
	/// 2 orange vs 1 orange = 2.
	Max,
}

impl ZagMismatchCount {
	pub fn parse(s: &str) -> Result<Self, String> {
		match s.to_ascii_lowercase().replace('_', "-").as_str() {
			"sum" | "add" => Ok(Self::Sum),
			"max" | "maximum" => Ok(Self::Max),
			other => Err(format!("unknown zag-mismatch '{other}' (expected sum|max)")),
		}
	}

	pub fn name(self) -> &'static str {
		match self {
			Self::Sum => "sum",
			Self::Max => "max",
		}
	}

	/// Zag-mismatch contribution of the edge between `tile` and `other`.
	/// Both variants are symmetric (`count(a, b) == count(b, a)`).
	pub fn count(self, tile: Tile, other: Tile) -> i32 {
		match self {
			Self::Sum => tile.zag_hits_on(other) + other.zag_hits_on(tile),
			Self::Max => TileColor::ALL
				.iter()
				.map(|&c| tile.zag_hits_of_color(other, c).max(other.zag_hits_of_color(tile, c)))
				.sum(),
		}
	}
}

/// Failed-braid mismatch on an edge where at least one tile has a braid and
/// none is fulfilled. Default [`Self::One`] is a single +1 for the edge.
/// [`Self::Weight`] is the largest braid's slot count (2 or 3).
/// [`Self::Sum`] adds every braid's slot count on both sides (2+3 = 5).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum BraidFailMismatch {
	One,
	Weight,
	Sum,
}

impl BraidFailMismatch {
	pub fn parse(s: &str) -> Result<Self, String> {
		match s.to_ascii_lowercase().replace('_', "-").as_str() {
			"one" | "1" | "unit" => Ok(Self::One),
			"weight" | "full" | "braid-weight" | "max" => Ok(Self::Weight),
			"sum" | "threads" | "all" => Ok(Self::Sum),
			other => {
				Err(format!("unknown braid-fail-mismatch '{other}' (expected one|weight|sum)"))
			}
		}
	}

	pub fn name(self) -> &'static str {
		match self {
			Self::One => "one",
			Self::Weight => "weight",
			Self::Sum => "sum",
		}
	}

	/// Mismatch the braid channel applies when no braid on `a`–`b` is fulfilled.
	pub fn cost(self, a: Tile, b: Tile) -> i32 {
		let a_n = a.braid_ring_count();
		let b_n = b.braid_ring_count();
		match self {
			Self::One => 1,
			Self::Weight => i32::from(a_n.max(b_n).max(1)),
			Self::Sum => i32::from(a_n) + i32::from(b_n),
		}
	}
}

/// Independent per-type edge channels. Match weights are positive here; the
/// signed edge total is [`Self::effective`] (`mismatch − match`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct EdgeChannels {
	pub normal_match: i32,
	pub normal_mismatch: i32,
	pub zag_mismatch: i32,
	pub braid_match: i32,
	pub braid_mismatch: i32,
}

impl EdgeChannels {
	pub fn match_weight(self) -> i32 {
		self.normal_match + self.braid_match
	}

	pub fn mismatch_weight(self) -> i32 {
		self.normal_mismatch + self.zag_mismatch + self.braid_mismatch
	}

	/// Per-edge `mismatch_weight − match_weight`. Summed over a tile's edges
	/// and compared with `ring_weight` (`n_rings`).
	pub fn effective(self) -> i32 {
		self.mismatch_weight() - self.match_weight()
	}
}

/// Lose when board tiles + remaining deck cannot reach the control-set occupancy
/// threshold (`control_n`, default 5). Default [`Self::Off`].
///
/// Captures earn spawn tokens, not extra deck tiles. "Can they still get to
/// 5?" is `board + unplaced deck`:
///
/// - [`Self::Strict`]: lose as soon as `board + remaining deck < 5`.
/// - [`Self::NoRecapture`]: lose when `board + remaining deck < 5` **and** they
///   cannot gain a tile this turn. With the default public deck there is no
///   hidden bag, so this matches strict unless `capture_draw_on_move` is on
///   (legacy logs) and a capturing MOVE could still draw.
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
	/// Opening spawn-token stack (that many blank tokens). Default 7.
	/// Independent of [`Self::bag_size`] (the public 11-tile bench). CLI:
	/// `--starting-tokens` / `--starting-bench`. ADD spends 1 token; each
	/// opponent capture earns 1. Own-tile captures earn 1 iff
	/// [`Self::self_capture_budget`]. Usable budget is min(tokens, unplaced
	/// bench tiles).
	pub starting_bench_size: u8,
	/// Public in-game bench size (unplaced tiles that can be ADDed). Default
	/// [`BAG_SIZE_DEFAULT`] (11): the 7+4 draft. Reserves are not this field.
	pub bag_size: u8,
	/// Full collection size per player. Default [`BAG_SIZE`] (27). CLI:
	/// `--deck-size` / `--bag-size` (`1..=`[`BAG_SIZE`]). Each player drafts
	/// a 7-tile bench then 4 more from reserves; leftover tiles stay in
	/// reserve and are never usable in play.
	pub deck_size: u8,
	/// If true (default), capturing your own tile on your turn earns +1
	/// budget the same as capturing an opponent tile. False still removes
	/// the tile (overwhelm is unchanged) but does not refund a token.
	/// CLI: `--self-capture-budget on|off`.
	pub self_capture_budget: bool,
	/// Both players receive the same shuffled [`Self::deck_size`]-tile collection. Default true.
	pub identical_bags: bool,
	/// Fill the collection by sampling [`Self::deck_size`] tiles (historical name: one-of-each
	/// 27-kind). Default true. False leaves an empty collection.
	pub bag_one_of_each_kind: bool,
	/// Max distinct rings per tile (1–3). Default 3. `1` is the
	/// single-ring baseline (still the six-color tree).
	pub max_rings: u8,
	/// Each ring is a zag with probability `zag_numer / 7`. Default 1 (1/7).
	/// `0` disables zags (`--zags off` / `--zag-chance 0`).
	pub zag_numer: u8,
	/// A piece forms a braid with probability `braid_numer / 7`. Default 1
	/// (1/7, same rate as zags). `0` disables braids (`--braids off` /
	/// `--braid-chance 0`).
	pub braid_numer: u8,
	/// Legacy: on a MOVE, each removed opponent tile draws one hidden bag tile
	/// onto the bench. Default **false** (the whole deck is public; captures
	/// earn tokens only). Kept so historical JSONL logs replay.
	pub capture_draw_on_move: bool,
	/// If true, a mismatch adds the neighbor's ring count instead of 1. Default false.
	pub mismatch_uses_neighbor_weight: bool,
	/// Actions a player may take before the turn passes. Default 1.
	pub actions_per_turn: u8,
	/// Draw when this many full turns have been completed. Default 200.
	pub turn_limit: u32,
	/// ADD is only legal on empty spawn nodes. Default true.
	pub add_only_on_spawns: bool,
	/// MOVE must consume exactly `dots` steps. Default true.
	pub move_exact_steps: bool,
	/// Hopped tiles must have ring count >= the mover. Default true.
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
	/// capture exception. Accepted default [`SamePieceTwice::Off`] (doubles
	/// allowed). [`SamePieceTwice::Always`] / [`SamePieceTwice::WhenDeckEmpty`]
	/// remain as non-default variants.
	pub no_same_piece_twice: SamePieceTwice,
	/// If true, a hop may only cross tiles of the mover's color, compared with
	/// [`Self::hop_weight_cmp`]. Default true.
	pub color_restricted_hops: bool,
	/// Occupant-vs-mover ring-count comparison used when color-restricted hops
	/// are on. Default [`HopWeightCmp::Le`] (occupant rings `<=` mover).
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
	/// Lose when board + remaining deck cannot reach 5 tiles (see
	/// [`InsufficientTilesLoss`]). Default off.
	pub insufficient_tiles_loss: InsufficientTilesLoss,
	/// Uniqueness constraint along a single MOVE path.
	/// Default [`PathConstraint::NoEdgeRevisit`] (`no_repeated_edge` on;
	/// node revisits via a different arc are allowed).
	pub path_constraint: PathConstraint,
	/// If true (default), occupied tiles are never hopped: a path may
	/// **pass through** (not land on) an occupant that shares a normal color
	/// with the mover and whose ring count compares with
	/// [`Self::pass_through_ring_cmp`]. Each such node costs one of the
	/// exact-`d` steps. False restores graph hops (`--graph-hops on`).
	pub counted_pass_through: bool,
	/// Occupant-vs-mover ring-count comparison for pass-through.
	/// Default [`PassThroughRingCmp::Ge`] (`occupant.n_rings >= mover`).
	/// [`PassThroughRingCmp::Gt`] is strict, so 3-ring tiles never pass through.
	pub pass_through_ring_cmp: PassThroughRingCmp,
	/// How zag mismatches are counted. Default [`ZagMismatchCount::Sum`]:
	/// hitting zags on either tile are an undirected edge event; both tiles
	/// take that count. [`ZagMismatchCount::Max`] takes, per color, the
	/// larger of the two tiles' hitting zag-stroke counts of that color.
	pub zag_mismatch: ZagMismatchCount,
	/// Failed-braid channel mismatch. Accepted default
	/// [`BraidFailMismatch::One`]: exactly +1 for the edge.
	/// [`BraidFailMismatch::Weight`] / [`BraidFailMismatch::Sum`] are
	/// research variants (issue #39).
	pub braid_fail_mismatch: BraidFailMismatch,
}

impl Default for RuleSet {
	fn default() -> Self {
		Self {
			starting_bench_size: STARTING_TOKENS_DEFAULT,
			bag_size: BAG_SIZE_DEFAULT as u8,
			deck_size: BAG_SIZE as u8,
			self_capture_budget: true,
			identical_bags: true,
			bag_one_of_each_kind: true,
			max_rings: MAX_COLOR_RINGS,
			zag_numer: ZAG_NUMER_DEFAULT,
			braid_numer: BRAID_NUMER_DEFAULT,
			capture_draw_on_move: false,
			mismatch_uses_neighbor_weight: false,
			actions_per_turn: 1,
			turn_limit: 200,
			add_only_on_spawns: true,
			move_exact_steps: true,
			hop_requires_ge_weight: true,
			no_moves_loses: true,
			center_hold_win: false,
			center_hold_n: 5,
			no_same_piece_twice: SamePieceTwice::Off,
			color_restricted_hops: true,
			hop_weight_cmp: HopWeightCmp::Le,
			petal_majority_win: false,
			petal_majority_n: 1,
			control_win: true,
			control_n: 5,
			control_hold_turns: 1,
			insufficient_tiles_loss: InsufficientTilesLoss::Off,
			path_constraint: PathConstraint::NoEdgeRevisit,
			counted_pass_through: true,
			pass_through_ring_cmp: PassThroughRingCmp::Ge,
			zag_mismatch: ZagMismatchCount::Sum,
			braid_fail_mismatch: BraidFailMismatch::One,
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
	/// Spawn-token stack. Tokens are earned and spent here; usable ADD
	/// permission is [`Self::budget`] = min(stack length, remaining deck).
	budget_tokens: [Vec<BudgetToken>; 2],
	/// Original deck in visual order, snapshotted after the opening deal.
	/// The UI shows this many slots for the whole game; used tiles stay here
	/// even after they leave the remaining deck.
	opening_deck: [Vec<Tile>; 2],
	/// Reconstructible 27-tile collections and 7+4 picks. `None` on
	/// constructed [`Self::empty`] fixtures.
	setup: Option<crate::deck::SetupRecord>,
}

impl Game {
	pub fn new(rules: RuleSet, seed: u64) -> Self {
		Self::new_with_setup(rules, seed, crate::deck::SetupConfig::for_seed(seed))
	}

	pub fn new_with_setup(rules: RuleSet, seed: u64, setup: crate::deck::SetupConfig) -> Self {
		Self::new_on(Arc::new(Board::rosette_8()), rules, seed, setup)
	}

	pub fn start(
		rules: RuleSet,
		seed: u64,
		scenario: Option<&crate::scenario::Scenario>,
	) -> Result<Self, String> {
		Self::start_with(rules, seed, scenario, crate::deck::SetupConfig::for_seed(seed))
	}

	pub fn start_with(
		rules: RuleSet,
		seed: u64,
		scenario: Option<&crate::scenario::Scenario>,
		setup: crate::deck::SetupConfig,
	) -> Result<Self, String> {
		match scenario {
			Some(s) => Self::from_scenario_with(rules, seed, s, setup),
			None => Ok(Self::new_with_setup(rules, seed, setup)),
		}
	}

	/// Opening deal from a named or file-backed [`crate::scenario::Scenario`].
	pub fn from_scenario(
		rules: RuleSet,
		seed: u64,
		scenario: &crate::scenario::Scenario,
	) -> Result<Self, String> {
		Self::from_scenario_with(rules, seed, scenario, crate::deck::SetupConfig::for_seed(seed))
	}

	pub fn from_scenario_with(
		rules: RuleSet,
		seed: u64,
		scenario: &crate::scenario::Scenario,
		setup: crate::deck::SetupConfig,
	) -> Result<Self, String> {
		if scenario.generative {
			let mut rules = rules;
			rules.identical_bags = scenario.mirror;
			return Ok(Self::new_with_setup(rules, seed, setup));
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
					Tile::with_typed_rings(id, owner, &spec.parts(), spec.weight, spec.dots)
				})
				.collect::<Vec<_>>()
		};
		let p1_bag = to_tiles(Player::One, bags[0].clone(), &mut next_tile_id);
		let p2_bag = to_tiles(Player::Two, bags[1].clone(), &mut next_tile_id);
		let p1_bench = to_tiles(Player::One, benches[0].clone(), &mut next_tile_id);
		let p2_bench = to_tiles(Player::Two, benches[1].clone(), &mut next_tile_id);
		let mut p1_col = p1_bench;
		p1_col.extend(p1_bag);
		let mut p2_col = p2_bench;
		p2_col.extend(p2_bag);
		if p1_col.is_empty() || p2_col.is_empty() {
			return Err("scenario produced an empty collection".into());
		}
		let record = crate::deck::pick_from_collections([p1_col, p2_col], &rules, &setup, seed);
		let mut game = Self {
			board,
			rules,
			tiles: vec![None; n],
			benches: [record.bench(Player::One), record.bench(Player::Two)],
			bags: [record.reserves(Player::One), record.reserves(Player::Two)],
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
			budget_tokens: [
				blank_budget(rules.starting_bench_size),
				blank_budget(rules.starting_bench_size),
			],
			opening_deck: [Vec::new(), Vec::new()],
			setup: Some(record),
		};
		game.sync_public_bench_size();
		game.snapshot_opening_decks();
		Ok(game)
	}

	pub fn new_on(
		board: Arc<Board>,
		rules: RuleSet,
		seed: u64,
		setup: crate::deck::SetupConfig,
	) -> Self {
		let n = board.node_count();
		let mut next_tile_id = 1u32;
		let record = if rules.bag_one_of_each_kind {
			crate::deck::deal_and_pick(&rules, &setup, seed, &mut next_tile_id)
		} else {
			crate::deck::SetupRecord {
				decks: [Vec::new(), Vec::new()],
				pick7: [Vec::new(), Vec::new()],
				extend: [Vec::new(), Vec::new()],
				strategy: setup.pick,
				deck_seed: setup.deck_seed(seed),
				deck_noise: setup.deck_noise,
			}
		};
		let mut game = Self {
			board,
			rules,
			tiles: vec![None; n],
			benches: [record.bench(Player::One), record.bench(Player::Two)],
			bags: [record.reserves(Player::One), record.reserves(Player::Two)],
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
			budget_tokens: [
				blank_budget(rules.starting_bench_size),
				blank_budget(rules.starting_bench_size),
			],
			opening_deck: [Vec::new(), Vec::new()],
			setup: Some(record),
		};
		game.sync_public_bench_size();
		game.snapshot_opening_decks();
		game
	}

	/// Empty board, empty decks. For constructed test positions.
	pub fn empty(rules: RuleSet) -> Self {
		Self::empty_on(Arc::new(Board::rosette_8()), rules)
	}

	/// Install a completed 7+4 draft. Reserves stay in `bags` and are never usable.
	pub fn from_setup_record(rules: RuleSet, seed: u64, record: crate::deck::SetupRecord) -> Self {
		let mut game = Self::empty(rules);
		game.seed = seed;
		let mut max_id = 0u32;
		for t in record.decks[0].iter().chain(record.decks[1].iter()) {
			max_id = max_id.max(t.id);
		}
		game.next_tile_id = max_id.saturating_add(1).max(1);
		game.benches = [record.bench(Player::One), record.bench(Player::Two)];
		game.bags = [record.reserves(Player::One), record.reserves(Player::Two)];
		game.setup = Some(record);
		game.sync_public_bench_size();
		game.snapshot_opening_decks();
		game
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
			budget_tokens: [
				blank_budget(rules.starting_bench_size),
				blank_budget(rules.starting_bench_size),
			],
			opening_deck: [Vec::new(), Vec::new()],
			setup: None,
		}
	}

	fn sync_public_bench_size(&mut self) {
		let n = self.benches[0].len().max(self.benches[1].len());
		if n > 0 {
			self.rules.bag_size = n as u8;
		}
	}

	fn snapshot_opening_decks(&mut self) {
		self.opening_deck = [
			visual_deck_order(self.bench(Player::One), self.bag(Player::One)),
			visual_deck_order(self.bench(Player::Two), self.bag(Player::Two)),
		];
	}

	/// Fill each player's visual deck to `bag_size` from inferred live/spent
	/// tiles so constructed fixtures have no blank bag faces.
	pub fn remember_full_decks(&mut self) {
		let n = self.rules.bag_size.max(1) as usize;
		for player in [Player::One, Player::Two] {
			let mut deck = self.inferred_opening_deck(player);
			while deck.len() < n {
				let color = TileColor::ALL[deck.len() % TileColor::ALL.len()];
				deck.push(self.alloc_tile(player, color, 1, 1));
			}
			deck.truncate(n);
			self.opening_deck[player.index()] = deck;
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

	/// Board tiles + remaining (unplaced) deck tiles.
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

	/// Original 27-tile (or scenario) collection. Empty on constructed fixtures.
	pub fn collection(&self, player: Player) -> &[Tile] {
		self.setup.as_ref().map(|s| s.decks[player.index()].as_slice()).unwrap_or(&[])
	}

	/// Reconstructible 7+4 draft. `None` on constructed [`Self::empty`] fixtures.
	pub fn setup_record(&self) -> Option<&crate::deck::SetupRecord> {
		self.setup.as_ref()
	}

	/// Usable spawn budget: `min(tokens in the stack, unplaced deck tiles)`.
	/// Tokens beyond the remaining deck stay on the stack (and in the UI) but
	/// cannot be spent until a later capture leaves a tile still unplaced, or
	/// (legacy `capture_draw_on_move`) a draw puts a tile on the bench.
	pub fn budget(&self, player: Player) -> usize {
		self.budget_token_count(player).min(self.bench(player).len())
	}

	/// Physical token-stack length, including unusable surplus.
	pub fn budget_token_count(&self, player: Player) -> usize {
		self.budget_tokens[player.index()].len()
	}

	/// Tokens that exceed the remaining deck, so they cannot be spent yet.
	pub fn budget_surplus(&self, player: Player) -> usize {
		self.budget_token_count(player).saturating_sub(self.budget(player))
	}

	/// Visual capture-budget stack: opening blanks plus captured tiles.
	pub fn budget_tokens(&self, player: Player) -> &[BudgetToken] {
		&self.budget_tokens[player.index()]
	}

	pub fn set_budget_tokens(&mut self, player: Player, tokens: Vec<BudgetToken>) {
		self.budget_tokens[player.index()] = tokens;
	}

	/// Unplaced deck tiles in opening order. Placed tiles leave the row;
	/// the opening deal still shows all [`RuleSet::bag_size`] tiles (default 11).
	pub fn deck_slots(&self, player: Player) -> Vec<DeckSlot> {
		let n = self.rules.bag_size.max(1) as usize;
		let source = if self.opening_deck[player.index()].is_empty() {
			self.inferred_opening_deck(player)
		} else {
			self.opening_deck[player.index()].clone()
		};
		source
			.into_iter()
			.take(n)
			.filter_map(|tile| match self.classify_deck_tile(player, tile) {
				DeckSlot::Live(tile) => Some(DeckSlot::Live(tile)),
				DeckSlot::Spent(_) | DeckSlot::Empty => None,
			})
			.collect()
	}

	fn classify_deck_tile(&self, player: Player, tile: Tile) -> DeckSlot {
		if self.bench(player).iter().any(|t| t.id == tile.id) {
			DeckSlot::Live(tile)
		} else {
			DeckSlot::Spent(tile)
		}
	}

	fn inferred_opening_deck(&self, player: Player) -> Vec<Tile> {
		let mut out = Vec::new();
		let mut seen: Vec<u32> = Vec::new();
		let mut push = |tile: Tile| {
			if !seen.contains(&tile.id) {
				seen.push(tile.id);
				out.push(tile);
			}
		};
		for tile in self.bench(player) {
			push(*tile);
		}
		for (_, tile) in self.tiles() {
			if tile.owner == player {
				push(tile);
			}
		}
		for p in [Player::One, Player::Two] {
			for token in self.budget_tokens(p) {
				if let BudgetToken::Captured(tile) = token {
					if tile.owner == player {
						push(*tile);
					}
				}
			}
		}
		for tile in self.bag(player).iter().rev() {
			push(*tile);
		}
		out
	}

	pub fn bag_len(&self, player: Player) -> usize {
		self.bags[player.index()].len()
	}

	pub fn bag(&self, player: Player) -> &[Tile] {
		&self.bags[player.index()]
	}

	/// Reorder remaining reserve tiles. New games keep reserves unusable;
	/// they are never drawn. Kept so MCTS can determinize legacy
	/// `capture_draw_on_move` positions and historical JSONL replays.
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

	/// Pieces that can still reach the board: on-board tiles plus usable
	/// ADD budget (`min(tokens, unplaced deck tiles)`). Surplus tokens on an
	/// empty remaining deck do not count.
	pub fn possible_board_tiles(&self, player: Player) -> usize {
		self.board_count(player) + self.budget(player)
	}

	/// True when both players have [`Self::possible_board_tiles`] strictly
	/// below [`LOW_VALUE_BOARD_LT`]. Does not require usable budget to be 0.
	pub fn low_value_applies(&self) -> bool {
		let cap = usize::from(LOW_VALUE_BOARD_LT);
		self.possible_board_tiles(Player::One) < cap && self.possible_board_tiles(Player::Two) < cap
	}

	/// Give each side enough remaining pieces that low-value does not already
	/// fire. Fixtures that are not about low-value should call this after
	/// placing their real tiles. `keep_empty` players are padded on the board
	/// (spawns / inner / center left alone) so their remaining deck can stay
	/// empty.
	#[cfg(test)]
	pub(crate) fn pad_above_low_value(&mut self) {
		self.pad_above_low_value_keeping_empty_deck(&[]);
	}

	#[cfg(test)]
	pub(crate) fn pad_above_low_value_keeping_empty_deck(&mut self, keep_empty: &[Player]) {
		let cap = usize::from(LOW_VALUE_BOARD_LT);
		let inner = self.board().inner_ring().to_vec();
		let center = self.board().center();
		let n = self.board().node_count() as NodeId;
		for player in [Player::One, Player::Two] {
			if keep_empty.contains(&player) {
				for id in 0..n {
					if self.possible_board_tiles(player) >= cap {
						break;
					}
					if id == center
						|| inner.contains(&id)
						|| self.board().is_spawn(id)
						|| self.tile_at(id).is_some()
					{
						continue;
					}
					let tile = self.alloc_rings(
						player,
						&[TileColor::Green, TileColor::Blue, TileColor::Orange],
						3,
						1,
					);
					self.place_tile(id, tile);
				}
			} else {
				while self.possible_board_tiles(player) < cap {
					let mut bench = self.bench(player).to_vec();
					bench.push(self.alloc_tile(player, TileColor::Green, 1, 1));
					self.set_bench(player, bench);
				}
				let need = self.bench(player).len();
				if self.budget_token_count(player) < need {
					self.set_budget_tokens(player, vec![BudgetToken::Blank; need]);
				}
			}
			assert!(
				self.possible_board_tiles(player) >= cap,
				"{:?} still under the low-value cap after pad",
				player
			);
		}
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

	pub fn alloc_typed_rings(
		&mut self,
		owner: Player,
		parts: &[(TileColor, RingType)],
		weight: u8,
		dots: u8,
	) -> Tile {
		let id = self.next_tile_id;
		self.next_tile_id += 1;
		Tile::with_typed_rings(id, owner, parts, weight, dots)
	}

	/// Independent per-type channels on the undirected edge `tile`–`other`.
	///
	/// A channel contributes only if at least one of the two tiles has a ring
	/// of that type. Channels never override each other: a zag vs a same-color
	/// normal nets 0, and a fulfilled braid stacks with a normal match.
	///
	/// - **Normal:** match 1 if some normal-ring color of either tile is in
	///   the other tile's color multiset; otherwise mismatch 1 (or the
	///   neighbor's ring count when `mismatch_uses_neighbor_weight`).
	/// - **Zag:** [`RuleSet::zag_mismatch`] hitting-zag count. Never a match.
	/// - **Braid:** the single largest fulfilled braid (match = its slot
	///   count). If none is fulfilled, one mismatch of
	///   [`RuleSet::braid_fail_mismatch`] (default 1).
	pub fn edge_channels(&self, tile: Tile, other: Tile) -> EdgeChannels {
		let mut ch = EdgeChannels::default();
		if tile.normal_ring_count() > 0 || other.normal_ring_count() > 0 {
			if tile.normal_channel_matches(other) {
				ch.normal_match = 1;
			} else if self.rules.mismatch_uses_neighbor_weight {
				ch.normal_mismatch = i32::from(other.weight);
			} else {
				ch.normal_mismatch = 1;
			}
		}
		if tile.zag_ring_count() > 0 || other.zag_ring_count() > 0 {
			ch.zag_mismatch = self.rules.zag_mismatch.count(tile, other);
		}
		if tile.braid_ring_count() >= 2 || other.braid_ring_count() >= 2 {
			if let Some((_, n)) = selected_braid(tile, other) {
				ch.braid_match = i32::from(n);
			} else {
				ch.braid_mismatch = self.rules.braid_fail_mismatch.cost(tile, other);
			}
		}
		ch
	}

	/// Match/mismatch this `tile` receives from neighbor `other`.
	///
	/// Returns `(match_weight, mismatch_weight)` summed across independent
	/// channels. The signed edge total is `mismatch − match`
	/// ([`EdgeChannels::effective`]). A tile is overwhelmed when the sum of
	/// those edge totals is `>=` its ring count.
	pub fn edge_score(&self, tile: Tile, other: Tile) -> (i32, i32) {
		let ch = self.edge_channels(tile, other);
		(ch.match_weight(), ch.mismatch_weight())
	}

	/// Sum of per-edge `mismatch − match` against every incident
	/// generating-circle arc. Inner↔center has two petals, so that neighbor
	/// is scored twice; both tiles take the same undirected channel total on
	/// each petal (not +1 per side of one collapsed pair).
	pub fn tile_effective_weight(&self, node: NodeId) -> Option<i32> {
		let tile = self.tile_at(node)?;
		let mut effective = 0i32;
		for nb in self.board.incident_others(node) {
			if let Some(other) = self.tile_at(nb) {
				effective += self.edge_channels(tile, other).effective();
			}
		}
		Some(effective)
	}

	/// Occupied-edge braid stats: `(edges with a braid, satisfied of those)`.
	/// Each generating-circle arc is counted once (center↔inner is two).
	pub fn braid_edge_counts(&self) -> (u32, u32) {
		let mut with_braid = 0u32;
		let mut satisfied = 0u32;
		for arc in self.board.arcs() {
			let Some(a) = self.tile_at(arc.from) else {
				continue;
			};
			let Some(b) = self.tile_at(arc.to) else {
				continue;
			};
			if a.braid_ring_count() < 2 && b.braid_ring_count() < 2 {
				continue;
			}
			with_braid += 1;
			if selected_braid(a, b).is_some() {
				satisfied += 1;
			}
		}
		(with_braid, satisfied)
	}

	pub fn tile_score(&self, node: NodeId) -> Option<(i32, i32)> {
		let tile = self.tile_at(node)?;
		let mut matches = i32::from(tile.weight);
		let mut mismatches = 0i32;
		for nb in self.board.incident_others(node) {
			if let Some(other) = self.tile_at(nb) {
				let (m, x) = self.edge_score(tile, other);
				matches += m;
				mismatches += x;
			}
		}
		Some((matches, mismatches))
	}

	pub fn is_overwhelmed(&self, node: NodeId) -> bool {
		match (self.tile_at(node), self.tile_effective_weight(node)) {
			(Some(tile), Some(eff)) => eff >= i32::from(tile.weight),
			_ => false,
		}
	}

	/// Would `tile` be overwhelmed if placed at `node` (other tiles unchanged)?
	pub fn would_overwhelm_add(&self, tile: Tile, node: NodeId) -> bool {
		if self.tile_at(node).is_some() {
			return false;
		}
		let mut effective = 0i32;
		for nb in self.board.incident_others(node) {
			if let Some(other) = self.tile_at(nb) {
				effective += self.edge_channels(tile, other).effective();
			}
		}
		effective >= i32::from(tile.weight)
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
		// Usable budget is 0 when the stack is empty or the remaining deck is empty.
		if self.budget(self.current) == 0 {
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
			SamePieceTwice::WhenDeckEmpty => self.benches[player.index()].is_empty(),
		}
	}

	/// True if `player` has a legal MOVE (as if it were their turn) that would
	/// remove at least one opponent tile. Used by `no-recapture` insufficient-
	/// tiles when legacy `capture_draw_on_move` can still gain a tile.
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
			for &next in self.board.neighbors(state.node) {
				for edge in self.board.connecting_arc_indices(state.node, next) {
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
					} else if self.rules.counted_pass_through {
						let Some(occupant) = occupancy[next as usize] else {
							continue;
						};
						if !self.pass_through_ok(occupant, tile) {
							continue;
						}
						if forbid_node && state.visited_nodes & (1u32 << next) != 0 {
							continue;
						}
						stack.push(PathState {
							node: next,
							remaining: state.remaining - 1,
							visited_nodes: state.visited_nodes | (1u32 << next),
							visited_edges: state.visited_edges | (1u64 << edge),
						});
					} else {
						for (land, hopped_nodes, hopped_edges) in
							self.hop_from(&occupancy, next, edge, tile, state.visited_edges)
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
		}
		found.sort();
		found
	}

	fn pass_through_ok(&self, occupant: Tile, mover: Tile) -> bool {
		occupant.shares_normal(mover)
			&& self.rules.pass_through_ring_cmp.allows(occupant.n_rings, mover.n_rings)
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

	/// Hop landings from an occupied entry `first` reached via `entry_edge`.
	/// Floods any-adjacency through hoppable occupied tiles (shared normal,
	/// weight ok), turning at every node including the center, and lands on
	/// any empty neighbor of that chain. The whole hop costs one step.
	fn hop_from(
		&self,
		occupancy: &[Option<Tile>],
		first: NodeId,
		entry_edge: usize,
		mover: Tile,
		already_used: u64,
	) -> Vec<(NodeId, u32, u64)> {
		let Some(occupant) = occupancy[first as usize] else {
			return Vec::new();
		};
		if !self.hop_occupant_ok(occupant, mover) {
			return Vec::new();
		}
		if already_used & (1u64 << entry_edge) != 0 {
			return Vec::new();
		}
		let mut hopped_nodes = 1u32 << first;
		let mut occ_edges = 1u64 << entry_edge;
		let mut queue = VecDeque::from([first]);
		while let Some(t) = queue.pop_front() {
			for &u in self.board.neighbors(t) {
				let Some(next_occ) = occupancy[u as usize] else {
					continue;
				};
				if !self.hop_occupant_ok(next_occ, mover) {
					continue;
				}
				if hopped_nodes & (1u32 << u) != 0 {
					continue;
				}
				let used = already_used | occ_edges;
				let Some(edge) =
					self.board.connecting_arc_indices(t, u).find(|&e| used & (1u64 << e) == 0)
				else {
					continue;
				};
				hopped_nodes |= 1u32 << u;
				occ_edges |= 1u64 << edge;
				queue.push_back(u);
			}
		}
		let mut out = Vec::new();
		let mut t = 0u8;
		while t < 32 {
			if hopped_nodes & (1u32 << t) == 0 {
				t += 1;
				continue;
			}
			for &u in self.board.neighbors(t) {
				if occupancy[u as usize].is_some() {
					continue;
				}
				for edge in self.board.connecting_arc_indices(t, u) {
					if already_used & (1u64 << edge) != 0 || occ_edges & (1u64 << edge) != 0 {
						continue;
					}
					let edges = occ_edges | (1u64 << edge);
					if !out
						.iter()
						.any(|&(land, nodes, ed)| land == u && nodes == hopped_nodes && ed == edges)
					{
						out.push((u, hopped_nodes, edges));
					}
				}
			}
			t += 1;
		}
		out
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
			// Opponent tiles always earn +1. Own-tile removals (self-capture
			// via simultaneous overwhelm) earn +1 only when
			// `self_capture_budget` is on (default).
			if rem.tile.owner != player || self.rules.self_capture_budget {
				self.earn_budget(player, rem.tile);
			}
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
		if !self.status.is_over() {
			self.check_low_value();
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

	/// Both players have `board + usable budget` strictly below [`LOW_VALUE_BOARD_LT`].
	/// Usable budget need not be 0: a side with 1 on the board and 3 unplaced
	/// tiles (usable 3) is already below 5. Surplus tokens on an empty
	/// remaining deck do not count toward possible pieces. Compared after
	/// lockout: a player who cannot act still loses by lockout even if they
	/// hold the lowest piece.
	fn check_low_value(&mut self) {
		if self.status.is_over() {
			return;
		}
		if !self.low_value_applies() {
			return;
		}
		self.status = match self.low_value_resolution() {
			LowValueResolution::Draw => GameStatus::Draw { reason: EndReason::LowValue },
			LowValueResolution::Win { winner, .. } => {
				GameStatus::Won { winner, reason: EndReason::LowValue }
			}
		};
	}

	#[cfg(test)]
	pub(crate) fn evaluate_low_value_for_test(&mut self) {
		self.check_low_value();
	}

	/// Sorted `(primary, tiebreak)` keys of `player`'s on-board pieces,
	/// ascending. Used by the progressive lowest-tile pair check.
	pub fn sorted_piece_keys(&self, player: Player) -> Vec<(i32, i32)> {
		let mut keys: Vec<(i32, i32)> = self
			.tiles()
			.filter(|(_, tile)| tile.owner == player)
			.map(|(_, tile)| {
				(
					tile.piece_value(PieceValueWeights::PRIMARY),
					tile.piece_value(PieceValueWeights::TIEBREAK),
				)
			})
			.collect();
		keys.sort_unstable();
		keys
	}

	/// Progressive lowest-tile pair check. Each player's pieces are sorted
	/// by [`PieceValueWeights::PRIMARY`] then [`PieceValueWeights::TIEBREAK`].
	/// Pair `i` compares those keys (primary pass, then zag-weighted
	/// tiebreak). A missing piece is maximum value, so the side with more
	/// tiles wins once every real pair has tied. Equal counts with every
	/// pair tied on both passes is a draw.
	pub fn low_value_resolution(&self) -> LowValueResolution {
		let a = self.sorted_piece_keys(Player::One);
		let b = self.sorted_piece_keys(Player::Two);
		let n = a.len().max(b.len());
		for i in 0..n {
			let a_missing = i >= a.len();
			let b_missing = i >= b.len();
			let ka = if a_missing { LOW_VALUE_EMPTY } else { a[i] };
			let kb = if b_missing { LOW_VALUE_EMPTY } else { b[i] };
			if ka.0 != kb.0 {
				return LowValueResolution::Win {
					winner: if ka.0 < kb.0 { Player::One } else { Player::Two },
					pair: i,
					by_piece_count: a_missing || b_missing,
				};
			}
			if ka.1 != kb.1 {
				return LowValueResolution::Win {
					winner: if ka.1 < kb.1 { Player::One } else { Player::Two },
					pair: i,
					by_piece_count: a_missing || b_missing,
				};
			}
		}
		LowValueResolution::Draw
	}

	fn spend_budget(&mut self, player: Player) -> Result<(), String> {
		let tokens = &mut self.budget_tokens[player.index()];
		if tokens.is_empty() {
			return Err("no capture budget".into());
		}
		tokens.remove(0);
		Ok(())
	}

	fn earn_budget(&mut self, player: Player, tile: Tile) {
		self.budget_tokens[player.index()].push(BudgetToken::Captured(tile));
	}

	fn apply_add(&mut self, bench_index: usize, node: NodeId) -> Result<(), String> {
		if self.budget(self.current) == 0 {
			return Err("no capture budget".into());
		}
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
		self.spend_budget(self.current)?;
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
	braid_numer: u8,
	next_id: &mut u32,
	rng: &mut impl Rng,
) -> Tile {
	let (mut rings, mut n_rings) = sample_rings(rng, max_rings);
	let mut ring_type = sample_braid(rng, &mut rings, &mut n_rings, max_rings, braid_numer);
	sample_zag_types(rng, n_rings, &mut ring_type, zag_numer);
	let weight = n_rings.max(1);
	let dots = rng.gen_range(1..=3);
	let id = *next_id;
	*next_id += 1;
	Tile { id, owner, rings, n_rings, ring_type, weight, dots }
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
	fn take_rings(game: &mut Game, owner: Player, colors: &[TileColor], dots: u8) -> Tile {
		game.alloc_rings(owner, colors, colors.len() as u8, dots)
	}
	fn put_rings(game: &mut Game, node: NodeId, owner: Player, colors: &[TileColor], dots: u8) {
		let tile = take_rings(game, owner, colors, dots);
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
	fn opening_deal_is_full_public_deck_with_seven_tokens() {
		let a = Game::new(RuleSet::default(), 7);
		let b = Game::new(RuleSet::default(), 7);
		assert_eq!(a.bench(Player::One).len(), BAG_SIZE_DEFAULT);
		assert_eq!(a.bench(Player::Two).len(), BAG_SIZE_DEFAULT);
		assert_eq!(a.budget(Player::One), 7);
		assert_eq!(a.budget(Player::Two), 7);
		assert!(a.budget_tokens(Player::One).iter().all(|t| matches!(t, BudgetToken::Blank)));
		assert_eq!(a.collection(Player::One).len(), BAG_SIZE);
		assert_eq!(a.bag_len(Player::One), BAG_SIZE - BAG_SIZE_DEFAULT);
		assert_eq!(a.bag_len(Player::Two), BAG_SIZE - BAG_SIZE_DEFAULT);
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
		assert!(crate::deck::respects_type_cap(a.bench(Player::One)));
	}

	#[test]
	fn opening_tokens_are_independent_of_deck_size() {
		let mut rules = RuleSet::default();
		rules.starting_bench_size = 5;
		let game = Game::new(rules, 1);
		assert_eq!(game.bench(Player::One).len(), BAG_SIZE_DEFAULT);
		assert_eq!(game.bench(Player::Two).len(), BAG_SIZE_DEFAULT);
		assert_eq!(game.budget(Player::One), 5);
		assert_eq!(game.budget(Player::Two), 5);
		assert_eq!(game.bag_len(Player::One), BAG_SIZE - BAG_SIZE_DEFAULT);
		assert_eq!(game.bag_len(Player::Two), BAG_SIZE - BAG_SIZE_DEFAULT);
		assert_eq!(game.legal_adds().len(), BAG_SIZE_DEFAULT * 8);
	}

	#[test]
	fn visual_deck_shows_only_unplaced_tiles() {
		let mut game = Game::new(RuleSet::default(), 3);
		let before = game.deck_slots(Player::One);
		assert_eq!(before.len(), BAG_SIZE_DEFAULT);
		assert_eq!(before.iter().filter(|s| s.is_live()).count(), BAG_SIZE_DEFAULT);
		assert_eq!(before.iter().filter(|s| s.shows_blank_face()).count(), 0);
		let action = game.legal_adds().into_iter().next().expect("opening ADD");
		let added_id = match action {
			Action::Add { bench_index, .. } => game.bench(Player::One)[bench_index].id,
			other => panic!("expected ADD, got {other:?}"),
		};
		game.apply(action).unwrap();
		assert_eq!(game.bench(Player::One).len(), BAG_SIZE_DEFAULT - 1);
		let after = game.deck_slots(Player::One);
		assert_eq!(after.len(), BAG_SIZE_DEFAULT - 1);
		assert!(after.iter().all(|s| s.is_live()));
		assert!(!after.iter().any(|s| s.tile().is_some_and(|t| t.id == added_id)));
		assert_eq!(after.iter().filter(|s| matches!(s, DeckSlot::Spent(_))).count(), 0);
	}

	#[test]
	fn opening_deal_respects_bag_size_eleven() {
		let game = Game::new(RuleSet::default(), 1);
		assert_eq!(game.rules().bag_size, BAG_SIZE_DEFAULT as u8);
		assert_eq!(game.rules().deck_size, BAG_SIZE as u8);
		assert_eq!(game.bench(Player::One).len(), BAG_SIZE_DEFAULT);
		assert_eq!(game.bench(Player::Two).len(), BAG_SIZE_DEFAULT);
		assert_eq!(game.budget(Player::One), 7);
		assert_eq!(game.bag_len(Player::One), BAG_SIZE - BAG_SIZE_DEFAULT);
		assert_eq!(game.bag_len(Player::Two), BAG_SIZE - BAG_SIZE_DEFAULT);
		assert_eq!(game.legal_adds().len(), BAG_SIZE_DEFAULT * 8);
	}

	#[test]
	fn opening_deal_respects_deck_size_twenty_seven() {
		let mut rules = RuleSet::default();
		rules.deck_size = BAG_SIZE as u8;
		let game = Game::new(rules, 1);
		assert_eq!(game.collection(Player::One).len(), BAG_SIZE);
		assert_eq!(game.bench(Player::One).len(), BAG_SIZE_DEFAULT);
		assert_eq!(game.bag_len(Player::One), BAG_SIZE - BAG_SIZE_DEFAULT);
		assert_eq!(game.budget(Player::One), 7);
	}

	#[test]
	fn opening_deal_respects_deck_size_thirteen() {
		let mut rules = RuleSet::default();
		rules.deck_size = 13;
		let game = Game::new(rules, 2);
		assert_eq!(game.collection(Player::One).len(), 13);
		assert_eq!(game.bench(Player::One).len(), BAG_SIZE_DEFAULT);
		assert_eq!(game.bag_len(Player::One), 2);
		assert_eq!(game.budget(Player::One), 7);
	}

	#[test]
	fn tokens_can_exceed_a_small_deck() {
		let mut rules = RuleSet::default();
		rules.deck_size = 7;
		rules.starting_bench_size = 7;
		let game = Game::new(rules, 3);
		assert_eq!(game.collection(Player::One).len(), 7);
		assert_eq!(game.bench(Player::One).len(), 7);
		assert_eq!(game.bag_len(Player::One), 0);
		assert_eq!(game.budget(Player::One), 7);
	}

	#[test]
	fn default_rules_are_control_win_with_seven_tokens() {
		let rules = RuleSet::default();
		assert!(!rules.center_hold_win);
		assert!(rules.control_win);
		assert_eq!(rules.control_n, 5);
		assert_eq!(rules.control_hold_turns, 1);
		assert_eq!(rules.starting_bench_size, 7);
		assert_eq!(rules.bag_size, BAG_SIZE_DEFAULT as u8);
		assert_eq!(rules.deck_size, BAG_SIZE as u8);
		assert!(!rules.capture_draw_on_move);
		assert!(rules.self_capture_budget);
		assert_eq!(rules.max_rings, 3);
		assert_eq!(rules.zag_numer, ZAG_NUMER_DEFAULT);
		assert_eq!(rules.braid_numer, BRAID_NUMER_DEFAULT);
		assert!(!rules.petal_majority_win);
		assert_eq!(rules.insufficient_tiles_loss, InsufficientTilesLoss::Off);
		assert!(rules.counted_pass_through);
		assert_eq!(rules.pass_through_ring_cmp, PassThroughRingCmp::Ge);
		assert_eq!(rules.zag_mismatch, ZagMismatchCount::Sum);
		assert_eq!(rules.braid_fail_mismatch, BraidFailMismatch::One);
		assert_eq!(rules.no_same_piece_twice, SamePieceTwice::Off);
		assert!(rules.no_moves_loses);
		let game = Game::new(rules, 1);
		assert_eq!(game.bench(Player::One).len(), BAG_SIZE_DEFAULT);
		assert_eq!(game.bench(Player::Two).len(), BAG_SIZE_DEFAULT);
		assert_eq!(game.budget(Player::One), 7);
		assert_eq!(game.bag_len(Player::One), BAG_SIZE - BAG_SIZE_DEFAULT);
		assert_eq!(game.legal_adds().len(), BAG_SIZE_DEFAULT * 8);
	}

	#[test]
	fn reserves_are_never_legal_adds() {
		let game = Game::new(RuleSet::default(), 4);
		let reserve_ids: Vec<u32> = game.bag(Player::One).iter().map(|t| t.id).collect();
		assert_eq!(reserve_ids.len(), BAG_SIZE - BAG_SIZE_DEFAULT);
		for action in game.legal_adds() {
			let Action::Add { bench_index, .. } = action else {
				panic!("expected ADD");
			};
			let id = game.bench(Player::One)[bench_index].id;
			assert!(!reserve_ids.contains(&id), "reserve {id} must not be ADD-able");
		}
		assert!(crate::deck::respects_type_cap(game.bench(Player::One)));
		assert!(crate::deck::respects_type_cap(game.setup_record().unwrap().pick7[0].as_slice()));
	}

	#[test]
	fn from_setup_record_installs_the_draft() {
		let rec = crate::deck::deal_and_pick(
			&RuleSet::default(),
			&crate::deck::SetupConfig::for_seed(11),
			11,
			&mut 1u32,
		);
		let game = Game::from_setup_record(RuleSet::default(), 11, rec.clone());
		assert_eq!(game.bench(Player::One), rec.bench(Player::One));
		assert_eq!(game.bag(Player::One), rec.reserves(Player::One));
		assert_eq!(game.collection(Player::One).len(), BAG_SIZE);
		assert_eq!(game.deck_slots(Player::One).len(), BAG_SIZE_DEFAULT);
	}

	#[test]
	fn double_green_overwhelmed_by_two_oranges_survives_with_green_neighbor() {
		let mut game = Game::empty(RuleSet::default());
		let spawn = game.board().spawns()[0];
		let nbs = game.board().neighbors(spawn).to_vec();
		assert!(nbs.len() >= 3);
		let double = game.alloc_ring_kinds(
			Player::One,
			&[(TileColor::Green, false), (TileColor::Green, false)],
			2,
			1,
		);
		game.place_tile(spawn, double);
		put(&mut game, nbs[0], Player::Two, TileColor::Orange, 1, 1);
		put(&mut game, nbs[1], Player::One, TileColor::Orange, 1, 2);
		let (m, n) = game.tile_score(spawn).unwrap();
		assert_eq!((m, n), (2, 2), "own ring count 2, two different-color neighbors");
		assert!(game.is_overwhelmed(spawn));

		put(&mut game, nbs[2], Player::Two, TileColor::Green, 1, 3);
		let (m, n) = game.tile_score(spawn).unwrap();
		assert_eq!((m, n), (3, 2), "two green rings plus one green neighbor");
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
		put(&mut game, spawn, Player::One, TileColor::Green, 1, 1);
		let heavy = game.alloc_rings(
			Player::Two,
			&[TileColor::Orange, TileColor::Blue, TileColor::Teal],
			3,
			1,
		);
		game.place_tile(nbs[0], heavy);
		let (m, n) = game.tile_score(spawn).unwrap();
		assert_eq!(m, 1, "own ring count");
		assert_eq!(n, 3, "mismatch uses neighbor ring count");
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

	fn hop_rules() -> RuleSet {
		let mut rules = RuleSet::default();
		rules.counted_pass_through = false;
		rules
	}

	#[test]
	fn hop_costs_one_and_requires_weight() {
		let mut rules = hop_rules();
		rules.color_restricted_hops = false;
		let mut game = Game::empty(rules);
		let b = game.board().clone();
		let (from, hop_over, land) = {
			let circle = 0u8;
			let cycle = b.circle_cycle(circle);
			(cycle[0], cycle[1], cycle[2])
		};
		put(&mut game, from, Player::One, TileColor::Green, 1, 1);
		let heavier = game.alloc_rings(Player::Two, &[TileColor::Orange, TileColor::Blue], 2, 1);
		game.place_tile(hop_over, heavier);
		game.set_current(Player::One);
		let dests = game.destinations(from);
		assert!(dests.contains(&land), "d=1 hop over heavier/equal tile to {land}, got {dests:?}");

		game.clear_tile(hop_over);
		put(&mut game, hop_over, Player::Two, TileColor::Orange, 1, 1);
		game.clear_tile(from);
		let heavy_mover = game.alloc_rings(
			Player::One,
			&[TileColor::Green, TileColor::Blue, TileColor::Teal],
			3,
			1,
		);
		game.place_tile(from, heavy_mover);
		game.set_current(Player::One);
		let dests = game.destinations(from);
		assert!(!dests.contains(&land), "cannot hop a lighter (fewer-ring) tile");
	}

	#[test]
	fn hop_along_a_run_of_equal_or_greater_weight() {
		let mut rules = hop_rules();
		rules.color_restricted_hops = false;
		let mut game = Game::empty(rules);
		let b = game.board().clone();
		let circle = 0u8;
		let cycle = b.circle_cycle(circle).to_vec();
		let from = cycle[0];
		let a = cycle[1];
		let c = cycle[2];
		let land = cycle[3];
		let mover = game.alloc_rings(Player::One, &[TileColor::Blue, TileColor::Teal], 2, 1);
		game.place_tile(from, mover);
		let hop_a = game.alloc_rings(Player::Two, &[TileColor::Green, TileColor::Maroon], 2, 1);
		game.place_tile(a, hop_a);
		let hop_c = game.alloc_rings(
			Player::One,
			&[TileColor::Orange, TileColor::LavenderPink, TileColor::Blue],
			3,
			1,
		);
		game.place_tile(c, hop_c);
		game.set_current(Player::One);
		let dests = game.destinations(from);
		assert!(dests.contains(&land), "hop the two-tile run to {land}");
	}

	#[test]
	fn hop_turns_at_the_center_and_lands_on_any_adjacent_empty() {
		let mut game = Game::empty(hop_rules());
		let board = game.board().clone();
		let center = board.center();
		let spawn = board.spawns()[0];
		let inner = *board
			.neighbors(spawn)
			.iter()
			.find(|&&n| board.neighbors(n).contains(&center))
			.expect("spawn neighbors an inner that touches center");
		let other_inner = *board
			.neighbors(center)
			.iter()
			.find(|&&n| n != inner)
			.expect("center has another inner neighbor");
		assert!(!board.neighbors(spawn).contains(&other_inner));
		put(&mut game, spawn, Player::One, TileColor::Green, 3, 1);
		put(&mut game, inner, Player::Two, TileColor::Green, 1, 1);
		put(&mut game, center, Player::Two, TileColor::Green, 1, 1);
		game.set_current(Player::One);
		let dests = game.destinations(spawn);
		assert!(
			dests.contains(&other_inner),
			"d=1 hop must turn at center onto inner {other_inner}, got {dests:?}"
		);
		assert!(!dests.contains(&inner), "cannot land on an occupied hoppable tile");
		assert!(!dests.contains(&center), "cannot land on an occupied hoppable tile");
	}

	#[test]
	fn hop_changes_direction_at_a_non_center_node() {
		let mut game = Game::empty(hop_rules());
		let board = game.board().clone();
		let spawn = board.spawns()[0];
		let inner = *board
			.neighbors(spawn)
			.iter()
			.find(|&&n| !board.is_spawn(n) && n != board.center())
			.expect("spawn has an inner neighbor");
		let side = *board
			.neighbors(inner)
			.iter()
			.find(|&&n| n != spawn && n != board.center() && game.tile_at(n).is_none())
			.expect("inner has a side neighbor");
		let land = *board
			.neighbors(side)
			.iter()
			.find(|&&n| {
				n != inner && game.tile_at(n).is_none() && !board.neighbors(spawn).contains(&n)
			})
			.expect("side node has a landing that is not a spawn neighbor");
		put(&mut game, spawn, Player::One, TileColor::Blue, 3, 1);
		put(&mut game, inner, Player::Two, TileColor::Blue, 1, 1);
		put(&mut game, side, Player::One, TileColor::Blue, 1, 1);
		game.set_current(Player::One);
		let dests = game.destinations(spawn);
		assert!(
			dests.contains(&land),
			"hop chain spawn-inner-side must land on {land} off the entry circle, got {dests:?}"
		);
	}

	fn pass_through_game() -> Game {
		Game::empty(RuleSet::default())
	}

	fn pass_through_with(cmp: PassThroughRingCmp) -> Game {
		let mut rules = RuleSet::default();
		rules.counted_pass_through = true;
		rules.pass_through_ring_cmp = cmp;
		Game::empty(rules)
	}

	#[test]
	fn counted_pass_through_is_exact_d_not_up_to() {
		let mut game = pass_through_game();
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
		assert!(found, "spawn should have a 2-step empty walk");
		let tile = green(&mut game, Player::One, 1, 2);
		game.place_tile(from, tile);
		game.set_current(Player::One);
		let dests2 = game.destinations(from);
		assert!(dests2.contains(&step2), "d=2 must land on {step2}, got {dests2:?}");
		game.clear_tile(from);
		let one = green(&mut game, Player::One, 1, 1);
		game.place_tile(from, one);
		let dests1 = game.destinations(from);
		assert!(dests1.contains(&step1), "d=1 should reach {step1}");
		assert!(!dests1.contains(&step2), "d=1 must not reach the two-step node {step2}");
		assert_ne!(dests1, dests2, "exact-d MOVE: d=1 and d=2 destination sets differ");
	}

	#[test]
	fn counted_pass_through_costs_one_and_cannot_land_on_occupant() {
		let mut game = pass_through_game();
		let b = game.board().clone();
		let cycle = b.circle_cycle(0).to_vec();
		let (from, mid, land) = (cycle[0], cycle[1], cycle[2]);
		put(&mut game, from, Player::One, TileColor::Green, 1, 2);
		put(&mut game, mid, Player::Two, TileColor::Green, 1, 1);
		game.set_current(Player::One);
		let dests = game.destinations(from);
		assert!(dests.contains(&land), "d=2 pass-through to {land}, got {dests:?}");
		assert!(!dests.contains(&mid), "cannot land on the occupied pass-through tile");
		game.clear_tile(from);
		put(&mut game, from, Player::One, TileColor::Green, 1, 1);
		let dests1 = game.destinations(from);
		assert!(
			!dests1.contains(&land),
			"d=1 cannot pass through and land beyond; pass-through costs a space"
		);
		assert!(!dests1.contains(&mid), "d=1 cannot land on occupied");
	}

	#[test]
	fn counted_pass_through_two_tiles_cost_two_unlike_a_hop() {
		let mut hop = Game::empty(hop_rules());
		let mut pass = pass_through_game();
		let b = hop.board().clone();
		let cycle = b.circle_cycle(0).to_vec();
		let (from, a, c, land) = (cycle[0], cycle[1], cycle[2], cycle[3]);
		for game in [&mut hop, &mut pass] {
			put(game, from, Player::One, TileColor::Blue, 2, 1);
			put(game, a, Player::Two, TileColor::Blue, 1, 1);
			put(game, c, Player::One, TileColor::Blue, 1, 1);
			game.set_current(Player::One);
		}
		assert!(hop.destinations(from).contains(&land), "hop skips the two-tile chain in one step");
		assert!(
			!pass.destinations(from).contains(&land),
			"d=1 pass-through cannot clear two occupied tiles plus a landing"
		);
		pass.clear_tile(from);
		put(&mut pass, from, Player::One, TileColor::Blue, 2, 3);
		pass.set_current(Player::One);
		let pass_d3 = pass.destinations(from);
		assert!(
			pass_d3.contains(&land),
			"d=3 pass-through reaches {land} (A + C + land), got {pass_d3:?}"
		);
	}

	#[test]
	fn counted_pass_through_needs_shared_normal_and_enough_rings() {
		let mut game = pass_through_game();
		let b = game.board().clone();
		let cycle = b.circle_cycle(0).to_vec();
		let (from, mid, land) = (cycle[0], cycle[1], cycle[2]);
		let mover = game.alloc_rings(Player::One, &[TileColor::Green, TileColor::Blue], 2, 2);
		game.place_tile(from, mover);
		put(&mut game, mid, Player::Two, TileColor::Orange, 2, 1);
		game.set_current(Player::One);
		assert!(
			!game.destinations(from).contains(&land),
			"other-color occupant cannot be passed through"
		);
		game.clear_tile(mid);
		let zag = game.alloc_ring_kinds(Player::Two, &[(TileColor::Green, true)], 1, 1);
		game.place_tile(mid, zag);
		assert!(!game.destinations(from).contains(&land), "zag green is not a pass-through color");
		game.clear_tile(mid);
		put(&mut game, mid, Player::Two, TileColor::Green, 1, 1);
		assert!(
			!game.destinations(from).contains(&land),
			"occupant with fewer rings than the mover cannot be passed through"
		);
		game.clear_tile(mid);
		let enough = game.alloc_rings(Player::Two, &[TileColor::Green, TileColor::Orange], 2, 1);
		game.place_tile(mid, enough);
		assert!(
			game.destinations(from).contains(&land),
			"shared normal and occupant.n_rings >= mover.n_rings"
		);
	}

	#[test]
	fn pass_through_ring_cmp_gt_is_strict() {
		let mut ge = pass_through_with(PassThroughRingCmp::Ge);
		let b = ge.board().clone();
		let cycle = b.circle_cycle(0).to_vec();
		let (from, mid, land) = (cycle[0], cycle[1], cycle[2]);
		let place = |game: &mut Game, occ_rings: &[TileColor]| {
			let mover = game.alloc_rings(Player::One, &[TileColor::Green, TileColor::Blue], 2, 2);
			game.place_tile(from, mover);
			let occ = game.alloc_rings(Player::Two, occ_rings, 2, 1);
			game.place_tile(mid, occ);
			game.set_current(Player::One);
		};

		place(&mut ge, &[TileColor::Green, TileColor::Orange]);
		assert!(ge.destinations(from).contains(&land), "GE allows equal ring counts");

		let mut gt_eq = pass_through_with(PassThroughRingCmp::Gt);
		place(&mut gt_eq, &[TileColor::Green, TileColor::Orange]);
		assert!(
			!gt_eq.destinations(from).contains(&land),
			"GT forbids occupant.n_rings == mover.n_rings"
		);

		let mut gt_gt = pass_through_with(PassThroughRingCmp::Gt);
		place(&mut gt_gt, &[TileColor::Green, TileColor::Orange, TileColor::Blue]);
		assert!(
			gt_gt.destinations(from).contains(&land),
			"GT allows occupant.n_rings > mover.n_rings"
		);
	}

	#[test]
	fn pass_through_gt_triples_cannot_pass_through() {
		let mut game = pass_through_with(PassThroughRingCmp::Gt);
		let b = game.board().clone();
		let cycle = b.circle_cycle(0).to_vec();
		let (from, mid, land) = (cycle[0], cycle[1], cycle[2]);
		let mover = game.alloc_rings(
			Player::One,
			&[TileColor::Green, TileColor::Blue, TileColor::Orange],
			3,
			2,
		);
		game.place_tile(from, mover);
		let occ = game.alloc_rings(
			Player::Two,
			&[TileColor::Green, TileColor::Blue, TileColor::Orange],
			3,
			1,
		);
		game.place_tile(mid, occ);
		game.set_current(Player::One);
		assert!(
			!game.destinations(from).contains(&land),
			"GT: a 3-ring mover can never pass through (max rings is 3)"
		);

		let mut ge = pass_through_with(PassThroughRingCmp::Ge);
		let mover = ge.alloc_rings(
			Player::One,
			&[TileColor::Green, TileColor::Blue, TileColor::Orange],
			3,
			2,
		);
		ge.place_tile(from, mover);
		let occ = ge.alloc_rings(
			Player::Two,
			&[TileColor::Green, TileColor::Blue, TileColor::Orange],
			3,
			1,
		);
		ge.place_tile(mid, occ);
		ge.set_current(Player::One);
		assert!(
			ge.destinations(from).contains(&land),
			"GE still lets a 3-ring tile pass through another 3-ring tile"
		);
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
	fn capture_does_not_draw_on_add_or_move() {
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
		assert_eq!(game.bag_len(Player::One), bag_before, "captures do not draw from a hidden bag");
		assert_eq!(outcome.draws, 0);
	}

	#[test]
	fn move_capture_earns_a_token_and_does_not_draw() {
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
		let tokens_before = game.budget_token_count(Player::One);
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
		assert_eq!(outcome.draws, 0);
		assert_eq!(game.bag_len(Player::One), bag_before);
		assert_eq!(game.bench(Player::One).len(), bench_before);
		assert_eq!(game.budget_token_count(Player::One), tokens_before + outcome.removals.len());
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
	fn hidden_bag_does_not_keep_a_player_alive() {
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
		assert_eq!(adds.len(), BAG_SIZE_DEFAULT * 8);
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
		game.pad_above_low_value();
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
		put_rings(
			&mut game,
			from,
			Player::One,
			&[TileColor::Blue, TileColor::Teal, TileColor::Maroon],
			1,
		);
		put_rings(
			&mut game,
			inner[1],
			Player::One,
			&[TileColor::Green, TileColor::LavenderPink, TileColor::Orange],
			1,
		);
		{
			let t = take(&mut game, Player::One, TileColor::Green, 3, 1);
			game.set_bench(Player::One, vec![t]);
		}
		{
			let t = take(&mut game, Player::Two, TileColor::Orange, 3, 1);
			game.set_bench(Player::Two, vec![t]);
		}
		game.pad_above_low_value();
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
			put_rings(
				&mut game,
				node,
				Player::One,
				&[TileColor::Green, TileColor::Teal, TileColor::Maroon],
				1,
			);
		}
		{
			let t = take(&mut game, Player::One, TileColor::Green, 2, 1);
			game.set_bench(Player::One, vec![t]);
		}
		{
			let t = take_rings(
				&mut game,
				Player::Two,
				&[TileColor::Orange, TileColor::LavenderPink, TileColor::Blue],
				1,
			);
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
		game.pad_above_low_value();
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
	fn small_deck_opening_stays_above_insufficient_floor() {
		let mut rules = RuleSet::default();
		rules.deck_size = 11;
		rules.insufficient_tiles_loss = InsufficientTilesLoss::Strict;
		let game = Game::new(rules, 1);
		assert_eq!(game.reachable_tiles(Player::One), 11);
		assert_eq!(game.bench(Player::One).len(), 11);
		assert_eq!(game.bag_len(Player::One), 0);
		assert_eq!(game.status(), GameStatus::InProgress);
	}

	#[test]
	fn insufficient_tiles_strict_loses_below_five_ignoring_hidden_bag() {
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
		assert_eq!(game.bag_len(Player::One), 1, "a leftover hidden bag is ignored in strict mode");
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
	fn insufficient_no_recapture_does_not_gain_from_capture() {
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
		// Proven capture geometry from `move_capture_earns_a_token_and_does_not_draw`.
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
		// Turn-boundary check without playing the capture: board+deck is 4.
		// Captures earn tokens, not tiles, so a capturing MOVE does not save them.
		game.evaluate_insufficient_tiles_for_test();
		assert_eq!(game.reachable_tiles(Player::One), 4);
		assert_eq!(game.bag_len(Player::One), 1);
		assert_eq!(
			game.status(),
			GameStatus::Won { winner: Player::Two, reason: EndReason::InsufficientTiles }
		);
	}

	#[test]
	fn insufficient_no_recapture_legacy_draw_still_saves() {
		let mut rules = RuleSet::default();
		rules.center_hold_win = false;
		rules.insufficient_tiles_loss = InsufficientTilesLoss::NoRecapture;
		rules.no_same_piece_twice = SamePieceTwice::Off;
		rules.capture_draw_on_move = true;
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
		assert!(game.has_capturing_move(Player::One));
		game.evaluate_insufficient_tiles_for_test();
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
	fn no_same_piece_twice_defaults_to_off() {
		assert_eq!(RuleSet::default().no_same_piece_twice, SamePieceTwice::Off);
		let mut game = Game::empty(RuleSet::default());
		let inner = inner_ring(&game);
		let a = inner[0];
		let center = game.board().center();
		put(&mut game, a, Player::One, TileColor::Blue, 3, 1);
		game.set_bench(Player::One, vec![]);
		{
			let t = take(&mut game, Player::Two, TileColor::Orange, 3, 1);
			game.set_bench(Player::Two, vec![t]);
		}
		game.pad_above_low_value_keeping_empty_deck(&[Player::One]);
		game.set_current(Player::One);
		game.apply(Action::Move { from: a, to: center }).unwrap();
		let p2_spawn = game.board().spawns()[0];
		game.apply(Action::Add { bench_index: 0, node: p2_spawn }).unwrap();
		assert!(game.bench(Player::One).is_empty());
		assert!(
			game.legal_destinations(center).contains(&a),
			"default Off must allow a repeat MOVE even with an empty remaining deck"
		);
	}

	#[test]
	fn no_same_piece_twice_blocks_repeat_unless_capture() {
		let mut rules = RuleSet::default();
		rules.no_same_piece_twice = SamePieceTwice::Always;
		let mut game = Game::empty(rules);
		let inner = inner_ring(&game);
		let a = inner[0];
		let center = game.board().center();
		put_rings(
			&mut game,
			a,
			Player::One,
			&[TileColor::Blue, TileColor::Teal, TileColor::Maroon],
			1,
		);
		{
			let t = take_rings(
				&mut game,
				Player::Two,
				&[TileColor::Orange, TileColor::LavenderPink, TileColor::Green],
				1,
			);
			game.set_bench(Player::Two, vec![t]);
		}
		game.pad_above_low_value();
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
		put_rings(
			&mut game,
			from,
			Player::One,
			&[TileColor::Blue, TileColor::Teal, TileColor::Maroon],
			1,
		);
		{
			let t = take(&mut game, Player::One, TileColor::Green, 3, 1);
			game.set_bench(Player::One, vec![t]);
		}
		{
			let t = take_rings(
				&mut game,
				Player::Two,
				&[TileColor::Orange, TileColor::LavenderPink, TileColor::Green],
				1,
			);
			game.set_bench(Player::Two, vec![t]);
		}
		game.pad_above_low_value();
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
	fn no_same_piece_twice_when_deck_empty_only() {
		let mut rules = RuleSet::default();
		rules.no_same_piece_twice = SamePieceTwice::WhenDeckEmpty;
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
		game.pad_above_low_value();
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
		let mut rules = hop_rules();
		rules.color_restricted_hops = true;
		rules.hop_weight_cmp = HopWeightCmp::Lt;
		let mut game = Game::empty(rules);
		let b = game.board().clone();
		let cycle = b.circle_cycle(0).to_vec();
		let (from, hop_over, land) = (cycle[0], cycle[1], cycle[2]);

		let heavy_green = |game: &mut Game, owner: Player| {
			game.alloc_rings(owner, &[TileColor::Green, TileColor::Blue, TileColor::Teal], 3, 1)
		};
		let from_tile = heavy_green(&mut game, Player::One);
		game.place_tile(from, from_tile);
		let orange = game.alloc_rings(
			Player::Two,
			&[TileColor::Orange, TileColor::LavenderPink, TileColor::Maroon],
			3,
			1,
		);
		game.place_tile(hop_over, orange);
		game.set_current(Player::One);
		assert!(
			!game.destinations(from).contains(&land),
			"other-color occupant cannot be hopped when color_restricted_hops is on"
		);

		game.clear_tile(hop_over);
		let equal = heavy_green(&mut game, Player::Two);
		game.place_tile(hop_over, equal);
		assert!(
			!game.destinations(from).contains(&land),
			"equal ring-count same color is not < under hop_weight_cmp=Lt"
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
		let le_from = le_game.alloc_rings(
			Player::One,
			&[TileColor::Green, TileColor::Blue, TileColor::Teal],
			3,
			1,
		);
		le_game.place_tile(from, le_from);
		let le_over = le_game.alloc_rings(
			Player::Two,
			&[TileColor::Green, TileColor::Blue, TileColor::Teal],
			3,
			1,
		);
		le_game.place_tile(hop_over, le_over);
		le_game.set_current(Player::One);
		assert!(
			le_game.destinations(from).contains(&land),
			"hop_weight_cmp=Le allows equal ring-count same color"
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
			for dots in 1..=3 {
				let t = Tile::mono(0, Player::One, color, 1, dots);
				assert_eq!(t.weight, 1);
				assert!(t.kind_index() < KIND_COUNT);
				assert!(seen.insert(t.kind_index()));
			}
		}
		assert_eq!(seen.len(), TileColor::ALL.len() * KIND_DOTS);
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
		assert_eq!((m, n), (3, 0), "ring count 2 + shared teal");
		assert!(!game.is_overwhelmed(spawn));

		game.clear_tile(nbs[0]);
		let c = game.alloc_rings(Player::Two, &[TileColor::Orange], 1, 1);
		game.place_tile(nbs[0], c);
		let (m, n) = game.tile_score(spawn).unwrap();
		assert_eq!((m, n), (2, 1), "no shared ring is a mismatch");
	}

	#[test]
	fn color_restricted_hops_allow_any_shared_ring() {
		let mut rules = hop_rules();
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
	fn sampled_decks_are_mirrored_and_size_11() {
		let game = Game::new(RuleSet::default(), 11);
		assert_eq!(game.rules().bag_size as usize, BAG_SIZE_DEFAULT);
		assert_eq!(game.collection(Player::One).len(), BAG_SIZE);
		assert_eq!(game.bench(Player::One).len(), BAG_SIZE_DEFAULT);
		assert_eq!(game.bag_len(Player::One), BAG_SIZE - BAG_SIZE_DEFAULT);
		let p1: Vec<_> = game
			.bench(Player::One)
			.iter()
			.map(|t| (t.color_mask(), t.n_rings, t.zag(), t.weight, t.dots))
			.collect();
		let p2: Vec<_> = game
			.bench(Player::Two)
			.iter()
			.map(|t| (t.color_mask(), t.n_rings, t.zag(), t.weight, t.dots))
			.collect();
		assert_eq!(p1, p2);
		for t in game.bench(Player::One).iter().chain(game.bag(Player::One)) {
			assert_eq!(t.weight, t.n_rings, "weight is the ring-slot count");
			assert_eq!(t.ring_strokes(), t.n_rings.max(1));
		}
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

	fn rules_with_zag_mismatch(mode: ZagMismatchCount) -> RuleSet {
		let mut rules = RuleSet::default();
		rules.zag_mismatch = mode;
		rules
	}

	#[test]
	fn zag_blue_normal_orange_zag_sums_both_hitting_zags() {
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
		assert_eq!((m, x), (3, 2), "ring count 2 + blue match; SUM counts both orange zags");
		assert!(!game.is_overwhelmed(spawn), "SUM 2 < matches 3");
		let (m2, x2) = game.tile_score(nb).unwrap();
		assert_eq!((m2, x2), (3, 2), "both tiles take the same edge count");
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
		assert_eq!(
			game.edge_score(t, u),
			(1, 2),
			"normal match (T's orange is in U) stacks with two zag hits"
		);
		let (m, x) = game.tile_score(spawn).unwrap();
		assert_eq!(m, 3, "ring count 2 + normal match");
		assert_eq!(x, 2, "two orange zags on U whose color T has");
		assert_eq!(game.edge_score(t, u), game.edge_score(u, t), "SUM is symmetric");
	}

	#[test]
	fn one_ring_zag_nets_zero_against_same_color_normal() {
		let mut game = Game::empty(RuleSet::default());
		let spawn = game.board().spawns()[0];
		let nb = game.board().neighbors(spawn)[0];
		let zag = game.alloc_ring_kinds(Player::One, &[(TileColor::Orange, true)], 1, 1);
		game.place_tile(spawn, zag);
		assert!(!game.is_overwhelmed(spawn), "isolated zag is inert");
		let normal = game.alloc_rings(Player::Two, &[TileColor::Orange], 1, 1);
		game.place_tile(nb, normal);
		assert_eq!(
			game.edge_score(zag, normal),
			(1, 1),
			"normal match −1 and zag mismatch +1 net 0"
		);
		let (m, x) = game.tile_score(spawn).unwrap();
		assert_eq!((m, x), (2, 1), "ring count 1 + normal match vs one zag hit");
		assert!(!game.is_overwhelmed(spawn), "effective 0 < ring_weight 1");
		let (om, ox) = game.tile_score(nb).unwrap();
		assert_eq!((om, ox), (2, 1));
		assert!(!game.is_overwhelmed(nb));
		assert_eq!(game.tile_effective_weight(spawn).unwrap(), 0);
	}

	#[test]
	fn one_ring_zag_dies_against_heavier_color_carrier() {
		let mut game = Game::empty(RuleSet::default());
		let spawn = game.board().spawns()[0];
		let nb = game.board().neighbors(spawn)[0];
		let zag = game.alloc_ring_kinds(Player::One, &[(TileColor::Orange, true)], 1, 1);
		let heavy = game.alloc_rings(
			Player::Two,
			&[TileColor::Orange, TileColor::Blue, TileColor::Green],
			3,
			1,
		);
		game.place_tile(spawn, zag);
		game.place_tile(nb, heavy);
		assert_eq!(heavy.weight, 3);
		assert_eq!(game.edge_score(zag, heavy), (1, 1), "same-color zag vs normal nets 0");
		assert!(!game.is_overwhelmed(spawn), "net 0 does not overwhelm the zag");
		assert!(!game.is_overwhelmed(nb), "3-ring carrier takes net 0 and lives");
	}

	#[test]
	fn zag_mismatches_zag_of_the_same_color() {
		let mut game = Game::empty(RuleSet::default());
		let spawn = game.board().spawns()[0];
		let nb = game.board().neighbors(spawn)[0];
		let a = game.alloc_ring_kinds(Player::One, &[(TileColor::Blue, true)], 1, 1);
		let b = game.alloc_ring_kinds(Player::Two, &[(TileColor::Blue, true)], 1, 1);
		game.place_tile(spawn, a);
		game.place_tile(nb, b);
		assert_eq!(game.edge_score(a, b), (0, 2), "SUM: one live zag ring each side");
		assert_eq!(game.edge_score(b, a), (0, 2));
		let (m, x) = game.tile_score(spawn).unwrap();
		assert_eq!(m, 1, "zag never matches, only own ring count");
		assert_eq!(x, 2, "both tiles' one blue zag ring hit");
		assert!(game.is_overwhelmed(spawn), "ring count 1 vs 2 zag hits is captured");
	}

	#[test]
	fn three_blue_zags_vs_three_blue_zags_is_six_under_sum() {
		let mut game = Game::empty(RuleSet::default());
		let spawn = game.board().spawns()[0];
		let nb = game.board().neighbors(spawn)[0];
		let rings = [(TileColor::Blue, true), (TileColor::Blue, true), (TileColor::Blue, true)];
		let a = game.alloc_ring_kinds(Player::One, &rings, 3, 1);
		let b = game.alloc_ring_kinds(Player::Two, &rings, 3, 1);
		game.place_tile(spawn, a);
		game.place_tile(nb, b);
		assert_eq!(game.edge_score(a, b), (0, 6), "SUM: 3+3 strokes");
		assert_eq!(game.edge_score(b, a), (0, 6));
		let (m, x) = game.tile_score(spawn).unwrap();
		assert_eq!((m, x), (3, 6), "weight 3, no color match, six zag hits");
		assert!(game.is_overwhelmed(spawn));
		assert!(game.is_overwhelmed(nb));
	}

	#[test]
	fn one_ring_zag_counts_one_zag_stroke() {
		let mut game = Game::empty(RuleSet::default());
		let spawn = game.board().spawns()[0];
		let nb = game.board().neighbors(spawn)[0];
		let a = game.alloc_ring_kinds(Player::One, &[(TileColor::Blue, true)], 3, 1);
		let b = game.alloc_ring_kinds(Player::Two, &[(TileColor::Blue, true)], 3, 1);
		assert_eq!(a.weight, 1);
		assert_eq!(a.n_rings, 1);
		assert_eq!(a.ring_strokes(), 1, "no extra concentric copies");
		game.place_tile(spawn, a);
		game.place_tile(nb, b);
		assert_eq!(game.edge_score(a, b), (0, 2), "one live zag ring on each tile");
		assert!(game.is_overwhelmed(spawn));
	}

	#[test]
	fn weight_equals_live_ring_slot_count() {
		let one = Tile::mono(0, Player::One, TileColor::Green, 3, 2);
		assert_eq!(one.n_rings, 1);
		assert_eq!(one.weight, 1);
		assert_eq!(one.ring_strokes(), 1);
		let braid = Tile::with_typed_rings(
			1,
			Player::One,
			&[(TileColor::Orange, RingType::Braid), (TileColor::Green, RingType::Braid)],
			9,
			1,
		);
		assert_eq!(braid.n_rings, 2);
		assert_eq!(braid.weight, 2, "braid slots count as rings");
		assert_eq!(braid.ring_strokes(), 2);
		let mixed = Tile::with_typed_rings(
			2,
			Player::One,
			&[
				(TileColor::Orange, RingType::Braid),
				(TileColor::Green, RingType::Braid),
				(TileColor::Blue, RingType::Normal),
			],
			1,
			1,
		);
		assert_eq!(mixed.n_rings, 3);
		assert_eq!(mixed.weight, 3);
		let triple = Tile::with_typed_rings(
			3,
			Player::One,
			&[
				(TileColor::Orange, RingType::Braid),
				(TileColor::Green, RingType::Braid),
				(TileColor::Blue, RingType::Braid),
			],
			1,
			1,
		);
		assert_eq!(triple.weight, 3);
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
		assert_eq!(game.edge_score(a, b), (0, 2), "SUM: one hitting zag on each tile");
		assert_eq!(game.edge_score(b, a), (0, 2));
		let (m, x) = game.tile_score(spawn).unwrap();
		assert_eq!((m, x), (1, 2));
		assert!(game.is_overwhelmed(spawn));
		assert!(game.is_overwhelmed(nb));
	}

	#[test]
	fn zag_color_alone_never_counts_as_a_zag_match() {
		let mut game = Game::empty(RuleSet::default());
		let a = game.alloc_ring_kinds(Player::One, &[(TileColor::Blue, true)], 1, 1);
		let b = game.alloc_rings(Player::Two, &[TileColor::Blue], 1, 1);
		assert!(!a.shares_normal(b));
		assert!(a.normal_channel_matches(b), "B's blue normal is in A's colors");
		let ch = game.edge_channels(a, b);
		assert_eq!(ch.normal_match, 1);
		assert_eq!(ch.zag_mismatch, 1);
		assert_eq!(ch.braid_match, 0);
		assert_eq!(game.edge_score(a, b), (1, 1), "zags never give a match; normal still can");
		assert_eq!(game.edge_score(b, a), (1, 1));
	}

	#[test]
	fn zag_only_vs_other_color_normal_is_a_normal_mismatch() {
		let mut game = Game::empty(RuleSet::default());
		let spawn = game.board().spawns()[0];
		let nb = game.board().neighbors(spawn)[0];
		let zag = game.alloc_ring_kinds(Player::One, &[(TileColor::Orange, true)], 1, 1);
		let blue = game.alloc_rings(Player::Two, &[TileColor::Blue], 1, 1);
		assert_eq!(zag.zag_hits_on(blue), 0, "blue does not hold orange");
		assert_eq!(blue.zag_hits_on(zag), 0);
		let ch = game.edge_channels(zag, blue);
		assert_eq!(ch.normal_mismatch, 1, "normal channel still fires");
		assert_eq!(ch.zag_mismatch, 0, "zag color is absent on the other tile");
		assert_eq!(game.edge_score(zag, blue), (0, 1));
		assert_eq!(game.edge_score(blue, zag), (0, 1));
		game.place_tile(spawn, zag);
		game.place_tile(nb, blue);
		let (m, x) = game.tile_score(spawn).unwrap();
		assert_eq!((m, x), (1, 1), "normal mismatch of 1; zag does not add");
		assert!(game.is_overwhelmed(spawn));
		assert!(game.is_overwhelmed(nb));
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
		assert_eq!((m, x), (3, 0), "ring count 2 + blue match, orange zag is empty");
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
	fn blue_vs_orange_plus_green_zag_is_a_normal_mismatch() {
		let mut game = Game::empty(RuleSet::default());
		let blue = game.alloc_rings(Player::One, &[TileColor::Blue], 1, 1);
		let orange_green_zag = game.alloc_ring_kinds(
			Player::Two,
			&[(TileColor::Orange, false), (TileColor::Green, true)],
			1,
			1,
		);
		assert_eq!(orange_green_zag.zag_hits_on(blue), 0, "blue does not hold green");
		assert_eq!(blue.zag_hits_on(orange_green_zag), 0);
		assert!(!blue.shares_normal(orange_green_zag));
		assert_eq!(
			game.edge_score(blue, orange_green_zag),
			(0, 1),
			"blue vs orange is still a normal conflict beside a non-hitting zag"
		);
		assert_eq!(game.edge_score(orange_green_zag, blue), (0, 1));
	}

	#[test]
	fn green_zag_vs_orange_plus_two_greens_is_a_zag_hit() {
		let mut game = Game::empty(RuleSet::default());
		let zag = game.alloc_ring_kinds(Player::One, &[(TileColor::Green, true)], 1, 1);
		let holder = game.alloc_rings(
			Player::Two,
			&[TileColor::Orange, TileColor::Green, TileColor::Green],
			3,
			1,
		);
		assert_eq!(zag.zag_hits_on(holder), 1);
		assert!(!zag.shares_normal(holder));
		assert!(holder.normal_color_in(zag), "holder's green normal is in the zag tile");
		assert_eq!(
			game.edge_score(zag, holder),
			(1, 1),
			"green zag +1 stacks with the holder's green normal match"
		);
		assert_eq!(game.edge_score(holder, zag), (1, 1));
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

	fn every_zag_mismatch() -> [ZagMismatchCount; 2] {
		[ZagMismatchCount::Sum, ZagMismatchCount::Max]
	}

	#[test]
	fn orange_zag_vs_orange_zag_is_sum_two_max_one() {
		let a_kinds = [(TileColor::Orange, true)];
		let b_kinds = [(TileColor::Orange, true)];
		for (mode, want) in [(ZagMismatchCount::Sum, 2), (ZagMismatchCount::Max, 1)] {
			let mut game = Game::empty(rules_with_zag_mismatch(mode));
			let a = game.alloc_ring_kinds(Player::One, &a_kinds, 1, 1);
			let b = game.alloc_ring_kinds(Player::Two, &b_kinds, 1, 1);
			assert_eq!(game.edge_score(a, b), (0, want), "{mode:?}");
			assert_eq!(game.edge_score(a, b), game.edge_score(b, a), "{mode:?} is symmetric");
			let spawn = game.board().spawns()[0];
			let nb = game.board().neighbors(spawn)[0];
			game.place_tile(spawn, a);
			game.place_tile(nb, b);
			let (m, x) = game.tile_score(spawn).unwrap();
			assert_eq!((m, x), (1, want), "{mode:?} both tiles take {want}");
			assert_eq!(game.tile_score(nb).unwrap(), (1, want));
			assert!(game.is_overwhelmed(spawn));
			assert!(game.is_overwhelmed(nb));
		}
	}

	#[test]
	fn one_sided_zag_hit_nets_zero_with_the_normal_match() {
		let zag_kinds = [(TileColor::Orange, true)];
		let holder_kinds = [(TileColor::Orange, false)];
		for mode in every_zag_mismatch() {
			let mut game = Game::empty(rules_with_zag_mismatch(mode));
			let zag = game.alloc_ring_kinds(Player::One, &zag_kinds, 1, 1);
			let holder = game.alloc_ring_kinds(Player::Two, &holder_kinds, 1, 1);
			assert_eq!(game.edge_score(zag, holder), (1, 1), "{mode:?} zag side");
			assert_eq!(game.edge_score(holder, zag), (1, 1), "{mode:?} holder side");
			let spawn = game.board().spawns()[0];
			let nb = game.board().neighbors(spawn)[0];
			game.place_tile(spawn, zag);
			game.place_tile(nb, holder);
			assert_eq!(game.tile_score(spawn).unwrap(), (2, 1));
			assert_eq!(game.tile_score(nb).unwrap(), (2, 1));
			assert!(!game.is_overwhelmed(spawn));
			assert!(!game.is_overwhelmed(nb));
		}
	}

	#[test]
	fn zag_and_normal_channels_stack_on_the_same_edge() {
		// Blue-normal + orange-zag vs green-normal + orange-normal: the orange
		// zag hits, and the other tile's orange *normal* is in the zag tile's
		// colors, so the normal channel matches rather than hitching.
		let mixed_kinds = [(TileColor::Blue, false), (TileColor::Orange, true)];
		let other_kinds = [(TileColor::Green, false), (TileColor::Orange, false)];
		for mode in every_zag_mismatch() {
			let mut game = Game::empty(rules_with_zag_mismatch(mode));
			let mixed = game.alloc_ring_kinds(Player::One, &mixed_kinds, 2, 1);
			let other = game.alloc_ring_kinds(Player::Two, &other_kinds, 2, 1);
			assert!(!mixed.shares_normal(other));
			assert!(other.normal_color_in(mixed));
			assert_eq!(mixed.zag_hits_on(other), 1);
			assert_eq!(other.zag_hits_on(mixed), 0);
			assert_eq!(game.edge_score(mixed, other), (1, 1), "{mode:?}");
			assert_eq!(game.edge_score(other, mixed), (1, 1), "{mode:?} same edge count");
			let spawn = game.board().spawns()[0];
			let nb = game.board().neighbors(spawn)[0];
			game.place_tile(spawn, mixed);
			game.place_tile(nb, other);
			assert_eq!(game.tile_score(spawn).unwrap(), (3, 1));
			assert_eq!(game.tile_score(nb).unwrap(), (3, 1));
			assert!(!game.is_overwhelmed(spawn));
		}

		// Mutual orange zags plus unmatched leftover normals: zag SUM 2 /
		// MAX 1 plus a normal mismatch of 1.
		let both_zag = [(TileColor::Blue, false), (TileColor::Orange, true)];
		let vs_zag = [(TileColor::Green, false), (TileColor::Orange, true)];
		let mut sum = Game::empty(rules_with_zag_mismatch(ZagMismatchCount::Sum));
		let a = sum.alloc_ring_kinds(Player::One, &both_zag, 2, 1);
		let b = sum.alloc_ring_kinds(Player::Two, &vs_zag, 2, 1);
		assert_eq!(sum.edge_score(a, b), (0, 3));
		assert_eq!(sum.edge_score(b, a), (0, 3));
		let mut max = Game::empty(rules_with_zag_mismatch(ZagMismatchCount::Max));
		let a = max.alloc_ring_kinds(Player::One, &both_zag, 2, 1);
		let b = max.alloc_ring_kinds(Player::Two, &vs_zag, 2, 1);
		assert_eq!(max.edge_score(a, b), (0, 2));
		assert_eq!(max.edge_score(b, a), (0, 2));
	}

	#[test]
	fn non_hitting_zag_does_not_cancel_the_normal_channel() {
		for mode in every_zag_mismatch() {
			let mut game = Game::empty(rules_with_zag_mismatch(mode));
			let zag = game.alloc_ring_kinds(Player::One, &[(TileColor::Orange, true)], 1, 1);
			let blue = game.alloc_rings(Player::Two, &[TileColor::Blue], 1, 1);
			assert_eq!(game.edge_score(zag, blue), (0, 1), "{mode:?}");
			assert_eq!(game.edge_score(blue, zag), (0, 1), "{mode:?}");
			let spawn = game.board().spawns()[0];
			let nb = game.board().neighbors(spawn)[0];
			game.place_tile(spawn, zag);
			game.place_tile(nb, blue);
			assert_eq!(game.tile_score(spawn).unwrap(), (1, 1));
			assert_eq!(game.tile_score(nb).unwrap(), (1, 1));
			assert!(game.is_overwhelmed(spawn));
			assert!(game.is_overwhelmed(nb));
		}
	}

	#[test]
	fn max_takes_per_color_max_and_sums_across_colors() {
		let mut max = Game::empty(rules_with_zag_mismatch(ZagMismatchCount::Max));
		let mut sum = Game::empty(rules_with_zag_mismatch(ZagMismatchCount::Sum));

		let three_blue =
			[(TileColor::Blue, true), (TileColor::Blue, true), (TileColor::Blue, true)];
		let a = max.alloc_ring_kinds(Player::One, &three_blue, 3, 1);
		let b = max.alloc_ring_kinds(Player::Two, &three_blue, 3, 1);
		assert_eq!(max.edge_score(a, b), (0, 3), "3 blue zags vs 3 blue zags = 3");
		assert_eq!(max.edge_score(b, a), (0, 3));
		let a = sum.alloc_ring_kinds(Player::One, &three_blue, 3, 1);
		let b = sum.alloc_ring_kinds(Player::Two, &three_blue, 3, 1);
		assert_eq!(sum.edge_score(a, b), (0, 6));

		let two_orange = [(TileColor::Orange, true), (TileColor::Orange, true)];
		let one_orange = [(TileColor::Orange, true)];
		let a = max.alloc_ring_kinds(Player::One, &two_orange, 2, 1);
		let b = max.alloc_ring_kinds(Player::Two, &one_orange, 1, 1);
		assert_eq!(max.edge_score(a, b), (0, 2), "2 orange vs 1 orange = 2");
		assert_eq!(max.edge_score(b, a), (0, 2));
		let a = sum.alloc_ring_kinds(Player::One, &two_orange, 2, 1);
		let b = sum.alloc_ring_kinds(Player::Two, &one_orange, 1, 1);
		assert_eq!(sum.edge_score(a, b), (0, 3));

		// A: 2 orange zags + green (so B's green zag hits). B: orange + green zag.
		let a_kinds =
			[(TileColor::Orange, true), (TileColor::Orange, true), (TileColor::Green, false)];
		let b_kinds = [(TileColor::Orange, false), (TileColor::Green, true)];
		let a = max.alloc_ring_kinds(Player::One, &a_kinds, 3, 1);
		let b = max.alloc_ring_kinds(Player::Two, &b_kinds, 2, 1);
		assert_eq!(
			max.edge_score(a, b),
			(1, 3),
			"normal match plus max(2,0) orange + max(0,1) green"
		);
		assert_eq!(max.edge_score(b, a), (1, 3));
		let a = sum.alloc_ring_kinds(Player::One, &a_kinds, 3, 1);
		let b = sum.alloc_ring_kinds(Player::Two, &b_kinds, 2, 1);
		assert_eq!(sum.edge_score(a, b), (1, 3), "SUM also 2 orange + 1 green, plus the match");

		// Two orange zags vs an orange holder: normal match stacks with zag MAX 2.
		let a = max.alloc_ring_kinds(Player::One, &two_orange, 2, 1);
		let holder = max.alloc_rings(Player::Two, &[TileColor::Orange], 1, 1);
		assert_eq!(max.edge_score(a, holder), (1, 2));
		assert_eq!(max.edge_score(holder, a), (1, 2));
	}

	#[test]
	fn color_restricted_hops_ignore_zag_colors() {
		let mut rules = hop_rules();
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
			.all(|t| t.zag() == [false, false, false]));
	}

	#[test]
	fn braid_off_deal_has_no_braid_rings() {
		let mut off = RuleSet::default();
		off.braid_numer = 0;
		let game = Game::new(off, 11);
		assert!(game
			.bench(Player::One)
			.iter()
			.chain(game.bag(Player::One))
			.all(|t| t.braid_ring_count() == 0));
	}

	#[test]
	fn braid_generation_is_about_one_in_seven_pieces() {
		let mut rng = StdRng::seed_from_u64(5);
		let mut next_id = 1u32;
		let n = 20_000u32;
		let mut braided = 0u32;
		let mut rings = 0u32;
		let mut braid_rings = 0u32;
		for _ in 0..n {
			let t = sample_tile(Player::One, 3, 0, BRAID_NUMER_DEFAULT, &mut next_id, &mut rng);
			let k = t.n_rings.clamp(1, 3);
			rings += u32::from(k);
			let b = t.braid_ring_count();
			braid_rings += u32::from(b);
			if b >= 2 {
				braided += 1;
				assert!(b == 2 || b == 3);
				assert!(t.n_rings == b || (b == 2 && t.n_rings == 3));
			}
		}
		let piece_rate = f64::from(braided) / f64::from(n);
		assert!(
			(0.12..0.17).contains(&piece_rate),
			"braid piece rate={piece_rate:.4} (expected ~0.1429 = 1/7)"
		);
		let ring_rate = f64::from(braid_rings) / f64::from(rings);
		assert!(ring_rate > 0.05, "braid rings should show up (rate={ring_rate:.4})");
		let mut rng = StdRng::seed_from_u64(6);
		let mut rings = [TileColor::Green; 3];
		let mut n = 2u8;
		for _ in 0..2_000 {
			let types = sample_braid(&mut rng, &mut rings, &mut n, 3, 0);
			assert_eq!(types, [RingType::Normal; 3]);
		}
	}

	#[test]
	fn braid_kind_is_distinct_from_normal_of_the_same_colors() {
		let normal = Tile::with_rings(0, Player::One, &[TileColor::Orange, TileColor::Green], 1, 1);
		let braid = Tile::with_typed_rings(
			0,
			Player::One,
			&[(TileColor::Orange, RingType::Braid), (TileColor::Green, RingType::Braid)],
			1,
			1,
		);
		assert_ne!(normal.kind_index(), braid.kind_index());
		assert!(braid.kind_index() < KIND_COUNT);
		let same_color = Tile::with_typed_rings(
			0,
			Player::One,
			&[(TileColor::Orange, RingType::Braid), (TileColor::Orange, RingType::Braid)],
			1,
			1,
		);
		assert_ne!(same_color.kind_index(), braid.kind_index());
		assert_ne!(
			same_color.kind_index(),
			Tile::mono(0, Player::One, TileColor::Orange, 1, 1).kind_index()
		);
	}

	#[test]
	fn shuffle_bags_does_not_touch_the_public_bench() {
		let mut game = Game::new(RuleSet::default(), 12);
		assert_eq!(game.bag_len(Player::One), BAG_SIZE - BAG_SIZE_DEFAULT);
		let snapshot =
			|g: &Game, p: Player| g.bench(p).iter().map(|t| t.kind_index()).collect::<Vec<_>>();
		let before = snapshot(&game, Player::One);
		let mut rng = StdRng::seed_from_u64(99);
		game.shuffle_bags(&mut rng);
		assert_eq!(before, snapshot(&game, Player::One));
		assert_eq!(game.bag_len(Player::One), BAG_SIZE - BAG_SIZE_DEFAULT);
	}

	#[test]
	fn shuffle_bags_determinizes_a_legacy_hidden_remainder() {
		let mut game = Game::empty(RuleSet::default());
		let leftovers: Vec<_> =
			(0..4).map(|_| game.alloc_tile(Player::One, TileColor::Orange, 1, 1)).collect();
		game.set_bag(Player::One, leftovers);
		let snapshot = |g: &Game, p: Player| {
			let mut v: Vec<_> = g.bag(p).iter().map(|t| t.kind_index()).collect();
			v.sort_unstable();
			v
		};
		let before = snapshot(&game, Player::One);
		let mut rng = StdRng::seed_from_u64(99);
		game.shuffle_bags(&mut rng);
		assert_eq!(before, snapshot(&game, Player::One));
		assert_eq!(game.bag_len(Player::One), 4);
	}

	fn zero_budget(game: &mut Game) {
		game.set_budget_tokens(Player::One, Vec::new());
		game.set_budget_tokens(Player::Two, Vec::new());
	}

	fn low_value_rules() -> RuleSet {
		let mut rules = RuleSet::default();
		rules.control_win = false;
		rules.center_hold_win = false;
		rules.petal_majority_win = false;
		rules
	}

	#[test]
	fn capture_budget_starts_at_seven_and_add_spends_one() {
		let mut game = Game::new(RuleSet::default(), 3);
		assert_eq!(game.budget(Player::One), 7);
		assert_eq!(game.budget_tokens(Player::One).len(), 7);
		assert!(game.budget_tokens(Player::One).iter().all(|t| matches!(t, BudgetToken::Blank)));
		let action = game.legal_adds().into_iter().next().expect("opening ADD");
		game.apply(action).unwrap();
		assert_eq!(game.budget(Player::One), 6);
		assert_eq!(game.bench(Player::One).len(), BAG_SIZE_DEFAULT - 1);
	}

	#[test]
	fn add_is_legal_from_any_bench_index() {
		let mut game = Game::empty(RuleSet::default());
		stock_bench(&mut game, Player::One, 3);
		game.set_current(Player::One);
		let adds = game.legal_adds();
		let mut indices: Vec<usize> = adds
			.iter()
			.filter_map(|a| match *a {
				Action::Add { bench_index, .. } => Some(bench_index),
				_ => None,
			})
			.collect();
		indices.sort_unstable();
		indices.dedup();
		assert_eq!(indices, vec![0, 1, 2], "every bench tile is selectable");
		assert_eq!(adds.len(), 3 * game.board().spawns().len());
	}

	#[test]
	fn zero_budget_blocks_add_even_with_bench_tiles() {
		let mut game = Game::empty(RuleSet::default());
		stock_bench(&mut game, Player::One, 3);
		game.set_budget_tokens(Player::One, Vec::new());
		game.set_current(Player::One);
		assert!(game.legal_adds().is_empty());
		let spawn = game.board().spawns()[0];
		assert!(game.apply(Action::Add { bench_index: 0, node: spawn }).is_err());
	}

	#[test]
	fn opponent_capture_earns_budget() {
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
		game.set_current(Player::One);
		assert_eq!(game.budget_token_count(Player::One), 7);
		assert_eq!(game.budget(Player::One), 0, "empty bench makes leftover tokens unusable");
		let outcome = game.apply(Action::Move { from, to: land }).unwrap();
		assert!(
			outcome.removals.iter().any(|r| r.tile.owner == Player::Two),
			"opponent tile should be removed: {:?}",
			outcome.removals
		);
		assert_eq!(game.budget_token_count(Player::One), 7 + outcome.removals.len());
		assert_eq!(game.budget(Player::One), 0);
		assert!(matches!(game.budget_tokens(Player::One).last(), Some(BudgetToken::Captured(_))));
	}

	#[test]
	fn self_capture_on_own_turn_earns_budget() {
		// Overwhelm already removes own tiles. A MOVE that leaves an existing
		// own tile overwhelmed is a self-capture; it must earn +1 without a
		// new capture rule.
		let mut game = Game::empty(RuleSet::default());
		let spawn = game.board().spawns()[0];
		let nbs = game.board().neighbors(spawn).to_vec();
		put(&mut game, spawn, Player::One, TileColor::Green, 1, 1);
		put_rings(
			&mut game,
			nbs[0],
			Player::Two,
			&[TileColor::Orange, TileColor::Teal, TileColor::Maroon],
			1,
		);
		put_rings(
			&mut game,
			nbs[1],
			Player::Two,
			&[TileColor::Orange, TileColor::Teal, TileColor::Maroon],
			1,
		);
		assert!(game.is_overwhelmed(spawn), "own tile is already overwhelmed");
		let from = game.board().spawns()[4];
		put_rings(
			&mut game,
			from,
			Player::One,
			&[TileColor::Blue, TileColor::LavenderPink, TileColor::Green],
			1,
		);
		game.set_current(Player::One);
		let to = game.destinations(from).into_iter().next().expect("blue d=1 can step");
		assert_eq!(game.budget_token_count(Player::One), 7);
		assert_eq!(game.budget(Player::One), 0);
		let outcome = game.apply(Action::Move { from, to }).unwrap();
		assert!(
			outcome.removals.iter().any(|r| r.node == spawn && r.tile.owner == Player::One),
			"own overwhelmed tile must be removed: {:?}",
			outcome.removals
		);
		assert_eq!(outcome.removals.len(), 1);
		let own = outcome.removals.iter().filter(|r| r.tile.owner == Player::One).count();
		assert_eq!(own, 1);
		assert_eq!(game.budget_token_count(Player::One), 8);
		assert_eq!(game.budget(Player::One), 0);
		assert!(game
			.budget_tokens(Player::One)
			.iter()
			.any(|t| matches!(t, BudgetToken::Captured(_))));
	}

	#[test]
	fn suicidal_add_is_self_capture_and_nets_zero_budget() {
		let mut game = Game::empty(RuleSet::default());
		let spawn = game.board().spawns()[0];
		let nbs = game.board().neighbors(spawn).to_vec();
		put_rings(
			&mut game,
			nbs[0],
			Player::Two,
			&[TileColor::Orange, TileColor::Teal, TileColor::Maroon],
			1,
		);
		put_rings(
			&mut game,
			nbs[1],
			Player::Two,
			&[TileColor::Orange, TileColor::Teal, TileColor::Maroon],
			1,
		);
		let tile = green(&mut game, Player::One, 1, 1);
		game.set_bench(Player::One, vec![tile]);
		game.set_current(Player::One);
		assert_eq!(game.budget_token_count(Player::One), 7);
		assert_eq!(game.budget(Player::One), 1);
		let outcome = game.apply(Action::Add { bench_index: 0, node: spawn }).unwrap();
		assert_eq!(outcome.removals.len(), 1);
		assert!(outcome.removals.iter().any(|r| r.node == spawn && r.tile.owner == Player::One));
		// Spent 1 to ADD, earned 1 for the self-capture. Bench is empty, so
		// the remaining tokens are surplus.
		assert_eq!(game.budget_token_count(Player::One), 7);
		assert_eq!(game.budget(Player::One), 0);
	}

	fn self_capture_rules(refund: bool) -> RuleSet {
		let mut rules = RuleSet::default();
		rules.self_capture_budget = refund;
		rules
	}

	#[test]
	fn self_capture_on_own_turn_does_not_earn_budget_when_toggled_off() {
		let mut game = Game::empty(self_capture_rules(false));
		let spawn = game.board().spawns()[0];
		let nbs = game.board().neighbors(spawn).to_vec();
		put(&mut game, spawn, Player::One, TileColor::Green, 1, 1);
		put_rings(
			&mut game,
			nbs[0],
			Player::Two,
			&[TileColor::Orange, TileColor::Teal, TileColor::Maroon],
			1,
		);
		put_rings(
			&mut game,
			nbs[1],
			Player::Two,
			&[TileColor::Orange, TileColor::Teal, TileColor::Maroon],
			1,
		);
		assert!(game.is_overwhelmed(spawn), "own tile is already overwhelmed");
		let from = game.board().spawns()[4];
		put_rings(
			&mut game,
			from,
			Player::One,
			&[TileColor::Blue, TileColor::LavenderPink, TileColor::Green],
			1,
		);
		game.set_current(Player::One);
		let to = game.destinations(from).into_iter().next().expect("blue d=1 can step");
		assert_eq!(game.budget_token_count(Player::One), 7);
		let outcome = game.apply(Action::Move { from, to }).unwrap();
		assert!(
			outcome.removals.iter().any(|r| r.node == spawn && r.tile.owner == Player::One),
			"own overwhelmed tile must be removed: {:?}",
			outcome.removals
		);
		assert_eq!(outcome.removals.len(), 1);
		assert_eq!(
			game.budget_token_count(Player::One),
			7,
			"self-capture must not refund when toggled off"
		);
		assert_eq!(game.budget(Player::One), 0);
		assert!(game.budget_tokens(Player::One).iter().all(|t| matches!(t, BudgetToken::Blank)));
	}

	#[test]
	fn suicidal_add_spends_without_refund_when_self_capture_budget_off() {
		let mut game = Game::empty(self_capture_rules(false));
		let spawn = game.board().spawns()[0];
		let nbs = game.board().neighbors(spawn).to_vec();
		put_rings(
			&mut game,
			nbs[0],
			Player::Two,
			&[TileColor::Orange, TileColor::Teal, TileColor::Maroon],
			1,
		);
		put_rings(
			&mut game,
			nbs[1],
			Player::Two,
			&[TileColor::Orange, TileColor::Teal, TileColor::Maroon],
			1,
		);
		let tile = green(&mut game, Player::One, 1, 1);
		game.set_bench(Player::One, vec![tile]);
		game.set_current(Player::One);
		assert_eq!(game.budget_token_count(Player::One), 7);
		assert_eq!(game.budget(Player::One), 1);
		let outcome = game.apply(Action::Add { bench_index: 0, node: spawn }).unwrap();
		assert_eq!(outcome.removals.len(), 1);
		assert!(outcome.removals.iter().any(|r| r.node == spawn && r.tile.owner == Player::One));
		assert_eq!(game.budget_token_count(Player::One), 6);
		assert_eq!(game.budget(Player::One), 0);
	}

	#[test]
	fn opponent_capture_still_earns_when_self_capture_budget_off() {
		let mut game = Game::empty(self_capture_rules(false));
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
		game.set_current(Player::One);
		assert_eq!(game.budget_token_count(Player::One), 7);
		let outcome = game.apply(Action::Move { from, to: land }).unwrap();
		assert!(
			outcome.removals.iter().any(|r| r.tile.owner == Player::Two),
			"opponent tile should be removed: {:?}",
			outcome.removals
		);
		let earned = outcome.removals.iter().filter(|r| r.tile.owner != Player::One).count();
		assert!(earned > 0);
		assert_eq!(game.budget_token_count(Player::One), 7 + earned);
		assert_eq!(game.budget(Player::One), 0);
	}

	#[test]
	fn empty_bag_skips_legacy_capture_draw_on_move() {
		let mut rules = RuleSet::default();
		rules.capture_draw_on_move = true;
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
			.unwrap_or(&nbs[2]);
		put(&mut game, spawn, Player::Two, TileColor::Green, 2, 1);
		put(&mut game, nbs[1], Player::One, TileColor::Orange, 1, 1);
		put(&mut game, from, Player::One, TileColor::Blue, 3, 1);
		game.set_bag(Player::One, Vec::new());
		game.set_current(Player::One);
		let bench_before = game.bench(Player::One).len();
		let outcome = game.apply(Action::Move { from, to: land }).unwrap();
		assert!(outcome.removals.iter().any(|r| r.tile.owner == Player::Two));
		assert_eq!(outcome.draws, 0);
		assert_eq!(game.bag_len(Player::One), 0);
		assert_eq!(game.bench(Player::One).len(), bench_before);
	}

	#[test]
	fn piece_value_weights_are_dots_plus_rings() {
		let mut game = Game::empty(RuleSet::default());
		let normal = game.alloc_rings(Player::One, &[TileColor::Green], 1, 1);
		assert_eq!(normal.piece_value(PieceValueWeights::PRIMARY), 2);
		assert_eq!(normal.piece_value(PieceValueWeights::TIEBREAK), 2);
		let zag = game.alloc_ring_kinds(Player::One, &[(TileColor::Green, true)], 1, 1);
		assert_eq!(zag.piece_value(PieceValueWeights::PRIMARY), 2);
		assert_eq!(zag.piece_value(PieceValueWeights::TIEBREAK), 3);
		let mixed = game.alloc_ring_kinds(
			Player::One,
			&[(TileColor::Blue, false), (TileColor::Orange, true)],
			2,
			2,
		);
		assert_eq!(mixed.piece_value(PieceValueWeights::PRIMARY), 4);
		assert_eq!(mixed.piece_value(PieceValueWeights::TIEBREAK), 5);
		let braid = game.alloc_typed_rings(
			Player::One,
			&[(TileColor::Orange, RingType::Braid), (TileColor::Orange, RingType::Braid)],
			1,
			1,
		);
		assert_eq!(braid.piece_value(PieceValueWeights::PRIMARY), 3, "dots 1 + two braid rings");
		assert_eq!(
			braid.piece_value(PieceValueWeights::TIEBREAK),
			5,
			"braid rings are worth 2 in the zag-weighted pass"
		);
		let braid_plus_zag = game.alloc_typed_rings(
			Player::One,
			&[
				(TileColor::Orange, RingType::Braid),
				(TileColor::Green, RingType::Braid),
				(TileColor::Blue, RingType::Zag),
			],
			1,
			2,
		);
		assert_eq!(braid_plus_zag.piece_value(PieceValueWeights::PRIMARY), 5);
		assert_eq!(braid_plus_zag.piece_value(PieceValueWeights::TIEBREAK), 8);
	}

	fn orange_green_braid(game: &mut Game, owner: Player) -> Tile {
		game.alloc_typed_rings(
			owner,
			&[(TileColor::Orange, RingType::Braid), (TileColor::Green, RingType::Braid)],
			1,
			1,
		)
	}

	fn blue_teal_braid(game: &mut Game, owner: Player) -> Tile {
		game.alloc_typed_rings(
			owner,
			&[(TileColor::Blue, RingType::Braid), (TileColor::Teal, RingType::Braid)],
			1,
			1,
		)
	}

	fn two_inner_neighbors(game: &Game) -> (NodeId, NodeId) {
		let inner = game.board().inner_ring();
		assert!(inner.len() >= 2, "center has an 8-node inner ring");
		(inner[0], inner[1])
	}

	#[test]
	fn braid_satisfied_when_neighbor_covers_color_counts() {
		let mut game = Game::empty(RuleSet::default());
		let a = orange_green_braid(&mut game, Player::One);
		let b = game.alloc_rings(Player::Two, &[TileColor::Orange, TileColor::Green], 1, 1);
		assert_eq!(
			game.edge_score(a, b),
			(3, 0),
			"braid match 2 stacks with the neighbor's normal match"
		);
		assert_eq!(game.edge_score(b, a), (3, 0), "undirected: both tiles take the same channels");
	}

	#[test]
	fn braid_unsatisfied_is_a_braid_mismatch() {
		let mut game = Game::empty(RuleSet::default());
		let a = orange_green_braid(&mut game, Player::One);
		let b = game.alloc_rings(Player::Two, &[TileColor::Blue], 1, 1);
		let ch = game.edge_channels(a, b);
		assert_eq!(ch.braid_mismatch, 1);
		assert_eq!(ch.normal_mismatch, 1, "blue is not among the braid colors");
		assert_eq!(game.edge_score(a, b), (0, 2));
		assert_eq!(game.edge_score(b, a), (0, 2));
	}

	#[test]
	fn single_color_braid_needs_two_of_that_color() {
		let mut game = Game::empty(RuleSet::default());
		let braid = game.alloc_typed_rings(
			Player::One,
			&[(TileColor::Orange, RingType::Braid), (TileColor::Orange, RingType::Braid)],
			1,
			1,
		);
		let one_orange = game.alloc_rings(Player::Two, &[TileColor::Orange], 1, 1);
		assert_eq!(
			game.edge_score(braid, one_orange),
			(1, 1),
			"unfulfilled orange+orange braid +1; the single orange normal still matches"
		);
		let two_normal = game.alloc_typed_rings(
			Player::Two,
			&[(TileColor::Orange, RingType::Normal), (TileColor::Orange, RingType::Normal)],
			1,
			1,
		);
		assert_eq!(
			game.edge_score(braid, two_normal),
			(3, 0),
			"braid match 2 stacks with the orange normal match"
		);
		let normal_and_zag = game.alloc_typed_rings(
			Player::Two,
			&[(TileColor::Orange, RingType::Normal), (TileColor::Orange, RingType::Zag)],
			1,
			1,
		);
		assert_eq!(
			game.edge_score(braid, normal_and_zag),
			(3, 1),
			"zag orange still counts toward the braid; the jag also mismatches"
		);
		let one_orange_heavy_arg = game.alloc_rings(Player::Two, &[TileColor::Orange], 3, 1);
		assert_eq!(one_orange_heavy_arg.n_rings, 1);
		assert_eq!(one_orange_heavy_arg.weight, 1, "constructor weight is ignored");
		assert_eq!(one_orange_heavy_arg.color_count(TileColor::Orange), 1);
		assert_eq!(one_orange_heavy_arg.ring_strokes(), 1, "no extra concentric copies");
		assert_eq!(
			game.edge_score(braid, one_orange_heavy_arg),
			(1, 1),
			"a single orange ring cannot cover orange+orange; two orange rings are required"
		);
	}

	#[test]
	fn failed_braid_still_allows_a_leftover_normal_match() {
		let mut game = Game::empty(RuleSet::default());
		let mixed = game.alloc_typed_rings(
			Player::One,
			&[
				(TileColor::Orange, RingType::Braid),
				(TileColor::Green, RingType::Braid),
				(TileColor::Orange, RingType::Normal),
			],
			1,
			1,
		);
		let orange_only = game.alloc_rings(Player::Two, &[TileColor::Orange], 1, 1);
		assert_eq!(
			game.edge_score(mixed, orange_only),
			(1, 1),
			"braid fails (+1) and leftover orange normal still matches (−1)"
		);
		let orange_green =
			game.alloc_rings(Player::Two, &[TileColor::Orange, TileColor::Green], 1, 1);
		assert_eq!(
			game.edge_score(mixed, orange_green),
			(3, 0),
			"satisfied braid stacks with the leftover orange normal match"
		);
	}

	#[test]
	fn largest_fulfilled_braid_wins_the_edge() {
		let mut game = Game::empty(RuleSet::default());
		let size3 = game.alloc_typed_rings(
			Player::One,
			&[
				(TileColor::Orange, RingType::Braid),
				(TileColor::Green, RingType::Braid),
				(TileColor::Blue, RingType::Braid),
			],
			1,
			1,
		);
		let size2 = game.alloc_typed_rings(
			Player::Two,
			&[
				(TileColor::Orange, RingType::Braid),
				(TileColor::Green, RingType::Braid),
				(TileColor::Blue, RingType::Normal),
			],
			1,
			1,
		);
		assert_eq!(
			game.edge_score(size3, size2),
			(4, 0),
			"largest fulfilled braid is 3; leftover blue normal still matches"
		);
		let a = orange_green_braid(&mut game, Player::One);
		let b = game.alloc_typed_rings(
			Player::Two,
			&[(TileColor::Orange, RingType::Braid), (TileColor::Green, RingType::Braid)],
			1,
			1,
		);
		assert_eq!(game.edge_score(a, b), (2, 0), "equal braids pick one (weight 2, not 4)");
		let picked = selected_braid(a, b).expect("both size-2 braids qualify");
		assert_eq!(picked.1, 2);
		assert_eq!(picked.0.id, a.id.min(b.id), "equal size: lower tile id");
	}

	#[test]
	fn braid_and_jag_scores_sum() {
		let mut game = Game::empty(RuleSet::default());
		let braid_plus_zag = game.alloc_typed_rings(
			Player::One,
			&[
				(TileColor::Orange, RingType::Braid),
				(TileColor::Green, RingType::Braid),
				(TileColor::Blue, RingType::Zag),
			],
			1,
			1,
		);
		let neighbor = game.alloc_rings(
			Player::Two,
			&[TileColor::Orange, TileColor::Green, TileColor::Blue],
			1,
			1,
		);
		assert_eq!(
			game.edge_score(braid_plus_zag, neighbor),
			(3, 1),
			"braid match 2 stacks with the neighbor's normal match; blue jag +1"
		);
	}

	#[test]
	fn independent_channel_table_rows() {
		// RFC table: (name, A parts, B parts, match, mismatch, notes).
		struct Row {
			name: &'static str,
			a: &'static [(TileColor, RingType)],
			b: &'static [(TileColor, RingType)],
			want_match: i32,
			want_mismatch: i32,
			normal_match: i32,
			normal_mismatch: i32,
			zag: i32,
			braid_match: i32,
			braid_mismatch: i32,
		}
		let rows = [
			Row {
				name: "zag vs normal same color",
				a: &[(TileColor::Orange, RingType::Zag)],
				b: &[(TileColor::Orange, RingType::Normal)],
				want_match: 1,
				want_mismatch: 1,
				normal_match: 1,
				normal_mismatch: 0,
				zag: 1,
				braid_match: 0,
				braid_mismatch: 0,
			},
			Row {
				name: "braid plus normal stacking",
				a: &[
					(TileColor::Orange, RingType::Braid),
					(TileColor::Green, RingType::Braid),
					(TileColor::Orange, RingType::Normal),
				],
				b: &[(TileColor::Orange, RingType::Normal), (TileColor::Green, RingType::Normal)],
				want_match: 3,
				want_mismatch: 0,
				normal_match: 1,
				normal_mismatch: 0,
				zag: 0,
				braid_match: 2,
				braid_mismatch: 0,
			},
			Row {
				name: "both sides with braids where one fulfills",
				a: &[
					(TileColor::Orange, RingType::Braid),
					(TileColor::Green, RingType::Braid),
					(TileColor::Blue, RingType::Braid),
				],
				b: &[
					(TileColor::Orange, RingType::Braid),
					(TileColor::Green, RingType::Braid),
					(TileColor::Blue, RingType::Normal),
				],
				want_match: 4,
				want_mismatch: 0,
				normal_match: 1,
				normal_mismatch: 0,
				zag: 0,
				braid_match: 3,
				braid_mismatch: 0,
			},
			Row {
				name: "neither braid fulfilled",
				a: &[(TileColor::Orange, RingType::Braid), (TileColor::Green, RingType::Braid)],
				b: &[(TileColor::Blue, RingType::Braid), (TileColor::Teal, RingType::Braid)],
				want_match: 0,
				want_mismatch: 1,
				normal_match: 0,
				normal_mismatch: 0,
				zag: 0,
				braid_match: 0,
				braid_mismatch: 1,
			},
			Row {
				name: "zag-only tile next to a normal-only tile",
				a: &[(TileColor::Orange, RingType::Zag)],
				b: &[(TileColor::Blue, RingType::Normal)],
				want_match: 0,
				want_mismatch: 1,
				normal_match: 0,
				normal_mismatch: 1,
				zag: 0,
				braid_match: 0,
				braid_mismatch: 0,
			},
			Row {
				name: "same-color braid needing two rings",
				a: &[(TileColor::Orange, RingType::Braid), (TileColor::Orange, RingType::Braid)],
				b: &[(TileColor::Orange, RingType::Normal)],
				want_match: 1,
				want_mismatch: 1,
				normal_match: 1,
				normal_mismatch: 0,
				zag: 0,
				braid_match: 0,
				braid_mismatch: 1,
			},
		];
		for row in rows {
			let mut game = Game::empty(RuleSet::default());
			let a = game.alloc_typed_rings(Player::One, row.a, 1, 1);
			let b = game.alloc_typed_rings(Player::Two, row.b, 1, 1);
			let ch = game.edge_channels(a, b);
			assert_eq!(ch.normal_match, row.normal_match, "{} normal_match", row.name);
			assert_eq!(ch.normal_mismatch, row.normal_mismatch, "{} normal_mismatch", row.name);
			assert_eq!(ch.zag_mismatch, row.zag, "{} zag", row.name);
			assert_eq!(ch.braid_match, row.braid_match, "{} braid_match", row.name);
			assert_eq!(ch.braid_mismatch, row.braid_mismatch, "{} braid_mismatch", row.name);
			assert_eq!(game.edge_score(a, b), (row.want_match, row.want_mismatch), "{}", row.name);
			assert_eq!(
				game.edge_score(b, a),
				(row.want_match, row.want_mismatch),
				"{} sym",
				row.name
			);
			let spawn = game.board().spawns()[0];
			let nb = game.board().neighbors(spawn)[0];
			game.place_tile(spawn, a);
			game.place_tile(nb, b);
			let eff = game.tile_effective_weight(spawn).unwrap();
			assert_eq!(eff, row.want_mismatch - row.want_match, "{} effective", row.name);
			assert_eq!(
				game.is_overwhelmed(spawn),
				eff >= i32::from(a.weight),
				"{} overwhelm vs ring_weight {}",
				row.name,
				a.weight
			);
			let (m, x) = game.tile_score(spawn).unwrap();
			assert_eq!(m, i32::from(a.weight) + row.want_match, "{} tile matches", row.name);
			assert_eq!(x, row.want_mismatch, "{} tile mismatches", row.name);
			assert_eq!(x >= m, game.is_overwhelmed(spawn), "{} tile_score vs overwhelm", row.name);
		}
	}

	#[test]
	fn center_inner_failed_braid_scores_plus_two_on_each_side() {
		let mut game = Game::empty(RuleSet::default());
		let board = game.board();
		let center = board.center();
		assert_eq!(board.neighbors(center).len(), 8);
		assert_eq!(board.incident_arc_count(center), 16);
		let inner = board.inner_ring()[0];
		assert_eq!(board.connecting_arc_indices(center, inner).count(), 2);
		assert_eq!(board.incident_others(inner).filter(|&n| n == center).count(), 2);

		let a = orange_green_braid(&mut game, Player::One);
		let b = blue_teal_braid(&mut game, Player::Two);
		assert_eq!(a.weight, 2);
		assert_eq!(b.weight, 2);
		assert_eq!(
			game.edge_score(a, b),
			(0, 1),
			"each petal is still +1, not per side of one petal"
		);
		assert_eq!(game.edge_score(b, a), (0, 1));

		game.place_tile(center, a);
		game.place_tile(inner, b);
		assert_eq!(
			game.tile_effective_weight(center),
			Some(2),
			"two petals: +1 + +1 = +2 on the center"
		);
		assert_eq!(
			game.tile_effective_weight(inner),
			Some(2),
			"the same two petals: +1 + +1 = +2 on the petal"
		);
		assert_eq!(game.tile_score(center), Some((2, 2)));
		assert_eq!(game.tile_score(inner), Some((2, 2)));
		assert!(game.is_overwhelmed(center), "2 >= ring_weight 2");
		assert!(game.is_overwhelmed(inner), "2-ring petal is also overwhelmed");
		let mut nodes: Vec<NodeId> = game.collect_removals().iter().map(|r| r.node).collect();
		nodes.sort();
		let mut want = vec![center, inner];
		want.sort();
		assert_eq!(nodes, want);
	}

	#[test]
	fn center_inner_normal_match_scores_minus_two_on_each_side() {
		let mut game = Game::empty(RuleSet::default());
		let center = game.board().center();
		let inner = game.board().inner_ring()[0];
		let a = game.alloc_rings(Player::One, &[TileColor::Orange], 1, 1);
		let b = game.alloc_rings(Player::Two, &[TileColor::Orange], 1, 1);
		assert_eq!(a.weight, 1);
		assert_eq!(game.edge_score(a, b), (1, 0));
		game.place_tile(center, a);
		game.place_tile(inner, b);
		assert_eq!(game.tile_effective_weight(center), Some(-2), "match −1 on each of two petals");
		assert_eq!(game.tile_effective_weight(inner), Some(-2));
		assert_eq!(game.tile_score(center), Some((1 + 2, 0)));
		assert_eq!(game.tile_score(inner), Some((1 + 2, 0)));
		assert!(!game.is_overwhelmed(center));
		assert!(!game.is_overwhelmed(inner));
	}

	#[test]
	fn failed_double_braid_at_center_pops_center_and_both_petals() {
		// Two mismatching inner 2-braids vs a 2-braid at the origin. Each
		// center↔petal pair is two edges at +1, so the petal is +2 and the
		// center is +4. Capture is simultaneous on the pre-removal board.
		let mut rules = RuleSet::default();
		rules.add_only_on_spawns = false;
		let mut game = Game::empty(rules);
		let center = game.board().center();
		let (n0, n1) = two_inner_neighbors(&game);
		assert_ne!(n0, n1);
		assert!(!game.board().neighbors(n0).contains(&n1));

		let placed = orange_green_braid(&mut game, Player::One);
		let nb0 = blue_teal_braid(&mut game, Player::Two);
		let nb1 = blue_teal_braid(&mut game, Player::Two);
		assert_eq!(game.edge_channels(placed, nb0).braid_mismatch, 1);

		game.place_tile(n0, nb0);
		game.place_tile(n1, nb1);
		game.set_bench(Player::One, vec![placed]);

		assert!(game.would_overwhelm_add(placed, center));
		let outcome = game
			.apply(Action::Add { bench_index: 0, node: center })
			.expect("ADD to the empty center");

		let mut nodes: Vec<NodeId> = outcome.removals.iter().map(|r| r.node).collect();
		nodes.sort();
		let mut want = vec![center, n0, n1];
		want.sort();
		assert_eq!(nodes, want, "placed tile and both petals pop");
		let center_rem = outcome.removals.iter().find(|r| r.node == center).unwrap();
		assert_eq!(center_rem.matches, 2, "ring_weight 2 + match 0");
		assert_eq!(center_rem.mismatches, 4, "+1 × 2 petals × 2 neighbors");
		let petal_rem = outcome.removals.iter().find(|r| r.node == n0).unwrap();
		assert_eq!(petal_rem.matches, 2);
		assert_eq!(petal_rem.mismatches, 2);
		assert!(game.tile_at(center).is_none());
		assert!(game.tile_at(n0).is_none());
		assert!(game.tile_at(n1).is_none());
	}

	#[test]
	fn failed_double_braid_weight_variant_still_pops_center_and_both_petals() {
		let mut rules = RuleSet::default();
		rules.add_only_on_spawns = false;
		rules.braid_fail_mismatch = BraidFailMismatch::Weight;
		let mut game = Game::empty(rules);
		let center = game.board().center();
		let (n0, n1) = two_inner_neighbors(&game);
		let placed = orange_green_braid(&mut game, Player::One);
		let nb0 = blue_teal_braid(&mut game, Player::Two);
		let nb1 = blue_teal_braid(&mut game, Player::Two);
		assert_eq!(game.edge_channels(placed, nb0).braid_mismatch, 2);

		game.place_tile(n0, nb0);
		game.place_tile(n1, nb1);
		game.set_bench(Player::One, vec![placed]);
		let outcome = game
			.apply(Action::Add { bench_index: 0, node: center })
			.expect("ADD to the empty center");

		let mut nodes: Vec<NodeId> = outcome.removals.iter().map(|r| r.node).collect();
		nodes.sort();
		let mut want = vec![center, n0, n1];
		want.sort();
		assert_eq!(nodes, want);
		let center_rem = outcome.removals.iter().find(|r| r.node == center).unwrap();
		assert_eq!(center_rem.mismatches, 8, "weight 2 × 2 petals × 2 neighbors");
	}

	#[test]
	fn spawn_inner_is_still_one_scoring_edge() {
		let mut game = Game::empty(RuleSet::default());
		let spawn = game.board().spawns()[0];
		let inner = *game
			.board()
			.neighbors(spawn)
			.iter()
			.find(|&&n| game.board().inner_ring().contains(&n))
			.expect("spawn attaches to an inner node");
		assert_eq!(game.board().connecting_arc_indices(spawn, inner).count(), 1);
		let a = orange_green_braid(&mut game, Player::One);
		let b = blue_teal_braid(&mut game, Player::Two);
		game.place_tile(spawn, a);
		game.place_tile(inner, b);
		assert_eq!(game.tile_effective_weight(spawn), Some(1));
		assert_eq!(game.tile_effective_weight(inner), Some(1));
		assert!(!game.is_overwhelmed(spawn));
		assert!(!game.is_overwhelmed(inner));
	}

	#[test]
	fn capture_scores_neighbors_simultaneously_with_the_placed_tile() {
		// 1-ring non-hitting zags next to a failed 2-braid: each edge is +1.
		// Center 2 >= 2 pops; each 1-ring neighbor 1 >= 1 pops. If capture
		// popped the center first and re-scored, the neighbors would survive
		// (pressure gone). We evaluate everyone on the pre-removal board.
		let mut rules = RuleSet::default();
		rules.add_only_on_spawns = false;
		let mut game = Game::empty(rules);
		let center = game.board().center();
		let (n0, n1) = two_inner_neighbors(&game);
		let placed = orange_green_braid(&mut game, Player::One);
		let z0 = game.alloc_typed_rings(Player::Two, &[(TileColor::Blue, RingType::Zag)], 1, 1);
		let z1 = game.alloc_typed_rings(Player::Two, &[(TileColor::Blue, RingType::Zag)], 1, 1);
		assert_eq!(z0.weight, 1);
		assert_eq!(game.edge_score(placed, z0), (0, 1));

		game.place_tile(n0, z0);
		game.place_tile(n1, z1);
		game.place_tile(center, placed);
		let before = game.collect_removals();
		let mut before_nodes: Vec<NodeId> = before.iter().map(|r| r.node).collect();
		before_nodes.sort();
		let mut want = vec![center, n0, n1];
		want.sort();
		assert_eq!(before_nodes, want, "neighbors are in the simultaneous removal set");
		assert!(game.is_overwhelmed(center));
		assert!(game.is_overwhelmed(n0));
		assert!(game.is_overwhelmed(n1));

		game.clear_tile(center);
		assert!(
			game.collect_removals().is_empty(),
			"popping the placed tile first would have saved the neighbors; we do not do that"
		);
		game.set_bench(Player::One, vec![placed]);
		let outcome = game
			.apply(Action::Add { bench_index: 0, node: center })
			.expect("ADD to the empty center");
		let mut nodes: Vec<NodeId> = outcome.removals.iter().map(|r| r.node).collect();
		nodes.sort();
		assert_eq!(nodes, want);
		assert!(game.tile_at(center).is_none());
		assert!(game.tile_at(n0).is_none());
		assert!(game.tile_at(n1).is_none());
	}

	#[test]
	fn failed_braid_weight_variant_uses_the_braid_slot_count() {
		let mut rules = RuleSet::default();
		rules.braid_fail_mismatch = BraidFailMismatch::Weight;
		let mut game = Game::empty(rules);
		let a = orange_green_braid(&mut game, Player::One);
		let b = game.alloc_rings(Player::Two, &[TileColor::Blue], 1, 1);
		let ch = game.edge_channels(a, b);
		assert_eq!(ch.braid_mismatch, 2, "failed 2-braid costs its weight");
		assert_eq!(ch.normal_mismatch, 1);
		assert_eq!(game.edge_score(a, b), (0, 3));
		let size3 = game.alloc_typed_rings(
			Player::One,
			&[
				(TileColor::Orange, RingType::Braid),
				(TileColor::Green, RingType::Braid),
				(TileColor::Blue, RingType::Braid),
			],
			1,
			1,
		);
		let miss = game.alloc_rings(Player::Two, &[TileColor::Teal], 1, 1);
		assert_eq!(game.edge_channels(size3, miss).braid_mismatch, 3);
	}

	#[test]
	fn failed_braid_sum_variant_adds_every_braid_on_the_edge() {
		let mut rules = RuleSet::default();
		rules.braid_fail_mismatch = BraidFailMismatch::Sum;
		let mut game = Game::empty(rules);
		let two = orange_green_braid(&mut game, Player::One);
		let miss = game.alloc_rings(Player::Two, &[TileColor::Blue], 1, 1);
		assert_eq!(game.edge_channels(two, miss).braid_mismatch, 2, "lone 2-braid is +2");
		let three = game.alloc_typed_rings(
			Player::One,
			&[
				(TileColor::Orange, RingType::Braid),
				(TileColor::Green, RingType::Braid),
				(TileColor::Blue, RingType::Braid),
			],
			1,
			1,
		);
		let teal = game.alloc_rings(Player::Two, &[TileColor::Teal], 1, 1);
		assert_eq!(game.edge_channels(three, teal).braid_mismatch, 3, "lone 3-braid is +3");
		let other_two = game.alloc_typed_rings(
			Player::Two,
			&[(TileColor::Blue, RingType::Braid), (TileColor::Teal, RingType::Braid)],
			1,
			1,
		);
		assert_eq!(
			game.edge_channels(two, other_two).braid_mismatch,
			4,
			"2-braid vs 2-braid is +4"
		);
		assert_eq!(
			game.edge_channels(three, other_two).braid_mismatch,
			5,
			"2-braid vs 3-braid is +5"
		);
		assert_eq!(game.edge_channels(three, other_two), game.edge_channels(other_two, three));
		assert_eq!(
			game.edge_channels(two, miss).braid_mismatch,
			BraidFailMismatch::Sum.cost(two, miss)
		);
	}

	#[test]
	fn ring_weight_is_the_live_ring_count() {
		let mut game = Game::empty(RuleSet::default());
		let mixed = game.alloc_typed_rings(
			Player::One,
			&[
				(TileColor::Orange, RingType::Braid),
				(TileColor::Green, RingType::Braid),
				(TileColor::Blue, RingType::Zag),
			],
			9,
			1,
		);
		assert_eq!(mixed.weight, mixed.n_rings);
		assert_eq!(mixed.weight, 3);
		assert_eq!(
			mixed.braid_ring_count() + mixed.zag_ring_count() + mixed.normal_ring_count(),
			3
		);
	}

	fn place_isolated_dot(game: &mut Game, spawn_idx: usize, player: Player, zag: bool, dots: u8) {
		place_valued(game, spawn_idx, player, dots, 1, zag);
	}

	fn place_valued(
		game: &mut Game,
		spawn_idx: usize,
		player: Player,
		dots: u8,
		n_rings: u8,
		zag: bool,
	) {
		let spawn = game.board().spawns()[spawn_idx];
		let colors = [TileColor::Green, TileColor::Blue, TileColor::Orange];
		let n = usize::from(n_rings.max(1));
		let tile = if zag {
			let mut parts = Vec::with_capacity(n);
			parts.push((colors[0], true));
			for c in colors.iter().skip(1).take(n.saturating_sub(1)) {
				parts.push((*c, false));
			}
			game.alloc_ring_kinds(player, &parts, n_rings.max(1), dots)
		} else {
			game.alloc_rings(player, &colors[..n], n_rings.max(1), dots)
		};
		game.place_tile(spawn, tile);
	}

	fn step_p1_spawn0(game: &mut Game) {
		zero_budget(game);
		game.set_current(Player::One);
		let from = game.board().spawns()[0];
		let to = game.destinations(from).into_iter().next().expect("P1 can step");
		game.apply(Action::Move { from, to }).unwrap();
	}

	fn arm_low_value(game: &mut Game) {
		zero_budget(game);
		game.evaluate_low_value_for_test();
	}

	#[test]
	fn low_value_awards_the_lower_primary_piece() {
		let mut game = Game::empty(low_value_rules());
		place_isolated_dot(&mut game, 0, Player::One, false, 1);
		place_isolated_dot(&mut game, 4, Player::Two, false, 2);
		step_p1_spawn0(&mut game);
		assert_eq!(
			game.status(),
			GameStatus::Won { winner: Player::One, reason: EndReason::LowValue }
		);
		assert_eq!(
			game.low_value_resolution(),
			LowValueResolution::Win { winner: Player::One, pair: 0, by_piece_count: false }
		);
	}

	#[test]
	fn low_value_tiebreak_uses_zag_weighted_value() {
		// Same primary (dots+rings = 2). Tie-break: zag ring worth 2, so a
		// 1-dot/1-zag loses to a 1-dot/1-normal.
		let mut game = Game::empty(low_value_rules());
		place_isolated_dot(&mut game, 0, Player::One, true, 1);
		place_isolated_dot(&mut game, 4, Player::Two, false, 1);
		step_p1_spawn0(&mut game);
		assert_eq!(
			game.status(),
			GameStatus::Won { winner: Player::Two, reason: EndReason::LowValue }
		);
		assert_eq!(
			game.low_value_resolution(),
			LowValueResolution::Win { winner: Player::Two, pair: 0, by_piece_count: false }
		);
	}

	#[test]
	fn low_value_pair1_primary_breaks_a_lowest_tie() {
		let mut game = Game::empty(low_value_rules());
		place_valued(&mut game, 0, Player::One, 1, 1, false);
		place_valued(&mut game, 1, Player::One, 2, 1, false);
		place_valued(&mut game, 4, Player::Two, 1, 1, false);
		place_valued(&mut game, 5, Player::Two, 3, 1, false);
		arm_low_value(&mut game);
		assert_eq!(
			game.status(),
			GameStatus::Won { winner: Player::One, reason: EndReason::LowValue }
		);
		assert_eq!(
			game.low_value_resolution(),
			LowValueResolution::Win { winner: Player::One, pair: 1, by_piece_count: false }
		);
	}

	#[test]
	fn low_value_pair2_primary_breaks_after_two_ties() {
		let mut game = Game::empty(low_value_rules());
		place_valued(&mut game, 0, Player::One, 1, 1, false);
		place_valued(&mut game, 1, Player::One, 2, 1, false);
		place_valued(&mut game, 2, Player::One, 3, 1, false);
		place_valued(&mut game, 4, Player::Two, 1, 1, false);
		place_valued(&mut game, 5, Player::Two, 2, 1, false);
		place_valued(&mut game, 6, Player::Two, 3, 2, false);
		arm_low_value(&mut game);
		assert_eq!(
			game.status(),
			GameStatus::Won { winner: Player::One, reason: EndReason::LowValue }
		);
		assert_eq!(
			game.low_value_resolution(),
			LowValueResolution::Win { winner: Player::One, pair: 2, by_piece_count: false }
		);
	}

	#[test]
	fn low_value_equal_count_full_tie_is_a_draw() {
		let mut game = Game::empty(low_value_rules());
		place_valued(&mut game, 0, Player::One, 1, 1, false);
		place_valued(&mut game, 1, Player::One, 2, 1, false);
		place_valued(&mut game, 4, Player::Two, 1, 1, false);
		place_valued(&mut game, 5, Player::Two, 2, 1, false);
		arm_low_value(&mut game);
		assert_eq!(game.status(), GameStatus::Draw { reason: EndReason::LowValue });
		assert_eq!(game.low_value_resolution(), LowValueResolution::Draw);
	}

	#[test]
	fn low_value_unequal_count_awards_the_side_with_more_pieces() {
		let mut game = Game::empty(low_value_rules());
		place_isolated_dot(&mut game, 0, Player::One, false, 1);
		place_isolated_dot(&mut game, 4, Player::Two, false, 1);
		// Extra high-value leftover still beats an empty slot (empty = MAX).
		let extra = game.board().spawns()[2];
		let heavy = game.alloc_rings(
			Player::One,
			&[TileColor::Blue, TileColor::Orange, TileColor::Green],
			3,
			3,
		);
		game.place_tile(extra, heavy);
		arm_low_value(&mut game);
		assert_eq!(
			game.status(),
			GameStatus::Won { winner: Player::One, reason: EndReason::LowValue }
		);
		assert_eq!(
			game.low_value_resolution(),
			LowValueResolution::Win { winner: Player::One, pair: 1, by_piece_count: true }
		);
	}

	#[test]
	fn lockout_beats_low_value() {
		let mut game = Game::empty(low_value_rules());
		place_isolated_dot(&mut game, 0, Player::One, false, 1);
		// P2 is alive on the bench but cannot ADD (budget 0) and has no board
		// tile, so they lock out after P1's turn.
		let t = take(&mut game, Player::Two, TileColor::Orange, 3, 1);
		game.set_bench(Player::Two, vec![t]);
		zero_budget(&mut game);
		game.set_current(Player::One);
		assert!(!game.is_alive(Player::Two) || game.board_count(Player::Two) == 0);
		let from = game.board().spawns()[0];
		let to = game.destinations(from).into_iter().next().expect("P1 can step");
		game.apply(Action::Move { from, to }).unwrap();
		assert_eq!(
			game.status(),
			GameStatus::Won { winner: Player::One, reason: EndReason::LockedOut }
		);
	}

	#[test]
	fn surplus_tokens_are_not_usable_for_add() {
		let mut game = Game::empty(RuleSet::default());
		game.set_current(Player::One);
		assert_eq!(game.budget_token_count(Player::One), 7);
		assert_eq!(game.budget(Player::One), 0);
		assert_eq!(game.budget_surplus(Player::One), 7);
		assert!(game.legal_adds().is_empty());
		let spawn = game.board().spawns()[0];
		assert!(game.apply(Action::Add { bench_index: 0, node: spawn }).is_err());

		let tile = green(&mut game, Player::One, 1, 1);
		game.set_bench(Player::One, vec![tile]);
		assert_eq!(game.budget(Player::One), 1);
		assert_eq!(game.budget_surplus(Player::One), 6);
		assert_eq!(game.legal_adds().len(), game.board().spawns().len());
	}

	#[test]
	fn low_value_fires_with_empty_bench_and_surplus_tokens() {
		let mut game = Game::empty(low_value_rules());
		place_isolated_dot(&mut game, 0, Player::One, false, 1);
		place_isolated_dot(&mut game, 4, Player::Two, false, 2);
		assert_eq!(game.budget_token_count(Player::One), 7);
		assert_eq!(game.budget_token_count(Player::Two), 7);
		assert_eq!(game.budget(Player::One), 0);
		assert_eq!(game.budget(Player::Two), 0);
		game.set_current(Player::One);
		let from = game.board().spawns()[0];
		let to = game.destinations(from).into_iter().next().expect("P1 can step");
		game.apply(Action::Move { from, to }).unwrap();
		assert_eq!(
			game.status(),
			GameStatus::Won { winner: Player::One, reason: EndReason::LowValue }
		);
	}

	#[test]
	fn low_value_fires_when_both_possible_on_board_are_below_five() {
		// User position: P1 1 board + 0 usable (5 surplus, empty remaining
		// deck); P2 1 board + 3 usable. Neither can field 5 pieces, so the
		// game ends even though P2 still has budget to ADD.
		let mut game = Game::empty(low_value_rules());
		place_isolated_dot(&mut game, 0, Player::One, false, 1);
		place_isolated_dot(&mut game, 4, Player::Two, false, 2);
		game.set_bench(Player::One, Vec::new());
		game.set_budget_tokens(Player::One, vec![BudgetToken::Blank; 5]);
		let p2_deck = vec![
			green(&mut game, Player::Two, 1, 1),
			green(&mut game, Player::Two, 1, 1),
			green(&mut game, Player::Two, 1, 1),
		];
		game.set_bench(Player::Two, p2_deck);
		assert_eq!(game.board_count(Player::One), 1);
		assert_eq!(game.budget(Player::One), 0);
		assert_eq!(game.budget_surplus(Player::One), 5);
		assert_eq!(game.bench(Player::One).len(), 0);
		assert_eq!(game.board_count(Player::Two), 1);
		assert_eq!(game.bench(Player::Two).len(), 3);
		assert_eq!(game.budget(Player::Two), 3);
		assert_eq!(game.possible_board_tiles(Player::One), 1);
		assert_eq!(game.possible_board_tiles(Player::Two), 4);
		assert!(game.low_value_applies());
		game.set_current(Player::One);
		let from = game.board().spawns()[0];
		let to = game.destinations(from).into_iter().next().expect("P1 can step");
		game.apply(Action::Move { from, to }).unwrap();
		assert_eq!(
			game.status(),
			GameStatus::Won { winner: Player::One, reason: EndReason::LowValue }
		);
	}

	#[test]
	fn low_value_does_not_fire_when_one_side_can_still_field_five() {
		let mut game = Game::empty(low_value_rules());
		place_isolated_dot(&mut game, 0, Player::One, false, 1);
		place_isolated_dot(&mut game, 4, Player::Two, false, 2);
		game.set_bench(Player::One, Vec::new());
		game.set_budget_tokens(Player::One, vec![BudgetToken::Blank; 5]);
		let p2_deck = vec![
			green(&mut game, Player::Two, 1, 1),
			green(&mut game, Player::Two, 1, 1),
			green(&mut game, Player::Two, 1, 1),
			green(&mut game, Player::Two, 1, 1),
		];
		game.set_bench(Player::Two, p2_deck);
		assert_eq!(game.possible_board_tiles(Player::One), 1);
		assert_eq!(game.possible_board_tiles(Player::Two), 5);
		assert!(!game.low_value_applies());
		game.set_current(Player::One);
		let from = game.board().spawns()[0];
		let to = game.destinations(from).into_iter().next().expect("P1 can step");
		game.apply(Action::Move { from, to }).unwrap();
		assert_eq!(game.status(), GameStatus::InProgress);
	}

	#[test]
	fn remaining_deck_tile_re_enables_surplus_tokens() {
		let mut game = Game::empty(low_value_rules());
		place_isolated_dot(&mut game, 4, Player::Two, false, 1);
		game.set_current(Player::One);
		assert_eq!(game.bench(Player::One).len(), 0);
		assert_eq!(game.budget(Player::One), 0);
		assert_eq!(game.budget_token_count(Player::One), 7);
		let tile = green(&mut game, Player::One, 2, 1);
		game.set_bench(Player::One, vec![tile]);
		assert_eq!(game.budget(Player::One), 1);
		assert!(!game.legal_adds().is_empty());
		let dest = game
			.board()
			.spawns()
			.iter()
			.copied()
			.find(|&s| game.tile_at(s).is_none())
			.expect("empty spawn for re-enabled ADD");
		game.apply(Action::Add { bench_index: 0, node: dest }).unwrap();
		assert_eq!(game.bench(Player::One).len(), 0);
		assert_eq!(game.budget(Player::One), 0);
		assert!(game.budget_token_count(Player::One) >= 1);
	}
}
