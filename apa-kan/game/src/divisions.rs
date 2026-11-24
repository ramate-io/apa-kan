//! Offline Divisions: a local, bot-only Elo ladder that needs no server.
//!
//! Follows the core-split RFC's divisions decision: ratings stay inside a mode
//! and bot games are kept apart from human games. This ladder is the "bot
//! division" for the Iye tile option, stored on the device (browser
//! `localStorage` on web, a JSON file under the data dir on native). It never
//! writes the account Elo; a server ladder can import or ignore it later.
//!
//! Divisions are starting-Elo bands. Each band names an opponent from the
//! existing bot roster and a think budget. That only picks how much search the
//! bot gets; it does not add any tactics.

use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::ai::Actor;

/// Elo every new ladder starts at.
pub const START_ELO: i32 = 1000;
/// Elo K-factor for offline ladder games.
pub const K_FACTOR: f64 = 32.0;
/// Floor so a losing streak cannot go negative.
pub const MIN_ELO: i32 = 100;
/// Bound device-edited data and keep rating arithmetic finite.
pub const MAX_ELO: i32 = 4000;
/// How many recent results the ladder keeps for display.
pub const HISTORY_LEN: usize = 20;

const STORAGE_KEY: &str = "apakan.divisions.v1";

/// A starting-Elo band and the bot that guards it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Division {
	pub name: &'static str,
	/// Lowest Elo in this band (inclusive).
	pub floor: i32,
	pub bot: Actor,
	/// Rating the bot plays at. Ladder Elo moves against this number.
	pub bot_elo: i32,
	/// Per-move search budget for the bot.
	pub think_ms: u64,
}

impl Division {
	pub fn think(&self) -> Duration {
		Duration::from_millis(self.think_ms)
	}
}

/// Bands from lowest to highest. Ratings and band thresholds are provisional
/// offline progression defaults, not empirically calibrated playing strengths.
pub const DIVISIONS: [Division; 5] = [
	Division { name: "Bronze", floor: 0, bot: Actor::Mcts, bot_elo: 900, think_ms: 80 },
	Division { name: "Silver", floor: 1100, bot: Actor::Mcts, bot_elo: 1150, think_ms: 150 },
	Division { name: "Gold", floor: 1250, bot: Actor::Mcts, bot_elo: 1300, think_ms: 600 },
	Division { name: "Platinum", floor: 1400, bot: Actor::Mcts, bot_elo: 1450, think_ms: 1000 },
	Division { name: "Diamond", floor: 1550, bot: Actor::Mcts, bot_elo: 1600, think_ms: 2500 },
];

pub fn division_for(elo: i32) -> Division {
	let mut out = DIVISIONS[0];
	for d in DIVISIONS {
		if elo >= d.floor {
			out = d;
		}
	}
	out
}

pub fn next_division(elo: i32) -> Option<Division> {
	DIVISIONS.into_iter().find(|d| d.floor > elo)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Outcome {
	Win,
	Loss,
	Draw,
}

impl Outcome {
	fn score(self) -> f64 {
		match self {
			Outcome::Win => 1.0,
			Outcome::Draw => 0.5,
			Outcome::Loss => 0.0,
		}
	}

	pub fn label(self) -> &'static str {
		match self {
			Outcome::Win => "win",
			Outcome::Loss => "loss",
			Outcome::Draw => "draw",
		}
	}
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LadderGame {
	pub division: String,
	pub bot: String,
	pub outcome: Outcome,
	pub delta: i32,
	pub elo_after: i32,
}

/// Persistent local ladder.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Ladder {
	pub v: u32,
	pub elo: i32,
	pub wins: u32,
	pub losses: u32,
	pub draws: u32,
	#[serde(default)]
	pub history: Vec<LadderGame>,
}

impl Default for Ladder {
	fn default() -> Self {
		Self { v: 1, elo: START_ELO, wins: 0, losses: 0, draws: 0, history: Vec::new() }
	}
}

/// Result of recording one game.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LadderUpdate {
	pub before: Division,
	pub after: Division,
	pub delta: i32,
	pub elo: i32,
	pub outcome: Outcome,
}

impl LadderUpdate {
	pub fn message(&self) -> String {
		let sign = if self.delta >= 0 { "+" } else { "" };
		let mut s =
			format!("Division {}: {sign}{} Elo ({}).", self.outcome.label(), self.delta, self.elo);
		if self.after.floor > self.before.floor {
			s.push_str(&format!(" Promoted to {}.", self.after.name));
		} else if self.after.floor < self.before.floor {
			s.push_str(&format!(" Dropped to {}.", self.after.name));
		}
		s
	}
}

/// Expected score of a player rated `a` against `b`.
pub fn expected(a: i32, b: i32) -> f64 {
	1.0 / (1.0 + 10f64.powf((f64::from(b) - f64::from(a)) / 400.0))
}

impl Ladder {
	pub fn division(&self) -> Division {
		division_for(self.elo)
	}

	pub fn games(&self) -> u32 {
		self.wins.saturating_add(self.losses).saturating_add(self.draws)
	}

	/// Record a finished game against the current division's bot.
	pub fn record(&mut self, outcome: Outcome) -> LadderUpdate {
		let before = self.division();
		let exp = expected(self.elo, before.bot_elo);
		let proposed = (K_FACTOR * (outcome.score() - exp)).round() as i32;
		let old = self.elo;
		self.elo = (self.elo + proposed).clamp(MIN_ELO, MAX_ELO);
		let delta = self.elo - old;
		match outcome {
			Outcome::Win => self.wins = self.wins.saturating_add(1),
			Outcome::Loss => self.losses = self.losses.saturating_add(1),
			Outcome::Draw => self.draws = self.draws.saturating_add(1),
		}
		self.history.push(LadderGame {
			division: before.name.into(),
			bot: before.bot.name().into(),
			outcome,
			delta,
			elo_after: self.elo,
		});
		if self.history.len() > HISTORY_LEN {
			let drop = self.history.len() - HISTORY_LEN;
			self.history.drain(..drop);
		}
		LadderUpdate { before, after: self.division(), delta, elo: self.elo, outcome }
	}

	/// One-line lobby summary.
	pub fn summary(&self) -> String {
		let d = self.division();
		let next = match next_division(self.elo) {
			Some(n) => format!(" · {} at {}", n.name, n.floor),
			None => " · top division".into(),
		};
		format!(
			"{} · {} Elo · {}W {}L {}D · opponent {} ({} Elo){next}",
			d.name,
			self.elo,
			self.wins,
			self.losses,
			self.draws,
			d.bot.name(),
			d.bot_elo
		)
	}

	pub fn to_json(&self) -> String {
		serde_json::to_string(self).unwrap_or_default()
	}

	pub fn from_json(s: &str) -> Option<Self> {
		serde_json::from_str::<Ladder>(s).ok().filter(|l| {
			l.v == 1 && (MIN_ELO..=MAX_ELO).contains(&l.elo) && l.history.len() <= HISTORY_LEN
		})
	}

	/// Load from device storage; a missing or unreadable ladder starts fresh.
	pub fn load() -> Self {
		storage_get().and_then(|s| Self::from_json(&s)).unwrap_or_default()
	}

	/// Best-effort save. Storage failures (private mode, read-only FS) are
	/// reported so the UI can say the result was not kept.
	pub fn save(&self) -> Result<(), String> {
		storage_set(&self.to_json())
	}
}

#[cfg(target_arch = "wasm32")]
fn storage_get() -> Option<String> {
	let storage = web_sys::window()?.local_storage().ok()??;
	storage.get_item(STORAGE_KEY).ok()?
}

#[cfg(target_arch = "wasm32")]
fn storage_set(json: &str) -> Result<(), String> {
	let storage = web_sys::window()
		.and_then(|w| w.local_storage().ok().flatten())
		.ok_or("localStorage unavailable")?;
	storage.set_item(STORAGE_KEY, json).map_err(|e| format!("{e:?}"))
}

#[cfg(not(target_arch = "wasm32"))]
fn storage_path() -> Option<std::path::PathBuf> {
	if let Ok(p) = std::env::var("APAKAN_DIVISIONS_FILE") {
		if !p.is_empty() {
			return Some(p.into());
		}
	}
	let base = std::env::var_os("XDG_DATA_HOME").map(std::path::PathBuf::from).or_else(|| {
		std::env::var_os("HOME").map(|h| std::path::Path::new(&h).join(".local/share"))
	})?;
	Some(base.join("apa-kan").join(format!("{STORAGE_KEY}.json")))
}

#[cfg(not(target_arch = "wasm32"))]
fn storage_get() -> Option<String> {
	std::fs::read_to_string(storage_path()?).ok()
}

#[cfg(not(target_arch = "wasm32"))]
fn storage_set(json: &str) -> Result<(), String> {
	let path = storage_path().ok_or("no data directory")?;
	if let Some(dir) = path.parent() {
		std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
	}
	std::fs::write(&path, json).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
	#![allow(clippy::unwrap_used)]

	use super::*;

	#[test]
	fn bands_are_ordered_and_cover_everything() {
		for w in DIVISIONS.windows(2) {
			assert!(w[0].floor < w[1].floor);
			assert!(w[0].bot_elo < w[1].bot_elo);
		}
		assert_eq!(division_for(MIN_ELO).name, "Bronze");
		assert_eq!(division_for(START_ELO).name, "Bronze");
		assert_eq!(division_for(1100).name, "Silver");
		assert_eq!(division_for(9999).name, "Diamond");
		assert_eq!(next_division(9999), None);
		assert_eq!(next_division(START_ELO).map(|d| d.name), Some("Silver"));
		assert!(DIVISIONS.iter().all(|d| !d.bot.is_human()));
	}

	#[test]
	fn win_raises_loss_lowers_and_is_symmetric_at_equal_rating() {
		let mut l = Ladder { elo: 900, ..Ladder::default() };
		let up = l.record(Outcome::Win);
		assert_eq!(up.delta, 16);
		assert_eq!(l.elo, 916);
		let mut l = Ladder { elo: 900, ..Ladder::default() };
		let down = l.record(Outcome::Loss);
		assert_eq!(down.delta, -16);
		let mut l = Ladder { elo: 900, ..Ladder::default() };
		assert_eq!(l.record(Outcome::Draw).delta, 0);
		assert_eq!((l.wins, l.losses, l.draws), (0, 0, 1));
	}

	#[test]
	fn promotion_and_floor() {
		let mut l = Ladder { elo: 1095, ..Ladder::default() };
		let up = l.record(Outcome::Win);
		assert_eq!(up.after.name, "Silver");
		assert!(up.message().contains("Promoted to Silver"));
		let mut l = Ladder { elo: MIN_ELO, ..Ladder::default() };
		l.record(Outcome::Loss);
		assert_eq!(l.elo, MIN_ELO);
	}

	#[test]
	fn history_is_capped_and_json_round_trips() {
		let mut l = Ladder::default();
		for i in 0..(HISTORY_LEN + 5) {
			l.record(if i % 2 == 0 { Outcome::Win } else { Outcome::Loss });
		}
		assert_eq!(l.history.len(), HISTORY_LEN);
		assert_eq!(l.games() as usize, HISTORY_LEN + 5);
		let back = Ladder::from_json(&l.to_json()).unwrap();
		assert_eq!(back, l);
		assert!(Ladder::from_json("{not json").is_none());
		assert!(l.summary().contains("Elo"));
	}

	#[test]
	fn native_file_storage_round_trip() {
		let dir = std::env::temp_dir().join(format!("apakan-div-{}", std::process::id()));
		let path = dir.join("ladder.json");
		std::env::set_var("APAKAN_DIVISIONS_FILE", &path);
		let mut l = Ladder::default();
		l.record(Outcome::Win);
		l.save().unwrap();
		assert_eq!(Ladder::load(), l);
		std::env::remove_var("APAKAN_DIVISIONS_FILE");
		let _ = std::fs::remove_dir_all(dir);
	}
}
