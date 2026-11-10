use crate::game::core::{Game, Position};
use crate::ui::menu::GameScreen;
use bevy::prelude::*;

#[derive(Resource)]
pub struct GameResource(pub Game);

pub struct BoardPlugin;

impl Plugin for BoardPlugin {
	fn build(&self, app: &mut App) {
		app.insert_resource(GameResource(Game::new()))
			.add_systems(OnEnter(GameScreen::Playing), setup_board)
			.add_systems(Update, update_board.run_if(in_state(GameScreen::Playing)));
	}
}

#[derive(Component)]
struct BoardSquare {
	position: Position,
}

#[derive(Component)]
struct BoardEntity;

const BOARD_SIZE: f32 = 500.0;

fn setup_board(mut commands: Commands, game: Res<GameResource>) {
	let board_size = game.0.board.size() as i8;
	let square_size = BOARD_SIZE / board_size as f32;
	let center = (board_size - 1) as f32 / 2.0;

	for row in 0..board_size {
		for col in 0..board_size {
			let pos = Position::new(row, col);
			let x = (col as f32 - center) * square_size;
			let y = (center - row as f32) * square_size;

			let color = if game.0.board.is_corner(pos) {
				Color::srgb(0.2, 0.2, 0.2)
			} else if (row + col) % 2 == 0 {
				Color::srgb(0.9, 0.8, 0.7)
			} else {
				Color::srgb(0.8, 0.7, 0.6)
			};

			commands.spawn((
				BoardEntity,
				BoardSquare { position: pos },
			Sprite {
				color,
				custom_size: Some(Vec2::new(square_size - 2.0, square_size - 2.0)),
				..default()
			},
				Transform::from_xyz(x, y, 0.0),
			));

		if game.0.board.is_corner(pos) {
			let font_size = (square_size * 0.8).min(40.0);
			commands.spawn((
				BoardEntity,
				Text::new("✗"),
					TextFont { font_size: FontSize::Px(font_size), ..default() },
					TextColor(Color::srgb(0.5, 0.0, 0.0)),
					Node {
						position_type: PositionType::Absolute,
						left: Val::Px(400.0 + x - font_size * 0.375),
						top: Val::Px(300.0 - y - font_size * 0.5),
						..default()
					},
					TextLayout { justify: Justify::Center, ..default() },
				));
			}
		}
	}

	commands.spawn((
		BoardEntity,
		Text::new("Player 1's Turn"),
		TextFont { font_size: FontSize::Px(30.0), ..default() },
		TextColor(Color::srgb(1.0, 1.0, 1.0)),
		Node {
			position_type: PositionType::Absolute,
			top: Val::Px(20.0),
			width: Val::Percent(100.0),
			..default()
		},
		TextLayout { justify: Justify::Center, ..default() },
	));
}

fn update_board() {}

pub fn board_to_screen_pos(position: Position, board_size: u8) -> Vec2 {
	let square_size = BOARD_SIZE / board_size as f32;
	let center = (board_size - 1) as f32 / 2.0;
	let x = (position.col as f32 - center) * square_size;
	let y = (center - position.row as f32) * square_size;
	Vec2::new(x, y)
}

pub fn square_size(board_size: u8) -> f32 {
	BOARD_SIZE / board_size as f32
}
