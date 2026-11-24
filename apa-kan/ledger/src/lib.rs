//! Append-only, hash-linked transaction log.
//!
//! The log is a set of **streams** (`game/<id>` today; `account/<id>` and
//! `admin/...` later). Each stream is a chain of [`LedgerEntry`] rows keyed by
//! `(stream, seq)`, where `seq` counts from 0 and each entry names the hash of
//! the one before it (`prev`; the zero hash for `seq = 0`). Rows carry a
//! signed `apa-kan-tx` envelope as opaque JSON: this crate enforces ordering
//! and atomicity, never game or account rules. State is the fold of a stream
//! through the deterministic state machine in `apa-kan-tx`.
//!
//! [`LedgerStore::append`] is a compare-and-swap on the stream head: it lands
//! only when `seq` is the stream's next number **and** `prev` is its current
//! head hash. The head bump and the row insert commit together, so of any
//! number of concurrent appends at one `seq` exactly one lands; the rest get
//! [`LedgerError::Conflict`]. Hosts commit before acknowledging.
//!
//! Backends: [`memory::MemoryLedger`] (tests), `sqlite::SqliteLedger`
//! (feature `sqlite`, single node) and `postgres::PostgresLedger` (feature
//! `postgres`, matches #97). [`connect`] picks one from a URL.

use std::sync::Arc;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

pub mod memory;
#[cfg(feature = "postgres")]
pub mod postgres;
#[cfg(any(feature = "sqlite", feature = "postgres"))]
mod sql;
#[cfg(feature = "sqlite")]
pub mod sqlite;

/// Hex of the all-zero hash: `prev` of every stream's first entry.
pub const GENESIS_PREV: &str = "0000000000000000000000000000000000000000000000000000000000000000";

#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum LedgerError {
	#[error("stream not found")]
	NotFound,
	/// `seq`/`prev` did not match the stream head (someone else committed
	/// that number first, or the caller is behind or on a fork).
	#[error("head conflict: stream is at {current}, append was for {got}")]
	Conflict { current: u64, got: u64 },
	#[error("invalid entry: {0}")]
	Invalid(String),
	#[error("corrupt stream: {0}")]
	Corrupt(String),
	#[error("storage: {0}")]
	Storage(String),
}

pub type Result<T> = std::result::Result<T, LedgerError>;

/// One committed transaction.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LedgerEntry {
	pub stream: String,
	pub seq: u64,
	/// Hex SHA3-256 id of the transaction (`apa-kan-tx` message id).
	pub hash: String,
	/// `hash` of entry `seq - 1`, or [`GENESIS_PREV`].
	pub prev: String,
	/// Signing account (hex), for indexing.
	pub signer: String,
	/// Payload type (e.g. `game.move`), for indexing.
	pub kind: String,
	/// The signed envelope, as submitted.
	pub tx_json: String,
	pub committed_at_unix: i64,
}

/// Where a stream's chain ends.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct StreamHead {
	/// Number of entries (the next `seq`).
	pub next_seq: u64,
	/// Hash of the last entry.
	pub head: String,
}

#[async_trait]
pub trait LedgerStore: Send + Sync {
	/// Atomically append iff `entry.seq`/`entry.prev` match the stream head.
	/// `seq = 0` (with `prev = GENESIS_PREV`) creates the stream.
	async fn append(&self, entry: &LedgerEntry) -> Result<()>;
	/// Entries with `seq >= from`, in order.
	async fn read(&self, stream: &str, from: u64) -> Result<Vec<LedgerEntry>>;
	async fn head(&self, stream: &str) -> Result<StreamHead>;
	/// Streams whose id starts with `prefix`.
	async fn count_streams(&self, prefix: &str) -> Result<u64>;
	fn backend(&self) -> &'static str;
}

fn is_hash(s: &str) -> bool {
	s.len() == 64 && s.bytes().all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
}

/// Shape checks every backend runs before an append.
pub fn validate_entry(e: &LedgerEntry) -> Result<()> {
	if e.stream.is_empty() || e.stream.len() > 160 {
		return Err(LedgerError::Invalid("stream id must be 1..=160 bytes".into()));
	}
	if !is_hash(&e.hash) || !is_hash(&e.prev) {
		return Err(LedgerError::Invalid("hash and prev must be 64 lowercase hex".into()));
	}
	if e.seq == 0 && e.prev != GENESIS_PREV {
		return Err(LedgerError::Invalid("seq 0 must have the genesis prev".into()));
	}
	if e.seq > i64::MAX as u64 {
		return Err(LedgerError::Invalid("seq too large".into()));
	}
	Ok(())
}

/// Check a read-back stream is dense and hash-linked from `from`.
pub fn check_chain(stream: &str, from: u64, entries: &[LedgerEntry]) -> Result<()> {
	for (i, e) in entries.iter().enumerate() {
		let want = from + i as u64;
		if e.seq != want || e.stream != stream {
			return Err(LedgerError::Corrupt(format!("{stream}: gap at seq {want}")));
		}
		if i > 0 && e.prev != entries[i - 1].hash {
			return Err(LedgerError::Corrupt(format!("{stream}: broken link at seq {want}")));
		}
		if want == 0 && e.prev != GENESIS_PREV {
			return Err(LedgerError::Corrupt(format!("{stream}: bad genesis prev")));
		}
	}
	Ok(())
}

/// Open a ledger from a URL: `memory`, `sqlite:<path>` / `sqlite::memory:`
/// (feature `sqlite`), or `postgres://…` (feature `postgres`).
pub async fn connect(url: &str) -> Result<Arc<dyn LedgerStore>> {
	let url = url.trim();
	if url == "memory" || url == "memory:" {
		return Ok(Arc::new(memory::MemoryLedger::default()));
	}
	if url.starts_with("postgres://") || url.starts_with("postgresql://") {
		#[cfg(feature = "postgres")]
		{
			return Ok(Arc::new(postgres::PostgresLedger::connect(url).await?));
		}
		#[cfg(not(feature = "postgres"))]
		{
			return Err(LedgerError::Invalid(
				"postgres URL given but the `postgres` feature is off".into(),
			));
		}
	}
	if let Some(rest) = url.strip_prefix("sqlite:") {
		#[cfg(feature = "sqlite")]
		{
			return Ok(Arc::new(sqlite::SqliteLedger::connect(rest).await?));
		}
		#[cfg(not(feature = "sqlite"))]
		{
			let _ = rest;
			return Err(LedgerError::Invalid(
				"sqlite URL given but the `sqlite` feature is off".into(),
			));
		}
	}
	Err(LedgerError::Invalid(format!("unsupported ledger URL: {url}")))
}

/// Backend-independent conformance checks; each backend's tests call these.
#[doc(hidden)]
pub mod conformance {
	use super::*;

	pub fn hash_of(stream: &str, seq: u64, salt: u8) -> String {
		use std::fmt::Write;
		use std::hash::{Hash, Hasher};
		let mut s = String::with_capacity(64);
		for lane in 0..4u8 {
			let mut h = std::collections::hash_map::DefaultHasher::new();
			(stream, seq, salt, lane).hash(&mut h);
			let _ = write!(s, "{:016x}", h.finish());
		}
		s
	}

	pub fn entry(stream: &str, seq: u64, prev: &str, salt: u8) -> LedgerEntry {
		LedgerEntry {
			stream: stream.into(),
			seq,
			hash: hash_of(stream, seq, salt),
			prev: prev.into(),
			signer: "ab".repeat(32),
			kind: if seq == 0 { "game.create".into() } else { "game.move".into() },
			tx_json: format!("{{\"seq\":{seq},\"salt\":{salt}}}"),
			committed_at_unix: 1_700_000_000 + seq as i64,
		}
	}

	/// Round trip, CAS conflicts (seq and prev), unknown streams, reads.
	pub async fn run(store: Arc<dyn LedgerStore>, prefix: &str) {
		let s = format!("game/{prefix}-1");
		assert_eq!(store.head(&s).await, Err(LedgerError::NotFound));
		let mut prev = GENESIS_PREV.to_string();
		let mut written = Vec::new();
		for seq in 0..8u64 {
			let e = entry(&s, seq, &prev, 0);
			store.append(&e).await.unwrap_or_else(|err| panic!("append {seq}: {err}"));
			// Same seq again (even a different tx) loses.
			let mut rival = entry(&s, seq, &prev, 1);
			rival.hash = hash_of(&s, seq, 9);
			assert_eq!(
				store.append(&rival).await,
				Err(LedgerError::Conflict { current: seq + 1, got: seq })
			);
			prev = e.hash.clone();
			written.push(e);
		}
		// Right seq, wrong prev (a fork) loses.
		let fork = entry(&s, 8, &hash_of(&s, 3, 0), 2);
		assert_eq!(store.append(&fork).await, Err(LedgerError::Conflict { current: 8, got: 8 }));
		// Skipping ahead loses.
		let ahead = entry(&s, 10, &prev, 0);
		assert_eq!(store.append(&ahead).await, Err(LedgerError::Conflict { current: 8, got: 10 }));
		// Appending to a missing stream past genesis is NotFound.
		let orphan = entry(&format!("game/{prefix}-none"), 1, &prev, 0);
		assert_eq!(store.append(&orphan).await, Err(LedgerError::NotFound));

		assert_eq!(store.head(&s).await, Ok(StreamHead { next_seq: 8, head: prev.clone() }));
		assert_eq!(store.read(&s, 0).await, Ok(written.clone()));
		assert_eq!(store.read(&s, 5).await, Ok(written[5..].to_vec()));
		assert_eq!(store.read(&s, 99).await, Ok(Vec::new()));
		assert_eq!(store.read(&format!("game/{prefix}-none"), 0).await, Err(LedgerError::NotFound));
		check_chain(&s, 0, &written).unwrap_or_else(|e| panic!("{e}"));

		let mut bad = entry(&s, 8, &prev, 0);
		bad.hash = "XYZ".into();
		assert!(matches!(store.append(&bad).await, Err(LedgerError::Invalid(_))));
		assert!(store.count_streams(&format!("game/{prefix}")).await.unwrap_or(0) >= 1);
		assert_eq!(store.count_streams("nothing/").await, Ok(0));
	}

	/// Concurrent genesis and concurrent same-seq appends: exactly one lands.
	pub async fn race(store: Arc<dyn LedgerStore>, prefix: &str) {
		let s = format!("game/{prefix}-race");
		for seq in 0..2u64 {
			let prev = if seq == 0 {
				GENESIS_PREV.to_string()
			} else {
				store.head(&s).await.map(|h| h.head).unwrap_or_default()
			};
			let mut handles = Vec::new();
			for salt in 0..16u8 {
				let store = store.clone();
				let e = entry(&s, seq, &prev, salt);
				handles.push(tokio::spawn(async move { store.append(&e).await }));
			}
			let (mut ok, mut lost) = (0, 0);
			for h in handles {
				match h.await {
					Ok(Ok(())) => ok += 1,
					Ok(Err(LedgerError::Conflict { .. })) => lost += 1,
					other => panic!("unexpected race result: {other:?}"),
				}
			}
			assert_eq!((ok, lost), (1, 15), "seq {seq}");
		}
		assert_eq!(store.head(&s).await.map(|h| h.next_seq), Ok(2));
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[tokio::test]
	async fn memory_conformance() {
		let store: Arc<dyn LedgerStore> = Arc::new(memory::MemoryLedger::default());
		conformance::run(store.clone(), "mem").await;
		conformance::race(store, "mem").await;
	}

	#[tokio::test]
	async fn connect_parses_urls() {
		assert_eq!(connect("memory").await.map(|s| s.backend()).ok(), Some("memory"));
		assert!(matches!(connect("redis://x").await, Err(LedgerError::Invalid(_))));
	}

	#[test]
	fn chain_checks() {
		let a = conformance::entry("s", 0, GENESIS_PREV, 0);
		let b = conformance::entry("s", 1, &a.hash, 0);
		assert!(check_chain("s", 0, &[a.clone(), b.clone()]).is_ok());
		let c = conformance::entry("s", 1, GENESIS_PREV, 0);
		assert!(check_chain("s", 0, &[a.clone(), c]).is_err());
		assert!(check_chain("s", 0, &[b]).is_err());
		let mut d = a;
		d.prev = "11".repeat(32);
		assert!(validate_entry(&d).is_err());
	}
}
