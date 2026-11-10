//! Headless PNG renderer for the geometric board, tiles, and both benches.

use std::path::Path;

use image::{Rgba, RgbaImage};

use crate::game::{Game, Player, Tile};
use crate::visual::{self, ArcPaint, ViewLayout};

pub fn write_png(game: &Game, path: impl AsRef<Path>, caption: &str) -> Result<(), String> {
	write_png_sized(game, path, caption, 1280, 960)
}

pub fn write_png_sized(
	game: &Game,
	path: impl AsRef<Path>,
	caption: &str,
	width: u32,
	height: u32,
) -> Result<(), String> {
	let layout = ViewLayout::from_window(width as f32, height as f32);
	let bg = visual::rgb8(visual::BG_RGB);
	let mut img = RgbaImage::from_pixel(width, height, rgba(bg[0], bg[1], bg[2], 255));
	draw_arcs(&mut img, game, &layout);
	draw_nodes(&mut img, game, &layout);
	draw_tiles(&mut img, game, &layout);
	draw_benches(&mut img, game, &layout);
	draw_hud(&mut img, game, &layout, caption);
	img.save(path.as_ref()).map_err(|e| e.to_string())
}

fn draw_arcs(img: &mut RgbaImage, game: &Game, layout: &ViewLayout) {
	let board = game.board();
	let mut back: Vec<(Vec<[f64; 2]>, Rgba<u8>, f64)> = Vec::new();
	let mut front: Vec<(Vec<[f64; 2]>, Rgba<u8>, f64)> = Vec::new();
	for arc in board.arcs() {
		let samples = arc.sample(40);
		let pts: Vec<[f64; 2]> = samples.iter().map(|p| image_pt(layout, *p)).collect();
		let width = f64::from(layout.arc_width);
		match visual::arc_paint(game.tile_at(arc.from), game.tile_at(arc.to)) {
			ArcPaint::Neutral => {
				back.push((pts, rgb_of(visual::NEUTRAL_ARC_RGB), width));
			}
			ArcPaint::Solid { color, thick } => {
				let w = if thick {
					width * f64::from(visual::ARC_SAME_COLOR_WIDTH_SCALE)
				} else {
					width
				};
				back.push((pts, rgb_of(visual::tile_rgb(color)), w));
			}
			ArcPaint::Braid { a, b } => {
				let amp = if visual::arc_touches_center(arc, board.center()) {
					f64::from(layout.braid_amp_center)
				} else {
					f64::from(layout.braid_amp)
				};
				let braid = visual::braid_strands(&pts, amp, visual::braid_turns(arc));
				let strand_w = width * f64::from(visual::BRAID_STRAND_WIDTH_SCALE);
				split_braid(
					&braid.a,
					&braid.phase,
					true,
					rgb_of(visual::tile_rgb(a)),
					strand_w,
					&mut back,
					&mut front,
				);
				split_braid(
					&braid.b,
					&braid.phase,
					false,
					rgb_of(visual::tile_rgb(b)),
					strand_w,
					&mut back,
					&mut front,
				);
			}
		}
	}
	for (pts, color, width) in back.into_iter().chain(front) {
		stroke_polyline(img, &pts, color, width);
	}
}

fn split_braid(
	pts: &[[f64; 2]],
	phase: &[f64],
	strand_a: bool,
	color: Rgba<u8>,
	width: f64,
	back: &mut Vec<(Vec<[f64; 2]>, Rgba<u8>, f64)>,
	front: &mut Vec<(Vec<[f64; 2]>, Rgba<u8>, f64)>,
) {
	if pts.len() < 2 {
		return;
	}
	let mut start = 0usize;
	for i in 1..pts.len() {
		let front_a = phase[i].cos() >= 0.0;
		let this_front = if strand_a { front_a } else { !front_a };
		let prev_front_a = phase[i - 1].cos() >= 0.0;
		let prev_front = if strand_a { prev_front_a } else { !prev_front_a };
		if this_front != prev_front || i + 1 == pts.len() {
			let end = i;
			if end > start {
				let slice = pts[start..=end].to_vec();
				if prev_front {
					front.push((slice, color, width));
				} else {
					back.push((slice, color, width));
				}
			}
			start = i.saturating_sub(1);
		}
	}
}

fn draw_nodes(img: &mut RgbaImage, game: &Game, layout: &ViewLayout) {
	let board = game.board();
	for id in 0..board.node_count() as u8 {
		if game.tile_at(id).is_some() {
			continue;
		}
		let p = image_pt(layout, board.position(id));
		if board.is_spawn(id) {
			fill_circle(img, tup(p), f64::from(layout.spawn_radius), rgb_of(visual::SPAWN_RGB));
		} else if id == board.center() {
			fill_circle(
				img,
				tup(p),
				f64::from(layout.node_radius + 1.5),
				rgb_of(visual::CENTER_RGB),
			);
		} else {
			fill_circle(img, tup(p), f64::from(layout.node_radius), rgb_of(visual::NODE_RGB));
		}
	}
}

fn draw_tiles(img: &mut RgbaImage, game: &Game, layout: &ViewLayout) {
	let board = game.board();
	for id in 0..board.node_count() as u8 {
		if let Some(tile) = game.tile_at(id) {
			let p = image_pt(layout, board.position(id));
			draw_tile(img, tup(p), f64::from(layout.tile_radius), tile);
		}
	}
}

fn draw_benches(img: &mut RgbaImage, game: &Game, layout: &ViewLayout) {
	for player in [Player::Two, Player::One] {
		let bench = game.bench(player);
		for (i, tile) in bench.iter().enumerate() {
			let p = layout.bench_pos(player, i, bench.len());
			let q = layout.world_to_image(p);
			draw_tile(
				img,
				(f64::from(q[0]), f64::from(q[1])),
				f64::from(layout.bench_tile_radius),
				*tile,
			);
		}
	}
}

fn draw_hud(img: &mut RgbaImage, game: &Game, layout: &ViewLayout, caption: &str) {
	let hi = rgba(230, 230, 220, 255);
	let mid = rgba(200, 204, 190, 255);
	let lo = rgba(140, 146, 156, 255);
	draw_text(img, 16, 12, "APA KAN  |  8-PETAL ROSETTE", hi);
	draw_text(img, 16, 30, caption, mid);
	let turn = visual::format_turn_line(game, "human");
	draw_text(img, 16, 48, &turn, mid);
	draw_text(img, 16, (layout.height - 22.0) as u32, visual::HELP_TEXT, lo);
}

fn draw_tile(img: &mut RgbaImage, c: (f64, f64), r: f64, tile: Tile) {
	fill_circle(img, c, r, rgb_of(visual::owner_fill_rgb(tile.owner)));
	let col = rgb_of(visual::tile_rgb(tile.color));
	let scale = r / f64::from(visual::PREV_TILE_RADIUS_PX);
	let ring_gap = 3.5 * scale;
	for k in 0..tile.weight {
		let rr = r - 3.0 * scale - f64::from(k) * ring_gap;
		if rr > 4.0 * scale {
			stroke_circle(img, c, rr, col, (2.2 * scale).max(1.6));
		}
	}
	let dots = tile.dots.min(3);
	let spread = 7.0 * scale;
	let start = -spread * (f64::from(dots) - 1.0) / 2.0;
	let dot_r = (2.6 * scale).max(1.8);
	for i in 0..dots {
		let dx = start + f64::from(i) * spread;
		fill_circle(img, (c.0 + dx, c.1), dot_r, col);
	}
}

fn image_pt(layout: &ViewLayout, p: [f64; 2]) -> [f64; 2] {
	let q = layout.board_to_image(p);
	[f64::from(q[0]), f64::from(q[1])]
}

fn tup(p: [f64; 2]) -> (f64, f64) {
	(p[0], p[1])
}

fn rgb_of(rgb: [f32; 3]) -> Rgba<u8> {
	let c = visual::rgb8(rgb);
	rgba(c[0], c[1], c[2], 255)
}

fn stroke_polyline(img: &mut RgbaImage, pts: &[[f64; 2]], color: Rgba<u8>, width: f64) {
	for w in pts.windows(2) {
		draw_line(img, (w[0][0], w[0][1]), (w[1][0], w[1][1]), color, width);
	}
}

fn rgba(r: u8, g: u8, b: u8, a: u8) -> Rgba<u8> {
	Rgba([r, g, b, a])
}

fn blend(dst: &mut Rgba<u8>, src: Rgba<u8>, a: f64) {
	if a <= 0.0 {
		return;
	}
	let a = a.min(1.0);
	let ia = 1.0 - a;
	for i in 0..3 {
		dst.0[i] = (f64::from(dst.0[i]) * ia + f64::from(src.0[i]) * a) as u8;
	}
}

fn put(img: &mut RgbaImage, x: i32, y: i32, color: Rgba<u8>, a: f64) {
	if x < 0 || y < 0 {
		return;
	}
	let (w, h) = (img.width() as i32, img.height() as i32);
	if x >= w || y >= h {
		return;
	}
	blend(&mut img.get_pixel_mut(x as u32, y as u32), color, a);
}

fn fill_circle(img: &mut RgbaImage, c: (f64, f64), r: f64, color: Rgba<u8>) {
	let r2 = r + 0.5;
	let minx = (c.0 - r2 - 1.0).floor() as i32;
	let maxx = (c.0 + r2 + 1.0).ceil() as i32;
	let miny = (c.1 - r2 - 1.0).floor() as i32;
	let maxy = (c.1 + r2 + 1.0).ceil() as i32;
	for y in miny..=maxy {
		for x in minx..=maxx {
			let dx = f64::from(x) + 0.5 - c.0;
			let dy = f64::from(y) + 0.5 - c.1;
			let d = (dx * dx + dy * dy).sqrt();
			let a = (r2 - d + 0.5).clamp(0.0, 1.0);
			put(img, x, y, color, a);
		}
	}
}

fn stroke_circle(img: &mut RgbaImage, c: (f64, f64), r: f64, color: Rgba<u8>, width: f64) {
	let outer = r + width / 2.0;
	let inner = (r - width / 2.0).max(0.0);
	let minx = (c.0 - outer - 1.0).floor() as i32;
	let maxx = (c.0 + outer + 1.0).ceil() as i32;
	let miny = (c.1 - outer - 1.0).floor() as i32;
	let maxy = (c.1 + outer + 1.0).ceil() as i32;
	for y in miny..=maxy {
		for x in minx..=maxx {
			let dx = f64::from(x) + 0.5 - c.0;
			let dy = f64::from(y) + 0.5 - c.1;
			let d = (dx * dx + dy * dy).sqrt();
			let a = (outer + 0.5 - d).clamp(0.0, 1.0) * (d - inner + 0.5).clamp(0.0, 1.0);
			put(img, x, y, color, a);
		}
	}
}

fn draw_line(img: &mut RgbaImage, a: (f64, f64), b: (f64, f64), color: Rgba<u8>, width: f64) {
	let dx = b.0 - a.0;
	let dy = b.1 - a.1;
	let len = (dx * dx + dy * dy).sqrt().max(1.0);
	let n = (len.ceil() as i32).max(1);
	for i in 0..=n {
		let t = f64::from(i) / f64::from(n);
		fill_circle(img, (a.0 + dx * t, a.1 + dy * t), width / 2.0, color);
	}
}

fn draw_text(img: &mut RgbaImage, x: u32, y: u32, text: &str, color: Rgba<u8>) {
	let mut cx = x as i32;
	let cy = y as i32;
	for ch in text.chars() {
		let g = glyph(ch);
		for (row, bits) in g.iter().enumerate() {
			for col in 0..5 {
				if bits & (1 << (4 - col)) != 0 {
					for oy in 0..2 {
						for ox in 0..2 {
							put(
								img,
								cx + col as i32 * 2 + ox,
								cy + row as i32 * 2 + oy,
								color,
								1.0,
							);
						}
					}
				}
			}
		}
		cx += 12;
	}
}

fn glyph(c: char) -> [u8; 7] {
	match c.to_ascii_uppercase() {
		'A' => [0b01110, 0b10001, 0b10001, 0b11111, 0b10001, 0b10001, 0b10001],
		'B' => [0b11110, 0b10001, 0b10001, 0b11110, 0b10001, 0b10001, 0b11110],
		'C' => [0b01110, 0b10001, 0b10000, 0b10000, 0b10000, 0b10001, 0b01110],
		'D' => [0b11110, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b11110],
		'E' => [0b11111, 0b10000, 0b10000, 0b11110, 0b10000, 0b10000, 0b11111],
		'F' => [0b11111, 0b10000, 0b10000, 0b11110, 0b10000, 0b10000, 0b10000],
		'G' => [0b01110, 0b10001, 0b10000, 0b10111, 0b10001, 0b10001, 0b01110],
		'H' => [0b10001, 0b10001, 0b10001, 0b11111, 0b10001, 0b10001, 0b10001],
		'I' => [0b01110, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b01110],
		'K' => [0b10001, 0b10010, 0b10100, 0b11000, 0b10100, 0b10010, 0b10001],
		'L' => [0b10000, 0b10000, 0b10000, 0b10000, 0b10000, 0b10000, 0b11111],
		'M' => [0b10001, 0b11011, 0b10101, 0b10101, 0b10001, 0b10001, 0b10001],
		'N' => [0b10001, 0b11001, 0b10101, 0b10011, 0b10001, 0b10001, 0b10001],
		'O' => [0b01110, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01110],
		'P' => [0b11110, 0b10001, 0b10001, 0b11110, 0b10000, 0b10000, 0b10000],
		'R' => [0b11110, 0b10001, 0b10001, 0b11110, 0b10100, 0b10010, 0b10001],
		'S' => [0b01111, 0b10000, 0b10000, 0b01110, 0b00001, 0b00001, 0b11110],
		'T' => [0b11111, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100],
		'U' => [0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01110],
		'V' => [0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01010, 0b00100],
		'W' => [0b10001, 0b10001, 0b10001, 0b10101, 0b10101, 0b10101, 0b01010],
		'Y' => [0b10001, 0b10001, 0b01010, 0b00100, 0b00100, 0b00100, 0b00100],
		'0' => [0b01110, 0b10001, 0b10011, 0b10101, 0b11001, 0b10001, 0b01110],
		'1' => [0b00100, 0b01100, 0b00100, 0b00100, 0b00100, 0b00100, 0b01110],
		'2' => [0b01110, 0b10001, 0b00001, 0b00110, 0b01000, 0b10000, 0b11111],
		'3' => [0b11110, 0b00001, 0b00001, 0b01110, 0b00001, 0b00001, 0b11110],
		'4' => [0b00010, 0b00110, 0b01010, 0b10010, 0b11111, 0b00010, 0b00010],
		'5' => [0b11111, 0b10000, 0b11110, 0b00001, 0b00001, 0b10001, 0b01110],
		'6' => [0b01110, 0b10000, 0b11110, 0b10001, 0b10001, 0b10001, 0b01110],
		'7' => [0b11111, 0b00001, 0b00010, 0b00100, 0b01000, 0b01000, 0b01000],
		'8' => [0b01110, 0b10001, 0b10001, 0b01110, 0b10001, 0b10001, 0b01110],
		'9' => [0b01110, 0b10001, 0b10001, 0b01111, 0b00001, 0b00001, 0b01110],
		'-' => [0b00000, 0b00000, 0b00000, 0b11111, 0b00000, 0b00000, 0b00000],
		'.' => [0b00000, 0b00000, 0b00000, 0b00000, 0b00000, 0b00100, 0b00100],
		',' => [0b00000, 0b00000, 0b00000, 0b00000, 0b00100, 0b00100, 0b01000],
		':' => [0b00000, 0b00100, 0b00100, 0b00000, 0b00100, 0b00100, 0b00000],
		'=' => [0b00000, 0b00000, 0b11111, 0b00000, 0b11111, 0b00000, 0b00000],
		'+' => [0b00000, 0b00100, 0b00100, 0b11111, 0b00100, 0b00100, 0b00000],
		'/' => [0b00001, 0b00010, 0b00010, 0b00100, 0b01000, 0b01000, 0b10000],
		'|' => [0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100],
		'(' => [0b00010, 0b00100, 0b01000, 0b01000, 0b01000, 0b00100, 0b00010],
		')' => [0b01000, 0b00100, 0b00010, 0b00010, 0b00010, 0b00100, 0b01000],
		'\'' => [0b00100, 0b00100, 0b00000, 0b00000, 0b00000, 0b00000, 0b00000],
		' ' | _ => [0; 7],
	}
}

#[cfg(test)]
mod tests {
	#![allow(clippy::unwrap_used, clippy::expect_used)]

	use super::*;
	use crate::game::{Game, RuleSet};

	#[test]
	fn writes_a_png() {
		let game = Game::new(RuleSet::default(), 1);
		let path = std::env::temp_dir().join("apa-kan-render-test.png");
		write_png(&game, &path, "test").unwrap();
		assert!(path.is_file());
		let bytes = std::fs::read(&path).unwrap();
		assert!(bytes.len() > 100);
		assert_eq!(&bytes[0..8], &[137, 80, 78, 71, 13, 10, 26, 10]);
	}

	#[test]
	fn writes_midgame_at_window_sizes() {
		let game = crate::visual::demo_midgame();
		for (w, h) in [(1024_u32, 728_u32), (1280, 800), (1920, 1080)] {
			let path = std::env::temp_dir().join(format!("apa-kan-render-{w}x{h}.png"));
			write_png_sized(&game, &path, "mid-game visual fixture", w, h).unwrap();
			let bytes = std::fs::read(&path).unwrap();
			assert!(bytes.len() > 100);
			assert_eq!(&bytes[0..8], &[137, 80, 78, 71, 13, 10, 26, 10]);
		}
	}
}
