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
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Rules {
	Scored,
	Old,
}

impl Default for GameConfig {
	fn default() -> Self {
		Self { bot: None, human_seat: 1, board_size: 5, rules: Rules::Scored, think_ms: 500 }
	}
}

impl GameConfig {
	pub fn rule_set(&self) -> RuleSet {
		match (self.rules, self.board_size) {
			(Rules::Scored, 5) => RuleSet::scored(),
			(Rules::Scored, 7) => RuleSet::scored_7x7(),
			(Rules::Old, _) => RuleSet::old(),
			_ => RuleSet::scored(),
		}
	}

	pub fn think_time(&self) -> Duration {
		Duration::from_millis(self.think_ms)
	}

	#[cfg(not(target_os = "android"))]
	#[cfg(not(target_os = "ios"))]
	pub fn from_args() -> Self {
		let mut config = Self::default();
		let args: Vec<String> = std::env::args().collect();
		let mut i = 1;

		while i < args.len() {
			match args[i].as_str() {
				"--bot" => {
					if i + 1 < args.len() {
						config.bot = match args[i + 1].as_str() {
							"none" => None,
							"ore" => Some(BotType::Ore),
							"olusare" => Some(BotType::Olusare),
							"ologbon" => Some(BotType::Ologbon),
							"mcts" => Some(BotType::Mcts),
							_ => None,
						};
						i += 1;
					}
				}
				"--seat" => {
					if i + 1 < args.len() {
						if let Ok(seat) = args[i + 1].parse::<u8>() {
							if seat == 1 || seat == 2 {
								config.human_seat = seat;
							}
						}
						i += 1;
					}
				}
				"--size" => {
					if i + 1 < args.len() {
						if let Ok(size) = args[i + 1].parse::<u8>() {
							if size == 5 || size == 7 {
								config.board_size = size;
							}
						}
						i += 1;
					}
				}
				"--rules" => {
					if i + 1 < args.len() {
						config.rules = match args[i + 1].as_str() {
							"scored" => Rules::Scored,
							"old" => Rules::Old,
							_ => Rules::Scored,
						};
						i += 1;
					}
				}
				"--think-ms" => {
					if i + 1 < args.len() {
						if let Ok(ms) = args[i + 1].parse::<u64>() {
							config.think_ms = ms;
						}
						i += 1;
					}
				}
				_ => {}
			}
			i += 1;
		}

		config
	}

	#[cfg(any(target_os = "android", target_os = "ios"))]
	pub fn from_args() -> Self {
		Self::default()
	}
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
	}
}
