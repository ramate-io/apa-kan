//! Versioned core tile generation: rings, colors, markings, and emblems.
//!
//! [`CorePass::base`] is the published rarity chart from
//! `rfc/rfc-000-000-00n-generation`. Event rewards and collectors are other
//! values of the same struct. Each draw is a weighted [`TileId::choose`]
//! (or a coin) seeded from `id.mix(step)` so the same id always replays.

use std::hash::{Hash, Hasher};

use crate::game::{
	CaptureMark, Emblem, EmblemWeights, RingType, RuleSet, Tile, TileColor, MAX_COLOR_RINGS,
	ZAG_NUMER_DEFAULT,
};

/// Domain-separated generation seed. Stored as [`Tile::id`] so a deal replays.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct TileId(pub u32);

impl TileId {
	pub fn new(id: u32) -> Self {
		Self(id)
	}

	/// Domain-separated child of `self`. `step` is typically a tag (`"n"`)
	/// or a `(tag, slot)` pair (`("h", i)`).
	pub fn mix(self, step: impl Hash) -> Self {
		let mut hasher = MixHasher::new(u64::from(self.0));
		step.hash(&mut hasher);
		Self(hasher.finish32())
	}

	/// Weighted draw. Zero and negative weights floor to 0; at least one
	/// must stay positive (otherwise the first item is returned).
	pub fn choose<T: Copy>(self, from: &[T], weights: &[i32]) -> T {
		debug_assert_eq!(from.len(), weights.len());
		debug_assert!(!from.is_empty());
		let mut total = 0u32;
		for &w in weights {
			total = total.saturating_add(w.max(0) as u32);
		}
		if total == 0 {
			return from[0];
		}
		let mut roll = self.0 % total;
		for (item, &w) in from.iter().zip(weights.iter()) {
			let w = w.max(0) as u32;
			if roll < w {
				return *item;
			}
			roll -= w;
		}
		from[from.len() - 1]
	}

	/// `choose` on `[false, true]` with `[light, heavy]`-style weights.
	pub fn sample(self, weights: &[u32]) -> bool {
		let light = weights.first().copied().unwrap_or(0) as i32;
		let heavy = weights.get(1).copied().unwrap_or(0) as i32;
		Self::choose(self, &[false, true], &[light, heavy])
	}

	/// Splitmix-style mix used for fallback picks (`hash(id)` in the RFC).
	pub fn avalanche(self) -> u32 {
		MixHasher::new(u64::from(self.0)).finish32()
	}
}

/// Deterministic hasher. Not a cryptographic hash; used only to domain-split
/// [`TileId::mix`] steps. Length-prefixed writes match `std::hash::Hash`.
struct MixHasher {
	state: u64,
}

impl MixHasher {
	const GOLDEN: u64 = 0x9E37_79B9_7F4A_7C15;

	fn new(seed: u64) -> Self {
		Self { state: seed ^ Self::GOLDEN }
	}

	fn finish32(&self) -> u32 {
		let mut x = self.state;
		x ^= x >> 30;
		x = x.wrapping_mul(0xBF58_476D_1CE4_E5B9);
		x ^= x >> 27;
		x = x.wrapping_mul(0x94D0_49BB_1331_11EB);
		x ^= x >> 31;
		x as u32
	}
}

impl Hasher for MixHasher {
	fn write(&mut self, bytes: &[u8]) {
		self.write_u64(bytes.len() as u64);
		for &b in bytes {
			self.state = self.state.wrapping_add(u64::from(b)).wrapping_mul(Self::GOLDEN);
		}
	}

	fn write_u8(&mut self, i: u8) {
		self.state = self.state.wrapping_add(u64::from(i)).wrapping_mul(Self::GOLDEN);
	}

	fn write_u32(&mut self, i: u32) {
		self.state = self.state.wrapping_add(u64::from(i)).wrapping_mul(Self::GOLDEN);
	}

	fn write_u64(&mut self, i: u64) {
		self.state = self.state.wrapping_add(i).wrapping_mul(Self::GOLDEN);
	}

	fn finish(&self) -> u64 {
		u64::from(self.finish32())
	}
}

/// One versioned generator. Event rewards and collectors are other values.
#[derive(Clone, Debug, PartialEq)]
pub struct CorePass {
	pub version: u16,
	pub rings: RingPass,
	pub colors: ColorPass,
	pub markings: MarkingPass,
	pub emblems: EmblemPass,
}

/// Finished core face, before the character pass.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CoreTile {
	/// Slot order inner→outer.
	pub ring_type: Vec<RingType>,
	/// Parallel to [`Self::ring_type`].
	pub rings: Vec<TileColor>,
	/// MOVE length 1..=3.
	pub dots: u8,
	pub capture: CaptureMark,
	pub emblem: Emblem,
}

impl CoreTile {
	pub fn n_rings(&self) -> usize {
		self.rings.len().min(self.ring_type.len())
	}

	/// Convert to a live [`Tile`]. Character is cosmetic and filled in by
	/// the character pass.
	pub fn into_tile(
		self,
		id: u32,
		owner: crate::game::Player,
		character: Option<crate::characters::CharacterId>,
	) -> Tile {
		let n = self.n_rings().clamp(1, MAX_COLOR_RINGS as usize);
		let first = self.rings.first().copied().unwrap_or(TileColor::Green);
		let mut rings = [first; 3];
		let mut ring_type = [RingType::Normal; 3];
		for i in 0..n {
			rings[i] = self.rings[i];
			ring_type[i] = self.ring_type[i];
		}
		let n_rings = n as u8;
		Tile {
			id,
			owner,
			rings,
			n_rings,
			ring_type,
			weight: n_rings,
			dots: self.dots.clamp(1, 3),
			capture: self.capture,
			emblem: self.emblem,
			character,
		}
	}

	pub fn from_tile(tile: &Tile) -> Self {
		let n = tile.n_rings.clamp(1, MAX_COLOR_RINGS) as usize;
		Self {
			ring_type: tile.ring_type[..n].to_vec(),
			rings: tile.rings[..n].to_vec(),
			dots: tile.dots.clamp(1, 3),
			capture: tile.capture,
			emblem: tile.emblem,
		}
	}
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RingPass {
	/// Weights for n_rings = 1, 2, 3.
	pub count_weights: [u32; 3],
	/// Per slot: [light, heavy].
	pub heavy_weights: [u32; 2],
	/// Consecutive heavies (≥2 adjacent slots): [braid, bead].
	/// Gapped heavies (slots 0 and 2 only) are always bead.
	pub consecutive_weights: [u32; 2],
	/// Leftover slots, including a lone heavy: [normal, zag].
	pub light_weights: [u32; 2],
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ColorPass {
	/// Green, Blue, Orange, LavenderPink, Teal, Maroon.
	pub weights: [u32; 6],
	/// After drawing color i, add `drift[i]` to the running weights.
	/// Base is zero: independent draws.
	pub drift: [[i32; 6]; 6],
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MarkingPass {
	/// Weights for dots = 1, 2, 3 (MOVE length).
	pub count_weights: [u32; 3],
	/// [triangle, dot].
	pub capture_weights: [u32; 2],
	/// Per `RingType` (Normal, Zag, Braid, Bead): row = min(count, 2),
	/// col = dots 1/2/3.
	pub depreciations: [[[i32; 3]; 3]; 4],
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EmblemPass {
	/// None, CrescentMoon, TwoZagZs, Blocker, Rosette.
	pub weights: [u32; 5],
	/// Per ring type, row = min(count, 2), col = emblem outcome.
	pub ring_depreciations: [[[i32; 5]; 3]; 4],
	/// Per capture (Triangle, Dot), row = dots-1, col = emblem outcome.
	pub marking_depreciations: [[[i32; 5]; 3]; 2],
}

impl RingPass {
	pub fn base() -> Self {
		Self {
			count_weights: [5, 3, 1],
			heavy_weights: [3, 1],
			consecutive_weights: [3, 1],
			light_weights: [5, 1],
		}
	}

	fn consecutive(idx: &[usize]) -> bool {
		idx.windows(2).all(|w| w[1] == w[0] + 1)
	}

	pub fn run(id: TileId, cfg: &RingPass) -> Vec<RingType> {
		let n = TileId::choose(id.mix("n"), &[1u8, 2, 3], &cfg.count_weights.map(|x| x as i32))
			as usize;
		let n = n.clamp(1, MAX_COLOR_RINGS as usize);
		let heavy: Vec<usize> = (0..n)
			.filter(|&i| TileId::sample(id.mix(("h", i as u32)), &cfg.heavy_weights))
			.collect();

		let mut types = vec![RingType::Normal; n];

		if heavy.len() >= 2 {
			let group = if RingPass::consecutive(&heavy) {
				TileId::choose(
					id.mix("grp"),
					&[RingType::Braid, RingType::Bead],
					&cfg.consecutive_weights.map(|x| x as i32),
				)
			} else {
				RingType::Bead
			};
			for &i in &heavy {
				types[i] = group;
			}
		}

		// 0–1 heavies: every slot is light (a lone heavy does not make a
		// 1-bead / 1-braid). Generated tiles still never have a single bead
		// or a 1-slot braid.
		for i in 0..n {
			let in_group = heavy.len() >= 2 && heavy.contains(&i);
			if !in_group {
				types[i] = TileId::choose(
					id.mix(("lite", i as u32)),
					&[RingType::Normal, RingType::Zag],
					&cfg.light_weights.map(|x| x as i32),
				);
			}
		}
		types
	}
}

impl ColorPass {
	pub const COLORS: [TileColor; 6] = [
		TileColor::Green,
		TileColor::Blue,
		TileColor::Orange,
		TileColor::LavenderPink,
		TileColor::Teal,
		TileColor::Maroon,
	];

	pub fn base() -> Self {
		Self { weights: [81, 81, 81, 9, 9, 3], drift: [[0; 6]; 6] }
	}

	pub fn run(id: TileId, cfg: &ColorPass, n: usize) -> Vec<TileColor> {
		let mut w: [i32; 6] = cfg.weights.map(|x| x as i32);
		let mut out = Vec::with_capacity(n);
		for i in 0..n {
			let color = TileId::choose(id.mix(("c", i as u32)), &ColorPass::COLORS, &w);
			out.push(color);
			for (j, d) in cfg.drift[color as usize].iter().enumerate() {
				w[j] = w[j].saturating_add(*d);
			}
		}
		out
	}
}

impl MarkingPass {
	pub fn base() -> Self {
		let tax = [[0, 0, 0], [0, -1, -2], [0, -2, -4]];
		Self { count_weights: [17, 11, 7], capture_weights: [1, 1], depreciations: [tax; 4] }
	}

	pub fn run(id: TileId, cfg: &MarkingPass, ring_type: &[RingType]) -> (u8, CaptureMark) {
		let mut w: [i32; 3] = cfg.count_weights.map(|x| x as i32);
		for typ in [RingType::Normal, RingType::Zag, RingType::Braid, RingType::Bead] {
			let n = ring_type.iter().filter(|t| **t == typ).count().min(2);
			for (j, d) in cfg.depreciations[typ as usize][n].iter().enumerate() {
				w[j] = w[j].saturating_add(*d);
			}
		}
		let dots = TileId::choose(id.mix("dots"), &[1u8, 2, 3], &w);
		let capture = TileId::choose(
			id.mix("cap"),
			&[CaptureMark::Triangle, CaptureMark::Dot],
			&cfg.capture_weights.map(|x| x as i32),
		);
		(dots, capture)
	}
}

impl EmblemPass {
	pub const EMBLEMS: [Emblem; 5] =
		[Emblem::None, Emblem::CrescentMoon, Emblem::TwoZagZs, Emblem::Blocker, Emblem::Rosette];

	pub fn base() -> Self {
		let ring_tax = [[0, 0, 0, 0, 0], [0, -1, -1, -1, -1], [0, -2, -2, -2, -2]];
		let mark_tax = ring_tax;
		Self {
			weights: [197, 7, 7, 7, 7],
			ring_depreciations: [ring_tax; 4],
			marking_depreciations: [mark_tax; 2],
		}
	}

	pub fn run(
		id: TileId,
		cfg: &EmblemPass,
		ring_type: &[RingType],
		dots: u8,
		capture: CaptureMark,
	) -> Emblem {
		let mut w: [i32; 5] = cfg.weights.map(|x| x as i32);
		for typ in [RingType::Normal, RingType::Zag, RingType::Braid, RingType::Bead] {
			let n = ring_type.iter().filter(|t| **t == typ).count().min(2);
			for (j, d) in cfg.ring_depreciations[typ as usize][n].iter().enumerate() {
				w[j] = w[j].saturating_add(*d);
			}
		}
		let cap_i = capture as usize;
		let dot_i = (dots.clamp(1, 3) - 1) as usize;
		for (j, d) in cfg.marking_depreciations[cap_i][dot_i].iter().enumerate() {
			w[j] = w[j].saturating_add(*d);
		}
		TileId::choose(id.mix("emb"), &EmblemPass::EMBLEMS, &w)
	}
}

impl CorePass {
	/// Published rarity chart. Quote odds against this, not event skew.
	pub fn base() -> Self {
		Self {
			version: 1,
			rings: RingPass::base(),
			colors: ColorPass::base(),
			markings: MarkingPass::base(),
			emblems: EmblemPass::base(),
		}
	}

	/// Example collector: more consecutive heavies resolve as braid.
	pub fn braid_collector() -> Self {
		let mut cfg = Self::base();
		cfg.rings.heavy_weights = [1, 1];
		cfg.rings.consecutive_weights = [9, 1];
		cfg
	}

	/// Bead-heavy collector: consecutive heavies resolve as bead.
	pub fn bead_collector() -> Self {
		let mut cfg = Self::base();
		cfg.rings.heavy_weights = [1, 1];
		cfg.rings.consecutive_weights = [1, 9];
		cfg
	}

	/// Crescent Moon collector.
	pub fn crescent_collector() -> Self {
		let mut cfg = Self::base();
		cfg.emblems.weights = [1, 20, 0, 0, 0];
		cfg
	}

	/// Two Zag Zs collector.
	pub fn zs_collector() -> Self {
		let mut cfg = Self::base();
		cfg.emblems.weights = [1, 0, 20, 0, 0];
		cfg
	}

	/// Mixed crescent + zs, with a bit more heavy grouping.
	pub fn emblem_mix_collector() -> Self {
		let mut cfg = Self::base();
		cfg.emblems.weights = [1, 20, 20, 0, 0];
		cfg.rings.heavy_weights = [2, 1];
		cfg
	}

	/// Blocker collector.
	pub fn blocker_collector() -> Self {
		let mut cfg = Self::base();
		cfg.emblems.weights = [1, 0, 0, 20, 0];
		cfg
	}

	/// Rosette collector.
	pub fn rosette_collector() -> Self {
		let mut cfg = Self::base();
		cfg.emblems.weights = [1, 0, 0, 0, 20];
		cfg
	}

	/// Overlay live CLI knobs (`--zags off`, `--emblem-weights`, `--dot-chance`,
	/// `--max-rings`, collector numerators) onto [`Self::base`]. Emblem weights
	/// of the default RuleSet (200/1/1/1/1) replace the RFC rarity-chart
	/// 197/7/7/7/7 so mixed deals keep the historical glyph rate.
	pub fn from_rules(rules: &RuleSet) -> Self {
		Self::from_sampling(
			rules.max_rings,
			rules.zag_numer,
			rules.braid_numer,
			rules.bead_numer,
			rules.dot_percent,
			rules.emblem_weights,
		)
	}

	pub fn from_sampling(
		max_rings: u8,
		zag_numer: u8,
		braid_numer: u8,
		bead_numer: u8,
		dot_percent: u8,
		emblem_weights: EmblemWeights,
	) -> Self {
		let mut cfg = Self::base();
		match max_rings.clamp(1, MAX_COLOR_RINGS) {
			1 => cfg.rings.count_weights = [1, 0, 0],
			2 => cfg.rings.count_weights = [5, 3, 0],
			_ => {}
		}
		if zag_numer == 0 {
			cfg.rings.light_weights = [1, 0];
		} else if zag_numer != ZAG_NUMER_DEFAULT {
			let z = u32::from(zag_numer.min(7));
			cfg.rings.light_weights = [7u32.saturating_sub(z), z];
		}
		// Collector numerators (5/7 in deck.rs) raise the heavy coin and
		// skew consecutive heavies toward braid or bead. `0` disables that
		// heavy family.
		const COLLECTOR_NUMER: u8 = 5;
		if braid_numer == 0 && bead_numer == 0 {
			cfg.rings.heavy_weights = [1, 0];
		} else if braid_numer >= COLLECTOR_NUMER && bead_numer == 0 {
			cfg.rings.heavy_weights = [1, 1];
			cfg.rings.consecutive_weights = [1, 0];
		} else if bead_numer >= COLLECTOR_NUMER && braid_numer == 0 {
			cfg.rings.heavy_weights = [1, 1];
			cfg.rings.consecutive_weights = [0, 1];
		} else if braid_numer == 0 {
			cfg.rings.consecutive_weights = [0, 1];
		} else if bead_numer == 0 {
			cfg.rings.consecutive_weights = [1, 0];
		} else if braid_numer >= 2 && bead_numer >= 2 {
			cfg.rings.heavy_weights = [2, 1];
		}
		let dp = u32::from(dot_percent.min(100));
		cfg.markings.capture_weights = if dp == 0 {
			[1, 0]
		} else if dp >= 100 {
			[0, 1]
		} else {
			[100 - dp, dp]
		};
		cfg.emblems.weights = [
			u32::from(emblem_weights.none),
			u32::from(emblem_weights.crescent),
			u32::from(emblem_weights.zs),
			u32::from(emblem_weights.blocker),
			u32::from(emblem_weights.rosette),
		];
		if !emblem_weights.abilities_on() {
			cfg.emblems.weights = [1, 0, 0, 0, 0];
		}
		// Glyph taxes in `base()` assume RFC weights 197/7/7/7/7. Live CLI
		// weights (None 200 / each glyph 1) would floor every glyph to 0 after
		// a single ring tax, so the overlay uses the weights as written.
		cfg.emblems.ring_depreciations = [[[0; 5]; 3]; 4];
		cfg.emblems.marking_depreciations = [[[0; 5]; 3]; 2];
		cfg
	}

	pub fn run(&self, id: TileId) -> CoreTile {
		let ring_type = RingPass::run(id, &self.rings);
		let rings = ColorPass::run(id, &self.colors, ring_type.len());
		let (dots, capture) = MarkingPass::run(id, &self.markings, &ring_type);
		let emblem = EmblemPass::run(id, &self.emblems, &ring_type, dots, capture);
		CoreTile { ring_type, rings, dots, capture, emblem }
	}
}

#[cfg(test)]
mod tests {
	#![allow(clippy::unwrap_used, clippy::expect_used)]

	use super::*;
	use crate::game::Player;

	fn replay_ok(id: u32) {
		let a = CorePass::base().run(TileId(id));
		let b = CorePass::base().run(TileId(id));
		assert_eq!(a, b, "id {id} must replay");
	}

	#[test]
	fn base_replays_from_id() {
		for id in [0u32, 1, 7, 42, 255, 1_000_003, u32::MAX] {
			replay_ok(id);
		}
	}

	#[test]
	fn never_emits_a_single_bead_or_one_slot_braid() {
		let cfg = CorePass::base();
		for id in 0..8_000u32 {
			let t = cfg.run(TileId(id));
			let beads = t.ring_type.iter().filter(|r| r.is_bead()).count();
			let braids = t.ring_type.iter().filter(|r| r.is_braid()).count();
			assert_ne!(beads, 1, "id {id} had a single bead: {:?}", t.ring_type);
			assert_ne!(braids, 1, "id {id} had a 1-slot braid: {:?}", t.ring_type);
			assert!(t.rings.len() >= 1 && t.rings.len() <= 3);
			assert_eq!(t.rings.len(), t.ring_type.len());
			assert!((1..=3).contains(&t.dots));
		}
	}

	#[test]
	fn gapped_heavies_are_always_bead() {
		// Force 3 rings, always-heavy, so slots 0+2 (if those two land) are
		// gapped. We inspect the algorithm by using a pass that always draws
		// 3 rings and always marks heavy: consecutive groups may braid, but a
		// 2-heavy on 0 and 2 must be bead.
		let mut cfg = CorePass::base();
		cfg.rings.count_weights = [0, 0, 1];
		cfg.rings.heavy_weights = [0, 1];
		cfg.rings.consecutive_weights = [1, 0]; // prefer braid when consecutive
		for id in 0..4_000u32 {
			let t = cfg.run(TileId(id));
			assert_eq!(t.ring_type.len(), 3);
			let heavy: Vec<usize> = t
				.ring_type
				.iter()
				.enumerate()
				.filter(|(_, r)| r.is_braid() || r.is_bead())
				.map(|(i, _)| i)
				.collect();
			if heavy == [0, 2] {
				assert!(t.ring_type[0].is_bead() && t.ring_type[2].is_bead(), "{:?}", t.ring_type);
			}
			if heavy.len() >= 2 && RingPass::consecutive(&heavy) {
				assert!(
					t.ring_type[heavy[0]].is_braid(),
					"consecutive heavies should braid under this collector: {:?}",
					t.ring_type
				);
			}
		}
	}

	#[test]
	fn choose_floors_non_positive_weights() {
		let id = TileId(3);
		let got = id.choose(&[10i32, 20, 30], &[0, -4, 5]);
		assert_eq!(got, 30);
		let all_zero = id.choose(&[1u8, 2, 3], &[0, 0, 0]);
		assert_eq!(all_zero, 1);
	}

	#[test]
	fn mix_is_domain_separated() {
		let id = TileId(99);
		assert_ne!(id.mix("n"), id.mix("emb"));
		assert_ne!(id.mix(("h", 0u32)), id.mix(("h", 1u32)));
		assert_eq!(id.mix("n"), id.mix("n"));
	}

	#[test]
	fn into_tile_keeps_core_fields() {
		let core = CorePass::base().run(TileId(12345));
		let tile = core.clone().into_tile(12345, Player::One, None);
		assert_eq!(tile.id, 12345);
		assert_eq!(tile.n_rings as usize, core.n_rings());
		assert_eq!(tile.dots, core.dots);
		assert_eq!(tile.capture, core.capture);
		assert_eq!(tile.emblem, core.emblem);
		assert_eq!(tile.weight, tile.n_rings);
		assert!(tile.character.is_none());
		assert_eq!(CoreTile::from_tile(&tile), core);
	}

	#[test]
	fn zags_off_emits_no_zags() {
		let cfg = CorePass::from_sampling(3, 0, 1, 1, 50, EmblemWeights::default());
		for id in 0..2_000u32 {
			let t = cfg.run(TileId(id));
			assert!(t.ring_type.iter().all(|r| !r.is_zag()), "{:?}", t.ring_type);
		}
	}

	#[test]
	fn beads_and_braids_off_emits_only_light_slots() {
		let cfg = CorePass::from_sampling(3, 1, 0, 0, 50, EmblemWeights::default());
		for id in 0..2_000u32 {
			let t = cfg.run(TileId(id));
			assert!(t.ring_type.iter().all(|r| r.is_normal() || r.is_zag()), "{:?}", t.ring_type);
		}
	}

	#[test]
	fn emblems_off_is_always_none() {
		let cfg = CorePass::from_sampling(3, 1, 1, 1, 50, EmblemWeights::off());
		for id in 0..1_000u32 {
			assert_eq!(cfg.run(TileId(id)).emblem, Emblem::None);
		}
	}

	#[test]
	fn braid_collector_emits_multi_slot_braids() {
		let cfg = CorePass::braid_collector();
		let mut braids = 0u32;
		for id in 0..3_000u32 {
			let t = cfg.run(TileId(id));
			if t.ring_type.iter().filter(|r| r.is_braid()).count() >= 2 {
				braids += 1;
			}
		}
		assert!(braids > 80, "braid collector should land multi-braids, got {braids}");
	}
}
