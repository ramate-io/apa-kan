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

/// 1.5× the previous doubled radius (`2 * 22 / 210`). Closest inner-ring
/// nodes are ~0.5858 apart, so adjacent discs sit close; layout still
/// scales the whole board to the window so chrome and mobile benches fit.
pub const TILE_SIZE_SCALE: f32 = 1.5;
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
/// Tight zigzag amplitude, in board units. Small enough to stitch along the
/// arc, large enough to read as a zag rather than a wobbly match line.
pub const ZAG_AMP_UNITS: f32 = 0.016;
pub const ZAG_AMP_CENTER_UNITS: f32 = 0.011;
pub const ZAG_STRAND_WIDTH_SCALE: f32 = 0.78;
pub const ARC_SAMPLES: usize = 96;
/// Tile-ring layout in 22px-tile units; scaled by `radius / PREV_TILE_RADIUS_PX`.
pub const TILE_RING_INSET: f32 = 3.2;
pub const TILE_RING_GAP: f32 = 5.8;
pub const TILE_ZAG_AMP: f32 = 1.55;
pub const TILE_ZAG_TEETH: u32 = 28;
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
	pub zags: [Option<TileColor>; 2],
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ArcBody {
	Neutral,
	Solid {
		color: TileColor,
		thick: bool,
	},
	/// Two parallel strokes, one per candidate color (highway stripes).
	Stripes {
		a: TileColor,
		b: TileColor,
	},
	/// A firing zag is the only stroke on this arc. The match line is
	/// omitted visually; scoring is unchanged (pending a rules call).
	Hidden,
}

impl ArcPaint {
	pub fn neutral() -> Self {
		Self { body: ArcBody::Neutral, zags: [None; 2] }
	}

	pub fn solid(color: TileColor, thick: bool) -> Self {
		Self { body: ArcBody::Solid { color, thick }, zags: [None; 2] }
	}

	pub fn stripes(a: TileColor, b: TileColor) -> Self {
		Self { body: ArcBody::Stripes { a, b }, zags: [None; 2] }
	}

	pub fn zag_only(zags: [Option<TileColor>; 2]) -> Self {
		Self { body: ArcBody::Hidden, zags }
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

/// Zag colors that actually fire on this edge (either tile's zag vs the other).
pub fn firing_zags(a: Tile, b: Tile) -> [Option<TileColor>; 2] {
	let mut out = [None; 2];
	let mut n = 0usize;
	for (src, dst) in [(a, b), (b, a)] {
		for (i, &c) in src.ring_colors().iter().enumerate() {
			if src.zag[i] && dst.has_color(c) && !out.contains(&Some(c)) && n < 2 {
				out[n] = Some(c);
				n += 1;
			}
		}
	}
	out
}

pub fn arc_paint(from: Option<Tile>, to: Option<Tile>) -> ArcPaint {
	match (from, to) {
		(None, None) => ArcPaint::neutral(),
		(Some(t), None) | (None, Some(t)) => ArcPaint::solid(t.primary(), false),
		(Some(a), Some(b)) => {
			let zags = firing_zags(a, b);
			if zags[0].is_some() {
				return ArcPaint::zag_only(zags);
			}
			let shared = shared_normal_colors(a, b);
			let body = match (shared[0], shared[1]) {
				(Some(color), None) => ArcBody::Solid { color, thick: true },
				(Some(c1), Some(c2)) => ArcBody::Stripes { a: c1, b: c2 },
				(None, _) => ArcBody::Stripes { a: a.primary(), b: b.primary() },
			};
			ArcPaint { body, zags: [None; 2] }
		}
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
			zag_amp: (TILE_ZAG_AMP * scale).max(0.65),
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
		let zag_amp = (ZAG_AMP_UNITS * scale).max(1.4);
		let zag_amp_center = (ZAG_AMP_CENTER_UNITS * scale).max(1.1);

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

/// High-frequency teeth so the zag stitches along the arc instead of waving.
pub fn zag_teeth(arc: &ArcEdge) -> f64 {
	(arc_length(arc) / 0.07).clamp(14.0, 36.0)
}

pub fn zag_side_offset(index: usize, n_zags: usize, stripe_off: f64) -> f64 {
	if n_zags <= 1 {
		0.0
	} else if index == 0 {
		-stripe_off
	} else {
		stripe_off
	}
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
/// color, same-color, two-color (highway stripe) arcs, single and double zag
/// tiles, and a decided zag-only arc, plus a tile on the center.
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
		let t = game.alloc_rings(Player::One, &[TileColor::Green, TileColor::LavenderPink], 2, 1);
		let u = game.alloc_rings(Player::Two, &[TileColor::Green, TileColor::LavenderPink], 2, 1);
		game.place_tile(a, t);
		game.place_tile(b, u);
	}
	if let Some((a, b)) = empty_neighbor_pair(&game) {
		let t = game.alloc_ring_kinds(Player::One, &[(TileColor::Orange, true)], 1, 1);
		let u = game.alloc_rings(Player::Two, &[TileColor::Orange], 2, 1);
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
		place(&mut game, n, Player::Two, TileColor::Green, 3, 1);
	}
	if let Some(&n) = inner.get(1) {
		place(&mut game, n, Player::Two, TileColor::Orange, 2, 2);
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
	if let Some(&n) = outer.get(7) {
		if game.tile_at(n).is_none() {
			let tile =
				game.alloc_rings(Player::One, &[TileColor::Green, TileColor::LavenderPink], 2, 1);
			game.place_tile(n, tile);
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
	fn tiles_are_one_and_a_half_times_previous() {
		let prev = (PREV_TILE_RADIUS_PX / PREV_SCALE) * 2.0;
		assert!((TILE_RADIUS_UNITS - prev * TILE_SIZE_SCALE).abs() < 1e-6);
		assert!((TILE_SIZE_SCALE - 1.5).abs() < 1e-6);
	}

	#[test]
	fn larger_tiles_still_read_at_closest_nodes() {
		let board = Board::rosette_8();
		let min_d = min_node_distance(&board);
		// Adjacent inner-ring nodes are the tightest pair (~0.5858).
		assert!((min_d - 0.5858).abs() < 0.01, "closest node spacing changed: {min_d}");
		let diameter = f64::from(TILE_RADIUS_UNITS) * 2.0;
		// 1.5× previous sits close on this pair; discs may kiss but must not
		// swallow the nodes (overlap well under a radius).
		assert!(
			diameter < min_d + f64::from(TILE_RADIUS_UNITS) * 0.25,
			"1.5x tile diameter {diameter} swallows closest nodes at {min_d}"
		);
	}

	#[test]
	fn tile_zag_rings_keep_concentric_gap() {
		let d = TileRingDraw::for_radius(33.0);
		assert!(
			2.0 * d.zag_amp + d.stroke < d.gap,
			"zag amp {} + stroke {} must fit in ring gap {}",
			d.zag_amp,
			d.stroke,
			d.gap
		);
		assert!(d.zag_teeth >= 24);
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
			ArcPaint::stripes(TileColor::Orange, TileColor::Green)
		);
		let duo = Tile::with_rings(3, Player::One, &[TileColor::Green, TileColor::Teal], 1, 1);
		assert_eq!(arc_paint(Some(green), Some(duo)), ArcPaint::solid(TileColor::Green, true));
		let gb = Tile::with_rings(4, Player::One, &[TileColor::Green, TileColor::Blue], 1, 1);
		let paint = arc_paint(Some(gb), Some(gb));
		assert_eq!(paint.body, ArcBody::Stripes { a: TileColor::Green, b: TileColor::Blue });
		let zag = Tile::with_ring_kinds(
			5,
			Player::One,
			&[(TileColor::Blue, false), (TileColor::Orange, true)],
			1,
			1,
		);
		let paint = arc_paint(Some(zag), Some(zag));
		assert_eq!(paint.body, ArcBody::Hidden);
		assert_eq!(paint.zags[0], Some(TileColor::Orange));
		let orange_zag = Tile::with_ring_kinds(6, Player::One, &[(TileColor::Orange, true)], 1, 1);
		let orange_normal = Tile::mono(7, Player::Two, TileColor::Orange, 1, 1);
		let paint = arc_paint(Some(orange_zag), Some(orange_normal));
		assert_eq!(paint.body, ArcBody::Hidden);
		assert_eq!(paint.zags[0], Some(TileColor::Orange));
	}

	#[test]
	fn demo_midgame_has_center_and_mixed_arcs() {
		let game = demo_midgame();
		assert!(game.tile_at(game.board().center()).is_some(), "center must have a tile");
		let mut gray = 0;
		let mut one = 0;
		let mut same = 0;
		let mut stripes = 0;
		let mut hidden = 0;
		let mut zag_arcs = 0;
		let mut two_match = 0;
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
			let paint = arc_paint(game.tile_at(arc.from), game.tile_at(arc.to));
			if paint.zags[0].is_some() {
				zag_arcs += 1;
			}
			match paint.body {
				ArcBody::Neutral => gray += 1,
				ArcBody::Solid { thick: false, .. } => one += 1,
				ArcBody::Solid { thick: true, .. } => same += 1,
				ArcBody::Stripes { .. } => {
					stripes += 1;
					if let (Some(a), Some(b)) = (game.tile_at(arc.from), game.tile_at(arc.to)) {
						if shared_normal_colors(a, b)[1].is_some() {
							two_match += 1;
						}
					}
				}
				ArcBody::Hidden => hidden += 1,
			}
		}
		assert!(gray > 0, "need empty gray arcs");
		assert!(one > 0, "need one-ended colored arcs");
		assert!(same > 0, "need same-color solid arcs");
		assert!(stripes > 0, "need two-color highway-stripe arcs");
		assert!(two_match > 0, "need an undecided two-shared-color arc");
		assert!(zag_arcs > 0, "need a firing zag on an arc");
		assert!(hidden > 0, "need a decided zag-only arc (no match line)");
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
	fn zigzag_alongside_stays_tight_to_the_path() {
		let pts: Vec<[f64; 2]> = (0..80).map(|i| [i as f64 * 0.05, 0.0]).collect();
		let amp = 0.04;
		let zig = zigzag_alongside(&pts, 0.0, amp, 20.0);
		assert_eq!(zig.len(), pts.len());
		let max_dev = zig.iter().map(|p| p[1].abs()).fold(0.0, f64::max);
		assert!(max_dev <= amp + 1e-9, "zag departed from the path: {max_dev} > {amp}");
		assert!(zig.iter().any(|p| p[1].abs() > amp * 0.5));
		let side = zigzag_alongside(&pts, 0.2, amp, 20.0);
		let mean_y: f64 = side.iter().map(|p| p[1]).sum::<f64>() / side.len() as f64;
		assert!(mean_y > 0.1, "alongside zag should sit beside the path, got mean {mean_y}");
		let ring = zigzag_circle([0.0, 0.0], 10.0, 0.4, 28, 96);
		assert!(ring.len() > 8);
		assert!((ring[0][0] - ring[ring.len() - 1][0]).abs() < 1e-9);
		let rs: Vec<f64> = ring.iter().map(|p| (p[0] * p[0] + p[1] * p[1]).sqrt()).collect();
		let rmin = rs.iter().copied().fold(f64::INFINITY, f64::min);
		let rmax = rs.iter().copied().fold(0.0, f64::max);
		assert!(rmax - rmin <= 0.8 + 1e-6, "tile zag amplitude too wide: {}", rmax - rmin);
	}
}
