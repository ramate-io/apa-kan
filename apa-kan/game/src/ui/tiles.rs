use crate::game::core::{Color as TileColor, Position, Tile};
use crate::ui::board::{board_to_screen_pos, GameResource};
use crate::ui::menu::GameScreen;
use bevy::prelude::*;

pub struct TileRenderPlugin;

impl Plugin for TileRenderPlugin {
	fn build(&self, app: &mut App) {
		app.add_systems(Update, render_tiles.run_if(in_state(GameScreen::Playing)))
			.add_systems(Update, render_stacks.run_if(in_state(GameScreen::Playing)));
	}
}

#[derive(Component)]
struct TileSprite {
	tile_id: u32,
}

#[derive(Component)]
struct StackDisplay;

fn render_tiles(
	mut commands: Commands,
	game: Res<GameResource>,
	query: Query<Entity, With<TileSprite>>,
	mut gizmos: Gizmos,
) {
	for entity in query.iter() {
		commands.entity(entity).despawn();
	}

	for (pos, tile) in game.0.board.tiles() {
		let screen_pos = board_to_screen_pos(*pos);
		spawn_tile_sprite(&mut commands, tile, screen_pos, &mut gizmos);
	}
}

fn spawn_tile_sprite(commands: &mut Commands, tile: &Tile, screen_pos: Vec2, gizmos: &mut Gizmos) {
	const TILE_SIZE: f32 = 80.0;

	commands.spawn((
		TileSprite { tile_id: tile.id },
		Sprite {
			color: Color::srgb(0.95, 0.95, 0.85),
			custom_size: Some(Vec2::new(TILE_SIZE, TILE_SIZE)),
			..default()
		},
		Transform::from_xyz(screen_pos.x, screen_pos.y, 1.0),
	));

	draw_tile_edges(gizmos, screen_pos, tile, TILE_SIZE);
}

fn draw_tile_edges(gizmos: &mut Gizmos, center: Vec2, tile: &Tile, size: f32) {
	let half = size / 2.0;
	let edge_offset = 3.0;

	let north_color = tile_color_to_bevy(tile.north.color);
	let north_weight = tile.north.weight.value();
	draw_edge(
		gizmos,
		Vec2::new(center.x - half + edge_offset, center.y + half),
		Vec2::new(center.x + half - edge_offset, center.y + half),
		north_color,
		north_weight,
	);

	let east_color = tile_color_to_bevy(tile.east.color);
	let east_weight = tile.east.weight.value();
	draw_edge(
		gizmos,
		Vec2::new(center.x + half, center.y + half - edge_offset),
		Vec2::new(center.x + half, center.y - half + edge_offset),
		east_color,
		east_weight,
	);

	let south_color = tile_color_to_bevy(tile.south.color);
	let south_weight = tile.south.weight.value();
	draw_edge(
		gizmos,
		Vec2::new(center.x + half - edge_offset, center.y - half),
		Vec2::new(center.x - half + edge_offset, center.y - half),
		south_color,
		south_weight,
	);

	let west_color = tile_color_to_bevy(tile.west.color);
	let west_weight = tile.west.weight.value();
	draw_edge(
		gizmos,
		Vec2::new(center.x - half, center.y - half + edge_offset),
		Vec2::new(center.x - half, center.y + half - edge_offset),
		west_color,
		west_weight,
	);
}

fn draw_edge(gizmos: &mut Gizmos, start: Vec2, end: Vec2, color: Color, weight: u8) {
	if weight == 1 {
		gizmos.line_2d(start, end, color);
	} else {
		let perpendicular = (end - start).perp().normalize() * 1.5;
		gizmos.line_2d(start + perpendicular, end + perpendicular, color);
		gizmos.line_2d(start - perpendicular, end - perpendicular, color);
	}
}

fn tile_color_to_bevy(color: TileColor) -> Color {
	match color {
		TileColor::Maroon => Color::srgb(0.5, 0.0, 0.0),
		TileColor::Green => Color::srgb(0.0, 0.5, 0.0),
		TileColor::Blue => Color::srgb(0.0, 0.0, 0.8),
		TileColor::Orange => Color::srgb(1.0, 0.5, 0.0),
	}
}

fn render_stacks(mut gizmos: Gizmos, _game: Res<GameResource>) {
	let stack_offset_x = 350.0;

	gizmos.rect_2d(
		Isometry2d::from_translation(Vec2::new(-stack_offset_x, 200.0)),
		Vec2::new(120.0, 60.0),
		Color::srgb(0.3, 0.3, 0.3),
	);

	gizmos.rect_2d(
		Isometry2d::from_translation(Vec2::new(stack_offset_x, 200.0)),
		Vec2::new(120.0, 60.0),
		Color::srgb(0.3, 0.3, 0.3),
	);
}
