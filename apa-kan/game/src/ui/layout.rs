use crate::game::core::Position;
use bevy::prelude::*;

pub const WINDOW_WIDTH: f32 = 1280.0;
pub const WINDOW_HEIGHT: f32 = 900.0;
pub const BOARD_PX: f32 = 420.0;
pub const BOARD_Y: f32 = -36.0;
pub const BENCH_MAX_TILE: f32 = 56.0;
pub const BENCH_GAP: f32 = 6.0;

pub fn square_size(board_size: u8) -> f32 {
	BOARD_PX / board_size as f32
}

pub fn board_to_screen(position: Position, board_size: u8) -> Vec2 {
	let s = square_size(board_size);
	let center = (board_size - 1) as f32 / 2.0;
	Vec2::new((position.col as f32 - center) * s, (center - position.row as f32) * s + BOARD_Y)
}

pub fn screen_to_board(screen_pos: Vec2, board_size: u8) -> Option<Position> {
	let s = square_size(board_size);
	let n = board_size as f32;
	let center = (n - 1.0) / 2.0;
	let col_f = screen_pos.x / s + center;
	let row_f = center - (screen_pos.y - BOARD_Y) / s;
	if col_f < -0.5 || col_f > n - 0.5 || row_f < -0.5 || row_f > n - 0.5 {
		return None;
	}
	let col = col_f.round() as i8;
	let row = row_f.round() as i8;
	if row < 0 || col < 0 || row >= board_size as i8 || col >= board_size as i8 {
		None
	} else {
		Some(Position::new(row, col))
	}
}

pub fn board_half() -> f32 {
	BOARD_PX / 2.0
}

pub fn opponent_bench_y() -> f32 {
	board_half() + BOARD_Y + 78.0
}

pub fn human_bench_y() -> f32 {
	-board_half() + BOARD_Y - 78.0
}

pub fn bench_tile_size(count: usize) -> f32 {
	if count == 0 {
		return BENCH_MAX_TILE;
	}
	let available = WINDOW_WIDTH - 80.0;
	let size = (available / count as f32) - BENCH_GAP;
	size.clamp(28.0, BENCH_MAX_TILE)
}

pub fn bench_slot_x(index: usize, count: usize, tile_size: f32) -> f32 {
	let total = count as f32 * (tile_size + BENCH_GAP) - BENCH_GAP;
	let start = -total / 2.0 + tile_size / 2.0;
	start + index as f32 * (tile_size + BENCH_GAP)
}

pub fn p1_fill() -> Color {
	Color::srgb(0.98, 0.90, 0.70)
}

pub fn p2_fill() -> Color {
	Color::srgb(0.78, 0.72, 0.96)
}

pub fn p1_border() -> Color {
	Color::srgb(0.92, 0.48, 0.08)
}

pub fn p2_border() -> Color {
	Color::srgb(0.42, 0.22, 0.78)
}

pub fn add_green() -> Color {
	Color::srgb(0.18, 0.78, 0.38)
}

pub fn move_purple() -> Color {
	Color::srgb(0.62, 0.42, 0.95)
}

pub fn board_light() -> Color {
	Color::srgb(0.93, 0.88, 0.80)
}

pub fn board_dark() -> Color {
	Color::srgb(0.72, 0.58, 0.46)
}

pub fn corner_color() -> Color {
	Color::srgb(0.08, 0.08, 0.09)
}
