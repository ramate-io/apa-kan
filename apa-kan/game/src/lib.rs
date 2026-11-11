//! Apa Kan — 8-petal rosette redesign.

pub mod ai;
pub mod board;
pub mod config;
pub mod game;
pub mod render;
pub mod scenario;
#[cfg(feature = "desktop")]
pub mod ui;
pub mod visual;

use config::GameConfig;
use game::RuleSet;

pub fn run() {
	let cfg = GameConfig::from_args();
	if let Some(path) = cfg.screenshot.clone() {
		#[cfg(feature = "desktop")]
		{
			let _ = path;
			ui::run_app(cfg);
			return;
		}
		#[cfg(not(feature = "desktop"))]
		{
			let game = visual::demo_midgame();
			if let Err(err) = render::write_png_sized(
				&game,
				path,
				"mid-game visual fixture",
				cfg.width,
				cfg.height,
			) {
				eprintln!("screenshot failed: {err}");
				std::process::exit(1);
			}
			return;
		}
	}
	#[cfg(feature = "desktop")]
	ui::run_app(cfg);
	#[cfg(not(feature = "desktop"))]
	{
		let _ = cfg;
		eprintln!("apa-kan was built without the desktop feature");
		std::process::exit(1);
	}
}

pub fn default_rules() -> RuleSet {
	RuleSet::default()
}

#[cfg(all(feature = "desktop", target_os = "android"))]
use bevy::prelude::bevy_main;

/// Native-activity entry used by `cargo-apk` on Android.
#[cfg(all(feature = "desktop", target_os = "android"))]
#[bevy_main]
pub fn main() {
	run();
}
