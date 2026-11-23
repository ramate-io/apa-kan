//! Starter kit granted on account create (Decision #58-17).
//!
//! Q14 stand-in until a central rarity table exists: 15 fairly common tiles
//! in green / blue / orange, no maroon (or other rare colors), mixed capture
//! marks, under the locked live engine types (`RingType`, `CaptureMark`,
//! `Tile::kind_index`).
//!
//! Depends on the `apa-kan` game crate for now. Switch to `apa-kan-core`
//! once that split lands.

use apa_kan::game::{CaptureMark, Player, RingType, Tile, TileColor, KIND_COUNT};

use apa_kan_store::TileKindId;

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

/// `(tile_kind_id, origin)` rows granted on account create.
pub fn starter_grants() -> Vec<(TileKindId, String)> {
	starter_tiles()
		.into_iter()
		.map(|tile| (inventory_kind_id(tile), "starter".to_string()))
		.collect()
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
	use apa_kan::game::TileColor;
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
