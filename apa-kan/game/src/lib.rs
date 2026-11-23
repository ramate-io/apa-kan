//! Apa Kan — Bevy client (display and in-match UI).
//!
//! Rules, bots, JSONL replay, clocks, and generation passes live in
//! [`apa_kan_core`]. This crate re-exports those modules so in-match UI can
//! keep `crate::game::…` paths. Character PNG art stays here (`characters`).

pub use apa_kan_core::{
	ai, bench_log, board, character_pass, clock, core_pass, deck, game, scenario,
};

pub mod characters;
pub mod config;
#[cfg(feature = "desktop")]
pub mod platform;
pub mod play_log;
pub mod render;
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
