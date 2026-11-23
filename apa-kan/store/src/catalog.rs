//! Catalog, issued-tile, and role records for the admin store.

use apa_kan_account::id::AccountId;
use apa_kan_core::character_pass::SparseCharacterRow;
use serde::{Deserialize, Serialize};

use crate::model::TileKindId;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CharacterRecord {
	pub character_id: String,
	pub display_name: String,
	pub image_png: Option<Vec<u8>>,
	pub created_at_unix: i64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CharacterDetail {
	pub record: CharacterRecord,
	pub embedding: Option<SparseCharacterRow>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CoreTileRecord {
	pub core_tile_id: String,
	pub rings: Vec<(String, String)>,
	pub dots: u8,
	pub capture: String,
	pub emblem: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TileTypeRecord {
	pub tile_type_id: String,
	pub rings: Vec<(String, String)>,
	pub dots: u8,
	pub capture: String,
	pub emblem: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TileGrant {
	pub tile_kind_id: TileKindId,
	pub origin: String,
	pub generation_id: u32,
	pub core_tile_id: String,
	pub tile_type_id: String,
	pub character_id: String,
	pub pass_version: u16,
}

impl TileGrant {
	pub fn kind_only(tile_kind_id: TileKindId, origin: impl Into<String>) -> Self {
		Self {
			tile_kind_id,
			origin: origin.into(),
			generation_id: 0,
			core_tile_id: String::new(),
			tile_type_id: String::new(),
			character_id: String::new(),
			pass_version: 0,
		}
	}
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TileFilter {
	pub character_id: Option<String>,
	pub core_tile_id: Option<String>,
	pub tile_type_id: Option<String>,
	pub account_id: Option<AccountId>,
	pub limit: Option<u32>,
	pub offset: Option<u32>,
}
