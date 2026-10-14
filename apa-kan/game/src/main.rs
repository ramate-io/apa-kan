use bevy::prelude::*;

fn main() {
	App::new()
		.add_plugins(
			DefaultPlugins.set(WindowPlugin {
				primary_window: Some(Window {
					title: "Apa Kan".into(),
					resolution: (800, 600).into(),
					..default()
				}),
				..default()
			}),
		)
		.add_systems(Startup, setup)
		.run();
}

fn setup(mut commands: Commands) {
	commands.spawn(Camera2d::default());

	commands.spawn((
		Text::new("Apa Kan"),
		TextFont {
			font_size: FontSize::Px(80.0),
			..default()
		},
		TextColor(Color::srgb(1.0, 1.0, 1.0)),
		Node {
			position_type: PositionType::Absolute,
			top: Val::Percent(40.0),
			left: Val::Percent(50.0),
			..default()
		},
		TextLayout {
			justify: Justify::Center,
			..default()
		},
	));
}
