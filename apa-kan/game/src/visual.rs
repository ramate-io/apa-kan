//! Shared board layout, palette, and arc styling for the desktop UI and PNG renderer.
//!
//! Game rules and bots do not belong here. This module only decides how the
//! already-computed position is drawn: z-order, tile size, window layout, and
//! the color of each circular-arc edge.

use std::f64::consts::PI;

use crate::board::{ArcEdge, Board, NodeId};
use crate::game::{Game, GameStatus, Player, RuleSet, Tile, TileColor};

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
/// Compact trefoil glyph at a conflict-arc midpoint. Sized to read as a
/// tangled loop at board scale, not a spawn-sized dot.
pub const KNOT_RADIUS_UNITS: f32 = 0.085;
pub const KNOT_RADIUS_CENTER_UNITS: f32 = 0.055;
pub const KNOT_RADIUS_MIN_PX: f32 = 11.0;
pub const KNOT_RADIUS_CENTER_MIN_PX: f32 = 8.0;
pub const KNOT_STRAND_WIDTH_SCALE: f32 = 1.05;
pub const KNOT_OUTLINE_WIDTH_SCALE: f32 = 1.75;
pub const KNOT_SAMPLES: usize = 64;
/// Neutral rope so unmatched stripe colors stay the story.
pub const KNOT_RGB: [f32; 3] = [0.93, 0.90, 0.80];
pub const KNOT_OUTLINE_RGB: [f32; 3] = [0.16, 0.14, 0.12];
/// Spellcheck-underline zag: low amplitude, long wavelength, thin stroke.
pub const ZAG_AMP_UNITS: f32 = 0.008;
pub const ZAG_AMP_CENTER_UNITS: f32 = 0.006;
/// 1.5× the previous 0.22 scale; still finer than a highway strand (0.70).
pub const ZAG_STRAND_WIDTH_SCALE: f32 = 0.33;
/// Parallel zag-lane spacing as a fraction of the highway stripe offset.
/// Scaled with the thicker stroke so air between lines stays countable.
pub const ZAG_LANE_SPACING_SCALE: f64 = 1.55;
/// One overlay line per zag stroke on a single tile (weight.max(n_rings) ≤ 3).
pub const MAX_TILE_ZAGS: usize = 3;
/// Packed unmatched **normal** colors on one half of a conflict knot.
pub const MAX_KNOT_COLORS: usize = 3;
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
	"Click a spawn then a bench tile to ADD, or click your tile to MOVE. Win: 5 of 9 (center+inner).";

pub const NEUTRAL_ARC_RGB: [f32; 3] = [0.38, 0.40, 0.45];
pub const BG_RGB: [f32; 3] = [0.07, 0.08, 0.10];
pub const P1_FILL_RGB: [f32; 3] = [0.91, 0.89, 0.82];
pub const P2_FILL_RGB: [f32; 3] = [0.12, 0.12, 0.14];
pub const SPAWN_RGB: [f32; 3] = [0.93, 0.84, 0.43];
pub const NODE_RGB: [f32; 3] = [0.75, 0.75, 0.75];
pub const CENTER_RGB: [f32; 3] = [0.85, 0.85, 0.85];
pub const HIGHLIGHT_RGB: [f32; 3] = [0.95, 0.85, 0.25];
pub const DEST_RGB: [f32; 3] = [0.55, 0.85, 1.00];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ArcPaint {
	pub body: ArcBody,
	/// Zag strokes **from** the arc's `from` tile, fanning out on the + side.
	pub zags_from: [Option<TileColor>; MAX_TILE_ZAGS],
	/// Zag strokes **from** the arc's `to` tile, fanning out on the − side.
	pub zags_to: [Option<TileColor>; MAX_TILE_ZAGS],
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ArcBody {
	Neutral,
	Solid {
		color: TileColor,
		thick: bool,
	},
	/// Midpoint knot joining unmatched **normal** colors. Two-ended true
	/// conflict only (both ends have a normal ring, and they share none).
	Knot {
		from: [Option<TileColor>; MAX_KNOT_COLORS],
		to: [Option<TileColor>; MAX_KNOT_COLORS],
	},
	/// Parallel highway stripes: one-ended undecided edges, or two-ended
	/// edges that share more than one normal color. Zag overlays sit outside.
	Stripes {
		a: TileColor,
		b: TileColor,
		zag_a: bool,
		zag_b: bool,
	},
	/// No connection line in the middle. Used when both ends are occupied
	/// but they share no **normal** color (zag-only contact, or empty).
	Hidden,
}

impl ArcPaint {
	fn no_zags() -> [Option<TileColor>; MAX_TILE_ZAGS] {
		[None; MAX_TILE_ZAGS]
	}

	pub fn neutral() -> Self {
		Self { body: ArcBody::Neutral, zags_from: Self::no_zags(), zags_to: Self::no_zags() }
	}

	pub fn solid(color: TileColor, thick: bool) -> Self {
		Self {
			body: ArcBody::Solid { color, thick },
			zags_from: Self::no_zags(),
			zags_to: Self::no_zags(),
		}
	}

	pub fn knot(
		from: [Option<TileColor>; MAX_KNOT_COLORS],
		to: [Option<TileColor>; MAX_KNOT_COLORS],
	) -> Self {
		Self {
			body: ArcBody::Knot { from, to },
			zags_from: Self::no_zags(),
			zags_to: Self::no_zags(),
		}
	}

	pub fn stripes(a: TileColor, b: TileColor, zag_a: bool, zag_b: bool) -> Self {
		Self {
			body: ArcBody::Stripes { a, b, zag_a, zag_b },
			zags_from: Self::no_zags(),
			zags_to: Self::no_zags(),
		}
	}

	pub fn zag_count(self) -> usize {
		self.zag_from_count() + self.zag_to_count()
	}

	pub fn zag_from_count(self) -> usize {
		self.zags_from.iter().filter(|z| z.is_some()).count()
	}

	pub fn zag_to_count(self) -> usize {
		self.zags_to.iter().filter(|z| z.is_some()).count()
	}
}

pub fn shared_normal_colors(a: Tile, b: Tile) -> [Option<TileColor>; 2] {
	let mut out = [None; 2];
	let mut n = 0usize;
	for (i, &c) in a.ring_colors().iter().enumerate() {
		if a.zag[i] || !b.has_normal(c) {
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
/// Drawn even when the neighbor does not hold that color (scoring treats
/// those zags as empty; the squiggle is still shown on this tile's side).
pub fn tile_zag_strokes(tile: Tile) -> [Option<TileColor>; MAX_TILE_ZAGS] {
	pack_zags(
		(0..tile.ring_strokes())
			.filter(|&k| tile.stroke_is_zag(k))
			.map(|k| tile.stroke_color(k)),
	)
}

/// Unique **normal** ring colors on `tile`, packed densely. Zags are omitted:
/// they already draw as side squiggles, so conflict-knot stripes only show
/// the unmatched highway options.
pub fn unique_normals(tile: Tile) -> [Option<TileColor>; MAX_KNOT_COLORS] {
	let mut out = [None; MAX_KNOT_COLORS];
	let mut n = 0usize;
	for k in 0..tile.ring_strokes() {
		if tile.stroke_is_zag(k) {
			continue;
		}
		let c = tile.stroke_color(k);
		if !out.contains(&Some(c)) && n < MAX_KNOT_COLORS {
			out[n] = Some(c);
			n += 1;
		}
	}
	out
}

pub fn packed_color_count(colors: [Option<TileColor>; MAX_KNOT_COLORS]) -> usize {
	colors.iter().filter(|c| c.is_some()).count()
}

/// Lane offsets for 1 / 2 / 3 parallel highway stripes on a knot half.
pub fn highway_offsets(n: usize, stripe_off: f64) -> [f64; MAX_KNOT_COLORS] {
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

fn two_ended_body(a: Tile, b: Tile) -> ArcBody {
	let shared = shared_normal_colors(a, b);
	match (shared[0], shared[1]) {
		(Some(color), None) => ArcBody::Solid { color, thick: true },
		(Some(c1), Some(c2)) => ArcBody::Stripes { a: c1, b: c2, zag_a: false, zag_b: false },
		(None, _) if a.normal_mask() != 0 && b.normal_mask() != 0 => {
			ArcBody::Knot { from: unique_normals(a), to: unique_normals(b) }
		}
		// No shared normal and not a true normal-vs-normal conflict: empty
		// middle (zag-only contact, or a non-matching zag). Zags still draw.
		(None, _) => ArcBody::Hidden,
	}
}

pub fn arc_paint(from: Option<Tile>, to: Option<Tile>) -> ArcPaint {
	match (from, to) {
		(None, None) => ArcPaint::neutral(),
		(Some(t), None) => one_ended(t, true),
		(None, Some(t)) => one_ended(t, false),
		(Some(a), Some(b)) => ArcPaint {
			body: two_ended_body(a, b),
			zags_from: tile_zag_strokes(a),
			zags_to: tile_zag_strokes(b),
		},
	}
}

/// One-ended edges: unique **normal** colors as a highway in the middle,
/// and this tile's zag strokes fanned on one side.
fn one_ended(tile: Tile, on_from: bool) -> ArcPaint {
	let mut normals: Vec<TileColor> = Vec::new();
	let zags = tile_zag_strokes(tile);
	for k in 0..tile.ring_strokes() {
		if tile.stroke_is_zag(k) {
			continue;
		}
		let c = tile.stroke_color(k);
		if !normals.contains(&c) {
			normals.push(c);
		}
	}
	let body = match normals.as_slice() {
		[] if zags[0].is_none() => ArcBody::Neutral,
		[] => ArcBody::Hidden,
		[c] => ArcBody::Solid { color: *c, thick: false },
		[a, b, ..] => ArcBody::Stripes { a: *a, b: *b, zag_a: false, zag_b: false },
	};
	if on_from {
		ArcPaint { body, zags_from: zags, zags_to: ArcPaint::no_zags() }
	} else {
		ArcPaint { body, zags_from: ArcPaint::no_zags(), zags_to: zags }
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
	pub arc_width: f32,
	pub stripe_off: f32,
	pub stripe_off_center: f32,
	pub knot_r: f32,
	pub knot_r_center: f32,
	pub zag_amp: f32,
	pub zag_amp_center: f32,
}

impl ViewLayout {
	pub fn from_window(width: f32, height: f32) -> Self {
		let width = width.max(320.0);
		let height = height.max(240.0);
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
		// Seven-tile benches must fit the window width on phones; shrink the
		// bench discs (not the board tiles) and cap spacing if needed.
		let n_slots = 7.0_f32;
		let available_w = width - 2.0 * SIDE_MARGIN_PX;
		let min_gap = 6.0;
		let max_bench_r = ((available_w - (n_slots - 1.0) * min_gap) / (2.0 * n_slots)).max(8.0);
		let bench_tile_radius = tile_radius.min(max_bench_r);
		let desired_spacing = 2.0 * bench_tile_radius + (0.40 * bench_tile_radius).max(min_gap);
		let max_spacing = (available_w - 2.0 * bench_tile_radius) / (n_slots - 1.0);
		let bench_spacing = desired_spacing.min(max_spacing.max(2.0 * bench_tile_radius + 1.0));

		let top = height / 2.0;
		let bottom = -height / 2.0;
		let p2_bench_y = top - HUD_TOP_PX - MARGIN_PX - bench_tile_radius;
		let p1_bench_y = bottom + HELP_BOTTOM_PX + MARGIN_PX + bench_tile_radius;

		let board_top = p2_bench_y - bench_tile_radius - BOARD_GAP_PX;
		let board_bot = p1_bench_y + bench_tile_radius + BOARD_GAP_PX;
		let origin_y = 0.5 * (board_top + board_bot);
		let origin_x = 0.0;

		let arc_width = (ARC_WIDTH_UNITS * scale).max(ARC_WIDTH_MIN_PX);
		let stripe_off = (STRIPE_OFFSET_UNITS * scale).max(2.2);
		let stripe_off_center = (STRIPE_OFFSET_CENTER_UNITS * scale).max(1.6);
		let knot_r = (KNOT_RADIUS_UNITS * scale).max(KNOT_RADIUS_MIN_PX);
		let knot_r_center = (KNOT_RADIUS_CENTER_UNITS * scale).max(KNOT_RADIUS_CENTER_MIN_PX);
		let zag_amp = (ZAG_AMP_UNITS * scale).max(0.85);
		let zag_amp_center = (ZAG_AMP_CENTER_UNITS * scale).max(0.7);

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
			arc_width,
			stripe_off,
			stripe_off_center,
			knot_r,
			knot_r_center,
			zag_amp,
			zag_amp_center,
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
		let start = -total / 2.0;
		[start + index as f32 * self.bench_spacing, y]
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
pub struct KnotFrame {
	pub index: usize,
	pub mid: [f64; 2],
	pub tangent: [f64; 2],
	pub normal: [f64; 2],
}

pub fn midpoint_frame(pts: &[[f64; 2]]) -> KnotFrame {
	let index = pts.len() / 2;
	let mid = pts.get(index).copied().unwrap_or([0.0, 0.0]);
	let (nx, ny) = if pts.is_empty() { (0.0, 1.0) } else { polyline_normal(pts, index) };
	KnotFrame { index, mid, tangent: [ny, -nx], normal: [nx, ny] }
}

/// Compact trefoil projection: a tangled loop readable at board scale.
pub fn knot_polyline(
	mid: [f64; 2],
	tangent: [f64; 2],
	normal: [f64; 2],
	radius: f64,
) -> Vec<[f64; 2]> {
	let mut tan = tangent;
	let tlen = (tan[0] * tan[0] + tan[1] * tan[1]).sqrt();
	if tlen > 1e-12 {
		tan[0] /= tlen;
		tan[1] /= tlen;
	} else {
		tan = [1.0, 0.0];
	}
	let mut nor = normal;
	let nlen = (nor[0] * nor[0] + nor[1] * nor[1]).sqrt();
	if nlen > 1e-12 {
		nor[0] /= nlen;
		nor[1] /= nlen;
	} else {
		nor = [-tan[1], tan[0]];
	}
	let s = radius / 3.05;
	let n = KNOT_SAMPLES.max(16);
	let mut pts = Vec::with_capacity(n + 1);
	for i in 0..=n {
		let t = 2.0 * PI * i as f64 / n as f64;
		let u = t.sin() + 2.0 * (2.0 * t).sin();
		let v = t.cos() - 2.0 * (2.0 * t).cos();
		pts.push([mid[0] + s * (tan[0] * u + nor[0] * v), mid[1] + s * (tan[1] * u + nor[1] * v)]);
	}
	pts
}

fn polyline_dist(a: [f64; 2], b: [f64; 2]) -> f64 {
	let dx = a[0] - b[0];
	let dy = a[1] - b[1];
	(dx * dx + dy * dy).sqrt()
}

fn polyline_lerp(a: [f64; 2], b: [f64; 2], t: f64) -> [f64; 2] {
	[a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t]
}

/// Drop the last `clearance` of path length so stripes stop at the knot.
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

pub fn knot_clearance(knot_r: f64) -> f64 {
	knot_r * 0.85
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
	(arc_length(arc) / 0.12).clamp(7.0, 16.0)
}

pub fn zag_lane_spacing(stripe_off: f64) -> f64 {
	stripe_off * ZAG_LANE_SPACING_SCALE
}

/// Distance from the arc path to the first zag lane. Lane 0 is skipped so a
/// Hidden body keeps a visible middle gap (3 | gap | 3), and a match/highway/
/// knot keeps the connection line in the center. `knot_r` pushes zags outside
/// a conflict knot so they don't run through the glyph.
pub fn zag_lane_inner(body: ArcBody, stripe_off: f64, knot_r: f64) -> f64 {
	let spacing = zag_lane_spacing(stripe_off);
	match body {
		ArcBody::Knot { .. } => knot_r.max(stripe_off) + spacing,
		ArcBody::Stripes { .. } | ArcBody::Solid { .. } => stripe_off + spacing,
		ArcBody::Hidden | ArcBody::Neutral => spacing,
	}
}

/// Fan zag `index` (0 = nearest the middle) to `sign` (+1 from, −1 to).
pub fn zag_fan_offset(index: usize, sign: f64, inner: f64, spacing: f64) -> f64 {
	sign * (inner + index as f64 * spacing)
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
				"Turn {}{HUD_SEP}{} ({}) to play{HUD_SEP}P1 light {}/{}{HUD_SEP}P2 dark {}/{}{hold}",
				game.turn_count() + 1,
				game.current_player().label(),
				actor_name,
				game.bench(Player::One).len(),
				game.bag_len(Player::One),
				game.bench(Player::Two).len(),
				game.bag_len(Player::Two)
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
/// color, same-color, two-ended multi-shared highways, conflict knots,
/// one-ended highway stripes (including a zag strand), single and double zag
/// tiles, a match with a zag on one side, a match with zags on both sides,
/// a zag-on-zag edge with an empty middle lane, a non-matching zag that
/// still draws on its side (no knot), and a match with a non-hitting zag.
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
		// Dedicated multi-color conflict knot: A blue+green vs B orange+teal.
		let t = game.alloc_rings(Player::One, &[TileColor::Blue, TileColor::Green], 2, 1);
		let u = game.alloc_rings(Player::Two, &[TileColor::Orange, TileColor::Teal], 2, 1);
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
			// Green vs center orange is a knot; the pink zag sits on this side.
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
		let is_blue_only =
			|t: Tile| t.has_normal(TileColor::Blue) && t.n_rings == 1 && t.zag.iter().all(|z| !*z);
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

	let mut bag1 = Vec::new();
	let mut bag2 = Vec::new();
	for i in 0..14 {
		let color = TileColor::ALL[i % 3];
		bag1.push(game.alloc_tile(Player::One, color, 1 + (i % 3) as u8, 1 + ((i + 1) % 3) as u8));
		bag2.push(game.alloc_tile(Player::Two, color, 1 + ((i + 2) % 3) as u8, 1 + (i % 3) as u8));
	}
	game.set_bag(Player::One, bag1);
	game.set_bag(Player::Two, bag2);
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
	let mut game = Game::empty(RuleSet::default());
	if let Some((a, b)) = empty_neighbor_pair(&game) {
		let t = game.alloc_ring_kinds(Player::One, a_kinds, a_weight, 1);
		let u = game.alloc_ring_kinds(Player::Two, b_kinds, b_weight, 1);
		game.place_tile(a, t);
		game.place_tile(b, u);
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
		assert_eq!(paint.zag_from_count(), 3, "A's three zags stay on A's side");
		assert_eq!(paint.zag_to_count(), 3, "B's three zags stay on B's side");
		assert!(paint
			.zags_from
			.iter()
			.chain(paint.zags_to.iter())
			.all(|c| *c == Some(TileColor::Blue)));
		let one = Tile::with_ring_kinds(3, Player::One, &[(TileColor::Blue, true)], 1, 1);
		let paint = arc_paint(Some(one), Some(one));
		assert_eq!(paint.body, ArcBody::Hidden);
		assert_eq!(paint.zag_from_count(), 1);
		assert_eq!(paint.zag_to_count(), 1);
		assert_eq!(paint.zags_from[0], Some(TileColor::Blue));
		assert_eq!(paint.zags_to[0], Some(TileColor::Blue));
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
		assert_eq!(two_ended_two.zag_from_count(), 2);
		assert_eq!(two_ended_two.zag_to_count(), 2);
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
			let n = 7usize;
			let half =
				layout.bench_spacing * n.saturating_sub(1) as f32 / 2.0 + layout.bench_tile_radius;
			assert!(
				half + slack <= w / 2.0 - 8.0,
				"{w}x{h}: 7-tile bench overflows width (half={half})"
			);
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
			ArcPaint::knot(
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
		assert_eq!(
			paint.body,
			ArcBody::Solid { color: TileColor::Blue, thick: true },
			"shared normal sits in the middle; zags fan to both sides"
		);
		assert_eq!(paint.zags_from[0], Some(TileColor::Orange));
		assert_eq!(paint.zags_to[0], Some(TileColor::Orange));
		let orange_zag = Tile::with_ring_kinds(6, Player::One, &[(TileColor::Orange, true)], 1, 1);
		let orange_normal = Tile::mono(7, Player::Two, TileColor::Orange, 1, 1);
		let paint = arc_paint(Some(orange_zag), Some(orange_normal));
		assert_eq!(paint.body, ArcBody::Hidden);
		assert_eq!(paint.zags_from[0], Some(TileColor::Orange));
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
		assert_eq!(paint.body, ArcBody::Solid { color: TileColor::Blue, thick: true });
		assert_eq!(paint.zags_from[0], Some(TileColor::Orange));
		assert_eq!(paint.zag_to_count(), 0);
		let blue_only = Tile::mono(12, Player::Two, TileColor::Blue, 1, 1);
		let paint = arc_paint(Some(orange_zag), Some(blue_only));
		assert_eq!(paint.body, ArcBody::Hidden, "non-matching zag is not a conflict knot");
		assert_eq!(paint.zags_from[0], Some(TileColor::Orange), "zag still draws on its side");
		assert_eq!(paint.zag_to_count(), 0);
		let mixed_vs_blue = Tile::with_ring_kinds(
			13,
			Player::One,
			&[(TileColor::Blue, false), (TileColor::Orange, true)],
			2,
			1,
		);
		let paint = arc_paint(Some(mixed_vs_blue), Some(blue_only));
		assert_eq!(paint.body, ArcBody::Solid { color: TileColor::Blue, thick: true });
		assert_eq!(paint.zags_from[0], Some(TileColor::Orange), "orange zag sits beside the match");
		assert_eq!(paint.zag_to_count(), 0);
		let green = Tile::mono(14, Player::Two, TileColor::Green, 1, 1);
		let paint = arc_paint(Some(mixed_vs_blue), Some(green));
		assert!(
			matches!(paint.body, ArcBody::Knot { .. }),
			"blue vs green is still a real conflict"
		);
		assert_eq!(paint.zags_from[0], Some(TileColor::Orange));
		let bg = Tile::with_rings(15, Player::One, &[TileColor::Blue, TileColor::Green], 2, 1);
		let ot = Tile::with_rings(16, Player::Two, &[TileColor::Orange, TileColor::Teal], 2, 1);
		let paint = arc_paint(Some(bg), Some(ot));
		assert_eq!(
			paint.body,
			ArcBody::Knot {
				from: [Some(TileColor::Blue), Some(TileColor::Green), None],
				to: [Some(TileColor::Orange), Some(TileColor::Teal), None],
			},
			"knot halves list unmatched normals only"
		);
		assert_eq!(paint.zag_count(), 0);
		let mixed_knot = Tile::with_ring_kinds(
			17,
			Player::One,
			&[(TileColor::Blue, false), (TileColor::Orange, true)],
			2,
			1,
		);
		let paint = arc_paint(Some(mixed_knot), Some(green));
		match paint.body {
			ArcBody::Knot { from, to } => {
				assert_eq!(from, [Some(TileColor::Blue), None, None]);
				assert_eq!(to, [Some(TileColor::Green), None, None]);
			}
			other => panic!("expected knot, got {other:?}"),
		}
		assert_eq!(
			paint.zags_from[0],
			Some(TileColor::Orange),
			"zag stays on its side of the knot"
		);
	}

	#[test]
	fn demo_midgame_has_center_and_mixed_arcs() {
		let game = demo_midgame();
		assert!(game.tile_at(game.board().center()).is_some(), "center must have a tile");
		let mut gray = 0;
		let mut one = 0;
		let mut same = 0;
		let mut knot = 0;
		let mut multi_color_knot = 0;
		let mut knot_with_zag = 0;
		let mut stripes = 0;
		let mut two_ended_stripes = 0;
		let mut stripe_with_zag = 0;
		let mut one_ended_zag_highway = 0;
		let mut one_ended_triple_zag = 0;
		let mut hidden = 0;
		let mut zag_arcs = 0;
		let mut zag_beside_match = 0;
		let mut zag_both_sides = 0;
		let mut zag_on_zag = 0;
		let mut non_matching_zag = 0;
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
			let paint = arc_paint(from, to);
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
				if paint.body == ArcBody::Hidden
					&& a.zag_hits_on(b) > 0
					&& b.zag_hits_on(a) > 0
					&& !a.shares_normal(b)
				{
					zag_on_zag += 1;
				}
				if paint.body == ArcBody::Hidden
					&& paint.zag_count() > 0
					&& !a.shares_normal(b)
					&& a.zag_hits_on(b) == 0
					&& b.zag_hits_on(a) == 0
				{
					non_matching_zag += 1;
				}
				if matches!(paint.body, ArcBody::Solid { .. } | ArcBody::Stripes { .. })
					&& (paint.zag_from_count() > 0) != (paint.zag_to_count() > 0)
				{
					zag_beside_match += 1;
				}
				if matches!(paint.body, ArcBody::Solid { .. } | ArcBody::Stripes { .. })
					&& paint.zag_from_count() > 0
					&& paint.zag_to_count() > 0
				{
					zag_both_sides += 1;
				}
			}
			match paint.body {
				ArcBody::Neutral => gray += 1,
				ArcBody::Solid { thick: false, .. } => one += 1,
				ArcBody::Solid { thick: true, .. } => same += 1,
				ArcBody::Knot { from: cols_a, to: cols_b } => {
					knot += 1;
					assert!(from.is_some() && to.is_some(), "knot is two-ended only");
					if let (Some(a), Some(b)) = (from, to) {
						assert!(
							shared_normal_colors(a, b)[0].is_none()
								&& a.normal_mask() != 0 && b.normal_mask() != 0,
							"knot is for true color conflicts only"
						);
						assert_eq!(cols_a, unique_normals(a));
						assert_eq!(cols_b, unique_normals(b));
						if packed_color_count(cols_a) >= 2 && packed_color_count(cols_b) >= 2 {
							multi_color_knot += 1;
						}
						if paint.zag_count() > 0 {
							knot_with_zag += 1;
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
			}
		}
		assert!(gray > 0, "need empty gray arcs");
		assert!(one > 0, "need one-ended colored arcs");
		assert!(same > 0, "need same-color solid arcs");
		assert!(knot > 0, "need two-ended conflicting knots");
		assert!(multi_color_knot > 0, "need a multi-color vs multi-color knot");
		assert!(knot_with_zag > 0, "need a conflict knot with zags on the sides");
		assert!(stripes > 0, "need highway-stripe arcs");
		assert!(two_ended_stripes > 0, "need a two-ended multi-shared-color highway");
		assert!(
			stripe_with_zag > 0 || one_ended_zag_highway > 0,
			"need an undecided highway with a zag strand"
		);
		assert!(one_ended_triple_zag > 0, "need a one-ended triple-zag arc");
		assert!(zag_arcs > 0, "need a firing zag on an arc");
		assert!(zag_beside_match > 0, "need a match with a zag on one side");
		assert!(zag_both_sides > 0, "need a match with zags on both sides");
		assert!(hidden > 0, "need a decided zag-only arc (no match line)");
		assert!(zag_on_zag > 0, "need a zag-on-zag edge with no match stroke");
		assert!(non_matching_zag > 0, "need a non-matching zag edge that is not a knot");
		assert!(single_zag_tiles > 0, "need a single-zag tile");
		assert!(double_zag_tiles > 0, "need a double-zag tile");
		assert_eq!(game.bench(Player::One).len(), 5);
		assert_eq!(game.bench(Player::Two).len(), 5);
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
		assert!(turn.len() < 110, "turn line should fit a 1024px window: {turn}");
		assert!(turn.contains("ctl"), "control-win HUD should show on default rules: {turn}");
	}

	#[test]
	fn opening_hud_uses_control_win_and_seven_tile_bench() {
		let game = Game::new(RuleSet::default(), 1);
		assert!(game.rules().control_win);
		assert!(!game.rules().center_hold_win);
		assert_eq!(game.bench(Player::One).len(), 7);
		assert_eq!(game.bag_len(Player::One), 20);
		let turn = format_turn_line(&game, "human");
		assert!(turn.contains("ctl P1 0/5 0/1 P2 0/5 0/1"), "{turn}");
		assert!(turn.contains("P1 light 7/20"), "{turn}");
		assert!(turn.contains("P2 dark 7/20"), "{turn}");
		assert!(!turn.contains("hold P1"), "{turn}");
		assert!(turn.len() < 110, "turn line should fit a 1024px window: {turn}");
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
	fn knot_glyph_is_a_compact_closed_loop() {
		let pts = knot_polyline([0.0, 0.0], [1.0, 0.0], [0.0, 1.0], 1.0);
		assert!(pts.len() > 16);
		assert!((pts[0][0] - pts[pts.len() - 1][0]).abs() < 1e-9);
		assert!((pts[0][1] - pts[pts.len() - 1][1]).abs() < 1e-9);
		let max_r = pts.iter().map(|p| (p[0] * p[0] + p[1] * p[1]).sqrt()).fold(0.0, f64::max);
		assert!(max_r < 1.15, "knot escaped radius: {max_r}");
		let frame = midpoint_frame(&[[0.0, 0.0], [1.0, 0.0], [2.0, 0.0], [3.0, 0.0]]);
		assert_eq!(frame.index, 2);
		assert!((frame.mid[0] - 2.0).abs() < 1e-9);
		let half = shorten_polyline_end(&[[0.0, 0.0], [1.0, 0.0], [2.0, 0.0], [3.0, 0.0]], 0.5);
		assert!((half[half.len() - 1][0] - 2.5).abs() < 1e-9);
		let layout = ViewLayout::from_window(1280.0, 800.0);
		assert!(
			layout.knot_r >= KNOT_RADIUS_MIN_PX,
			"knot must stay readable at board scale (r={})",
			layout.knot_r
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
