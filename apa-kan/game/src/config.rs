//! Fail-closed CLI parsing. Unknown arguments print usage and exit 1.

use std::env;
use std::path::PathBuf;
use std::process;
use std::time::Duration;

use crate::ai::{Actor, SearchConfig};
use crate::deck::{DeckArchetype, PickStrategy, SetupConfig, DECK_SIZE_DEFAULT, DECK_SIZE_MAX};
use crate::game::{
	color_drift_diagonal, BeadScoring, BlockerBlocks, BraidFailMismatch, BraidScoring, CaptureMode,
	ChannelMode, EmblemWeights, EndgameRule, ExtraConvertMode, ExtraSwapMode, ExtraTurnAction,
	HopWeightCmp, InsufficientTilesLoss, PassThroughRingCmp, PathConstraint, RingCountWeights,
	RuleSet, SamePieceTwice, SwapMode, ZagMismatchCount, BAG_SIZE,
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
	pub search: SearchConfig,
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
			search: SearchConfig::default(),
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
		let mut same_deck = None;
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
				"--p1-deck" => {
					cfg.setup.p1_deck =
						DeckArchetype::parse(&require_value("--p1-deck", it.next())?)?;
				}
				"--p2-deck" => {
					cfg.setup.p2_deck =
						DeckArchetype::parse(&require_value("--p2-deck", it.next())?)?;
				}
				"--same-deck" => {
					same_deck =
						Some(parse_bool("--same-deck", &require_value("--same-deck", it.next())?)?);
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
				"--search-profile" => {
					cfg.search = SearchConfig::parse_profile(&require_value(
						"--search-profile",
						it.next(),
					)?)?;
				}
				"--bot-config" => {
					let path = PathBuf::from(require_value("--bot-config", it.next())?);
					cfg.search = SearchConfig::load_path(&path)?;
				}
				"--list-search-profiles" => {
					print!("{}", SearchConfig::list_text());
					process::exit(0);
				}
				"--search" => {
					cfg.search = cfg.search.with_spec(&require_value("--search", it.next())?)?;
				}
				"--search-threads" => {
					let n = parse_u64(
						"--search-threads",
						&require_value("--search-threads", it.next())?,
					)?;
					if n == 0 || n > 32 {
						return Err("invalid --search-threads (expected 1..32)".into());
					}
					cfg.search = cfg.search.with_threads(n as u8);
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
		if same_deck == Some(true) {
			cfg.rules.identical_bags = true;
			cfg.rules.deck_size = DECK_SIZE_DEFAULT as u8;
			cfg.setup.apply_same_deck();
		} else if same_deck == Some(false) {
			cfg.rules.identical_bags = false;
		}
		validate_rule_flags(&cfg.rules)?;
		if cfg.setup.deck_seed.is_none() {
			cfg.setup.deck_seed = Some(cfg.seed);
		}
		cfg.setup.p1_search = cfg.search;
		cfg.setup.p2_search = cfg.search;
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

fn parse_i64(flag: &str, value: &str) -> Result<i64, String> {
	value.parse::<i64>().map_err(|_| format!("invalid {flag} '{value}'"))
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
		"--endgame-rule" => {
			rules.endgame_rule = EndgameRule::parse(&require_value(arg, it.next())?)?;
		}
		"--cascade-turn-gain" => {
			rules.cascade_turn_gain = parse_bool(arg, &require_value(arg, it.next())?)?;
		}
		"--cascade-winner-swap" => {
			rules.cascade_winner_swap = parse_bool(arg, &require_value(arg, it.next())?)?;
		}
		"--extra-turn-action" => {
			rules.extra_turn_action = ExtraTurnAction::parse(&require_value(arg, it.next())?)?;
		}
		"--extra-convert-mode" => {
			rules.extra_convert_mode = ExtraConvertMode::parse(&require_value(arg, it.next())?)?;
		}
		"--extra-swap-mode" => {
			rules.extra_swap_mode = ExtraSwapMode::parse(&require_value(arg, it.next())?)?;
		}
		"--center-mismatch" => {
			rules.center_mismatch = parse_bool(arg, &require_value(arg, it.next())?)?;
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
		"--bead-scoring" => {
			rules.bead_scoring = BeadScoring::parse(&require_value(arg, it.next())?)?;
		}
		"--channel-mode" => {
			rules.channel_mode = ChannelMode::parse(&require_value(arg, it.next())?)?;
		}
		"--independent-channels" => {
			let on = parse_bool(arg, &require_value(arg, it.next())?)?;
			rules.channel_mode =
				if on { ChannelMode::IndependentMax } else { ChannelMode::Additive };
		}
		"--ring-count-weights" => {
			rules.ring_count_weights = RingCountWeights::parse(&require_value(arg, it.next())?)?;
		}
		"--color-drift" => {
			let n = parse_i64(arg, &require_value(arg, it.next())?)?;
			if !(-10_000..=10_000).contains(&n) {
				return Err(
					"invalid --color-drift (expected -10000..10000, RFC ColorPass.drift diagonal)"
						.into(),
				);
			}
			rules.color_drift = color_drift_diagonal(n as i32);
		}
		"--identical-bags" => {
			rules.identical_bags = parse_bool(arg, &require_value(arg, it.next())?)?;
		}
		"--capture-mode" => {
			rules.capture_mode = CaptureMode::parse(&require_value(arg, it.next())?)?;
		}
		"--singles-swap" | "--swap-mode" => {
			rules.swap_mode = SwapMode::parse_cli(&require_value(arg, it.next())?)?;
		}
		"--dot-chance" => {
			let n = parse_u64(arg, &require_value(arg, it.next())?)?;
			if n > 100 {
				return Err("invalid --dot-chance (expected 0..100 percent)".into());
			}
			rules.dot_percent = n as u8;
		}
		"--emblem-weights" => {
			rules.emblem_weights = EmblemWeights::parse(&require_value(arg, it.next())?)?;
		}
		"--emblems" => {
			let on = parse_bool(arg, &require_value(arg, it.next())?)?;
			rules.emblem_weights = if on { EmblemWeights::default() } else { EmblemWeights::off() };
		}
		"--blocker-blocks" => {
			rules.blocker_blocks = BlockerBlocks::parse(&require_value(arg, it.next())?)?;
		}
		"--zag-mismatch" => {
			rules.zag_mismatch = ZagMismatchCount::parse(&require_value(arg, it.next())?)?;
		}
		"--braid-fail-mismatch" => {
			rules.braid_fail_mismatch = BraidFailMismatch::parse(&require_value(arg, it.next())?)?;
		}
		"--braid-scoring" => {
			rules.braid_scoring = BraidScoring::parse(&require_value(arg, it.next())?)?;
		}
		"--single-braids" => {
			rules.allow_single_braids = parse_bool(arg, &require_value(arg, it.next())?)?;
		}
		"--score-unique-neighbors" | "--link-once" => {
			rules.score_unique_neighbors = parse_bool(arg, &require_value(arg, it.next())?)?;
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
	   --search-profile <enhanced|classic|plain|tactical|positional|explorative|deep_mcts|counterplay|exactplay>\n\
	                                              Search preset (default: enhanced; aliases: aggressive, cautious, explore, deep, focused, contest, playtest)\n\
	   --bot-config <path.toml>                   Load a SearchConfig TOML (overlays missing keys with enhanced defaults)\n\
	   --list-search-profiles                     Print named presets and their labels\n\
	   --search <k=v,...>                         Overlay any knob: ordering, tt, lmr, pvs, q-depth, pw, puct,\n\
	                                              priors, pw-order, reuse, mcts-tt, factored, leaf, solver, threads,\n\
	                                              eval-margin, ...\n\
	   --search-threads <1..32>                   MCTS root-parallel trees (default: 1; 1 keeps benches deterministic)\n\
	   --seed <u64>                               Deal/shuffle seed (default: 1)\n\
	   --deck-seed <u64>                          Collection generation seed (default: --seed)\n\
	   --deck-noise <0..100>                      Resample that percent of a mirrored collection (default: 0)\n\
	   --pick <heuristic|search|greedy>           Bench 7+4 picker (default: heuristic)\n\
	   --p1-deck <mixed|braid|bead|crescent|zs|emblem|blocker|rosette>  P1 collection archetype (default: mixed)\n\
	   --p2-deck <mixed|braid|bead|crescent|zs|emblem|blocker|rosette>  P2 collection archetype (default: mixed)\n\
	   --same-deck <on|off>                       Identical 27-tile start for both players (default: on)\n\
	   --identical-bags <on|off>                  Mirror collections when archetypes match (default: on)\n\
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
	   --no-moves-loses <on|off>                  No ADD/MOVE/SWAP/CLAIM loses (default: on)\n\
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
	   --control-n <1..9>                         Control-set occupancy cap (default: 5)\n\
	   --control-hold-turns <1..64>               Turns to hold control (default: 1)\n\
	   --endgame-rule <control-cascade|low-value>  One-way ratchet 5→2 with max material, then low-value at ≤1 (default: control-cascade)\n\
	   --cascade-turn-gain <on|off>               Extra turn for the material leader on a non-winning cascade drop (default: on)\n\
	   --cascade-winner-swap <on|off>             Extra turn when a pre-action center occupant is captured (default: off)\n\
	   --extra-turn-action <convert|swap>         Extra-turn CONVERT (ownership flip, default) or permissive position-exchange SWAP\n\
	   --extra-convert-mode <any|not-center|inner> CONVERT target filter (default: any opponent piece, including center)\n\
	   --extra-swap-mode <any|own|not-center>     Extra-turn SWAP partner filter when --extra-turn-action swap\n\
	   --center-mismatch <on|off>                 Unmatched center occupant scores +1 mismatch (default: off)\n\
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
	   --bead-chance <0..7>                       Bead light-election weight (default: 1; 0 disables)\n\
	   --bead-scoring <independent|shared>        Additive-mode normal/bead combine (default independent = beads separate; shared = max fold)\n\
	   --braid-scoring <current|braid-only-match|braid-only-exact>\n\
	                                              Braid match total (default: braid-only-exact = only exact braid matches score, 1-slot +1, other matches still paint; current = coverage; braid-only-match = legacy coverage-only)\n\
	   --single-braids <on|off>                   Lone heavies (incl. 1-ring) become 1-slot braids (default: on; same heavy coin, no extra roll)\n\
	   --score-unique-neighbors <on|off>          Legacy unique-neighbor scoring (default: off; alias --link-once). Live scoring is per-arc double-count\n\
	   --link-once <on|off>                       Alias for --score-unique-neighbors\n\
	   --channel-mode <independent-max|additive>  Edge combine only (default: additive). Generation is always clustering\n\
	   --independent-channels <on|off>            Alias: on = independent-max, off = additive scoring\n\
	   --ring-count-weights <s,d,t>               Singles:doubles:triples generation weights (default: 4,3,1)\n\
	   --color-drift <n>                          RFC ColorPass.drift diagonal (default 160; 0 restores uniqueness)\n\
	   --capture-mode <local-claim|cascade>       Local ADD/MOVE/SWAP captures + Claim (default: local-claim; cascade = PR #70)\n\
	   --singles-swap <on|off|normal-only>        Adjacent singles-swap (default: normal-only = 1-ring normals only; on = any 1-ring; off disables; alias --swap-mode)\n\
	   --dot-chance <0..100>                      Generated tiles that are dots (rest triangles; default: 50)\n\
	   --emblem-weights <none,crescent,zs[,blocker,rosette]>\n\
	                                              Emblem generation weights (default: 200,1,1,1,1)\n\
	   --emblems <on|off>                         Alias for default weights or 1,0,0,0,0 (skip RNG)\n\
	   --blocker-blocks <traversal|pass|hop|off>  What Blocker stops as a MOVE intermediate (default: traversal)\n\
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
		assert_eq!(cfg.search, SearchConfig::enhanced());
	}

	#[test]
	fn parses_search_profile_and_spec() {
		let cfg = GameConfig::parse([
			"--search-profile",
			"classic",
			"--search",
			"lmr=on,pvs=on",
			"--search-threads",
			"4",
		])
		.unwrap();
		assert!(cfg.search.transposition);
		assert!(cfg.search.lmr && cfg.search.pvs);
		assert_eq!(cfg.search.mcts_root_threads, 4);
		// classic + LMR + PVS is the enhanced preset (PW stays off). Threads
		// are ignored when naming the profile.
		assert_eq!(cfg.search.profile_name(), "enhanced");
		let custom =
			GameConfig::parse(["--search-profile", "classic", "--search", "pw=on"]).unwrap();
		assert_eq!(custom.search.profile_name(), "custom");
	}

	#[test]
	fn parses_personality_profiles() {
		let cfg = GameConfig::parse(["--search-profile", "tactical"]).unwrap();
		assert_eq!(cfg.search, SearchConfig::tactical());
		let cfg = GameConfig::parse(["--search-profile", "aggressive"]).unwrap();
		assert_eq!(cfg.search.profile_name(), "tactical");
		let cfg = GameConfig::parse(["--search-profile", "positional"]).unwrap();
		assert_eq!(cfg.search, SearchConfig::positional());
		let cfg = GameConfig::parse(["--search-profile", "explorative"]).unwrap();
		assert!(cfg.search.mcts_progressive_widening);
		assert_eq!(cfg.search.profile_name(), "explorative");
		let cfg = GameConfig::parse(["--search-profile", "deep"]).unwrap();
		assert_eq!(cfg.search, SearchConfig::deep_mcts());
		assert_eq!(cfg.search.profile_name(), "deep_mcts");
		let cfg = GameConfig::parse(["--search-profile", "counterplay"]).unwrap();
		assert_eq!(cfg.search, SearchConfig::counterplay());
		assert_eq!(cfg.search.profile_name(), "counterplay");
		let cfg = GameConfig::parse(["--search-profile", "exactplay"]).unwrap();
		assert_eq!(cfg.search, SearchConfig::exactplay());
		assert_eq!(cfg.search.profile_name(), "exactplay");
		assert_eq!(cfg.setup.p1_search, cfg.search);
		assert_eq!(cfg.setup.p2_search, cfg.search);
	}

	#[test]
	fn parses_bot_config_toml() {
		let path = format!("{}/bots/tactical.toml", env!("CARGO_MANIFEST_DIR"));
		let cfg = GameConfig::parse(["--bot-config", &path]).unwrap();
		assert_eq!(cfg.search, SearchConfig::tactical());
		assert_eq!(cfg.search.profile_name(), "tactical");
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
	fn parses_endgame_rule() {
		let cascade = GameConfig::parse(["--endgame-rule", "control-cascade"]).unwrap();
		assert_eq!(cascade.rules.endgame_rule, crate::game::EndgameRule::ControlCascade);
		let baseline = GameConfig::parse(["--endgame-rule", "low-value"]).unwrap();
		assert_eq!(baseline.rules.endgame_rule, crate::game::EndgameRule::LowValue);
		assert!(GameConfig::parse(["--endgame-rule", "sudden-death"]).is_err());
	}

	#[test]
	fn parses_cascade_turn_gain() {
		let on = GameConfig::parse(["--cascade-turn-gain", "on"]).unwrap();
		assert!(on.rules.cascade_turn_gain);
		let off = GameConfig::parse(["--cascade-turn-gain", "off"]).unwrap();
		assert!(!off.rules.cascade_turn_gain);
		assert!(GameConfig::parse(["--cascade-turn-gain", "maybe"]).is_err());
	}

	#[test]
	fn parses_cascade_winner_swap_and_center_mismatch() {
		let def = GameConfig::parse([] as [&str; 0]).unwrap();
		assert!(!def.rules.cascade_winner_swap);
		assert_eq!(def.rules.extra_turn_action, crate::game::ExtraTurnAction::Convert);
		assert_eq!(def.rules.extra_convert_mode, crate::game::ExtraConvertMode::Any);
		assert_eq!(def.rules.extra_swap_mode, crate::game::ExtraSwapMode::Any);
		assert!(!def.rules.center_mismatch);
		let on = GameConfig::parse(["--cascade-winner-swap", "on"]).unwrap();
		assert!(on.rules.cascade_winner_swap);
		assert_eq!(on.rules.extra_turn_action, crate::game::ExtraTurnAction::Convert);
		let off = GameConfig::parse(["--cascade-winner-swap", "off"]).unwrap();
		assert!(!off.rules.cascade_winner_swap);
		let swap = GameConfig::parse(["--extra-turn-action", "swap"]).unwrap();
		assert_eq!(swap.rules.extra_turn_action, crate::game::ExtraTurnAction::Swap);
		let inner = GameConfig::parse(["--extra-convert-mode", "inner"]).unwrap();
		assert_eq!(inner.rules.extra_convert_mode, crate::game::ExtraConvertMode::Inner);
		let nc_c = GameConfig::parse(["--extra-convert-mode", "not-center"]).unwrap();
		assert_eq!(nc_c.rules.extra_convert_mode, crate::game::ExtraConvertMode::NotCenter);
		let own = GameConfig::parse(["--extra-swap-mode", "own"]).unwrap();
		assert_eq!(own.rules.extra_swap_mode, crate::game::ExtraSwapMode::Own);
		let nc = GameConfig::parse(["--extra-swap-mode", "not-center"]).unwrap();
		assert_eq!(nc.rules.extra_swap_mode, crate::game::ExtraSwapMode::NotCenter);
		let mm = GameConfig::parse(["--center-mismatch", "on"]).unwrap();
		assert!(mm.rules.center_mismatch);
		assert!(GameConfig::parse(["--cascade-winner-swap", "maybe"]).is_err());
		assert!(GameConfig::parse(["--extra-turn-action", "teleport"]).is_err());
		assert!(GameConfig::parse(["--extra-convert-mode", "outer"]).is_err());
		assert!(GameConfig::parse(["--extra-swap-mode", "adjacent"]).is_err());
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
		assert_eq!(cfg.rules.endgame_rule, crate::game::EndgameRule::ControlCascade);
		assert!(cfg.rules.cascade_turn_gain);
		assert!(!cfg.rules.cascade_winner_swap);
		assert_eq!(cfg.rules.extra_turn_action, crate::game::ExtraTurnAction::Convert);
		assert_eq!(cfg.rules.extra_convert_mode, crate::game::ExtraConvertMode::Any);
		assert_eq!(cfg.rules.extra_swap_mode, crate::game::ExtraSwapMode::Any);
		assert!(!cfg.rules.center_mismatch);
		assert_eq!(cfg.rules.insufficient_tiles_loss, crate::game::InsufficientTilesLoss::Off);
		assert_eq!(cfg.rules.starting_bench_size, 7);
		assert_eq!(cfg.rules.bag_size, crate::game::BAG_SIZE_DEFAULT as u8);
		assert_eq!(cfg.rules.deck_size, DECK_SIZE_DEFAULT as u8);
		assert_eq!(cfg.setup.pick, PickStrategy::Heuristic);
		assert_eq!(cfg.setup.deck_noise, 0);
		assert!(cfg.rules.self_capture_budget);
		assert_eq!(cfg.rules.max_rings, 3);
		assert_eq!(cfg.rules.zag_numer, 1);
		assert_eq!(cfg.rules.ring_count_weights, crate::game::LIVE_RING_COUNT_WEIGHTS);
		assert_eq!(
			cfg.rules.color_drift,
			crate::game::color_drift_diagonal(crate::game::COLOR_DRIFT_DEFAULT)
		);
		assert_eq!(cfg.rules.braid_numer, 1);
		assert_eq!(cfg.rules.bead_numer, 1);
		assert_eq!(cfg.rules.bead_scoring, crate::game::BeadScoring::Independent);
		assert_eq!(cfg.rules.braid_scoring, crate::game::BraidScoring::BraidOnlyExact);
		assert!(cfg.rules.allow_single_braids);
		assert!(!cfg.rules.score_unique_neighbors);
		assert_eq!(cfg.rules.channel_mode, crate::game::ChannelMode::Additive);
		assert_eq!(cfg.rules.capture_mode, crate::game::CaptureMode::LocalClaim);
		assert_eq!(cfg.setup.p1_deck, crate::deck::DeckArchetype::Mixed);
		assert_eq!(cfg.setup.p2_deck, crate::deck::DeckArchetype::Mixed);
		assert_eq!(cfg.rules.dot_percent, crate::game::DOT_PERCENT_DEFAULT);
		assert_eq!(cfg.rules.emblem_weights, crate::game::EmblemWeights::default());
		assert_eq!(cfg.rules.path_constraint, PathConstraint::NoEdgeRevisit);
		assert!(cfg.rules.counted_pass_through);
		assert_eq!(cfg.rules.pass_through_ring_cmp, PassThroughRingCmp::Ge);
		assert_eq!(cfg.rules.zag_mismatch, crate::game::ZagMismatchCount::Sum);
		assert_eq!(cfg.rules.braid_fail_mismatch, crate::game::BraidFailMismatch::One);
		assert_eq!(cfg.rules.swap_mode, crate::game::SwapMode::NormalOnly);
		assert!(!cfg.rules.cascade_winner_swap);
		assert!(!cfg.rules.center_mismatch);
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
	fn parses_braid_scoring_and_link_once() {
		let def = GameConfig::parse([] as [&str; 0]).unwrap();
		assert_eq!(def.rules.braid_scoring, crate::game::BraidScoring::BraidOnlyExact);
		assert!(!def.rules.score_unique_neighbors);
		assert!(def.rules.allow_single_braids);
		let only = GameConfig::parse(["--braid-scoring", "braid-only-match"]).unwrap();
		assert_eq!(only.rules.braid_scoring, crate::game::BraidScoring::BraidOnlyMatch);
		let exact = GameConfig::parse(["--braid-scoring", "braid-only-exact"]).unwrap();
		assert_eq!(exact.rules.braid_scoring, crate::game::BraidScoring::BraidOnlyExact);
		let current = GameConfig::parse(["--braid-scoring", "current"]).unwrap();
		assert_eq!(current.rules.braid_scoring, crate::game::BraidScoring::Current);
		let gone = GameConfig::parse(["--braid-exact", "on"]).unwrap_err();
		assert!(gone.contains("unknown argument") || gone.contains("--braid-exact"), "{gone}");
		let err = GameConfig::parse(["--braid-scoring", "product"]).unwrap_err();
		assert!(err.contains("braid-scoring"));
		let singles = GameConfig::parse(["--single-braids", "off"]).unwrap();
		assert!(!singles.rules.allow_single_braids);
		let once = GameConfig::parse(["--link-once", "on"]).unwrap();
		assert!(once.rules.score_unique_neighbors);
		let unique = GameConfig::parse(["--score-unique-neighbors", "on"]).unwrap();
		assert!(unique.rules.score_unique_neighbors);
		let off = GameConfig::parse(["--link-once", "off"]).unwrap();
		assert!(!off.rules.score_unique_neighbors);
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
		let chance = GameConfig::parse(["--bead-chance", "0"]).unwrap();
		assert_eq!(chance.rules.bead_numer, 0);
		let three = GameConfig::parse(["--bead-chance", "3"]).unwrap();
		assert_eq!(three.rules.bead_numer, 3);
		let err = GameConfig::parse(["--bead-chance", "8"]).unwrap_err();
		assert!(err.contains("bead-chance"));
		let gone = GameConfig::parse(["--beads", "off"]).unwrap_err();
		assert!(gone.contains("--beads") || gone.contains("unknown"), "{gone}");
	}

	#[test]
	fn parses_bead_scoring_and_deck_archetypes() {
		let shared = GameConfig::parse(["--bead-scoring", "shared"]).unwrap();
		assert_eq!(shared.rules.bead_scoring, crate::game::BeadScoring::SharedChannel);
		let ind = GameConfig::parse(["--bead-scoring", "independent"]).unwrap();
		assert_eq!(ind.rules.bead_scoring, crate::game::BeadScoring::Independent);
		let def = GameConfig::parse([] as [&str; 0]).unwrap();
		assert_eq!(def.rules.bead_scoring, crate::game::BeadScoring::Independent);
		let err = GameConfig::parse(["--bead-scoring", "average"]).unwrap_err();
		assert!(err.contains("bead-scoring"));
		let decks = GameConfig::parse(["--p1-deck", "braid", "--p2-deck", "bead"]).unwrap();
		assert_eq!(decks.setup.p1_deck, crate::deck::DeckArchetype::BraidHeavy);
		assert_eq!(decks.setup.p2_deck, crate::deck::DeckArchetype::BeadHeavy);
	}

	#[test]
	fn parses_channel_mode() {
		let def = GameConfig::parse([] as [&str; 0]).unwrap();
		assert_eq!(def.rules.channel_mode, crate::game::ChannelMode::Additive);
		let max = GameConfig::parse(["--channel-mode", "independent-max"]).unwrap();
		assert_eq!(max.rules.channel_mode, crate::game::ChannelMode::IndependentMax);
		let add = GameConfig::parse(["--channel-mode", "additive"]).unwrap();
		assert_eq!(add.rules.channel_mode, crate::game::ChannelMode::Additive);
		let legacy = GameConfig::parse(["--channel-mode", "legacy"]).unwrap();
		assert_eq!(legacy.rules.channel_mode, crate::game::ChannelMode::Additive);
		let err = GameConfig::parse(["--channel-mode", "average"]).unwrap_err();
		assert!(err.contains("channel-mode"));
		let on = GameConfig::parse(["--independent-channels", "on"]).unwrap();
		assert_eq!(on.rules.channel_mode, crate::game::ChannelMode::IndependentMax);
		let off = GameConfig::parse(["--independent-channels", "off"]).unwrap();
		assert_eq!(off.rules.channel_mode, crate::game::ChannelMode::Additive);
	}

	#[test]
	fn parses_same_deck() {
		let on = GameConfig::parse(["--same-deck", "on"]).unwrap();
		assert!(on.rules.identical_bags);
		assert_eq!(on.rules.deck_size as usize, DECK_SIZE_DEFAULT);
		assert_eq!(on.setup.deck_noise, 0);
		assert_eq!(on.setup.p1_deck, crate::deck::DeckArchetype::Mixed);
		assert_eq!(on.setup.p2_deck, crate::deck::DeckArchetype::Mixed);
		let off = GameConfig::parse(["--same-deck", "off"]).unwrap();
		assert!(!off.rules.identical_bags);
		let bags = GameConfig::parse(["--identical-bags", "off"]).unwrap();
		assert!(!bags.rules.identical_bags);
	}

	#[test]
	fn parses_ring_count_weights() {
		let def = GameConfig::parse([] as [&str; 0]).unwrap();
		assert_eq!(def.rules.ring_count_weights, crate::game::LIVE_RING_COUNT_WEIGHTS);
		let custom = GameConfig::parse(["--ring-count-weights", "1,2,3"]).unwrap();
		assert_eq!(custom.rules.ring_count_weights.singles, 1);
		assert_eq!(custom.rules.ring_count_weights.doubles, 2);
		assert_eq!(custom.rules.ring_count_weights.triples, 3);
		let err = GameConfig::parse(["--ring-count-weights", "0,0,0"]).unwrap_err();
		assert!(err.contains("ring-count-weights"));
	}

	#[test]
	fn parses_color_drift_as_rfc_diagonal() {
		let def = GameConfig::parse([] as [&str; 0]).unwrap();
		assert_eq!(
			def.rules.color_drift,
			crate::game::color_drift_diagonal(crate::game::COLOR_DRIFT_DEFAULT)
		);
		let forty = GameConfig::parse(["--color-drift", "40"]).unwrap();
		assert_eq!(forty.rules.color_drift, crate::game::color_drift_diagonal(40));
		assert_eq!(crate::game::color_drift_diagonal_value(forty.rules.color_drift), Some(40));
		let err = GameConfig::parse(["--color-drift", "nope"]).unwrap_err();
		assert!(err.contains("color-drift"));
	}

	#[test]
	fn parses_capture_mode() {
		let def = GameConfig::parse([] as [&str; 0]).unwrap();
		assert_eq!(def.rules.capture_mode, crate::game::CaptureMode::LocalClaim);
		let local = GameConfig::parse(["--capture-mode", "local-claim"]).unwrap();
		assert_eq!(local.rules.capture_mode, crate::game::CaptureMode::LocalClaim);
		let cascade = GameConfig::parse(["--capture-mode", "cascade"]).unwrap();
		assert_eq!(cascade.rules.capture_mode, crate::game::CaptureMode::Cascade);
		let err = GameConfig::parse(["--capture-mode", "explode"]).unwrap_err();
		assert!(err.contains("capture-mode"));
	}

	#[test]
	fn parses_singles_swap() {
		let def = GameConfig::parse([] as [&str; 0]).unwrap();
		assert_eq!(def.rules.swap_mode, crate::game::SwapMode::NormalOnly);
		let off = GameConfig::parse(["--singles-swap", "off"]).unwrap();
		assert_eq!(off.rules.swap_mode, crate::game::SwapMode::Off);
		let on = GameConfig::parse(["--swap-mode", "on"]).unwrap();
		assert_eq!(on.rules.swap_mode, crate::game::SwapMode::On);
		let adj = GameConfig::parse(["--singles-swap", "adjacent-any"]).unwrap();
		assert_eq!(adj.rules.swap_mode, crate::game::SwapMode::On);
		let normal = GameConfig::parse(["--singles-swap", "normal-only"]).unwrap();
		assert_eq!(normal.rules.swap_mode, crate::game::SwapMode::NormalOnly);
		let err = GameConfig::parse(["--singles-swap", "any"]).unwrap_err();
		assert!(err.contains("singles-swap"));
		let gone = GameConfig::parse(["--swap-no-immediate-back", "on"]).unwrap_err();
		assert!(gone.contains("unknown argument"));
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
	fn parses_emblem_weights() {
		let cfg = GameConfig::parse(["--emblem-weights", "1,20,0"]).unwrap();
		assert_eq!(cfg.rules.emblem_weights.none, 1);
		assert_eq!(cfg.rules.emblem_weights.crescent, 20);
		assert_eq!(cfg.rules.emblem_weights.zs, 0);
		assert_eq!(cfg.rules.emblem_weights.blocker, 0);
		assert_eq!(cfg.rules.emblem_weights.rosette, 0);
		let five = GameConfig::parse(["--emblem-weights", "200,1,1,1,1"]).unwrap();
		assert_eq!(five.rules.emblem_weights, crate::game::EmblemWeights::default());
		let off = GameConfig::parse(["--emblems", "off"]).unwrap();
		assert_eq!(off.rules.emblem_weights, crate::game::EmblemWeights::off());
		let on = GameConfig::parse(["--emblems", "on"]).unwrap();
		assert_eq!(on.rules.emblem_weights, crate::game::EmblemWeights::default());
		let err = GameConfig::parse(["--emblem-weights", "0,0,0"]).unwrap_err();
		assert!(err.contains("emblem-weights"));
		let err = GameConfig::parse(["--emblem-weights", "0,0,0,0,0"]).unwrap_err();
		assert!(err.contains("emblem-weights"));
		let decks = GameConfig::parse(["--p1-deck", "blocker", "--p2-deck", "rosette"]).unwrap();
		assert_eq!(decks.setup.p1_deck, crate::deck::DeckArchetype::Blocker);
		assert_eq!(decks.setup.p2_deck, crate::deck::DeckArchetype::Rosette);
		let blocks = GameConfig::parse(["--blocker-blocks", "off"]).unwrap();
		assert_eq!(blocks.rules.blocker_blocks, crate::game::BlockerBlocks::Off);
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
