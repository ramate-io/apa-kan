//! Load the bundled catalog + character-pass v1 into an empty store.

use apa_kan_core::character_pass::{JsonCharacterTables, SnapshotCharacterTables};
use apa_kan_core::characters::CharacterLibrary;
use apa_kan_core::characters::PNG_MAGIC;

use crate::catalog::CharacterRecord;
use crate::error::StoreError;
use crate::repos::{CatalogRepo, PassRepo};

pub async fn seed_bundled_v1<S: CatalogRepo + PassRepo + ?Sized>(
	store: &S,
	now_unix: i64,
) -> Result<(), StoreError> {
	let lib = CharacterLibrary::load().map_err(StoreError::InvalidInput)?;
	let json = JsonCharacterTables::load_v1().map_err(StoreError::InvalidInput)?;
	let snap = SnapshotCharacterTables::from_tables(&json).map_err(StoreError::InvalidInput)?;
	for ch in lib.all() {
		let image_png = lib.asset_path(ch).and_then(|path| {
			let bytes = std::fs::read(path).ok()?;
			bytes.starts_with(PNG_MAGIC).then_some(bytes)
		});
		store
			.upsert_character(CharacterRecord {
				character_id: ch.id.clone(),
				display_name: ch.display_name.clone(),
				image_png,
				created_at_unix: now_unix,
			})
			.await?;
	}
	store.seed_snapshot(snap).await?;
	Ok(())
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::memory::MemoryStore;
	use crate::repos::PassRepo;
	use apa_kan_core::character_pass::{assign_character, CharacterTables};
	use apa_kan_core::core_pass::{CorePass, TileId};

	#[tokio::test]
	async fn bundled_seed_matches_json_generation() {
		let store = MemoryStore::new();
		seed_bundled_v1(&store, 0).await.unwrap();
		let pointer = store.pass_pointer().await.unwrap();
		assert_eq!(pointer.published_version, Some(1));
		let tables = store.load_tables(1).await.unwrap();
		let json = JsonCharacterTables::load_v1().unwrap();
		let cfg = CorePass::base();
		for id in 0..64u32 {
			let tile = cfg.run(TileId(id));
			assert_eq!(assign_character(&tables, &tile, id), assign_character(&json, &tile, id));
		}
		assert_eq!(tables.embedding_rows().len(), json.embedding_rows().len());
	}
}
