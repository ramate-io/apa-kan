//! Character illustration library: ids, Bevy asset paths, display names.
//!
//! Groundwork only. Tile/deck generation does not assign characters yet.
//! The source of truth is [`MANIFEST_TOML`]
//! (`assets/characters/manifest.toml`). PNG files live next to that manifest
//! so Bevy's default `assets/` folder can load them later via
//! `characters/<file>.png`.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use serde::Deserialize;

/// Bundled TOML listing every character in `assets/characters/`.
pub const MANIFEST_TOML: &str = include_str!("../assets/characters/manifest.toml");

/// PNG magic number (`\x89PNG\r\n\x1a\n`).
pub const PNG_MAGIC: &[u8] = &[0x89, b'P', b'N', b'G', b'\r', b'\n', 0x1a, b'\n'];

/// One character medallion in the library.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct Character {
	/// Stable kebab-case id (filename slug without the numeric prefix).
	pub id: String,
	/// Path relative to the Bevy `assets/` folder, e.g. `characters/01-….png`.
	pub path: String,
	/// Human-readable name, currently derived from the filename.
	pub display_name: String,
}

#[derive(Deserialize)]
struct ManifestFile {
	characters: Vec<Character>,
}

/// Parsed character catalog, keyed by [`Character::id`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CharacterLibrary {
	characters: Vec<Character>,
}

impl CharacterLibrary {
	/// Parse the bundled manifest.
	pub fn load() -> Result<Self, String> {
		Self::parse(MANIFEST_TOML)
	}

	/// Parse a manifest TOML string.
	pub fn parse(toml: &str) -> Result<Self, String> {
		let file: ManifestFile =
			toml::from_str(toml).map_err(|e| format!("invalid character manifest: {e}"))?;
		Self::from_entries(file.characters)
	}

	fn from_entries(characters: Vec<Character>) -> Result<Self, String> {
		if characters.is_empty() {
			return Err("character manifest is empty".into());
		}
		let mut seen = BTreeSet::new();
		for ch in &characters {
			if ch.id.is_empty() {
				return Err("character id is empty".into());
			}
			if !seen.insert(ch.id.as_str()) {
				return Err(format!("duplicate character id '{}'", ch.id));
			}
			validate_asset_path(&ch.id, &ch.path)?;
			if ch.display_name.trim().is_empty() {
				return Err(format!("character '{}' has empty display_name", ch.id));
			}
		}
		Ok(Self { characters })
	}

	pub fn len(&self) -> usize {
		self.characters.len()
	}

	pub fn is_empty(&self) -> bool {
		self.characters.is_empty()
	}

	pub fn all(&self) -> &[Character] {
		&self.characters
	}

	pub fn get(&self, id: &str) -> Option<&Character> {
		self.characters.iter().find(|ch| ch.id == id)
	}

	/// Absolute path on disk: `{crate}/assets/{path}`.
	pub fn asset_path(&self, character: &Character) -> PathBuf {
		assets_root().join(&character.path)
	}
}

/// `{CARGO_MANIFEST_DIR}/assets`, the Bevy default asset folder for this crate.
pub fn assets_root() -> PathBuf {
	Path::new(env!("CARGO_MANIFEST_DIR")).join("assets")
}

/// `{CARGO_MANIFEST_DIR}/assets/characters`.
pub fn characters_dir() -> PathBuf {
	assets_root().join("characters")
}

fn validate_asset_path(id: &str, path: &str) -> Result<(), String> {
	if path.is_empty() {
		return Err(format!("character '{id}' has empty path"));
	}
	if path.starts_with('/') || path.contains("..") {
		return Err(format!(
			"character '{id}' path '{path}' must be relative and stay under assets/"
		));
	}
	if !path.starts_with("characters/") || !path.ends_with(".png") {
		return Err(format!("character '{id}' path '{path}' must look like characters/<file>.png"));
	}
	Ok(())
}

#[cfg(test)]
mod tests {
	#![allow(clippy::unwrap_used, clippy::expect_used)]

	use super::*;

	#[test]
	fn bundled_manifest_has_fourteen_unique_characters() {
		let lib = CharacterLibrary::load().unwrap();
		assert_eq!(lib.len(), 14);
		assert!(!lib.is_empty());
		let first = lib.get("blue-diamond-lattice").expect("first catalog id");
		assert_eq!(first.display_name, "Blue Diamond Lattice");
		assert_eq!(first.path, "characters/01-blue-diamond-lattice.png");
		let last = lib.get("green-hatched-bowl").expect("last catalog id");
		assert_eq!(last.display_name, "Green Hatched Bowl");
		assert_eq!(last.path, "characters/14-green-hatched-bowl.png");
	}

	#[test]
	fn every_manifest_entry_exists_and_loads_as_png() {
		let lib = CharacterLibrary::load().unwrap();
		assert!(!lib.is_empty(), "character library must list at least one PNG");
		for ch in lib.all() {
			let path = lib.asset_path(ch);
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
		let mut listed: Vec<PathBuf> = lib.all().iter().map(|ch| lib.asset_path(ch)).collect();
		listed.sort();
		assert_eq!(on_disk, listed, "manifest.toml must list every PNG in assets/characters/");
	}

	#[test]
	fn parse_rejects_duplicate_ids() {
		let err = CharacterLibrary::parse(
			r#"
[[characters]]
id = "dup"
path = "characters/a.png"
display_name = "A"
[[characters]]
id = "dup"
path = "characters/b.png"
display_name = "B"
"#,
		)
		.unwrap_err();
		assert!(err.contains("duplicate character id"), "{err}");
	}
}
