//! Game host: admits signed `game/<id>` transactions, folds them through the
//! `apa-kan-tx` state machine, and commits them to an `apa-kan-ledger` store.
//!
//! Every mutation is one [`SignedTx`] posted to `POST /v1/tx`; every read is a
//! view derived from the folded state. Submission order:
//!
//! 1. look up the signer key's status in the key directory (the account
//!    folds, see [`crate::accounts`]), then [`admit`] (domain, chain,
//!    signature, key authority);
//! 2. commit through [`Streams`]: lock, refresh, [`GameState::apply`], append
//!    (CAS on seq + prev), swap, acknowledge.
//!
//! Loading a stream replays its committed transactions, re-verifying every
//! signature and link.

use std::sync::Arc;

use apa_kan_account::crypto::VerifyingKeyBytes;
use apa_kan_account::AccountId;
use apa_kan_ledger::LedgerStore;
use apa_kan_tx::{
	admit, game_id_of, game_stream, GameState, KeyStatus, SignedTx, TxError, TxReceipt, TxResponse,
	TX_DOMAIN,
};
use apa_kan_view::HostedGameView;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

pub use crate::streams::CommittedTx;
use crate::streams::{host_err, ledger_err, now_unix, Machine, Streams};

/// Refuse new game streams past this many.
pub const DEFAULT_MAX_GAMES: usize = 10_000;
/// Chain id when `APAKAN_CHAIN_ID` is unset.
pub const DEFAULT_CHAIN_ID: &str = "apa-kan-local";

/// Account key lookups for admission (the account crate's indirection).
#[async_trait]
pub trait KeyDirectory: Send + Sync {
	async fn status(
		&self,
		account: &AccountId,
		vk: &VerifyingKeyBytes,
	) -> Result<KeyStatus, String>;
}

/// No registered accounts: only self-certifying genesis keys pass.
pub struct GenesisOnly;

#[async_trait]
impl KeyDirectory for GenesisOnly {
	async fn status(&self, _: &AccountId, _: &VerifyingKeyBytes) -> Result<KeyStatus, String> {
		Ok(KeyStatus::UnknownAccount)
	}
}

/// `GET /v1/ledger`: what a client needs to build transactions.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LedgerInfo {
	pub domain: String,
	pub chain: String,
	pub backend: String,
	/// Hex operator account (signs recovery challenges; administers roles).
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub operator: Option<String>,
}

/// Ledger URL from the environment: `APAKAN_LEDGER_URL` (`memory`,
/// `sqlite:<path>`, `postgres://…`), else `APA_KAN_DATABASE_URL` (share the
/// account database), else a SQLite file in the temp dir.
pub fn ledger_url_from_env() -> String {
	let nonempty = |k: &str| std::env::var(k).ok().filter(|v| !v.trim().is_empty());
	nonempty("APAKAN_LEDGER_URL")
		.or_else(|| {
			// main.rs accepts a bare path for the account DB; mirror that.
			nonempty("APA_KAN_DATABASE_URL").map(|u| {
				if u.contains(':') && !u.starts_with('/') {
					u
				} else {
					format!("sqlite:{u}")
				}
			})
		})
		.unwrap_or_else(|| {
			format!("sqlite:{}", std::env::temp_dir().join("apa-kan-ledger.db").display())
		})
}

/// `APAKAN_CHAIN_ID`, else [`DEFAULT_CHAIN_ID`].
pub fn chain_id_from_env() -> String {
	std::env::var("APAKAN_CHAIN_ID")
		.ok()
		.map(|s| s.trim().to_string())
		.filter(|s| !s.is_empty())
		.unwrap_or_else(|| DEFAULT_CHAIN_ID.into())
}

/// [`Machine`] for `game/<id>` streams.
pub struct GameMachine;

impl Machine for GameMachine {
	type State = GameState;

	fn head(s: &GameState) -> (u64, &str) {
		(s.next_seq, s.head.as_str())
	}

	fn apply(
		&self,
		state: Option<&GameState>,
		tx: &SignedTx,
		id: &[u8; 32],
		_now: Option<i64>,
	) -> Result<(GameState, TxReceipt), TxError> {
		GameState::apply(state, tx, id)
	}
}

pub struct GameHost {
	streams: Streams<GameMachine>,
	keys: Arc<dyn KeyDirectory>,
	max_games: usize,
}

impl Default for GameHost {
	/// Memory ledger, genesis-only keys, default chain (tests).
	fn default() -> Self {
		Self::new(
			Arc::new(apa_kan_ledger::memory::MemoryLedger::default()),
			Arc::new(GenesisOnly),
			DEFAULT_CHAIN_ID,
			DEFAULT_MAX_GAMES,
		)
	}
}

impl GameHost {
	pub fn new(
		ledger: Arc<dyn LedgerStore>,
		keys: Arc<dyn KeyDirectory>,
		chain: impl Into<String>,
		max_games: usize,
	) -> Self {
		Self { streams: Streams::new(ledger, chain, GameMachine), keys, max_games }
	}

	/// Same host with a different key directory.
	pub fn with_keys(mut self, keys: Arc<dyn KeyDirectory>) -> Self {
		self.keys = keys;
		self
	}

	pub fn chain(&self) -> &str {
		self.streams.chain()
	}

	pub fn ledger(&self) -> &Arc<dyn LedgerStore> {
		self.streams.ledger()
	}

	pub fn info(&self) -> LedgerInfo {
		LedgerInfo {
			domain: TX_DOMAIN.into(),
			chain: self.chain().into(),
			backend: self.ledger().backend().into(),
			operator: None,
		}
	}

	pub async fn view(&self, id: &str) -> Result<HostedGameView, TxError> {
		let stream = game_stream(id);
		game_id_of(&stream).ok_or(TxError::NotFound)?;
		self.streams
			.current(&stream)
			.await?
			.as_ref()
			.map(GameState::view)
			.ok_or(TxError::NotFound)
	}

	/// Committed transactions of a game from `from` on (resync / audit / p2p).
	pub async fn log(&self, id: &str, from: u64) -> Result<Vec<CommittedTx>, TxError> {
		let stream = game_stream(id);
		game_id_of(&stream).ok_or(TxError::NotFound)?;
		self.streams.log(&stream, from).await
	}

	/// Admit, apply, commit, then acknowledge one transaction.
	pub async fn submit(&self, tx: SignedTx) -> Result<TxResponse, TxError> {
		let stream = tx.body.stream.clone();
		game_id_of(&stream).ok_or_else(|| TxError::UnsupportedStream { stream: stream.clone() })?;
		let key = self.keys.status(&tx.body.signer, &tx.body.signer_vk).await.map_err(host_err)?;
		let chain = self.chain().to_string();
		// ML-DSA verification is CPU work; keep it off the async workers.
		let (tx, id) =
			tokio::task::spawn_blocking(move || admit(&tx, &chain, key).map(|id| (tx, id)))
				.await
				.map_err(|e| host_err(e.to_string()))??;
		if tx.body.seq == 0
			&& self.ledger().count_streams("game/").await.map_err(ledger_err)?
				>= self.max_games as u64
		{
			return Err(host_err("game capacity reached"));
		}
		let (next, receipt) = self.streams.commit(&tx, &id, Some(now_unix())).await?;
		Ok(TxResponse { receipt, view: next.view() })
	}

	/// Forget cached state (tests: simulate a restart on the same ledger).
	pub async fn clear_cache(&self) {
		self.streams.clear_cache().await;
	}
}

#[cfg(test)]
mod tests {
	#![allow(clippy::unwrap_used, clippy::expect_used)]

	use super::*;
	use apa_kan_account::Seed;
	use apa_kan_ledger::memory::MemoryLedger;
	use apa_kan_tx::{TxPayload, TxSigner, GENESIS_PREV};
	use apa_kan_view::{ActionWire, GamePhase};

	fn signer(n: u8) -> TxSigner {
		TxSigner::from_seed(&Seed::from_bytes([n; 32]))
	}

	fn next_tx(who: &TxSigner, view: &HostedGameView, payload: TxPayload) -> SignedTx {
		who.sign(
			&view.ledger.chain,
			&view.ledger.stream,
			view.ledger.seq,
			&view.ledger.head,
			payload,
		)
		.unwrap()
	}

	async fn start(host: &GameHost, a: &TxSigner, b: &TxSigner, id: &str) -> HostedGameView {
		let create = a
			.sign(
				host.chain(),
				&game_stream(id),
				0,
				GENESIS_PREV,
				TxPayload::GameCreate {
					seat: 1,
					label: "ana".into(),
					seed_commit: commit(id),
					preview_pk: None,
				},
			)
			.unwrap();
		let v = host.submit(create).await.unwrap().view;
		assert_eq!(v.phase, GamePhase::Open);
		let v = host
			.submit(next_tx(b, &v, TxPayload::GameJoin { label: "bo".into(), preview_pk: None }))
			.await
			.unwrap()
			.view;
		assert_eq!(v.phase, GamePhase::Revealing);
		host.submit(next_tx(a, &v, TxPayload::GameReveal { salt: hex::encode(SALT) }))
			.await
			.unwrap()
			.view
	}

	const SALT: [u8; 32] = [9; 32];

	fn commit(id: &str) -> String {
		apa_kan_tx::seed_commitment(&game_stream(id), &SALT)
	}

	fn first_move(view: &HostedGameView) -> ActionWire {
		view.game.as_ref().unwrap().legal[0]
	}

	fn mover<'a>(view: &HostedGameView, a: &'a TxSigner, b: &'a TxSigner) -> &'a TxSigner {
		let to_move = view.game.as_ref().unwrap().to_move;
		if view.seat_of_account(&a.account().to_hex()) == Some(to_move) {
			a
		} else {
			b
		}
	}

	#[tokio::test]
	async fn create_join_play_through_one_endpoint() {
		let host = GameHost::default();
		let (a, b) = (signer(1), signer(2));
		let mut v = start(&host, &a, &b, "g1").await;
		assert_eq!(v.phase, GamePhase::Playing);
		for _ in 0..4 {
			let who = mover(&v, &a, &b);
			v = host
				.submit(next_tx(who, &v, TxPayload::GameMove { action: first_move(&v) }))
				.await
				.unwrap()
				.view;
		}
		assert_eq!(v.ledger.seq, 7);
		assert_eq!(v.game.as_ref().unwrap().seq, 4);
		assert_eq!(host.view("g1").await.unwrap(), v);
		let log = host.log("g1", 3).await.unwrap();
		assert_eq!(log.len(), 4);
		assert_eq!(log[0].seq, 3);
		assert_eq!(host.view("missing").await.unwrap_err(), TxError::NotFound);
		assert_eq!(host.view("BAD ID").await.unwrap_err(), TxError::NotFound);
	}

	#[tokio::test]
	async fn wrong_chain_bad_sig_and_foreign_account_are_refused() {
		let host = GameHost::default();
		let (a, b) = (signer(1), signer(2));
		let v = start(&host, &a, &b, "g2").await;
		let who = mover(&v, &a, &b);
		let mut tx = next_tx(who, &v, TxPayload::GameMove { action: first_move(&v) });
		tx.body.chain = "elsewhere".into();
		assert!(matches!(host.submit(tx).await, Err(TxError::WrongChain { .. })));
		let mut tx = next_tx(who, &v, TxPayload::GameMove { action: first_move(&v) });
		tx.body.payload = TxPayload::GameResign;
		assert_eq!(host.submit(tx).await.unwrap_err(), TxError::BadSignature);
		let outsider = signer(3);
		let tx = next_tx(&outsider, &v, TxPayload::GameMove { action: first_move(&v) });
		assert_eq!(host.submit(tx).await.unwrap_err(), TxError::Unauthorized);
		let tx = outsider
			.sign(host.chain(), "account/x", 0, GENESIS_PREV, TxPayload::GameResign)
			.unwrap();
		assert!(matches!(host.submit(tx).await, Err(TxError::UnsupportedStream { .. })));
	}

	struct Revoked;

	#[async_trait]
	impl KeyDirectory for Revoked {
		async fn status(&self, _: &AccountId, _: &VerifyingKeyBytes) -> Result<KeyStatus, String> {
			Ok(KeyStatus::Inactive)
		}
	}

	#[tokio::test]
	async fn revoked_keys_cannot_act() {
		let host = GameHost::default().with_keys(Arc::new(Revoked));
		let a = signer(1);
		let tx = a
			.sign(
				host.chain(),
				"game/r",
				0,
				GENESIS_PREV,
				TxPayload::GameCreate {
					seat: 1,
					label: "a".into(),
					seed_commit: commit("r"),
					preview_pk: None,
				},
			)
			.unwrap();
		assert_eq!(host.submit(tx).await.unwrap_err(), TxError::KeyNotAuthorized);
	}

	/// Acknowledged transactions survive dropping the cache, and a second host
	/// on the same ledger sees them and rejects a replay.
	#[tokio::test]
	async fn durable_and_shared_through_the_ledger() {
		let ledger: Arc<dyn LedgerStore> = Arc::new(MemoryLedger::default());
		let mk = || GameHost::new(ledger.clone(), Arc::new(GenesisOnly), "c", 10);
		let (h1, h2) = (mk(), mk());
		let (a, b) = (signer(1), signer(2));
		let mut v = start(&h1, &a, &b, "g3").await;
		for _ in 0..3 {
			let who = mover(&v, &a, &b);
			v = h1
				.submit(next_tx(who, &v, TxPayload::GameMove { action: first_move(&v) }))
				.await
				.unwrap()
				.view;
		}
		h1.clear_cache().await;
		assert_eq!(h1.view("g3").await.unwrap(), v);
		let who = mover(&v, &a, &b);
		let tx = next_tx(who, &v, TxPayload::GameMove { action: first_move(&v) });
		let next = h2.submit(tx.clone()).await.unwrap().view;
		assert_eq!(h1.view("g3").await.unwrap(), next);
		assert_eq!(h1.submit(tx).await.unwrap_err(), TxError::StaleSeq { current: 7, got: 6 });
	}

	/// Concurrent submits at one seq across hosts: exactly one is acknowledged.
	#[tokio::test]
	async fn concurrent_same_seq_commits_once() {
		let ledger: Arc<dyn LedgerStore> = Arc::new(MemoryLedger::default());
		let hosts: Vec<Arc<GameHost>> = (0..4)
			.map(|_| Arc::new(GameHost::new(ledger.clone(), Arc::new(GenesisOnly), "c", 10)))
			.collect();
		let (a, b) = (signer(1), signer(2));
		let v = start(&hosts[0], &a, &b, "g4").await;
		let who = mover(&v, &a, &b);
		let legal = v.game.as_ref().unwrap().legal.clone();
		let mut joins = Vec::new();
		for i in 0..8 {
			let host = hosts[i % hosts.len()].clone();
			let tx = next_tx(who, &v, TxPayload::GameMove { action: legal[i % legal.len()] });
			joins.push(tokio::spawn(async move { host.submit(tx).await }));
		}
		let mut ok = 0;
		for j in joins {
			match j.await.unwrap() {
				Ok(_) => ok += 1,
				Err(TxError::StaleSeq { current: 4, got: 3 }) => {}
				Err(e) => panic!("unexpected: {e:?}"),
			}
		}
		assert_eq!(ok, 1);
		assert_eq!(ledger.head("game/g4").await.unwrap().next_seq, 4);
	}

	#[tokio::test]
	async fn capacity_counts_game_streams() {
		let host = GameHost::new(Arc::new(MemoryLedger::default()), Arc::new(GenesisOnly), "c", 1);
		let a = signer(1);
		let mk = |id: &str| {
			a.sign(
				"c",
				&game_stream(id),
				0,
				GENESIS_PREV,
				TxPayload::GameCreate {
					seat: 1,
					label: "a".into(),
					seed_commit: commit(id),
					preview_pk: None,
				},
			)
			.unwrap()
		};
		host.submit(mk("one")).await.unwrap();
		assert!(matches!(host.submit(mk("two")).await, Err(TxError::Host { .. })));
	}
}
