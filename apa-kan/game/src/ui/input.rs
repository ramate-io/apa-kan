use crate::config::GameConfig;
use crate::game::actions::{generate_legal_actions, has_legal_add_or_move, Action, ActionType};
use crate::game::core::{Game, Player, Position};
use crate::game::rules::Rules;
use crate::ui::board::{BotState, GameResource};
use crate::ui::layout::{
	bench_slot_x, bench_tile_size, human_bench_y, opponent_bench_y, screen_to_board,
};
use crate::ui::menu::GameScreen;
use crate::ui::state::{
	action_squares, actor_label, describe_action, human_player, is_human_turn, opponent_count,
	InputState, LastMove, Selection,
};
use bevy::prelude::*;

pub struct InputPlugin;

impl Plugin for InputPlugin {
	fn build(&self, app: &mut App) {
		app.init_resource::<InputState>().add_systems(
			Update,
			(handle_input, update_hud, handle_pass_button).run_if(in_state(GameScreen::Playing)),
		);
	}
}

#[derive(Component)]
struct HudRoot;

#[derive(Component)]
struct HudStatus;

#[derive(Component)]
struct HudLastMove;

#[derive(Component)]
struct PassButton;

pub fn apply_recorded_action(
	game: &mut Game,
	config: &GameConfig,
	input: &mut InputState,
	action: Action,
	who: &str,
) {
	let from = match action.action_type {
		ActionType::Move { tile_id, .. } => {
			game.board.tiles().find(|(_, t)| t.id == tile_id).map(|(pos, _)| *pos)
		}
		_ => None,
	};
	let opp_before = opponent_count(game, action.player);
	let self_before = game.board.tiles().filter(|(_, t)| t.owner == action.player).count();
	match Rules::apply_action(game, action) {
		Ok(()) => {
			let captured = opp_before.saturating_sub(opponent_count(game, action.player));
			let self_after = game.board.tiles().filter(|(_, t)| t.owner == action.player).count();
			let self_removed =
				matches!(action.action_type, ActionType::Add { .. }) && self_after <= self_before;
			let text = describe_action(who, action, from, captured, self_removed);
			println!("{text}");
			input.last_move = Some(LastMove { text, squares: action_squares(action, from) });
			input.selection = Selection::None;
			input.rotation = 0;
		}
		Err(e) => {
			eprintln!("Action failed: {e}");
		}
	}
}

fn handle_input(
	mut game: ResMut<GameResource>,
	mut input: ResMut<InputState>,
	bot_state: Res<BotState>,
	config: Res<GameConfig>,
	keyboard: Res<ButtonInput<KeyCode>>,
	mouse: Res<ButtonInput<MouseButton>>,
	windows: Query<&Window>,
	camera: Query<(&Camera, &GlobalTransform)>,
) {
	if game.0.is_game_over() || bot_state.thinking {
		return;
	}
	if !is_human_turn(&config, game.0.current_player) {
		return;
	}

	if keyboard.just_pressed(KeyCode::Escape) {
		input.selection = Selection::None;
		input.rotation = 0;
		return;
	}

	if keyboard.just_pressed(KeyCode::KeyR) {
		if let Selection::MoveTile { .. } = input.selection {
			let max = if game.0.rule_set.fixed_rotation { 2 } else { 4 };
			input.rotation = (input.rotation + 1) % max;
		}
		return;
	}

	if keyboard.just_pressed(KeyCode::KeyP) {
		try_pass(&mut game, &config, &mut input);
		return;
	}

	if !mouse.just_pressed(MouseButton::Left) {
		return;
	}

	let Some(click) = cursor_world(&windows, &camera) else {
		return;
	};

	if let Some(index) = bench_hit(click, &game.0, game.0.current_player, &config) {
		handle_bench_click(&mut game, &mut input, &config, index);
		return;
	}

	let Some(pos) = screen_to_board(click, game.0.board.size()) else {
		input.selection = Selection::None;
		input.rotation = 0;
		return;
	};

	handle_board_click(&mut game, &mut input, &config, pos);
}

fn handle_bench_click(
	game: &mut GameResource,
	input: &mut InputState,
	config: &GameConfig,
	index: usize,
) {
	if let Selection::PendingAdd { position } = input.selection {
		let action = Action::add(game.0.current_player, index, position);
		let who = actor_label(config, game.0.current_player);
		apply_recorded_action(&mut game.0, config, input, action, &who);
	}
}

fn handle_board_click(
	game: &mut GameResource,
	input: &mut InputState,
	config: &GameConfig,
	pos: Position,
) {
	let legal = generate_legal_actions(&game.0);

	if let Selection::MoveTile { tile_id, from } = input.selection {
		if pos == from {
			input.selection = Selection::None;
			input.rotation = 0;
			return;
		}
		if let Some(action) = legal.iter().copied().find(|action| {
			matches!(
				action.action_type,
				ActionType::Move { tile_id: id, destination, rotation_count }
					if id == tile_id && destination == pos && rotation_count == input.rotation
			)
		}) {
			let who = actor_label(config, game.0.current_player);
			apply_recorded_action(&mut game.0, config, input, action, &who);
			return;
		}
		if let Some(action) = legal.iter().copied().find(|action| {
			matches!(
				action.action_type,
				ActionType::Move { tile_id: id, destination, .. }
					if id == tile_id && destination == pos
			)
		}) {
			let who = actor_label(config, game.0.current_player);
			apply_recorded_action(&mut game.0, config, input, action, &who);
			return;
		}
	}

	if game.0.board.get(pos).is_none() {
		let can_add = legal.iter().any(
			|action| matches!(action.action_type, ActionType::Add { position, .. } if position == pos),
		);
		if can_add {
			input.selection = Selection::PendingAdd { position: pos };
			input.rotation = 0;
			return;
		}
	}

	if let Some(tile) = game.0.board.get(pos) {
		if tile.owner == game.0.current_player {
			input.selection = Selection::MoveTile { tile_id: tile.id, from: pos };
			input.rotation = 0;
			return;
		}
	}

	input.selection = Selection::None;
	input.rotation = 0;
}

fn try_pass(game: &mut GameResource, config: &GameConfig, input: &mut InputState) {
	if !game.0.rule_set.scored_mode {
		return;
	}
	if has_legal_add_or_move(&game.0, game.0.current_player) {
		return;
	}
	let action = Action::pass(game.0.current_player);
	let who = actor_label(config, game.0.current_player);
	apply_recorded_action(&mut game.0, config, input, action, &who);
}

fn bench_hit(click: Vec2, game: &Game, player: Player, config: &GameConfig) -> Option<usize> {
	let human = human_player(config);
	let bottom = match human {
		Some(Player::Two) => Player::Two,
		_ => Player::One,
	};
	let y = if player == bottom { human_bench_y() } else { opponent_bench_y() };
	if player != game.current_player {
		return None;
	}
	let stack = game.stack(player);
	if stack.is_empty() {
		return None;
	}
	let tile_size = bench_tile_size(stack.len());
	for index in 0..stack.len() {
		let x = bench_slot_x(index, stack.len(), tile_size);
		if (click.x - x).abs() <= tile_size * 0.5 && (click.y - y).abs() <= tile_size * 0.5 {
			return Some(index);
		}
	}
	None
}

fn cursor_world(
	windows: &Query<&Window>,
	camera: &Query<(&Camera, &GlobalTransform)>,
) -> Option<Vec2> {
	let window = windows.single().ok()?;
	let (camera, transform) = camera.single().ok()?;
	let cursor = window.cursor_position()?;
	camera.viewport_to_world_2d(transform, cursor).ok()
}

fn update_hud(
	mut commands: Commands,
	game: Res<GameResource>,
	bot: Res<BotState>,
	config: Res<GameConfig>,
	input: Res<InputState>,
	mut status: Query<&mut Text, (With<HudStatus>, Without<HudLastMove>)>,
	mut last: Query<&mut Text, (With<HudLastMove>, Without<HudStatus>)>,
	pass_btn: Query<Entity, With<PassButton>>,
) {
	if status.is_empty() {
		spawn_hud(&mut commands);
		return;
	}

	let p1_tiles = game.0.board.tiles().filter(|(_, t)| t.owner == Player::One).count();
	let p2_tiles = game.0.board.tiles().filter(|(_, t)| t.owner == Player::Two).count();
	let turn_line = match game.0.state {
		crate::game::core::GameState::InProgress => {
			if bot.thinking {
				format!("Bot thinking...  ({})", actor_label(&config, game.0.current_player))
			} else if !has_legal_add_or_move(&game.0, game.0.current_player)
				&& game.0.rule_set.scored_mode
				&& is_human_turn(&config, game.0.current_player)
			{
				format!(
					"{} - no legal ADD/MOVE. Press P or Pass.",
					actor_label(&config, game.0.current_player)
				)
			} else {
				let hint = match input.selection {
					Selection::PendingAdd { position } => {
						format!(" | pick a bench tile to ADD at {},{}", position.row, position.col)
					}
					Selection::MoveTile { .. } => " | click a MOVE square (R rotates)".to_string(),
					Selection::None => String::new(),
				};
				format!("{} to play{hint}", actor_label(&config, game.0.current_player))
			}
		}
		crate::game::core::GameState::Won(player) => {
			format!(
				"{} wins ({})",
				actor_label(&config, player),
				end_reason(&game.0, p1_tiles, p2_tiles)
			)
		}
		crate::game::core::GameState::Draw => {
			format!("Draw ({})", end_reason(&game.0, p1_tiles, p2_tiles))
		}
	};

	let stats = format!(
		"{}: {} on board, {} ADDs left  |  {}: {} on board, {} ADDs left  |  Turn {}/100",
		actor_label(&config, Player::One),
		p1_tiles,
		game.0.add_bank_p1,
		actor_label(&config, Player::Two),
		p2_tiles,
		game.0.add_bank_p2,
		game.0.turn_count
	);

	for mut text in status.iter_mut() {
		**text = format!("{turn_line}\n{stats}");
	}

	let last_text = input
		.last_move
		.as_ref()
		.map(|m| format!("Last move: {}", m.text))
		.unwrap_or_else(|| "Last move: -".to_string());
	for mut text in last.iter_mut() {
		**text = last_text.clone();
	}

	let show_pass = game.0.rule_set.scored_mode
		&& !game.0.is_game_over()
		&& !bot.thinking
		&& is_human_turn(&config, game.0.current_player)
		&& !has_legal_add_or_move(&game.0, game.0.current_player);

	if show_pass && pass_btn.is_empty() {
		spawn_pass_button(&mut commands);
	}
	if !show_pass {
		for entity in pass_btn.iter() {
			commands.entity(entity).despawn();
		}
	}
}

fn spawn_hud(commands: &mut Commands) {
	commands
		.spawn((
			HudRoot,
			Node {
				position_type: PositionType::Absolute,
				top: Val::Px(6.0),
				left: Val::Px(18.0),
				right: Val::Px(18.0),
				flex_direction: FlexDirection::Column,
				align_items: AlignItems::Center,
				row_gap: Val::Px(4.0),
				..default()
			},
		))
		.with_children(|parent| {
			parent.spawn((
				HudStatus,
				Text::new(""),
				TextFont { font_size: FontSize::Px(20.0), ..default() },
				TextColor(Color::srgb(1.0, 1.0, 1.0)),
				TextLayout::justify(Justify::Center),
			));
			parent.spawn((
				HudLastMove,
				Text::new("Last move: -"),
				TextFont { font_size: FontSize::Px(16.0), ..default() },
				TextColor(Color::srgb(1.0, 0.84, 0.35)),
				TextLayout::justify(Justify::Center),
			));
		});
}

fn spawn_pass_button(commands: &mut Commands) {
	commands
		.spawn((
			PassButton,
			Button,
			Node {
				position_type: PositionType::Absolute,
				bottom: Val::Px(18.0),
				right: Val::Px(24.0),
				padding: UiRect::axes(Val::Px(16.0), Val::Px(10.0)),
				..default()
			},
			BackgroundColor(Color::srgb(0.18, 0.45, 0.28)),
		))
		.with_children(|parent| {
			parent.spawn((
				Text::new("Pass (P)"),
				TextFont { font_size: FontSize::Px(18.0), ..default() },
				TextColor(Color::WHITE),
			));
		});
}

fn handle_pass_button(
	mut game: ResMut<GameResource>,
	mut input: ResMut<InputState>,
	config: Res<GameConfig>,
	bot: Res<BotState>,
	interaction: Query<&Interaction, (Changed<Interaction>, With<PassButton>)>,
) {
	if bot.thinking || !is_human_turn(&config, game.0.current_player) {
		return;
	}
	for interaction in interaction.iter() {
		if *interaction == Interaction::Pressed {
			try_pass(&mut game, &config, &mut input);
		}
	}
}

fn end_reason(game: &Game, p1_tiles: usize, p2_tiles: usize) -> &'static str {
	if game.turn_count >= 100 {
		"100-turn limit"
	} else if p1_tiles == 0 || p2_tiles == 0 {
		"elimination"
	} else if game.rule_set.scored_mode {
		"tile count"
	} else {
		"no legal moves"
	}
}
