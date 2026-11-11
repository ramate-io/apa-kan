//! Named and file-backed opening decks for the app and bench harness.

use std::fs;
use std::path::{Path, PathBuf};

use rand::rngs::StdRng;
use rand::seq::SliceRandom;
use rand::SeedableRng;
use serde::Deserialize;

use crate::game::{Tile, TileColor};

/// One (color, weight, dots) kind. `count` repeats in the bag.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct KindCount {
	pub color: TileColor,
	pub weight: u8,
	pub dots: u8,
	pub count: u8,
}

impl KindCount {
	pub fn tag(self) -> String {
		kind_tag(self.color, self.weight, self.dots)
	}
}

pub fn kind_tag(color: TileColor, weight: u8, dots: u8) -> String {
	format!("{}{}-{}", &color.name()[..1], weight, dots)
}

pub fn tile_tag(tile: Tile) -> String {
	kind_tag(tile.color, tile.weight, tile.dots)
}

/// Sum of weights, dots, and distinct colors — a simple deck-strength view.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DeckStrength {
	pub n_tiles: u32,
	pub weight_sum: u32,
	pub dots_sum: u32,
	pub n_colors: u32,
}

impl DeckStrength {
	pub fn of(tiles: &[Tile]) -> Self {
		let mut colors = [false; 3];
		let mut weight_sum = 0u32;
		let mut dots_sum = 0u32;
		for t in tiles {
			weight_sum += u32::from(t.weight);
			dots_sum += u32::from(t.dots);
			if (t.color as usize) < 3 {
				colors[t.color as usize] = true;
			}
		}
		Self {
			n_tiles: tiles.len() as u32,
			weight_sum,
			dots_sum,
			n_colors: colors.iter().filter(|c| **c).count() as u32,
		}
	}

	/// `weight + dots + color count`, matching the requested simple measure.
	pub fn combined(self) -> u32 {
		self.weight_sum + self.dots_sum + self.n_colors
	}

	pub fn label(self) -> String {
		format!("n{} w{} d{} c{}", self.n_tiles, self.weight_sum, self.dots_sum, self.n_colors)
	}
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DeckSpec {
	pub kinds: Vec<KindCount>,
	/// If set, these kinds are dealt first (removed from the bag). Remaining
	/// bag tiles follow in listed or shuffled order.
	pub bench: Option<Vec<(TileColor, u8, u8)>>,
}

impl DeckSpec {
	pub fn one_of_each_kind() -> Self {
		Self { kinds: all_kinds(TileColor::ALL), bench: None }
	}

	pub fn from_colors(colors: &[TileColor]) -> Self {
		Self { kinds: all_kinds(colors.iter().copied()), bench: None }
	}
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Scenario {
	pub name: String,
	pub description: String,
	pub shuffle: bool,
	/// Copy P1's kind sequence onto P2 (new tile ids / owner).
	pub mirror: bool,
	pub p1: DeckSpec,
	pub p2: DeckSpec,
	/// Complementary construction: exclusive P1 / exclusive P2 / shared color.
	/// When set, `p1`/`p2` kinds are ignored and both bags share one (role, w, d) shuffle.
	pub complementary: Option<Complementary>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Complementary {
	pub exclusive_p1: TileColor,
	pub exclusive_p2: TileColor,
	pub shared: TileColor,
}

impl Scenario {
	pub fn same() -> Self {
		must_parse("same", include_str!("../scenarios/same.toml"))
	}

	pub fn complementary() -> Self {
		must_parse("complementary", include_str!("../scenarios/complementary.toml"))
	}

	pub fn complementary_swapped() -> Self {
		must_parse("complementary-swapped", include_str!("../scenarios/complementary-swapped.toml"))
	}

	pub fn independent() -> Self {
		must_parse("independent", include_str!("../scenarios/independent.toml"))
	}

	pub fn builtin(name: &str) -> Option<Self> {
		match name {
			"default" | "same" => Some(Self::same()),
			"independent" => Some(Self::independent()),
			"complementary" => Some(Self::complementary()),
			"complementary-swapped" => Some(Self::complementary_swapped()),
			_ => None,
		}
	}

	pub fn builtin_names() -> &'static [&'static str] {
		&["default", "same", "independent", "complementary", "complementary-swapped"]
	}

	/// Name of a builtin, or a path to a `.toml` file.
	pub fn load(name_or_path: &str) -> Result<Self, String> {
		if let Some(s) = Self::builtin(name_or_path) {
			return Ok(s);
		}
		let path = resolve_path(name_or_path)?;
		let text = fs::read_to_string(&path)
			.map_err(|e| format!("cannot read scenario '{}': {e}", path.display()))?;
		parse_toml(&text)
	}

	pub fn list_text() -> String {
		let mut out = String::from("Builtin scenarios:\n");
		for name in Self::builtin_names() {
			if let Some(s) = Self::builtin(name) {
				out.push_str(&format!("  {name:24} {}\n", s.description));
			}
		}
		out.push_str("Pass a name or a path to a TOML file as --scenario.\n");
		out
	}
}

fn must_parse(name: &str, src: &str) -> Scenario {
	parse_toml(src).unwrap_or_else(|e| panic!("builtin scenario {name} is invalid: {e}"))
}

fn resolve_path(name_or_path: &str) -> Result<PathBuf, String> {
	let p = Path::new(name_or_path);
	if p.exists() {
		return Ok(p.to_path_buf());
	}
	let with_toml = if name_or_path.ends_with(".toml") {
		PathBuf::from(name_or_path)
	} else {
		PathBuf::from(format!("{name_or_path}.toml"))
	};
	if with_toml.exists() {
		return Ok(with_toml);
	}
	let crate_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("scenarios");
	let cand = crate_dir.join(&with_toml);
	if cand.exists() {
		return Ok(cand);
	}
	let cand = crate_dir.join(name_or_path);
	if cand.exists() {
		return Ok(cand);
	}
	Err(format!(
		"unknown scenario '{name_or_path}' (builtins: {}; or a .toml path)",
		Scenario::builtin_names().join(", ")
	))
}

fn all_kinds(colors: impl IntoIterator<Item = TileColor>) -> Vec<KindCount> {
	let mut out = Vec::new();
	for color in colors {
		for weight in 1..=3 {
			for dots in 1..=3 {
				out.push(KindCount { color, weight, dots, count: 1 });
			}
		}
	}
	out
}

#[derive(Deserialize)]
struct FileScenario {
	#[serde(default)]
	name: Option<String>,
	#[serde(default)]
	description: Option<String>,
	#[serde(default)]
	shuffle: Option<bool>,
	#[serde(default)]
	mirror: Option<bool>,
	#[serde(default)]
	exclusive_p1: Option<String>,
	#[serde(default)]
	exclusive_p2: Option<String>,
	#[serde(default)]
	shared: Option<String>,
	#[serde(default)]
	p1: Option<FileDeck>,
	#[serde(default)]
	p2: Option<FileDeck>,
}

#[derive(Deserialize)]
struct FileDeck {
	#[serde(default)]
	colors: Option<Vec<String>>,
	#[serde(default)]
	tiles: Option<Vec<FileTile>>,
	#[serde(default)]
	bench: Option<Vec<FileTile>>,
}

#[derive(Deserialize)]
struct FileTile {
	color: String,
	#[serde(default)]
	weight: Option<u8>,
	#[serde(default)]
	dots: Option<u8>,
	#[serde(default)]
	count: Option<u8>,
}

pub fn parse_toml(text: &str) -> Result<Scenario, String> {
	let file: FileScenario =
		toml::from_str(text).map_err(|e| format!("invalid scenario TOML: {e}"))?;
	let complementary = match (file.exclusive_p1, file.exclusive_p2, file.shared) {
		(None, None, None) => None,
		(Some(a), Some(b), Some(c)) => {
			let exclusive_p1 = TileColor::parse(&a)?;
			let exclusive_p2 = TileColor::parse(&b)?;
			let shared = TileColor::parse(&c)?;
			if exclusive_p1 == exclusive_p2 || exclusive_p1 == shared || exclusive_p2 == shared {
				return Err(
					"complementary exclusive_p1, exclusive_p2, and shared must be three colors"
						.into(),
				);
			}
			Some(Complementary { exclusive_p1, exclusive_p2, shared })
		}
		_ => {
			return Err(
				"complementary scenarios need exclusive_p1, exclusive_p2, and shared together"
					.into(),
			);
		}
	};
	let p1 = file
		.p1
		.as_ref()
		.map(deck_from_file)
		.transpose()?
		.unwrap_or_else(DeckSpec::one_of_each_kind);
	let p2 = file
		.p2
		.as_ref()
		.map(deck_from_file)
		.transpose()?
		.unwrap_or_else(DeckSpec::one_of_each_kind);
	Ok(Scenario {
		name: file.name.unwrap_or_else(|| "unnamed".into()),
		description: file.description.unwrap_or_default(),
		shuffle: file.shuffle.unwrap_or(true),
		mirror: file.mirror.unwrap_or(false),
		p1,
		p2,
		complementary,
	})
}

fn deck_from_file(d: &FileDeck) -> Result<DeckSpec, String> {
	let mut kinds = Vec::new();
	if let Some(colors) = &d.colors {
		let mut parsed = Vec::new();
		for c in colors {
			parsed.push(TileColor::parse(c)?);
		}
		kinds.extend(all_kinds(parsed));
	}
	if let Some(tiles) = &d.tiles {
		for t in tiles {
			kinds.extend(expand_tile(t)?);
		}
	}
	if kinds.is_empty() {
		kinds = all_kinds(TileColor::ALL);
	}
	let bench = match &d.bench {
		None => None,
		Some(b) if b.is_empty() => None,
		Some(b) => {
			let mut out = Vec::new();
			for t in b {
				if t.weight.is_none() || t.dots.is_none() {
					return Err("scenario bench entries need color, weight, and dots".into());
				}
				out.push((TileColor::parse(&t.color)?, t.weight.unwrap_or(1), t.dots.unwrap_or(1)));
			}
			Some(out)
		}
	};
	Ok(DeckSpec { kinds, bench })
}

fn expand_tile(t: &FileTile) -> Result<Vec<KindCount>, String> {
	let color = TileColor::parse(&t.color)?;
	let count = t.count.unwrap_or(1);
	if count == 0 {
		return Err("tile count must be >= 1".into());
	}
	match (t.weight, t.dots) {
		(None, None) => Ok(all_kinds(std::iter::once(color))
			.into_iter()
			.map(|mut k| {
				k.count = count;
				k
			})
			.collect()),
		(Some(weight), Some(dots)) => {
			if !(1..=3).contains(&weight) || !(1..=3).contains(&dots) {
				return Err("weight and dots must be 1..=3".into());
			}
			Ok(vec![KindCount { color, weight, dots, count }])
		}
		_ => {
			Err("tile entries need both weight and dots, or neither (all 9 kinds of that color)"
				.into())
		}
	}
}

/// Materialize both bags (draw piles, last entry drawn first) and optional
/// pre-dealt benches. `shuffle` uses `seed`.
pub fn materialize(
	scenario: &Scenario,
	seed: u64,
) -> Result<([Vec<(TileColor, u8, u8)>; 2], [Vec<(TileColor, u8, u8)>; 2]), String> {
	let mut rng = StdRng::seed_from_u64(seed);
	if let Some(c) = scenario.complementary {
		let mut slots = Vec::new();
		for role in [0u8, 1] {
			for weight in 1..=3u8 {
				for dots in 1..=3u8 {
					slots.push((role, weight, dots));
				}
			}
		}
		if scenario.shuffle {
			slots.shuffle(&mut rng);
		}
		let paint = |excl: TileColor| -> Vec<(TileColor, u8, u8)> {
			slots
				.iter()
				.map(|&(role, w, d)| {
					let color = if role == 0 { excl } else { c.shared };
					(color, w, d)
				})
				.collect()
		};
		let p1_bag = paint(c.exclusive_p1);
		let p2_bag = paint(c.exclusive_p2);
		return Ok(([p1_bag, p2_bag], [Vec::new(), Vec::new()]));
	}

	let mut p1_bag = expand_kinds(&scenario.p1.kinds);
	if scenario.shuffle {
		p1_bag.shuffle(&mut rng);
	}
	let mut p2_bag = if scenario.mirror {
		p1_bag.clone()
	} else {
		let mut b = expand_kinds(&scenario.p2.kinds);
		if scenario.shuffle {
			b.shuffle(&mut rng);
		}
		b
	};
	let p1_bench = take_fixed_bench(&mut p1_bag, scenario.p1.bench.as_deref())?;
	let p2_bench = if scenario.mirror {
		take_fixed_bench(&mut p2_bag, scenario.p1.bench.as_deref())?
	} else {
		take_fixed_bench(&mut p2_bag, scenario.p2.bench.as_deref())?
	};
	Ok(([p1_bag, p2_bag], [p1_bench, p2_bench]))
}

fn expand_kinds(kinds: &[KindCount]) -> Vec<(TileColor, u8, u8)> {
	let mut out = Vec::new();
	for k in kinds {
		for _ in 0..k.count.max(1) {
			out.push((k.color, k.weight, k.dots));
		}
	}
	out
}

fn take_fixed_bench(
	bag: &mut Vec<(TileColor, u8, u8)>,
	want: Option<&[(TileColor, u8, u8)]>,
) -> Result<Vec<(TileColor, u8, u8)>, String> {
	let Some(want) = want else {
		return Ok(Vec::new());
	};
	let mut bench = Vec::new();
	for &k in want {
		let pos = bag
			.iter()
			.position(|&x| x == k)
			.ok_or_else(|| format!("fixed bench {} is not in the bag", kind_tag(k.0, k.1, k.2)))?;
		bench.push(bag.remove(pos));
	}
	Ok(bench)
}

pub fn format_kind_list(tiles: &[Tile]) -> String {
	tiles.iter().map(|t| tile_tag(*t)).collect::<Vec<_>>().join(",")
}

pub fn unique_colors(tiles: &[Tile]) -> Vec<TileColor> {
	let mut out = Vec::new();
	for t in tiles {
		if !out.contains(&t.color) {
			out.push(t.color);
		}
	}
	out
}

#[cfg(test)]
mod tests {
	#![allow(clippy::unwrap_used, clippy::expect_used)]

	use super::*;
	use crate::game::{Game, Player, RuleSet};

	#[test]
	fn builtins_parse() {
		for name in Scenario::builtin_names() {
			let s = Scenario::builtin(name).expect(name);
			assert!(!s.name.is_empty() || name == &"default", "{name}");
		}
	}

	#[test]
	fn same_matches_default_new_game() {
		for seed in [1u64, 7, 42] {
			let a = Game::new(RuleSet::default(), seed);
			let b = Game::from_scenario(RuleSet::default(), seed, &Scenario::same()).unwrap();
			let kinds = |g: &Game, p: Player| {
				g.bench(p).iter().map(|t| (t.color, t.weight, t.dots)).collect::<Vec<_>>()
			};
			assert_eq!(kinds(&a, Player::One), kinds(&b, Player::One), "seed {seed}");
			assert_eq!(kinds(&a, Player::One), kinds(&a, Player::Two));
			assert_eq!(kinds(&b, Player::One), kinds(&b, Player::Two));
			assert_eq!(a.bag_len(Player::One), b.bag_len(Player::One));
		}
	}

	#[test]
	fn complementary_is_mirror_aside_from_exclusive_color() {
		let s = Scenario::complementary();
		let game = Game::from_scenario(RuleSet::default(), 3, &s).unwrap();
		assert_eq!(game.bench(Player::One).len(), 7);
		assert_eq!(game.bag_len(Player::One), 11);
		assert_eq!(game.bag_len(Player::Two), 11);
		let seq = |p: Player| {
			game.bench(p)
				.iter()
				.chain(game.bag(p).iter().rev())
				.map(|t| (t.weight, t.dots, t.color == TileColor::Orange))
				.collect::<Vec<_>>()
		};
		assert_eq!(seq(Player::One), seq(Player::Two), "weight/dot/shared-role sequence");
		let all = |p: Player| game.bench(p).iter().chain(game.bag(p)).copied().collect::<Vec<_>>();
		let p1c = unique_colors(&all(Player::One));
		let p2c = unique_colors(&all(Player::Two));
		assert!(p1c.iter().all(|c| matches!(c, TileColor::Green | TileColor::Orange)));
		assert!(p2c.iter().all(|c| matches!(c, TileColor::Blue | TileColor::Orange)));
		assert!(p1c.contains(&TileColor::Green));
		assert!(p2c.contains(&TileColor::Blue));
	}

	#[test]
	fn complementary_swapped_flips_exclusive_colors() {
		let a = Game::from_scenario(RuleSet::default(), 5, &Scenario::complementary()).unwrap();
		let b =
			Game::from_scenario(RuleSet::default(), 5, &Scenario::complementary_swapped()).unwrap();
		let excl = |g: &Game, p: Player| {
			g.bench(p)
				.iter()
				.chain(g.bag(p))
				.find(|t| t.color != TileColor::Orange)
				.map(|t| t.color)
		};
		assert_eq!(excl(&a, Player::One), Some(TileColor::Green));
		assert_eq!(excl(&a, Player::Two), Some(TileColor::Blue));
		assert_eq!(excl(&b, Player::One), Some(TileColor::Blue));
		assert_eq!(excl(&b, Player::Two), Some(TileColor::Green));
	}

	#[test]
	fn load_builtin_and_file() {
		let from_name = Scenario::load("complementary").unwrap();
		assert_eq!(from_name.name, "complementary");
		let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("scenarios/same.toml");
		let from_file = Scenario::load(path.to_str().unwrap()).unwrap();
		assert_eq!(from_file.name, "same");
		assert!(Scenario::load("no-such-scenario").is_err());
	}

	#[test]
	fn custom_toml_fixed_bench() {
		let toml = r#"
name = "fixed"
shuffle = false
mirror = true
[[p1.tiles]]
color = "green"
weight = 1
dots = 1
count = 1
[[p1.tiles]]
color = "green"
weight = 2
dots = 2
count = 1
[[p1.tiles]]
color = "green"
weight = 3
dots = 3
count = 1
[[p1.bench]]
color = "green"
weight = 3
dots = 3
"#;
		let s = parse_toml(toml).unwrap();
		let mut rules = RuleSet::default();
		rules.starting_bench_size = 1;
		let game = Game::from_scenario(rules, 1, &s).unwrap();
		assert_eq!(game.bench(Player::One).len(), 1);
		assert_eq!(game.bench(Player::One)[0].weight, 3);
		assert_eq!(game.bench(Player::One)[0].dots, 3);
		assert_eq!(game.bench(Player::Two)[0].weight, 3);
	}

	#[test]
	fn opening_strength_equal_on_same_deck() {
		let game = Game::from_scenario(RuleSet::default(), 9, &Scenario::same()).unwrap();
		assert_eq!(
			DeckStrength::of(game.bench(Player::One)),
			DeckStrength::of(game.bench(Player::Two))
		);
	}

	#[test]
	fn independent_can_differ() {
		let mut differed = false;
		for seed in 1u64..40 {
			let game =
				Game::from_scenario(RuleSet::default(), seed, &Scenario::independent()).unwrap();
			let a: Vec<_> =
				game.bench(Player::One).iter().map(|t| (t.color, t.weight, t.dots)).collect();
			let b: Vec<_> =
				game.bench(Player::Two).iter().map(|t| (t.color, t.weight, t.dots)).collect();
			if a != b {
				differed = true;
				break;
			}
		}
		assert!(differed, "independent shuffles should sometimes deal different benches");
	}
}
