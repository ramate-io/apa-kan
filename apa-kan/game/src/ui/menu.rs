use crate::config::GameConfig;
use bevy::prelude::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, States, Default)]
pub enum GameScreen {
	#[default]
	Title,
	Playing,
}

pub struct MenuPlugin;

impl Plugin for MenuPlugin {
	fn build(&self, app: &mut App) {
		app.init_state::<GameScreen>()
			.add_systems(Startup, skip_title_for_bot)
			.add_systems(OnEnter(GameScreen::Title), setup_title_screen)
			.add_systems(Update, title_input.run_if(in_state(GameScreen::Title)))
			.add_systems(OnExit(GameScreen::Title), cleanup_title_screen);
	}
}

fn skip_title_for_bot(config: Res<GameConfig>, mut next_state: ResMut<NextState<GameScreen>>) {
	if config.bot.is_some() || std::env::var("APA_KAN_SNAPSHOT_DIR").is_ok() {
		next_state.set(GameScreen::Playing);
	}
}

#[derive(Component)]
struct TitleScreenEntity;

fn setup_title_screen(mut commands: Commands) {
	commands.spawn((
		TitleScreenEntity,
		Text::new("APA KAN"),
		TextFont { font_size: FontSize::Px(100.0), ..default() },
		TextColor(Color::srgb(0.8, 0.4, 0.2)),
		Node {
			position_type: PositionType::Absolute,
			top: Val::Percent(30.0),
			width: Val::Percent(100.0),
			..default()
		},
		TextLayout { justify: Justify::Center, ..default() },
	));

	commands.spawn((
		TitleScreenEntity,
		Text::new("Press SPACE or tap to start"),
		TextFont { font_size: FontSize::Px(30.0), ..default() },
		TextColor(Color::srgb(0.7, 0.7, 0.7)),
		Node {
			position_type: PositionType::Absolute,
			top: Val::Percent(55.0),
			width: Val::Percent(100.0),
			..default()
		},
		TextLayout { justify: Justify::Center, ..default() },
	));
}

fn title_input(
	keyboard: Res<ButtonInput<KeyCode>>,
	mouse: Res<ButtonInput<MouseButton>>,
	touches: Res<Touches>,
	mut next_state: ResMut<NextState<GameScreen>>,
) {
	if keyboard.just_pressed(KeyCode::Space)
		|| mouse.just_pressed(MouseButton::Left)
		|| touches.iter_just_pressed().next().is_some()
	{
		next_state.set(GameScreen::Playing);
	}
}

fn cleanup_title_screen(mut commands: Commands, query: Query<Entity, With<TitleScreenEntity>>) {
	for entity in query.iter() {
		commands.entity(entity).despawn();
	}
}
