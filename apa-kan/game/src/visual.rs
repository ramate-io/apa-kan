//! Shared board layout, palette, and arc styling for the desktop UI and PNG renderer.
//!
//! Game rules and bots do not belong here. This module only decides how the
//! already-computed position is drawn: z-order, tile size, window layout, and
//! the color of each circular-arc edge.

use std::f64::consts::PI;

use crate::board::{ArcEdge, Board, NodeId};
use crate::game::{
	selected_braid, BudgetToken, Game, GameStatus, Player, RingType, RuleSet, Tile, TileColor,
	ZagMismatchCount, BAG_SIZE_DEFAULT,
};

/// Previous desktop tile radius was 22px at a fixed scale of 210 board-px.
pub const PREV_TILE_RADIUS_PX: f32 = 22.0;
pub const PREV_SCALE: f32 = 210.0;

/// Doubled tile radius in board units (`2 * 22 / 210`), the size from before
/// the 1.5× / 1.35× enlargements. Closest inner-ring nodes are ~0.5858 apart.
pub const TILE_SIZE_SCALE: f32 = 1.0;
pub const TILE_RADIUS_UNITS: f32 = (PREV_TILE_RADIUS_PX / PREV_SCALE) * 2.0 * TILE_SIZE_SCALE;

/// Empty-node radius in board units; not doubled, so tiles stay the focus.
pub const NODE_RADIUS_UNITS: f32 = 8.0 / PREV_SCALE;
pub const SPAWN_RADIUS_UNITS: f32 = 10.0 / PREV_SCALE;

/// Generating circles have radius 1 centered 1 from the origin, so the
/// drawn petals reach 2 board units. Tiles on the outer ring stick out a bit
/// further; [`BOARD_EXTENT_UNITS`] covers both.
pub const BOARD_EXTENT_UNITS: f32 = 2.0 + TILE_RADIUS_UNITS;

/// Single-color arc width in board units (~4.2px at the old scale of 210).
pub const ARC_WIDTH_UNITS: f32 = 0.020;
pub const ARC_WIDTH_MIN_PX: f32 = 3.0;
pub const ARC_SAME_COLOR_WIDTH_SCALE: f32 = 1.45;
pub const STRIPE_STRAND_WIDTH_SCALE: f32 = 0.70;
pub const STRIPE_OFFSET_UNITS: f32 = 0.022;
pub const STRIPE_OFFSET_CENTER_UNITS: f32 = 0.014;
/// Compact diamond at a conflict-arc midpoint. `hitch_r` is the half-length
/// along the arc; the half-width across it is `hitch_r * DIAMOND_SLOPE` so
/// each edge has the same rise/run as a zag tooth.
pub const HITCH_RADIUS_UNITS: f32 = 0.052;
pub const HITCH_RADIUS_CENTER_UNITS: f32 = 0.040;
pub const HITCH_RADIUS_MIN_PX: f32 = 9.0;
pub const HITCH_RADIUS_CENTER_MIN_PX: f32 = 7.0;
/// Samples along each diamond edge so parallel offsets stay even.
pub const DIAMOND_EDGE_SAMPLES: usize = 4;
/// Finer than a highway strand so the skinny diamond does not fill in.
pub const DIAMOND_STRAND_WIDTH_SCALE: f32 = 0.42;
/// Spellcheck-underline zag: low amplitude, long wavelength, thin stroke.
pub const ZAG_AMP_UNITS: f32 = 0.008;
pub const ZAG_AMP_CENTER_UNITS: f32 = 0.006;
/// Trough-to-trough length of one arc zag tooth in board units.
pub const ZAG_TOOTH_UNITS: f64 = 0.12;
/// Diamond edge slope (across / along). Equal to a zag tooth's peak-to-trough
/// rise over a half-period (`ZAG_AMP` × 2 over `ZAG_TOOTH_UNITS / 2`).
pub const DIAMOND_SLOPE: f64 = (2.0 * ZAG_AMP_UNITS as f64) / (ZAG_TOOTH_UNITS * 0.5);
/// 1.5× the previous 0.22 scale; still finer than a highway strand (0.70).
pub const ZAG_STRAND_WIDTH_SCALE: f32 = 0.33;
/// Parallel zag-lane spacing as a fraction of the highway stripe offset.
/// Scaled with the thicker stroke so air between lines stays countable.
pub const ZAG_LANE_SPACING_SCALE: f64 = 1.55;
/// One overlay line per zag ring on a single tile (`n_rings` ≤ 3).
pub const MAX_TILE_ZAGS: usize = 3;
/// Two-ended SUM can ink every hitting zag on both tiles (3 + 3).
pub const MAX_EDGE_ZAGS: usize = MAX_TILE_ZAGS * 2;
/// Packed unmatched **normal** colors on one half of a conflict diamond.
pub const MAX_HITCH_COLORS: usize = 3;
pub const ARC_SAMPLES: usize = 96;
/// Tile-ring layout in 22px-tile units; scaled by `radius / PREV_TILE_RADIUS_PX`.
pub const TILE_RING_INSET: f32 = 1.7;
pub const TILE_RING_GAP: f32 = 3.3;
pub const TILE_ZAG_AMP: f32 = 0.80;
/// Tile zag stroke in 22px-tile units; 1.5× the previous 0.60, still thinner
/// than a normal ring so concentric zags keep air inside the rim.
pub const TILE_ZAG_STROKE: f32 = 0.90;
pub const TILE_ZAG_TEETH: u32 = 16;
pub const TILE_ZAG_SAMPLES: u32 = 96;
/// Tile-rim braid: the old conflict weave, one thread per braid slot.
pub const TILE_BRAID_AMP: f32 = 1.6;
/// Thinner than a zag ring so over/under threads keep visible air between them.
pub const TILE_BRAID_STROKE: f32 = 0.70;
pub const TILE_BRAID_SAMPLES: u32 = 128;
/// Tile-rim weave period: half the arc conflict period (~2× frequency).
pub const TILE_BRAID_TURN_UNITS: f64 = BRAID_TURN_UNITS * 0.5;
/// Satisfied-braid weave (the old two-ended conflict braid). Amplitude and
/// strand width match that glyph; turn count follows arc length.
pub const BRAID_AMP_UNITS: f32 = 0.030;
pub const BRAID_AMP_CENTER_UNITS: f32 = 0.016;
pub const BRAID_STRAND_WIDTH_SCALE: f32 = 0.78;
/// Board units per full braid twist. The original conflict-weave period
/// (`arc_length / 0.42`), not the zag tooth length.
pub const BRAID_TURN_UNITS: f64 = 0.42;
/// Floor/ceiling on [`braid_turns`] / [`braid_turns_along`], from the original
/// conflict braid (`(arc_length / 0.42).clamp(1.2, 3.6)`).
pub const BRAID_TURNS_MIN: f64 = 1.2;
pub const BRAID_TURNS_MAX: f64 = 3.6;

pub const Z_ARC: f32 = 0.0;
pub const Z_ARC_OVER: f32 = 0.08;
pub const Z_NODE: f32 = 1.0;
pub const Z_TILE: f32 = 2.0;
pub const Z_TILE_RING: f32 = 2.15;
pub const Z_TILE_DOT: f32 = 2.3;
pub const Z_HIGHLIGHT: f32 = 3.0;
pub const Z_MARKER: f32 = 4.0;

/// Top HUD band (title, turn, last action, warning) in window pixels.
pub const HUD_TOP_PX: f32 = 112.0;
/// Bottom help line in window pixels.
pub const HELP_BOTTOM_PX: f32 = 36.0;
pub const MARGIN_PX: f32 = 14.0;
pub const BOARD_GAP_PX: f32 = 12.0;
pub const SIDE_MARGIN_PX: f32 = 16.0;

/// ASCII separator: the default Bevy font cannot draw U+00B7 (`·`), which
/// showed up as tofu boxes between HUD items.
pub const HUD_SEP: &str = " | ";

pub const HELP_TEXT: &str =
	"Click a spawn then any unplaced deck tile to ADD (costs 1 spawn token). MOVE your tile. Win: 5 of 9, or lowest pieces in order if both usable budgets are 0 with <5 tiles.";

pub const NEUTRAL_ARC_RGB: [f32; 3] = [0.38, 0.40, 0.45];
pub const BG_RGB: [f32; 3] = [0.07, 0.08, 0.10];
pub const P1_FILL_RGB: [f32; 3] = [0.91, 0.89, 0.82];
pub const P2_FILL_RGB: [f32; 3] = [0.12, 0.12, 0.14];
pub const SPAWN_RGB: [f32; 3] = [0.93, 0.84, 0.43];
pub const NODE_RGB: [f32; 3] = [0.75, 0.75, 0.75];
pub const CENTER_RGB: [f32; 3] = [0.85, 0.85, 0.85];
pub const HIGHLIGHT_RGB: [f32; 3] = [0.95, 0.85, 0.25];
pub const DEST_RGB: [f32; 3] = [0.55, 0.85, 1.00];
/// Rim on a blank budget token so dark P2 blanks stay visible.
pub const BLANK_TOKEN_RIM_RGB: [f32; 3] = [0.55, 0.56, 0.58];

/// Unusable surplus or unfilled blanks: opaque owner-fill face, rim only.
pub fn budget_slot_is_blank_face(token: &BudgetToken, index: usize, usable: usize) -> bool {
	index >= usable || matches!(token, BudgetToken::Blank)
}

/// Usable budget, or `usable+surplus` when tokens exceed the remaining deck.
pub fn format_budget(game: &Game, player: Player) -> String {
	let usable = game.budget(player);
	let surplus = game.budget_surplus(player);
	if surplus == 0 {
		usable.to_string()
	} else {
		format!("{usable}+{surplus}")
	}
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ArcPaint {
	pub body: ArcBody,
	/// Zag strokes **from** the arc's `from` tile, fanning out on the + side.
	/// Used for one-ended potentials.
	pub zags_from: [Option<TileColor>; MAX_TILE_ZAGS],
	/// Zag strokes **from** the arc's `to` tile, fanning out on the − side.
	pub zags_to: [Option<TileColor>; MAX_TILE_ZAGS],
	/// Realized zag mismatches bundled around the arc center line (SUM / MAX
	/// two-ended edges). One-ended edges leave this empty.
	pub zags_mid: [Option<TileColor>; MAX_EDGE_ZAGS],
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ArcBody {
	Neutral,
	Solid {
		color: TileColor,
		thick: bool,
	},
	/// Midpoint diamond joining unmatched leftover **normal** colors. Two-ended
	/// true conflict only (both ends present toward each other, they share no
	/// normal, no zag mismatch, and no satisfied braid). One elongated diamond
	/// per edge: leftover highways wrap one pair of edges as a bundle. An
	/// unsatisfied braid still weaves its threads along that half's path so
	/// the braid stays inside the conflict, mixed with leftover stripes.
	Hitch {
		from: [Option<TileColor>; MAX_HITCH_COLORS],
		to: [Option<TileColor>; MAX_HITCH_COLORS],
		from_braid: [Option<TileColor>; 3],
		to_braid: [Option<TileColor>; 3],
	},
	/// Parallel highway stripes: one-ended undecided edges, or two-ended
	/// edges that share more than one normal color. Zag overlays sit outside.
	Stripes {
		a: TileColor,
		b: TileColor,
		zag_a: bool,
		zag_b: bool,
	},
	/// No connection line in the middle. Used when a confirmed zag mismatch
	/// occupies a two-ended edge (the zag is the only realized effect).
	Hidden,
	/// Threaded braid weave, one thread per slot, colored by that slot.
	/// Used for a satisfied two-ended braid and for a one-ended would-be
	/// braid (no neighbor yet). Jag overlays may still sit on the same edge.
	Braid {
		colors: [Option<TileColor>; 3],
	},
}

impl ArcPaint {
	fn no_zags() -> [Option<TileColor>; MAX_TILE_ZAGS] {
		[None; MAX_TILE_ZAGS]
	}

	fn no_mid() -> [Option<TileColor>; MAX_EDGE_ZAGS] {
		[None; MAX_EDGE_ZAGS]
	}

	fn with_sides(
		body: ArcBody,
		zags_from: [Option<TileColor>; MAX_TILE_ZAGS],
		zags_to: [Option<TileColor>; MAX_TILE_ZAGS],
	) -> Self {
		Self { body, zags_from, zags_to, zags_mid: Self::no_mid() }
	}

	pub fn neutral() -> Self {
		Self::with_sides(ArcBody::Neutral, Self::no_zags(), Self::no_zags())
	}

	pub fn solid(color: TileColor, thick: bool) -> Self {
		Self::with_sides(ArcBody::Solid { color, thick }, Self::no_zags(), Self::no_zags())
	}

	pub fn hitch(
		from: [Option<TileColor>; MAX_HITCH_COLORS],
		to: [Option<TileColor>; MAX_HITCH_COLORS],
	) -> Self {
		Self::hitch_braid(from, to, [None; 3], [None; 3])
	}

	pub fn hitch_braid(
		from: [Option<TileColor>; MAX_HITCH_COLORS],
		to: [Option<TileColor>; MAX_HITCH_COLORS],
		from_braid: [Option<TileColor>; 3],
		to_braid: [Option<TileColor>; 3],
	) -> Self {
		Self::with_sides(
			ArcBody::Hitch { from, to, from_braid, to_braid },
			Self::no_zags(),
			Self::no_zags(),
		)
	}

	pub fn stripes(a: TileColor, b: TileColor, zag_a: bool, zag_b: bool) -> Self {
		Self::with_sides(ArcBody::Stripes { a, b, zag_a, zag_b }, Self::no_zags(), Self::no_zags())
	}

	pub fn braid(colors: [Option<TileColor>; 3]) -> Self {
		Self::with_sides(ArcBody::Braid { colors }, Self::no_zags(), Self::no_zags())
	}

	pub fn zag_count(self) -> usize {
		self.zag_from_count() + self.zag_to_count() + self.zag_mid_count()
	}

	pub fn zag_from_count(self) -> usize {
		self.zags_from.iter().filter(|z| z.is_some()).count()
	}

	pub fn zag_to_count(self) -> usize {
		self.zags_to.iter().filter(|z| z.is_some()).count()
	}

	pub fn zag_mid_count(self) -> usize {
		self.zags_mid.iter().filter(|z| z.is_some()).count()
	}
}

pub fn shared_normal_colors(a: Tile, b: Tile) -> [Option<TileColor>; 2] {
	let mut out = [None; 2];
	let mut n = 0usize;
	for (i, &c) in a.ring_colors().iter().enumerate() {
		if !a.ring_type[i].is_normal() || !b.has_normal(c) {
			continue;
		}
		if !out.contains(&Some(c)) && n < 2 {
			out[n] = Some(c);
			n += 1;
		}
	}
	out
}

fn pack_zags(colors: impl IntoIterator<Item = TileColor>) -> [Option<TileColor>; MAX_TILE_ZAGS] {
	let mut out = [None; MAX_TILE_ZAGS];
	let mut n = 0usize;
	for c in colors {
		if n >= MAX_TILE_ZAGS {
			break;
		}
		out[n] = Some(c);
		n += 1;
	}
	out
}

/// Zag strokes on a single tile, one color per drawn zag ring.
/// One-ended edges draw every zag (potential). Two-ended edges use
/// [`edge_zag_strokes`] so only a realized mismatch is inked.
pub fn tile_zag_strokes(tile: Tile) -> [Option<TileColor>; MAX_TILE_ZAGS] {
	pack_zags(
		(0..tile.ring_strokes())
			.filter(|&k| tile.stroke_is_zag(k))
			.map(|k| tile.stroke_color(k)),
	)
}

fn hitting_zag_colors(tile: Tile, other: Tile) -> impl Iterator<Item = TileColor> {
	(0..tile.ring_strokes()).filter_map(move |k| {
		if tile.stroke_is_zag(k) && other.has_color(tile.stroke_color(k)) {
			Some(tile.stroke_color(k))
		} else {
			None
		}
	})
}

fn pack_edge_zags(
	colors: impl IntoIterator<Item = TileColor>,
) -> [Option<TileColor>; MAX_EDGE_ZAGS] {
	let mut out = [None; MAX_EDGE_ZAGS];
	let mut n = 0usize;
	for c in colors {
		if n >= MAX_EDGE_ZAGS {
			break;
		}
		out[n] = Some(c);
		n += 1;
	}
	out
}

/// Realized zag strokes inked on the center line for an undirected edge count.
pub fn edge_zag_strokes(
	a: Tile,
	b: Tile,
	mode: ZagMismatchCount,
) -> [Option<TileColor>; MAX_EDGE_ZAGS] {
	match mode {
		ZagMismatchCount::Sum => {
			pack_edge_zags(hitting_zag_colors(a, b).chain(hitting_zag_colors(b, a)))
		}
		ZagMismatchCount::Max => pack_edge_zags(TileColor::ALL.iter().flat_map(|&c| {
			let n = a.zag_hits_of_color(b, c).max(b.zag_hits_of_color(a, c));
			std::iter::repeat(c).take(n as usize)
		})),
	}
}

/// Leftover unique **normal** colors on `tile` for a conflict diamond. Zags
/// and braid slots are omitted: zags already draw as squiggles, and an
/// unsatisfied braid weaves along the hitch half instead of becoming extra
/// highway stripes.
pub fn unique_normals(tile: Tile) -> [Option<TileColor>; MAX_HITCH_COLORS] {
	let mut out = [None; MAX_HITCH_COLORS];
	let mut n = 0usize;
	for k in 0..tile.ring_strokes() {
		if !tile.stroke_type(k).is_normal() {
			continue;
		}
		let c = tile.stroke_color(k);
		if !out.contains(&Some(c)) && n < MAX_HITCH_COLORS {
			out[n] = Some(c);
			n += 1;
		}
	}
	out
}

/// Braid threads mixed into a hitch half. Empty when this tile has no braid.
pub fn hitch_braid_colors(tile: Tile) -> [Option<TileColor>; 3] {
	if tile.braid_ring_count() >= 2 {
		tile.braid_colors()
	} else {
		[None; 3]
	}
}

pub fn packed_color_count(colors: [Option<TileColor>; MAX_HITCH_COLORS]) -> usize {
	colors.iter().filter(|c| c.is_some()).count()
}

/// Lane offsets for 1 / 2 / 3 parallel highway stripes on a hitch half.
pub fn highway_offsets(n: usize, stripe_off: f64) -> [f64; MAX_HITCH_COLORS] {
	match n {
		0 | 1 => [0.0, 0.0, 0.0],
		2 => [stripe_off, -stripe_off, 0.0],
		_ => [stripe_off, 0.0, -stripe_off],
	}
}

pub fn highway_strand_width(n: usize, arc_width: f64) -> f64 {
	if n <= 1 {
		arc_width
	} else {
		arc_width * f64::from(STRIPE_STRAND_WIDTH_SCALE)
	}
}

/// Bundle offsets around **one** diamond. Lane 0 is the diamond centerline;
/// extra colors pack on the outside (`outside` is +1 left / −1 right) so a
/// multi-color edge never draws a second inner diamond.
pub fn diamond_bundle_offsets(n: usize, stripe_off: f64, outside: f64) -> [f64; MAX_HITCH_COLORS] {
	let s = if outside < 0.0 { -stripe_off.abs() } else { stripe_off.abs() };
	match n {
		0 | 1 => [0.0, 0.0, 0.0],
		2 => [0.0, s, 0.0],
		_ => [0.0, s, 2.0 * s],
	}
}

/// Spacing that keeps leftover highway stripes outside a hitch-half braid
/// weave so both stay readable in the diamond bundle.
pub fn hitch_leftover_lane(stripe_off: f64, braid_amp: f64, braid_w: f64, leftover_w: f64) -> f64 {
	let need = braid_amp.abs() + braid_w.abs() * 0.5 + leftover_w.abs() * 0.5 + 0.8;
	stripe_off.abs().max(need)
}

/// Leftover highway lanes when a braid weave occupies the diamond centerline.
pub fn diamond_leftover_offsets(n: usize, lane: f64, outside: f64) -> [f64; MAX_HITCH_COLORS] {
	let s = if outside < 0.0 { -lane.abs() } else { lane.abs() };
	match n {
		0 => [0.0, 0.0, 0.0],
		1 => [s, 0.0, 0.0],
		_ => [s, 2.0 * s, 0.0],
	}
}

pub fn diamond_strand_width(arc_width: f64) -> f64 {
	arc_width * f64::from(DIAMOND_STRAND_WIDTH_SCALE)
}

pub fn diamond_across(along_r: f64) -> f64 {
	along_r * DIAMOND_SLOPE
}

fn two_ended(a: Tile, b: Tile, mode: ZagMismatchCount) -> ArcPaint {
	let zag_mid = if a.zag_hits_on(b) > 0 || b.zag_hits_on(a) > 0 {
		edge_zag_strokes(a, b, mode)
	} else {
		ArcPaint::no_mid()
	};
	if let Some((tile, _)) = selected_braid(a, b) {
		return ArcPaint {
			body: ArcBody::Braid { colors: tile.braid_colors() },
			zags_from: ArcPaint::no_zags(),
			zags_to: ArcPaint::no_zags(),
			zags_mid: zag_mid,
		};
	}
	if zag_mid[0].is_some() {
		// Confirmed zag mismatch, no satisfied braid: draw only the zag(s).
		return ArcPaint {
			body: ArcBody::Hidden,
			zags_from: ArcPaint::no_zags(),
			zags_to: ArcPaint::no_zags(),
			zags_mid: zag_mid,
		};
	}
	let shared = shared_normal_colors(a, b);
	let body = match (shared[0], shared[1]) {
		(Some(color), None) => ArcBody::Solid { color, thick: true },
		(Some(c1), Some(c2)) => ArcBody::Stripes { a: c1, b: c2, zag_a: false, zag_b: false },
		(None, _) if a.present_toward(b) && b.present_toward(a) => ArcBody::Hitch {
			from: unique_normals(a),
			to: unique_normals(b),
			from_braid: hitch_braid_colors(a),
			to_braid: hitch_braid_colors(b),
		},
		(None, _) => ArcBody::Neutral,
	};
	ArcPaint::with_sides(body, ArcPaint::no_zags(), ArcPaint::no_zags())
}

/// Paint an arc using the default SUM zag-mismatch rule.
pub fn arc_paint(from: Option<Tile>, to: Option<Tile>) -> ArcPaint {
	arc_paint_with(from, to, ZagMismatchCount::Sum)
}

pub fn arc_paint_with(from: Option<Tile>, to: Option<Tile>, mode: ZagMismatchCount) -> ArcPaint {
	match (from, to) {
		(None, None) => ArcPaint::neutral(),
		(Some(t), None) => one_ended(t, true),
		(None, Some(t)) => one_ended(t, false),
		(Some(a), Some(b)) => two_ended(a, b, mode),
	}
}

/// One-ended edges: unique **normal** colors as a highway in the middle,
/// this tile's zag strokes fanned on one side, and a would-be braid as the
/// threaded weave in the braid's ring colors.
fn one_ended(tile: Tile, on_from: bool) -> ArcPaint {
	let zags = tile_zag_strokes(tile);
	let body = if tile.braid_ring_count() >= 2 {
		ArcBody::Braid { colors: tile.braid_colors() }
	} else {
		let mut normals: Vec<TileColor> = Vec::new();
		for k in 0..tile.ring_strokes() {
			if tile.stroke_is_zag(k) {
				continue;
			}
			let c = tile.stroke_color(k);
			if !normals.contains(&c) {
				normals.push(c);
			}
		}
		match normals.as_slice() {
			[] if zags[0].is_none() => ArcBody::Neutral,
			[] => ArcBody::Hidden,
			[c] => ArcBody::Solid { color: *c, thick: false },
			[a, b, ..] => ArcBody::Stripes { a: *a, b: *b, zag_a: false, zag_b: false },
		}
	};
	if on_from {
		ArcPaint::with_sides(body, zags, ArcPaint::no_zags())
	} else {
		ArcPaint::with_sides(body, ArcPaint::no_zags(), zags)
	}
}

/// Stroke counts and radii for concentric tile rings, including zag amplitude
/// small enough that two zag rings stay distinguishable.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TileRingDraw {
	pub scale: f32,
	pub inset: f32,
	pub gap: f32,
	pub stroke: f32,
	pub zag_amp: f32,
	pub zag_stroke: f32,
	pub zag_teeth: u32,
	pub zag_samples: u32,
	pub braid_amp: f32,
	pub braid_stroke: f32,
	pub braid_samples: u32,
	pub dot_r: f32,
	pub dot_spread: f32,
}

impl TileRingDraw {
	pub fn for_radius(radius: f32) -> Self {
		let scale = radius / PREV_TILE_RADIUS_PX;
		Self {
			scale,
			inset: TILE_RING_INSET * scale,
			gap: TILE_RING_GAP * scale,
			stroke: (2.2 * scale).max(1.6),
			zag_amp: (TILE_ZAG_AMP * scale).max(0.6),
			zag_stroke: (TILE_ZAG_STROKE * scale).max(0.5),
			zag_teeth: TILE_ZAG_TEETH,
			zag_samples: TILE_ZAG_SAMPLES,
			braid_amp: (TILE_BRAID_AMP * scale).max(0.4),
			braid_stroke: (TILE_BRAID_STROKE * scale).max(0.28),
			braid_samples: TILE_BRAID_SAMPLES,
			dot_r: (2.6 * scale).max(1.8),
			dot_spread: 7.0 * scale,
		}
	}

	pub fn ring_radius(self, tile_radius: f32, k: u8) -> f32 {
		tile_radius - self.inset - f32::from(k) * self.gap
	}
}

pub fn tile_rgb(color: TileColor) -> [f32; 3] {
	match color {
		TileColor::Green => [0.18, 0.66, 0.32],
		TileColor::Blue => [0.24, 0.48, 0.90],
		TileColor::Orange => [0.91, 0.52, 0.16],
		TileColor::LavenderPink => [0.90, 0.48, 0.74],
		TileColor::Teal => [0.08, 0.66, 0.64],
		TileColor::Maroon => [0.52, 0.10, 0.18],
	}
}

pub fn rgb8(rgb: [f32; 3]) -> [u8; 3] {
	[
		(rgb[0] * 255.0).round().clamp(0.0, 255.0) as u8,
		(rgb[1] * 255.0).round().clamp(0.0, 255.0) as u8,
		(rgb[2] * 255.0).round().clamp(0.0, 255.0) as u8,
	]
}

pub fn owner_fill_rgb(player: Player) -> [f32; 3] {
	match player {
		Player::One => P1_FILL_RGB,
		Player::Two => P2_FILL_RGB,
	}
}

/// Pixel layout for a window whose camera origin is the window center, +Y up.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ViewLayout {
	pub width: f32,
	pub height: f32,
	pub scale: f32,
	pub origin_x: f32,
	pub origin_y: f32,
	pub tile_radius: f32,
	pub node_radius: f32,
	pub spawn_radius: f32,
	pub bench_tile_radius: f32,
	pub bench_spacing: f32,
	pub p1_bench_y: f32,
	pub p2_bench_y: f32,
	pub n_deck_slots: usize,
	pub deck_origin_x: f32,
	pub arc_width: f32,
	pub stripe_off: f32,
	pub stripe_off_center: f32,
	pub hitch_r: f32,
	pub hitch_r_center: f32,
	pub zag_amp: f32,
	pub zag_amp_center: f32,
	pub braid_amp: f32,
	pub braid_amp_center: f32,
}

impl ViewLayout {
	pub fn from_window(width: f32, height: f32) -> Self {
		Self::from_window_with_deck(width, height, BAG_SIZE_DEFAULT)
	}

	pub fn from_window_with_deck(width: f32, height: f32, deck_slots: usize) -> Self {
		let width = width.max(320.0);
		let height = height.max(240.0);
		let n_deck_slots = deck_slots.max(1);
		let tile_u = TILE_RADIUS_UNITS;
		let extent_u = BOARD_EXTENT_UNITS;
		let fixed_h = HUD_TOP_PX + HELP_BOTTOM_PX + 2.0 * MARGIN_PX + 2.0 * BOARD_GAP_PX;
		let h_denom = 4.0 * tile_u + 2.0 * extent_u;
		let scale_h = (height - fixed_h) / h_denom;
		let scale_w = (width - 2.0 * SIDE_MARGIN_PX) / (2.0 * extent_u);
		let scale = scale_h.min(scale_w).max(8.0);

		let tile_radius = tile_u * scale;
		let node_radius = NODE_RADIUS_UNITS * scale;
		let spawn_radius = SPAWN_RADIUS_UNITS * scale;
		// Full-deck row (default 11) plus a budget-token stack on the left;
		// shrink the deck discs (not the board tiles) and cap spacing.
		let n_slots = n_deck_slots as f32;
		let min_gap = 4.0;
		let stack_reserve = ((width - 2.0 * SIDE_MARGIN_PX) / 9.0).max(26.0);
		let available_w = (width - 2.0 * SIDE_MARGIN_PX - stack_reserve).max(80.0);
		let gaps = (n_slots - 1.0).max(1.0);
		let max_bench_r = ((available_w - gaps * min_gap) / (2.0 * n_slots)).max(7.0);
		let bench_tile_radius = tile_radius.min(max_bench_r);
		let desired_spacing = 2.0 * bench_tile_radius + (0.28 * bench_tile_radius).max(min_gap);
		let max_spacing = (available_w - 2.0 * bench_tile_radius) / gaps;
		let bench_spacing = desired_spacing.min(max_spacing.max(2.0 * bench_tile_radius + 1.0));

		let top = height / 2.0;
		let bottom = -height / 2.0;
		let p2_bench_y = top - HUD_TOP_PX - MARGIN_PX - bench_tile_radius;
		let p1_bench_y = bottom + HELP_BOTTOM_PX + MARGIN_PX + bench_tile_radius;

		let board_top = p2_bench_y - bench_tile_radius - BOARD_GAP_PX;
		let board_bot = p1_bench_y + bench_tile_radius + BOARD_GAP_PX;
		let origin_y = 0.5 * (board_top + board_bot);
		let origin_x = 0.0;
		// Keep the budget stack in the left gutter; shift the deck right if a
		// centered 11-tile row would collide with it (narrow/mobile windows).
		let row_span = bench_spacing * (n_slots - 1.0);
		let stack_x = -width / 2.0 + SIDE_MARGIN_PX + bench_tile_radius;
		let first_min = stack_x + bench_spacing;
		let last_max = width / 2.0 - SIDE_MARGIN_PX - bench_tile_radius;
		let mut deck_origin_x = 0.0;
		if -row_span / 2.0 < first_min {
			deck_origin_x = first_min + row_span / 2.0;
		}
		if deck_origin_x + row_span / 2.0 > last_max {
			deck_origin_x = last_max - row_span / 2.0;
		}

		let arc_width = (ARC_WIDTH_UNITS * scale).max(ARC_WIDTH_MIN_PX);
		let stripe_off = (STRIPE_OFFSET_UNITS * scale).max(2.2);
		let stripe_off_center = (STRIPE_OFFSET_CENTER_UNITS * scale).max(1.6);
		let hitch_r = (HITCH_RADIUS_UNITS * scale).max(HITCH_RADIUS_MIN_PX);
		let hitch_r_center = (HITCH_RADIUS_CENTER_UNITS * scale).max(HITCH_RADIUS_CENTER_MIN_PX);
		let zag_amp = (ZAG_AMP_UNITS * scale).max(0.85);
		let zag_amp_center = (ZAG_AMP_CENTER_UNITS * scale).max(0.7);
		let braid_amp = (BRAID_AMP_UNITS * scale).max(2.0);
		let braid_amp_center = (BRAID_AMP_CENTER_UNITS * scale).max(1.4);

		Self {
			width,
			height,
			scale,
			origin_x,
			origin_y,
			tile_radius,
			node_radius,
			spawn_radius,
			bench_tile_radius,
			bench_spacing,
			p1_bench_y,
			p2_bench_y,
			n_deck_slots,
			deck_origin_x,
			arc_width,
			stripe_off,
			stripe_off_center,
			hitch_r,
			hitch_r_center,
			zag_amp,
			zag_amp_center,
			braid_amp,
			braid_amp_center,
		}
	}

	pub fn visual_radius(&self) -> f32 {
		BOARD_EXTENT_UNITS * self.scale
	}

	pub fn board_to_world(&self, p: [f64; 2]) -> [f32; 2] {
		[self.origin_x + p[0] as f32 * self.scale, self.origin_y + p[1] as f32 * self.scale]
	}

	pub fn world_to_image(&self, w: [f32; 2]) -> [f32; 2] {
		[w[0] + self.width / 2.0, self.height / 2.0 - w[1]]
	}

	pub fn board_to_image(&self, p: [f64; 2]) -> [f32; 2] {
		self.world_to_image(self.board_to_world(p))
	}

	pub fn bench_pos(&self, player: Player, index: usize, len: usize) -> [f32; 2] {
		let y = match player {
			Player::Two => self.p2_bench_y,
			Player::One => self.p1_bench_y,
		};
		let total = self.bench_spacing * len.saturating_sub(1) as f32;
		let start = self.deck_origin_x - total / 2.0;
		[start + index as f32 * self.bench_spacing, y]
	}

	/// Overlapping pile just to the left of the full-deck row. Index 0 is
	/// the back of the stack (opening blanks); later tokens sit toward the board.
	pub fn budget_stack_pos(&self, player: Player, index: usize, _len: usize) -> [f32; 2] {
		let y = match player {
			Player::Two => self.p2_bench_y,
			Player::One => self.p1_bench_y,
		};
		let bench_left = self.bench_pos(player, 0, self.n_deck_slots);
		let min_x = -self.width / 2.0 + SIDE_MARGIN_PX + self.bench_tile_radius;
		let x = (bench_left[0] - self.bench_spacing).max(min_x);
		let toward_board = match player {
			Player::One => 1.0,
			Player::Two => -1.0,
		};
		let step = (self.bench_tile_radius * 0.22).max(2.5);
		let i = index as f32;
		[x, y + toward_board * i * step]
	}

	pub fn board_top(&self) -> f32 {
		self.origin_y + self.visual_radius()
	}

	pub fn board_bottom(&self) -> f32 {
		self.origin_y - self.visual_radius()
	}

	pub fn p2_bench_bottom(&self) -> f32 {
		self.p2_bench_y - self.bench_tile_radius
	}

	pub fn p1_bench_top(&self) -> f32 {
		self.p1_bench_y + self.bench_tile_radius
	}

	pub fn hud_world_bottom(&self) -> f32 {
		self.height / 2.0 - HUD_TOP_PX
	}

	pub fn help_world_top(&self) -> f32 {
		-self.height / 2.0 + HELP_BOTTOM_PX
	}
}

pub fn offset_polyline(centerline: &[[f64; 2]], offset: f64) -> Vec<[f64; 2]> {
	let mut out = Vec::with_capacity(centerline.len());
	for i in 0..centerline.len() {
		let (nx, ny) = polyline_normal(centerline, i);
		let p = centerline[i];
		out.push([p[0] + nx * offset, p[1] + ny * offset]);
	}
	out
}

/// Constant-offset parallel strokes (highway stripes) along `centerline`.
pub fn parallel_strands(centerline: &[[f64; 2]], offset: f64) -> (Vec<[f64; 2]>, Vec<[f64; 2]>) {
	(offset_polyline(centerline, offset), offset_polyline(centerline, -offset))
}

#[derive(Clone, Copy, Debug)]
pub struct HitchFrame {
	pub index: usize,
	pub mid: [f64; 2],
	pub tangent: [f64; 2],
	pub normal: [f64; 2],
}

pub fn midpoint_frame(pts: &[[f64; 2]]) -> HitchFrame {
	let index = pts.len() / 2;
	let mid = pts.get(index).copied().unwrap_or([0.0, 0.0]);
	let (nx, ny) = if pts.is_empty() { (0.0, 1.0) } else { polyline_normal(pts, index) };
	HitchFrame { index, mid, tangent: [ny, -nx], normal: [nx, ny] }
}

fn unit2(v: [f64; 2], fallback: [f64; 2]) -> [f64; 2] {
	let len = (v[0] * v[0] + v[1] * v[1]).sqrt();
	if len > 1e-12 {
		[v[0] / len, v[1] / len]
	} else {
		fallback
	}
}

/// Vertices of the compact conflict diamond at an arc midpoint.
/// Diagonals sit on the arc tangent (`along_from` / `along_to`) and its normal
/// (`left` / `right`). Tile A wraps the left pair of edges; tile B the right.
#[derive(Clone, Copy, Debug)]
pub struct ConflictDiamond {
	pub along_from: [f64; 2],
	pub along_to: [f64; 2],
	pub left: [f64; 2],
	pub right: [f64; 2],
}

pub fn conflict_diamond(
	mid: [f64; 2],
	tangent: [f64; 2],
	normal: [f64; 2],
	radius: f64,
) -> ConflictDiamond {
	let tan = unit2(tangent, [1.0, 0.0]);
	let nor = unit2(normal, [-tan[1], tan[0]]);
	let along = radius.max(0.0);
	let across = diamond_across(along);
	ConflictDiamond {
		along_from: [mid[0] - tan[0] * along, mid[1] - tan[1] * along],
		along_to: [mid[0] + tan[0] * along, mid[1] + tan[1] * along],
		left: [mid[0] + nor[0] * across, mid[1] + nor[1] * across],
		right: [mid[0] - nor[0] * across, mid[1] - nor[1] * across],
	}
}

fn push_diamond_edge(out: &mut Vec<[f64; 2]>, a: [f64; 2], b: [f64; 2]) {
	let n = DIAMOND_EDGE_SAMPLES.max(1);
	for i in 1..=n {
		let t = i as f64 / n as f64;
		out.push(polyline_lerp(a, b, t));
	}
}

/// From-tile highway continued around the left two edges of the diamond
/// (`along_from` → `left` → `along_to`). `from_pts` is the shortened arc half
/// that already ends at the from-side vertex.
pub fn diamond_from_path(from_pts: &[[f64; 2]], diamond: &ConflictDiamond) -> Vec<[f64; 2]> {
	let mut out = from_pts.to_vec();
	match out.last_mut() {
		Some(last) => *last = diamond.along_from,
		None => out.push(diamond.along_from),
	}
	push_diamond_edge(&mut out, diamond.along_from, diamond.left);
	push_diamond_edge(&mut out, diamond.left, diamond.along_to);
	out
}

/// To-tile highway continued around the right two edges of the diamond
/// (`along_from` → `right` → `along_to`, then the shortened to-half).
pub fn diamond_to_path(to_pts: &[[f64; 2]], diamond: &ConflictDiamond) -> Vec<[f64; 2]> {
	let mut out = Vec::with_capacity(to_pts.len() + 2 * DIAMOND_EDGE_SAMPLES + 1);
	out.push(diamond.along_from);
	push_diamond_edge(&mut out, diamond.along_from, diamond.right);
	push_diamond_edge(&mut out, diamond.right, diamond.along_to);
	if to_pts.is_empty() {
		return out;
	}
	let mut rest = to_pts.to_vec();
	if let Some(first) = rest.first_mut() {
		*first = diamond.along_to;
	}
	if let Some(&last) = out.last() {
		if polyline_dist(last, rest[0]) < 1e-9 {
			out.extend(rest.into_iter().skip(1));
			return out;
		}
	}
	out.extend(rest);
	out
}

fn polyline_dist(a: [f64; 2], b: [f64; 2]) -> f64 {
	let dx = a[0] - b[0];
	let dy = a[1] - b[1];
	(dx * dx + dy * dy).sqrt()
}

pub fn polyline_length(pts: &[[f64; 2]]) -> f64 {
	pts.windows(2).map(|w| polyline_dist(w[0], w[1])).sum()
}

fn polyline_lerp(a: [f64; 2], b: [f64; 2], t: f64) -> [f64; 2] {
	[a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t]
}

/// Drop the last `clearance` of path length so stripes meet the diamond vertex.
pub fn shorten_polyline_end(pts: &[[f64; 2]], clearance: f64) -> Vec<[f64; 2]> {
	if pts.len() < 2 || clearance <= 0.0 {
		return pts.to_vec();
	}
	let mut acc = 0.0;
	let mut i = pts.len() - 1;
	while i > 0 {
		let d = polyline_dist(pts[i], pts[i - 1]);
		if acc + d >= clearance {
			let t = ((clearance - acc) / d.max(1e-12)).clamp(0.0, 1.0);
			let p = polyline_lerp(pts[i], pts[i - 1], t);
			let mut out = pts[..i].to_vec();
			out.push(p);
			return out;
		}
		acc += d;
		i -= 1;
	}
	vec![pts[0]]
}

pub fn shorten_polyline_start(pts: &[[f64; 2]], clearance: f64) -> Vec<[f64; 2]> {
	let mut rev = pts.to_vec();
	rev.reverse();
	let mut out = shorten_polyline_end(&rev, clearance);
	out.reverse();
	out
}

pub fn hitch_clearance(hitch_r: f64) -> f64 {
	hitch_r
}

/// Tight zigzag that traces `centerline`, optionally shifted alongside it.
pub fn zigzag_alongside(
	centerline: &[[f64; 2]],
	side_offset: f64,
	amplitude: f64,
	teeth: f64,
) -> Vec<[f64; 2]> {
	let mut out = Vec::with_capacity(centerline.len());
	let n = centerline.len();
	for i in 0..n {
		let t = if n <= 1 { 0.0 } else { i as f64 / (n - 1) as f64 };
		let wave = triangle_wave(t * teeth);
		let (nx, ny) = polyline_normal(centerline, i);
		let p = centerline[i];
		let off = side_offset + amplitude * wave;
		out.push([p[0] + nx * off, p[1] + ny * off]);
	}
	out
}

pub fn zigzag_polyline(centerline: &[[f64; 2]], amplitude: f64, teeth: f64) -> Vec<[f64; 2]> {
	zigzag_alongside(centerline, 0.0, amplitude, teeth)
}

/// One thread of the old conflict-style braid weave.
#[derive(Clone, Debug)]
pub struct BraidThread {
	pub pts: Vec<[f64; 2]>,
	/// `2π · turns · t + slot_offset`. Over/under uses `phase.cos() >= 0`.
	pub phase: Vec<f64>,
	pub color: TileColor,
}

pub fn braid_strand_count(colors: [Option<TileColor>; 3]) -> usize {
	colors.iter().filter(|c| c.is_some()).count()
}

/// Closed circle used as a braid centerline (last point repeats the first).
pub fn circle_polyline(center: [f64; 2], radius: f64, samples: u32) -> Vec<[f64; 2]> {
	let n = samples.max(8);
	let mut pts = Vec::with_capacity(n as usize + 1);
	for i in 0..=n {
		let ang = f64::from(i) / f64::from(n) * 2.0 * PI;
		pts.push([center[0] + radius * ang.cos(), center[1] + radius * ang.sin()]);
	}
	pts
}

/// Old conflict braid: opposite sines for two slots (`0` and `π`), 120°
/// spacing for three. Each slot is one thread in that slot's color.
pub fn braid_threads(
	centerline: &[[f64; 2]],
	amplitude: f64,
	turns: f64,
	colors: [Option<TileColor>; 3],
) -> Vec<BraidThread> {
	let n_slots = braid_strand_count(colors).max(1) as f64;
	let n = centerline.len();
	let mut out = Vec::new();
	for (i, color) in colors.iter().enumerate() {
		let Some(color) = *color else {
			continue;
		};
		let offset = 2.0 * PI * (i as f64) / n_slots;
		let mut pts = Vec::with_capacity(n);
		let mut phase = Vec::with_capacity(n);
		for j in 0..n {
			let t = if n <= 1 { 0.0 } else { j as f64 / (n - 1) as f64 };
			let ph = 2.0 * PI * turns * t + offset;
			let (nx, ny) = polyline_normal(centerline, j);
			let p = centerline[j];
			let off = amplitude * ph.sin();
			pts.push([p[0] + nx * off, p[1] + ny * off]);
			phase.push(ph);
		}
		out.push(BraidThread { pts, phase, color });
	}
	out
}

/// Split a thread into over/under slices so the weave stacks in z-order.
/// `true` = this slice sits in front ([`Z_ARC_OVER`]).
pub fn for_braid_segments(
	pts: &[[f64; 2]],
	phase: &[f64],
	mut emit: impl FnMut(&[[f64; 2]], bool),
) {
	if pts.len() < 2 || phase.len() != pts.len() {
		return;
	}
	let mut start = 0usize;
	for i in 1..pts.len() {
		let this_front = phase[i].cos() >= 0.0;
		let prev_front = phase[i - 1].cos() >= 0.0;
		if this_front != prev_front || i + 1 == pts.len() {
			if i > start {
				emit(&pts[start..=i], prev_front);
			}
			start = i.saturating_sub(1);
		}
	}
}

pub fn zigzag_circle(
	center: [f64; 2],
	radius: f64,
	amplitude: f64,
	teeth: u32,
	samples: u32,
) -> Vec<[f64; 2]> {
	let n = samples.max(8);
	let mut pts = Vec::with_capacity(n as usize + 1);
	for i in 0..=n {
		let t = f64::from(i) / f64::from(n);
		let ang = t * 2.0 * PI;
		let wave = triangle_wave(t * f64::from(teeth));
		let rr = radius + amplitude * wave;
		pts.push([center[0] + rr * ang.cos(), center[1] + rr * ang.sin()]);
	}
	pts
}

fn triangle_wave(t: f64) -> f64 {
	let phase = t.rem_euclid(1.0);
	if phase < 0.5 {
		4.0 * phase - 1.0
	} else {
		3.0 - 4.0 * phase
	}
}

pub fn arc_length(arc: &ArcEdge) -> f64 {
	let mut sweep = arc.angle_to - arc.angle_from;
	if sweep <= 0.0 {
		sweep += 2.0 * PI;
	}
	arc.radius * sweep
}

/// Spellcheck-underline teeth: twice the previous frequency, still low amp.
pub fn zag_teeth(arc: &ArcEdge) -> f64 {
	(arc_length(arc) / ZAG_TOOTH_UNITS).clamp(7.0, 16.0)
}

/// How many twists the old conflict braid makes along `arc`.
pub fn braid_turns(arc: &ArcEdge) -> f64 {
	(arc_length(arc) / BRAID_TURN_UNITS).clamp(BRAID_TURNS_MIN, BRAID_TURNS_MAX)
}

/// Twist count along a pixel-space path (hitch halves), using the original
/// conflict-weave board-unit period. Short paths keep that wavelength instead
/// of packing a full-arc twist count into the diamond.
pub fn braid_turns_along(pts: &[[f64; 2]], scale: f32) -> f64 {
	let len_board = polyline_length(pts) / f64::from(scale).max(1e-6);
	(len_board / BRAID_TURN_UNITS).clamp(BRAID_TURNS_MIN, BRAID_TURNS_MAX)
}

/// Weave amplitude on a hitch half: stay near the diamond so leftover
/// highway stripes can pack outside the threads.
pub fn hitch_braid_amp(full_amp: f64, hitch_r: f64) -> f64 {
	let cap = diamond_across(hitch_r) * 1.2;
	if cap <= 0.0 {
		full_amp
	} else {
		full_amp.min(cap).max(1.0)
	}
}

/// Integer twists around a tile rim so a closed weave meets itself.
/// ~2× the arc conflict frequency (`TILE_BRAID_TURN_UNITS`), not
/// [`TILE_ZAG_TEETH`] and not the slower arc clamp.
pub fn tile_braid_turns(radius: f64) -> f64 {
	((2.0 * PI * radius) / (TILE_BRAID_TURN_UNITS * f64::from(PREV_SCALE)))
		.clamp(4.0, 6.0)
		.round()
}

pub fn zag_lane_spacing(stripe_off: f64) -> f64 {
	stripe_off * ZAG_LANE_SPACING_SCALE
}

/// Distance from the arc path to the first zag lane. Lane 0 is skipped so a
/// Hidden body keeps a visible middle gap (3 | gap | 3), and a match/highway/
/// diamond keeps the connection line in the center. `hitch_r` pushes zags
/// outside a conflict diamond so they don't run through the glyph.
pub fn zag_lane_inner(body: ArcBody, stripe_off: f64, hitch_r: f64) -> f64 {
	let spacing = zag_lane_spacing(stripe_off);
	match body {
		ArcBody::Hitch { .. } => hitch_r.max(stripe_off) + spacing,
		ArcBody::Stripes { .. } | ArcBody::Solid { .. } | ArcBody::Braid { .. } => {
			stripe_off + spacing
		}
		ArcBody::Hidden | ArcBody::Neutral => spacing,
	}
}

/// Fan zag `index` (0 = nearest the middle) to `sign` (+1 from, −1 to).
pub fn zag_fan_offset(index: usize, sign: f64, inner: f64, spacing: f64) -> f64 {
	sign * (inner + index as f64 * spacing)
}

/// Offsets for `n` zag strokes bundled around the arc center line.
/// One stroke sits on the path; two or more straddle it symmetrically.
pub fn zag_center_offsets(n: usize, spacing: f64) -> [f64; MAX_EDGE_ZAGS] {
	let mut out = [0.0; MAX_EDGE_ZAGS];
	if n == 0 {
		return out;
	}
	let count = n.min(MAX_EDGE_ZAGS);
	let start = -((count as f64 - 1.0) * spacing) / 2.0;
	for (i, slot) in out.iter_mut().enumerate().take(count) {
		*slot = start + i as f64 * spacing;
	}
	out
}

pub fn arc_touches_center(arc: &ArcEdge, center: NodeId) -> bool {
	arc.from == center || arc.to == center
}

fn polyline_normal(pts: &[[f64; 2]], i: usize) -> (f64, f64) {
	let n = pts.len();
	let prev = if i == 0 { pts[0] } else { pts[i - 1] };
	let next = if i + 1 >= n { pts[i] } else { pts[i + 1] };
	let dx = next[0] - prev[0];
	let dy = next[1] - prev[1];
	let len = (dx * dx + dy * dy).sqrt();
	if len <= 1e-12 {
		(0.0, 1.0)
	} else {
		(-dy / len, dx / len)
	}
}

pub fn min_node_distance(board: &Board) -> f64 {
	let n = board.node_count();
	let mut min = f64::INFINITY;
	for i in 0..n {
		for j in (i + 1)..n {
			let a = board.position(i as NodeId);
			let b = board.position(j as NodeId);
			let dx = a[0] - b[0];
			let dy = a[1] - b[1];
			let d = (dx * dx + dy * dy).sqrt();
			if d < min {
				min = d;
			}
		}
	}
	min
}

pub fn is_hud_safe(s: &str) -> bool {
	s.chars().all(|c| c.is_ascii() && (!c.is_ascii_control() || c == '\n'))
}

pub fn format_title(bot1: &str, bot2: &str) -> String {
	format!("Apa Kan    P1 {bot1} vs P2 {bot2}")
}

pub fn format_header(bot1: &str, bot2: &str, turn: &str) -> String {
	format!("{}{}{}", format_title(bot1, bot2), HUD_SEP, turn)
}

pub fn format_turn_line(game: &Game, actor_name: &str) -> String {
	match game.status() {
		GameStatus::InProgress => {
			let mut extra = String::new();
			if game.rules().center_hold_win {
				extra.push_str(&format!(
					"{HUD_SEP}hold P1 {}/{} P2 {}/{}",
					game.center_hold(Player::One),
					game.rules().center_hold_n,
					game.center_hold(Player::Two),
					game.rules().center_hold_n
				));
			}
			if game.rules().petal_majority_win {
				extra.push_str(&format!(
					"{HUD_SEP}petals P1 {}/{} ({}/{}) P2 {}/{} ({}/{})",
					game.petal_count(Player::One),
					game.petal_majority_need(),
					game.petal_majority(Player::One),
					game.rules().petal_majority_n,
					game.petal_count(Player::Two),
					game.petal_majority_need(),
					game.petal_majority(Player::Two),
					game.rules().petal_majority_n
				));
			}
			if game.rules().control_win {
				extra.push_str(&format!(
					"{HUD_SEP}ctl P1 {}/{} {}/{} P2 {}/{} {}/{}",
					game.control_count(Player::One),
					game.control_need(),
					game.control_hold(Player::One),
					game.rules().control_hold_turns,
					game.control_count(Player::Two),
					game.control_need(),
					game.control_hold(Player::Two),
					game.rules().control_hold_turns
				));
			}
			let hold = extra;
			format!(
				"Turn {}{HUD_SEP}{} ({}) to play{HUD_SEP}P1 light {}/{} bud {}{HUD_SEP}P2 dark {}/{} bud {}{hold}",
				game.turn_count() + 1,
				game.current_player().label(),
				actor_name,
				game.bench(Player::One).len(),
				game.rules().bag_size,
				format_budget(game, Player::One),
				game.bench(Player::Two).len(),
				game.rules().bag_size,
				format_budget(game, Player::Two)
			)
		}
		GameStatus::Won { winner, reason } => {
			format!("Game over: {} wins by {:?}", winner.label(), reason)
		}
		GameStatus::Draw { reason } => format!("Draw ({reason:?})"),
	}
}

pub fn format_log_line(log: &str, last_removals: &str) -> String {
	if log.is_empty() && last_removals.is_empty() {
		return String::new();
	}
	if last_removals.is_empty() {
		format!("Last: {log}")
	} else {
		format!("Last: {log}{HUD_SEP}{last_removals}")
	}
}

/// Constructed mid-game fixture used by screenshots. Tiles are placed directly
/// (no `apply`), so overwhelm does not clear the position. Covers gray, one-
/// color, same-color, two-ended multi-shared highways, conflict diamonds,
/// one-ended highway stripes (including a zag strand), single and double zag
/// tiles, a two-ended zag-only mismatch, a zag-on-zag edge, a grey no-effect
/// edge (non-hitting zag), and one-ended potential zags.
pub fn demo_midgame() -> Game {
	let mut game = Game::empty(RuleSet::default());
	let board = game.board().clone();
	let center = board.center();
	let r = board.construction_radius();

	let mut inner: Vec<NodeId> = Vec::new();
	let mut outer: Vec<NodeId> = Vec::new();
	for id in 0..board.node_count() as NodeId {
		if id == center || board.is_spawn(id) {
			continue;
		}
		if board.radius_of(id) < 1.1 * r {
			inner.push(id);
		} else {
			outer.push(id);
		}
	}
	inner.sort_by(|&a, &b| angle_of(board.position(a)).total_cmp(&angle_of(board.position(b))));
	outer.sort_by(|&a, &b| angle_of(board.position(a)).total_cmp(&angle_of(board.position(b))));
	let spawns = board.spawns().to_vec();

	place(&mut game, center, Player::One, TileColor::Orange, 2, 2);

	if let Some((a, b)) = empty_neighbor_pair(&game) {
		let t = game.alloc_ring_kinds(
			Player::One,
			&[(TileColor::Blue, true), (TileColor::Blue, true), (TileColor::Blue, true)],
			3,
			1,
		);
		let u = game.alloc_ring_kinds(
			Player::Two,
			&[(TileColor::Blue, true), (TileColor::Blue, true), (TileColor::Blue, true)],
			3,
			1,
		);
		game.place_tile(a, t);
		game.place_tile(b, u);
	}

	if let Some((a, b)) = empty_neighbor_pair(&game) {
		let t = game.alloc_rings(Player::One, &[TileColor::Green, TileColor::LavenderPink], 2, 1);
		let u = game.alloc_rings(Player::Two, &[TileColor::Green, TileColor::LavenderPink], 2, 1);
		game.place_tile(a, t);
		game.place_tile(b, u);
	}
	if let Some((a, b)) = empty_neighbor_pair(&game) {
		// Dedicated multi-color conflict diamond: A blue+green vs B orange+teal.
		let t = game.alloc_rings(Player::One, &[TileColor::Blue, TileColor::Green], 2, 1);
		let u = game.alloc_rings(Player::Two, &[TileColor::Orange, TileColor::Teal], 2, 1);
		game.place_tile(a, t);
		game.place_tile(b, u);
	}
	if let Some((a, b)) = empty_neighbor_pair(&game) {
		let t = game.alloc_typed_rings(
			Player::One,
			&[(TileColor::Orange, RingType::Braid), (TileColor::Green, RingType::Braid)],
			1,
			1,
		);
		let u = game.alloc_rings(Player::Two, &[TileColor::Orange, TileColor::Green], 1, 1);
		game.place_tile(a, t);
		game.place_tile(b, u);
	}
	if let Some((a, b)) = empty_neighbor_pair(&game) {
		let t = game.alloc_typed_rings(
			Player::One,
			&[(TileColor::Orange, RingType::Braid), (TileColor::Orange, RingType::Braid)],
			1,
			1,
		);
		let u = game.alloc_rings(Player::Two, &[TileColor::Blue, TileColor::Teal], 1, 1);
		game.place_tile(a, t);
		game.place_tile(b, u);
	}
	if let Some((a, b)) = empty_neighbor_pair(&game) {
		let t = game.alloc_ring_kinds(
			Player::One,
			&[(TileColor::Blue, false), (TileColor::Orange, true)],
			2,
			1,
		);
		let u = game.alloc_ring_kinds(
			Player::Two,
			&[(TileColor::Blue, false), (TileColor::Orange, true)],
			2,
			1,
		);
		game.place_tile(a, t);
		game.place_tile(b, u);
	}

	if let Some(&n) = inner.first() {
		if game.tile_at(n).is_none() {
			// Green vs center orange is a diamond; the pink zag does not hit.
			let tile = game.alloc_ring_kinds(
				Player::Two,
				&[(TileColor::Green, false), (TileColor::LavenderPink, true)],
				3,
				1,
			);
			game.place_tile(n, tile);
		}
	}
	if let Some(&n) = inner.get(1) {
		if game.tile_at(n).is_none() {
			// Orange match with the center, plus a blue zag beside it.
			let tile = game.alloc_ring_kinds(
				Player::Two,
				&[(TileColor::Orange, false), (TileColor::Blue, true)],
				2,
				2,
			);
			game.place_tile(n, tile);
		}
	}
	if let Some(&n) = inner.get(2) {
		place(&mut game, n, Player::One, TileColor::Blue, 2, 2);
	}
	if let Some(&n) = inner.get(4) {
		place(&mut game, n, Player::One, TileColor::Green, 1, 3);
	}
	if let Some(&n) = inner.get(5) {
		place(&mut game, n, Player::Two, TileColor::Blue, 3, 1);
	}
	if let Some(&n) = inner.get(7) {
		if game.tile_at(n).is_none() {
			let tile = game.alloc_ring_kinds(
				Player::One,
				&[(TileColor::Orange, true), (TileColor::Orange, true)],
				2,
				1,
			);
			game.place_tile(n, tile);
		}
	}
	if let Some(n) = inner.iter().copied().find(|&id| game.tile_at(id).is_none()) {
		let tile = game.alloc_ring_kinds(
			Player::Two,
			&[(TileColor::Orange, true), (TileColor::Green, true)],
			2,
			1,
		);
		game.place_tile(n, tile);
	}

	if let Some(&n) = inner.first() {
		if let Some(spawn) = board.neighbors(n).iter().copied().find(|&k| board.is_spawn(k)) {
			place(&mut game, spawn, Player::One, TileColor::Blue, 2, 1);
		}
	}
	if let Some(&s) = spawns.get(3) {
		if game.tile_at(s).is_none() {
			place(&mut game, s, Player::Two, TileColor::Green, 2, 2);
		}
	}
	if let Some(&s) = spawns.get(6) {
		if game.tile_at(s).is_none() {
			place(&mut game, s, Player::One, TileColor::Orange, 3, 1);
		}
	}

	if let Some(&n) = outer.get(1) {
		place(&mut game, n, Player::Two, TileColor::Blue, 1, 2);
	}
	if let Some(&n) = outer.get(2) {
		place(&mut game, n, Player::One, TileColor::Green, 2, 1);
	}
	if let Some(&n) = outer.get(5) {
		place(&mut game, n, Player::Two, TileColor::Orange, 2, 3);
	}
	if let Some(n) = outer.iter().copied().find(|&id| game.tile_at(id).is_none()) {
		// One-ended highway with a zag strand (undecided: only this end occupied).
		let tile = game.alloc_ring_kinds(
			Player::One,
			&[(TileColor::Green, false), (TileColor::Orange, true)],
			2,
			1,
		);
		game.place_tile(n, tile);
	}
	if let Some(n) = outer.iter().copied().find(|&id| game.tile_at(id).is_none()) {
		// One-ended triple zag: three parallel squiggles, no match body.
		let tile = game.alloc_ring_kinds(
			Player::Two,
			&[(TileColor::Blue, true), (TileColor::Blue, true), (TileColor::Blue, true)],
			3,
			1,
		);
		game.place_tile(n, tile);
	}

	{
		let is_blue_only = |t: Tile| {
			t.has_normal(TileColor::Blue) && t.n_rings == 1 && t.zag().iter().all(|z| !*z)
		};
		let mut extra = 0u8;
		for id in 0..board.node_count() as NodeId {
			let Some(t) = game.tile_at(id) else {
				continue;
			};
			if !is_blue_only(t) {
				continue;
			}
			let Some(&nb) = board.neighbors(id).iter().find(|&&k| game.tile_at(k).is_none()) else {
				continue;
			};
			if extra == 0 {
				let zag = game.alloc_ring_kinds(Player::One, &[(TileColor::Orange, true)], 1, 1);
				game.place_tile(nb, zag);
				extra = 1;
			} else {
				let mixed = game.alloc_ring_kinds(
					Player::One,
					&[(TileColor::Blue, false), (TileColor::Orange, true)],
					2,
					1,
				);
				game.place_tile(nb, mixed);
				break;
			}
		}
	}

	let p1_bench = vec![
		game.alloc_ring_kinds(
			Player::One,
			&[(TileColor::Orange, true), (TileColor::Orange, true)],
			2,
			1,
		),
		game.alloc_ring_kinds(Player::One, &[(TileColor::Orange, true)], 1, 1),
		game.alloc_tile(Player::One, TileColor::Blue, 1, 3),
		game.alloc_rings(Player::One, &[TileColor::Green, TileColor::LavenderPink], 2, 1),
		game.alloc_tile(Player::One, TileColor::Orange, 1, 2),
	];
	let p2_bench = vec![
		game.alloc_tile(Player::Two, TileColor::Orange, 1, 2),
		game.alloc_tile(Player::Two, TileColor::Green, 2, 1),
		game.alloc_tile(Player::Two, TileColor::Blue, 3, 2),
		game.alloc_tile(Player::Two, TileColor::Orange, 2, 3),
		game.alloc_tile(Player::Two, TileColor::Green, 1, 1),
	];
	game.set_bench(Player::One, p1_bench);
	game.set_bench(Player::Two, p2_bench);
	let p1_cap = game.alloc_tile(Player::Two, TileColor::Orange, 2, 1);
	let p1_cap2 = game.alloc_rings(Player::Two, &[TileColor::Blue, TileColor::Green], 2, 2);
	let mut p1_tokens = game.budget_tokens(Player::One).to_vec();
	p1_tokens.push(BudgetToken::Captured(p1_cap));
	p1_tokens.push(BudgetToken::Captured(p1_cap2));
	game.set_budget_tokens(Player::One, p1_tokens);
	let p2_cap = game.alloc_tile(Player::One, TileColor::Green, 1, 3);
	let mut p2_tokens = game.budget_tokens(Player::Two).to_vec();
	p2_tokens.push(BudgetToken::Captured(p2_cap));
	game.set_budget_tokens(Player::Two, p2_tokens);
	game.remember_full_decks();
	game.set_current(Player::One);
	game
}

/// Two neighboring tiles only, for close-up screenshots of a single edge.
pub fn demo_pair(
	a_kinds: &[(TileColor, bool)],
	a_weight: u8,
	b_kinds: &[(TileColor, bool)],
	b_weight: u8,
) -> Game {
	demo_pair_with(RuleSet::default(), a_kinds, a_weight, b_kinds, b_weight)
}

pub fn demo_pair_with(
	rules: RuleSet,
	a_kinds: &[(TileColor, bool)],
	a_weight: u8,
	b_kinds: &[(TileColor, bool)],
	b_weight: u8,
) -> Game {
	let mut game = Game::empty(rules);
	if let Some((a, b)) = empty_neighbor_pair(&game) {
		let t = game.alloc_ring_kinds(Player::One, a_kinds, a_weight, 1);
		let u = game.alloc_ring_kinds(Player::Two, b_kinds, b_weight, 1);
		game.place_tile(a, t);
		game.place_tile(b, u);
	}
	game
}

pub fn demo_typed_pair(
	a_parts: &[(TileColor, RingType)],
	a_weight: u8,
	b_parts: &[(TileColor, RingType)],
	b_weight: u8,
) -> Game {
	let mut game = Game::empty(RuleSet::default());
	if let Some((a, b)) = empty_neighbor_pair(&game) {
		let t = game.alloc_typed_rings(Player::One, a_parts, a_weight, 1);
		let u = game.alloc_typed_rings(Player::Two, b_parts, b_weight, 1);
		game.place_tile(a, t);
		game.place_tile(b, u);
	}
	game
}

/// One occupied node with a braid, for one-ended would-be-braid close-ups.
pub fn demo_typed_tile(parts: &[(TileColor, RingType)], weight: u8) -> Game {
	let mut game = Game::empty(RuleSet::default());
	let board = game.board().clone();
	let center = board.center();
	let id = (0..board.node_count() as NodeId).find(|&n| n != center && !board.is_spawn(n));
	if let Some(id) = id {
		let tile = game.alloc_typed_rings(Player::One, parts, weight, 1);
		game.place_tile(id, tile);
	}
	game
}

/// One occupied node with concentric zag rings, for stroke-weight close-ups.
pub fn demo_zag_tile() -> Game {
	let mut game = Game::empty(RuleSet::default());
	let board = game.board().clone();
	let center = board.center();
	let id = (0..board.node_count() as NodeId).find(|&n| n != center && !board.is_spawn(n));
	if let Some(id) = id {
		let tile = game.alloc_ring_kinds(
			Player::One,
			&[(TileColor::Blue, true), (TileColor::Orange, true), (TileColor::Green, true)],
			3,
			1,
		);
		game.place_tile(id, tile);
	}
	game
}

fn ring_nodes(board: &Board, inner_band: bool) -> Vec<NodeId> {
	let center = board.center();
	let r = board.construction_radius();
	let mut nodes: Vec<NodeId> = (0..board.node_count() as NodeId)
		.filter(|&id| {
			if id == center || board.is_spawn(id) {
				return false;
			}
			let inner = board.radius_of(id) < 1.1 * r;
			if inner_band {
				inner
			} else {
				!inner
			}
		})
		.collect();
	nodes.sort_by(|&a, &b| angle_of(board.position(a)).total_cmp(&angle_of(board.position(b))));
	nodes
}

fn closest_angle(board: &Board, nodes: &[NodeId], target: f64) -> Option<NodeId> {
	nodes.iter().copied().min_by(|&a, &b| {
		angle_dist(angle_of(board.position(a)), target)
			.total_cmp(&angle_dist(angle_of(board.position(b)), target))
	})
}

fn angle_dist(a: f64, b: f64) -> f64 {
	let mut d = (a - b).abs();
	if d > PI {
		d = 2.0 * PI - d;
	}
	d
}

/// Left-side orange/green/blue cluster from the board screenshot: a green zag
/// against an orange holder, a blue-zag tile with no realized effect, and a
/// one-ended potential-effects edge.
pub fn demo_left_cluster() -> Game {
	let mut game = Game::empty(RuleSet::default());
	let board = game.board().clone();
	let center = board.center();
	let inner = ring_nodes(&board, true);
	let outer = ring_nodes(&board, false);
	let spawns = board.spawns().to_vec();
	let west_spawn = closest_angle(&board, &spawns, PI).unwrap();
	let west_inner = closest_angle(&board, &inner, PI).unwrap();
	let sw_outer = closest_angle(&board, &outer, PI + PI / 4.0).unwrap();
	let sw_inner = closest_angle(&board, &inner, PI + PI / 4.0).unwrap();
	let south_inner = closest_angle(&board, &inner, 3.0 * PI / 2.0).unwrap();
	let sse_inner = closest_angle(&board, &inner, 1.75 * PI).unwrap();
	let nw_inner = closest_angle(&board, &inner, 3.0 * PI / 4.0).unwrap();

	let cream_zag = game.alloc_ring_kinds(
		Player::One,
		&[(TileColor::Orange, false), (TileColor::Green, true)],
		2,
		3,
	);
	game.place_tile(west_spawn, cream_zag);
	let dark_holder = game.alloc_rings(Player::Two, &[TileColor::Orange, TileColor::Green], 2, 1);
	game.place_tile(west_inner, dark_holder);
	let blue_zags = game.alloc_ring_kinds(
		Player::Two,
		&[(TileColor::Blue, true), (TileColor::Blue, true), (TileColor::Blue, true)],
		3,
		3,
	);
	game.place_tile(sw_inner, blue_zags);
	let cream_og = game.alloc_rings(Player::One, &[TileColor::Orange, TileColor::Green], 2, 1);
	game.place_tile(sw_outer, cream_og);
	let green_zag = game.alloc_ring_kinds(
		Player::Two,
		&[(TileColor::Green, true), (TileColor::Green, true), (TileColor::Green, true)],
		3,
		3,
	);
	game.place_tile(south_inner, green_zag);
	place(&mut game, sse_inner, Player::One, TileColor::Orange, 2, 1);
	place(&mut game, center, Player::One, TileColor::Blue, 2, 2);
	place(&mut game, nw_inner, Player::Two, TileColor::Blue, 2, 2);
	if let Some((a, b)) = empty_neighbor_pair(&game) {
		let zag = game.alloc_ring_kinds(Player::Two, &[(TileColor::Blue, true)], 1, 1);
		let orange = game.alloc_rings(Player::One, &[TileColor::Orange], 1, 1);
		game.place_tile(a, zag);
		game.place_tile(b, orange);
	}
	game
}

/// d=1 hop that turns at the center: green mover on a spawn, hoppable green
/// tiles on the inner neighbor and the center.
pub fn demo_hop_turn() -> (Game, NodeId) {
	let mut rules = RuleSet::default();
	rules.counted_pass_through = false;
	let mut game = Game::empty(rules);
	let board = game.board().clone();
	let center = board.center();
	let spawn = board.spawns()[0];
	let inner = *board
		.neighbors(spawn)
		.iter()
		.find(|&&n| board.neighbors(n).contains(&center))
		.expect("spawn neighbors an inner that touches center");
	place(&mut game, spawn, Player::One, TileColor::Green, 3, 1);
	place(&mut game, inner, Player::Two, TileColor::Green, 1, 1);
	place(&mut game, center, Player::Two, TileColor::Green, 1, 1);
	game.set_current(Player::One);
	(game, spawn)
}

fn place(game: &mut Game, node: NodeId, owner: Player, color: TileColor, weight: u8, dots: u8) {
	if game.tile_at(node).is_some() {
		return;
	}
	let tile = game.alloc_tile(owner, color, weight, dots);
	game.place_tile(node, tile);
}

fn empty_neighbor_pair(game: &Game) -> Option<(NodeId, NodeId)> {
	let board = game.board();
	let center = board.center();
	for id in 0..board.node_count() as NodeId {
		if game.tile_at(id).is_some() || board.is_spawn(id) || id == center {
			continue;
		}
		for &nb in board.neighbors(id) {
			if game.tile_at(nb).is_none() && !board.is_spawn(nb) && nb != center {
				return Some((id, nb));
			}
		}
	}
	None
}

fn angle_of(p: [f64; 2]) -> f64 {
	let a = p[1].atan2(p[0]);
	if a < 0.0 {
		a + 2.0 * PI
	} else {
		a
	}
}

#[cfg(test)]
mod tests {
	#![allow(clippy::unwrap_used, clippy::expect_used)]

	use super::*;
	use crate::game::DeckSlot;

	#[test]
	fn tiles_are_scaled_from_the_doubled_radius() {
		let prev = (PREV_TILE_RADIUS_PX / PREV_SCALE) * 2.0;
		assert!((TILE_RADIUS_UNITS - prev * TILE_SIZE_SCALE).abs() < 1e-6);
		assert!((TILE_SIZE_SCALE - 1.0).abs() < 1e-6);
	}

	#[test]
	fn larger_tiles_still_read_at_closest_nodes() {
		let board = Board::rosette_8();
		let min_d = min_node_distance(&board);
		// Adjacent inner-ring nodes are the tightest pair (~0.5858).
		assert!((min_d - 0.5858).abs() < 0.01, "closest node spacing changed: {min_d}");
		let diameter = f64::from(TILE_RADIUS_UNITS) * 2.0;
		assert!(diameter < min_d, "tile diameter {diameter} collides at min spacing {min_d}");
		let gap = min_d - diameter;
		assert!(gap > 0.12, "expected a comfortable gap, got {gap} (min_d={min_d})");
	}

	#[test]
	fn tile_zag_rings_keep_concentric_gap() {
		let d = TileRingDraw::for_radius(33.0);
		assert!(
			2.0 * d.zag_amp + d.zag_stroke < d.gap,
			"zag amp {} + stroke {} must fit in ring gap {}",
			d.zag_amp,
			d.zag_stroke,
			d.gap
		);
		assert_eq!(d.zag_teeth, 16, "zag rings should double the underline frequency");
		assert!(d.zag_stroke * 2.0 < d.stroke, "zag rings should be noticeably finer than normals");
		assert!(
			d.gap - d.zag_stroke > d.zag_stroke,
			"air between concentric zags should stay wider than the stroke (stroke={} gap={})",
			d.zag_stroke,
			d.gap
		);
		assert!(
			TILE_RING_GAP <= 3.6 && TILE_RING_INSET <= 2.0,
			"rings should hug the rim (gap={}, inset={})",
			TILE_RING_GAP,
			TILE_RING_INSET
		);
	}

	#[test]
	fn zag_on_zag_is_decided_zag_only_not_a_match() {
		let rings = [(TileColor::Blue, true), (TileColor::Blue, true), (TileColor::Blue, true)];
		let a = Tile::with_ring_kinds(1, Player::One, &rings, 3, 1);
		let b = Tile::with_ring_kinds(2, Player::Two, &rings, 3, 1);
		assert!(shared_normal_colors(a, b)[0].is_none());
		let paint = arc_paint(Some(a), Some(b));
		assert_eq!(paint.body, ArcBody::Hidden, "zag-on-zag must not draw a match stroke");
		assert_eq!(paint.zag_from_count(), 0, "SUM inks the center, not the sides");
		assert_eq!(paint.zag_to_count(), 0);
		assert_eq!(paint.zag_mid_count(), 6, "SUM: three hitting strokes per tile");
		assert!(paint.zags_mid.iter().all(|c| *c == Some(TileColor::Blue)));
		let one = Tile::with_ring_kinds(3, Player::One, &[(TileColor::Blue, true)], 1, 1);
		let paint = arc_paint(Some(one), Some(one));
		assert_eq!(paint.body, ArcBody::Hidden);
		assert_eq!(paint.zag_mid_count(), 2, "A orange-vs-orange analogue: two blue zags");
		assert_eq!(paint.zags_mid[0], Some(TileColor::Blue));
		assert_eq!(paint.zags_mid[1], Some(TileColor::Blue));
		let max = arc_paint_with(Some(a), Some(b), ZagMismatchCount::Max);
		assert_eq!(max.body, ArcBody::Hidden);
		assert_eq!(max.zag_from_count(), 0);
		assert_eq!(max.zag_to_count(), 0);
		assert_eq!(max.zag_mid_count(), 3, "MAX: 3 blue vs 3 blue is three squiggles");
		assert!(max.zags_mid.iter().take(3).all(|c| *c == Some(TileColor::Blue)));
		let one_max = arc_paint_with(Some(one), Some(one), ZagMismatchCount::Max);
		assert_eq!(one_max.zag_mid_count(), 1, "MAX orange-vs-orange analogue: one squiggle");
		assert_eq!(one_max.zags_mid[0], Some(TileColor::Blue));
	}

	#[test]
	fn each_zag_stroke_is_its_own_arc_line() {
		let three = Tile::with_ring_kinds(
			1,
			Player::One,
			&[(TileColor::Blue, true), (TileColor::Blue, true), (TileColor::Blue, true)],
			3,
			1,
		);
		let two = Tile::with_ring_kinds(
			2,
			Player::One,
			&[(TileColor::Orange, true), (TileColor::Orange, true)],
			2,
			1,
		);
		let one_ended_three = arc_paint(Some(three), None);
		assert_eq!(one_ended_three.body, ArcBody::Hidden);
		assert_eq!(one_ended_three.zag_from_count(), 3);
		assert_eq!(one_ended_three.zag_to_count(), 0);
		let one_ended_to = arc_paint(None, Some(three));
		assert_eq!(one_ended_to.zag_from_count(), 0);
		assert_eq!(one_ended_to.zag_to_count(), 3);
		let one_ended_two = arc_paint(Some(two), None);
		assert_eq!(one_ended_two.body, ArcBody::Hidden);
		assert_eq!(one_ended_two.zag_from_count(), 2);
		let two_ended_two = arc_paint(Some(two), Some(two));
		assert_eq!(two_ended_two.body, ArcBody::Hidden);
		assert_eq!(two_ended_two.zag_from_count(), 0);
		assert_eq!(two_ended_two.zag_to_count(), 0);
		assert_eq!(two_ended_two.zag_mid_count(), 4, "SUM bundles 2+2 on the center line");
		let two_vs_one = Tile::with_ring_kinds(4, Player::Two, &[(TileColor::Orange, true)], 1, 1);
		let sum_2v1 = arc_paint(Some(two), Some(two_vs_one));
		assert_eq!(sum_2v1.zag_mid_count(), 3, "SUM: 2 orange vs 1 orange = 3");
		let max_2v1 = arc_paint_with(Some(two), Some(two_vs_one), ZagMismatchCount::Max);
		assert_eq!(max_2v1.zag_mid_count(), 2, "MAX: 2 orange vs 1 orange = 2");
		assert!(max_2v1.zags_mid.iter().take(2).all(|c| *c == Some(TileColor::Orange)));
		let mixed = Tile::with_ring_kinds(
			3,
			Player::One,
			&[(TileColor::Green, false), (TileColor::Orange, true), (TileColor::Orange, true)],
			2,
			1,
		);
		let paint = arc_paint(Some(mixed), None);
		assert_eq!(paint.body, ArcBody::Solid { color: TileColor::Green, thick: false });
		assert_eq!(paint.zag_from_count(), 2);
		assert_eq!(paint.zags_from[0], Some(TileColor::Orange));
		assert_eq!(paint.zags_from[1], Some(TileColor::Orange));
		assert_eq!(paint.zag_to_count(), 0);
	}

	#[test]
	fn zag_lanes_fan_outward_from_an_empty_middle() {
		let off = 4.0;
		let spacing = zag_lane_spacing(off);
		let inner = zag_lane_inner(ArcBody::Hidden, off, 0.0);
		let a0 = zag_fan_offset(0, 1.0, inner, spacing);
		let a1 = zag_fan_offset(1, 1.0, inner, spacing);
		let a2 = zag_fan_offset(2, 1.0, inner, spacing);
		let b0 = zag_fan_offset(0, -1.0, inner, spacing);
		assert!(a0 > 0.0 && b0 < 0.0, "A and B occupy opposite sides");
		assert!((a0 + b0).abs() < 1e-9, "innermost lanes are symmetric");
		assert!(
			(a0 - b0).abs() >= 2.0 * spacing - 1e-9,
			"middle gap between A and B must stay empty, gap={}",
			a0 - b0
		);
		assert!(a1 > a0 && a2 > a1, "A's zags fan outward");
		let match_inner =
			zag_lane_inner(ArcBody::Solid { color: TileColor::Blue, thick: true }, off, 0.0);
		assert!(match_inner > inner + 1e-9, "zags sit outside a center match stroke");
	}

	#[test]
	fn zag_center_offsets_bundle_around_the_path() {
		let spacing = 4.0;
		let one = zag_center_offsets(1, spacing);
		assert!((one[0]).abs() < 1e-12, "a single zag sits on the center line");
		let two = zag_center_offsets(2, spacing);
		assert!((two[0] + two[1]).abs() < 1e-12, "two zags straddle the center");
		assert!((two[1] - two[0] - spacing).abs() < 1e-12);
		let three = zag_center_offsets(3, spacing);
		assert!((three[1]).abs() < 1e-12, "odd counts keep a center stroke");
		assert!((three[2] - three[0] - 2.0 * spacing).abs() < 1e-12);
	}

	#[test]
	fn parallel_zag_strokes_are_finer_than_the_lane_gap() {
		let layout = ViewLayout::from_window(1280.0, 800.0);
		let zag_w = f64::from(layout.arc_width) * f64::from(ZAG_STRAND_WIDTH_SCALE);
		let spacing = f64::from(layout.stripe_off) * ZAG_LANE_SPACING_SCALE;
		assert!(
			spacing > zag_w * 2.5,
			"parallel zags must show empty space (width={zag_w} spacing={spacing})"
		);
		assert!(
			(ZAG_STRAND_WIDTH_SCALE - 0.33).abs() < 1e-6,
			"arc zag stroke should be 1.5× the previous 0.22 scale"
		);
		assert!(
			ZAG_STRAND_WIDTH_SCALE < STRIPE_STRAND_WIDTH_SCALE,
			"arc zag stroke should stay finer than a highway strand"
		);
		let span = 6.0 * spacing;
		assert!(
			span < f64::from(layout.tile_radius) * 1.2,
			"3|gap|3 bundle should still fit beside a tile (span={span} r={})",
			layout.tile_radius
		);
		assert!((TILE_ZAG_AMP - 0.80).abs() < 1e-6, "keep low tile-zag amplitude");
		assert_eq!(TILE_ZAG_TEETH, 16, "keep 2x squiggle frequency");
		assert!((ZAG_TOOTH_UNITS - 0.12).abs() < 1e-12, "zag tooth period is unchanged");
		assert!((BRAID_TURN_UNITS - 0.42).abs() < 1e-12, "braid uses the original conflict period");
		assert!(
			(BRAID_TURN_UNITS - ZAG_TOOTH_UNITS).abs() > 1e-6,
			"braid weave must not share the zag tooth period"
		);
		assert!((TILE_BRAID_TURN_UNITS - BRAID_TURN_UNITS * 0.5).abs() < 1e-12);
		assert!(
			(TILE_BRAID_STROKE - 0.70).abs() < 1e-6,
			"tile-rim braid threads stay thinner than a zag ring"
		);
		assert!(TILE_BRAID_STROKE < TILE_ZAG_STROKE);
		assert_eq!(
			tile_braid_turns(22.0),
			((2.0 * PI * 22.0) / (TILE_BRAID_TURN_UNITS * f64::from(PREV_SCALE)))
				.clamp(4.0, 6.0)
				.round(),
			"tile-rim braid is ~2× the arc conflict frequency, not TILE_ZAG_TEETH"
		);
		assert_eq!(tile_braid_turns(22.0), 4.0);
		assert_ne!(tile_braid_turns(22.0), f64::from(TILE_ZAG_TEETH));
	}

	#[test]
	fn layout_keeps_chrome_off_the_rosette() {
		for (w, h) in [
			(1024.0, 728.0),
			(1280.0, 800.0),
			(1920.0, 1080.0),
			(800.0, 600.0),
			(390.0, 844.0),
			(430.0, 932.0),
			(375.0, 667.0),
		] {
			let layout = ViewLayout::from_window(w, h);
			let slack = 3.0;
			assert!(
				layout.p2_bench_bottom() + slack <= layout.hud_world_bottom(),
				"{w}x{h}: P2 bench overlaps HUD"
			);
			assert!(
				layout.board_top() + slack <= layout.p2_bench_bottom(),
				"{w}x{h}: board overlaps P2 bench (board_top={} bench_bot={})",
				layout.board_top(),
				layout.p2_bench_bottom()
			);
			assert!(
				layout.board_bottom() >= layout.p1_bench_top() + slack,
				"{w}x{h}: board overlaps P1 bench (board_bot={} bench_top={})",
				layout.board_bottom(),
				layout.p1_bench_top()
			);
			assert!(
				layout.p1_bench_y - layout.bench_tile_radius >= layout.help_world_top() + slack,
				"{w}x{h}: P1 bench overlaps help text"
			);
			assert!(layout.tile_radius > 8.0, "{w}x{h}: tiles too small ({})", layout.tile_radius);
			assert_eq!(layout.n_deck_slots, BAG_SIZE_DEFAULT);
			let n = layout.n_deck_slots;
			let half =
				layout.bench_spacing * n.saturating_sub(1) as f32 / 2.0 + layout.bench_tile_radius;
			assert!(
				half + slack <= w / 2.0 - 8.0,
				"{w}x{h}: {n}-tile deck overflows width (half={half})"
			);
			for player in [Player::One, Player::Two] {
				let bench0 = layout.bench_pos(player, 0, n);
				let stack0 = layout.budget_stack_pos(player, 0, 7);
				let stack_n = layout.budget_stack_pos(player, 6, 7);
				assert!(
					stack0[0] + layout.bench_tile_radius + slack
						<= bench0[0] - layout.bench_tile_radius,
					"{w}x{h}: budget stack overlaps P{player:?} deck (stack_x={} bench_x={})",
					stack0[0],
					bench0[0]
				);
				assert!(
					stack0[0] - layout.bench_tile_radius + slack >= -w / 2.0,
					"{w}x{h}: budget stack overflows left (x={} r={})",
					stack0[0],
					layout.bench_tile_radius
				);
				assert!(
					stack_n[0] - layout.bench_tile_radius + slack >= -w / 2.0,
					"{w}x{h}: stacked token overflows left"
				);
			}
		}
	}

	#[test]
	fn budget_stack_sits_on_the_left() {
		let layout = ViewLayout::from_window(1280.0, 800.0);
		for player in [Player::One, Player::Two] {
			let bench0 = layout.bench_pos(player, 0, layout.n_deck_slots);
			let stack = layout.budget_stack_pos(player, 0, 9);
			assert!(
				stack[0] < bench0[0],
				"stack must sit left of the bench (stack={} bench={})",
				stack[0],
				bench0[0]
			);
			assert!(
				stack[0] < 0.0,
				"stack should be on the left half of the window, got {}",
				stack[0]
			);
		}
		let p1_0 = layout.budget_stack_pos(Player::One, 0, 9);
		let p1_1 = layout.budget_stack_pos(Player::One, 1, 9);
		assert!(p1_1[1] > p1_0[1], "P1 stack grows toward the board (+Y)");
		let p2_0 = layout.budget_stack_pos(Player::Two, 0, 9);
		let p2_1 = layout.budget_stack_pos(Player::Two, 1, 9);
		assert!(p2_1[1] < p2_0[1], "P2 stack grows toward the board (-Y)");
	}

	#[test]
	fn empty_budget_slots_are_blank_faces() {
		let blank = BudgetToken::Blank;
		let captured = BudgetToken::Captured(Tile::mono(1, Player::Two, TileColor::Orange, 2, 1));
		assert!(budget_slot_is_blank_face(&blank, 0, 5), "unfilled blanks stay blank");
		assert!(budget_slot_is_blank_face(&captured, 5, 5), "unusable surplus is a blank face");
		assert!(budget_slot_is_blank_face(&captured, 6, 5), "tokens past usable are blank faces");
		assert!(
			!budget_slot_is_blank_face(&captured, 4, 5),
			"a usable captured token keeps its tile face"
		);
	}

	#[test]
	fn demo_midgame_deck_keeps_placed_tiles_in_eleven_slots() {
		let game = demo_midgame();
		for player in [Player::One, Player::Two] {
			let slots = game.deck_slots(player);
			assert_eq!(slots.len(), BAG_SIZE_DEFAULT, "{player:?} deck length");
			assert!(slots.iter().any(|s| s.is_live()), "{player:?} still has live bench tiles");
			assert!(
				slots.iter().any(|s| matches!(s, DeckSlot::Spent(_))),
				"{player:?} placed tiles must stay in the deck"
			);
			assert_eq!(slots.iter().filter(|s| s.is_live()).count(), game.bench(player).len());
		}
	}

	#[test]
	fn arc_paint_covers_gray_one_two_and_same() {
		let orange = Tile::mono(1, Player::One, TileColor::Orange, 1, 1);
		let green = Tile::mono(2, Player::One, TileColor::Green, 1, 1);
		assert_eq!(arc_paint(None, None), ArcPaint::neutral());
		assert_eq!(arc_paint(Some(orange), None), ArcPaint::solid(TileColor::Orange, false));
		assert_eq!(arc_paint(Some(orange), Some(orange)), ArcPaint::solid(TileColor::Orange, true));
		assert_eq!(
			arc_paint(Some(orange), Some(green)),
			ArcPaint::hitch(
				[Some(TileColor::Orange), None, None],
				[Some(TileColor::Green), None, None]
			)
		);
		let duo = Tile::with_rings(3, Player::One, &[TileColor::Green, TileColor::Teal], 1, 1);
		assert_eq!(arc_paint(Some(green), Some(duo)), ArcPaint::solid(TileColor::Green, true));
		let gb = Tile::with_rings(4, Player::One, &[TileColor::Green, TileColor::Blue], 1, 1);
		let paint = arc_paint(Some(gb), Some(gb));
		assert_eq!(
			paint.body,
			ArcBody::Stripes {
				a: TileColor::Green,
				b: TileColor::Blue,
				zag_a: false,
				zag_b: false
			}
		);
		assert_eq!(
			arc_paint(Some(gb), None),
			ArcPaint::stripes(TileColor::Green, TileColor::Blue, false, false)
		);
		let zag = Tile::with_ring_kinds(
			5,
			Player::One,
			&[(TileColor::Blue, false), (TileColor::Orange, true)],
			1,
			1,
		);
		let paint = arc_paint(Some(zag), Some(zag));
		assert_eq!(paint.body, ArcBody::Hidden, "orange zag hits both ways: draw only the zags");
		assert_eq!(paint.zag_from_count(), 0);
		assert_eq!(paint.zag_to_count(), 0);
		assert_eq!(paint.zag_mid_count(), 2);
		assert_eq!(paint.zags_mid[0], Some(TileColor::Orange));
		assert_eq!(paint.zags_mid[1], Some(TileColor::Orange));
		let max = arc_paint_with(Some(zag), Some(zag), ZagMismatchCount::Max);
		assert_eq!(max.zag_mid_count(), 1, "MAX orange vs orange is one squiggle");
		let orange_zag = Tile::with_ring_kinds(6, Player::One, &[(TileColor::Orange, true)], 1, 1);
		let orange_normal = Tile::mono(7, Player::Two, TileColor::Orange, 1, 1);
		let paint = arc_paint(Some(orange_zag), Some(orange_normal));
		assert_eq!(paint.body, ArcBody::Hidden);
		assert_eq!(paint.zag_mid_count(), 1, "one-sided hit still sits on the center line");
		assert_eq!(paint.zags_mid[0], Some(TileColor::Orange));
		assert_eq!(paint.zag_from_count(), 0);
		assert_eq!(paint.zag_to_count(), 0);
		let mixed = Tile::with_ring_kinds(
			8,
			Player::One,
			&[(TileColor::Green, false), (TileColor::Orange, true)],
			2,
			1,
		);
		let mixed_one = arc_paint(Some(mixed), None);
		assert_eq!(mixed_one.body, ArcBody::Solid { color: TileColor::Green, thick: false });
		assert_eq!(mixed_one.zag_from_count(), 1);
		assert_eq!(mixed_one.zags_from[0], Some(TileColor::Orange));
		assert_eq!(arc_paint(Some(orange_zag), None).body, ArcBody::Hidden);
		assert_eq!(arc_paint(Some(orange_zag), None).zags_from[0], Some(TileColor::Orange));
		let match_and_zag = Tile::with_ring_kinds(
			9,
			Player::One,
			&[(TileColor::Blue, false), (TileColor::Orange, true)],
			2,
			1,
		);
		let blue_and_orange =
			Tile::with_rings(10, Player::Two, &[TileColor::Blue, TileColor::Orange], 2, 1);
		let paint = arc_paint(Some(match_and_zag), Some(blue_and_orange));
		assert_eq!(paint.body, ArcBody::Hidden, "hitting zag suppresses the blue match line");
		assert_eq!(paint.zags_mid[0], Some(TileColor::Orange));
		assert_eq!(paint.zag_mid_count(), 1);
		assert_eq!(paint.zag_from_count(), 0);
		assert_eq!(paint.zag_to_count(), 0);
		let blue_only = Tile::mono(12, Player::Two, TileColor::Blue, 1, 1);
		let paint = arc_paint(Some(orange_zag), Some(blue_only));
		assert_eq!(paint.body, ArcBody::Neutral, "non-hitting zag is a grey no-effect edge");
		assert_eq!(paint.zag_count(), 0, "non-hitting zag is not drawn two-ended");
		let mixed_vs_blue = Tile::with_ring_kinds(
			13,
			Player::One,
			&[(TileColor::Blue, false), (TileColor::Orange, true)],
			2,
			1,
		);
		let paint = arc_paint(Some(mixed_vs_blue), Some(blue_only));
		assert_eq!(paint.body, ArcBody::Solid { color: TileColor::Blue, thick: true });
		assert_eq!(paint.zag_count(), 0, "non-hitting orange zag is omitted on a match");
		let green = Tile::mono(14, Player::Two, TileColor::Green, 1, 1);
		let paint = arc_paint(Some(mixed_vs_blue), Some(green));
		assert!(
			matches!(paint.body, ArcBody::Hitch { .. }),
			"blue vs green is still a real conflict"
		);
		assert_eq!(paint.zag_count(), 0, "non-hitting zag is omitted on a hitch");
		let bg = Tile::with_rings(15, Player::One, &[TileColor::Blue, TileColor::Green], 2, 1);
		let ot = Tile::with_rings(16, Player::Two, &[TileColor::Orange, TileColor::Teal], 2, 1);
		let paint = arc_paint(Some(bg), Some(ot));
		assert_eq!(
			paint.body,
			ArcBody::Hitch {
				from: [Some(TileColor::Blue), Some(TileColor::Green), None],
				to: [Some(TileColor::Orange), Some(TileColor::Teal), None],
				from_braid: [None; 3],
				to_braid: [None; 3],
			},
			"diamond halves list unmatched normals only"
		);
		assert_eq!(paint.zag_count(), 0);
		let mixed_hitch = Tile::with_ring_kinds(
			17,
			Player::One,
			&[(TileColor::Blue, false), (TileColor::Orange, true)],
			2,
			1,
		);
		let paint = arc_paint(Some(mixed_hitch), Some(green));
		match paint.body {
			ArcBody::Hitch { from, to, from_braid, to_braid } => {
				assert_eq!(from, [Some(TileColor::Blue), None, None]);
				assert_eq!(to, [Some(TileColor::Green), None, None]);
				assert_eq!(from_braid, [None; 3]);
				assert_eq!(to_braid, [None; 3]);
			}
			other => panic!("expected hitch, got {other:?}"),
		}
		assert_eq!(paint.zag_count(), 0);
		let orange_green_zag = Tile::with_ring_kinds(
			18,
			Player::Two,
			&[(TileColor::Orange, false), (TileColor::Green, true)],
			1,
			1,
		);
		let paint = arc_paint(Some(blue_only), Some(orange_green_zag));
		assert!(
			matches!(paint.body, ArcBody::Hitch { .. }),
			"blue vs orange is a hitch; green zag does not hit blue"
		);
		assert_eq!(paint.zag_count(), 0);
		let two_green = Tile::with_rings(
			19,
			Player::Two,
			&[TileColor::Orange, TileColor::Green, TileColor::Green],
			3,
			1,
		);
		let green_zag_only =
			Tile::with_ring_kinds(20, Player::One, &[(TileColor::Green, true)], 1, 1);
		let paint = arc_paint(Some(green_zag_only), Some(two_green));
		assert_eq!(paint.body, ArcBody::Hidden, "zag mismatch is the only realized ink");
		assert_eq!(paint.zags_mid[0], Some(TileColor::Green));
		assert_eq!(paint.zag_mid_count(), 1);
		assert_eq!(paint.zag_from_count(), 0);
		assert_eq!(paint.zag_to_count(), 0);
		let mixed = Tile::with_ring_kinds(
			21,
			Player::One,
			&[(TileColor::Blue, false), (TileColor::Orange, true)],
			2,
			1,
		);
		let other = Tile::with_ring_kinds(
			22,
			Player::Two,
			&[(TileColor::Green, false), (TileColor::Orange, false)],
			2,
			1,
		);
		let paint = arc_paint(Some(mixed), Some(other));
		assert_eq!(
			paint.body,
			ArcBody::Hidden,
			"zag + normal conflict on one edge still draws only the zag"
		);
		assert_eq!(paint.zags_mid[0], Some(TileColor::Orange));
		assert_eq!(paint.zag_mid_count(), 1);
	}

	#[test]
	fn satisfied_braid_paints_twisted_strands() {
		let braid = Tile::with_typed_rings(
			1,
			Player::One,
			&[(TileColor::Orange, RingType::Braid), (TileColor::Green, RingType::Braid)],
			1,
			1,
		);
		let cover = Tile::with_rings(2, Player::Two, &[TileColor::Orange, TileColor::Green], 1, 1);
		let paint = arc_paint(Some(braid), Some(cover));
		assert_eq!(
			paint.body,
			ArcBody::Braid { colors: [Some(TileColor::Orange), Some(TileColor::Green), None] }
		);
		assert_eq!(paint.zag_count(), 0);
		let failed = Tile::with_rings(3, Player::Two, &[TileColor::Blue], 1, 1);
		let hitch = arc_paint(Some(braid), Some(failed));
		match hitch.body {
			ArcBody::Hitch { from, to, from_braid, to_braid } => {
				assert_eq!(from, [None; 3], "braid slots are not leftover highway");
				assert_eq!(to, [Some(TileColor::Blue), None, None]);
				assert_eq!(
					from_braid,
					[Some(TileColor::Orange), Some(TileColor::Green), None],
					"unsatisfied braid still carries its threads into the diamond"
				);
				assert_eq!(to_braid, [None; 3]);
			}
			other => panic!("expected hitch, got {other:?}"),
		}
		let one_ended = arc_paint(Some(braid), None);
		assert_eq!(
			one_ended.body,
			ArcBody::Braid { colors: [Some(TileColor::Orange), Some(TileColor::Green), None] },
			"unconnected edge still shows the would-be braid weave"
		);
		let mixed = Tile::with_typed_rings(
			8,
			Player::One,
			&[
				(TileColor::Orange, RingType::Braid),
				(TileColor::Green, RingType::Braid),
				(TileColor::Blue, RingType::Normal),
			],
			1,
			1,
		);
		let teal = Tile::mono(9, Player::Two, TileColor::Teal, 1, 1);
		match arc_paint(Some(mixed), Some(teal)).body {
			ArcBody::Hitch { from, to, from_braid, to_braid } => {
				assert_eq!(from, [Some(TileColor::Blue), None, None]);
				assert_eq!(to, [Some(TileColor::Teal), None, None]);
				assert_eq!(from_braid, [Some(TileColor::Orange), Some(TileColor::Green), None]);
				assert_eq!(to_braid, [None; 3]);
			}
			other => panic!("expected mixed hitch, got {other:?}"),
		}
		let braid_zag = Tile::with_typed_rings(
			4,
			Player::One,
			&[
				(TileColor::Orange, RingType::Braid),
				(TileColor::Green, RingType::Braid),
				(TileColor::Blue, RingType::Zag),
			],
			1,
			1,
		);
		let cover3 = Tile::with_rings(
			5,
			Player::Two,
			&[TileColor::Orange, TileColor::Green, TileColor::Blue],
			1,
			1,
		);
		let both = arc_paint(Some(braid_zag), Some(cover3));
		assert!(
			matches!(both.body, ArcBody::Braid { .. }),
			"satisfied braid stays visible next to a jag"
		);
		assert_eq!(both.zag_mid_count(), 1);
		let same = Tile::with_typed_rings(
			6,
			Player::One,
			&[(TileColor::Orange, RingType::Braid), (TileColor::Orange, RingType::Braid)],
			1,
			1,
		);
		let two_orange = Tile::with_typed_rings(
			7,
			Player::Two,
			&[(TileColor::Orange, RingType::Normal), (TileColor::Orange, RingType::Normal)],
			1,
			1,
		);
		match arc_paint(Some(same), Some(two_orange)).body {
			ArcBody::Braid { colors } => {
				assert_eq!(colors, [Some(TileColor::Orange), Some(TileColor::Orange), None]);
				assert_eq!(braid_strand_count(colors), 2);
			}
			other => panic!("expected braid, got {other:?}"),
		}
	}

	#[test]
	fn braid_threads_are_one_per_slot_with_old_opposite_sines() {
		let pts = [[0.0, 0.0], [1.0, 0.0], [2.0, 0.0], [3.0, 0.0]];
		let two =
			braid_threads(&pts, 0.5, 1.0, [Some(TileColor::Orange), Some(TileColor::Green), None]);
		assert_eq!(two.len(), 2);
		assert_eq!(two[0].color, TileColor::Orange);
		assert_eq!(two[1].color, TileColor::Green);
		assert_eq!(two[0].pts.len(), 4);
		assert!(two[0].pts[1][1].abs() > 0.2);
		assert!(
			(two[0].pts[1][1] + two[1].pts[1][1]).abs() < 1e-9,
			"two-slot braid is the old +sin / −sin pair"
		);
		let three = braid_threads(
			&pts,
			0.5,
			1.0,
			[Some(TileColor::Orange), Some(TileColor::Green), Some(TileColor::Blue)],
		);
		assert_eq!(three.len(), 3);
		assert_eq!(three[2].color, TileColor::Blue);
		let same =
			braid_threads(&pts, 0.5, 1.0, [Some(TileColor::Orange), Some(TileColor::Orange), None]);
		assert_eq!(same.len(), 2);
		assert!(same.iter().all(|t| t.color == TileColor::Orange));
	}

	#[test]
	fn braid_turns_use_original_conflict_period_not_zag_teeth() {
		let arc = crate::board::ArcEdge {
			circle: 0,
			from: 0,
			to: 1,
			center: [0.0, 0.0],
			radius: 1.0,
			angle_from: 0.0,
			angle_to: 1.2,
		};
		let expected = (arc_length(&arc) / 0.42).clamp(1.2, 3.6);
		assert!((braid_turns(&arc) - expected).abs() < 1e-12);
		assert!((braid_turns(&arc) - zag_teeth(&arc)).abs() > 1.0, "braid is not zag cadence");
		let layout = ViewLayout::from_window(1280.0, 800.0);
		let pts: Vec<[f64; 2]> =
			(0..20).map(|i| [i as f64 * f64::from(layout.scale) * 0.05, 0.0]).collect();
		let turns = braid_turns_along(&pts, layout.scale);
		let expected_along =
			(polyline_length(&pts) / f64::from(layout.scale) / 0.42).clamp(1.2, 3.6);
		assert!((turns - expected_along).abs() < 1e-9);
		assert!(turns <= BRAID_TURNS_MAX);
		assert!(turns >= BRAID_TURNS_MIN);
	}

	#[test]
	fn demo_midgame_has_center_and_mixed_arcs() {
		let game = demo_midgame();
		assert!(game.tile_at(game.board().center()).is_some(), "center must have a tile");
		let mut gray = 0;
		let mut one = 0;
		let mut same = 0;
		let mut hitch = 0;
		let mut multi_color_hitch = 0;
		let mut hitch_with_zag = 0;
		let mut stripes = 0;
		let mut two_ended_stripes = 0;
		let mut stripe_with_zag = 0;
		let mut one_ended_zag_highway = 0;
		let mut one_ended_triple_zag = 0;
		let mut hidden = 0;
		let mut braid_arcs = 0;
		let mut zag_arcs = 0;
		let mut zag_only = 0;
		let mut zag_on_zag = 0;
		let mut grey_no_effect = 0;
		let mut double_zag_tiles = 0;
		let mut single_zag_tiles = 0;
		for id in 0..game.board().node_count() as NodeId {
			if let Some(tile) = game.tile_at(id) {
				let zags = (0..tile.n_rings as usize).filter(|&i| tile.ring_is_zag(i)).count();
				if zags == 1 {
					single_zag_tiles += 1;
				}
				if zags >= 2 {
					double_zag_tiles += 1;
				}
			}
		}
		for arc in game.board().arcs() {
			let from = game.tile_at(arc.from);
			let to = game.tile_at(arc.to);
			let paint = arc_paint_with(from, to, game.rules().zag_mismatch);
			if paint.zag_count() > 0 {
				zag_arcs += 1;
			}
			match (from, to) {
				(Some(_), None) | (None, Some(_)) => {
					if paint.zag_count() > 0 && !matches!(paint.body, ArcBody::Hidden) {
						one_ended_zag_highway += 1;
					}
					if paint.body == ArcBody::Hidden && paint.zag_count() >= 3 {
						one_ended_triple_zag += 1;
					}
				}
				_ => {}
			}
			if let (Some(a), Some(b)) = (from, to) {
				if paint.body == ArcBody::Hidden && paint.zag_count() > 0 {
					assert_eq!(
						paint.zag_from_count() + paint.zag_to_count(),
						0,
						"two-ended SUM zags sit on the center line"
					);
					assert!(paint.zag_mid_count() > 0);
					zag_only += 1;
				}
				if paint.body == ArcBody::Hidden
					&& a.zag_hits_on(b) > 0
					&& b.zag_hits_on(a) > 0
					&& !a.shares_normal(b)
				{
					zag_on_zag += 1;
				}
				if paint.body == ArcBody::Neutral
					&& paint.zag_count() == 0
					&& (a.zag_hits_on(b) == 0 && b.zag_hits_on(a) == 0)
					&& !a.shares_normal(b)
					&& (a.normal_mask() == 0 || b.normal_mask() == 0)
				{
					grey_no_effect += 1;
				}
				if matches!(paint.body, ArcBody::Hitch { .. }) {
					assert_eq!(paint.zag_count(), 0, "hitch never shares an edge with a zag");
				}
				if matches!(paint.body, ArcBody::Solid { .. } | ArcBody::Stripes { .. }) {
					assert_eq!(paint.zag_count(), 0, "two-ended match has no zag ink");
				}
			}
			match paint.body {
				ArcBody::Neutral => gray += 1,
				ArcBody::Solid { thick: false, .. } => one += 1,
				ArcBody::Solid { thick: true, .. } => same += 1,
				ArcBody::Hitch {
					from: cols_a,
					to: cols_b,
					from_braid: braid_a,
					to_braid: braid_b,
				} => {
					hitch += 1;
					assert!(from.is_some() && to.is_some(), "hitch is two-ended only");
					if let (Some(a), Some(b)) = (from, to) {
						assert!(
							shared_normal_colors(a, b)[0].is_none()
								&& selected_braid(a, b).is_none()
								&& a.present_toward(b) && b.present_toward(a),
							"hitch is for true color conflicts only"
						);
						assert_eq!(cols_a, unique_normals(a));
						assert_eq!(cols_b, unique_normals(b));
						assert_eq!(braid_a, hitch_braid_colors(a));
						assert_eq!(braid_b, hitch_braid_colors(b));
						if packed_color_count(cols_a) >= 2 && packed_color_count(cols_b) >= 2 {
							multi_color_hitch += 1;
						}
						if paint.zag_count() > 0 {
							hitch_with_zag += 1;
						}
					}
				}
				ArcBody::Stripes { zag_a, zag_b, .. } => {
					stripes += 1;
					match (from, to) {
						(Some(_), Some(_)) => two_ended_stripes += 1,
						(Some(_), None) | (None, Some(_)) => {}
						(None, None) => panic!("stripes on an empty-empty arc"),
					}
					if zag_a || zag_b {
						stripe_with_zag += 1;
					}
				}
				ArcBody::Hidden => hidden += 1,
				ArcBody::Braid { .. } => braid_arcs += 1,
			}
		}
		assert!(braid_arcs > 0, "need a satisfied-braid arc");
		assert!(gray > 0, "need empty gray arcs");
		assert!(one > 0, "need one-ended colored arcs");
		assert!(same > 0, "need same-color solid arcs");
		assert!(hitch > 0, "need two-ended conflicting hitches");
		assert!(multi_color_hitch > 0, "need a multi-color vs multi-color hitch");
		assert_eq!(hitch_with_zag, 0, "a hitch is not drawn beside a zag");
		assert!(stripes > 0, "need highway-stripe arcs");
		assert!(two_ended_stripes > 0, "need a two-ended multi-shared-color highway");
		assert!(
			stripe_with_zag > 0 || one_ended_zag_highway > 0,
			"need an undecided highway with a zag strand"
		);
		assert!(one_ended_triple_zag > 0, "need a one-ended triple-zag arc");
		assert!(zag_arcs > 0, "need a firing zag on an arc");
		assert!(hidden > 0, "need a decided zag-only arc (no match line)");
		assert!(zag_only > 0, "need a two-ended zag-only mismatch");
		assert!(zag_on_zag > 0, "need a zag-on-zag edge with no match stroke");
		assert!(grey_no_effect > 0, "need a two-ended grey no-effect edge");
		assert!(single_zag_tiles > 0, "need a single-zag tile");
		assert!(double_zag_tiles > 0, "need a double-zag tile");
		assert_eq!(game.bench(Player::One).len(), 5);
		assert_eq!(game.bench(Player::Two).len(), 5);
	}

	#[test]
	fn left_cluster_draws_zag_only_and_grey_no_effect() {
		let game = demo_left_cluster();
		let board = game.board();
		let mut zag_only_orange_green = 0;
		let mut grey_blue_zag = 0;
		let mut hitch = 0;
		let mut one_ended_potential = 0;
		for arc in board.arcs() {
			let from = game.tile_at(arc.from);
			let to = game.tile_at(arc.to);
			let paint = arc_paint_with(from, to, game.rules().zag_mismatch);
			match (from, to) {
				(Some(a), Some(b)) => {
					let orange_and_green_zag = |t: crate::game::Tile, other: crate::game::Tile| {
						t.has_normal(TileColor::Orange)
							&& other.has_normal(TileColor::Orange)
							&& other.has_color(TileColor::Green)
							&& t.zag_hits_on(other) > 0
					};
					if orange_and_green_zag(a, b) || orange_and_green_zag(b, a) {
						assert_eq!(
							paint.body,
							ArcBody::Hidden,
							"green zag vs orange holder must not draw the orange link"
						);
						assert!(paint.zag_count() > 0);
						zag_only_orange_green += 1;
					}
					if paint.body == ArcBody::Neutral
						&& paint.zag_count() == 0
						&& a.zag_hits_on(b) == 0
						&& b.zag_hits_on(a) == 0
						&& (a.has_color(TileColor::Blue) || b.has_color(TileColor::Blue))
					{
						grey_blue_zag += 1;
					}
					if matches!(paint.body, ArcBody::Hitch { .. }) {
						hitch += 1;
						assert_eq!(paint.zag_count(), 0);
					}
				}
				(Some(t), None) | (None, Some(t)) => {
					if paint.zag_count() > 0 {
						one_ended_potential += 1;
						assert_eq!(
							paint.zag_count(),
							tile_zag_strokes(t).iter().filter(|c| c.is_some()).count()
						);
					}
				}
				_ => {}
			}
		}
		assert!(
			zag_only_orange_green > 0,
			"left cluster must include the green-zag vs orange-holder edge"
		);
		assert!(grey_blue_zag > 0, "left cluster must include a grey no-effect blue-zag edge");
		assert!(hitch > 0, "left cluster still has a normal-vs-normal hitch");
		assert!(one_ended_potential > 0, "one-ended edges keep potential zags");
	}

	#[test]
	fn hud_strings_are_ascii() {
		let game = demo_midgame();
		let turn = format_turn_line(&game, "human");
		let title = format_title("human", "greedy");
		let header = format_header("human", "greedy", &turn);
		let log = format_log_line("P1 ADD orange w2 d2 at 0", "P2 green gone at 4 (m2 / x2)");
		assert!(is_hud_safe(&title), "{title}");
		assert!(is_hud_safe(&header), "{header}");
		assert!(is_hud_safe(&turn), "{turn}");
		assert!(is_hud_safe(&log), "{log}");
		assert!(is_hud_safe(HELP_TEXT), "{HELP_TEXT}");
		assert!(HELP_TEXT.contains("5 of 9"));
		assert!(!header.contains('·'));
		assert!(!turn.contains('·'));
		assert!(turn.len() < 140, "turn line should fit a 1024px window: {turn}");
		assert!(turn.contains("ctl"), "control-win HUD should show on default rules: {turn}");
	}

	#[test]
	fn opening_hud_uses_control_win_and_full_public_deck() {
		let game = Game::new(RuleSet::default(), 1);
		assert!(game.rules().control_win);
		assert!(!game.rules().center_hold_win);
		assert_eq!(game.bench(Player::One).len(), 11);
		assert_eq!(game.bag_len(Player::One), 0);
		let turn = format_turn_line(&game, "human");
		assert!(turn.contains("ctl P1 0/5 0/1 P2 0/5 0/1"), "{turn}");
		assert!(turn.contains("P1 light 11/11 bud 7"), "{turn}");
		assert!(turn.contains("P2 dark 11/11 bud 7"), "{turn}");
		assert!(!turn.contains("hold P1"), "{turn}");
		assert!(turn.len() < 140, "turn line should fit a 1024px window: {turn}");
	}

	#[test]
	fn opening_hud_shows_full_public_deck() {
		let mut rules = RuleSet::default();
		rules.bag_size = 27;
		let game = Game::new(rules, 1);
		assert_eq!(game.bench(Player::One).len(), 27);
		assert_eq!(game.bag_len(Player::One), 0);
		let turn = format_turn_line(&game, "human");
		assert!(turn.contains("P1 light 27/27 bud 7"), "{turn}");
		assert!(turn.contains("P2 dark 27/27 bud 7"), "{turn}");
		assert!(turn.len() < 140, "turn line should fit a 1024px window: {turn}");
	}

	#[test]
	fn hud_shows_surplus_tokens_beside_usable_budget() {
		let game = demo_midgame();
		assert_eq!(game.budget(Player::One), 5);
		assert_eq!(game.budget_surplus(Player::One), 4);
		assert_eq!(game.budget(Player::Two), 5);
		assert_eq!(game.budget_surplus(Player::Two), 3);
		let turn = format_turn_line(&game, "human");
		assert!(turn.contains("P1 light 5/11 bud 5+4"), "{turn}");
		assert!(turn.contains("P2 dark 5/11 bud 5+3"), "{turn}");
		assert!(turn.len() < 140, "turn line should fit a 1024px window: {turn}");
	}

	#[test]
	fn parallel_strands_are_constant_offset() {
		let pts = [[0.0, 0.0], [1.0, 0.0], [2.0, 0.0], [3.0, 0.0]];
		let (a, b) = parallel_strands(&pts, 0.5);
		assert_eq!(a.len(), 4);
		assert!((a[1][1] - 0.5).abs() < 1e-9);
		assert!((b[1][1] + 0.5).abs() < 1e-9);
		assert!((a[0][1] - a[2][1]).abs() < 1e-9);
	}

	#[test]
	fn hitch_glyph_is_a_compact_diamond() {
		let zag_rise = 2.0 * f64::from(ZAG_AMP_UNITS);
		let zag_run = ZAG_TOOTH_UNITS * 0.5;
		assert!(
			(DIAMOND_SLOPE - zag_rise / zag_run).abs() < 1e-12,
			"diamond slope {} != zag tooth {}",
			DIAMOND_SLOPE,
			zag_rise / zag_run
		);
		let d = conflict_diamond([0.0, 0.0], [1.0, 0.0], [0.0, 1.0], 1.0);
		assert!((d.along_from[0] + 1.0).abs() < 1e-9);
		assert!((d.along_to[0] - 1.0).abs() < 1e-9);
		assert!((d.left[1] - DIAMOND_SLOPE).abs() < 1e-9);
		assert!((d.right[1] + DIAMOND_SLOPE).abs() < 1e-9);
		assert!(d.left[1].abs() < d.along_to[0], "diamond must be longer along the arc");
		let from = diamond_from_path(&[d.along_from], &d);
		let to = diamond_to_path(&[d.along_to], &d);
		let near =
			|p: [f64; 2], q: [f64; 2]| (p[0] - q[0]).abs() < 1e-9 && (p[1] - q[1]).abs() < 1e-9;
		assert!(from.iter().copied().any(|p| near(p, d.left)), "from wrap misses left vertex");
		assert!(to.iter().copied().any(|p| near(p, d.right)), "to wrap misses right vertex");
		assert!(near(*from.last().expect("from wrap"), d.along_to), "halves must meet at along_to");
		assert!(near(to[0], d.along_from), "halves must meet at along_from");
		assert!(!from.iter().copied().any(|p| near(p, d.right)), "from wrap must stay on one side");
		assert!(
			!to.iter().copied().any(|p| near(p, d.left)),
			"to wrap must stay on the other side"
		);
		let along_r = [d.along_from, d.along_to]
			.iter()
			.map(|p| (p[0] * p[0] + p[1] * p[1]).sqrt())
			.fold(0.0, f64::max);
		let across_r = [d.left, d.right]
			.iter()
			.map(|p| (p[0] * p[0] + p[1] * p[1]).sqrt())
			.fold(0.0, f64::max);
		assert!(along_r < 1.01, "diamond escaped along radius: {along_r}");
		assert!(across_r < along_r * 0.4, "diamond should stay skinny across the arc");
		let left_off = diamond_bundle_offsets(2, 0.12, 1.0);
		let right_off = diamond_bundle_offsets(2, 0.12, -1.0);
		assert_eq!(left_off[0], 0.0, "lane 0 is the single diamond centerline");
		assert_eq!(right_off[0], 0.0, "both halves share one diamond");
		assert!(left_off[1] > 0.0 && right_off[1] < 0.0, "extra colors pack outside, not inward");
		let lane = hitch_leftover_lane(0.12, 0.05, 0.04, 0.03);
		assert!(lane > 0.05 + 0.02 + 0.015, "leftover lane clears the weave envelope");
		let beside = diamond_leftover_offsets(1, lane, 1.0);
		assert!(beside[0] > 0.0, "leftover beside a braid weave sits outside the centerline");
		let a = offset_polyline(&from, left_off[0]);
		let b = offset_polyline(&from, left_off[1]);
		assert_eq!(a.len(), b.len());
		let sep = a
			.iter()
			.zip(&b)
			.map(|(p, q)| polyline_dist(*p, *q))
			.fold(f64::INFINITY, f64::min);
		assert!(sep > 0.1, "bundle stripes should stay parallel, sep={sep}");
		let frame = midpoint_frame(&[[0.0, 0.0], [1.0, 0.0], [2.0, 0.0], [3.0, 0.0]]);
		assert_eq!(frame.index, 2);
		assert!((frame.mid[0] - 2.0).abs() < 1e-9);
		let half = shorten_polyline_end(&[[0.0, 0.0], [1.0, 0.0], [2.0, 0.0], [3.0, 0.0]], 0.5);
		assert!((half[half.len() - 1][0] - 2.5).abs() < 1e-9);
		let layout = ViewLayout::from_window(1280.0, 800.0);
		assert!(
			layout.hitch_r >= HITCH_RADIUS_MIN_PX,
			"diamond must stay readable at board scale (r={})",
			layout.hitch_r
		);
		let across = diamond_across(f64::from(layout.hitch_r));
		assert!(across < f64::from(layout.hitch_r) * 0.4, "across={across}");
		assert!(
			layout.hitch_r < 14.0,
			"along-arc half-length must stay compact (r={})",
			layout.hitch_r
		);
	}

	#[test]
	fn zigzag_alongside_stays_tight_to_the_path() {
		let pts: Vec<[f64; 2]> = (0..80).map(|i| [i as f64 * 0.05, 0.0]).collect();
		let amp = 0.04;
		let zig = zigzag_alongside(&pts, 0.0, amp, 4.0);
		assert_eq!(zig.len(), pts.len());
		let max_dev = zig.iter().map(|p| p[1].abs()).fold(0.0, f64::max);
		assert!(max_dev <= amp + 1e-9, "zag departed from the path: {max_dev} > {amp}");
		assert!(zig.iter().any(|p| p[1].abs() > amp * 0.5));
		let side = zigzag_alongside(&pts, 0.2, amp, 4.0);
		let mean_y: f64 = side.iter().map(|p| p[1]).sum::<f64>() / side.len() as f64;
		assert!(mean_y > 0.1, "alongside zag should sit beside the path, got mean {mean_y}");
		let ring = zigzag_circle([0.0, 0.0], 10.0, 0.4, 16, 96);
		assert!(ring.len() > 8);
		assert!((ring[0][0] - ring[ring.len() - 1][0]).abs() < 1e-9);
		let rs: Vec<f64> = ring.iter().map(|p| (p[0] * p[0] + p[1] * p[1]).sqrt()).collect();
		let rmin = rs.iter().copied().fold(f64::INFINITY, f64::min);
		let rmax = rs.iter().copied().fold(0.0, f64::max);
		assert!(rmax - rmin <= 0.8 + 1e-6, "tile zag amplitude too wide: {}", rmax - rmin);
	}
}
