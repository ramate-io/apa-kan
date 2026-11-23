//! ANN lookup: exact brute-force cosine (Memory/SQLite/tests) and pgvector
//! (Postgres). Generation never uses this path; it loads a snapshot and calls
//! [`apa_kan_core::character_pass::assign_character`].

use apa_kan_core::character_pass::{cosine, encode_character, SparseCharacterRow, EMBED_DIM};

#[derive(Clone, Debug, PartialEq)]
pub struct AnnHit {
	pub character_id: String,
	pub cosine: f32,
}

/// Exact scan. Deterministic: cosine desc, then id asc.
pub fn brute_force_nearest(
	rows: &[(String, Vec<f32>)],
	query: &[f32],
	tau: f32,
	k: Option<usize>,
) -> Vec<AnnHit> {
	let mut hits: Vec<AnnHit> = rows
		.iter()
		.filter_map(|(id, vec)| {
			let c = cosine(query, vec);
			(c >= tau).then_some(AnnHit { character_id: id.clone(), cosine: c })
		})
		.collect();
	hits.sort_by(|a, b| {
		b.cosine
			.partial_cmp(&a.cosine)
			.unwrap_or(std::cmp::Ordering::Equal)
			.then(a.character_id.cmp(&b.character_id))
	});
	if let Some(k) = k {
		hits.truncate(k.max(1));
	}
	hits
}

pub fn vector_from_row(row: &SparseCharacterRow) -> Vec<f32> {
	encode_character(row)
}

pub fn vector_to_blob(v: &[f32]) -> Vec<u8> {
	let mut out = Vec::with_capacity(EMBED_DIM * 4);
	for slot in v.iter().take(EMBED_DIM) {
		out.extend_from_slice(&slot.to_le_bytes());
	}
	while out.len() < EMBED_DIM * 4 {
		out.extend_from_slice(&0f32.to_le_bytes());
	}
	out
}

pub fn blob_to_vector(blob: &[u8]) -> Vec<f32> {
	let mut v = Vec::with_capacity(EMBED_DIM);
	for chunk in blob.chunks_exact(4).take(EMBED_DIM) {
		let mut b = [0u8; 4];
		b.copy_from_slice(chunk);
		v.push(f32::from_le_bytes(b));
	}
	while v.len() < EMBED_DIM {
		v.push(0.0);
	}
	v
}

#[cfg(test)]
mod tests {
	use super::*;
	use apa_kan_core::character_pass::SparseCharacterRow;
	use apa_kan_core::game::TileColor;
	use std::collections::BTreeMap;

	#[test]
	fn blob_round_trip() {
		let row = SparseCharacterRow {
			id: "westerlit".into(),
			color: BTreeMap::from([(TileColor::Teal, 2.0)]),
			ring_type: BTreeMap::new(),
			emblem: BTreeMap::new(),
			n_rings: 0.0,
			capture: BTreeMap::new(),
			dots: 0.0,
		};
		let v = vector_from_row(&row);
		assert_eq!(v.len(), EMBED_DIM);
		assert_eq!(blob_to_vector(&vector_to_blob(&v)), v);
	}
}
