//! Headless PNG renderer for the geometric board, tiles, and both benches.

use std::path::Path;

use image::{Rgba, RgbaImage};

use crate::game::{Game, Player, Tile};
use crate::visual::{self, ArcBody, ViewLayout};

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
	write_png_view(game, path, caption, width, height, None)
}

pub fn write_png_view(
	game: &Game,
	path: impl AsRef<Path>,
	caption: &str,
	width: u32,
	height: u32,
	selected: Option<crate::board::NodeId>,
) -> Result<(), String> {
	let layout = ViewLayout::from_window(width as f32, height as f32);
	let bg = visual::rgb8(visual::BG_RGB);
	let mut img = RgbaImage::from_pixel(width, height, rgba(bg[0], bg[1], bg[2], 255));
	draw_arcs(&mut img, game, &layout);
	draw_nodes(&mut img, game, &layout);
	draw_tiles(&mut img, game, &layout);
	draw_marks(&mut img, game, &layout, selected);
	draw_benches(&mut img, game, &layout);
	draw_hud(&mut img, game, &layout, caption);
	img.save(path.as_ref()).map_err(|e| e.to_string())
}

fn draw_arcs(img: &mut RgbaImage, game: &Game, layout: &ViewLayout) {
	let board = game.board();
	let mut back: Vec<(Vec<[f64; 2]>, Rgba<u8>, f64)> = Vec::new();
	let mut front: Vec<(Vec<[f64; 2]>, Rgba<u8>, f64)> = Vec::new();
	for arc in board.arcs() {
		let samples = arc.sample(visual::ARC_SAMPLES);
		let pts: Vec<[f64; 2]> = samples.iter().map(|p| image_pt(layout, *p)).collect();
		let width = f64::from(layout.arc_width);
		let paint = visual::arc_paint(game.tile_at(arc.from), game.tile_at(arc.to));
		let stripe_off = if visual::arc_touches_center(arc, board.center()) {
			f64::from(layout.stripe_off_center)
		} else {
			f64::from(layout.stripe_off)
		};
		match paint.body {
			ArcBody::Neutral => {
				back.push((pts.clone(), rgb_of(visual::NEUTRAL_ARC_RGB), width));
			}
			ArcBody::Solid { color, thick } => {
				let w = if thick {
					width * f64::from(visual::ARC_SAME_COLOR_WIDTH_SCALE)
				} else {
					width
				};
				back.push((pts.clone(), rgb_of(visual::tile_rgb(color)), w));
			}
			ArcBody::Hitch { from, to } => {
				let hitch_r = if visual::arc_touches_center(arc, board.center()) {
					f64::from(layout.hitch_r_center)
				} else {
					f64::from(layout.hitch_r)
				};
				let frame = visual::midpoint_frame(&pts);
				let clearance = visual::hitch_clearance(hitch_r);
				if frame.index < pts.len() {
					let diamond =
						visual::conflict_diamond(frame.mid, frame.tangent, frame.normal, hitch_r);
					let from_pts = visual::shorten_polyline_end(&pts[..=frame.index], clearance);
					let to_pts = visual::shorten_polyline_start(&pts[frame.index..], clearance);
					push_hitch_half(
						&visual::diamond_from_path(&from_pts, &diamond),
						from,
						stripe_off,
						width,
						1.0,
						&mut back,
					);
					push_hitch_half(
						&visual::diamond_to_path(&to_pts, &diamond),
						to,
						stripe_off,
						width,
						-1.0,
						&mut back,
					);
				}
			}
			ArcBody::Stripes { a, b, zag_a, zag_b } => {
				let strand_w = width * f64::from(visual::STRIPE_STRAND_WIDTH_SCALE);
				let zag_w = width * f64::from(visual::ZAG_STRAND_WIDTH_SCALE);
				let zag_amp = if visual::arc_touches_center(arc, board.center()) {
					f64::from(layout.zag_amp_center)
				} else {
					f64::from(layout.zag_amp)
				};
				let teeth = visual::zag_teeth(arc);
				push_stripe(
					&pts,
					stripe_off,
					zag_a,
					rgb_of(visual::tile_rgb(a)),
					strand_w,
					zag_w,
					zag_amp,
					teeth,
					&mut back,
					&mut front,
				);
				push_stripe(
					&pts,
					-stripe_off,
					zag_b,
					rgb_of(visual::tile_rgb(b)),
					strand_w,
					zag_w,
					zag_amp,
					teeth,
					&mut back,
					&mut front,
				);
			}
			ArcBody::Hidden => {}
		}
		let zag_amp = if visual::arc_touches_center(arc, board.center()) {
			f64::from(layout.zag_amp_center)
		} else {
			f64::from(layout.zag_amp)
		};
		let zag_w = width * f64::from(visual::ZAG_STRAND_WIDTH_SCALE);
		let teeth = visual::zag_teeth(arc);
		let spacing = visual::zag_lane_spacing(stripe_off);
		let hitch_r = if visual::arc_touches_center(arc, board.center()) {
			f64::from(layout.hitch_r_center)
		} else {
			f64::from(layout.hitch_r)
		};
		let inner = visual::zag_lane_inner(paint.body, stripe_off, hitch_r);
		for (i, zag) in paint.zags_from.iter().enumerate() {
			let Some(color) = *zag else {
				continue;
			};
			let side = visual::zag_fan_offset(i, 1.0, inner, spacing);
			let zig = visual::zigzag_alongside(&pts, side, zag_amp, teeth);
			front.push((zig, rgb_of(visual::tile_rgb(color)), zag_w));
		}
		for (i, zag) in paint.zags_to.iter().enumerate() {
			let Some(color) = *zag else {
				continue;
			};
			let side = visual::zag_fan_offset(i, -1.0, inner, spacing);
			let zig = visual::zigzag_alongside(&pts, side, zag_amp, teeth);
			front.push((zig, rgb_of(visual::tile_rgb(color)), zag_w));
		}
	}
	for (pts, color, width) in back.into_iter().chain(front) {
		stroke_polyline(img, &pts, color, width);
	}
}

fn push_stripe(
	centerline: &[[f64; 2]],
	offset: f64,
	is_zag: bool,
	color: Rgba<u8>,
	strand_w: f64,
	zag_w: f64,
	zag_amp: f64,
	teeth: f64,
	back: &mut Vec<(Vec<[f64; 2]>, Rgba<u8>, f64)>,
	front: &mut Vec<(Vec<[f64; 2]>, Rgba<u8>, f64)>,
) {
	if is_zag {
		let zig = visual::zigzag_alongside(centerline, offset, zag_amp, teeth);
		front.push((zig, color, zag_w));
	} else {
		back.push((visual::offset_polyline(centerline, offset), color, strand_w));
	}
}

fn push_hitch_half(
	pts: &[[f64; 2]],
	colors: [Option<crate::game::TileColor>; visual::MAX_HITCH_COLORS],
	stripe_off: f64,
	arc_width: f64,
	outside: f64,
	back: &mut Vec<(Vec<[f64; 2]>, Rgba<u8>, f64)>,
) {
	let n = visual::packed_color_count(colors);
	if n == 0 || pts.len() < 2 {
		return;
	}
	let offsets = visual::diamond_bundle_offsets(n, stripe_off, outside);
	let strand_w = visual::diamond_strand_width(arc_width);
	let mut k = 0usize;
	for color in colors {
		let Some(c) = color else {
			continue;
		};
		back.push((
			visual::offset_polyline(pts, offsets[k]),
			rgb_of(visual::tile_rgb(c)),
			strand_w,
		));
		k += 1;
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

fn draw_marks(
	img: &mut RgbaImage,
	game: &Game,
	layout: &ViewLayout,
	selected: Option<crate::board::NodeId>,
) {
	let Some(from) = selected else {
		return;
	};
	let board = game.board();
	// Destinations follow RuleSet movement (pass-through default, hops optional).
	for id in game.legal_destinations(from) {
		let p = image_pt(layout, board.position(id));
		stroke_circle(
			img,
			tup(p),
			f64::from(layout.tile_radius + 5.0),
			rgb_of(visual::DEST_RGB),
			3.2,
		);
	}
	let p = image_pt(layout, board.position(from));
	stroke_circle(
		img,
		tup(p),
		f64::from(layout.tile_radius + 7.0),
		rgb_of(visual::HIGHLIGHT_RGB),
		3.6,
	);
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
	let primary = rgb_of(visual::tile_rgb(tile.primary()));
	let draw = visual::TileRingDraw::for_radius(r as f32);
	let strokes = tile.ring_strokes();
	for k in 0..strokes {
		let rr = f64::from(draw.ring_radius(r as f32, k));
		if rr > 4.0 * f64::from(draw.scale) {
			let color = tile.stroke_color(k);
			let width =
				f64::from(if tile.stroke_is_zag(k) { draw.zag_stroke } else { draw.stroke });
			if tile.stroke_is_zag(k) {
				let zig = visual::zigzag_circle(
					[c.0, c.1],
					rr,
					f64::from(draw.zag_amp),
					draw.zag_teeth,
					draw.zag_samples,
				);
				stroke_polyline(img, &zig, rgb_of(visual::tile_rgb(color)), width);
			} else {
				stroke_circle(img, c, rr, rgb_of(visual::tile_rgb(color)), width);
			}
		}
	}
	let dots = tile.dots.min(3);
	let spread = f64::from(draw.dot_spread);
	let start = -spread * (f64::from(dots) - 1.0) / 2.0;
	let dot_r = f64::from(draw.dot_r);
	for i in 0..dots {
		let dx = start + f64::from(i) * spread;
		fill_circle(img, (c.0 + dx, c.1), dot_r, primary);
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

	#[test]
	fn dump_hitch_and_zag_artifacts() {
		let dir = std::path::Path::new("/opt/cursor/artifacts");
		if !dir.is_dir() {
			return;
		}
		use crate::game::TileColor;
		use crate::visual::{
			demo_hop_turn, demo_left_cluster, demo_midgame, demo_pair, demo_zag_tile,
		};
		let board = demo_midgame();
		write_png_sized(
			&board,
			dir.join("zag_board_1280x800.png"),
			"diamonds + realised-state edges",
			1280,
			800,
		)
		.unwrap();
		write_png_sized(
			&board,
			dir.join("diamond_board_1280x800.png"),
			"conflict diamonds, full board",
			1280,
			800,
		)
		.unwrap();
		write_cropped(
			&demo_pair(
				&[(TileColor::Blue, false), (TileColor::Green, false)],
				2,
				&[(TileColor::Orange, false), (TileColor::Teal, false)],
				2,
			),
			dir.join("diamond_blue_green_vs_orange_teal.png"),
			"A blue+green vs B orange+teal diamond",
			36.0,
		);
		write_cropped(
			&demo_pair(&[(TileColor::Blue, false)], 2, &[(TileColor::Orange, false)], 2),
			dir.join("diamond_blue_vs_orange.png"),
			"blue vs orange diamond",
			36.0,
		);
		write_cropped(
			&demo_pair(
				&[(TileColor::Blue, false), (TileColor::Orange, true)],
				2,
				&[(TileColor::Green, false)],
				2,
			),
			dir.join("diamond_next_to_zags.png"),
			"diamond next to a non-hitting orange zag",
			48.0,
		);
		write_cropped(
			&demo_pair(
				&[(TileColor::Orange, false), (TileColor::Green, true)],
				2,
				&[(TileColor::Orange, false), (TileColor::Green, false)],
				2,
			),
			dir.join("zag_only_vs_orange_holder.png"),
			"green zag vs orange holder: zag only",
			48.0,
		);
		write_cropped(
			&demo_pair(
				&[(TileColor::Blue, true)],
				3,
				&[(TileColor::Orange, false), (TileColor::Green, false)],
				2,
			),
			dir.join("grey_no_effect_edge.png"),
			"blue zag vs orange+green: grey, no effect",
			48.0,
		);
		write_cropped(
			&demo_zag_tile(),
			dir.join("one_ended_potential_zags.png"),
			"one-ended potential zags",
			28.0,
		);
		write_png_sized(
			&demo_left_cluster(),
			dir.join("left_cluster_realized.png"),
			"left cluster, realized-state edges",
			1280,
			800,
		)
		.unwrap();
		let (hop, from) = demo_hop_turn();
		write_png_view(
			&hop,
			dir.join("hop_multidir_highlights.png"),
			"d=1 hop turns at center",
			1280,
			800,
			Some(from),
		)
		.unwrap();
		write_cropped(
			&demo_pair(
				&[(TileColor::Blue, true), (TileColor::Blue, true), (TileColor::Blue, true)],
				3,
				&[(TileColor::Orange, true), (TileColor::Orange, true), (TileColor::Orange, true)],
				3,
			),
			dir.join("zag_thickness_arc.png"),
			"thicker directional zags",
			48.0,
		);
		write_cropped(
			&demo_zag_tile(),
			dir.join("zag_thickness_tile.png"),
			"thicker tile zags",
			28.0,
		);
	}

	fn write_cropped(game: &Game, path: impl AsRef<std::path::Path>, caption: &str, pad: f32) {
		let w = 1280u32;
		let h = 800u32;
		let tmp = std::env::temp_dir().join("apa-kan-crop-src.png");
		write_png_sized(game, &tmp, caption, w, h).unwrap();
		let img = image::open(&tmp).unwrap().to_rgba8();
		let layout = crate::visual::ViewLayout::from_window(w as f32, h as f32);
		let (x, y, cw, ch) = occupied_crop(game, &layout, pad, w, h);
		image::imageops::crop_imm(&img, x, y, cw, ch)
			.to_image()
			.save(path.as_ref())
			.unwrap();
	}

	fn occupied_crop(
		game: &Game,
		layout: &crate::visual::ViewLayout,
		pad: f32,
		width: u32,
		height: u32,
	) -> (u32, u32, u32, u32) {
		let board = game.board();
		let mut minx = f32::MAX;
		let mut miny = f32::MAX;
		let mut maxx = f32::MIN;
		let mut maxy = f32::MIN;
		let bump = |x: f32,
		            y: f32,
		            r: f32,
		            minx: &mut f32,
		            miny: &mut f32,
		            maxx: &mut f32,
		            maxy: &mut f32| {
			*minx = minx.min(x - r);
			*miny = miny.min(y - r);
			*maxx = maxx.max(x + r);
			*maxy = maxy.max(y + r);
		};
		for id in 0..board.node_count() as u8 {
			if game.tile_at(id).is_none() {
				continue;
			}
			let p = layout.board_to_image(board.position(id));
			bump(p[0], p[1], layout.tile_radius + pad, &mut minx, &mut miny, &mut maxx, &mut maxy);
		}
		for arc in board.arcs() {
			if game.tile_at(arc.from).is_none() && game.tile_at(arc.to).is_none() {
				continue;
			}
			for p in arc.sample(24) {
				let q = layout.board_to_image(p);
				bump(q[0], q[1], pad, &mut minx, &mut miny, &mut maxx, &mut maxy);
			}
		}
		if !minx.is_finite() {
			return (0, 0, width, height);
		}
		let x = minx.max(0.0).floor() as u32;
		let y = miny.max(0.0).floor() as u32;
		let x2 = maxx.min(width as f32).ceil() as u32;
		let y2 = maxy.min(height as f32).ceil() as u32;
		(x, y, x2.saturating_sub(x).max(1), y2.saturating_sub(y).max(1))
	}
}
