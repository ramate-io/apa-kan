//! Symbolic pass versioning: rows carry `valid_from`/`valid_to` membership
//! plus a published/draft pointer. No table copies.

use std::collections::BTreeMap;

use apa_kan_core::character_pass::{
	CharacterTables, CoreKey, SnapshotCharacterTables, SparseCharacterRow,
};
use serde::{Deserialize, Serialize};

use crate::ann::{brute_force_nearest, vector_from_row, AnnHit};
use crate::error::StoreError;

pub const ROLE_ADMIN: &str = "admin";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PassPointer {
	pub draft_version: u16,
	pub published_version: Option<u16>,
}

impl Default for PassPointer {
	fn default() -> Self {
		Self { draft_version: 1, published_version: None }
	}
}

impl PassPointer {
	/// First edit after a publish (or after seeding with draft == published)
	/// opens the next version. Unchanged rows keep `valid_to = NULL` so they
	/// remain members of the new draft.
	pub fn ensure_draft(&mut self) -> u16 {
		if self.published_version == Some(self.draft_version) {
			self.draft_version = self.draft_version.saturating_add(1);
		}
		self.draft_version
	}

	pub fn publish(&mut self) -> Result<u16, StoreError> {
		if self.published_version == Some(self.draft_version) {
			return Err(StoreError::InvalidInput(
				"nothing to publish (draft equals published)".into(),
			));
		}
		self.published_version = Some(self.draft_version);
		Ok(self.draft_version)
	}
}

pub fn is_live(valid_from: u16, valid_to: Option<u16>, version: u16) -> bool {
	valid_from <= version && valid_to.map(|t| version < t).unwrap_or(true)
}

#[derive(Clone, Debug)]
pub struct ConfigRev {
	pub valid_from: u16,
	pub valid_to: Option<u16>,
	pub tau: f32,
	pub k: Option<usize>,
}

#[derive(Clone, Debug)]
pub struct EmbeddingRev {
	pub character_id: String,
	pub valid_from: u16,
	pub valid_to: Option<u16>,
	pub row: SparseCharacterRow,
	pub vector: Vec<f32>,
}

#[derive(Clone, Debug)]
pub struct FallbackRev {
	pub valid_from: u16,
	pub valid_to: Option<u16>,
	pub ids: Vec<String>,
}

#[derive(Clone, Debug)]
pub struct SpecificRev {
	pub valid_from: u16,
	pub valid_to: Option<u16>,
	pub maps: BTreeMap<CoreKey, Vec<String>>,
}

/// In-process book used by [`crate::memory::MemoryStore`] and as the
/// reference for SQL backends.
#[derive(Clone, Debug, Default)]
pub struct PassBook {
	pub pointer: PassPointer,
	pub configs: Vec<ConfigRev>,
	pub embeddings: Vec<EmbeddingRev>,
	pub fallbacks: Vec<FallbackRev>,
	pub specifics: Vec<SpecificRev>,
}

impl PassBook {
	pub fn seed_from_tables<T: CharacterTables>(&mut self, tables: &T) -> Result<(), StoreError> {
		let version = tables.version();
		self.pointer = PassPointer { draft_version: version, published_version: Some(version) };
		self.configs = vec![ConfigRev {
			valid_from: version,
			valid_to: None,
			tau: tables.tau(),
			k: tables.k(),
		}];
		self.embeddings = tables
			.embedding_rows()
			.iter()
			.map(|row| EmbeddingRev {
				character_id: row.id.clone(),
				valid_from: version,
				valid_to: None,
				vector: vector_from_row(row),
				row: row.clone(),
			})
			.collect();
		self.fallbacks = vec![FallbackRev {
			valid_from: version,
			valid_to: None,
			ids: tables.fallback_ids().to_vec(),
		}];
		self.specifics = vec![SpecificRev {
			valid_from: version,
			valid_to: None,
			maps: tables.specific_all().into_iter().collect(),
		}];
		Ok(())
	}

	pub fn snapshot(&self, version: u16) -> Result<SnapshotCharacterTables, StoreError> {
		let cfg = self
			.configs
			.iter()
			.find(|c| is_live(c.valid_from, c.valid_to, version))
			.ok_or_else(|| StoreError::NotFound)?;
		let rows: Vec<_> = self
			.embeddings
			.iter()
			.filter(|e| is_live(e.valid_from, e.valid_to, version))
			.map(|e| e.row.clone())
			.collect();
		let fallback = self
			.fallbacks
			.iter()
			.find(|f| is_live(f.valid_from, f.valid_to, version))
			.map(|f| f.ids.clone())
			.ok_or(StoreError::NotFound)?;
		let maps = self
			.specifics
			.iter()
			.find(|s| is_live(s.valid_from, s.valid_to, version))
			.map(|s| s.maps.clone())
			.unwrap_or_default();
		SnapshotCharacterTables::new(version, cfg.tau, cfg.k, maps, rows, fallback)
			.map_err(StoreError::InvalidInput)
	}

	pub fn nearest(&self, version: u16, query: &[f32], tau: f32, k: Option<usize>) -> Vec<AnnHit> {
		let rows: Vec<(String, Vec<f32>)> = self
			.embeddings
			.iter()
			.filter(|e| is_live(e.valid_from, e.valid_to, version))
			.map(|e| (e.character_id.clone(), e.vector.clone()))
			.collect();
		brute_force_nearest(&rows, query, tau, k)
	}

	pub fn close_open<T>(
		items: &mut [T],
		draft: u16,
		is_open: impl Fn(&T) -> bool,
		set_to: impl Fn(&mut T, u16),
	) {
		for item in items {
			if is_open(item) {
				set_to(item, draft);
			}
		}
	}

	pub fn upsert_embedding(&mut self, row: SparseCharacterRow) -> Result<u16, StoreError> {
		row.validate().map_err(StoreError::InvalidInput)?;
		let draft = self.pointer.ensure_draft();
		for e in &mut self.embeddings {
			if e.character_id == row.id && e.valid_to.is_none() {
				e.valid_to = Some(draft);
			}
		}
		self.embeddings.push(EmbeddingRev {
			character_id: row.id.clone(),
			valid_from: draft,
			valid_to: None,
			vector: vector_from_row(&row),
			row,
		});
		Ok(draft)
	}

	pub fn set_fallback(&mut self, ids: Vec<String>) -> Result<u16, StoreError> {
		if ids.is_empty() {
			return Err(StoreError::InvalidInput("fallback set is empty".into()));
		}
		let draft = self.pointer.ensure_draft();
		for f in &mut self.fallbacks {
			if f.valid_to.is_none() {
				f.valid_to = Some(draft);
			}
		}
		self.fallbacks.push(FallbackRev { valid_from: draft, valid_to: None, ids });
		Ok(draft)
	}

	pub fn set_specific(
		&mut self,
		maps: BTreeMap<CoreKey, Vec<String>>,
	) -> Result<u16, StoreError> {
		let draft = self.pointer.ensure_draft();
		for s in &mut self.specifics {
			if s.valid_to.is_none() {
				s.valid_to = Some(draft);
			}
		}
		self.specifics.push(SpecificRev { valid_from: draft, valid_to: None, maps });
		Ok(draft)
	}

	pub fn set_config(&mut self, tau: f32, k: Option<usize>) -> Result<u16, StoreError> {
		if !(0.0..=1.0).contains(&tau) {
			return Err(StoreError::InvalidInput(format!("tau {tau} is outside [0, 1]")));
		}
		if let Some(0) = k {
			return Err(StoreError::InvalidInput("k must be >= 1 when set".into()));
		}
		let draft = self.pointer.ensure_draft();
		for c in &mut self.configs {
			if c.valid_to.is_none() {
				c.valid_to = Some(draft);
			}
		}
		self.configs.push(ConfigRev { valid_from: draft, valid_to: None, tau, k });
		Ok(draft)
	}

	pub fn publish(&mut self) -> Result<u16, StoreError> {
		let version = self.pointer.publish()?;
		let _ = self.snapshot(version)?;
		Ok(version)
	}
}
