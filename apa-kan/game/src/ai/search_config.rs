//! Independently toggleable search levers. `enhanced` is the live default;
//! `classic` is the PR #62 engine (TT + ordering + quiescence, heuristic
//! playouts, no LMR/PVS/progressive widening, one search thread).

use std::fmt;

/// Search-algorithm flags. Each can be compared in isolation via
/// `--search-profile` / `--search` / `--search-threads`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SearchConfig {
	/// Captures / TT-PV first, then killers, then history.
	pub move_ordering: bool,
	/// Zobrist TT with exact/lower/upper bounds, reused across ID.
	pub transposition: bool,
	/// Late-move reductions with a full-depth re-search on fail-high.
	pub lmr: bool,
	/// Principal-variation (null-window) search with re-search.
	pub pvs: bool,
	/// Capture-only quiescence at depth-0 leaves.
	pub quiescence: bool,
	/// Expand MCTS moves in heuristic order; widen with visit count.
	pub mcts_progressive_widening: bool,
	/// Short epsilon-greedy playout then static eval (vs uniform random).
	pub mcts_heuristic_playouts: bool,
	/// Independent MCTS trees at the root. `1` is sequential and is the
	/// only setting that keeps `--jobs` benches bit-identical.
	pub mcts_root_threads: u8,
}

impl SearchConfig {
	/// Live default: every pruning lever on, one MCTS search thread.
	pub fn enhanced() -> Self {
		Self {
			move_ordering: true,
			transposition: true,
			lmr: true,
			pvs: true,
			quiescence: true,
			mcts_progressive_widening: true,
			mcts_heuristic_playouts: true,
			mcts_root_threads: 1,
		}
	}

	/// PR #62 / pre-pruning engine.
	pub fn classic() -> Self {
		Self {
			move_ordering: true,
			transposition: true,
			lmr: false,
			pvs: false,
			quiescence: true,
			mcts_progressive_widening: false,
			mcts_heuristic_playouts: true,
			mcts_root_threads: 1,
		}
	}

	/// Plain alpha-beta / vanilla UCT. For correctness tests.
	pub fn plain() -> Self {
		Self {
			move_ordering: false,
			transposition: false,
			lmr: false,
			pvs: false,
			quiescence: false,
			mcts_progressive_widening: false,
			mcts_heuristic_playouts: false,
			mcts_root_threads: 1,
		}
	}

	pub fn parse_profile(s: &str) -> Result<Self, String> {
		match s.trim().to_ascii_lowercase().as_str() {
			"enhanced" | "new" | "full" | "pruned" => Ok(Self::enhanced()),
			"classic" | "old" | "baseline" => Ok(Self::classic()),
			"plain" | "ab" | "alpha-beta" | "alphabeta" => Ok(Self::plain()),
			other => {
				Err(format!("unknown --search-profile '{other}' (expected enhanced|classic|plain)"))
			}
		}
	}

	pub fn profile_name(self) -> &'static str {
		let mut core = self;
		core.mcts_root_threads = 1;
		if core == Self::enhanced() {
			"enhanced"
		} else if core == Self::classic() {
			"classic"
		} else if core == Self::plain() {
			"plain"
		} else {
			"custom"
		}
	}

	/// Overlay `key=on|off` / `threads=N` pairs onto `self`.
	pub fn with_spec(mut self, spec: &str) -> Result<Self, String> {
		for part in spec.split(',') {
			let part = part.trim();
			if part.is_empty() {
				continue;
			}
			let (key, val) = part.split_once('=').ok_or_else(|| {
				format!("invalid --search '{spec}' (expected key=on|off or threads=N)")
			})?;
			let key = key.trim().to_ascii_lowercase();
			let val = val.trim();
			match key.as_str() {
				"ordering" | "move-ordering" | "move_ordering" => {
					self.move_ordering = parse_on_off(&key, val)?;
				}
				"tt" | "transposition" | "hash" => {
					self.transposition = parse_on_off(&key, val)?;
				}
				"lmr" => self.lmr = parse_on_off(&key, val)?,
				"pvs" | "null-window" | "null_window" => {
					self.pvs = parse_on_off(&key, val)?;
				}
				"quiescence" | "qsearch" | "q" => {
					self.quiescence = parse_on_off(&key, val)?;
				}
				"pw" | "progressive-widening" | "progressive_widening" | "widening" => {
					self.mcts_progressive_widening = parse_on_off(&key, val)?;
				}
				"heuristic-playouts" | "heuristic_playouts" | "playouts" | "cut-playouts" => {
					self.mcts_heuristic_playouts = parse_on_off(&key, val)?;
				}
				"threads" | "search-threads" | "root-threads" => {
					let n: u64 = val.parse().map_err(|_| {
						format!("invalid --search threads '{val}' (expected 1..32)")
					})?;
					if n == 0 || n > 32 {
						return Err("invalid --search threads (expected 1..32)".into());
					}
					self.mcts_root_threads = n as u8;
				}
				other => {
					return Err(format!(
						"unknown --search key '{other}' (expected ordering|tt|lmr|pvs|quiescence|pw|heuristic-playouts|threads)"
					));
				}
			}
		}
		Ok(self)
	}

	pub fn with_threads(mut self, n: u8) -> Self {
		self.mcts_root_threads = n.max(1);
		self
	}

	pub fn label(self) -> String {
		format!(
			"profile={} ordering={} tt={} lmr={} pvs={} q={} pw={} heur_playouts={} threads={}",
			self.profile_name(),
			on_off(self.move_ordering),
			on_off(self.transposition),
			on_off(self.lmr),
			on_off(self.pvs),
			on_off(self.quiescence),
			on_off(self.mcts_progressive_widening),
			on_off(self.mcts_heuristic_playouts),
			self.mcts_root_threads
		)
	}
}

impl Default for SearchConfig {
	fn default() -> Self {
		Self::enhanced()
	}
}

impl fmt::Display for SearchConfig {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		f.write_str(&self.label())
	}
}

fn parse_on_off(flag: &str, value: &str) -> Result<bool, String> {
	match value.to_ascii_lowercase().as_str() {
		"on" | "true" | "1" | "yes" => Ok(true),
		"off" | "false" | "0" | "no" => Ok(false),
		other => Err(format!("invalid {flag} '{other}' (expected on|off)")),
	}
}

fn on_off(v: bool) -> &'static str {
	if v {
		"on"
	} else {
		"off"
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn profiles_round_trip() {
		assert_eq!(SearchConfig::parse_profile("enhanced").unwrap(), SearchConfig::enhanced());
		assert_eq!(SearchConfig::parse_profile("classic").unwrap(), SearchConfig::classic());
		assert_eq!(SearchConfig::parse_profile("plain").unwrap(), SearchConfig::plain());
		assert!(SearchConfig::parse_profile("stockfish").is_err());
	}

	#[test]
	fn spec_overlays_a_profile() {
		let cfg = SearchConfig::classic().with_spec("lmr=on,pvs=on,pw=on,threads=4").unwrap();
		assert!(cfg.lmr && cfg.pvs && cfg.mcts_progressive_widening);
		assert_eq!(cfg.mcts_root_threads, 4);
		assert!(cfg.transposition);
	}
}
