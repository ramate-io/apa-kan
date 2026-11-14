//! Fail-closed CLI parsing. Unknown arguments print usage and exit 1.

use std::env;
use std::path::PathBuf;
use std::process;
use std::time::Duration;

use crate::ai::Actor;
use crate::deck::{PickStrategy, SetupConfig, DECK_SIZE_DEFAULT, DECK_SIZE_MAX};
use crate::game::{
	BraidFailMismatch, HopWeightCmp, InsufficientTilesLoss, PassThroughRingCmp, PathConstraint,
	RuleSet, SamePieceTwice, ZagMismatchCount, BAG_SIZE,
};
use crate::scenario::Scenario;

#[derive(Clone, Debug)]
pub struct GameConfig {
	pub bot1: Actor,
	pub bot2: Actor,
	pub seed: u64,
	pub think: Duration,
	pub delay: Duration,
	pub screenshot: Option<PathBuf>,
	/// When set with `--screenshot`, capture `Game::start` (opening deal)
	/// instead of the mid-game fixture.
	pub screenshot_from_start: bool,
	pub width: u32,
	pub height: u32,
	pub rules: RuleSet,
	pub scenario: Option<Scenario>,
	pub setup: SetupConfig,
}

impl Default for GameConfig {
	fn default() -> Self {
		Self {
			bot1: Actor::Human,
			bot2: Actor::Greedy,
			seed: 1,
			think: Duration::from_millis(80),
			delay: Duration::from_millis(350),
			screenshot: None,
			screenshot_from_start: false,
			width: 1280,
			height: 900,
			rules: RuleSet::default(),
			scenario: None,
			setup: SetupConfig::default(),
		}
	}
}

impl GameConfig {
	pub fn from_args() -> Self {
		#[cfg(any(target_os = "android", target_os = "ios"))]
		{
			return Self::default();
		}
		#[cfg(not(any(target_os = "android", target_os = "ios")))]
		{
			match Self::parse(env::args().skip(1)) {
				Ok(cfg) => cfg,
				Err(err) => {
					eprintln!("{err}\n\n{}", usage());
					process::exit(1);
				}
			}
		}
	}

	pub fn parse<I, S>(args: I) -> Result<Self, String>
	where
		I: IntoIterator<Item = S>,
		S: AsRef<str>,
	{
		let mut cfg = Self::default();
		let mut it = args.into_iter();
		while let Some(raw) = it.next() {
			let arg = raw.as_ref();
			match arg {
				"-h" | "--help" => {
					print!("{}", usage());
					process::exit(0);
				}
				"--bot1" => {
					cfg.bot1 = Actor::parse(&require_value("--bot1", it.next())?)?;
				}
				"--bot2" => {
					cfg.bot2 = Actor::parse(&require_value("--bot2", it.next())?)?;
				}
				"--seed" => {
					cfg.seed = parse_u64("--seed", &require_value("--seed", it.next())?)?;
				}
				"--deck-seed" => {
					cfg.setup.deck_seed =
						Some(parse_u64("--deck-seed", &require_value("--deck-seed", it.next())?)?);
				}
				"--deck-noise" => {
					let n = parse_u64("--deck-noise", &require_value("--deck-noise", it.next())?)?;
					if n > 100 {
						return Err("invalid --deck-noise (expected 0..100 percent)".into());
					}
					cfg.setup.deck_noise = n as u8;
				}
				"--pick" => {
					cfg.setup.pick = PickStrategy::parse(&require_value("--pick", it.next())?)?;
				}
				"--think-ms" => {
					let ms = parse_u64("--think-ms", &require_value("--think-ms", it.next())?)?;
					cfg.think = Duration::from_millis(ms);
				}
				"--delay-ms" => {
					let ms = parse_u64("--delay-ms", &require_value("--delay-ms", it.next())?)?;
					cfg.delay = Duration::from_millis(ms);
				}
				"--screenshot" => {
					cfg.screenshot = Some(PathBuf::from(require_value("--screenshot", it.next())?));
				}
				"--from-start" => {
					cfg.screenshot_from_start = true;
				}
				"--scenario" => {
					cfg.scenario = Some(Scenario::load(&require_value("--scenario", it.next())?)?);
				}
				"--list-scenarios" => {
					print!("{}", Scenario::list_text());
					process::exit(0);
				}
				"--width" => {
					cfg.width = parse_u32("--width", &require_value("--width", it.next())?)?;
				}
				"--height" => {
					cfg.height = parse_u32("--height", &require_value("--height", it.next())?)?;
				}
				"--actions" => {
					let n = parse_u64("--actions", &require_value("--actions", it.next())?)?;
					if n == 0 || n > 8 {
						return Err("invalid --actions (expected 1..8)".into());
					}
					cfg.rules.actions_per_turn = n as u8;
				}
				"--turn-limit" => {
					let n = parse_u64("--turn-limit", &require_value("--turn-limit", it.next())?)?;
					if n == 0 {
						return Err("invalid --turn-limit (expected >= 1)".into());
					}
					cfg.rules.turn_limit = n as u32;
				}
				other => {
					if apply_rule_flag(&mut cfg.rules, other, &mut it)? {
						continue;
					}
					return Err(format!("unknown argument '{other}'"));
				}
			}
		}
		validate_rule_flags(&cfg.rules)?;
		if cfg.setup.deck_seed.is_none() {
			cfg.setup.deck_seed = Some(cfg.seed);
		}
		Ok(cfg)
	}

	pub fn actor(&self, player: crate::game::Player) -> Actor {
		match player {
			crate::game::Player::One => self.bot1,
			crate::game::Player::Two => self.bot2,
		}
	}
}

fn require_value(flag: &str, value: Option<impl AsRef<str>>) -> Result<String, String> {
	match value {
		Some(v) => Ok(v.as_ref().to_string()),
		None => Err(format!("missing value for {flag}")),
	}
}

fn parse_u64(flag: &str, value: &str) -> Result<u64, String> {
	value.parse::<u64>().map_err(|_| format!("invalid {flag} '{value}'"))
}

fn parse_bool(flag: &str, value: &str) -> Result<bool, String> {
	match value.to_ascii_lowercase().as_str() {
		"on" | "true" | "1" | "yes" => Ok(true),
		"off" | "false" | "0" | "no" => Ok(false),
		other => Err(format!("invalid {flag} '{other}' (expected on|off)")),
	}
}

fn parse_u32(flag: &str, value: &str) -> Result<u32, String> {
	let n = parse_u64(flag, value)?;
	if n < 320 || n > 7680 {
		return Err(format!("invalid {flag} '{value}' (expected 320..7680)"));
	}
	u32::try_from(n).map_err(|_| format!("invalid {flag} '{value}'"))
}

/// Apply a RuleSet CLI flag. Returns `Ok(true)` if `arg` was consumed.
pub fn apply_rule_flag<I, S>(rules: &mut RuleSet, arg: &str, it: &mut I) -> Result<bool, String>
where
	I: Iterator<Item = S>,
	S: AsRef<str>,
{
	match arg {
		"--no-moves-loses" => {
			rules.no_moves_loses = parse_bool(arg, &require_value(arg, it.next())?)?;
		}
		"--center-hold-win" => {
			rules.center_hold_win = parse_bool(arg, &require_value(arg, it.next())?)?;
		}
		"--center-hold-n" => {
			let n = parse_u64(arg, &require_value(arg, it.next())?)?;
			if n == 0 || n > 64 {
				return Err("invalid --center-hold-n (expected 1..64)".into());
			}
			rules.center_hold_n = n as u8;
		}
		"--no-same-piece-twice" => {
			rules.no_same_piece_twice = SamePieceTwice::parse(&require_value(arg, it.next())?)?;
		}
		"--color-restricted-hops" => {
			rules.color_restricted_hops = parse_bool(arg, &require_value(arg, it.next())?)?;
		}
		"--counted-pass-through" => {
			rules.counted_pass_through = parse_bool(arg, &require_value(arg, it.next())?)?;
		}
		"--graph-hops" => {
			rules.counted_pass_through = !parse_bool(arg, &require_value(arg, it.next())?)?;
		}
		"--pass-through-ring-cmp" => {
			rules.pass_through_ring_cmp =
				PassThroughRingCmp::parse(&require_value(arg, it.next())?)?;
		}
		"--hop-weight-cmp" => {
			rules.hop_weight_cmp = HopWeightCmp::parse(&require_value(arg, it.next())?)?;
		}
		"--petal-majority-win" => {
			rules.petal_majority_win = parse_bool(arg, &require_value(arg, it.next())?)?;
		}
		"--petal-majority-n" => {
			let n = parse_u64(arg, &require_value(arg, it.next())?)?;
			if n == 0 || n > 64 {
				return Err("invalid --petal-majority-n (expected 1..64)".into());
			}
			rules.petal_majority_n = n as u8;
		}
		"--control-win" => {
			rules.control_win = parse_bool(arg, &require_value(arg, it.next())?)?;
		}
		"--control-n" => {
			let n = parse_u64(arg, &require_value(arg, it.next())?)?;
			if n == 0 || n > 9 {
				return Err("invalid --control-n (expected 1..9)".into());
			}
			rules.control_n = n as u8;
		}
		"--control-hold-turns" => {
			let n = parse_u64(arg, &require_value(arg, it.next())?)?;
			if n == 0 || n > 64 {
				return Err("invalid --control-hold-turns (expected 1..64)".into());
			}
			rules.control_hold_turns = n as u8;
		}
		"--insufficient-tiles-loss" => {
			rules.insufficient_tiles_loss =
				InsufficientTilesLoss::parse(&require_value(arg, it.next())?)?;
		}
		"--starting-bench" | "--starting-tokens" => {
			let n = parse_u64(arg, &require_value(arg, it.next())?)?;
			if n == 0 || n > BAG_SIZE as u64 {
				return Err(format!(
					"invalid {arg} (expected 1..{BAG_SIZE} spawn tokens, independent of deck size)"
				));
			}
			rules.starting_bench_size = n as u8;
		}
		"--bag-size" | "--deck-size" => {
			let n = parse_u64(arg, &require_value(arg, it.next())?)?;
			if n == 0 || n > DECK_SIZE_MAX as u64 {
				return Err(format!(
					"invalid {arg} (expected 1..{DECK_SIZE_MAX} collection tiles, default {DECK_SIZE_DEFAULT})"
				));
			}
			rules.deck_size = n as u8;
		}
		"--self-capture-budget" => {
			rules.self_capture_budget = parse_bool(arg, &require_value(arg, it.next())?)?;
		}
		"--max-rings" => {
			let n = parse_u64(arg, &require_value(arg, it.next())?)?;
			if n == 0 || n > 3 {
				return Err("invalid --max-rings (expected 1..3)".into());
			}
			rules.max_rings = n as u8;
		}
		"--zag-chance" => {
			let n = parse_u64(arg, &require_value(arg, it.next())?)?;
			if n > 7 {
				return Err("invalid --zag-chance (expected 0..7, k/7)".into());
			}
			rules.zag_numer = n as u8;
		}
		"--zags" => {
			let on = parse_bool(arg, &require_value(arg, it.next())?)?;
			rules.zag_numer = if on { crate::game::ZAG_NUMER_DEFAULT } else { 0 };
		}
		"--braid-chance" => {
			let n = parse_u64(arg, &require_value(arg, it.next())?)?;
			if n > 7 {
				return Err("invalid --braid-chance (expected 0..7, k/7)".into());
			}
			rules.braid_numer = n as u8;
		}
		"--braids" => {
			let on = parse_bool(arg, &require_value(arg, it.next())?)?;
			rules.braid_numer = if on { crate::game::BRAID_NUMER_DEFAULT } else { 0 };
		}
		"--bead-chance" => {
			let n = parse_u64(arg, &require_value(arg, it.next())?)?;
			if n > 7 {
				return Err("invalid --bead-chance (expected 0..7, k/7)".into());
			}
			rules.bead_numer = n as u8;
		}
		"--beads" => {
			let on = parse_bool(arg, &require_value(arg, it.next())?)?;
			rules.bead_numer = if on { crate::game::BEAD_NUMER_DEFAULT } else { 0 };
		}
		"--dot-chance" => {
			let n = parse_u64(arg, &require_value(arg, it.next())?)?;
			if n > 100 {
				return Err("invalid --dot-chance (expected 0..100 percent)".into());
			}
			rules.dot_percent = n as u8;
		}
		"--zag-mismatch" => {
			rules.zag_mismatch = ZagMismatchCount::parse(&require_value(arg, it.next())?)?;
		}
		"--braid-fail-mismatch" => {
			rules.braid_fail_mismatch = BraidFailMismatch::parse(&require_value(arg, it.next())?)?;
		}
		"--path-constraint" => {
			rules.path_constraint = PathConstraint::parse(&require_value(arg, it.next())?)?;
		}
		"--no-repeated-edge" => {
			rules.path_constraint = match parse_bool(arg, &require_value(arg, it.next())?)? {
				true => match rules.path_constraint {
					PathConstraint::NoNodeRevisit | PathConstraint::Both => PathConstraint::Both,
					PathConstraint::NoEdgeRevisit => PathConstraint::NoEdgeRevisit,
				},
				false => PathConstraint::NoNodeRevisit,
			};
		}
		_ => return Ok(false),
	}
	Ok(true)
}

/// Cross-field RuleSet checks that cannot run inside a single flag parse.
pub fn validate_rule_flags(_rules: &RuleSet) -> Result<(), String> {
	Ok(())
}

pub fn usage() -> &'static str {
	"Usage: apa-kan [OPTIONS]\n\
	 \n\
	 Options:\n\
	   --bot1 <human|random|greedy|minimax|minimax-legacy|mcts>  Player 1, light (default: human)\n\
	   --bot2 <human|random|greedy|minimax|minimax-legacy|mcts>  Player 2, dark (default: greedy)\n\
	   --seed <u64>                               Deal/shuffle seed (default: 1)\n\
	   --deck-seed <u64>                          Collection generation seed (default: --seed)\n\
	   --deck-noise <0..100>                      Resample that percent of a mirrored collection (default: 0)\n\
	   --pick <heuristic|search|greedy>           Bench 7+4 picker (default: heuristic)\n\
	   --think-ms <n>                             Bot think time in ms (default: 80)\n\
	   --delay-ms <n>                             Delay between bot moves (default: 350)\n\
	   --actions <1..8>                           Actions per turn (default: 1)\n\
	   --turn-limit <n>                           Safety-net draw (default: 200)\n\
	   --width <px>                               Window width (default: 1280)\n\
	   --height <px>                              Window height (default: 900)\n\
	   --screenshot <path>                        Capture a mid-game board PNG and exit\n\
	   --from-start                               With --screenshot, capture the opening deal\n\
	   --scenario <name|path>                     Opening deck (default|same|complementary|...)\n\
	   --list-scenarios                           Print builtin deck scenarios\n\
	   --no-moves-loses <on|off>                  No ADD/MOVE loses (default: on)\n\
	   --center-hold-win <on|off>                 Center-hold win (default: off)\n\
	   --center-hold-n <1..64>                    Turns to hold center (default: 5)\n\
	   --no-same-piece-twice <off|always|when-deck-empty>\n\
	                                              Repeat-piece rule (default: off)\n\
	   --color-restricted-hops <on|off>           Hop only own color (default: on; hops off by default)\n\
	   --hop-weight-cmp <lt|le>                   Color-hop weight compare (default: le)\n\
	   --counted-pass-through <on|off>            Pass through occupied tiles at 1/step (default: on)\n\
	   --graph-hops <on|off>                      Restore graph hops (alias: counted-pass-through off)\n\
	   --pass-through-ring-cmp <ge|gt>            Pass-through ring gate (default: ge)\n\
	   --petal-majority-win <on|off>              Inner-ring majority win (default: off)\n\
	   --petal-majority-n <1..64>                 Turns to hold majority (default: 1)\n\
	   --control-win <on|off>                     Center+inner-ring control win (default: on)\n\
	   --control-n <1..9>                         Control-set occupancy needed (default: 5)\n\
	   --control-hold-turns <1..64>               Turns to hold control (default: 1)\n\
	   --insufficient-tiles-loss <off|strict|no-recapture>\n\
	                                              Can't-reach-control loss (default: off)\n\
	   --starting-tokens <1..27>                  Opening spawn tokens (default: 7; alias --starting-bench)\n\
	   --deck-size <1..27>                        Collection size per player (default: 27; alias --bag-size)\n\
	   --self-capture-budget <on|off>             Own-tile capture earns budget (default: on)\n\
	   --max-rings <1..3>                         Distinct rings per tile (default: 3)\n\
	   --zag-chance <0..7>                        Each ring is a zag with k/7 chance (default: 1)\n\
	   --zags <on|off>                            Alias for --zag-chance 1 or 0 (default: on)\n\
	   --braid-chance <0..7>                      A piece forms a braid with k/7 chance (default: 1)\n\
	   --braids <on|off>                          Alias for --braid-chance 1 or 0 (default: on)\n\
	   --bead-chance <0..7>                       Each remaining normal ring is a bead with k/7 chance (default: 1)\n\
	   --beads <on|off>                           Alias for --bead-chance 1 or 0 (default: on)\n\
	   --dot-chance <0..100>                      Generated tiles that are dots (rest triangles; default: 50)\n\
	   --zag-mismatch <sum|max>                   Zag edge count (default: sum)\n\
	   --braid-fail-mismatch <one|weight|sum>     Failed-braid mismatch (default: one)\n\
	   --path-constraint <no-node-revisit|no-edge-revisit|both>\n\
	                                              MOVE path uniqueness (default: no-edge-revisit)\n\
	   --no-repeated-edge <on|off>                Alias for path-constraint edge/node\n\
	   -h, --help                                 Print help\n"
}

#[cfg(test)]
mod tests {
	#![allow(clippy::unwrap_used, clippy::expect_used)]

	use super::*;
	use crate::game::BAG_SIZE_DEFAULT;

	#[test]
	fn parses_bot_flags() {
		let cfg =
			GameConfig::parse(["--bot1", "mcts", "--bot2", "minimax", "--seed", "42"]).unwrap();
		assert_eq!(cfg.bot1, Actor::Mcts);
		assert_eq!(cfg.bot2, Actor::Minimax);
		assert_eq!(cfg.seed, 42);
	}

	#[test]
	fn unknown_arg_is_an_error() {
		let err = GameConfig::parse(["--bot3", "human"]).unwrap_err();
		assert!(err.contains("unknown argument"));
	}

	#[test]
	fn parses_screenshot_from_start() {
		let cfg = GameConfig::parse(["--screenshot", "out.png", "--from-start"]).unwrap();
		assert_eq!(cfg.screenshot.as_deref(), Some(std::path::Path::new("out.png")));
		assert!(cfg.screenshot_from_start);
		assert!(!GameConfig::parse(["--screenshot", "out.png"]).unwrap().screenshot_from_start);
	}

	#[test]
	fn unknown_actor_is_an_error() {
		let err = GameConfig::parse(["--bot1", "ore"]).unwrap_err();
		assert!(err.contains("unknown actor"));
	}

	#[test]
	fn missing_value_is_an_error() {
		let err = GameConfig::parse(["--bot2"]).unwrap_err();
		assert!(err.contains("missing value"));
	}

	#[test]
	fn glued_unknown_flag_errors() {
		let err = GameConfig::parse(["--bot1human"]).unwrap_err();
		assert!(err.contains("unknown argument"));
	}

	#[test]
	fn parses_stall_rule_flags() {
		let cfg = GameConfig::parse([
			"--no-moves-loses",
			"off",
			"--center-hold-win",
			"on",
			"--center-hold-n",
			"3",
			"--no-same-piece-twice",
			"always",
			"--color-restricted-hops",
			"on",
			"--hop-weight-cmp",
			"le",
			"--path-constraint",
			"both",
			"--turn-limit",
			"50",
		])
		.unwrap();
		assert!(!cfg.rules.no_moves_loses);
		assert!(cfg.rules.center_hold_win);
		assert_eq!(cfg.rules.center_hold_n, 3);
		assert_eq!(cfg.rules.no_same_piece_twice, SamePieceTwice::Always);
		assert!(cfg.rules.color_restricted_hops);
		assert_eq!(cfg.rules.hop_weight_cmp, HopWeightCmp::Le);
		assert_eq!(cfg.rules.path_constraint, PathConstraint::Both);
		assert_eq!(cfg.rules.turn_limit, 50);
	}

	#[test]
	fn parses_control_and_insufficient_flags() {
		let cfg = GameConfig::parse([
			"--control-win",
			"on",
			"--control-n",
			"5",
			"--control-hold-turns",
			"3",
			"--insufficient-tiles-loss",
			"no-recapture",
			"--center-hold-win",
			"off",
		])
		.unwrap();
		assert!(cfg.rules.control_win);
		assert_eq!(cfg.rules.control_n, 5);
		assert_eq!(cfg.rules.control_hold_turns, 3);
		assert_eq!(
			cfg.rules.insufficient_tiles_loss,
			crate::game::InsufficientTilesLoss::NoRecapture
		);
		assert!(!cfg.rules.center_hold_win);
	}

	#[test]
	fn stall_rule_defaults_match_ruleset() {
		let cfg = GameConfig::parse([] as [&str; 0]).unwrap();
		assert_eq!(cfg.rules, RuleSet::default());
		assert!(cfg.rules.no_moves_loses);
		assert!(!cfg.rules.center_hold_win);
		assert_eq!(cfg.rules.center_hold_n, 5);
		assert_eq!(cfg.rules.no_same_piece_twice, SamePieceTwice::Off);
		assert!(cfg.rules.color_restricted_hops);
		assert_eq!(cfg.rules.hop_weight_cmp, HopWeightCmp::Le);
		assert!(!cfg.rules.petal_majority_win);
		assert_eq!(cfg.rules.petal_majority_n, 1);
		assert!(cfg.rules.control_win);
		assert_eq!(cfg.rules.control_n, 5);
		assert_eq!(cfg.rules.control_hold_turns, 1);
		assert_eq!(cfg.rules.insufficient_tiles_loss, crate::game::InsufficientTilesLoss::Off);
		assert_eq!(cfg.rules.starting_bench_size, 7);
		assert_eq!(cfg.rules.bag_size, crate::game::BAG_SIZE_DEFAULT as u8);
		assert_eq!(cfg.rules.deck_size, DECK_SIZE_DEFAULT as u8);
		assert_eq!(cfg.setup.pick, PickStrategy::Heuristic);
		assert_eq!(cfg.setup.deck_noise, 0);
		assert!(cfg.rules.self_capture_budget);
		assert_eq!(cfg.rules.max_rings, 3);
		assert_eq!(cfg.rules.zag_numer, 1);
		assert_eq!(cfg.rules.braid_numer, 1);
		assert_eq!(cfg.rules.bead_numer, 1);
		assert_eq!(cfg.rules.dot_percent, crate::game::DOT_PERCENT_DEFAULT);
		assert_eq!(cfg.rules.path_constraint, PathConstraint::NoEdgeRevisit);
		assert!(cfg.rules.counted_pass_through);
		assert_eq!(cfg.rules.pass_through_ring_cmp, PassThroughRingCmp::Ge);
		assert_eq!(cfg.rules.zag_mismatch, crate::game::ZagMismatchCount::Sum);
		assert_eq!(cfg.rules.braid_fail_mismatch, crate::game::BraidFailMismatch::One);
	}

	#[test]
	fn parses_zag_mismatch() {
		let sum = GameConfig::parse(["--zag-mismatch", "sum"]).unwrap();
		assert_eq!(sum.rules.zag_mismatch, crate::game::ZagMismatchCount::Sum);
		let max = GameConfig::parse(["--zag-mismatch", "max"]).unwrap();
		assert_eq!(max.rules.zag_mismatch, crate::game::ZagMismatchCount::Max);
		let err = GameConfig::parse(["--zag-mismatch", "collapsed"]).unwrap_err();
		assert!(err.contains("zag-mismatch"));
		let err = GameConfig::parse(["--zag-mismatch", "directional"]).unwrap_err();
		assert!(err.contains("zag-mismatch"));
	}

	#[test]
	fn parses_braid_fail_mismatch() {
		let one = GameConfig::parse(["--braid-fail-mismatch", "one"]).unwrap();
		assert_eq!(one.rules.braid_fail_mismatch, crate::game::BraidFailMismatch::One);
		let weight = GameConfig::parse(["--braid-fail-mismatch", "weight"]).unwrap();
		assert_eq!(weight.rules.braid_fail_mismatch, crate::game::BraidFailMismatch::Weight);
		let sum = GameConfig::parse(["--braid-fail-mismatch", "sum"]).unwrap();
		assert_eq!(sum.rules.braid_fail_mismatch, crate::game::BraidFailMismatch::Sum);
		let err = GameConfig::parse(["--braid-fail-mismatch", "product"]).unwrap_err();
		assert!(err.contains("braid-fail-mismatch"));
	}

	#[test]
	fn parses_counted_pass_through() {
		let on = GameConfig::parse(["--counted-pass-through", "on"]).unwrap();
		assert!(on.rules.counted_pass_through);
		let off = GameConfig::parse(["--counted-pass-through", "off"]).unwrap();
		assert!(!off.rules.counted_pass_through);
	}

	#[test]
	fn parses_graph_hops() {
		let on = GameConfig::parse(["--graph-hops", "on"]).unwrap();
		assert!(!on.rules.counted_pass_through);
		let off = GameConfig::parse(["--graph-hops", "off"]).unwrap();
		assert!(off.rules.counted_pass_through);
	}

	#[test]
	fn parses_pass_through_ring_cmp() {
		let ge = GameConfig::parse(["--pass-through-ring-cmp", "ge"]).unwrap();
		assert_eq!(ge.rules.pass_through_ring_cmp, PassThroughRingCmp::Ge);
		let gt = GameConfig::parse(["--pass-through-ring-cmp", "gt"]).unwrap();
		assert_eq!(gt.rules.pass_through_ring_cmp, PassThroughRingCmp::Gt);
		let err = GameConfig::parse(["--pass-through-ring-cmp", "le"]).unwrap_err();
		assert!(err.contains("pass-through-ring-cmp"));
	}

	#[test]
	fn parses_max_rings() {
		let cfg = GameConfig::parse(["--max-rings", "1"]).unwrap();
		assert_eq!(cfg.rules.max_rings, 1);
		let err = GameConfig::parse(["--max-rings", "4"]).unwrap_err();
		assert!(err.contains("max-rings"));
	}

	#[test]
	fn parses_zag_flags() {
		let off = GameConfig::parse(["--zags", "off"]).unwrap();
		assert_eq!(off.rules.zag_numer, 0);
		let chance = GameConfig::parse(["--zag-chance", "0"]).unwrap();
		assert_eq!(chance.rules.zag_numer, 0);
		let on = GameConfig::parse(["--zags", "on"]).unwrap();
		assert_eq!(on.rules.zag_numer, 1);
		let err = GameConfig::parse(["--zag-chance", "8"]).unwrap_err();
		assert!(err.contains("zag-chance"));
	}

	#[test]
	fn parses_braid_flags() {
		let off = GameConfig::parse(["--braids", "off"]).unwrap();
		assert_eq!(off.rules.braid_numer, 0);
		let chance = GameConfig::parse(["--braid-chance", "0"]).unwrap();
		assert_eq!(chance.rules.braid_numer, 0);
		let on = GameConfig::parse(["--braids", "on"]).unwrap();
		assert_eq!(on.rules.braid_numer, 1);
		let three = GameConfig::parse(["--braid-chance", "3"]).unwrap();
		assert_eq!(three.rules.braid_numer, 3);
		let err = GameConfig::parse(["--braid-chance", "8"]).unwrap_err();
		assert!(err.contains("braid-chance"));
	}

	#[test]
	fn parses_bead_flags() {
		let off = GameConfig::parse(["--beads", "off"]).unwrap();
		assert_eq!(off.rules.bead_numer, 0);
		let chance = GameConfig::parse(["--bead-chance", "0"]).unwrap();
		assert_eq!(chance.rules.bead_numer, 0);
		let on = GameConfig::parse(["--beads", "on"]).unwrap();
		assert_eq!(on.rules.bead_numer, 1);
		let three = GameConfig::parse(["--bead-chance", "3"]).unwrap();
		assert_eq!(three.rules.bead_numer, 3);
		let err = GameConfig::parse(["--bead-chance", "8"]).unwrap_err();
		assert!(err.contains("bead-chance"));
	}

	#[test]
	fn parses_starting_bench() {
		let cfg = GameConfig::parse(["--starting-bench", "5"]).unwrap();
		assert_eq!(cfg.rules.starting_bench_size, 5);
		let err = GameConfig::parse(["--starting-bench", "28"]).unwrap_err();
		assert!(err.contains("starting-bench"));
	}

	#[test]
	fn parses_bag_size() {
		let default = GameConfig::parse([] as [&str; 0]).unwrap();
		assert_eq!(default.rules.bag_size, BAG_SIZE_DEFAULT as u8);
		assert_eq!(default.rules.deck_size, DECK_SIZE_DEFAULT as u8);
		let eleven = GameConfig::parse(["--bag-size", "11"]).unwrap();
		assert_eq!(eleven.rules.deck_size, 11);
		assert_eq!(eleven.rules.bag_size, BAG_SIZE_DEFAULT as u8);
		assert_eq!(eleven.rules.starting_bench_size, 7);
		let full = GameConfig::parse(["--bag-size", "27"]).unwrap();
		assert_eq!(full.rules.deck_size, BAG_SIZE as u8);
		assert_eq!(full.rules.bag_size, BAG_SIZE_DEFAULT as u8);
		let err = GameConfig::parse(["--bag-size", "28"]).unwrap_err();
		assert!(err.contains("bag-size"));
		assert!(err.contains("default 27"), "{err}");
		let err = GameConfig::parse(["--bag-size", "0"]).unwrap_err();
		assert!(err.contains("bag-size"));
	}

	#[test]
	fn tokens_are_independent_of_deck_size() {
		let small = GameConfig::parse(["--bag-size", "5"]).unwrap();
		assert_eq!(small.rules.deck_size, 5);
		assert_eq!(small.rules.starting_bench_size, 7);
		let extra = GameConfig::parse(["--deck-size", "11", "--starting-tokens", "12"]).unwrap();
		assert_eq!(extra.rules.deck_size, 11);
		assert_eq!(extra.rules.starting_bench_size, 12);
		let ok = GameConfig::parse(["--bag-size", "11", "--starting-bench", "7"]).unwrap();
		assert_eq!(ok.rules.deck_size, 11);
		assert_eq!(ok.rules.starting_bench_size, 7);
		let dealt = GameConfig::parse(["--bag-size", "7", "--starting-bench", "7"]).unwrap();
		assert_eq!(dealt.rules.deck_size, 7);
	}

	#[test]
	fn parses_starting_tokens_and_deck_size_aliases() {
		let tokens = GameConfig::parse(["--starting-tokens", "5"]).unwrap();
		assert_eq!(tokens.rules.starting_bench_size, 5);
		let deck = GameConfig::parse(["--deck-size", "13"]).unwrap();
		assert_eq!(deck.rules.deck_size, 13);
		assert_eq!(deck.rules.bag_size, BAG_SIZE_DEFAULT as u8);
	}

	#[test]
	fn parses_deck_seed_noise_and_pick() {
		let cfg = GameConfig::parse([
			"--seed",
			"3",
			"--deck-seed",
			"99",
			"--deck-noise",
			"40",
			"--pick",
			"search",
		])
		.unwrap();
		assert_eq!(cfg.seed, 3);
		assert_eq!(cfg.setup.deck_seed, Some(99));
		assert_eq!(cfg.setup.deck_noise, 40);
		assert_eq!(cfg.setup.pick, PickStrategy::Search);
		let inherit = GameConfig::parse(["--seed", "8"]).unwrap();
		assert_eq!(inherit.setup.deck_seed, Some(8));
		let err = GameConfig::parse(["--pick", "random"]).unwrap_err();
		assert!(err.contains("--pick") || err.contains("unknown"));
		let err = GameConfig::parse(["--deck-noise", "101"]).unwrap_err();
		assert!(err.contains("deck-noise"));
	}

	#[test]
	fn parses_self_capture_budget() {
		let on = GameConfig::parse(["--self-capture-budget", "on"]).unwrap();
		assert!(on.rules.self_capture_budget);
		let off = GameConfig::parse(["--self-capture-budget", "off"]).unwrap();
		assert!(!off.rules.self_capture_budget);
	}

	#[test]
	fn parses_dot_chance() {
		let cfg = GameConfig::parse(["--dot-chance", "0"]).unwrap();
		assert_eq!(cfg.rules.dot_percent, 0);
		let cfg = GameConfig::parse(["--dot-chance", "100"]).unwrap();
		assert_eq!(cfg.rules.dot_percent, 100);
		let err = GameConfig::parse(["--dot-chance", "101"]).unwrap_err();
		assert!(err.contains("dot-chance"));
	}

	#[test]
	fn parses_window_size() {
		let cfg = GameConfig::parse(["--width", "1920", "--height", "1080"]).unwrap();
		assert_eq!(cfg.width, 1920);
		assert_eq!(cfg.height, 1080);
	}

	#[test]
	fn parses_scenario() {
		let cfg = GameConfig::parse(["--scenario", "complementary"]).unwrap();
		let s = cfg.scenario.expect("scenario");
		assert_eq!(s.name, "complementary");
		assert!(s.complementary.is_some());
		let err = GameConfig::parse(["--scenario", "nope"]).unwrap_err();
		assert!(err.contains("unknown scenario"), "{err}");
	}
}
