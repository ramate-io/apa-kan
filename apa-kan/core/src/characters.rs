//! Character catalog: ids and display names. No Bevy, no PNG decode.
//!
//! The source of truth is [`MANIFEST_TOML`]
//! (`apa-kan/game/assets/characters/manifest.toml`). PNG files live next to
//! that manifest so the Bevy client's `assets/` folder can load them via
//! `characters/<file>.png`. Generation assigns a [`CharacterId`] on
//! [`crate::game::Tile`], including the explicit no-art id [`FACELESS_ID`].

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use serde::Deserialize;

/// Catalog id for a tile with no painted face. Not [`Option::None`]: generation
/// always stores a [`CharacterId`], and this is the explicit empty outcome.
pub const FACELESS_ID: &str = "faceless";

/// Copy-sized catalog handle. Indexes the bundled [`CharacterLibrary`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct CharacterId(u8);

impl CharacterId {
	/// Sentinel used only if the catalog failed to load (should not happen
	/// with the bundled manifest).
	pub const INVALID: Self = Self(u8::MAX);

	pub fn index(self) -> Option<usize> {
		(self.0 != u8::MAX).then_some(self.0 as usize)
	}

	pub fn as_str(self) -> &'static str {
		bundled_ids().get(self.0 as usize).copied().unwrap_or("unknown")
	}

	pub fn parse(id: &str) -> Option<Self> {
		bundled_ids().iter().position(|&s| s == id).map(|i| CharacterId(i as u8))
	}

	/// Explicit no-art catalog id. Constructors and old JSONL replay use this
	/// instead of absence.
	pub fn faceless() -> Self {
		Self::parse(FACELESS_ID).unwrap_or(Self::INVALID)
	}

	pub fn is_faceless(self) -> bool {
		self.as_str() == FACELESS_ID
	}
}

fn bundled_ids() -> &'static [&'static str] {
	static IDS: OnceLock<Vec<&'static str>> = OnceLock::new();
	IDS.get_or_init(|| match CharacterLibrary::load() {
		Ok(lib) => lib
			.characters
			.iter()
			.map(|c| Box::leak(c.id.clone().into_boxed_str()) as &'static str)
			.collect(),
		Err(_) => Vec::new(),
	})
}

/// Bundled TOML listing every character in the Bevy client's `assets/characters/`.
pub const MANIFEST_TOML: &str = include_str!("../../game/assets/characters/manifest.toml");

/// PNG magic number (`\x89PNG\r\n\x1a\n`).
pub const PNG_MAGIC: &[u8] = &[0x89, b'P', b'N', b'G', b'\r', b'\n', 0x1a, b'\n'];

/// One character medallion in the library.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct Character {
	/// Stable kebab-case id (PNG file stem, e.g. `waking-rose`).
	pub id: String,
	/// Path relative to the Bevy `assets/` folder, e.g. `characters/waking-rose.png`.
	/// [`FACELESS_ID`] omits this: there is no PNG.
	#[serde(default)]
	pub path: Option<String>,
	/// Human-readable name: file stem, dashes turned into spaces, title-cased.
	pub display_name: String,
}

impl Character {
	pub fn is_faceless(&self) -> bool {
		self.id == FACELESS_ID
	}

	pub fn has_art(&self) -> bool {
		self.path.as_deref().is_some_and(|p| !p.is_empty())
	}
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
			match ch.path.as_deref() {
				None | Some("") if ch.is_faceless() => {}
				None | Some("") => {
					return Err(format!(
						"character '{}' has empty path (only '{FACELESS_ID}' may omit art)",
						ch.id
					));
				}
				Some(path) => validate_asset_path(&ch.id, path)?,
			}
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

	pub fn id_of(&self, id: &str) -> Option<CharacterId> {
		self.characters.iter().position(|ch| ch.id == id).map(|i| CharacterId(i as u8))
	}

	pub fn id_at(&self, index: usize) -> Option<CharacterId> {
		(index < self.characters.len()).then_some(CharacterId(index as u8))
	}

	pub fn get_id(&self, id: CharacterId) -> Option<&Character> {
		id.index().and_then(|i| self.characters.get(i))
	}

	/// Absolute path on disk: `{workspace}/apa-kan/game/assets/{path}`.
	pub fn asset_path(&self, character: &Character) -> Option<PathBuf> {
		character.path.as_ref().map(|rel| assets_root().join(rel))
	}
}

/// Bevy client asset folder. PNGs stay in `apa-kan/game/assets`.
pub fn assets_root() -> PathBuf {
	Path::new(env!("CARGO_MANIFEST_DIR")).join("../game/assets")
}

/// `{assets_root}/characters`.
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
	let expected = format!("characters/{id}.png");
	if path != expected {
		return Err(format!("character '{id}' path '{path}' must be '{expected}'"));
	}
	Ok(())
}

#[cfg(test)]
mod tests {
	#![allow(clippy::unwrap_used, clippy::expect_used)]

	use super::*;

	#[test]
	fn bundled_manifest_has_twenty_five_painted_and_faceless() {
		let lib = CharacterLibrary::load().unwrap();
		assert_eq!(lib.len(), 26);
		assert!(!lib.is_empty());
		let faceless = lib.get(FACELESS_ID).expect("faceless catalog id");
		assert!(faceless.is_faceless());
		assert!(!faceless.has_art());
		assert_eq!(CharacterId::faceless().as_str(), FACELESS_ID);
		assert!(CharacterId::faceless().is_faceless());
		let first = lib.get("cadredl").expect("first painted catalog id");
		assert_eq!(first.display_name, "Cadredl");
		assert_eq!(first.path.as_deref(), Some("characters/cadredl.png"));
		let rose = lib.get("waking-rose").expect("dashed id");
		assert_eq!(rose.display_name, "Waking Rose");
		assert_eq!(rose.path.as_deref(), Some("characters/waking-rose.png"));
		let last = lib.get("zaster").expect("last painted catalog id");
		assert_eq!(last.display_name, "Zaster");
		assert_eq!(last.path.as_deref(), Some("characters/zaster.png"));
		for ch in lib.all() {
			if ch.is_faceless() {
				continue;
			}
			let stem =
				Path::new(ch.path.as_deref().unwrap()).file_stem().unwrap().to_str().unwrap();
			assert_eq!(ch.id, stem, "id must match the PNG file stem");
		}
	}

	#[test]
	fn parse_rejects_duplicate_ids() {
		let err = CharacterLibrary::parse(
			r#"
[[characters]]
id = "dup"
path = "characters/dup.png"
display_name = "A"
[[characters]]
id = "dup"
path = "characters/dup.png"
display_name = "B"
"#,
		)
		.unwrap_err();
		assert!(err.contains("duplicate character id"), "{err}");
	}
}
