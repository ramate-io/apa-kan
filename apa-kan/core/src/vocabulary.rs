//! Frozen admin-facing symbol vocabulary for sparse character rows.
//!
//! Keys match `embedding.json`: `color.*`, `type.*`, `emblem.*`,
//! `marks.n_rings` / `marks.dots` / `marks.capture.*`. Omitted keys are no
//! opinion. An all-zero row is invalid. Negative weights repel.

use crate::game::{CaptureMark, Emblem, RingType, TileColor};

/// One slider/chip in the admin attribute panel.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct VocabEntry {
	pub key: &'static str,
	pub label: &'static str,
	pub group: VocabGroup,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VocabGroup {
	Color,
	RingType,
	Emblem,
	Marks,
}

/// Emblem::None is the origin and is not a vocabulary entry.
pub const VOCABULARY: &[VocabEntry] = &[
	VocabEntry { key: "color.green", label: "Green", group: VocabGroup::Color },
	VocabEntry { key: "color.blue", label: "Blue", group: VocabGroup::Color },
	VocabEntry { key: "color.orange", label: "Orange", group: VocabGroup::Color },
	VocabEntry { key: "color.lavender-pink", label: "Lavender pink", group: VocabGroup::Color },
	VocabEntry { key: "color.teal", label: "Teal", group: VocabGroup::Color },
	VocabEntry { key: "color.maroon", label: "Maroon", group: VocabGroup::Color },
	VocabEntry { key: "type.normal", label: "Normal", group: VocabGroup::RingType },
	VocabEntry { key: "type.zag", label: "Zag", group: VocabGroup::RingType },
	VocabEntry { key: "type.braid", label: "Braid", group: VocabGroup::RingType },
	VocabEntry { key: "type.bead", label: "Bead", group: VocabGroup::RingType },
	VocabEntry { key: "emblem.crescent", label: "Crescent moon", group: VocabGroup::Emblem },
	VocabEntry { key: "emblem.zs", label: "Two zag Zs", group: VocabGroup::Emblem },
	VocabEntry { key: "emblem.blocker", label: "Blocker", group: VocabGroup::Emblem },
	VocabEntry { key: "emblem.rosette", label: "Rosette", group: VocabGroup::Emblem },
	VocabEntry { key: "marks.n_rings", label: "Ring count", group: VocabGroup::Marks },
	VocabEntry { key: "marks.dots", label: "Dots (MOVE)", group: VocabGroup::Marks },
	VocabEntry { key: "marks.capture.dot", label: "Capture: dot", group: VocabGroup::Marks },
	VocabEntry {
		key: "marks.capture.triangle",
		label: "Capture: triangle",
		group: VocabGroup::Marks,
	},
];

pub fn color_key(color: TileColor) -> String {
	format!("color.{}", color.name())
}

pub fn type_key(typ: RingType) -> String {
	format!("type.{}", typ.name())
}

pub fn emblem_key(emblem: Emblem) -> Option<String> {
	match emblem {
		Emblem::None => None,
		other => Some(format!("emblem.{}", other.name())),
	}
}

pub fn capture_key(mark: CaptureMark) -> String {
	format!("marks.capture.{}", mark.name())
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn vocabulary_covers_embedding_json_keys() {
		let keys: Vec<_> = VOCABULARY.iter().map(|e| e.key).collect();
		assert!(keys.contains(&"color.lavender-pink"));
		assert!(keys.contains(&"type.zag"));
		assert!(keys.contains(&"emblem.zs"));
		assert!(keys.contains(&"marks.n_rings"));
		assert!(keys.contains(&"marks.dots"));
		assert!(keys.contains(&"marks.capture.dot"));
		assert_eq!(VOCABULARY.len(), 18);
		assert!(emblem_key(Emblem::None).is_none());
	}
}
