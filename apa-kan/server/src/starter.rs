//! Starter kit granted on account create (Decision #58-17).
//!
//! Q14 stand-in until a central rarity table exists: 15 fairly common tiles
//! in green / blue / orange, no maroon (or other rare colors), mixed capture
//! marks, under the locked live engine types (`RingType`, `CaptureMark`,
//! `Tile::kind_index`).
//!
//! Tile kinds come from `apa-kan-core` (`Tile::kind_index` plus capture mark).

use apa_kan_core::character_pass::{assign_character, bundled};
use apa_kan_core::core_pass::CoreTile;
use apa_kan_core::game::{CaptureMark, Player, RingType, Tile, TileColor, KIND_COUNT};

use apa_kan_core::character_pass::CoreKey;
use apa_kan_store::{
	core_tile_id, tile_type_id_from_core, CoreTileRecord, TileGrant, TileKindId, TileTypeRecord,
};

/// Decision #58-17: each new account receives 15 starter tiles.
pub const STARTER_TILE_COUNT: usize = 15;

/// Encode a live engine tile as an inventory kind id.
///
/// `id = kind_index + KIND_COUNT * capture` with triangle = 0 and dot = 1.
/// Emblems are not granted in the starter kit and are not part of this encoding
/// yet; `apa-kan-core` should own the stable identity later.
pub fn inventory_kind_id(tile: Tile) -> TileKindId {
	let capture = if tile.capture.is_dot() { 1 } else { 0 };
	(tile.kind_index() + KIND_COUNT * capture) as TileKindId
}

/// Inventory rows granted on account create, with core/type/character ids.
pub fn starter_grants() -> Vec<TileGrant> {
	starter_kit().0
}

pub fn starter_kit() -> (Vec<TileGrant>, Vec<CoreTileRecord>, Vec<TileTypeRecord>) {
	let pass = bundled();
	let mut grants = Vec::new();
	let mut cores = Vec::new();
	let mut types = Vec::new();
	for tile in starter_tiles() {
		let core = CoreTile::from_tile(&tile);
		let character_id = pass
			.map(|p| assign_character(p.tables(), &core, tile.id))
			.unwrap_or_else(|| "faceless".into());
		let core_id = core_tile_id(&core);
		let type_id = tile_type_id_from_core(&core);
		let rings: Vec<(String, String)> = core
			.rings
			.iter()
			.zip(core.ring_type.iter())
			.map(|(c, t)| (c.name().to_string(), t.name().to_string()))
			.collect();
		let key = CoreKey::from_core(&core);
		let type_rings: Vec<(String, String)> = key
			.rings()
			.iter()
			.map(|(c, t)| (c.name().to_string(), t.name().to_string()))
			.collect();
		cores.push(CoreTileRecord {
			core_tile_id: core_id.clone(),
			rings: rings.clone(),
			dots: core.dots,
			capture: core.capture.name().into(),
			emblem: core.emblem.name().into(),
		});
		types.push(TileTypeRecord {
			tile_type_id: type_id.clone(),
			rings: type_rings,
			dots: key.dots(),
			capture: key.capture().name().into(),
			emblem: key.emblem().name().into(),
		});
		grants.push(TileGrant {
			tile_kind_id: inventory_kind_id(tile),
			origin: "starter".into(),
			generation_id: tile.id,
			core_tile_id: core_id,
			tile_type_id: type_id,
			character_id,
			pass_version: pass.map(|p| p.version()).unwrap_or(1),
		});
	}
	(grants, cores, types)
}

fn tile(parts: &[(TileColor, RingType)], dots: u8, capture: CaptureMark) -> Tile {
	Tile::with_typed_rings(0, Player::One, parts, 0, dots).with_capture(capture)
}

/// Common green/blue/orange kit: 1–3 rings, mixed dots and capture marks,
/// a few zags (live default has zags on), no beads, braids, emblems, or maroon.
fn starter_tiles() -> Vec<Tile> {
	use CaptureMark::{Dot, Triangle};
	use RingType::{Normal, Zag};
	use TileColor::{Blue, Green, Orange};

	vec![
		tile(&[(Green, Normal)], 1, Triangle),
		tile(&[(Blue, Normal)], 1, Triangle),
		tile(&[(Orange, Normal)], 1, Triangle),
		tile(&[(Green, Normal)], 1, Dot),
		tile(&[(Blue, Normal)], 1, Dot),
		tile(&[(Orange, Normal)], 1, Dot),
		tile(&[(Green, Normal), (Blue, Normal)], 2, Triangle),
		tile(&[(Green, Normal), (Orange, Normal)], 2, Triangle),
		tile(&[(Blue, Normal), (Orange, Normal)], 2, Dot),
		tile(&[(Green, Zag)], 1, Triangle),
		tile(&[(Blue, Zag)], 1, Dot),
		tile(&[(Orange, Zag)], 2, Triangle),
		tile(&[(Green, Normal), (Blue, Normal), (Orange, Normal)], 3, Triangle),
		tile(&[(Green, Normal), (Blue, Normal), (Orange, Normal)], 3, Dot),
		tile(&[(Green, Normal), (Orange, Zag)], 2, Dot),
	]
}

#[cfg(test)]
mod tests {
	use super::*;
	use apa_kan_core::game::TileColor;
	use std::collections::HashSet;

	#[test]
	fn starter_kit_is_fifteen_distinct_common_tiles() {
		let tiles = starter_tiles();
		assert_eq!(tiles.len(), STARTER_TILE_COUNT);
		assert_eq!(starter_grants().len(), STARTER_TILE_COUNT);

		let ids: HashSet<_> = tiles.iter().map(|t| inventory_kind_id(*t)).collect();
		assert_eq!(ids.len(), STARTER_TILE_COUNT);

		let mut captures = HashSet::new();
		for tile in &tiles {
			assert!(tile.emblem.is_none());
			assert_eq!(tile.bead_ring_count(), 0);
			assert_eq!(tile.braid_ring_count(), 0);
			for color in tile.ring_colors() {
				assert!(
					matches!(color, TileColor::Green | TileColor::Blue | TileColor::Orange),
					"starter kit must stay on common colors, got {color:?}"
				);
				assert_ne!(*color, TileColor::Maroon);
			}
			captures.insert(tile.capture);
		}
		assert!(captures.contains(&CaptureMark::Dot));
		assert!(captures.contains(&CaptureMark::Triangle));
	}
}
