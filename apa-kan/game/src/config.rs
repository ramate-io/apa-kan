use crate::game::{BotType, RuleSet};
use bevy::prelude::*;
use std::time::Duration;

#[derive(Debug, Clone, Resource)]
pub struct GameConfig {
	pub bot: Option<BotType>,
	pub human_seat: u8,
	pub board_size: u8,
	pub rules: Rules,
	pub think_ms: u64,
	pub actions: u8,
	pub opening_handicap: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Rules {
	Scored,
	Old,
}

impl Default for GameConfig {
	fn default() -> Self {
		Self {
			bot: None,
			human_seat: 1,
			board_size: 5,
			rules: Rules::Scored,
			think_ms: 500,
			actions: 1,
			opening_handicap: true,
		}
	}
}

impl GameConfig {
	pub fn rule_set(&self) -> RuleSet {
		let mut rs = match (self.rules, self.board_size, self.actions) {
			(Rules::Scored, 5, 2) => RuleSet::scored_double(),
			(Rules::Scored, 7, 2) => RuleSet::scored_7x7_double(),
			(Rules::Old, _, 2) => RuleSet::old_double(),
			(Rules::Scored, 5, _) => RuleSet::scored(),
			(Rules::Scored, 7, _) => RuleSet::scored_7x7(),
			(Rules::Old, _, _) => RuleSet::old(),
			_ => RuleSet::scored(),
		};
		if rs.actions_per_turn > 1 {
			rs.opening_handicap = self.opening_handicap;
		}
		rs
	}

	pub fn think_time(&self) -> Duration {
		Duration::from_millis(self.think_ms)
	}

	pub fn usage() -> &'static str {
		"Usage: apa-kan [OPTIONS]\n\n\
Options:\n\
  --bot <none|ore|olusare|ologbon|mcts>  AI opponent (default: none)\n\
  --seat <1|2>                           Human seat (default: 1)\n\
  --size <5|7>                           Board size (default: 5)\n\
  --rules <scored|old>                   Rule set (default: scored)\n\
  --actions <1|2>                        Actions per turn (default: 1)\n\
  --no-opening-handicap                  Two-action: P1's first turn also gets 2 actions\n\
  --think-ms <n>                         Bot think time in ms (default: 500)\n\
  -h, --help                             Print help"
	}

	pub fn parse_args<I, S>(args: I) -> Result<Self, String>
	where
		I: IntoIterator<Item = S>,
		S: AsRef<str>,
	{
		let mut argv: Vec<String> = args.into_iter().map(|s| s.as_ref().to_string()).collect();
		if argv.first().is_some_and(|first| !first.starts_with('-')) {
			argv.remove(0);
		}

		let mut config = Self::default();
		let mut i = 0;
		while i < argv.len() {
			let arg = argv[i].as_str();
			match arg {
				"-h" | "--help" => {
					i += 1;
				}
				"--no-opening-handicap" => {
					config.opening_handicap = false;
					i += 1;
				}
				"--bot" | "--seat" | "--size" | "--rules" | "--think-ms" | "--actions" => {
					let flag = arg.to_string();
					let value = argv
						.get(i + 1)
						.filter(|v| !v.starts_with('-'))
						.ok_or_else(|| format!("missing value for {flag}"))?;
					match flag.as_str() {
						"--bot" => {
							config.bot = parse_bot(value)?;
						}
						"--seat" => {
							config.human_seat = parse_choice(value, "--seat", &[1, 2])?;
						}
						"--size" => {
							config.board_size = parse_choice(value, "--size", &[5, 7])?;
						}
						"--rules" => {
							config.rules = parse_rules(value)?;
						}
						"--think-ms" => {
							config.think_ms =
								value.parse::<u64>().map_err(|_| {
									format!("invalid value '{value}' for --think-ms, expected an integer")
								})?;
						}
						"--actions" => {
							config.actions = parse_choice(value, "--actions", &[1, 2])?;
						}
						_ => unreachable!(),
					}
					i += 2;
				}
				other if other.starts_with('-') => {
					return Err(format!("unknown argument '{other}'"));
				}
				other => {
					return Err(format!("unexpected argument '{other}'"));
				}
			}
		}
		Ok(config)
	}

	#[cfg(not(any(target_os = "android", target_os = "ios")))]
	pub fn from_args() -> Self {
		let args: Vec<String> = std::env::args().collect();
		if args.iter().any(|a| a == "--help" || a == "-h") {
			println!("{}", Self::usage());
			std::process::exit(0);
		}
		match Self::parse_args(&args) {
			Ok(config) => config,
			Err(err) => {
				eprintln!("error: {err}");
				eprintln!();
				eprintln!("{}", Self::usage());
				std::process::exit(1);
			}
		}
	}

	#[cfg(any(target_os = "android", target_os = "ios"))]
	pub fn from_args() -> Self {
		Self::default()
	}
}

fn parse_bot(value: &str) -> Result<Option<BotType>, String> {
	match value {
		"none" => Ok(None),
		"ore" => Ok(Some(BotType::Ore)),
		"olusare" => Ok(Some(BotType::Olusare)),
		"ologbon" => Ok(Some(BotType::Ologbon)),
		"mcts" => Ok(Some(BotType::Mcts)),
		_ => Err(format!(
			"invalid value '{value}' for --bot, expected none|ore|olusare|ologbon|mcts"
		)),
	}
}

fn parse_rules(value: &str) -> Result<Rules, String> {
	match value {
		"scored" => Ok(Rules::Scored),
		"old" => Ok(Rules::Old),
		_ => Err(format!("invalid value '{value}' for --rules, expected scored|old")),
	}
}

fn parse_choice<T: Copy + PartialEq + std::fmt::Display + std::str::FromStr>(
	value: &str,
	flag: &str,
	allowed: &[T],
) -> Result<T, String> {
	if let Ok(parsed) = value.parse::<T>() {
		if allowed.contains(&parsed) {
			return Ok(parsed);
		}
	}
	let expected = allowed.iter().map(|v| v.to_string()).collect::<Vec<_>>().join("|");
	Err(format!("invalid value '{value}' for {flag}, expected {expected}"))
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_default_config() {
		let config = GameConfig::default();
		assert_eq!(config.bot, None);
		assert_eq!(config.human_seat, 1);
		assert_eq!(config.board_size, 5);
		assert_eq!(config.rules, Rules::Scored);
		assert_eq!(config.think_ms, 500);
		assert_eq!(config.actions, 1);
		assert!(config.opening_handicap);
	}

	#[test]
	fn test_rule_set_mapping() {
		let mut config = GameConfig::default();

		config.rules = Rules::Scored;
		config.board_size = 5;
		let rs = config.rule_set();
		assert_eq!(rs.board_size, 5);
		assert_eq!(rs.scored_mode, true);
		assert_eq!(rs.add_bank_size, 5);
		assert_eq!(rs.actions_per_turn, 1);

		config.board_size = 7;
		let rs = config.rule_set();
		assert_eq!(rs.board_size, 7);
		assert_eq!(rs.scored_mode, true);
		assert_eq!(rs.add_bank_size, 5);

		config.rules = Rules::Old;
		let rs = config.rule_set();
		assert_eq!(rs.board_size, 5);
		assert_eq!(rs.scored_mode, false);
		assert_eq!(rs.add_bank_size, 100);
		assert_eq!(rs.actions_per_turn, 1);

		config.rules = Rules::Scored;
		config.board_size = 5;
		config.actions = 2;
		let rs = config.rule_set();
		assert_eq!(rs.actions_per_turn, 2);
		assert!(rs.opening_handicap);
		assert_eq!(rs.scored_mode, true);

		config.opening_handicap = false;
		let rs = config.rule_set();
		assert!(!rs.opening_handicap);
	}

	#[test]
	fn test_parse_valid_args() {
		let config = GameConfig::parse_args([
			"apa-kan",
			"--bot",
			"ologbon",
			"--seat",
			"1",
			"--size",
			"5",
			"--rules",
			"scored",
			"--think-ms",
			"500",
		])
		.expect("valid args");
		assert_eq!(config.bot, Some(BotType::Ologbon));
		assert_eq!(config.human_seat, 1);
		assert_eq!(config.board_size, 5);
		assert_eq!(config.rules, Rules::Scored);
		assert_eq!(config.think_ms, 500);
	}

	#[test]
	fn test_parse_unknown_glued_rules_arg() {
		let err = GameConfig::parse_args(["apa-kan", "--bot", "ologbon", "--rulesscored"])
			.expect_err("glued --rulesscored must fail");
		assert!(err.contains("--rulesscored"), "{err}");
		assert!(err.contains("unknown argument"), "{err}");
	}

	#[test]
	fn test_parse_invalid_bot() {
		let err = GameConfig::parse_args(["apa-kan", "--bot", "nope"]).unwrap_err();
		assert!(err.contains("--bot"), "{err}");
	}

	#[test]
	fn test_parse_missing_rules_value() {
		let err = GameConfig::parse_args(["apa-kan", "--rules"]).unwrap_err();
		assert!(err.contains("missing value for --rules"), "{err}");
	}

	#[test]
	fn test_parse_two_action_flags() {
		let config = GameConfig::parse_args([
			"apa-kan",
			"--bot",
			"ologbon",
			"--actions",
			"2",
			"--no-opening-handicap",
		])
		.expect("valid two-action args");
		assert_eq!(config.actions, 2);
		assert!(!config.opening_handicap);
		let rs = config.rule_set();
		assert_eq!(rs.actions_per_turn, 2);
		assert!(!rs.opening_handicap);
	}
}
