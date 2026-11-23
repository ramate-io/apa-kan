//! Playable-app JSONL capture: default path, load, and replay-to-action.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::bench_log::{self, GameLog};
use crate::game::{Action, Game};

/// Directory for per-game play logs.
///
/// Resolution order: `APA_KAN_GAME_LOG_DIR`, then `$XDG_DATA_HOME/apa-kan/logs`,
/// then `~/.local/share/apa-kan/logs`, then `./apa-kan-logs`.
pub fn default_dir() -> PathBuf {
	if let Ok(dir) = std::env::var("APA_KAN_GAME_LOG_DIR") {
		let trimmed = dir.trim();
		if !trimmed.is_empty() {
			return PathBuf::from(trimmed);
		}
	}
	if let Ok(xdg) = std::env::var("XDG_DATA_HOME") {
		let trimmed = xdg.trim();
		if !trimmed.is_empty() {
			return PathBuf::from(trimmed).join("apa-kan").join("logs");
		}
	}
	if let Ok(home) = std::env::var("HOME") {
		let trimmed = home.trim();
		if !trimmed.is_empty() {
			return PathBuf::from(trimmed).join(".local/share/apa-kan/logs");
		}
	}
	PathBuf::from("apa-kan-logs")
}

/// Timestamped JSONL path under [`default_dir`] for this seed.
pub fn new_path(seed: u64) -> PathBuf {
	let secs = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
	default_dir().join(format!("apa-kan-{secs}-seed{seed}.jsonl"))
}

/// Apply `APA_KAN_GAME_LOG` when the CLI did not set a log flag.
///
/// `off` / `none` / `false` / `0` disables capture. Any other non-empty value
/// is a file path.
pub fn apply_env(off: &mut bool, path: &mut Option<PathBuf>) {
	let Ok(raw) = std::env::var("APA_KAN_GAME_LOG") else {
		return;
	};
	let v = raw.trim();
	if v.is_empty() {
		return;
	}
	match v.to_ascii_lowercase().as_str() {
		"off" | "none" | "false" | "0" | "no" => *off = true,
		_ => *path = Some(PathBuf::from(v)),
	}
}

pub fn write_log(path: &Path, log: &GameLog) -> Result<(), String> {
	if let Some(parent) = path.parent() {
		if !parent.as_os_str().is_empty() {
			fs::create_dir_all(parent)
				.map_err(|e| format!("failed to create {}: {e}", parent.display()))?;
		}
	}
	let body = bench_log::encode_jsonl(std::slice::from_ref(log))?;
	fs::write(path, body).map_err(|e| format!("failed to write {}: {e}", path.display()))
}

/// Load one or more games from `.jsonl` or `.jsonl.gz`.
pub fn load_path(path: &Path) -> Result<Vec<GameLog>, String> {
	let bytes = fs::read(path).map_err(|e| format!("failed to read {}: {e}", path.display()))?;
	if bench_log::path_is_gzip(path) {
		bench_log::decode_jsonl_gz(&bytes)
	} else {
		let text = String::from_utf8(bytes)
			.map_err(|e| format!("{} is not utf-8: {e}", path.display()))?;
		bench_log::decode_jsonl(&text)
	}
}

/// Replay the first `n` actions of `log` (0 = opening position).
pub fn replay_prefix(log: &GameLog, n: usize) -> Result<Game, String> {
	if n > log.actions.len() {
		return Err(format!(
			"action {n} is past the end of the log ({} actions)",
			log.actions.len()
		));
	}
	let mut clipped = log.clone();
	clipped.actions.truncate(n);
	bench_log::replay(&clipped)
}

/// 1-based action index: search the position *before* that action.
pub fn replay_before_action(
	log: &GameLog,
	action_1based: usize,
) -> Result<(Game, Option<Action>), String> {
	if action_1based == 0 {
		return Err("--action is 1-based (use 1 for the first action)".into());
	}
	let idx = action_1based - 1;
	if idx > log.actions.len() {
		return Err(format!(
			"action {action_1based} is past the end of the log ({} actions)",
			log.actions.len()
		));
	}
	let pending = log.actions.get(idx).map(|a| a.to_action()).transpose()?;
	let game = replay_prefix(log, idx)?;
	Ok((game, pending))
}

/// First position whose *next* logged action has `turn >= turn`.
pub fn replay_at_turn(log: &GameLog, turn: u32) -> Result<(Game, usize), String> {
	for (i, act) in log.actions.iter().enumerate() {
		if act.turn >= turn {
			return Ok((replay_prefix(log, i)?, i + 1));
		}
	}
	Ok((bench_log::replay(log)?, log.actions.len().saturating_add(1)))
}

#[cfg(test)]
mod tests {
	#![allow(clippy::unwrap_used, clippy::expect_used)]

	use super::*;
	use crate::ai::{Actor, Searcher};
	use crate::bench_log::{self, GameLog};
	use crate::game::{Player, RuleSet};
	use rand::rngs::StdRng;
	use rand::SeedableRng;
	use std::time::Duration;

	#[test]
	fn replay_prefix_round_trips_a_short_game() {
		let mut game = crate::game::Game::new(RuleSet::default(), 11);
		let mut log =
			GameLog::begin(1, "greedy vs greedy", "greedy", "greedy", 1, "default", &game);
		let mut rng = StdRng::seed_from_u64(11);
		let mut s1 = Searcher::new();
		let mut s2 = Searcher::new();
		for _ in 0..8 {
			if game.status().is_over() {
				break;
			}
			let actor = Actor::Greedy;
			let searcher = match game.current_player() {
				Player::One => &mut s1,
				Player::Two => &mut s2,
			};
			let action = searcher.choose(&game, actor, &mut rng, Duration::from_millis(1));
			let piece = bench_log::piece_for_action(&game, action);
			let partner = bench_log::partner_for_action(&game, action);
			let outcome = game.apply(action).expect("greedy action applies");
			log.push_action_partner(action, piece, partner, &outcome, &game);
			s1.advance(&game);
			s2.advance(&game);
		}
		log.finish(&game, bench_log::low_value_of(&game));
		assert!(!log.actions.is_empty());
		let mid = replay_prefix(&log, log.actions.len() / 2).unwrap();
		assert!(!mid.status().is_over() || log.actions.len() / 2 == log.actions.len());
		let (before, pending) = replay_before_action(&log, 1).unwrap();
		assert!(pending.is_some());
		assert_eq!(before.current_player(), Player::One);
		let full = replay_prefix(&log, log.actions.len()).unwrap();
		assert_eq!(full.turn_count(), game.turn_count());
	}
}
