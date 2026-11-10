//! Headless PNG renderer for the geometric board, tiles, and both benches.

use std::path::Path;

use image::{Rgba, RgbaImage};

use crate::game::{Game, Player, Tile, TileColor};

const W: u32 = 1280;
const H: u32 = 960;
const CX: f64 = 640.0;
const CY: f64 = 490.0;
const SCALE: f64 = 168.0;

pub fn write_png(game: &Game, path: impl AsRef<Path>, caption: &str) -> Result<(), String> {
	let mut img = RgbaImage::from_pixel(W, H, rgba(18, 20, 26, 255));
	draw_board(&mut img, game);
	draw_benches(&mut img, game);
	draw_text(&mut img, 24, 20, "APA KAN  ·  8-PETAL ROSETTE", rgba(230, 230, 220, 255));
	draw_text(&mut img, 24, 42, caption, rgba(170, 176, 186, 255));
	let p2 = format!(
		"P2 DARK  bench {}  bag {}",
		game.bench(Player::Two).len(),
		game.bag_len(Player::Two)
	);
	let p1 = format!(
		"P1 LIGHT  bench {}  bag {}",
		game.bench(Player::One).len(),
		game.bag_len(Player::One)
	);
	draw_text(&mut img, 24, 78, &p2, rgba(210, 210, 200, 255));
	draw_text(&mut img, 24, H - 36, &p1, rgba(210, 210, 200, 255));
	draw_text(
		&mut img,
		24,
		H - 18,
		"Yellow nodes are spawns. Rings = weight, dots = move steps. Fill = owner.",
		rgba(140, 146, 156, 255),
	);
	img.save(path.as_ref()).map_err(|e| e.to_string())
}

fn draw_board(img: &mut RgbaImage, game: &Game) {
	let board = game.board();
	let arc_color = rgba(90, 96, 108, 255);
	for arc in board.arcs() {
		let pts = arc.sample(28);
		for w in pts.windows(2) {
			let a = world_to_px(w[0]);
			let b = world_to_px(w[1]);
			draw_line(img, a, b, arc_color, 2.0);
		}
	}
	for id in 0..board.node_count() as u8 {
		let p = world_to_px(board.position(id));
		if let Some(tile) = game.tile_at(id) {
			draw_tile(img, p, 22.0, tile);
		} else if board.is_spawn(id) {
			fill_circle(img, p, 9.0, rgba(238, 214, 110, 255));
			stroke_circle(img, p, 9.0, rgba(40, 36, 20, 255), 1.6);
		} else if id == board.center() {
			fill_circle(img, p, 8.0, rgba(16, 16, 18, 255));
			stroke_circle(img, p, 8.0, rgba(220, 220, 220, 255), 1.8);
		} else {
			fill_circle(img, p, 6.5, rgba(12, 12, 14, 255));
			stroke_circle(img, p, 6.5, rgba(200, 200, 200, 255), 1.5);
		}
	}
}

fn draw_benches(img: &mut RgbaImage, game: &Game) {
	draw_bench_row(img, game, Player::Two, 118);
	draw_bench_row(img, game, Player::One, H - 88);
}

fn draw_bench_row(img: &mut RgbaImage, game: &Game, player: Player, y: u32) {
	let bench = game.bench(player);
	let n = bench.len().max(1);
	let spacing = 56.0;
	let total = spacing * (bench.len().saturating_sub(1) as f64);
	let start = (W as f64 / 2.0) - total / 2.0;
	let _ = n;
	for (i, tile) in bench.iter().enumerate() {
		let x = start + i as f64 * spacing;
		draw_tile(img, (x, y as f64), 24.0, *tile);
	}
}

fn draw_tile(img: &mut RgbaImage, c: (f64, f64), r: f64, tile: Tile) {
	let fill = match tile.owner {
		Player::One => rgba(232, 228, 214, 255),
		Player::Two => rgba(28, 28, 32, 255),
	};
	fill_circle(img, c, r, fill);
	let (col, ring) = color_of(tile.color);
	for k in 0..tile.weight {
		let rr = r - 2.0 - f64::from(k) * 3.2;
		if rr > 4.0 {
			stroke_circle(img, c, rr, col, 2.2);
		}
	}
	let _ = ring;
	let dots = tile.dots.min(3);
	let spread = 7.0;
	let start = -spread * (f64::from(dots) - 1.0) / 2.0;
	for i in 0..dots {
		let dx = start + f64::from(i) * spread;
		fill_circle(img, (c.0 + dx, c.1), 2.6, col);
	}
}

fn color_of(color: TileColor) -> (Rgba<u8>, Rgba<u8>) {
	match color {
		TileColor::Green => (rgba(46, 168, 82, 255), rgba(46, 168, 82, 255)),
		TileColor::Blue => (rgba(62, 122, 220, 255), rgba(62, 122, 220, 255)),
		TileColor::Orange => (rgba(232, 132, 42, 255), rgba(232, 132, 42, 255)),
	}
}

fn world_to_px(p: [f64; 2]) -> (f64, f64) {
	(CX + p[0] * SCALE, CY - p[1] * SCALE)
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
}
