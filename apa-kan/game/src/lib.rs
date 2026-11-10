//! Apa Kan — 8-petal rosette redesign.

pub mod ai;
pub mod board;
pub mod config;
pub mod game;
pub mod render;
#[cfg(feature = "desktop")]
pub mod ui;

use config::GameConfig;
use game::{Game, RuleSet};

pub fn run() {
	let cfg = GameConfig::from_args();
	if let Some(path) = &cfg.screenshot {
		let game = Game::new(cfg.rules, cfg.seed);
		if let Err(err) = render::write_png(&game, path, "opening position, seed from --seed") {
			eprintln!("screenshot failed: {err}");
			std::process::exit(1);
		}
		return;
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
