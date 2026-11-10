//! Fail-closed CLI parsing. Unknown arguments print usage and exit 1.

use std::env;
use std::path::PathBuf;
use std::process;
use std::time::Duration;

use crate::ai::Actor;
use crate::game::RuleSet;

#[derive(Clone, Debug)]
pub struct GameConfig {
	pub bot1: Actor,
	pub bot2: Actor,
	pub seed: u64,
	pub think: Duration,
	pub delay: Duration,
	pub screenshot: Option<PathBuf>,
	pub rules: RuleSet,
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
			rules: RuleSet::default(),
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
				"--actions" => {
					let n = parse_u64("--actions", &require_value("--actions", it.next())?)?;
					if n == 0 || n > 8 {
						return Err("invalid --actions (expected 1..8)".into());
					}
					cfg.rules.actions_per_turn = n as u8;
				}
				other => {
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

pub fn usage() -> &'static str {
	"Usage: apa-kan [OPTIONS]\n\
	 \n\
	 Options:\n\
	   --bot1 <human|random|greedy|minimax|mcts>  Player 1, light (default: human)\n\
	   --bot2 <human|random|greedy|minimax|mcts>  Player 2, dark (default: greedy)\n\
	   --seed <u64>                               Deal/shuffle seed (default: 1)\n\
	   --think-ms <n>                             Bot think time in ms (default: 80)\n\
	   --delay-ms <n>                             Delay between bot moves (default: 350)\n\
	   --actions <1..8>                           Actions per turn (default: 1)\n\
	   --screenshot <path>                        Write a board PNG and exit\n\
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
}
