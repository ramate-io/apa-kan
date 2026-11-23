//! Stable JSON for core-tile and tile-type identity (order vs multiset).

use crate::character_pass::CoreKey;
use crate::core_pass::CoreTile;
use crate::game::{CaptureMark, Emblem, RingType, TileColor};

/// Ordered face: slot order inner→outer is part of the identity.
pub fn core_tile_canonical_json(tile: &CoreTile) -> String {
	face_json(
		tile.rings.iter().copied().zip(tile.ring_type.iter().copied()),
		tile.dots,
		tile.capture,
		tile.emblem,
	)
}

/// Unordered face: slot order is ignored (the RFC tile type / CoreKey).
pub fn tile_type_canonical_json(key: &CoreKey) -> String {
	face_json(key.rings().iter().copied(), key.dots(), key.capture(), key.emblem())
}

fn face_json(
	rings: impl IntoIterator<Item = (TileColor, RingType)>,
	dots: u8,
	capture: CaptureMark,
	emblem: Emblem,
) -> String {
	let rings: Vec<String> = rings
		.into_iter()
		.map(|(c, t)| format!("[\"{}\",\"{}\"]", c.name(), t.name()))
		.collect();
	format!(
		"{{\"capture\":\"{}\",\"dots\":{},\"emblem\":\"{}\",\"rings\":[{}]}}",
		capture.name(),
		dots.clamp(1, 3),
		emblem.name(),
		rings.join(",")
	)
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::character_pass::CoreKey;
	use crate::core_pass::CoreTile;
	use crate::game::{CaptureMark, Emblem, RingType, TileColor};

	#[test]
	fn core_tile_keeps_slot_order() {
		let a = CoreTile {
			ring_type: vec![RingType::Braid, RingType::Normal],
			rings: vec![TileColor::Green, TileColor::Blue],
			dots: 2,
			capture: CaptureMark::Dot,
			emblem: Emblem::None,
		};
		let b = CoreTile {
			ring_type: vec![RingType::Normal, RingType::Braid],
			rings: vec![TileColor::Blue, TileColor::Green],
			dots: 2,
			capture: CaptureMark::Dot,
			emblem: Emblem::None,
		};
		assert_ne!(core_tile_canonical_json(&a), core_tile_canonical_json(&b));
	}

	#[test]
	fn tile_type_ignores_slot_order() {
		let a = CoreKey::new(
			[(TileColor::Green, RingType::Braid), (TileColor::Blue, RingType::Normal)],
			2,
			CaptureMark::Dot,
			Emblem::None,
		);
		let b = CoreKey::new(
			[(TileColor::Blue, RingType::Normal), (TileColor::Green, RingType::Braid)],
			2,
			CaptureMark::Dot,
			Emblem::None,
		);
		assert_eq!(tile_type_canonical_json(&a), tile_type_canonical_json(&b));
	}
}
