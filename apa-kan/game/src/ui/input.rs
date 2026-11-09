use crate::game::actions::{generate_legal_actions, has_legal_add_or_move, Action, ActionType};
use crate::game::core::{Game, GameState, Player, Position};
use crate::game::rules::Rules;
use crate::ui::board::{board_to_screen_pos, GameResource};
use crate::ui::menu::GameScreen;
use bevy::prelude::*;
use std::time::Duration;

pub struct InputPlugin;

impl Plugin for InputPlugin {
	fn build(&self, app: &mut App) {
		app.insert_resource(InputState::default()).add_systems(
			Update,
			(handle_input, update_highlights, update_turn_display)
				.run_if(in_state(GameScreen::Playing)),
		);
	}
}

#[derive(Resource, Default)]
struct InputState {
	selection: Option<Selection>,
	hold_timer: Option<Timer>,
	rotation_mode: Option<RotationMode>,
	feedback_message: Option<(String, Timer)>,
}

#[derive(Clone)]
enum Selection {
	StackTile { player: Player, index: usize },
	BoardTile { tile_id: u32, position: Position },
}

#[derive(Clone)]
struct RotationMode {
	tile_id: u32,
	position: Position,
	rotations: u8,
	original_north_color: crate::game::core::Color,
}

#[derive(Component)]
struct Highlight;

#[derive(Component)]
struct TurnIndicator;

#[derive(Component)]
struct FeedbackText;

const HOLD_DURATION: Duration = Duration::from_millis(500);
const TILE_SIZE: f32 = 80.0;

fn handle_input(
	mut commands: Commands,
	mut input_state: ResMut<InputState>,
	mut game: ResMut<GameResource>,
	mouse: Res<ButtonInput<MouseButton>>,
	touches: Res<Touches>,
	windows: Query<&Window>,
	time: Res<Time>,
	camera: Query<(&Camera, &GlobalTransform)>,
) {
	if game.0.is_game_over() {
		return;
	}

	if let Some((_, ref mut timer)) = input_state.feedback_message {
		timer.tick(time.delta());
		if timer.just_finished() {
			input_state.feedback_message = None;
		}
	}

	if let Some(ref mut timer) = input_state.hold_timer {
		timer.tick(time.delta());

		if timer.just_finished() {
			if let Some(Selection::BoardTile { tile_id, position }) = &input_state.selection {
				let tile = game.0.board.get(*position);
				if let Some(tile) = tile {
					input_state.rotation_mode = Some(RotationMode {
						tile_id: *tile_id,
						position: *position,
						rotations: 0,
						original_north_color: tile.north.color,
					});
					input_state.hold_timer = None;
					input_state.selection = None;
				}
			}
		}
	}

	let click_pos = get_click_position(&mouse, &touches, &windows, &camera);

	if mouse.just_pressed(MouseButton::Left) || touches.any_just_pressed() {
		if let Some(pos) = click_pos {
			handle_click(&mut commands, &mut input_state, &mut game, pos);
		}
	}

	if mouse.just_released(MouseButton::Left) || touches.any_just_released() {
		input_state.hold_timer = None;
	}

	if mouse.pressed(MouseButton::Left)
		|| touches.iter().next().is_some() && input_state.hold_timer.is_some()
	{
	} else if mouse.just_released(MouseButton::Left) || touches.any_just_released() {
		input_state.hold_timer = None;
	}
}

fn get_click_position(
	mouse: &Res<ButtonInput<MouseButton>>,
	touches: &Res<Touches>,
	windows: &Query<&Window>,
	camera: &Query<(&Camera, &GlobalTransform)>,
) -> Option<Vec2> {
	if let Some(touch) = touches.iter().next() {
		return Some(touch.position());
	}

	if mouse.pressed(MouseButton::Left) {
		if let Ok(window) = windows.single() {
			if let Some(cursor_pos) = window.cursor_position() {
				if let Ok((camera, camera_transform)) = camera.single() {
					if let Ok(world_pos) = camera.viewport_to_world_2d(camera_transform, cursor_pos)
					{
						return Some(world_pos);
					}
				}
			}
		}
	}

	None
}

fn handle_click(
	_commands: &mut Commands,
	input_state: &mut InputState,
	game: &mut GameResource,
	click_pos: Vec2,
) {
	if let Some(ref mut rotation_mode) = input_state.rotation_mode {
		let tile_pos = board_to_screen_pos(rotation_mode.position);
		let distance = click_pos.distance(tile_pos);

		if distance < TILE_SIZE / 2.0 {
			rotation_mode.rotations = (rotation_mode.rotations + 1) % 4;

			let action = Action::rotate(game.0.current_player, rotation_mode.tile_id, true);
			if let Err(e) = Rules::apply_action(&mut game.0, action) {
				set_feedback(input_state, format!("Cannot rotate: {}", e));
			}
		} else {
			if rotation_mode.rotations == 0 {
				set_feedback(input_state, "Cannot end in starting orientation".to_string());
			} else {
				input_state.rotation_mode = None;
			}
		}
		return;
	}

	let board_pos = screen_to_board_pos(click_pos);
	if let Some(pos) = board_pos {
		if let Some(selection) = &input_state.selection {
			match selection {
				Selection::StackTile { index, .. } => {
					let action = Action::add(game.0.current_player, *index, pos);
					match Rules::apply_action(&mut game.0, action) {
						Ok(_) => {
							input_state.selection = None;
						}
						Err(e) => {
							set_feedback(input_state, format!("Cannot place: {}", e));
						}
					}
				}
				Selection::BoardTile { tile_id, .. } => {
					let action = Action::move_tile(game.0.current_player, *tile_id, pos);
					match Rules::apply_action(&mut game.0, action) {
						Ok(_) => {
							input_state.selection = None;
						}
						Err(e) => {
							set_feedback(input_state, format!("Cannot move: {}", e));
						}
					}
				}
			}
		} else {
			if let Some(tile) = game.0.board.get(pos) {
				if tile.owner == game.0.current_player {
					input_state.selection =
						Some(Selection::BoardTile { tile_id: tile.id, position: pos });
					input_state.hold_timer = Some(Timer::new(HOLD_DURATION, TimerMode::Once));
				}
			}
		}
	} else {
		let stack_selection = check_stack_click(click_pos, &game.0);
		if let Some(selection) = stack_selection {
			input_state.selection = Some(selection);
		} else {
			input_state.selection = None;
			input_state.rotation_mode = None;
		}
	}
}

fn screen_to_board_pos(screen_pos: Vec2) -> Option<Position> {
	const SQUARE_SIZE: f32 = 500.0 / 5.0;

	let col = ((screen_pos.x / SQUARE_SIZE + 2.5).floor() as i8).clamp(0, 4);
	let row = ((2.5 - screen_pos.y / SQUARE_SIZE).floor() as i8).clamp(0, 4);

	let pos = Position::new(row, col);
	if pos.is_valid() {
		Some(pos)
	} else {
		None
	}
}

fn check_stack_click(click_pos: Vec2, game: &Game) -> Option<Selection> {
	const STACK_OFFSET_X: f32 = 350.0;

	if (click_pos.x + STACK_OFFSET_X).abs() < 60.0 && (click_pos.y - 200.0).abs() < 30.0 {
		let stack = game.stack(Player::One);
		if !stack.is_empty() && game.current_player == Player::One {
			return Some(Selection::StackTile { player: Player::One, index: 0 });
		}
	}

	if (click_pos.x - STACK_OFFSET_X).abs() < 60.0 && (click_pos.y - 200.0).abs() < 30.0 {
		let stack = game.stack(Player::Two);
		if !stack.is_empty() && game.current_player == Player::Two {
			return Some(Selection::StackTile { player: Player::Two, index: 0 });
		}
	}

	None
}

fn set_feedback(input_state: &mut InputState, message: String) {
	input_state.feedback_message = Some((message, Timer::from_seconds(2.0, TimerMode::Once)));
}

fn update_highlights(
	mut commands: Commands,
	input_state: Res<InputState>,
	game: Res<GameResource>,
	highlights: Query<Entity, With<Highlight>>,
	mut gizmos: Gizmos,
) {
	for entity in highlights.iter() {
		commands.entity(entity).despawn();
	}

	if let Some(ref selection) = input_state.selection {
		let legal_actions = generate_legal_actions(&game.0);

		match selection {
			Selection::StackTile { index, .. } => {
				for action in legal_actions {
					if let ActionType::Add { tile_index, position } = action.action_type {
						if tile_index == *index {
							let screen_pos = board_to_screen_pos(position);
							gizmos.circle_2d(
								Isometry2d::from_translation(screen_pos),
								40.0,
								Color::srgba(0.0, 1.0, 0.0, 0.3),
							);
						}
					}
				}
			}
			Selection::BoardTile { tile_id, .. } => {
				for action in legal_actions {
					if let ActionType::Move { tile_id: move_tile_id, destination } =
						action.action_type
					{
						if move_tile_id == *tile_id {
							let screen_pos = board_to_screen_pos(destination);
							gizmos.circle_2d(
								Isometry2d::from_translation(screen_pos),
								40.0,
								Color::srgba(0.0, 1.0, 0.0, 0.3),
							);
						}
					}
				}
			}
		}
	}

	if let Some(ref rotation_mode) = input_state.rotation_mode {
		let screen_pos = board_to_screen_pos(rotation_mode.position);
		gizmos.circle_2d(
			Isometry2d::from_translation(screen_pos),
			50.0,
			Color::srgba(1.0, 1.0, 0.0, 0.5),
		);
	}
}

fn update_turn_display(
	mut commands: Commands,
	game: Res<GameResource>,
	mut query: Query<(Entity, &mut Text), With<TurnIndicator>>,
	feedback_query: Query<Entity, With<FeedbackText>>,
	input_state: Res<InputState>,
) {
	if query.is_empty() {
		commands.spawn((
			TurnIndicator,
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

		commands.spawn((
			FeedbackText,
			Text::new(""),
			TextFont { font_size: FontSize::Px(24.0), ..default() },
			TextColor(Color::srgb(1.0, 0.5, 0.0)),
			Node {
				position_type: PositionType::Absolute,
				bottom: Val::Px(20.0),
				width: Val::Percent(100.0),
				..default()
			},
			TextLayout { justify: Justify::Center, ..default() },
		));
	}

	for (_, mut text) in query.iter_mut() {
		match game.0.state {
			GameState::InProgress => {
				let player_name = match game.0.current_player {
					Player::One => "Player 1",
					Player::Two => "Player 2",
				};

				if !has_legal_add_or_move(&game.0, game.0.current_player) {
					**text = format!(
						"{} has no legal moves - {} wins!",
						player_name,
						match game.0.current_player {
							Player::One => "Player 2",
							Player::Two => "Player 1",
						}
					);
				} else {
					**text = format!("{}'s Turn", player_name);
				}
			}
			GameState::Won(player) => {
				let winner = match player {
					Player::One => "Player 1",
					Player::Two => "Player 2",
				};
				**text = format!("{} Wins!", winner);
			}
		}
	}

	for entity in feedback_query.iter() {
		if let Some((ref message, _)) = input_state.feedback_message {
			commands.entity(entity).insert(Text::new(message.clone()));
		} else {
			commands.entity(entity).insert(Text::new(""));
		}
	}
}
