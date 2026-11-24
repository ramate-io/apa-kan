//! In-process backend with the same CAS semantics as the SQL ledgers.

use std::collections::{HashMap, HashSet};

use async_trait::async_trait;
use tokio::sync::Mutex;

use crate::*;

#[derive(Default)]
struct Inner {
	streams: HashMap<String, Vec<LedgerEntry>>,
	hashes: HashSet<String>,
}

#[derive(Default)]
pub struct MemoryLedger {
	inner: Mutex<Inner>,
}

#[async_trait]
impl LedgerStore for MemoryLedger {
	async fn append(&self, e: &LedgerEntry) -> Result<()> {
		validate_entry(e)?;
		let mut inner = self.inner.lock().await;
		let current = match inner.streams.get(&e.stream) {
			None if e.seq == 0 => 0,
			None => return Err(LedgerError::NotFound),
			Some(log) => {
				let n = log.len() as u64;
				if e.seq != n || log.last().map(|l| l.hash.as_str()) != Some(e.prev.as_str()) {
					return Err(LedgerError::Conflict { current: n, got: e.seq });
				}
				n
			}
		};
		if !inner.hashes.insert(e.hash.clone()) {
			return Err(LedgerError::Conflict { current, got: e.seq });
		}
		inner.streams.entry(e.stream.clone()).or_default().push(e.clone());
		Ok(())
	}

	async fn read(&self, stream: &str, from: u64) -> Result<Vec<LedgerEntry>> {
		let inner = self.inner.lock().await;
		let log = inner.streams.get(stream).ok_or(LedgerError::NotFound)?;
		Ok(log.iter().skip(from.min(log.len() as u64) as usize).cloned().collect())
	}

	async fn head(&self, stream: &str) -> Result<StreamHead> {
		let inner = self.inner.lock().await;
		let log = inner.streams.get(stream).ok_or(LedgerError::NotFound)?;
		let head = log.last().map(|e| e.hash.clone()).ok_or(LedgerError::NotFound)?;
		Ok(StreamHead { next_seq: log.len() as u64, head })
	}

	async fn count_streams(&self, prefix: &str) -> Result<u64> {
		let inner = self.inner.lock().await;
		Ok(inner.streams.keys().filter(|k| k.starts_with(prefix)).count() as u64)
	}

	fn backend(&self) -> &'static str {
		"memory"
	}
}
