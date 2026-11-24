//! Generic ledger-backed stream host: cache, replay, and the commit step
//! shared by every stream kind (`game/`, `account/`, `admin/`).
//!
//! Commit order for one transaction (already admitted by the caller):
//! lock the stream's cache slot → refresh it if the ledger head moved →
//! apply on a clone → append to the ledger (CAS on seq + prev) → only then
//! swap the new state in and acknowledge.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use apa_kan_ledger::{LedgerEntry, LedgerError, LedgerStore};
use apa_kan_tx::{verify_self_contained, SignedTx, TxError, TxReceipt};
use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;

/// A deterministic transition function over one stream kind.
pub trait Machine: Send + Sync + 'static {
	type State: Clone + Send + Sync + 'static;

	/// `(next seq, head hash)` of a folded state.
	fn head(state: &Self::State) -> (u64, &str);

	/// Apply an already verified transaction. `now` is the host clock at
	/// admission, `None` on replay.
	fn apply(
		&self,
		state: Option<&Self::State>,
		tx: &SignedTx,
		id: &[u8; 32],
		now: Option<i64>,
	) -> Result<(Self::State, TxReceipt), TxError>;
}

/// One committed transaction, as served by the `/txs` views.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CommittedTx {
	pub seq: u64,
	pub hash: String,
	pub committed_at_unix: i64,
	pub tx: SignedTx,
}

pub fn host_err(reason: impl Into<String>) -> TxError {
	TxError::Host { reason: reason.into() }
}

pub fn ledger_err(e: LedgerError) -> TxError {
	match e {
		LedgerError::NotFound => TxError::NotFound,
		LedgerError::Conflict { current, got } => TxError::StaleSeq { current, got },
		other => host_err(other.to_string()),
	}
}

pub fn now_unix() -> i64 {
	SystemTime::now()
		.duration_since(UNIX_EPOCH)
		.map(|d| d.as_secs() as i64)
		.unwrap_or(0)
}

/// Verify a transaction's envelope and signature off the async workers.
pub async fn verify(tx: SignedTx, chain: &str) -> Result<(SignedTx, [u8; 32]), TxError> {
	let chain = chain.to_string();
	tokio::task::spawn_blocking(move || verify_self_contained(&tx, &chain).map(|id| (tx, id)))
		.await
		.map_err(|e| host_err(e.to_string()))?
}

type Slot<S> = Arc<Mutex<Option<S>>>;

pub struct Streams<M: Machine> {
	ledger: Arc<dyn LedgerStore>,
	chain: String,
	machine: Arc<M>,
	/// Folded streams; one lock per stream. Cleared past `max_cached`.
	cache: Mutex<HashMap<String, Slot<M::State>>>,
	max_cached: usize,
}

impl<M: Machine> Streams<M> {
	pub fn new(ledger: Arc<dyn LedgerStore>, chain: impl Into<String>, machine: M) -> Self {
		Self {
			ledger,
			chain: chain.into(),
			machine: Arc::new(machine),
			cache: Mutex::new(HashMap::new()),
			max_cached: 4_096,
		}
	}

	pub fn chain(&self) -> &str {
		&self.chain
	}

	pub fn ledger(&self) -> &Arc<dyn LedgerStore> {
		&self.ledger
	}

	pub fn machine(&self) -> &M {
		&self.machine
	}

	async fn slot(&self, stream: &str) -> Slot<M::State> {
		let mut cache = self.cache.lock().await;
		if let Some(s) = cache.get(stream) {
			return s.clone();
		}
		if cache.len() >= self.max_cached {
			cache.clear();
		}
		let slot: Slot<M::State> = Arc::new(Mutex::new(None));
		cache.insert(stream.to_string(), slot.clone());
		slot
	}

	/// Rebuild a stream from the ledger (`None` if it has no entries),
	/// re-verifying every signature and link.
	async fn load(&self, stream: &str) -> Result<Option<M::State>, TxError> {
		let entries = match self.ledger.read(stream, 0).await {
			Ok(e) => e,
			Err(LedgerError::NotFound) => return Ok(None),
			Err(e) => return Err(ledger_err(e)),
		};
		let chain = self.chain.clone();
		let machine = self.machine.clone();
		tokio::task::spawn_blocking(move || {
			let mut state: Option<M::State> = None;
			for e in &entries {
				let tx: SignedTx = serde_json::from_str(&e.tx_json)
					.map_err(|e| host_err(format!("stored tx does not parse: {e}")))?;
				let replayed = verify_self_contained(&tx, &chain)
					.and_then(|id| machine.apply(state.as_ref(), &tx, &id, None));
				let (next, _) = replayed
					.map_err(|e| host_err(format!("stored stream does not replay: {e}")))?;
				state = Some(next);
			}
			Ok(state)
		})
		.await
		.map_err(|e| host_err(e.to_string()))?
	}

	/// Bring a locked cached stream up to the ledger head.
	async fn refresh(&self, stream: &str, state: &mut Option<M::State>) -> Result<(), TxError> {
		let head = match self.ledger.head(stream).await {
			Ok(h) => Some(h),
			Err(LedgerError::NotFound) => None,
			Err(e) => return Err(ledger_err(e)),
		};
		let cached = state.as_ref().map(M::head);
		let fresh = head.as_ref().map(|h| (h.next_seq, h.head.as_str()));
		if cached != fresh {
			*state = self.load(stream).await?;
		}
		Ok(())
	}

	/// The stream's current fold (`None` if it has no transactions).
	pub async fn current(&self, stream: &str) -> Result<Option<M::State>, TxError> {
		let slot = self.slot(stream).await;
		let mut state = slot.lock().await;
		self.refresh(stream, &mut state).await?;
		Ok(state.clone())
	}

	/// Apply, commit, then swap in one admitted transaction.
	pub async fn commit(
		&self,
		tx: &SignedTx,
		id: &[u8; 32],
		now: Option<i64>,
	) -> Result<(M::State, TxReceipt), TxError> {
		let stream = &tx.body.stream;
		let slot = self.slot(stream).await;
		let mut state = slot.lock().await;
		self.refresh(stream, &mut state).await?;
		let (next, receipt) = self.machine.apply(state.as_ref(), tx, id, now)?;
		let entry = LedgerEntry {
			stream: stream.clone(),
			seq: tx.body.seq,
			hash: receipt.tx_id.clone(),
			prev: tx.body.prev.clone(),
			signer: tx.body.signer.to_hex(),
			kind: receipt.kind.clone(),
			tx_json: serde_json::to_string(tx).map_err(|e| host_err(e.to_string()))?,
			committed_at_unix: now.unwrap_or_else(now_unix),
		};
		match self.ledger.append(&entry).await {
			Ok(()) => {}
			Err(LedgerError::Conflict { current, got }) => {
				// Another writer committed this seq first; resync.
				if let Ok(fresh) = self.load(stream).await {
					*state = fresh;
				}
				return Err(TxError::StaleSeq { current, got });
			}
			Err(e) => return Err(ledger_err(e)),
		}
		*state = Some(next.clone());
		Ok((next, receipt))
	}

	/// Committed transactions of `stream` from `from` on.
	pub async fn log(&self, stream: &str, from: u64) -> Result<Vec<CommittedTx>, TxError> {
		let entries = self.ledger.read(stream, from).await.map_err(ledger_err)?;
		entries
			.into_iter()
			.map(|e| {
				Ok(CommittedTx {
					seq: e.seq,
					hash: e.hash,
					committed_at_unix: e.committed_at_unix,
					tx: serde_json::from_str(&e.tx_json)
						.map_err(|err| host_err(format!("stored tx does not parse: {err}")))?,
				})
			})
			.collect()
	}

	/// Forget cached state (tests: simulate a restart on the same ledger).
	pub async fn clear_cache(&self) {
		self.cache.lock().await.clear();
	}
}
