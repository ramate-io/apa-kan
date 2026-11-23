//! Character assignment pass: specific map, aesthetic embedding, fallback.
//!
//! A pure function of `(core tile, tile.id, pass version)`. Cosmetic: does
//! not affect overwhelm, search `kind_index`, or deck cost. Always returns a
//! [`CharacterId`], including [`crate::characters::FACELESS_ID`].
//!
//! The three tables the RFC calls for (specific mapping, aesthetic embedding,
//! fallback set) sit behind [`CharacterTables`] so a later database / admin
//! API can replace the bundled JSON files without changing the pick rule.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::characters::{CharacterId, CharacterLibrary};
use crate::core_pass::CoreTile;
use crate::game::{CaptureMark, Emblem, RingType, TileColor};

pub const PASS_CONFIG_JSON: &str = include_str!("../../game/assets/characters/pass/v1/config.json");
pub const PASS_SPECIFIC_JSON: &str =
	include_str!("../../game/assets/characters/pass/v1/specific.json");
pub const PASS_EMBEDDING_JSON: &str =
	include_str!("../../game/assets/characters/pass/v1/embedding.json");
pub const PASS_FALLBACK_JSON: &str =
	include_str!("../../game/assets/characters/pass/v1/fallback.json");

/// Frozen vector layout (tile and character encodings share this space):
/// 12 ring atoms + 4 emblem + 1 n_rings + 2 capture + 1 dots.
pub const EMBED_DIM: usize = 20;
const RING_DIM: usize = 12;
const EMBLEM_DIM: usize = 4;
const RING_WEIGHT: f32 = 1.0;
const EMBLEM_WEIGHT: f32 = 0.5;
const N_RINGS_WEIGHT: f32 = 0.2;
const CAPTURE_WEIGHT: f32 = 0.15;
const DOTS_WEIGHT: f32 = 0.15;
/// Residual so two identically tagged characters are not the same point.
const EPSILON_SCALE: f32 = 0.05;

/// Unordered multiset of live `(color, type)` slots plus markings.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct CoreKey {
	rings: Vec<(TileColor, RingType)>,
	dots: u8,
	capture: CaptureMark,
	emblem: Emblem,
}

impl CoreKey {
	pub fn new(
		rings: impl IntoIterator<Item = (TileColor, RingType)>,
		dots: u8,
		capture: CaptureMark,
		emblem: Emblem,
	) -> Self {
		let mut rings: Vec<_> = rings.into_iter().collect();
		rings.sort_by_key(|&(c, t)| (c as u8, t as u8));
		Self { rings, dots: dots.clamp(1, 3), capture, emblem }
	}

	pub fn from_core(tile: &CoreTile) -> Self {
		Self::new(
			tile.rings.iter().copied().zip(tile.ring_type.iter().copied()),
			tile.dots,
			tile.capture,
			tile.emblem,
		)
	}

	pub fn rings(&self) -> &[(TileColor, RingType)] {
		&self.rings
	}

	pub fn dots(&self) -> u8 {
		self.dots
	}

	pub fn capture(&self) -> CaptureMark {
		self.capture
	}

	pub fn emblem(&self) -> Emblem {
		self.emblem
	}

	pub fn to_map_json(&self, characters: &[String]) -> serde_json::Value {
		let rings: Vec<[String; 2]> = self
			.rings
			.iter()
			.map(|(c, t)| [c.name().to_string(), t.name().to_string()])
			.collect();
		serde_json::json!({
			"rings": rings,
			"dots": self.dots,
			"capture": self.capture.name(),
			"emblem": self.emblem.name(),
			"characters": characters,
		})
	}

	pub fn from_map_json(value: &serde_json::Value) -> Result<(Self, Vec<String>), String> {
		let entry: SpecificEntryFile = serde_json::from_value(value.clone())
			.map_err(|e| format!("invalid specific-map JSON: {e}"))?;
		if entry.characters.is_empty() {
			return Err("specific map entry has an empty character set".into());
		}
		let rings = parse_ring_pairs(&entry.rings)?;
		let key = Self::new(
			rings,
			entry.dots,
			CaptureMark::parse(&entry.capture)?,
			Emblem::parse(&entry.emblem)?,
		);
		Ok((key, entry.characters))
	}
}

/// Sparse admin row: omitted keys are 0 (no opinion).
#[derive(Clone, Debug, PartialEq)]
pub struct SparseCharacterRow {
	pub id: String,
	pub color: BTreeMap<TileColor, f32>,
	pub ring_type: BTreeMap<RingType, f32>,
	pub emblem: BTreeMap<Emblem, f32>,
	pub n_rings: f32,
	pub capture: BTreeMap<CaptureMark, f32>,
	pub dots: f32,
}

impl SparseCharacterRow {
	pub fn is_all_zero(&self) -> bool {
		self.color.values().all(|&w| w == 0.0)
			&& self.ring_type.values().all(|&w| w == 0.0)
			&& self.emblem.values().all(|&w| w == 0.0)
			&& self.n_rings == 0.0
			&& self.capture.values().all(|&w| w == 0.0)
			&& self.dots == 0.0
	}

	pub fn validate(&self) -> Result<(), String> {
		if self.id.is_empty() {
			return Err("embedding row has an empty id".into());
		}
		if self.is_all_zero() {
			return Err(format!("embedding row '{}' is all-zero (invalid)", self.id));
		}
		Ok(())
	}

	/// `embedding.json` row object (omitted keys stay absent).
	pub fn to_json_value(&self) -> serde_json::Value {
		let mut obj = serde_json::Map::new();
		obj.insert("id".into(), serde_json::Value::String(self.id.clone()));
		if !self.color.is_empty() {
			obj.insert(
				"color".into(),
				weight_map_json(self.color.iter().map(|(k, w)| (k.name(), *w))),
			);
		}
		if !self.ring_type.is_empty() {
			obj.insert(
				"type".into(),
				weight_map_json(self.ring_type.iter().map(|(k, w)| (k.name(), *w))),
			);
		}
		if !self.emblem.is_empty() {
			obj.insert(
				"emblem".into(),
				weight_map_json(self.emblem.iter().map(|(k, w)| (k.name(), *w))),
			);
		}
		let mut marks = serde_json::Map::new();
		if self.n_rings != 0.0 {
			marks.insert("n_rings".into(), serde_json::json!(self.n_rings));
		}
		if self.dots != 0.0 {
			marks.insert("dots".into(), serde_json::json!(self.dots));
		}
		if !self.capture.is_empty() {
			marks.insert(
				"capture".into(),
				weight_map_json(self.capture.iter().map(|(k, w)| (k.name(), *w))),
			);
		}
		if !marks.is_empty() {
			obj.insert("marks".into(), serde_json::Value::Object(marks));
		}
		serde_json::Value::Object(obj)
	}

	pub fn from_json_value(value: &serde_json::Value) -> Result<Self, String> {
		let raw: EmbeddingRowFile = serde_json::from_value(value.clone())
			.map_err(|e| format!("invalid embedding row JSON: {e}"))?;
		parse_embedding_row(raw)
	}
}

impl Serialize for SparseCharacterRow {
	fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
		self.to_json_value().serialize(serializer)
	}
}

impl<'de> Deserialize<'de> for SparseCharacterRow {
	fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
		let value = serde_json::Value::deserialize(deserializer)?;
		Self::from_json_value(&value).map_err(serde::de::Error::custom)
	}
}

fn weight_map_json<'a>(iter: impl Iterator<Item = (&'a str, f32)>) -> serde_json::Value {
	let mut map = serde_json::Map::new();
	for (k, w) in iter {
		if w != 0.0 {
			map.insert(k.to_string(), serde_json::json!(w));
		}
	}
	serde_json::Value::Object(map)
}

/// Backing store for the three character databases. JSON today; a SQL/admin
/// implementation can replace [`JsonCharacterTables`] later.
pub trait CharacterTables {
	fn version(&self) -> u16;
	fn tau(&self) -> f32;
	fn k(&self) -> Option<usize>;
	fn specific(&self, key: &CoreKey) -> Option<&[String]>;
	/// Every character id that appears in a specific-map set (for catalog checks).
	fn specific_ids(&self) -> Vec<&str>;
	/// All authored exact maps. Used to snapshot a version into the store.
	fn specific_all(&self) -> Vec<(CoreKey, Vec<String>)>;
	fn embedding_rows(&self) -> &[SparseCharacterRow];
	fn fallback_ids(&self) -> &[String];
}

/// Bundled JSON tables for character-pass v1.
#[derive(Clone, Debug)]
pub struct JsonCharacterTables {
	version: u16,
	tau: f32,
	k: Option<usize>,
	specific: BTreeMap<CoreKey, Vec<String>>,
	rows: Vec<SparseCharacterRow>,
	fallback: Vec<String>,
}

#[derive(Deserialize)]
struct ConfigFile {
	version: u16,
	tau: f32,
	k: Option<usize>,
}

#[derive(Deserialize)]
struct SpecificFile {
	maps: Vec<SpecificEntryFile>,
}

#[derive(Deserialize)]
struct SpecificEntryFile {
	rings: Vec<(String, String)>,
	dots: u8,
	capture: String,
	emblem: String,
	characters: Vec<String>,
}

#[derive(Deserialize)]
struct EmbeddingFile {
	rows: Vec<EmbeddingRowFile>,
}

#[derive(Deserialize, Default)]
struct EmbeddingRowFile {
	id: String,
	#[serde(default)]
	color: BTreeMap<String, f32>,
	#[serde(default, rename = "type")]
	ring_type: BTreeMap<String, f32>,
	#[serde(default)]
	emblem: BTreeMap<String, f32>,
	#[serde(default)]
	marks: MarksFile,
}

#[derive(Deserialize, Default)]
struct MarksFile {
	#[serde(default)]
	n_rings: f32,
	#[serde(default)]
	dots: f32,
	#[serde(default)]
	capture: BTreeMap<String, f32>,
}

#[derive(Deserialize)]
struct FallbackFile {
	ids: Vec<String>,
}

impl JsonCharacterTables {
	pub fn load_v1() -> Result<Self, String> {
		Self::parse(PASS_CONFIG_JSON, PASS_SPECIFIC_JSON, PASS_EMBEDDING_JSON, PASS_FALLBACK_JSON)
	}

	pub fn parse(
		config: &str,
		specific: &str,
		embedding: &str,
		fallback: &str,
	) -> Result<Self, String> {
		let cfg: ConfigFile =
			serde_json::from_str(config).map_err(|e| format!("invalid pass config JSON: {e}"))?;
		if !(0.0..=1.0).contains(&cfg.tau) {
			return Err(format!("tau {} is outside [0, 1]", cfg.tau));
		}
		if let Some(k) = cfg.k {
			if k == 0 {
				return Err("k must be >= 1 when set".into());
			}
		}
		let spec_file: SpecificFile = serde_json::from_str(specific)
			.map_err(|e| format!("invalid specific-map JSON: {e}"))?;
		let emb_file: EmbeddingFile =
			serde_json::from_str(embedding).map_err(|e| format!("invalid embedding JSON: {e}"))?;
		let fall_file: FallbackFile =
			serde_json::from_str(fallback).map_err(|e| format!("invalid fallback JSON: {e}"))?;

		let mut specific = BTreeMap::new();
		for entry in spec_file.maps {
			if entry.characters.is_empty() {
				return Err("specific map entry has an empty character set".into());
			}
			let rings = parse_ring_pairs(&entry.rings)?;
			let key = CoreKey::new(
				rings,
				entry.dots,
				CaptureMark::parse(&entry.capture)?,
				Emblem::parse(&entry.emblem)?,
			);
			if specific.insert(key, entry.characters).is_some() {
				return Err("duplicate specific-map key".into());
			}
		}

		let mut seen_rows = BTreeSet::new();
		let mut rows = Vec::with_capacity(emb_file.rows.len());
		for raw in emb_file.rows {
			if raw.id.is_empty() {
				return Err("embedding row has an empty id".into());
			}
			if !seen_rows.insert(raw.id.clone()) {
				return Err(format!("duplicate embedding id '{}'", raw.id));
			}
			let row = parse_embedding_row(raw)?;
			if row.is_all_zero() {
				return Err(format!("embedding row '{}' is all-zero (invalid)", row.id));
			}
			rows.push(row);
		}

		if fall_file.ids.is_empty() {
			return Err("fallback set is empty".into());
		}
		let mut seen_fb = BTreeSet::new();
		for id in &fall_file.ids {
			if id.is_empty() {
				return Err("fallback id is empty".into());
			}
			if !seen_fb.insert(id.clone()) {
				return Err(format!("duplicate fallback id '{id}'"));
			}
			if seen_rows.contains(id) && id != crate::characters::FACELESS_ID {
				return Err(format!(
					"fallback id '{id}' also appears in the embedding (must be disjoint)"
				));
			}
		}

		Ok(Self {
			version: cfg.version,
			tau: cfg.tau,
			k: cfg.k,
			specific,
			rows,
			fallback: fall_file.ids,
		})
	}
}

impl CharacterTables for JsonCharacterTables {
	fn version(&self) -> u16 {
		self.version
	}

	fn tau(&self) -> f32 {
		self.tau
	}

	fn k(&self) -> Option<usize> {
		self.k
	}

	fn specific(&self, key: &CoreKey) -> Option<&[String]> {
		self.specific.get(key).map(|v| v.as_slice())
	}

	fn specific_ids(&self) -> Vec<&str> {
		self.specific.values().flatten().map(String::as_str).collect()
	}

	fn specific_all(&self) -> Vec<(CoreKey, Vec<String>)> {
		self.specific.iter().map(|(k, v)| (k.clone(), v.clone())).collect()
	}

	fn embedding_rows(&self) -> &[SparseCharacterRow] {
		&self.rows
	}

	fn fallback_ids(&self) -> &[String] {
		&self.fallback
	}
}

fn parse_ring_pairs(pairs: &[(String, String)]) -> Result<Vec<(TileColor, RingType)>, String> {
	pairs
		.iter()
		.map(|(c, t)| Ok((TileColor::parse(c)?, RingType::parse(t)?)))
		.collect()
}

/// In-memory snapshot of one pass version. Store backends materialize this
/// for generation so Memory, SQLite, and Postgres pick identically.
#[derive(Clone, Debug)]
pub struct SnapshotCharacterTables {
	version: u16,
	tau: f32,
	k: Option<usize>,
	specific: BTreeMap<CoreKey, Vec<String>>,
	rows: Vec<SparseCharacterRow>,
	fallback: Vec<String>,
}

impl SnapshotCharacterTables {
	pub fn new(
		version: u16,
		tau: f32,
		k: Option<usize>,
		specific: BTreeMap<CoreKey, Vec<String>>,
		rows: Vec<SparseCharacterRow>,
		fallback: Vec<String>,
	) -> Result<Self, String> {
		if !(0.0..=1.0).contains(&tau) {
			return Err(format!("tau {tau} is outside [0, 1]"));
		}
		if let Some(0) = k {
			return Err("k must be >= 1 when set".into());
		}
		if fallback.is_empty() {
			return Err("fallback set is empty".into());
		}
		let mut seen = BTreeSet::new();
		for row in &rows {
			row.validate()?;
			if !seen.insert(row.id.as_str()) {
				return Err(format!("duplicate embedding id '{}'", row.id));
			}
		}
		let mut seen_fb = BTreeSet::new();
		for id in &fallback {
			if id.is_empty() {
				return Err("fallback id is empty".into());
			}
			if !seen_fb.insert(id.as_str()) {
				return Err(format!("duplicate fallback id '{id}'"));
			}
			if seen.contains(id.as_str()) && id != crate::characters::FACELESS_ID {
				return Err(format!(
					"fallback id '{id}' also appears in the embedding (must be disjoint)"
				));
			}
		}
		Ok(Self { version, tau, k, specific, rows, fallback })
	}

	pub fn from_tables<T: CharacterTables>(tables: &T) -> Result<Self, String> {
		Self::new(
			tables.version(),
			tables.tau(),
			tables.k(),
			tables.specific_all().into_iter().collect(),
			tables.embedding_rows().to_vec(),
			tables.fallback_ids().to_vec(),
		)
	}
}

impl CharacterTables for SnapshotCharacterTables {
	fn version(&self) -> u16 {
		self.version
	}

	fn tau(&self) -> f32 {
		self.tau
	}

	fn k(&self) -> Option<usize> {
		self.k
	}

	fn specific(&self, key: &CoreKey) -> Option<&[String]> {
		self.specific.get(key).map(|v| v.as_slice())
	}

	fn specific_ids(&self) -> Vec<&str> {
		self.specific.values().flatten().map(String::as_str).collect()
	}

	fn specific_all(&self) -> Vec<(CoreKey, Vec<String>)> {
		self.specific.iter().map(|(k, v)| (k.clone(), v.clone())).collect()
	}

	fn embedding_rows(&self) -> &[SparseCharacterRow] {
		&self.rows
	}

	fn fallback_ids(&self) -> &[String] {
		&self.fallback
	}
}

/// One cosine neighborhood hit. Sorted by id before `tile.id % len` in
/// [`assign_character`]; preview sorts by cosine instead.
#[derive(Clone, Debug, PartialEq)]
pub struct NeighborHit {
	pub id: String,
	pub cosine: f32,
}

/// Pure character pick as a catalog slug. Same rule as [`CharacterPass::assign`].
pub fn assign_character<T: CharacterTables>(tables: &T, tile: &CoreTile, tile_id: u32) -> String {
	let key = CoreKey::from_core(tile);
	if let Some(set) = tables.specific(&key) {
		if !set.is_empty() {
			return set[(tile_id as usize) % set.len()].clone();
		}
	}
	let tile_vec = encode_tile(tile);
	let mut hits = neighborhood(tables, &tile_vec);
	if hits.is_empty() {
		return fallback_pick(tables, tile_id);
	}
	if let Some(k) = tables.k() {
		hits.sort_by(|a, b| {
			b.cosine
				.partial_cmp(&a.cosine)
				.unwrap_or(std::cmp::Ordering::Equal)
				.then(a.id.cmp(&b.id))
		});
		hits.truncate(k.max(1));
	}
	hits.sort_by(|a, b| a.id.cmp(&b.id));
	hits[(tile_id as usize) % hits.len()].id.clone()
}

fn fallback_pick<T: CharacterTables>(tables: &T, tile_id: u32) -> String {
	let ids = tables.fallback_ids();
	let h = crate::core_pass::TileId(tile_id).avalanche() as usize;
	ids[h % ids.len()].clone()
}

/// Cosine neighborhood under τ, minus fallback-reserved ids (`faceless` stays).
pub fn neighborhood<T: CharacterTables>(tables: &T, query: &[f32]) -> Vec<NeighborHit> {
	neighborhood_rows(tables.embedding_rows(), tables.fallback_ids(), tables.tau(), query)
}

pub fn neighborhood_rows(
	rows: &[SparseCharacterRow],
	fallback: &[String],
	tau: f32,
	query: &[f32],
) -> Vec<NeighborHit> {
	let fallback: BTreeSet<&str> = fallback.iter().map(String::as_str).collect();
	rows.iter()
		.filter(|row| {
			!(fallback.contains(row.id.as_str()) && row.id != crate::characters::FACELESS_ID)
		})
		.filter_map(|row| {
			let c = cosine(query, &encode_character(row));
			(c >= tau).then_some(NeighborHit { id: row.id.clone(), cosine: c })
		})
		.collect()
}

fn parse_embedding_row(raw: EmbeddingRowFile) -> Result<SparseCharacterRow, String> {
	Ok(SparseCharacterRow {
		id: raw.id,
		color: parse_weight_map(&raw.color, TileColor::parse)?,
		ring_type: parse_weight_map(&raw.ring_type, RingType::parse)?,
		emblem: parse_weight_map(&raw.emblem, Emblem::parse)?,
		n_rings: raw.marks.n_rings,
		capture: parse_weight_map(&raw.marks.capture, CaptureMark::parse)?,
		dots: raw.marks.dots,
	})
}

fn parse_weight_map<T: Copy + Ord, E: ToString>(
	raw: &BTreeMap<String, f32>,
	parse: impl Fn(&str) -> Result<T, E>,
) -> Result<BTreeMap<T, f32>, String> {
	let mut out = BTreeMap::new();
	for (k, w) in raw {
		let key = parse(k).map_err(|e| e.to_string())?;
		out.insert(key, *w);
	}
	Ok(out)
}

/// Versioned character pass: tables plus a catalog for resolving ids.
pub struct CharacterPass<T: CharacterTables> {
	tables: T,
	library: CharacterLibrary,
}

impl CharacterPass<JsonCharacterTables> {
	pub fn v1() -> Result<Self, String> {
		Self::new(JsonCharacterTables::load_v1()?, CharacterLibrary::load()?)
	}
}

/// Bundled v1 pass. `None` if the JSON or catalog failed to load.
pub fn bundled() -> Option<&'static CharacterPass<JsonCharacterTables>> {
	static PASS: std::sync::OnceLock<Option<CharacterPass<JsonCharacterTables>>> =
		std::sync::OnceLock::new();
	PASS.get_or_init(|| CharacterPass::v1().ok()).as_ref()
}

impl<T: CharacterTables> CharacterPass<T> {
	pub fn new(tables: T, library: CharacterLibrary) -> Result<Self, String> {
		let fallback: BTreeSet<&str> = tables.fallback_ids().iter().map(String::as_str).collect();
		for id in tables.fallback_ids() {
			library
				.get(id)
				.ok_or_else(|| format!("fallback id '{id}' is not in the catalog"))?;
		}
		for id in tables.specific_ids() {
			library
				.get(id)
				.ok_or_else(|| format!("specific-map id '{id}' is not in the catalog"))?;
		}
		for row in tables.embedding_rows() {
			if fallback.contains(row.id.as_str()) && row.id != crate::characters::FACELESS_ID {
				return Err(format!("embedding row '{}' is reserved for fallback", row.id));
			}
			if library.id_of(&row.id).is_none() {
				return Err(format!("embedding id '{}' is not in the catalog", row.id));
			}
			let _ = encode_character(row);
		}
		Ok(Self { tables, library })
	}

	pub fn tables(&self) -> &T {
		&self.tables
	}

	pub fn library(&self) -> &CharacterLibrary {
		&self.library
	}

	pub fn version(&self) -> u16 {
		self.tables.version()
	}

	/// Assign a catalog character. Specific-map hit wins; otherwise the
	/// cosine neighborhood under τ (minus fallback); otherwise fallback.
	pub fn assign(&self, tile: &CoreTile, tile_id: u32) -> CharacterId {
		let slug = assign_character(&self.tables, tile, tile_id);
		self.library
			.id_of(&slug)
			.or_else(|| self.library.id_at(0))
			.unwrap_or(CharacterId::INVALID)
	}
}

/// Tile encoding: Kronecker ring atoms, L2-normalize, concat extras, normalize.
pub fn encode_tile(tile: &CoreTile) -> Vec<f32> {
	let mut atoms = [0.0f32; RING_DIM];
	let n = tile.n_rings();
	for i in 0..n {
		add_kronecker(&mut atoms, tile.rings[i].rgb(), tile.ring_type[i], 1.0);
	}
	l2_normalize(&mut atoms);
	let mut v = vec![0.0f32; EMBED_DIM];
	for (i, a) in atoms.iter().enumerate() {
		v[i] = a * RING_WEIGHT;
	}
	write_extras(&mut v, n as f32, tile.capture, tile.dots, tile.emblem, 1.0, 1.0, 1.0);
	l2_normalize(&mut v);
	v
}

/// Additive composition of sparse admin weights plus an id residual.
pub fn encode_character(row: &SparseCharacterRow) -> Vec<f32> {
	let mut atoms = [0.0f32; RING_DIM];
	for (&color, &w) in &row.color {
		add_color_anchor(&mut atoms, color, w);
	}
	for (&typ, &w) in &row.ring_type {
		add_type_anchor(&mut atoms, typ, w);
	}
	l2_normalize(&mut atoms);
	let mut v = vec![0.0f32; EMBED_DIM];
	for (i, a) in atoms.iter().enumerate() {
		v[i] = a * RING_WEIGHT;
	}
	for (&emblem, &w) in &row.emblem {
		if let Some(i) = emblem_axis(emblem) {
			v[RING_DIM + i] += w * EMBLEM_WEIGHT;
		}
	}
	v[RING_DIM + EMBLEM_DIM] += row.n_rings * N_RINGS_WEIGHT;
	for (&cap, &w) in &row.capture {
		let i = RING_DIM + EMBLEM_DIM + 1 + cap as usize;
		v[i] += w * CAPTURE_WEIGHT;
	}
	v[EMBED_DIM - 1] += row.dots * DOTS_WEIGHT;
	add_epsilon(&mut v, &row.id);
	if l2_norm(&v) == 0.0 {
		add_epsilon(&mut v, &row.id);
	}
	l2_normalize(&mut v);
	v
}

pub fn cosine(a: &[f32], b: &[f32]) -> f32 {
	let n = a.len().min(b.len());
	let mut dot = 0.0f32;
	let mut na = 0.0f32;
	let mut nb = 0.0f32;
	for i in 0..n {
		dot += a[i] * b[i];
		na += a[i] * a[i];
		nb += b[i] * b[i];
	}
	if na == 0.0 || nb == 0.0 {
		return 0.0;
	}
	(dot / (na.sqrt() * nb.sqrt())).clamp(-1.0, 1.0)
}

fn write_extras(
	v: &mut [f32],
	n_rings: f32,
	capture: CaptureMark,
	dots: u8,
	emblem: Emblem,
	n_rings_w: f32,
	capture_w: f32,
	dots_w: f32,
) {
	if let Some(i) = emblem_axis(emblem) {
		v[RING_DIM + i] += EMBLEM_WEIGHT;
	}
	v[RING_DIM + EMBLEM_DIM] += n_rings_w * N_RINGS_WEIGHT * (n_rings / 3.0);
	let cap_i = RING_DIM + EMBLEM_DIM + 1 + capture as usize;
	v[cap_i] += capture_w * CAPTURE_WEIGHT;
	v[EMBED_DIM - 1] += dots_w * DOTS_WEIGHT * (f32::from(dots.clamp(1, 3)) / 3.0);
}

fn add_kronecker(atoms: &mut [f32; RING_DIM], rgb: [f32; 3], typ: RingType, w: f32) {
	let t = typ as usize;
	for i in 0..3 {
		atoms[t * 3 + i] += w * rgb[i];
	}
}

fn add_color_anchor(atoms: &mut [f32; RING_DIM], color: TileColor, w: f32) {
	let rgb = color.rgb();
	for typ in [RingType::Normal, RingType::Zag, RingType::Braid, RingType::Bead] {
		add_kronecker(atoms, rgb, typ, w * 0.25);
	}
}

fn add_type_anchor(atoms: &mut [f32; RING_DIM], typ: RingType, w: f32) {
	let mut rgb = [0.0f32; 3];
	for c in TileColor::ALL {
		let r = c.rgb();
		rgb[0] += r[0];
		rgb[1] += r[1];
		rgb[2] += r[2];
	}
	let n = TileColor::ALL.len() as f32;
	rgb = [rgb[0] / n, rgb[1] / n, rgb[2] / n];
	add_kronecker(atoms, rgb, typ, w);
}

fn emblem_axis(emblem: Emblem) -> Option<usize> {
	match emblem {
		Emblem::None => None,
		Emblem::CrescentMoon => Some(0),
		Emblem::TwoZagZs => Some(1),
		Emblem::Blocker => Some(2),
		Emblem::Rosette => Some(3),
	}
}

fn add_epsilon(v: &mut [f32], id: &str) {
	let mut h = 0xcbf2_9ce4_8422_2325u64;
	for &b in id.as_bytes() {
		h ^= u64::from(b);
		h = h.wrapping_mul(0x1000_0000_01B3);
	}
	for slot in v.iter_mut() {
		h ^= h >> 33;
		h = h.wrapping_mul(0xff51_afd7_ed55_8ccd);
		let unit = ((h >> 32) as i32 as f32) / (i32::MAX as f32);
		*slot += EPSILON_SCALE * unit;
	}
}

fn l2_norm(v: &[f32]) -> f32 {
	v.iter().map(|x| x * x).sum::<f32>().sqrt()
}

fn l2_normalize(v: &mut [f32]) {
	let n = l2_norm(v);
	if n > 0.0 {
		for x in v.iter_mut() {
			*x /= n;
		}
	}
}

impl PartialOrd for CoreKey {
	fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
		Some(self.cmp(other))
	}
}

impl Ord for CoreKey {
	fn cmp(&self, other: &Self) -> std::cmp::Ordering {
		self.rings
			.len()
			.cmp(&other.rings.len())
			.then_with(|| {
				self.rings
					.iter()
					.map(|(c, t)| (*c as u8, *t as u8))
					.cmp(other.rings.iter().map(|(c, t)| (*c as u8, *t as u8)))
			})
			.then(self.dots.cmp(&other.dots))
			.then((self.capture as u8).cmp(&(other.capture as u8)))
			.then((self.emblem as u8).cmp(&(other.emblem as u8)))
	}
}

#[cfg(test)]
mod tests {
	#![allow(clippy::unwrap_used, clippy::expect_used)]

	use super::*;
	use crate::core_pass::{CorePass, TileId};
	use crate::game::Player;

	fn pass() -> CharacterPass<JsonCharacterTables> {
		CharacterPass::v1().unwrap()
	}

	fn core_with(
		rings: &[(TileColor, RingType)],
		dots: u8,
		capture: CaptureMark,
		emblem: Emblem,
	) -> CoreTile {
		CoreTile {
			ring_type: rings.iter().map(|r| r.1).collect(),
			rings: rings.iter().map(|r| r.0).collect(),
			dots,
			capture,
			emblem,
		}
	}

	#[test]
	fn json_tables_load_and_cover_the_catalog() {
		let tables = JsonCharacterTables::load_v1().unwrap();
		assert_eq!(tables.version(), 1);
		assert_eq!(tables.tau(), 0.32);
		assert!(tables.k().is_none());
		assert_eq!(tables.fallback_ids().len(), 3);
		assert!(tables.fallback_ids().contains(&"zaster".to_string()));
		assert!(tables.fallback_ids().contains(&"nocunist".to_string()));
		assert!(tables.fallback_ids().contains(&"faceless".to_string()));
		assert!(tables.embedding_rows().iter().any(|r| r.id == "faceless"));
		let lib = CharacterLibrary::load().unwrap();
		let mut covered: BTreeSet<&str> =
			tables.fallback_ids().iter().map(String::as_str).collect();
		for row in tables.embedding_rows() {
			assert!(lib.get(&row.id).is_some(), "embedding {}", row.id);
			covered.insert(&row.id);
		}
		assert_eq!(covered.len(), lib.len(), "every catalog id is embedding or fallback");
	}

	#[test]
	fn json_parse_rejects_all_zero_row_and_overlap() {
		let err = JsonCharacterTables::parse(
			r#"{"version":1,"tau":0.5,"k":null}"#,
			r#"{"maps":[]}"#,
			r#"{"rows":[{"id":"tissen"}]}"#,
			r#"{"ids":["nocunist"]}"#,
		)
		.unwrap_err();
		assert!(err.contains("all-zero"), "{err}");

		let err = JsonCharacterTables::parse(
			r#"{"version":1,"tau":0.5,"k":null}"#,
			r#"{"maps":[]}"#,
			r#"{"rows":[{"id":"nocunist","color":{"blue":1.0}}]}"#,
			r#"{"ids":["nocunist"]}"#,
		)
		.unwrap_err();
		assert!(err.contains("disjoint") || err.contains("also appears"), "{err}");

		JsonCharacterTables::parse(
			r#"{"version":1,"tau":0.5,"k":null}"#,
			r#"{"maps":[]}"#,
			r#"{"rows":[{"id":"faceless","type":{"normal":1.0}}]}"#,
			r#"{"ids":["faceless","zaster"]}"#,
		)
		.expect("faceless may sit in both embedding and fallback");
	}

	#[test]
	fn color_only_row_is_not_the_origin() {
		let row = SparseCharacterRow {
			id: "westerlit".into(),
			color: BTreeMap::from([(TileColor::Teal, 2.0)]),
			ring_type: BTreeMap::new(),
			emblem: BTreeMap::new(),
			n_rings: 0.0,
			capture: BTreeMap::new(),
			dots: 0.0,
		};
		let v = encode_character(&row);
		assert_eq!(v.len(), EMBED_DIM);
		assert!(l2_norm(&v) > 0.99, "normalized color-only row must not be the origin");
		let origin = vec![0.0; EMBED_DIM];
		assert!(cosine(&v, &origin) == 0.0 || l2_norm(&origin) == 0.0);
	}

	#[test]
	fn specific_mapping_hit_picks_id_mod_len() {
		let p = pass();
		let tile = core_with(
			&[
				(TileColor::Green, RingType::Braid),
				(TileColor::Blue, RingType::Braid),
				(TileColor::Teal, RingType::Normal),
			],
			3,
			CaptureMark::Dot,
			Emblem::Rosette,
		);
		let a = p.assign(&tile, 0);
		let b = p.assign(&tile, 1);
		let c = p.assign(&tile, 2);
		assert_eq!(a.as_str(), "westerlit");
		assert_eq!(b.as_str(), "nocunfall");
		assert_eq!(c.as_str(), "westerlit");
		assert_eq!(p.assign(&tile, 0), p.assign(&tile, 0));
	}

	#[test]
	fn specific_mapping_ignores_slot_order() {
		let p = pass();
		let a = core_with(
			&[
				(TileColor::Teal, RingType::Normal),
				(TileColor::Green, RingType::Braid),
				(TileColor::Blue, RingType::Braid),
			],
			3,
			CaptureMark::Dot,
			Emblem::Rosette,
		);
		assert_eq!(p.assign(&a, 0).as_str(), "westerlit");
	}

	#[test]
	fn embedding_nearest_neighborhood_prefers_matching_color() {
		let teal = core_with(
			&[(TileColor::Teal, RingType::Normal)],
			1,
			CaptureMark::Triangle,
			Emblem::None,
		);
		let maroon = core_with(
			&[(TileColor::Maroon, RingType::Normal)],
			1,
			CaptureMark::Triangle,
			Emblem::None,
		);
		let bundled = pass();
		let teal_row =
			bundled.tables.embedding_rows().iter().find(|r| r.id == "westerlit").unwrap();
		let maroon_row =
			bundled.tables.embedding_rows().iter().find(|r| r.id == "magrenedl").unwrap();
		let tt = cosine(&encode_tile(&teal), &encode_character(teal_row));
		let tm = cosine(&encode_tile(&teal), &encode_character(maroon_row));
		let mm = cosine(&encode_tile(&maroon), &encode_character(maroon_row));
		let mt = cosine(&encode_tile(&maroon), &encode_character(teal_row));
		assert!(tt > tm, "teal tile vs teal {tt} vs maroon {tm}");
		assert!(mm > mt, "maroon tile vs maroon {mm} vs teal {mt}");

		// Two-row catalog so |S|=1 for a strong color match (RFC pick is
		// neighborhood then id % len, not argmin). τ sits between the
		// same-color and cross-color scores measured above.
		let tau = (tt.min(mm) + tm.max(mt)) * 0.5;
		let tables = JsonCharacterTables::parse(
			&format!(r#"{{"version":1,"tau":{tau},"k":null}}"#),
			r#"{"maps":[]}"#,
			r#"{"rows":[{"id":"westerlit","color":{"teal":2.0}},{"id":"magrenedl","color":{"maroon":2.0}}]}"#,
			r#"{"ids":["zaster"]}"#,
		)
		.unwrap();
		let p = CharacterPass::new(tables, CharacterLibrary::load().unwrap()).unwrap();
		assert_eq!(p.assign(&teal, 0).as_str(), "westerlit");
		assert_eq!(p.assign(&teal, 99).as_str(), "westerlit");
		assert_eq!(p.assign(&maroon, 0).as_str(), "magrenedl");
		assert_eq!(p.assign(&maroon, 11).as_str(), "magrenedl");
	}

	#[test]
	fn specific_mapping_can_assign_faceless() {
		let p = pass();
		let tile =
			core_with(&[(TileColor::Maroon, RingType::Zag)], 3, CaptureMark::Dot, Emblem::None);
		assert!(p.assign(&tile, 0).is_faceless());
		assert!(p.assign(&tile, 99).is_faceless());
		let mixed = core_with(
			&[(TileColor::Blue, RingType::Normal), (TileColor::Blue, RingType::Normal)],
			1,
			CaptureMark::Dot,
			Emblem::None,
		);
		assert_eq!(p.assign(&mixed, 0).as_str(), "tissen");
		assert!(p.assign(&mixed, 1).is_faceless());
	}

	#[test]
	fn embedding_neighborhood_can_assign_faceless() {
		let tables = JsonCharacterTables::parse(
			r#"{"version":1,"tau":0.32,"k":null}"#,
			r#"{"maps":[]}"#,
			r#"{"rows":[{"id":"faceless","type":{"normal":2.0},"color":{"green":1.0}}]}"#,
			r#"{"ids":["faceless","zaster"]}"#,
		)
		.unwrap();
		let p = CharacterPass::new(tables, CharacterLibrary::load().unwrap()).unwrap();
		let tile = core_with(
			&[(TileColor::Green, RingType::Normal)],
			1,
			CaptureMark::Triangle,
			Emblem::None,
		);
		assert!(p.assign(&tile, 0).is_faceless(), "faceless embedding row is in the neighborhood");
		assert!(p.assign(&tile, 7).is_faceless());
	}

	#[test]
	fn tau_miss_uses_fallback_hash() {
		let tables = JsonCharacterTables::parse(
			r#"{"version":1,"tau":0.99,"k":null}"#,
			r#"{"maps":[]}"#,
			r#"{"rows":[{"id":"westerlit","color":{"teal":2.0}}]}"#,
			r#"{"ids":["zaster","nocunist"]}"#,
		)
		.unwrap();
		let p = CharacterPass::new(tables, CharacterLibrary::load().unwrap()).unwrap();
		let tile = core_with(
			&[(TileColor::Green, RingType::Normal)],
			1,
			CaptureMark::Triangle,
			Emblem::None,
		);
		let a = p.assign(&tile, 7);
		let b = p.assign(&tile, 7);
		assert_eq!(a, b);
		assert!(a.as_str() == "zaster" || a.as_str() == "nocunist");
		let other = p.assign(&tile, 8);
		// hash(id) % 2 should vary across nearby ids often enough; if not,
		// at least both remain in the fallback set.
		assert!(other.as_str() == "zaster" || other.as_str() == "nocunist");
	}

	#[test]
	fn fallback_can_assign_faceless() {
		let tables = JsonCharacterTables::parse(
			r#"{"version":1,"tau":0.99,"k":null}"#,
			r#"{"maps":[]}"#,
			r#"{"rows":[{"id":"westerlit","color":{"teal":2.0}}]}"#,
			r#"{"ids":["faceless"]}"#,
		)
		.unwrap();
		let p = CharacterPass::new(tables, CharacterLibrary::load().unwrap()).unwrap();
		let tile = core_with(
			&[(TileColor::Green, RingType::Normal)],
			1,
			CaptureMark::Triangle,
			Emblem::None,
		);
		assert!(p.assign(&tile, 0).is_faceless());
		assert!(p.assign(&tile, 7).is_faceless());
	}

	#[test]
	fn assign_is_deterministic_for_core_pass_ids() {
		let p = pass();
		let cfg = CorePass::base();
		for id in 0..200u32 {
			let tile = cfg.run(TileId(id));
			assert_eq!(p.assign(&tile, id), p.assign(&tile, id));
		}
	}

	#[test]
	fn generated_tile_always_gets_a_catalog_character() {
		let p = pass();
		let cfg = CorePass::base();
		let lib = CharacterLibrary::load().unwrap();
		for id in 0..400u32 {
			let core = cfg.run(TileId(id));
			let ch = p.assign(&core, id);
			assert!(lib.get(ch.as_str()).is_some(), "{}", ch.as_str());
			let tile = core.into_tile(id, Player::One, ch);
			assert_eq!(tile.character, ch);
			assert_eq!(
				tile.kind_index(),
				CoreTile::from_tile(&tile)
					.into_tile(id, Player::One, CharacterId::faceless())
					.kind_index()
			);
		}
	}

	#[test]
	fn version_is_frozen_at_one() {
		assert_eq!(pass().version(), 1);
	}

	#[test]
	fn snapshot_tables_match_json_assign() {
		let json = JsonCharacterTables::load_v1().unwrap();
		let snap = SnapshotCharacterTables::from_tables(&json).unwrap();
		let cfg = CorePass::base();
		for id in 0..80u32 {
			let tile = cfg.run(TileId(id));
			assert_eq!(assign_character(&json, &tile, id), assign_character(&snap, &tile, id));
		}
	}

	#[test]
	fn bundled_pass_spreads_across_the_catalog() {
		let p = pass();
		let cfg = CorePass::base();
		let lib = CharacterLibrary::load().unwrap();
		let n = 20_000u32;
		let mut counts: BTreeMap<String, u32> = BTreeMap::new();
		for id in 0..n {
			let ch = p.assign(&cfg.run(TileId(id)), id);
			*counts.entry(ch.as_str().to_string()).or_insert(0) += 1;
		}
		let mut ranked: Vec<_> = counts.iter().collect();
		ranked.sort_by(|a, b| b.1.cmp(a.1).then(a.0.cmp(b.0)));
		for (id, c) in &ranked {
			eprintln!("{id:>14} {c:>5} ({:.2}%)", 100.0 * f64::from(**c) / f64::from(n));
		}
		let max = counts.values().copied().max().unwrap_or(0);
		let max_share = f64::from(max) / f64::from(n);
		assert!(max_share < 0.12, "one character dominates {max_share:.3}: {counts:?}");
		for ch in lib.all() {
			let c = counts.get(&ch.id).copied().unwrap_or(0);
			assert!(c > 0, "catalog id {} never assigned in {n} samples", ch.id);
		}
	}
}
