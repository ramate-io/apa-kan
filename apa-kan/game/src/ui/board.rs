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
const SQUARE_SIZE: f32 = BOARD_SIZE / 5.0;

fn setup_board(mut commands: Commands) {
	for row in 0..5 {
		for col in 0..5 {
			let pos = Position::new(row, col);
			let x = (col as f32 - 2.0) * SQUARE_SIZE;
			let y = (2.0 - row as f32) * SQUARE_SIZE;

			let color = if pos.is_corner() {
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
					custom_size: Some(Vec2::new(SQUARE_SIZE - 2.0, SQUARE_SIZE - 2.0)),
					..default()
				},
				Transform::from_xyz(x, y, 0.0),
			));

			if pos.is_corner() {
				commands.spawn((
					BoardEntity,
					Text::new("✗"),
					TextFont { font_size: FontSize::Px(40.0), ..default() },
					TextColor(Color::srgb(0.5, 0.0, 0.0)),
					Node {
						position_type: PositionType::Absolute,
						left: Val::Px(400.0 + x - 15.0),
						top: Val::Px(300.0 - y - 20.0),
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

pub fn board_to_screen_pos(position: Position) -> Vec2 {
	let x = (position.col as f32 - 2.0) * SQUARE_SIZE;
	let y = (2.0 - position.row as f32) * SQUARE_SIZE;
	Vec2::new(x, y)
}
