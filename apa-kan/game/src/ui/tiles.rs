use crate::config::GameConfig;
use crate::game::actions::generate_legal_actions;
use crate::game::core::{Color as TileColor, EdgeWeight, Player, Position, Tile};
use crate::game::ActionType;
use crate::ui::board::{BotState, GameResource};
use crate::ui::layout::{
	self, add_green, bench_slot_x, bench_tile_size, board_to_screen, human_bench_y,
	opponent_bench_y, p1_border, p1_fill, p2_border, p2_fill, square_size,
};
use crate::ui::menu::GameScreen;
use crate::ui::state::{actor_label, human_player, is_human_turn, InputState, Selection};
use bevy::prelude::*;
use bevy::sprite::Anchor;

pub struct TileRenderPlugin;

impl Plugin for TileRenderPlugin {
	fn build(&self, app: &mut App) {
		app.add_systems(Update, refresh_dynamic_view.run_if(in_state(GameScreen::Playing)));
	}
}

#[derive(Component)]
struct DynamicView;

#[derive(Component)]
pub struct BenchTile {
	pub player: Player,
	pub index: usize,
}

#[derive(Resource, Default)]
struct ViewFingerprint(u64);

fn refresh_dynamic_view(
	mut commands: Commands,
	game: Res<GameResource>,
	config: Res<GameConfig>,
	input: Res<InputState>,
	bot: Res<BotState>,
	mut meshes: ResMut<Assets<Mesh>>,
	mut materials: ResMut<Assets<ColorMaterial>>,
	mut fingerprint: Local<ViewFingerprint>,
	existing: Query<Entity, With<DynamicView>>,
) {
	let fp = view_fingerprint(&game, &config, &input, &bot);
	if fp == fingerprint.0 && !existing.is_empty() {
		return;
	}
	fingerprint.0 = fp;
	for entity in existing.iter() {
		commands.entity(entity).despawn();
	}

	draw_last_move(&mut commands, &game, &input);
	draw_board_tiles(&mut commands, &game);
	draw_benches(&mut commands, &game, &config, &input);
	draw_markers(&mut commands, &mut meshes, &mut materials, &game, &config, &input, &bot);
}

fn view_fingerprint(
	game: &GameResource,
	config: &GameConfig,
	input: &InputState,
	bot: &BotState,
) -> u64 {
	use std::hash::{Hash, Hasher};
	let mut h = std::collections::hash_map::DefaultHasher::new();
	game.0.turn_count.hash(&mut h);
	game.0.current_player.hash(&mut h);
	game.0.add_bank_p1.hash(&mut h);
	game.0.add_bank_p2.hash(&mut h);
	game.0.stack(Player::One).len().hash(&mut h);
	game.0.stack(Player::Two).len().hash(&mut h);
	for (pos, tile) in game.0.board.tiles() {
		pos.row.hash(&mut h);
		pos.col.hash(&mut h);
		tile.id.hash(&mut h);
		tile.owner.hash(&mut h);
		tile.north.color.hash(&mut h);
		tile.east.color.hash(&mut h);
		tile.south.color.hash(&mut h);
		tile.west.color.hash(&mut h);
	}
	std::mem::discriminant(&input.selection).hash(&mut h);
	match input.selection {
		Selection::PendingAdd { position } => {
			position.row.hash(&mut h);
			position.col.hash(&mut h);
		}
		Selection::MoveTile { tile_id, from } => {
			tile_id.hash(&mut h);
			from.row.hash(&mut h);
			from.col.hash(&mut h);
		}
		Selection::None => {}
	}
	input.rotation.hash(&mut h);
	input.last_move.as_ref().map(|m| m.text.clone()).hash(&mut h);
	bot.thinking.hash(&mut h);
	config.human_seat.hash(&mut h);
	h.finish()
}

fn draw_board_tiles(commands: &mut Commands, game: &GameResource) {
	let size = square_size(game.0.board.size()) * 0.86;
	for (pos, tile) in game.0.board.tiles() {
		let center = board_to_screen(*pos, game.0.board.size());
		spawn_tile(commands, tile, center, size, 1.5, None);
	}
}

fn draw_benches(
	commands: &mut Commands,
	game: &GameResource,
	config: &GameConfig,
	input: &InputState,
) {
	let human = human_player(config);
	let (top_player, bottom_player) = match human {
		Some(Player::Two) => (Player::One, Player::Two),
		_ => (Player::Two, Player::One),
	};

	spawn_bench_row(commands, game, config, input, top_player, opponent_bench_y(), false);
	spawn_bench_row(commands, game, config, input, bottom_player, human_bench_y(), true);
}

fn spawn_bench_row(
	commands: &mut Commands,
	game: &GameResource,
	config: &GameConfig,
	input: &InputState,
	player: Player,
	y: f32,
	is_bottom: bool,
) {
	let stack = game.0.stack(player);
	let count = stack.len().max(1);
	let tile_size = bench_tile_size(stack.len().max(1));
	let picking = matches!(input.selection, Selection::PendingAdd { .. })
		&& game.0.current_player == player
		&& is_human_turn(config, player);

	let label_y = if is_bottom { y - tile_size * 0.72 } else { y + tile_size * 0.72 };
	let label = format!(
		"{} | {} tiles in hand | {} ADDs left",
		actor_label(config, player),
		stack.len(),
		match player {
			Player::One => game.0.add_bank_p1,
			Player::Two => game.0.add_bank_p2,
		}
	);
	spawn_world_label(
		commands,
		&label,
		Vec2::new(0.0, label_y),
		18.0,
		Color::srgb(0.95, 0.95, 0.95),
	);

	if picking {
		let width = (count as f32) * (tile_size + layout::BENCH_GAP) + 24.0;
		commands.spawn((
			DynamicView,
			Sprite {
				color: Color::srgba(0.25, 0.85, 0.40, 0.22),
				custom_size: Some(Vec2::new(width, tile_size + 22.0)),
				..default()
			},
			Transform::from_xyz(0.0, y, 1.2),
		));
		spawn_world_label(
			commands,
			"Click a bench tile to place it",
			Vec2::new(0.0, y + if is_bottom { 42.0 } else { -42.0 }),
			16.0,
			add_green(),
		);
	}

	if stack.is_empty() {
		spawn_world_label(
			commands,
			"(empty)",
			Vec2::new(0.0, y),
			16.0,
			Color::srgb(0.55, 0.55, 0.55),
		);
		return;
	}

	for (index, tile) in stack.iter().enumerate() {
		let x = bench_slot_x(index, stack.len(), tile_size);
		spawn_tile(
			commands,
			tile,
			Vec2::new(x, y),
			tile_size,
			2.0,
			Some(BenchTile { player, index }),
		);
	}
}

fn draw_markers(
	commands: &mut Commands,
	meshes: &mut Assets<Mesh>,
	materials: &mut Assets<ColorMaterial>,
	game: &GameResource,
	config: &GameConfig,
	input: &InputState,
	bot: &BotState,
) {
	if bot.thinking || game.0.is_game_over() {
		return;
	}
	if !is_human_turn(config, game.0.current_player) {
		return;
	}

	let sq = square_size(game.0.board.size());
	let legal = generate_legal_actions(&game.0);

	match &input.selection {
		Selection::None => {
			let bank = match game.0.current_player {
				Player::One => game.0.add_bank_p1,
				Player::Two => game.0.add_bank_p2,
			};
			if bank == 0 || game.0.stack(game.0.current_player).is_empty() {
				return;
			}
			let mut seen = std::collections::HashSet::new();
			for action in legal {
				if let ActionType::Add { position, .. } = action.action_type {
					if seen.insert(position) {
						spawn_add_marker(
							commands,
							meshes,
							materials,
							position,
							game.0.board.size(),
							sq,
						);
					}
				}
			}
		}
		Selection::PendingAdd { position } => {
			spawn_add_marker(commands, meshes, materials, *position, game.0.board.size(), sq);
		}
		Selection::MoveTile { tile_id, from } => {
			let mut seen = std::collections::HashSet::new();
			for action in legal {
				if let ActionType::Move { tile_id: id, destination, rotation_count } =
					action.action_type
				{
					if id == *tile_id
						&& rotation_count == input.rotation
						&& seen.insert(destination)
					{
						spawn_move_marker(commands, destination, game.0.board.size(), sq);
					}
				}
			}
			let hint = if game.0.rule_set.fixed_rotation {
				format!("Press R to rotate ({} of 1)", input.rotation)
			} else {
				format!("Press R to rotate ({}/3)", input.rotation)
			};
			let center = board_to_screen(*from, game.0.board.size());
			spawn_world_label(
				commands,
				&hint,
				Vec2::new(center.x, center.y + sq * 0.62),
				14.0,
				Color::srgb(1.0, 0.92, 0.45),
			);
		}
	}
}

fn spawn_add_marker(
	commands: &mut Commands,
	meshes: &mut Assets<Mesh>,
	materials: &mut Assets<ColorMaterial>,
	position: Position,
	board_size: u8,
	sq: f32,
) {
	let center = board_to_screen(position, board_size);
	let radius = sq * 0.32;
	commands.spawn((
		DynamicView,
		Mesh2d(meshes.add(Circle::new(radius))),
		MeshMaterial2d(materials.add(Color::srgb(0.18, 0.78, 0.38))),
		Transform::from_xyz(center.x, center.y, 2.4),
	));
	spawn_world_label(commands, "ADD", center, (sq * 0.22).clamp(12.0, 20.0), Color::WHITE);
}

fn spawn_move_marker(commands: &mut Commands, position: Position, board_size: u8, sq: f32) {
	let center = board_to_screen(position, board_size);
	commands.spawn((
		DynamicView,
		Sprite {
			color: Color::srgba(0.62, 0.42, 0.95, 0.90),
			custom_size: Some(Vec2::splat(sq * 0.78)),
			..default()
		},
		Transform::from_xyz(center.x, center.y, 2.4),
	));
	spawn_world_label(commands, "MOVE", center, (sq * 0.18).clamp(11.0, 18.0), Color::WHITE);
}

fn draw_last_move(commands: &mut Commands, game: &GameResource, input: &InputState) {
	let Some(last) = &input.last_move else {
		return;
	};
	let sq = square_size(game.0.board.size());
	for pos in &last.squares {
		let center = board_to_screen(*pos, game.0.board.size());
		commands.spawn((
			DynamicView,
			Sprite {
				color: Color::srgba(1.0, 0.85, 0.15, 0.35),
				custom_size: Some(Vec2::splat(sq * 0.96)),
				..default()
			},
			Transform::from_xyz(center.x, center.y, 0.6),
		));
	}
}

fn spawn_world_label(commands: &mut Commands, text: &str, pos: Vec2, size: f32, color: Color) {
	commands.spawn((
		DynamicView,
		Text2d::new(text),
		TextFont { font_size: FontSize::Px(size), ..default() },
		TextColor(color),
		TextLayout::justify(Justify::Center),
		Anchor::CENTER,
		Transform::from_xyz(pos.x, pos.y, 3.5),
	));
}

fn spawn_tile(
	commands: &mut Commands,
	tile: &Tile,
	center: Vec2,
	size: f32,
	z: f32,
	bench: Option<BenchTile>,
) {
	let (fill, border) = match tile.owner {
		Player::One => (p1_fill(), p1_border()),
		Player::Two => (p2_fill(), p2_border()),
	};

	let mut entity = commands.spawn((
		DynamicView,
		Sprite { color: border, custom_size: Some(Vec2::splat(size)), ..default() },
		Transform::from_xyz(center.x, center.y, z),
	));
	if let Some(bench) = bench {
		entity.insert(bench);
	}
	entity.with_children(|parent| {
		parent.spawn((
			Sprite { color: fill, custom_size: Some(Vec2::splat(size * 0.86)), ..default() },
			Transform::from_xyz(0.0, 0.0, 0.05),
		));
		spawn_edge(parent, tile.north, size, 0.0, 1.0);
		spawn_edge(parent, tile.east, size, 1.0, 0.0);
		spawn_edge(parent, tile.south, size, 0.0, -1.0);
		spawn_edge(parent, tile.west, size, -1.0, 0.0);
	});
}

fn spawn_edge(
	parent: &mut ChildSpawnerCommands,
	edge: crate::game::core::Edge,
	tile_size: f32,
	dir_x: f32,
	dir_y: f32,
) {
	let color = tile_color_to_bevy(edge.color);
	let thickness = (tile_size * 0.11).clamp(5.0, 9.0);
	let length = tile_size * 0.78;
	let inset = tile_size * 0.42;
	let bars = match edge.weight {
		EdgeWeight::Single => 1,
		EdgeWeight::Double => 2,
	};
	let gap = thickness * 0.95;

	for i in 0..bars {
		let (w, h, x, y) = if dir_y.abs() > 0.0 {
			(length, thickness, 0.0, dir_y * inset - dir_y * (i as f32) * gap)
		} else {
			(thickness, length, dir_x * inset - dir_x * (i as f32) * gap, 0.0)
		};
		parent.spawn((
			Sprite { color, custom_size: Some(Vec2::new(w, h)), ..default() },
			Transform::from_xyz(x, y, 0.2),
		));
	}
}

fn tile_color_to_bevy(color: TileColor) -> Color {
	match color {
		TileColor::Maroon => Color::srgb(0.86, 0.07, 0.14),
		TileColor::Green => Color::srgb(0.12, 0.78, 0.22),
		TileColor::Blue => Color::srgb(0.10, 0.38, 0.98),
		TileColor::Orange => Color::srgb(1.00, 0.52, 0.04),
	}
}
