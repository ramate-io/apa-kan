use crate::config::GameConfig;
use crate::game::actions::generate_legal_actions;
use crate::game::core::{Game, GameState, Player, Position};
use crate::game::{Action, AiConfig, AiPlayer};
use crate::ui::menu::GameScreen;
use bevy::prelude::*;
use bevy::tasks::{AsyncComputeTaskPool, Task};
use futures_lite::future;

#[derive(Resource)]
pub struct GameResource(pub Game);

#[derive(Resource, Default)]
pub struct BotState {
	pub thinking: bool,
	pub task: Option<Task<Action>>,
}

pub struct BoardPlugin;

impl Plugin for BoardPlugin {
	fn build(&self, app: &mut App) {
		app.init_resource::<BotState>()
			.add_systems(Startup, init_game)
			.add_systems(OnEnter(GameScreen::Playing), setup_board)
			.add_systems(
				Update,
				(update_board, check_bot_turn, apply_bot_action)
					.run_if(in_state(GameScreen::Playing)),
			);
	}
}

fn init_game(mut commands: Commands, config: Res<GameConfig>) {
	let game = Game::new_with_rules(config.rule_set());
	commands.insert_resource(GameResource(game));
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

fn check_bot_turn(
	game: Res<GameResource>,
	mut bot_state: ResMut<BotState>,
	config: Res<GameConfig>,
) {
	if bot_state.thinking || bot_state.task.is_some() {
		return;
	}

	if game.0.is_game_over() {
		return;
	}

	let bot_type = match config.bot {
		Some(bot) => bot,
		None => return,
	};

	let bot_seat = if config.human_seat == 1 { 2 } else { 1 };
	let current_player_num = match game.0.current_player {
		Player::One => 1,
		Player::Two => 2,
	};

	if current_player_num != bot_seat {
		return;
	}

	bot_state.thinking = true;

	let game_clone = game.0.clone();
	let think_time = config.think_time();

	let task = AsyncComputeTaskPool::get().spawn(async move {
		let ai = AiPlayer::new(AiConfig {
			depth: 3,
			bot_type,
			time_budget: Some(think_time),
			use_transposition_table: true,
			use_move_ordering: true,
			mcts_iterations: None,
			mcts_exploration: 1.41,
			mcts_rollout_depth: 20,
		});

		ai.choose_action(&game_clone)
			.unwrap_or_else(|| Action::pass(game_clone.current_player))
	});

	bot_state.task = Some(task);
}

fn apply_bot_action(
	mut game: ResMut<GameResource>,
	mut bot_state: ResMut<BotState>,
	config: Res<GameConfig>,
) {
	if let Some(mut task) = bot_state.task.take() {
		if let Some(action) = future::block_on(future::poll_once(&mut task)) {
			let bot_name = config.bot.map(|b| b.name()).unwrap_or("Bot");
			println!("{} chose: {:?}", bot_name, action);

			if let Err(e) = crate::game::Rules::apply_action(&mut game.0, action) {
				eprintln!("Bot action failed: {:?}", e);
			}

			bot_state.thinking = false;
		} else {
			bot_state.task = Some(task);
		}
	}
}

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
