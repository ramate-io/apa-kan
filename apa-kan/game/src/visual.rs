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

/// Doubled tile radius in board units (`2 * 22 / 210`). Closest nodes on the
/// default 8-petal rosette are ~0.5858 apart (adjacent inner-ring
/// intersections), so this still leaves ~0.167 board units of air gap.
pub const TILE_RADIUS_UNITS: f32 = (PREV_TILE_RADIUS_PX / PREV_SCALE) * 2.0;

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
pub const BRAID_STRAND_WIDTH_SCALE: f32 = 0.78;
pub const BRAID_AMP_UNITS: f32 = 0.030;
pub const BRAID_AMP_CENTER_UNITS: f32 = 0.016;

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
pub enum ArcPaint {
	Neutral,
	Solid { color: TileColor, thick: bool },
	Braid { a: TileColor, b: TileColor },
}

pub fn shared_ring_color(a: Tile, b: Tile) -> Option<TileColor> {
	a.ring_colors().iter().copied().find(|&c| b.has_color(c))
}

pub fn arc_paint(from: Option<Tile>, to: Option<Tile>) -> ArcPaint {
	match (from, to) {
		(None, None) => ArcPaint::Neutral,
		(Some(t), None) | (None, Some(t)) => ArcPaint::Solid { color: t.primary(), thick: false },
		(Some(a), Some(b)) => {
			if let Some(color) = shared_ring_color(a, b) {
				ArcPaint::Solid { color, thick: true }
			} else {
				ArcPaint::Braid { a: a.primary(), b: b.primary() }
			}
		}
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
	pub braid_amp: f32,
	pub braid_amp_center: f32,
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
		let bench_tile_radius = tile_radius;
		let bench_spacing = 2.0 * bench_tile_radius + (0.40 * bench_tile_radius).max(8.0);

		let top = height / 2.0;
		let bottom = -height / 2.0;
		let p2_bench_y = top - HUD_TOP_PX - MARGIN_PX - bench_tile_radius;
		let p1_bench_y = bottom + HELP_BOTTOM_PX + MARGIN_PX + bench_tile_radius;

		let board_top = p2_bench_y - bench_tile_radius - BOARD_GAP_PX;
		let board_bot = p1_bench_y + bench_tile_radius + BOARD_GAP_PX;
		let origin_y = 0.5 * (board_top + board_bot);
		let origin_x = 0.0;

		let arc_width = (ARC_WIDTH_UNITS * scale).max(ARC_WIDTH_MIN_PX);
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
			arc_width,
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

#[derive(Clone, Debug)]
pub struct Braid {
	pub a: Vec<[f64; 2]>,
	pub b: Vec<[f64; 2]>,
	pub phase: Vec<f64>,
}

pub fn braid_strands(centerline: &[[f64; 2]], amplitude: f64, turns: f64) -> Braid {
	let n = centerline.len().max(2);
	let mut a = Vec::with_capacity(n);
	let mut b = Vec::with_capacity(n);
	let mut phase = Vec::with_capacity(n);
	for i in 0..n {
		let t = if n == 1 { 0.0 } else { i as f64 / (n - 1) as f64 };
		let ph = 2.0 * PI * turns * t;
		let (nx, ny) = polyline_normal(centerline, i);
		let off = amplitude * ph.sin();
		let p = centerline[i];
		a.push([p[0] + nx * off, p[1] + ny * off]);
		b.push([p[0] - nx * off, p[1] - ny * off]);
		phase.push(ph);
	}
	Braid { a, b, phase }
}

pub fn braid_turns(arc: &ArcEdge) -> f64 {
	let mut sweep = arc.angle_to - arc.angle_from;
	if sweep <= 0.0 {
		sweep += 2.0 * PI;
	}
	let len = arc.radius * sweep;
	(len / 0.42).clamp(1.2, 3.6)
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
/// color, same-color, and two-color (braided) arcs, plus a tile on the center.
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
		place(&mut game, n, Player::One, TileColor::Orange, 1, 1);
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
		let tile =
			game.alloc_rings(Player::One, &[TileColor::Green, TileColor::LavenderPink], 2, 1);
		game.place_tile(n, tile);
	}

	let p1_bench = vec![
		game.alloc_tile(Player::One, TileColor::Orange, 2, 2),
		game.alloc_tile(Player::One, TileColor::Orange, 3, 1),
		game.alloc_tile(Player::One, TileColor::Blue, 1, 3),
		game.alloc_tile(Player::One, TileColor::Green, 2, 2),
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
	let tile = game.alloc_tile(owner, color, weight, dots);
	game.place_tile(node, tile);
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
	fn doubled_tiles_do_not_collide_at_closest_nodes() {
		let board = Board::rosette_8();
		let min_d = min_node_distance(&board);
		// Adjacent inner-ring nodes are the tightest pair (~0.5858).
		assert!((min_d - 0.5858).abs() < 0.01, "closest node spacing changed: {min_d}");
		let diameter = f64::from(TILE_RADIUS_UNITS) * 2.0;
		assert!(
			diameter < min_d,
			"doubled tile diameter {diameter} collides at min spacing {min_d}"
		);
		let gap = min_d - diameter;
		assert!(gap > 0.12, "expected a comfortable gap, got {gap} (min_d={min_d})");
	}

	#[test]
	fn layout_keeps_chrome_off_the_rosette() {
		for (w, h) in [(1024.0, 728.0), (1280.0, 800.0), (1920.0, 1080.0), (800.0, 600.0)] {
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
		assert_eq!(arc_paint(None, None), ArcPaint::Neutral);
		assert_eq!(
			arc_paint(Some(orange), None),
			ArcPaint::Solid { color: TileColor::Orange, thick: false }
		);
		assert_eq!(
			arc_paint(Some(orange), Some(orange)),
			ArcPaint::Solid { color: TileColor::Orange, thick: true }
		);
		assert_eq!(
			arc_paint(Some(orange), Some(green)),
			ArcPaint::Braid { a: TileColor::Orange, b: TileColor::Green }
		);
		let duo = Tile::with_rings(3, Player::One, &[TileColor::Green, TileColor::Teal], 1, 1);
		assert_eq!(
			arc_paint(Some(green), Some(duo)),
			ArcPaint::Solid { color: TileColor::Green, thick: true }
		);
	}

	#[test]
	fn demo_midgame_has_center_and_mixed_arcs() {
		let game = demo_midgame();
		assert!(game.tile_at(game.board().center()).is_some(), "center must have a tile");
		let mut gray = 0;
		let mut one = 0;
		let mut same = 0;
		let mut braid = 0;
		for arc in game.board().arcs() {
			match arc_paint(game.tile_at(arc.from), game.tile_at(arc.to)) {
				ArcPaint::Neutral => gray += 1,
				ArcPaint::Solid { thick: false, .. } => one += 1,
				ArcPaint::Solid { thick: true, .. } => same += 1,
				ArcPaint::Braid { .. } => braid += 1,
			}
		}
		assert!(gray > 0, "need empty gray arcs");
		assert!(one > 0, "need one-ended colored arcs");
		assert!(same > 0, "need same-color solid arcs");
		assert!(braid > 0, "need two-color braided arcs");
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
	fn braid_has_two_offset_strands() {
		let pts = [[0.0, 0.0], [1.0, 0.0], [2.0, 0.0], [3.0, 0.0]];
		let braid = braid_strands(&pts, 0.5, 1.0);
		assert_eq!(braid.a.len(), 4);
		assert!(braid.a[1][1].abs() > 0.2);
		assert!((braid.a[1][1] + braid.b[1][1]).abs() < 1e-9);
	}
}
