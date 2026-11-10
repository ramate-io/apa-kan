use crate::config::GameConfig;
use crate::game::core::{Game, Player, Position};
use crate::game::{Action, AiConfig, AiPlayer};
use crate::ui::input::apply_recorded_action;
use crate::ui::layout::{
	board_dark, board_light, board_to_screen, corner_color, square_size, BOARD_PX,
};
use crate::ui::menu::GameScreen;
use crate::ui::state::{actor_label, InputState};
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
				(check_bot_turn, apply_bot_action).run_if(in_state(GameScreen::Playing)),
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

fn setup_board(
	mut commands: Commands,
	game: Res<GameResource>,
	existing: Query<Entity, With<BoardEntity>>,
) {
	for entity in existing.iter() {
		commands.entity(entity).despawn();
	}

	let board_size = game.0.board.size() as i8;
	let sq = square_size(game.0.board.size());

	for row in 0..board_size {
		for col in 0..board_size {
			let pos = Position::new(row, col);
			let screen = board_to_screen(pos, game.0.board.size());
			let color = if game.0.board.is_corner(pos) {
				corner_color()
			} else if (row + col) % 2 == 0 {
				board_light()
			} else {
				board_dark()
			};

			commands.spawn((
				BoardEntity,
				BoardSquare { position: pos },
				Sprite { color, custom_size: Some(Vec2::splat(sq - 3.0)), ..default() },
				Transform::from_xyz(screen.x, screen.y, 0.0),
			));
		}
	}

	let _ = BOARD_PX;
}

fn check_bot_turn(
	game: Res<GameResource>,
	mut bot_state: ResMut<BotState>,
	config: Res<GameConfig>,
) {
	if std::env::var("APA_KAN_SNAPSHOT_DIR").is_ok() {
		return;
	}
	if bot_state.thinking || bot_state.task.is_some() {
		return;
	}
	if game.0.is_game_over() {
		return;
	}
	let Some(bot_type) = config.bot else {
		return;
	};

	let bot_seat = if config.human_seat == 2 { Player::Two } else { Player::One }.other();
	if game.0.current_player != bot_seat {
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
	mut input: ResMut<InputState>,
	config: Res<GameConfig>,
) {
	if let Some(mut task) = bot_state.task.take() {
		if let Some(action) = future::block_on(future::poll_once(&mut task)) {
			let who = actor_label(&config, action.player);
			apply_recorded_action(&mut game.0, &config, &mut input, action, &who);
			bot_state.thinking = false;
		} else {
			bot_state.task = Some(task);
		}
	}
}
