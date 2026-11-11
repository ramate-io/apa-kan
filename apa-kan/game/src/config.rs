//! Fail-closed CLI parsing. Unknown arguments print usage and exit 1.

use std::env;
use std::path::PathBuf;
use std::process;
use std::time::Duration;

use crate::ai::Actor;
use crate::game::{HopWeightCmp, InsufficientTilesLoss, PathConstraint, RuleSet, SamePieceTwice};
use crate::scenario::Scenario;

#[derive(Clone, Debug)]
pub struct GameConfig {
	pub bot1: Actor,
	pub bot2: Actor,
	pub seed: u64,
	pub think: Duration,
	pub delay: Duration,
	pub screenshot: Option<PathBuf>,
	pub width: u32,
	pub height: u32,
	pub rules: RuleSet,
	pub scenario: Option<Scenario>,
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
			width: 1280,
			height: 900,
			rules: RuleSet::default(),
			scenario: None,
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
		"--starting-bench" => {
			let n = parse_u64(arg, &require_value(arg, it.next())?)?;
			if n == 0 || n > 27 {
				return Err("invalid --starting-bench (expected 1..27, bag has 27 tiles)".into());
			}
			rules.starting_bench_size = n as u8;
		}
		"--max-rings" => {
			let n = parse_u64(arg, &require_value(arg, it.next())?)?;
			if n == 0 || n > 3 {
				return Err("invalid --max-rings (expected 1..3)".into());
			}
			rules.max_rings = n as u8;
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

pub fn usage() -> &'static str {
	"Usage: apa-kan [OPTIONS]\n\
	 \n\
	 Options:\n\
	   --bot1 <human|random|greedy|minimax|minimax-legacy|mcts>  Player 1, light (default: human)\n\
	   --bot2 <human|random|greedy|minimax|minimax-legacy|mcts>  Player 2, dark (default: greedy)\n\
	   --seed <u64>                               Deal/shuffle seed (default: 1)\n\
	   --think-ms <n>                             Bot think time in ms (default: 80)\n\
	   --delay-ms <n>                             Delay between bot moves (default: 350)\n\
	   --actions <1..8>                           Actions per turn (default: 1)\n\
	   --turn-limit <n>                           Safety-net draw (default: 200)\n\
	   --width <px>                               Window width (default: 1280)\n\
	   --height <px>                              Window height (default: 900)\n\
	   --screenshot <path>                        Capture a mid-game board PNG and exit\n\
	   --scenario <name|path>                     Opening deck (default|same|complementary|...)\n\
	   --list-scenarios                           Print builtin deck scenarios\n\
	   --no-moves-loses <on|off>                  No ADD/MOVE loses (default: on)\n\
	   --center-hold-win <on|off>                 Center-hold win (default: off)\n\
	   --center-hold-n <1..64>                    Turns to hold center (default: 5)\n\
	   --no-same-piece-twice <off|always|when-bench-empty>\n\
	                                              Repeat-piece rule (default: when-bench-empty)\n\
	   --color-restricted-hops <on|off>           Hop only own color (default: on)\n\
	   --hop-weight-cmp <lt|le>                   Color-hop weight compare (default: le)\n\
	   --petal-majority-win <on|off>              Inner-ring majority win (default: off)\n\
	   --petal-majority-n <1..64>                 Turns to hold majority (default: 1)\n\
	   --control-win <on|off>                     Center+inner-ring control win (default: on)\n\
	   --control-n <1..9>                         Control-set occupancy needed (default: 5)\n\
	   --control-hold-turns <1..64>               Turns to hold control (default: 1)\n\
	   --insufficient-tiles-loss <off|strict|no-recapture>\n\
	                                              Can't-reach-control loss (default: off)\n\
	   --starting-bench <1..27>                   Opening deal from the 27-tile bag (default: 7)\n\
	   --max-rings <1..3>                         Distinct normal rings per tile (default: 3)\n\
	   --path-constraint <no-node-revisit|no-edge-revisit|both>\n\
	                                              MOVE path uniqueness (default: no-edge-revisit)\n\
	   --no-repeated-edge <on|off>                Alias for path-constraint edge/node\n\
	   -h, --help                                 Print help\n"
}

#[cfg(test)]
mod tests {
	#![allow(clippy::unwrap_used, clippy::expect_used)]

	use super::*;

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
		assert_eq!(cfg.rules.no_same_piece_twice, SamePieceTwice::WhenBenchEmpty);
		assert!(cfg.rules.color_restricted_hops);
		assert_eq!(cfg.rules.hop_weight_cmp, HopWeightCmp::Le);
		assert!(!cfg.rules.petal_majority_win);
		assert_eq!(cfg.rules.petal_majority_n, 1);
		assert!(cfg.rules.control_win);
		assert_eq!(cfg.rules.control_n, 5);
		assert_eq!(cfg.rules.control_hold_turns, 1);
		assert_eq!(cfg.rules.insufficient_tiles_loss, crate::game::InsufficientTilesLoss::Off);
		assert_eq!(cfg.rules.starting_bench_size, 7);
		assert_eq!(cfg.rules.max_rings, 3);
		assert_eq!(cfg.rules.path_constraint, PathConstraint::NoEdgeRevisit);
	}

	#[test]
	fn parses_max_rings() {
		let cfg = GameConfig::parse(["--max-rings", "1"]).unwrap();
		assert_eq!(cfg.rules.max_rings, 1);
		let err = GameConfig::parse(["--max-rings", "4"]).unwrap_err();
		assert!(err.contains("max-rings"));
	}

	#[test]
	fn parses_starting_bench() {
		let cfg = GameConfig::parse(["--starting-bench", "5"]).unwrap();
		assert_eq!(cfg.rules.starting_bench_size, 5);
		let err = GameConfig::parse(["--starting-bench", "28"]).unwrap_err();
		assert!(err.contains("starting-bench"));
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
