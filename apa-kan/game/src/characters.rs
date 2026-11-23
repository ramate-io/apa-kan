//! Character illustration library: Bevy asset paths and PNG checks.
//!
//! Catalog types (`CharacterId`, `CharacterLibrary`) live in
//! [`apa_kan_core::characters`] so `Tile` and generation stay Bevy-free.
//! This module re-exports them and checks that every painted id has a PNG.

pub use apa_kan_core::characters::*;

#[cfg(test)]
mod tests {
	#![allow(clippy::unwrap_used, clippy::expect_used)]

	use std::path::PathBuf;

	use super::*;

	#[test]
	fn every_manifest_entry_exists_and_loads_as_png() {
		let lib = CharacterLibrary::load().unwrap();
		assert!(!lib.is_empty(), "character library must list at least one entry");
		let mut painted = 0usize;
		for ch in lib.all() {
			if !ch.has_art() {
				assert!(ch.is_faceless(), "only faceless may omit a PNG, got {}", ch.id);
				continue;
			}
			painted += 1;
			let path = lib.asset_path(ch).expect("painted character has a path");
			assert!(path.is_file(), "character '{}' file missing at {}", ch.id, path.display());
			let bytes = std::fs::read(&path).unwrap_or_else(|e| {
				panic!("character '{}' failed to read {}: {e}", ch.id, path.display())
			});
			assert!(
				bytes.starts_with(PNG_MAGIC),
				"character '{}' at {} is not a PNG (bad magic)",
				ch.id,
				path.display()
			);
			let img = image::load_from_memory_with_format(&bytes, image::ImageFormat::Png)
				.unwrap_or_else(|e| panic!("character '{}' failed to decode as PNG: {e}", ch.id));
			assert!(
				img.width() >= 1 && img.height() >= 1,
				"character '{}' decoded to an empty image",
				ch.id
			);
		}
		assert_eq!(painted, 25, "twenty-five painted medallions");
	}

	#[test]
	fn every_png_in_the_folder_is_listed() {
		let lib = CharacterLibrary::load().unwrap();
		let mut on_disk: Vec<PathBuf> = std::fs::read_dir(characters_dir())
			.unwrap()
			.filter_map(|entry| entry.ok().map(|e| e.path()))
			.filter(|path| path.extension().is_some_and(|ext| ext == "png"))
			.collect();
		on_disk.sort();
		let mut listed: Vec<PathBuf> = lib
			.all()
			.iter()
			.filter(|ch| ch.has_art())
			.filter_map(|ch| lib.asset_path(ch))
			.collect();
		listed.sort();
		assert_eq!(on_disk, listed, "manifest.toml must list every PNG in assets/characters/");
	}
}
