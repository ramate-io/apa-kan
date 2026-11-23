//! Stable hex ids for core tiles (ordered) and tile types (unordered).

use apa_kan_core::canonical::{core_tile_canonical_json, tile_type_canonical_json};
use apa_kan_core::character_pass::CoreKey;
use apa_kan_core::core_pass::CoreTile;
use sha3::{Digest, Sha3_256};

const CORE_TILE_DOMAIN: &[u8] = b"apa-kan/core-tile-id/v1";
const TILE_TYPE_DOMAIN: &[u8] = b"apa-kan/tile-type-id/v1";

/// Issued tile id is [`crate::model::TileInstanceRow::instance_id`].
pub type IssuedTileId = u64;

pub fn core_tile_id(tile: &CoreTile) -> String {
	hash_hex(CORE_TILE_DOMAIN, core_tile_canonical_json(tile).as_bytes())
}

pub fn tile_type_id(key: &CoreKey) -> String {
	hash_hex(TILE_TYPE_DOMAIN, tile_type_canonical_json(key).as_bytes())
}

pub fn tile_type_id_from_core(tile: &CoreTile) -> String {
	tile_type_id(&CoreKey::from_core(tile))
}

fn hash_hex(domain: &[u8], payload: &[u8]) -> String {
	let mut hasher = Sha3_256::new();
	hasher.update(domain);
	hasher.update(payload);
	hex::encode(hasher.finalize())
}

pub fn validate_character_id(id: &str) -> Result<(), crate::StoreError> {
	if id.is_empty() {
		return Err(crate::StoreError::InvalidInput("character id is empty".into()));
	}
	let mut chars = id.chars();
	let Some(first) = chars.next() else {
		return Err(crate::StoreError::InvalidInput("character id is empty".into()));
	};
	if !first.is_ascii_lowercase() {
		return Err(crate::StoreError::InvalidInput(
			"character id must start with a lowercase letter".into(),
		));
	}
	if !chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-') {
		return Err(crate::StoreError::InvalidInput(
			"character id must be kebab-case [a-z][a-z0-9-]*".into(),
		));
	}
	Ok(())
}

#[cfg(test)]
mod tests {
	use super::*;
	use apa_kan_core::core_pass::CoreTile;
	use apa_kan_core::game::{CaptureMark, Emblem, RingType, TileColor};

	fn tile(order_swap: bool) -> CoreTile {
		if order_swap {
			CoreTile {
				ring_type: vec![RingType::Normal, RingType::Braid],
				rings: vec![TileColor::Blue, TileColor::Green],
				dots: 1,
				capture: CaptureMark::Triangle,
				emblem: Emblem::None,
			}
		} else {
			CoreTile {
				ring_type: vec![RingType::Braid, RingType::Normal],
				rings: vec![TileColor::Green, TileColor::Blue],
				dots: 1,
				capture: CaptureMark::Triangle,
				emblem: Emblem::None,
			}
		}
	}

	#[test]
	fn core_tile_id_depends_on_slot_order() {
		assert_ne!(core_tile_id(&tile(false)), core_tile_id(&tile(true)));
	}

	#[test]
	fn tile_type_id_ignores_slot_order() {
		assert_eq!(tile_type_id_from_core(&tile(false)), tile_type_id_from_core(&tile(true)));
	}

	#[test]
	fn character_id_kebab() {
		validate_character_id("waking-rose").unwrap();
		assert!(validate_character_id("Waking").is_err());
		assert!(validate_character_id("").is_err());
	}
}
