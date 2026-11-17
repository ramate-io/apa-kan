//! Independently toggleable search knobs, serializable as a bot profile.
//!
//! Named presets (`--search-profile` / `--bot-config`):
//! - `enhanced` (default): LMR + PVS + quiescence; MCTS progressive widening **off**
//! - `classic`: PR #62 engine (TT + ordering + quiescence, no LMR/PVS/PW)
//! - `plain`: alpha-beta / uniform cut-off playouts
//! - `tactical`: aggressive capture/center weights, deeper qsearch, short playouts
//! - `positional`: cautious, longer playouts, lighter LMR
//! - `explorative`: high PUCT, progressive widening on
//! - `deep_mcts`: move-ordering priors + prior-ordered PW + lower PUCT +
//!   tree reuse + solver (focused MCTS; other levers stay independent knobs)
//!
//! Overlay any field with `--search k=v,...`. Load a TOML file with `--bot-config`.

use std::fmt;
use std::path::Path;

use serde::{Deserialize, Serialize};

/// Search-algorithm knobs. Missing TOML keys take [`SearchConfig::enhanced`] values.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct SearchConfig {
	/// Captures / TT-PV first, then killers, then history.
	pub move_ordering: bool,
	/// Bonus for the TT / previous-iteration PV move.
	pub order_tt_bonus: i32,
	/// Bonus for the primary killer at this ply (second killer is 10k less).
	pub order_killer_bonus: i32,
	/// Bonus for moves that touch the center.
	pub order_center_bonus: i32,
	/// Scale for capture ordering hints, percent (100 = current weights).
	pub order_capture_scale: i32,

	/// Zobrist TT with exact/lower/upper bounds, reused across ID.
	pub transposition: bool,
	/// log2 of TT entries. Clamped to 12..=22 (4 Ki … 4 Mi entries).
	pub tt_bits: u8,

	/// Late-move reductions with a full-depth re-search on fail-high.
	pub lmr: bool,
	/// Interior remaining depth must be at least this before LMR applies.
	pub lmr_min_depth: u8,
	/// Do not reduce the first this-many ordered moves.
	pub lmr_move_skip: u8,
	/// Plies subtracted on a reduced late move.
	pub lmr_reduce: u8,
	/// Extra-late move index at which the bigger reduction applies.
	pub lmr_late_move: u8,
	/// Remaining depth at which the bigger reduction applies.
	pub lmr_late_depth: u8,
	/// Plies subtracted for extra-late moves.
	pub lmr_late_reduce: u8,
	/// Do not reduce root moves (ply == 1). Recommended; reducing the root
	/// dropped ID depth in the ablation.
	pub lmr_skip_root: bool,

	/// Principal-variation (null-window) search with re-search.
	pub pvs: bool,

	/// Capture-only quiescence at depth-0 leaves.
	pub quiescence: bool,
	/// Max capture-ply extension at a leaf.
	pub q_depth: u8,
	/// Max captures tried at each quiescence node.
	pub q_capture_cap: u8,

	/// Expand MCTS moves in heuristic order; widen with visit count.
	pub mcts_progressive_widening: bool,
	/// Progressive widening: `1 + pw_a * visits^pw_alpha`.
	pub pw_a: f32,
	pub pw_alpha: f32,

	/// Short epsilon-greedy playout then static eval (vs uniform random).
	pub mcts_heuristic_playouts: bool,
	/// Playout steps before static eval.
	pub playout_cutoff: u8,
	/// Chance a heuristic playout picks uniformly instead of greedy.
	pub playout_epsilon: f32,
	/// Chance `pick_untried` samples uniformly instead of the heuristic.
	pub mcts_untried_random: f32,

	/// Independent MCTS trees at the root. `1` is sequential and is the
	/// only setting that keeps `--jobs` benches bit-identical.
	pub mcts_root_threads: u8,
	/// PUCT exploration constant.
	pub puct: f32,
	/// RAVE equivalence parameter (higher = RAVE lingers longer).
	pub rave_eq: f32,

	/// Softmax of move-ordering scores as PUCT priors (vs eval-of-child).
	pub mcts_move_priors: bool,
	/// Softmax temperature for move priors (higher = flatter).
	pub prior_temperature: f32,
	/// Expand untried moves in prior/order-score order (highest first).
	pub prior_widening: bool,

	/// Keep the subtree under the played move (and the opponent reply).
	pub mcts_tree_reuse: bool,
	/// Share node statistics for Zobrist-identical positions.
	pub mcts_transpositions: bool,
	/// Split ADD into choose-tile then choose-destination.
	pub mcts_factored_moves: bool,
	/// After a cut-off playout, score the leaf with this many ply of
	/// capture-first minimax (0 = static eval).
	pub playout_leaf_depth: u8,
	/// Propagate proven wins/losses so settled lines stop getting visits.
	pub mcts_solver: bool,
}

impl SearchConfig {
	/// Live default: LMR + PVS + quiescence, heuristic playouts, **no** PW.
	pub fn enhanced() -> Self {
		Self {
			move_ordering: true,
			order_tt_bonus: 1_000_000,
			order_killer_bonus: 90_000,
			order_center_bonus: 8_000,
			order_capture_scale: 100,
			transposition: true,
			tt_bits: 18,
			lmr: true,
			lmr_min_depth: 3,
			lmr_move_skip: 3,
			lmr_reduce: 1,
			lmr_late_move: 8,
			lmr_late_depth: 5,
			lmr_late_reduce: 2,
			lmr_skip_root: true,
			pvs: true,
			quiescence: true,
			q_depth: 4,
			q_capture_cap: 8,
			mcts_progressive_widening: false,
			pw_a: 1.5,
			pw_alpha: 0.5,
			mcts_heuristic_playouts: true,
			playout_cutoff: 12,
			playout_epsilon: 0.18,
			mcts_untried_random: 0.25,
			mcts_root_threads: 1,
			puct: 1.6,
			rave_eq: 300.0,
			mcts_move_priors: false,
			prior_temperature: 1.5,
			prior_widening: false,
			mcts_tree_reuse: true,
			mcts_transpositions: false,
			mcts_factored_moves: false,
			playout_leaf_depth: 0,
			mcts_solver: false,
		}
	}

	/// PR #62 / pre-pruning engine.
	pub fn classic() -> Self {
		let mut cfg = Self::enhanced();
		cfg.lmr = false;
		cfg.pvs = false;
		cfg.mcts_progressive_widening = false;
		cfg
	}

	/// Plain alpha-beta / vanilla UCT. For correctness tests.
	pub fn plain() -> Self {
		let mut cfg = Self::enhanced();
		cfg.move_ordering = false;
		cfg.transposition = false;
		cfg.lmr = false;
		cfg.pvs = false;
		cfg.quiescence = false;
		cfg.mcts_progressive_widening = false;
		cfg.mcts_heuristic_playouts = false;
		cfg
	}

	/// Aggressive / tactical: deeper qsearch, heavier captures, short playouts.
	pub fn tactical() -> Self {
		let mut cfg = Self::enhanced();
		cfg.q_depth = 8;
		cfg.order_capture_scale = 160;
		cfg.order_center_bonus = 20_000;
		cfg.playout_cutoff = 6;
		cfg.puct = 1.125;
		cfg.playout_epsilon = 0.125;
		cfg
	}

	/// Cautious / positional: lighter LMR, longer playouts, quieter qsearch.
	pub fn positional() -> Self {
		let mut cfg = Self::enhanced();
		cfg.q_depth = 2;
		cfg.lmr_min_depth = 4;
		cfg.lmr_reduce = 1;
		cfg.lmr_late_reduce = 1;
		cfg.order_capture_scale = 70;
		cfg.order_center_bonus = 16_000;
		cfg.playout_cutoff = 18;
		cfg.puct = 1.25;
		cfg
	}

	/// Explorative: progressive widening on, high PUCT, noisier playouts.
	pub fn explorative() -> Self {
		let mut cfg = Self::enhanced();
		cfg.mcts_progressive_widening = true;
		cfg.pw_a = 2.0;
		cfg.pw_alpha = 0.5;
		cfg.puct = 2.25;
		cfg.playout_epsilon = 0.375;
		cfg.mcts_untried_random = 0.375;
		cfg
	}

	/// Focused MCTS: move-ordering softmax priors, prior-ordered PW, lower
	/// PUCT, tree reuse, and a win/loss solver. Ablation dropped factored
	/// ADDs, leaf search, and MCTS transpositions (they spread visits or
	/// shallowed the tree on the opening/mid positions).
	pub fn deep_mcts() -> Self {
		let mut cfg = Self::enhanced();
		cfg.mcts_move_priors = true;
		cfg.prior_temperature = 1.25;
		cfg.prior_widening = true;
		cfg.mcts_progressive_widening = true;
		cfg.pw_a = 1.0;
		cfg.pw_alpha = 0.4;
		cfg.puct = 0.9;
		cfg.mcts_untried_random = 0.0;
		cfg.mcts_tree_reuse = true;
		cfg.mcts_transpositions = false;
		cfg.mcts_factored_moves = false;
		cfg.playout_leaf_depth = 0;
		cfg.mcts_solver = true;
		cfg
	}

	pub fn parse_profile(s: &str) -> Result<Self, String> {
		match s.trim().to_ascii_lowercase().as_str() {
			"enhanced" | "new" | "full" | "pruned" | "default" => Ok(Self::enhanced()),
			"classic" | "old" | "baseline" => Ok(Self::classic()),
			"plain" | "ab" | "alpha-beta" | "alphabeta" => Ok(Self::plain()),
			"tactical" | "aggressive" | "aggro" => Ok(Self::tactical()),
			"positional" | "cautious" | "solid" => Ok(Self::positional()),
			"explorative" | "explore" | "curious" => Ok(Self::explorative()),
			"deep_mcts" | "deep-mcts" | "deep" | "focused" => Ok(Self::deep_mcts()),
			other => Err(format!(
				"unknown --search-profile '{other}' (expected enhanced|classic|plain|tactical|positional|explorative|deep_mcts)"
			)),
		}
	}

	pub fn profile_names() -> &'static [&'static str] {
		&["enhanced", "classic", "plain", "tactical", "positional", "explorative", "deep_mcts"]
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
		} else if core == Self::tactical() {
			"tactical"
		} else if core == Self::positional() {
			"positional"
		} else if core == Self::explorative() {
			"explorative"
		} else if core == Self::deep_mcts() {
			"deep_mcts"
		} else {
			"custom"
		}
	}

	/// Overlay `key=on|off` / numeric pairs onto `self`.
	pub fn with_spec(mut self, spec: &str) -> Result<Self, String> {
		for part in spec.split(',') {
			let part = part.trim();
			if part.is_empty() {
				continue;
			}
			let (key, val) = part.split_once('=').ok_or_else(|| {
				format!("invalid --search '{spec}' (expected key=on|off or key=N)")
			})?;
			let key = key.trim().to_ascii_lowercase();
			let val = val.trim();
			match key.as_str() {
				"ordering" | "move-ordering" | "move_ordering" => {
					self.move_ordering = parse_on_off(&key, val)?;
				}
				"order-tt" | "order_tt" => self.order_tt_bonus = parse_i32(&key, val)?,
				"order-killer" | "order_killer" => {
					self.order_killer_bonus = parse_i32(&key, val)?;
				}
				"order-center" | "order_center" => {
					self.order_center_bonus = parse_i32(&key, val)?;
				}
				"order-capture" | "order_capture" | "order-capture-scale" => {
					self.order_capture_scale = parse_i32(&key, val)?;
				}
				"tt" | "transposition" | "hash" => {
					self.transposition = parse_on_off(&key, val)?;
				}
				"tt-bits" | "tt_bits" => {
					self.tt_bits = parse_u8_range(&key, val, 12, 22)?;
				}
				"lmr" => self.lmr = parse_on_off(&key, val)?,
				"lmr-min-depth" | "lmr_min_depth" => {
					self.lmr_min_depth = parse_u8_range(&key, val, 1, 16)?;
				}
				"lmr-move-skip" | "lmr_move_skip" => {
					self.lmr_move_skip = parse_u8_range(&key, val, 0, 32)?;
				}
				"lmr-reduce" | "lmr_reduce" => {
					self.lmr_reduce = parse_u8_range(&key, val, 1, 8)?;
				}
				"lmr-late-move" | "lmr_late_move" => {
					self.lmr_late_move = parse_u8_range(&key, val, 1, 64)?;
				}
				"lmr-late-depth" | "lmr_late_depth" => {
					self.lmr_late_depth = parse_u8_range(&key, val, 1, 16)?;
				}
				"lmr-late-reduce" | "lmr_late_reduce" => {
					self.lmr_late_reduce = parse_u8_range(&key, val, 1, 8)?;
				}
				"lmr-skip-root" | "lmr_skip_root" => {
					self.lmr_skip_root = parse_on_off(&key, val)?;
				}
				"pvs" | "null-window" | "null_window" => {
					self.pvs = parse_on_off(&key, val)?;
				}
				"quiescence" | "qsearch" | "q" => {
					self.quiescence = parse_on_off(&key, val)?;
				}
				"q-depth" | "q_depth" | "qsearch-depth" => {
					self.q_depth = parse_u8_range(&key, val, 1, 16)?;
				}
				"q-cap" | "q_capture_cap" | "q-capture-cap" => {
					self.q_capture_cap = parse_u8_range(&key, val, 1, 32)?;
				}
				"pw" | "progressive-widening" | "progressive_widening" | "widening" => {
					self.mcts_progressive_widening = parse_on_off(&key, val)?;
				}
				"pw-a" | "pw_a" => self.pw_a = parse_f32(&key, val)?,
				"pw-alpha" | "pw_alpha" => self.pw_alpha = parse_f32(&key, val)?,
				"heuristic-playouts" | "heuristic_playouts" | "playouts" | "cut-playouts" => {
					self.mcts_heuristic_playouts = parse_on_off(&key, val)?;
				}
				"playout-cutoff" | "playout_cutoff" => {
					self.playout_cutoff = parse_u8_range(&key, val, 1, 64)?;
				}
				"playout-epsilon" | "playout_epsilon" | "epsilon" => {
					self.playout_epsilon = parse_f32(&key, val)?;
				}
				"untried-random" | "mcts_untried_random" => {
					self.mcts_untried_random = parse_f32(&key, val)?;
				}
				"threads" | "search-threads" | "root-threads" => {
					self.mcts_root_threads = parse_u8_range(&key, val, 1, 32)?;
				}
				"puct" | "c-puct" | "exploration" => self.puct = parse_f32(&key, val)?,
				"rave-eq" | "rave_eq" => self.rave_eq = parse_f32(&key, val)?,
				"priors" | "move-priors" | "mcts_move_priors" | "mcts-move-priors" => {
					self.mcts_move_priors = parse_on_off(&key, val)?;
				}
				"prior-temp" | "prior_temperature" | "prior-temperature" => {
					self.prior_temperature = parse_f32(&key, val)?;
				}
				"prior-widening" | "prior_widening" | "pw-order" | "pw_order" => {
					self.prior_widening = parse_on_off(&key, val)?;
				}
				"reuse" | "tree-reuse" | "mcts_tree_reuse" | "mcts-tree-reuse" => {
					self.mcts_tree_reuse = parse_on_off(&key, val)?;
				}
				"mcts-tt" | "mcts_transpositions" | "mcts-transpositions" | "transpositions" => {
					self.mcts_transpositions = parse_on_off(&key, val)?;
				}
				"factored" | "factored-moves" | "mcts_factored_moves" | "mcts-factored-moves" => {
					self.mcts_factored_moves = parse_on_off(&key, val)?;
				}
				"leaf-depth" | "playout_leaf_depth" | "playout-leaf-depth" | "leaf" => {
					self.playout_leaf_depth = parse_u8_range(&key, val, 0, 4)?;
				}
				"solver" | "mcts-solver" | "mcts_solver" => {
					self.mcts_solver = parse_on_off(&key, val)?;
				}
				other => {
					return Err(format!(
						"unknown --search key '{other}' (see --list-search-profiles)"
					));
				}
			}
		}
		self.clamp();
		Ok(self)
	}

	pub fn with_threads(mut self, n: u8) -> Self {
		self.mcts_root_threads = n.max(1);
		self
	}

	pub fn load_path(path: &Path) -> Result<Self, String> {
		let body = std::fs::read_to_string(path)
			.map_err(|e| format!("failed to read {}: {e}", path.display()))?;
		Self::parse_toml(&body).map_err(|e| format!("invalid bot config {}: {e}", path.display()))
	}

	pub fn parse_toml(body: &str) -> Result<Self, String> {
		let mut cfg: SearchConfig = toml::from_str(body).map_err(|e| e.to_string())?;
		cfg.clamp();
		Ok(cfg)
	}

	pub fn list_text() -> String {
		let mut s = String::from("# search profiles (also load any TOML via --bot-config)\n");
		for name in Self::profile_names() {
			let cfg = Self::parse_profile(name).expect("builtin profile");
			s.push_str(&format!("# {name}\n#   {}\n", cfg.label()));
		}
		s
	}

	fn clamp(&mut self) {
		self.tt_bits = self.tt_bits.clamp(12, 22);
		self.lmr_min_depth = self.lmr_min_depth.clamp(1, 16);
		self.lmr_reduce = self.lmr_reduce.max(1);
		self.lmr_late_reduce = self.lmr_late_reduce.max(1);
		self.q_depth = self.q_depth.clamp(1, 16);
		self.q_capture_cap = self.q_capture_cap.clamp(1, 32);
		self.playout_cutoff = self.playout_cutoff.clamp(1, 64);
		self.mcts_root_threads = self.mcts_root_threads.clamp(1, 32);
		if self.puct < 0.0 {
			self.puct = 0.0;
		}
		self.playout_epsilon = self.playout_epsilon.clamp(0.0, 1.0);
		self.mcts_untried_random = self.mcts_untried_random.clamp(0.0, 1.0);
		if self.pw_a < 0.0 {
			self.pw_a = 0.0;
		}
		if self.pw_alpha < 0.0 {
			self.pw_alpha = 0.0;
		}
		if self.rave_eq < 1.0 {
			self.rave_eq = 1.0;
		}
		if self.order_capture_scale < 0 {
			self.order_capture_scale = 0;
		}
		if self.prior_temperature < 0.05 {
			self.prior_temperature = 0.05;
		}
		self.playout_leaf_depth = self.playout_leaf_depth.min(4);
	}

	pub fn label(self) -> String {
		format!(
			"profile={} ordering={} tt={}/{} lmr={} pvs={} q={}/{} pw={} heur_playouts={} puct={:.2} cutoff={} threads={} priors={} pw_order={} reuse={} mcts_tt={} factored={} leaf={} solver={}",
			self.profile_name(),
			on_off(self.move_ordering),
			on_off(self.transposition),
			self.tt_bits,
			on_off(self.lmr),
			on_off(self.pvs),
			on_off(self.quiescence),
			self.q_depth,
			on_off(self.mcts_progressive_widening),
			on_off(self.mcts_heuristic_playouts),
			self.puct,
			self.playout_cutoff,
			self.mcts_root_threads,
			on_off(self.mcts_move_priors),
			on_off(self.prior_widening),
			on_off(self.mcts_tree_reuse),
			on_off(self.mcts_transpositions),
			on_off(self.mcts_factored_moves),
			self.playout_leaf_depth,
			on_off(self.mcts_solver)
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

fn parse_u8_range(flag: &str, value: &str, lo: u8, hi: u8) -> Result<u8, String> {
	let n: u64 = value
		.parse()
		.map_err(|_| format!("invalid {flag} '{value}' (expected {lo}..{hi})"))?;
	if n < u64::from(lo) || n > u64::from(hi) {
		return Err(format!("invalid {flag} '{value}' (expected {lo}..{hi})"));
	}
	Ok(n as u8)
}

fn parse_i32(flag: &str, value: &str) -> Result<i32, String> {
	value
		.parse()
		.map_err(|_| format!("invalid {flag} '{value}' (expected integer)"))
}

fn parse_f32(flag: &str, value: &str) -> Result<f32, String> {
	value.parse().map_err(|_| format!("invalid {flag} '{value}' (expected number)"))
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
		for name in SearchConfig::profile_names() {
			let cfg = SearchConfig::parse_profile(name).unwrap();
			assert_eq!(cfg.profile_name(), *name, "{name}");
		}
		assert_eq!(SearchConfig::parse_profile("aggressive").unwrap(), SearchConfig::tactical());
		assert_eq!(SearchConfig::parse_profile("cautious").unwrap(), SearchConfig::positional());
		assert!(SearchConfig::parse_profile("stockfish").is_err());
	}

	#[test]
	fn spec_overlays_a_profile() {
		let cfg = SearchConfig::classic()
			.with_spec("lmr=on,pvs=on,pw=on,threads=4,q-depth=6,puct=2.5")
			.unwrap();
		assert!(cfg.lmr && cfg.pvs && cfg.mcts_progressive_widening);
		assert_eq!(cfg.mcts_root_threads, 4);
		assert_eq!(cfg.q_depth, 6);
		assert!((cfg.puct - 2.5).abs() < 1e-6);
		assert!(cfg.transposition);
		assert_eq!(cfg.profile_name(), "custom");
		// classic + LMR + PVS, PW still off, is enhanced (threads ignored).
		let promoted = SearchConfig::classic().with_spec("lmr=on,pvs=on,threads=4").unwrap();
		assert_eq!(promoted.profile_name(), "enhanced");
		assert_eq!(promoted.mcts_root_threads, 4);
	}

	#[test]
	fn enhanced_default_keeps_pw_off() {
		let cfg = SearchConfig::enhanced();
		assert!(cfg.lmr && cfg.pvs && cfg.quiescence);
		assert!(!cfg.mcts_progressive_widening);
		assert_eq!(SearchConfig::default(), cfg);
	}

	#[test]
	fn toml_partial_fills_enhanced_defaults() {
		let cfg = SearchConfig::parse_toml(
			"name = \"scratch\"\ndescription = \"pw on\"\nmcts_progressive_widening = true\npuct = 2.0\n",
		)
		.unwrap();
		assert!(cfg.lmr && cfg.quiescence);
		assert!(cfg.mcts_progressive_widening);
		assert!((cfg.puct - 2.0).abs() < 1e-6);
		assert_eq!(cfg.tt_bits, 18);
	}

	#[test]
	fn example_personality_tomls_match_presets() {
		let tactical = SearchConfig::parse_toml(include_str!("../../bots/tactical.toml")).unwrap();
		let positional =
			SearchConfig::parse_toml(include_str!("../../bots/positional.toml")).unwrap();
		let explorative =
			SearchConfig::parse_toml(include_str!("../../bots/explorative.toml")).unwrap();
		assert_eq!(tactical, SearchConfig::tactical());
		assert_eq!(positional, SearchConfig::positional());
		assert_eq!(explorative, SearchConfig::explorative());
		let deep = SearchConfig::parse_toml(include_str!("../../bots/deep_mcts.toml")).unwrap();
		assert_eq!(deep, SearchConfig::deep_mcts());
	}

	#[test]
	fn deep_mcts_enables_focus_levers() {
		let cfg = SearchConfig::deep_mcts();
		assert!(cfg.mcts_move_priors && cfg.prior_widening && cfg.mcts_progressive_widening);
		assert!(cfg.mcts_tree_reuse && cfg.mcts_solver);
		assert!(!cfg.mcts_transpositions && !cfg.mcts_factored_moves);
		assert_eq!(cfg.playout_leaf_depth, 0);
		assert!(cfg.puct < SearchConfig::enhanced().puct);
		assert_eq!(SearchConfig::parse_profile("focused").unwrap(), cfg);
		assert_eq!(cfg.profile_name(), "deep_mcts");
	}

	#[test]
	fn spec_overlays_deep_mcts_knobs() {
		let cfg = SearchConfig::enhanced()
			.with_spec("priors=on,pw-order=on,reuse=off,mcts-tt=on,factored=on,leaf=2,solver=on")
			.unwrap();
		assert!(cfg.mcts_move_priors && cfg.prior_widening);
		assert!(!cfg.mcts_tree_reuse);
		assert!(cfg.mcts_transpositions && cfg.mcts_factored_moves && cfg.mcts_solver);
		assert_eq!(cfg.playout_leaf_depth, 2);
		assert_eq!(cfg.profile_name(), "custom");
	}
}
