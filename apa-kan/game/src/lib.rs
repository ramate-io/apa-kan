use bevy::prelude::*;

pub mod config;
pub mod game;
pub mod ui;

pub fn run() {
	let config = config::GameConfig::from_args();

	App::new()
		.add_plugins(DefaultPlugins.set(WindowPlugin {
			primary_window: Some(Window {
				title: "Apa Kan".into(),
				resolution:
					(ui::layout::WINDOW_WIDTH as u32, ui::layout::WINDOW_HEIGHT as u32).into(),
				..default()
			}),
			..default()
		}))
		.insert_resource(config)
		.insert_resource(ClearColor(Color::srgb(0.12, 0.12, 0.13)))
		.add_plugins((
			ui::MenuPlugin,
			ui::BoardPlugin,
			ui::TileRenderPlugin,
			ui::InputPlugin,
			ui::snapshot::SnapshotPlugin,
		))
		.add_systems(Startup, setup)
		.run();
}

fn setup(mut commands: Commands) {
	commands.spawn(Camera2d::default());
}

#[bevy_main]
pub fn main() {
	run();
}
